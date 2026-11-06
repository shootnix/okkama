//! The Sparrow core (`sparrow-core`) behind `okkama exec`, `okkama run
//! --engine sparrow` and the OCS adapter. Process workers run the program's
//! bytecode in `okkama --worker FILE.spb`; a program compiled in memory gets
//! a temporary file for them when it spawns workers.
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Does the program start process workers (`worker.spawn`)?
pub(crate) fn spawns_workers(program: &okkama_sparrow::Program) -> bool {
    program
        .functions
        .iter()
        .flat_map(|function| &function.code)
        .any(|instruction| matches!(instruction, okkama_sparrow::Instruction::WorkerSpawn { .. }))
}

/// Bytecode written for the workers of one run; removed when dropped.
#[derive(Debug)]
pub(crate) struct WorkerFile(PathBuf);

impl WorkerFile {
    pub(crate) fn write(bytes: &[u8]) -> std::io::Result<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        let path = std::env::temp_dir().join(format!("okkama-worker-{}-{nanos}.spb", std::process::id()));
        std::fs::write(&path, bytes)?;
        Ok(Self(path))
    }

    pub(crate) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for WorkerFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// `okkama --worker FILE.spb` for the workers of `path` (the core inserts
/// `--worker` after the executable).
pub(crate) fn worker_command(path: &Path) -> Vec<OsString> {
    let exe = std::env::current_exe().unwrap_or_else(|_| "okkama".into());
    let file = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    vec![exe.into_os_string(), file.into_os_string()]
}

/// The worker command of an in-memory program, with the file it needs.
pub(crate) fn workers_for(
    program: &okkama_sparrow::Program,
    bytes: &[u8],
) -> Result<(Vec<OsString>, Option<WorkerFile>), String> {
    if !spawns_workers(program) {
        return Ok((Vec::new(), None));
    }
    let file = WorkerFile::write(bytes).map_err(|e| format!("cannot write worker bytecode: {e}"))?;
    Ok((worker_command(file.path()), Some(file)))
}

/// The child side of a process worker: the request on stdin, the response
/// on stdout; it never runs main.
pub(crate) fn worker(args: &[String]) -> Result<(), String> {
    let [path] = args else {
        return Err("--worker needs one bytecode file".into());
    };
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read `{path}`: {e}"))?;
    let mut request = Vec::new();
    std::io::stdin().read_to_end(&mut request).map_err(|e| format!("worker request: {e}"))?;
    let response =
        sparrow_core::run_worker(bytes, request, worker_command(Path::new(path))).map_err(|e| e.0)?;
    let mut out = std::io::stdout().lock();
    out.write_all(&response).and_then(|()| out.flush()).map_err(|e| format!("worker response: {e}"))
}

/// The report of an uncaught failure, ending with a newline.
pub(crate) fn report(error: sparrow_core::Error) -> String {
    let mut text = error.0;
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}
