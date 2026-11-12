//! Standard output of the process, buffered (#132). Every `Stdout` value, the
//! test report and the command line's own output go through here, so their
//! order holds. A terminal gets every line as it ends; a pipe or a file gets
//! the bytes in blocks of 64 KiB.
//!
//! Pending bytes go out on [`flush`]. The runtimes call it before standard
//! error is written, before a child process starts or the terminal is
//! written, and when the program ends; [`before_input`] flushes before
//! standard input is read from a terminal, so a prompt shows, as C stdio does.
use std::io::{self, IsTerminal, Write};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

/// The block a pipe or a file gets at once.
const BLOCK: usize = 64 * 1024;

fn buffer() -> &'static Mutex<Vec<u8>> {
    static PENDING: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();
    PENDING.get_or_init(|| Mutex::new(Vec::new()))
}

fn pending() -> MutexGuard<'static, Vec<u8>> {
    buffer().lock().unwrap_or_else(PoisonError::into_inner)
}

/// Is standard output a terminal (line buffering)?
pub fn terminal() -> bool {
    static TERMINAL: OnceLock<bool> = OnceLock::new();
    *TERMINAL.get_or_init(|| io::stdout().is_terminal())
}

/// Append `bytes`: a terminal gets every complete line now, and a full block
/// goes out at once.
pub fn write(bytes: &[u8]) -> io::Result<()> {
    let mut pending = pending();
    pending.extend_from_slice(bytes);
    if terminal() {
        if let Some(end) = pending.iter().rposition(|&byte| byte == b'\n') {
            let rest = pending.split_off(end + 1);
            let lines = std::mem::replace(&mut *pending, rest);
            return emit(&lines);
        }
    } else if pending.len() >= BLOCK {
        let block = std::mem::take(&mut *pending);
        return emit(&block);
    }
    Ok(())
}

/// Write out everything pending.
pub fn flush() -> io::Result<()> {
    let mut pending = pending();
    let all = std::mem::take(&mut *pending);
    emit(&all)
}

/// Flush before standard input is read: on a terminal a prompt must show.
pub fn before_input() -> io::Result<()> {
    if terminal() { flush() } else { Ok(()) }
}

fn emit(bytes: &[u8]) -> io::Result<()> {
    if bytes.is_empty() {
        return Ok(());
    }
    let mut out = io::stdout().lock();
    out.write_all(bytes)?;
    out.flush()
}

/// A [`Write`] over the process buffer, for writers handed to a runtime or a
/// test report.
#[derive(Debug, Default, Clone, Copy)]
pub struct Buffered;

impl Write for Buffered {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        write(bytes)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        flush()
    }
}

/// [`flush`] unless another thread holds the buffer: a watchdog that stops
/// the process must not wait for it.
pub fn try_flush() {
    if let Ok(mut pending) = buffer().try_lock() {
        let all = std::mem::take(&mut *pending);
        let _ = emit(&all);
    }
}
