//! A web application stops on SIGTERM or SIGINT (#80): it stops accepting,
//! lets the request in flight finish, and `listen()` returns, so the program
//! goes on and ends with code 0; a second signal during the drain ends the
//! process at once. Runs `tests/signals/serve.ok` on every engine.
#![cfg(unix)]

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread::{JoinHandle, sleep, spawn};
use std::time::{Duration, Instant};

fn engines() -> Vec<&'static str> {
    let mut engines = vec!["ir", "sparrow"];
    if Command::new("java").arg("-version").output().is_ok() {
        engines.push("jvm");
    }
    engines
}

/// A running `serve.ok`; dropped while running, it is killed.
struct App {
    engine: &'static str,
    child: Child,
    port: u16,
}

struct Ended {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

impl App {
    /// Start the program and wait until it accepts connections; `/slow`
    /// answers after `delay` seconds.
    fn start(engine: &'static str, delay: u64) -> App {
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        let port = TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .expect("a free port")
            .port();
        let child = Command::new(env!("CARGO_BIN_EXE_okkama"))
            .args(["run", "--engine", engine])
            .arg(here.join("tests/signals/serve.ok"))
            .arg(port.to_string())
            .arg(delay.to_string())
            .env("OKKAMA_STDLIB_DIR", here.join("../../../std"))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start okkama");
        let mut app = App { engine, child, port };
        let deadline = Instant::now() + Duration::from_secs(180);
        while TcpStream::connect(("127.0.0.1", port)).is_err() {
            if let Some(status) = app.child.try_wait().expect("poll the program") {
                let ended = app.outputs(status);
                panic!("{engine}: the program ended before listening ({status}):\n{}{}", ended.stdout, ended.stderr);
            }
            assert!(Instant::now() < deadline, "{engine}: the program never listened");
            sleep(Duration::from_millis(100));
        }
        app
    }

    fn signal(&self, name: &str) {
        let status = Command::new("sh")
            .arg("-c")
            .arg(format!("kill -{name} {}", self.child.id()))
            .status()
            .expect("run kill");
        assert!(status.success(), "{}: kill -{name} failed", self.engine);
    }

    /// GET `path` in a thread: the whole answer, or the error that cut it.
    fn get(&self, path: &'static str) -> JoinHandle<Result<String, String>> {
        let port = self.port;
        spawn(move || {
            let mut stream = TcpStream::connect(("127.0.0.1", port)).map_err(|error| error.to_string())?;
            stream.set_read_timeout(Some(Duration::from_secs(60))).map_err(|error| error.to_string())?;
            write!(stream, "GET {path} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
                .map_err(|error| error.to_string())?;
            let mut answer = String::new();
            stream.read_to_string(&mut answer).map_err(|error| error.to_string())?;
            Ok(answer)
        })
    }

    fn refuses(&self) -> bool {
        TcpStream::connect(("127.0.0.1", self.port)).is_err()
    }

    fn outputs(&mut self, status: ExitStatus) -> Ended {
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let Some(mut out) = self.child.stdout.take() {
            let _ = out.read_to_string(&mut stdout);
        }
        if let Some(mut err) = self.child.stderr.take() {
            let _ = err.read_to_string(&mut stderr);
        }
        Ended { status, stdout, stderr }
    }

    /// Wait for the program to end within `limit`.
    fn ended(mut self, limit: Duration) -> Ended {
        let deadline = Instant::now() + limit;
        loop {
            if let Some(status) = self.child.try_wait().expect("poll the program") {
                return self.outputs(status);
            }
            assert!(Instant::now() < deadline, "{}: the program did not end within {limit:?}", self.engine);
            sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        if matches!(self.child.try_wait(), Ok(None)) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[test]
fn sigterm_drains_the_request_in_flight_and_listen_returns() {
    for engine in engines() {
        let app = App::start(engine, 3);
        let hello = app.get("/").join().expect("hello thread").expect("hello answer");
        assert!(hello.starts_with("HTTP/1.1 200") && hello.ends_with("hello"), "{engine}: {hello}");
        let slow = app.get("/slow");
        sleep(Duration::from_secs(1));
        app.signal("TERM");
        let deadline = Instant::now() + Duration::from_millis(1500);
        while !app.refuses() && Instant::now() < deadline {
            sleep(Duration::from_millis(50));
        }
        assert!(app.refuses(), "{engine}: still accepting after SIGTERM");
        let answer = slow.join().expect("slow thread").expect("slow answer");
        assert!(answer.starts_with("HTTP/1.1 200") && answer.ends_with("slow and done"), "{engine}: {answer}");
        let ended = app.ended(Duration::from_secs(20));
        assert_eq!(ended.status.code(), Some(0), "{engine}: {}{}", ended.stdout, ended.stderr);
        assert_eq!(ended.stdout, "stopped\n", "{engine}: {}", ended.stderr);
    }
}

#[test]
fn a_second_signal_during_the_drain_ends_the_process_at_once() {
    for engine in engines() {
        let app = App::start(engine, 60);
        let slow = app.get("/slow");
        sleep(Duration::from_secs(1));
        app.signal("INT");
        sleep(Duration::from_millis(500));
        assert!(app.refuses(), "{engine}: still accepting after SIGINT");
        app.signal("INT");
        let ended = app.ended(Duration::from_secs(5));
        assert_ne!(ended.status.code(), Some(0), "{engine}: {}{}", ended.stdout, ended.stderr);
        assert!(!ended.stdout.contains("stopped"), "{engine}: {}", ended.stdout);
        let _ = slow.join();
    }
}
