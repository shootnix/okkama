//! Lazy files keep Rust buffers and OS resources outside the VM object graph.
use crate::{
    Error,
    handle::Handle,
    native::ProviderError,
    native_file::{RawFile, big},
    native_fs::{fail, host},
    value::*,
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
    pub(crate) temporary: bool,
    pub(crate) delete_on_close: Option<PathBuf>,
    flags: i64,
    encoding: String,
    binary: bool,
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
    pub(crate) fn temporary(path: PathBuf, channel: RawFile) -> Self {
        Self {
            delete_on_close: Some(path.clone()),
            path,
            temporary: true,
            flags: 3,
            encoding: String::new(),
            binary: true,
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
        if text == self.binary {
            return Err(fail(
                n,
                "invalid_input",
                "Operation does not match binary/text file mode",
            ));
        }
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
    let (text, used) = crate::native_text::decode_prefix(bytes, width, big);
    if used != bytes.len() {
        return Err(fail(n, "invalid_data", "Invalid encoded text"));
    }
    Ok(text)
}
fn encode(n: Native, text: &str, name: &str) -> Result<Vec<u8>, ProviderError> {
    let (width, big) = encoding(name).expect("validated encoding");
    if width != 0 {
        return Ok(crate::native_text::encode(text, width, big));
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
            | FsFileStateReadText
            | FsFileStateReadLine
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
            | FsFileLockClosed
            | FsFileLockClose
    )
}
impl Heap {
    pub(crate) fn file_state(&self, v: Slot) -> Result<Rc<RefCell<FileState>>, Error> {
        if v.tag != HANDLE {
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
        if n == FsFileStateCreate {
            let bytes = self.native_bytes(a[0])?;
            if bytes.is_empty() || bytes.contains(&0) {
                return Err(fail(n, "invalid_input", "Invalid file path"));
            }
            let flags = match self.integer(a[1])?.to_i64() {
                Some(0) => 1,
                Some(1) => 2 | 4 | 8,
                Some(2) => 2 | 4 | 16,
                Some(3) => 1 | 2,
                Some(4) => 1 | 2 | 4 | 16,
                _ => return Err(fail(n, "invalid_input", "Invalid file mode")),
            };
            let name = self.text(a[2])?.to_owned();
            let binary = a[3].bits != 0;
            if binary && !name.is_empty() {
                return Err(fail(n, "invalid_input", "Binary file cannot have encoding"));
            }
            if !binary && encoding(&name).is_none() {
                return Err(fail(n, "unsupported", "Unsupported encoding"));
            }
            let state = FileState {
                temporary: false,
                delete_on_close: None,
                path: PathBuf::from(String::from_utf8_lossy(&bytes).as_ref()),
                flags,
                encoding: name,
                binary,
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
            if a[0].tag != HANDLE {
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
                if let Some(channel) = &mut state.channel {
                    if let Some(oslock) = channel.lock.take() {
                        oslock
                            .release(channel.file.as_ref().expect("open"))
                            .map_err(|e| host(n, e))?;
                    }
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
                let limit = big(self.integer(a[1])?);
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
            FsFileStateWrite | FsFileStateWriteText => {
                let textual = n == FsFileStateWriteText;
                state.prepare(n, &cwd, textual, true)?;
                let bytes = if textual {
                    let mut text = String::new();
                    for v in self.elements(a[1])? {
                        text.push_str(self.text(*v)?);
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
            FsFileStateFlush => {
                if state.flags & 2 == 0 {
                    return Err(fail(n, "invalid_input", "File is not writable"));
                }
                state.opened(n, &cwd)?;
                Ok(yes)
            }
            FsFileStatePosition | FsFileStateSeek | FsFileStateTruncate => {
                if !state.binary {
                    return Err(fail(
                        n,
                        "invalid_input",
                        "Byte positions require binary mode",
                    ));
                }
                if n == FsFileStateTruncate {
                    if state.flags & 2 == 0 {
                        return Err(fail(n, "invalid_input", "File is not writable"));
                    }
                    let size = big(self.integer(a[1])?);
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
                let offset = big(self.integer(a[1])?);
                let origin = self
                    .integer(a[2])?
                    .to_i64()
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
                    .integer(a[1])?
                    .to_i64()
                    .filter(|v| (0..=1).contains(v))
                    .ok_or_else(|| fail(n, "invalid_input", "Invalid lock mode"))?;
                let wait = self
                    .integer(a[2])?
                    .to_i64()
                    .filter(|v| (0..=1).contains(v))
                    .ok_or_else(|| fail(n, "invalid_input", "Invalid lock waiting mode"))?;
                if state.flags & if mode == 0 { 1 } else { 2 } == 0 {
                    return Err(fail(n, "invalid_input", "File mode does not permit lock"));
                }
                let channel = state.opened(n, &cwd)?;
                #[cfg(unix)]
                {
                    channel.lock = Some(
                        crate::file_lock::Lock::acquire(
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    fn directory() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "sparrow-state-{}-{}",
            std::process::id(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        path
    }
    fn state(h: &mut Heap, name: &str, mode: i64, encoding: &str, binary: bool) -> Slot {
        let p = h.native_byte_array(name.as_bytes().to_vec()).unwrap();
        let e = h.allocate_text(encoding.into()).unwrap();
        h.file_state_execute(
            Native::FsFileStateCreate,
            &[p, Slot::small(mode), e, Slot::boolean(binary)],
        )
        .unwrap()
    }
    fn code(r: Result<Slot, ProviderError>) -> String {
        match r {
            Err(ProviderError::Native(e)) => e.code,
            other => panic!("expected native error: {other:?}"),
        }
    }
    #[test]
    fn lazy_file_open_resolves_current_vm_directory_and_close_does_not_create() {
        let dir = directory();
        let mut h = Heap::new(&[], &[]);
        let f = state(&mut h, "never-created", 1, "utf-8", false);
        h.cwd = Some(dir.clone());
        h.file_state_execute(Native::FsFileStateClose, &[f])
            .unwrap();
        assert!(!dir.join("never-created").exists());
        let f = state(&mut h, "created", 1, "utf-8", false);
        let text = h.allocate_text("hello".into()).unwrap();
        let values = h.allocate_array(vec![text]).unwrap();
        h.file_state_execute(
            Native::FsFileStateWriteText,
            &[f, values, Slot::boolean(true)],
        )
        .unwrap();
        assert_eq!(std::fs::read(dir.join("created")).unwrap(), b"hello\n");
        h.file_state_execute(Native::FsFileStateClose, &[f])
            .unwrap();
        assert_eq!(
            code(h.file_state_execute(Native::FsFileStateReadLine, &[f])),
            "already_closed"
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn encoded_lines_cross_buffer_boundaries_and_restore_position_before_writing() {
        let dir = directory();
        let mut h = Heap::new(&[], &[]);
        h.cwd = Some(dir.clone());
        for enc in [
            "utf-8",
            "utf-16-le",
            "utf-16-be",
            "utf-32-le",
            "utf-32-be",
            "ascii",
            "latin-1",
        ] {
            let contents = format!("{}\r\nTAIL", "x".repeat(8191));
            std::fs::write(
                dir.join("data"),
                encode(Native::FsFileStateWriteText, &contents, enc).unwrap(),
            )
            .unwrap();
            let f = state(&mut h, "data", 3, enc, false);
            let line = h
                .file_state_execute(Native::FsFileStateReadLine, &[f])
                .unwrap();
            assert_eq!(h.text(line).unwrap(), "x".repeat(8191));
            let text = h.allocate_text("OK".into()).unwrap();
            let values = h.allocate_array(vec![text]).unwrap();
            h.file_state_execute(
                Native::FsFileStateWriteText,
                &[f, values, Slot::boolean(false)],
            )
            .unwrap();
            let tail = h
                .file_state_execute(Native::FsFileStateReadText, &[f])
                .unwrap();
            assert_eq!(h.text(tail).unwrap(), "IL");
            assert_eq!(
                code(h.file_state_execute(Native::FsFileStateReadLine, &[f])),
                "end_of_input"
            );
            assert_eq!(
                code(h.file_state_execute(Native::FsFileStatePosition, &[f])),
                "invalid_input"
            );
            h.file_state_execute(Native::FsFileStateClose, &[f])
                .unwrap();
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn malformed_text_is_rejected_without_replacement_and_binary_has_exact_positions() {
        let dir = directory();
        let mut h = Heap::new(&[], &[]);
        h.cwd = Some(dir.clone());
        for (enc, bytes) in [
            ("utf-8", vec![0xff]),
            ("utf-16-le", vec![0, 0xd8]),
            ("utf-32-be", vec![0, 0x11, 0, 0]),
            ("ascii", vec![128]),
        ] {
            std::fs::write(dir.join("data"), bytes).unwrap();
            let f = state(&mut h, "data", 0, enc, false);
            assert_eq!(
                code(h.file_state_execute(Native::FsFileStateReadText, &[f])),
                "invalid_data"
            );
            h.file_state_execute(Native::FsFileStateClose, &[f])
                .unwrap();
        }
        std::fs::write(dir.join("data"), b"abc").unwrap();
        let f = state(&mut h, "data", 3, "", true);
        assert_eq!(
            code(h.file_state_execute(Native::FsFileStateRead, &[f, Slot::small(0)])),
            "invalid_input"
        );
        h.file_state_execute(
            Native::FsFileStateSeek,
            &[f, Slot::small(2), Slot::small(0)],
        )
        .unwrap();
        h.file_state_execute(Native::FsFileStateTruncate, &[f, Slot::small(1)])
            .unwrap();
        assert_eq!(
            h.file_state_execute(Native::FsFileStatePosition, &[f])
                .unwrap()
                .bits,
            2
        );
        let bytes = h
            .file_state_execute(Native::FsFileStateRead, &[f, Slot::small(4)])
            .unwrap();
        assert!(h.native_bytes(bytes).unwrap().is_empty());
        h.file_state_execute(Native::FsFileStateClose, &[f])
            .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[cfg(unix)]
    #[test]
    fn file_state_lock_is_closed_when_owner_is_collected() {
        let dir = directory();
        std::fs::write(dir.join("data"), []).unwrap();
        let mut h = Heap::new(&[], &[]);
        h.cwd = Some(dir.clone());
        let f = state(&mut h, "data", 3, "", true);
        let lock = h
            .file_state_execute(
                Native::FsFileStateLock,
                &[f, Slot::small(1), Slot::small(1)],
            )
            .unwrap();
        h.collect([lock].into_iter()).unwrap();
        assert_eq!(
            h.file_state_execute(Native::FsFileLockClosed, &[lock])
                .unwrap()
                .bits,
            1
        );
        h.file_state_execute(Native::FsFileLockClose, &[lock])
            .unwrap();
        std::fs::remove_dir_all(dir).unwrap();
    }
}
