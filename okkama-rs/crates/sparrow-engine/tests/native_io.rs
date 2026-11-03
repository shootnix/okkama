use sparrow_engine::{AsyncHost, Executable, Mode};
#[test]
fn resolver_future_runs_under_tokio_with_caching_groups_and_typed_failures() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-dns.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&program, mode).unwrap();
        let host = AsyncHost::new().unwrap();
        let result = host
            .block_on(async {
                tokio::time::timeout(std::time::Duration::from_secs(10), code.run_async()).await
            })
            .unwrap()
            .expect("native I/O stalled")
            .unwrap();
        assert_eq!(result, 42);
    }
}

#[test]
fn tcp_loopback_preserves_cancellation_busy_readers_binary_data_eof_and_close() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-tcp-native.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&program, mode).unwrap();
        let host = AsyncHost::new().unwrap();
        let result = host
            .block_on(async {
                tokio::time::timeout(std::time::Duration::from_secs(10), code.run_async()).await
            })
            .unwrap()
            .expect("TCP I/O stalled")
            .unwrap();
        assert_eq!(result, 42);
    }
}

#[cfg(unix)]
#[test]
fn public_unix_pair_exchanges_binary_data_and_delivers_eof() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-unix-native.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&program, mode).unwrap();
        let host = AsyncHost::new().unwrap();
        let result = host
            .block_on(async {
                tokio::time::timeout(std::time::Duration::from_secs(10), code.run_async()).await
            })
            .unwrap()
            .expect("Unix I/O stalled")
            .unwrap();
        assert_eq!(result, 42);
    }
}

#[test]
fn udp_loopback_preserves_atomic_packets_truncation_empty_payload_and_close() {
    let program = okkama_sparrow::assemble(include_str!("fixtures/async-udp-native.asm")).unwrap();
    for mode in [Mode::Cold, Mode::Jit] {
        let code = Executable::new(&program, mode).unwrap();
        let host = AsyncHost::new().unwrap();
        let result = host
            .block_on(async {
                tokio::time::timeout(std::time::Duration::from_secs(10), code.run_async()).await
            })
            .unwrap()
            .expect("UDP I/O stalled")
            .unwrap();
        assert_eq!(result, 42);
    }
}
