//! Isolates future dispatch and delivery from an existing worker; no database,
//! Okkama object construction or SQL is involved. Run in release mode.
use okkama_runtime_core::{IntValue, Value, sched};
use std::{sync::mpsc, time::Instant};
type Job = Box<dyn FnOnce() + Send>;
fn value(n: i64) -> Value {
    Value::Int(IntValue::from_i64(n))
}
fn measure(mode: &'static str, iterations: usize, jobs: mpsc::Sender<Job>) -> i64 {
    sched::reset();
    let root = sched::cold(
        Box::new(move || {
            let start = Instant::now();
            for _ in 0..iterations {
                let result = match mode {
                    "ready" => sched::await_future(&sched::resolved(Ok(value(1)))),
                    "cold" => {
                        sched::await_future(&sched::cold(Box::new(|| Ok(value(1))), Vec::new()))
                    }
                    "offload" => sched::await_offload(|| 1i64, |v| Ok(value(v))),
                    "worker_relay" => {
                        let (tx, rx) = mpsc::channel();
                        jobs.send(Box::new(move || {
                            tx.send(1i64).unwrap();
                        }))
                        .unwrap();
                        sched::await_offload(move || rx.recv().unwrap(), |v| Ok(value(v)))
                    }
                    "worker_direct" => {
                        let promise = sched::promise();
                        let target = promise.clone();
                        let (_op, completion) = sched::external(move |v: Option<i64>| {
                            sched::settle(&target, Ok(value(v.unwrap())))
                        });
                        jobs.send(Box::new(move || completion.deliver(1i64)))
                            .unwrap();
                        sched::await_future(&promise)
                    }
                    _ => unreachable!(),
                }?;
                assert_eq!(result.as_int().unwrap().to_i64(), Some(1));
            }
            Ok(value(start.elapsed().as_nanos() as i64))
        }),
        Vec::new(),
    );
    let result = sched::run_main(&root)
        .unwrap()
        .as_int()
        .unwrap()
        .to_i64()
        .unwrap();
    sched::reset();
    result
}
fn main() {
    let iterations = std::env::args()
        .nth(1)
        .map(|s| s.parse().unwrap())
        .unwrap_or(500usize);
    assert!(iterations > 0);
    let (tx, rx) = mpsc::channel::<Job>();
    let worker = std::thread::spawn(move || {
        while let Ok(job) = rx.recv() {
            job();
        }
    });
    let modes = ["ready", "cold", "offload", "worker_relay", "worker_direct"];
    for mode in modes {
        measure(mode, iterations, tx.clone());
    }
    for sample in 0..5 {
        for offset in 0..modes.len() {
            let mode = modes[(sample + offset) % modes.len()];
            println!(
                "{sample} {mode} {iterations} {}",
                measure(mode, iterations, tx.clone())
            );
        }
    }
    drop(tx);
    worker.join().unwrap();
}
