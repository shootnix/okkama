//! Lazy files keep Rust buffers and OS resources outside the VM object graph.
use crate::host::{
    Error,
    handle::Handle,
    ProviderError,
    native_file::RawFile,
    native_fs::{fail, host},
    *,
};
use num_traits::ToPrimitive;
use okkama_runtime_abi::Native;
use std::{
    cell::{Cell, RefCell},
    io::{Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    rc::{Rc, Weak},
};
#[derive(Debug)]
pub(crate) struct FileState {
    pub(crate) path: PathBuf,
    /// A temporary file's path as the program spelled it.
    pub(crate) shown: String,
    pub(crate) temporary: bool,
    pub(crate) delete_on_close: Option<PathBuf>,
    flags: i64,
    encoding: String,
    binary: bool,
    /// `file_state_open`: the first text or byte operation picks `binary`.
    undecided: bool,
    channel: Option<RawFile>,
    buffer: Vec<u8>,
    start: usize,
    eof: bool,
    pub(crate) closed: bool,
    lock: Option<Rc<FileLock>>,
}
#[derive(Debug)]
pub(crate) struct FileLock {
    owner: Weak<RefCell<FileState>>,
    closed: Cell<bool>,
}
impl FileState {
    fn cleanup(&mut self, n: Native) -> Result<(), ProviderError> {
        if let Some(path) = &self.delete_on_close {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(host(n, e)),
            }
            self.delete_on_close = None;
        }
        Ok(())
    }
    /// Picks the mode of an undecided state; then fails an operation of the other kind.
    fn settle_mode(&mut self, n: Native, text: bool) -> Result<(), ProviderError> {
        if self.undecided {
            self.binary = !text;
            self.undecided = false;
        }
        if text == self.binary {
            let message = if text { "The file is open as bytes" } else { "The file is open as text" };
            return Err(fail(n, "invalid_input", message));
        }
        Ok(())
    }
    pub(crate) fn temporary(path: PathBuf, shown: String, channel: RawFile) -> Self {
        Self {
            delete_on_close: Some(path.clone()),
            path,
            shown,
            temporary: true,
            flags: 3,
            encoding: String::new(),
            binary: true,
            undecided: false,
            channel: Some(channel),
            buffer: vec![],
            start: 0,
            eof: false,
            closed: false,
            lock: None,
        }
    }
    pub(crate) fn seal(&mut self, n: Native) -> Result<(), ProviderError> {
        if let Some(mut channel) = self.channel.take() {
            channel.close(n)?;
        }
        if let Some(lock) = self.lock.take() {
            lock.closed.set(true);
        }
        self.closed = true;
        Ok(())
    }
    fn opened(&mut self, n: Native, cwd: &Path) -> Result<&mut RawFile, ProviderError> {
        if self.closed {
            return Err(fail(n, "already_closed", "File is closed"));
        }
        if self.channel.is_none() {
            self.channel = Some(RawFile::create(
                n,
                &cwd.join(&self.path),
                self.flags,
                0o666,
            )?);
        }
        Ok(self.channel.as_mut().expect("opened"))
    }
    fn prepare(
        &mut self,
        n: Native,
        cwd: &Path,
        text: bool,
        write: bool,
    ) -> Result<(), ProviderError> {
        if self.closed {
            return Err(fail(n, "already_closed", "File is closed"));
        }
        self.settle_mode(n, text)?;
        if self.flags & if write { 2 } else { 1 } == 0 {
            return Err(fail(
                n,
                "invalid_input",
                "File mode does not permit operation",
            ));
        }
        self.opened(n, cwd)?;
        if write && self.buffer.len() > self.start {
            let back = (self.buffer.len() - self.start) as i64;
            self.opened(n, cwd)?
                .open(n)?
                .seek(SeekFrom::Current(-back))
                .map_err(|e| host(n, e))?;
            self.buffer.clear();
            self.start = 0;
            self.eof = false;
        }
        Ok(())
    }
    fn fill(&mut self, n: Native, cwd: &Path) -> Result<(), ProviderError> {
        if self.start != 0 {
            self.buffer.drain(..self.start);
            self.start = 0;
        }
        let mut bytes = [0; 8192];
        let size = self
            .opened(n, cwd)?
            .open(n)?
            .read(&mut bytes)
            .map_err(|e| host(n, e))?;
        if size == 0 {
            self.eof = true;
        } else {
            self.buffer.extend_from_slice(&bytes[..size]);
        }
        Ok(())
    }
    pub(crate) fn close(&mut self, n: Native) -> Result<(), ProviderError> {
        if let Some(mut channel) = self.channel.take() {
            channel.close(n)?;
        }
        if let Some(lock) = self.lock.take() {
            lock.closed.set(true);
        }
        self.buffer.clear();
        self.start = 0;
        self.closed = true;
        self.cleanup(n)
    }
}
impl Drop for FileState {
    fn drop(&mut self) {
        let _ = self.close(Native::FsFileStateClose);
    }
}
fn encoding(s: &str) -> Option<(usize, bool)> {
    Some(match s {
        "utf-8" => (1, false),
        "utf-16-le" => (2, false),
        "utf-16-be" => (2, true),
        "utf-32-le" => (4, false),
        "utf-32-be" => (4, true),
        "ascii" | "latin-1" => (0, false),
        _ => return None,
    })
}
fn decode(n: Native, bytes: &[u8], name: &str) -> Result<String, ProviderError> {
    let (width, big) = encoding(name).expect("validated encoding");
    if width == 0 {
        if name == "ascii" && !bytes.is_ascii() {
            return Err(fail(n, "invalid_data", "Invalid ASCII text"));
        }
        return Ok(bytes.iter().map(|b| char::from(*b)).collect());
    }
    let (text, used) = crate::host::native_text::decode_prefix(bytes, width, big);
    if used != bytes.len() {
        return Err(fail(n, "invalid_data", "Invalid encoded text"));
    }
    Ok(text)
}
fn encode(n: Native, text: &str, name: &str) -> Result<Vec<u8>, ProviderError> {
    let (width, big) = encoding(name).expect("validated encoding");
    if width != 0 {
        return Ok(crate::host::native_text::encode(text, width, big));
    }
    let max = if name == "ascii" { 127 } else { 255 };
    text.chars()
        .map(|c| {
            if c as u32 <= max {
                Ok(c as u8)
            } else {
                Err(fail(n, "invalid_data", "Text cannot be encoded"))
            }
        })
        .collect()
}
pub(crate) fn handles(n: Native) -> bool {
    use Native::*;
    matches!(
        n,
        FsFileStateCreate
            | FsFileStateOpen
            | FsFileStateReadText
            | FsFileStateReadLine
            | FsFileStateReadLines
            | FsFileStateRead
            | FsFileStateWrite
            | FsFileStateWriteText
            | FsFileStateFlush
            | FsFileStateClose
            | FsFileStateClosed
            | FsFileStatePosition
            | FsFileStateSeek
            | FsFileStateTruncate
            | FsFileStateLock
            | FsFileStateLocked
            | FsFileStateRewriteText
            | FsFileLockClosed
            | FsFileLockClose
    )
}
impl Heap {
    pub(crate) fn file_state(&self, v: Slot) -> Result<Rc<RefCell<FileState>>, Error> {
        if v.kind() != HANDLE {
            return Err(Error("expected FileState".into()));
        }
        match self.handles.get(v.bits)? {
            Handle::FileState(s) => Ok(s.clone()),
            _ => Err(Error("expected FileState".into())),
        }
    }
    pub(crate) fn file_state_execute(
        &mut self,
        n: Native,
        a: &[Slot],
    ) -> Result<Slot, ProviderError> {
        use Native::*;
        let yes = Slot::boolean(true);
        if n == FsFileStateCreate || n == FsFileStateOpen {
            let bytes = self.native_bytes(a[0])?;
            if bytes.is_empty() || bytes.contains(&0) {
                return Err(fail(n, "invalid_input", "Invalid file path"));
            }
            let flags = match self.int64(a[1])? {
                Some(0) => 1,
                Some(1) => 2 | 4 | 8,
                Some(2) => 2 | 4 | 16,
                Some(3) => 1 | 2,
                Some(4) => 1 | 2 | 4 | 16,
                _ => return Err(fail(n, "invalid_input", "Invalid file mode")),
            };
            let name = self.text(a[2])?.to_owned();
            let undecided = n == FsFileStateOpen;
            let binary = !undecided && a[3].bits != 0;
            if binary && !name.is_empty() {
                return Err(fail(n, "invalid_input", "Binary file cannot have encoding"));
            }
            if !binary && encoding(&name).is_none() {
                return Err(fail(n, "unsupported", "Unsupported encoding"));
            }
            let state = FileState {
                shown: String::new(),
                temporary: false,
                delete_on_close: None,
                path: PathBuf::from(String::from_utf8_lossy(&bytes).as_ref()),
                flags,
                encoding: name,
                binary,
                undecided,
                channel: None,
                buffer: vec![],
                start: 0,
                eof: false,
                closed: false,
                lock: None,
            };
            return Ok(Slot {
                tag: HANDLE,
                bits: self
                    .handles
                    .allocate(Handle::FileState(Rc::new(RefCell::new(state))))?,
            });
        }
        if matches!(n, FsFileLockClosed | FsFileLockClose) {
            if a[0].kind() != HANDLE {
                return Err(Error("expected FileLockState".into()).into());
            }
            let Handle::FileLock(lock) = self.handles.get(a[0].bits)? else {
                return Err(Error("expected FileLockState".into()).into());
            };
            let lock = lock.clone();
            if n == FsFileLockClosed {
                return Ok(Slot::boolean(lock.closed.get()));
            }
            if lock.closed.get() {
                return Ok(yes);
            }
            if let Some(owner) = lock.owner.upgrade() {
                let mut state = owner.borrow_mut();
                #[cfg(unix)]
                if let Some(channel) = &mut state.channel
                    && let Some(oslock) = channel.lock.take() {
                        oslock
                            .release(channel.file.as_ref().expect("open"))
                            .map_err(|e| host(n, e))?;
                    }
                state.lock = None;
            }
            lock.closed.set(true);
            return Ok(yes);
        }
        let shared = self.file_state(a[0])?;
        let mut state = shared.borrow_mut();
        if n == FsFileStateClosed {
            return Ok(Slot::boolean(state.closed));
        }
        if n == FsFileStateClose {
            state.close(n)?;
            return Ok(yes);
        }
        if state.closed {
            return Err(fail(n, "already_closed", "File is closed"));
        }
        let cwd = self.working_directory()?;
        match n {
            FsFileStateRead => {
                let limit = self.bigint(a[1])?.into_owned();
                if limit <= 0.into() {
                    return Err(fail(n, "invalid_input", "Read limit must be positive"));
                }
                state.prepare(n, &cwd, false, false)?;
                let mut bytes = vec![0; limit.to_i64().unwrap_or(i64::MAX).min(1 << 26) as usize];
                let size = state
                    .opened(n, &cwd)?
                    .open(n)?
                    .read(&mut bytes)
                    .map_err(|e| host(n, e))?;
                bytes.truncate(size);
                Ok(self.native_byte_array(bytes)?)
            }
            FsFileStateReadText | FsFileStateReadLine => {
                state.prepare(n, &cwd, true, false)?;
                let needle = encode(n, "\n", &state.encoding)?;
                loop {
                    if n == FsFileStateReadLine {
                        let unit = needle.len();
                        let pending = &state.buffer[state.start..];
                        if let Some(offset) = pending
                            .chunks_exact(unit)
                            .position(|v| v == needle)
                            .map(|i| i * unit)
                        {
                            let text = decode(n, &pending[..offset], &state.encoding);
                            state.start += offset + unit;
                            let text = text?;
                            return Ok(self.allocate_text(
                                text.strip_suffix('\r').unwrap_or(&text).to_owned(),
                            )?);
                        }
                    }
                    if state.eof {
                        if n == FsFileStateReadLine && state.start == state.buffer.len() {
                            return Err(fail(n, "end_of_input", "End of input"));
                        }
                        let text = decode(n, &state.buffer[state.start..], &state.encoding);
                        state.buffer.clear();
                        state.start = 0;
                        return Ok(self.allocate_text(text?)?);
                    }
                    state.fill(n, &cwd)?;
                }
            }
            FsFileStateReadLines => {
                state.prepare(n, &cwd, true, false)?;
                let target = self.int64(a[1])?.map_or(1 << 26, |v| v.clamp(1, 1 << 26)) as usize;
                let needle = encode(n, "\n", &state.encoding)?;
                let unit = needle.len();
                while state.buffer.len() - state.start < target && !state.eof {
                    state.fill(n, &cwd)?;
                }
                // The batch ends after its last whole line; a longer line is read whole.
                let last_end = |pending: &[u8]| {
                    pending.chunks_exact(unit).rposition(|v| v == needle).map(|i| (i + 1) * unit)
                };
                let mut end = last_end(&state.buffer[state.start..]);
                while end.is_none() && !state.eof {
                    state.fill(n, &cwd)?;
                    end = last_end(&state.buffer[state.start..]);
                }
                let end = end.unwrap_or(state.buffer.len() - state.start);
                if end == 0 {
                    return Ok(self.allocate_array(Vec::new())?);
                }
                let text = decode(n, &state.buffer[state.start..state.start + end], &state.encoding);
                state.start += end;
                let text = text?;
                let body = text.strip_suffix('\n').unwrap_or(&text);
                let slots = body
                    .split('\n')
                    .map(|line| self.allocate_text(line.strip_suffix('\r').unwrap_or(line).to_owned()))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(self.allocate_array(slots)?)
            }
            FsFileStateWrite | FsFileStateWriteText => {
                let textual = n == FsFileStateWriteText;
                state.prepare(n, &cwd, textual, true)?;
                let bytes = if textual {
                    let mut text = String::new();
                    for v in self.elements(a[1])? {
                        text.push_str(self.text(v)?);
                    }
                    if a[2].bits != 0 {
                        text.push('\n');
                    }
                    encode(n, &text, &state.encoding)?
                } else {
                    self.native_bytes(a[1])?
                };
                if bytes.is_empty() {
                    return Ok(Slot::small(0));
                }
                let append = state.flags & 16 != 0;
                let file = state.opened(n, &cwd)?.open(n)?;
                if append {
                    file.seek(SeekFrom::End(0)).map_err(|e| host(n, e))?;
                }
                let size = if textual {
                    file.write_all(&bytes).map_err(|e| host(n, e))?;
                    bytes.len()
                } else {
                    file.write(&bytes).map_err(|e| host(n, e))?
                };
                Ok(Slot::small(size as i64))
            }
            FsFileStateRewriteText => {
                state.prepare(n, &cwd, true, true)?;
                let mut text = String::new();
                for v in self.elements(a[1])? {
                    text.push_str(self.text(v)?);
                }
                let bytes = encode(n, &text, &state.encoding)?;
                state.buffer.clear();
                state.start = 0;
                state.eof = false;
                let file = state.opened(n, &cwd)?.open(n)?;
                file.set_len(0).map_err(|e| host(n, e))?;
                file.seek(SeekFrom::Start(0)).map_err(|e| host(n, e))?;
                file.write_all(&bytes).map_err(|e| host(n, e))?;
                Ok(Slot::small(bytes.len() as i64))
            }
            FsFileStateLocked => {
                let mode = self
                    .int64(a[1])?
                    .filter(|v| (0..=1).contains(v))
                    .ok_or_else(|| fail(n, "invalid_input", "Invalid lock mode"))?;
                let channel = state.opened(n, &cwd)?;
                #[cfg(unix)]
                {
                    let file = channel.file.as_ref().expect("open");
                    Ok(Slot::boolean(crate::host::file_lock::Lock::held(n, file, mode == 0)?))
                }
                #[cfg(not(unix))]
                {
                    let _ = (channel, mode);
                    Err(fail(n, "unsupported", "Record locks unavailable"))
                }
            }
            FsFileStateFlush => {
                if state.flags & 2 == 0 {
                    return Err(fail(n, "invalid_input", "File is not writable"));
                }
                state.opened(n, &cwd)?;
                Ok(yes)
            }
            FsFileStatePosition | FsFileStateSeek | FsFileStateTruncate => {
                state.settle_mode(n, false)?;
                if n == FsFileStateTruncate {
                    if state.flags & 2 == 0 {
                        return Err(fail(n, "invalid_input", "File is not writable"));
                    }
                    let size = self.bigint(a[1])?.into_owned();
                    if size.sign() == num_bigint::Sign::Minus {
                        return Err(fail(n, "invalid_input", "Negative truncate size"));
                    }
                    state
                        .opened(n, &cwd)?
                        .open(n)?
                        .set_len(size.to_i64().unwrap_or(i64::MAX) as u64)
                        .map_err(|e| host(n, e))?;
                    return Ok(yes);
                }
                if n == FsFileStatePosition {
                    return Ok(Slot::small(
                        state
                            .opened(n, &cwd)?
                            .open(n)?
                            .stream_position()
                            .map_err(|e| host(n, e))? as i64,
                    ));
                }
                let offset = self.bigint(a[1])?.into_owned();
                let origin = self
                    .int64(a[2])?
                    .filter(|v| (0..=2).contains(v))
                    .ok_or_else(|| fail(n, "invalid_input", "Invalid seek origin"))?;
                let file = state.opened(n, &cwd)?.open(n)?;
                let base = match origin {
                    0 => 0,
                    1 => file.stream_position().map_err(|e| host(n, e))?,
                    _ => file.metadata().map_err(|e| host(n, e))?.len(),
                };
                let target = (num_bigint::BigInt::from(base) + offset)
                    .to_i64()
                    .filter(|v| *v >= 0)
                    .ok_or_else(|| fail(n, "invalid_input", "Invalid seek target"))?;
                file.seek(SeekFrom::Start(target as u64))
                    .map_err(|e| host(n, e))?;
                state.buffer.clear();
                state.start = 0;
                state.eof = false;
                Ok(Slot::small(target))
            }
            FsFileStateLock => {
                if state.lock.as_ref().is_some_and(|l| !l.closed.get()) {
                    return Err(fail(n, "invalid_input", "File already has a live lock"));
                }
                let mode = self
                    .int64(a[1])?
                    .filter(|v| (0..=1).contains(v))
                    .ok_or_else(|| fail(n, "invalid_input", "Invalid lock mode"))?;
                let wait = self
                    .int64(a[2])?
                    .filter(|v| (0..=1).contains(v))
                    .ok_or_else(|| fail(n, "invalid_input", "Invalid lock waiting mode"))?;
                if state.flags & if mode == 0 { 1 } else { 2 } == 0 {
                    return Err(fail(n, "invalid_input", "File mode does not permit lock"));
                }
                let channel = state.opened(n, &cwd)?;
                #[cfg(unix)]
                {
                    channel.lock = Some(
                        crate::host::file_lock::Lock::acquire(
                            n,
                            channel.file.as_ref().expect("open"),
                            mode == 0,
                            wait == 0,
                        )
                        .map_err(|e| match e {
                            ProviderError::Native(e) if e.code == "invalid_input" => {
                                fail(n, "would_block", "File lock unavailable")
                            }
                            e => e,
                        })?
                        .ok_or_else(|| fail(n, "would_block", "File lock unavailable"))?,
                    );
                }
                #[cfg(not(unix))]
                {
                    let _ = (channel, mode, wait);
                    return Err(fail(n, "unsupported", "Record locks unavailable"));
                }
                let lock = Rc::new(FileLock {
                    owner: Rc::downgrade(&shared),
                    closed: Cell::new(false),
                });
                state.lock = Some(lock.clone());
                Ok(Slot {
                    tag: HANDLE,
                    bits: self.handles.allocate(Handle::FileLock(lock))?,
                })
            }
            _ => Err(Error("unsupported file state native".into()).into()),
        }
    }
}
