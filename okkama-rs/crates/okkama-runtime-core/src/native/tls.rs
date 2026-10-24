//! `runtime/tls_transport` — перенос `TlsNatives` JVM: TLS-клиент rustls
//! поверх TCP-потока, отобранного у `runtime/network`. Как у JDK-провайдера,
//! движок не проверяет цепочку пира сам (подписи рукопожатия — проверяет):
//! `authorize` — явная проверка `verify_chain` до прикладного ввода-вывода.
//! Шифротекст подаётся движку по одной записи, как `SSLEngine.unwrap`: одно
//! чтение отдаёт открытый текст не более чем одной записи.
use super::network::{Shared, read_chunk, take_tcp, write_all};
use super::support::{array, byte_array, bytes, handle, int, items, new_handle, string, text};
use super::Error;
use crate::failure::{Failure, Kind};
use crate::sched;
use crate::{Value, abi::Native};
use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{ClientConnection, DigitallySignedStruct, SignatureScheme};
use std::cell::RefCell;
use std::io::Read;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const DOMAIN: &str = "core.tls_transport";
const CHUNK: usize = 65536;

fn symbol(name: &str, arity: usize) -> String {
    format!("runtime/tls_transport@1::{name}/{arity}")
}

fn fail(code: &str, operation: &str, message: &str) -> Rc<Failure> {
    Failure::new(Kind::Native(Rc::new(crate::abi::NativeFailure {
        domain: DOMAIN.into(),
        code: code.into(),
        operation: operation.into(),
        message: message.into(),
    })))
}

fn as_error(failure: Rc<Failure>) -> Error {
    match &failure.kind {
        Kind::Native(native) => Error::Failure((**native).clone()),
        _ => Error::Trap(failure.message().to_string()),
    }
}

fn resolved(outcome: sched::Outcome) -> Value {
    Value::Future(sched::resolved(outcome))
}

/// Проверка пира отложена до `authorize` (`Deferred` JVM).
#[derive(Debug)]
struct Deferred(Arc<rustls::crypto::CryptoProvider>);

impl ServerCertVerifier for Deferred {
    fn verify_server_cert(
        &self,
        _end: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        Ok(ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.0.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

/// Порядок наборов SunJSSE по умолчанию (из тех, что есть у rustls).
fn provider() -> rustls::crypto::CryptoProvider {
    use rustls::crypto::ring::cipher_suite::*;
    let mut provider = rustls::crypto::ring::default_provider();
    provider.cipher_suites = vec![
        TLS13_AES_256_GCM_SHA384,
        TLS13_AES_128_GCM_SHA256,
        TLS13_CHACHA20_POLY1305_SHA256,
        TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384,
        TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256,
        TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256,
        TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384,
        TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256,
        TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256,
    ];
    provider
}

#[derive(Debug)]
struct Truncated;

/// Движок TLS, общий с рабочими потоками.
struct Engine {
    connection: Mutex<ClientConnection>,
    socket: Arc<Shared>,
    /// Шифротекст, полученный, но ещё не поданный движку.
    incoming: Mutex<Vec<u8>>,
    /// Открытый текст, прочитанный без ожидающего `read`.
    plain: Mutex<Vec<u8>>,
    /// Порядок записей на проводе: берётся до освобождения движка.
    sending: Mutex<()>,
    /// Однопоточные исполнители чтения и записи JVM.
    reader: Mutex<()>,
    writer: Mutex<()>,
    eof: AtomicBool,
}

impl Engine {
    /// Отправить накопленный шифротекст; движок освобождается до отправки.
    fn flush(&self, mut connection: std::sync::MutexGuard<'_, ClientConnection>) -> std::io::Result<()> {
        let mut out = Vec::new();
        while connection.wants_write() {
            connection.write_tls(&mut out)?;
        }
        let _order = self.sending.lock().expect("TLS send lock");
        drop(connection);
        write_all(&self.socket, &out)
    }

    fn flush_pending(&self) -> std::io::Result<()> {
        self.flush(self.connection.lock().expect("TLS lock"))
    }

    /// Подать движку одну запись; `false` — конец потока без записи.
    fn feed_record(&self) -> std::io::Result<bool> {
        loop {
            let record = {
                let mut incoming = self.incoming.lock().expect("TLS buffer lock");
                if incoming.len() >= 5 {
                    let size = 5 + usize::from(u16::from_be_bytes([incoming[3], incoming[4]]));
                    (incoming.len() >= size).then(|| incoming.drain(..size).collect::<Vec<u8>>())
                } else {
                    None
                }
            };
            if let Some(record) = record {
                let mut connection = self.connection.lock().expect("TLS lock");
                let mut slice = record.as_slice();
                while !slice.is_empty() {
                    connection.read_tls(&mut slice)?;
                }
                if let Err(error) = connection.process_new_packets() {
                    let _ = self.flush(connection);
                    return Err(std::io::Error::other(error));
                }
                return Ok(true);
            }
            match read_chunk(&self.socket)? {
                Some(data) => self.incoming.lock().expect("TLS buffer lock").extend(data),
                None => {
                    let mut connection = self.connection.lock().expect("TLS lock");
                    let _ = connection.read_tls(&mut &[][..]);
                    let _ = connection.process_new_packets();
                    return Ok(false);
                }
            }
        }
    }

    fn negotiate(&self) -> std::io::Result<()> {
        loop {
            self.flush_pending()?;
            if !self.connection.lock().expect("TLS lock").is_handshaking() {
                break;
            }
            if !self.feed_record()? {
                return Err(std::io::Error::other("connection closed during handshake"));
            }
        }
        self.flush_pending()?;
        let mut connection = self.connection.lock().expect("TLS lock");
        let mut early = Vec::new();
        let _ = connection.reader().read_to_end(&mut early);
        self.plain.lock().expect("TLS plain lock").extend(early);
        Ok(())
    }

    fn take_plain(&self, limit: usize) -> Option<Vec<u8>> {
        let mut plain = self.plain.lock().expect("TLS plain lock");
        if plain.is_empty() {
            return None;
        }
        let count = plain.len().min(limit);
        Some(plain.drain(..count).collect())
    }

    fn return_plain(&self, bytes: Vec<u8>) {
        let mut plain = self.plain.lock().expect("TLS plain lock");
        let rest = std::mem::take(&mut *plain);
        plain.extend(bytes);
        plain.extend(rest);
    }

    /// Следующий открытый текст; пусто — `close_notify` пира.
    fn receive(&self, limit: usize) -> Result<Vec<u8>, Result<std::io::Error, Truncated>> {
        let _gate = self.reader.lock().expect("TLS reader lock");
        if let Some(ready) = self.take_plain(limit) {
            return Ok(ready);
        }
        if self.eof.load(Ordering::SeqCst) {
            return Ok(Vec::new());
        }
        loop {
            {
                let mut connection = self.connection.lock().expect("TLS lock");
                let mut buffer = vec![0u8; limit];
                match connection.reader().read(&mut buffer) {
                    Ok(0) => {
                        self.eof.store(true, Ordering::SeqCst);
                        return Ok(Vec::new());
                    }
                    Ok(count) => {
                        buffer.truncate(count);
                        // Остаток записи — в буфер открытого текста, как у JVM.
                        let mut rest = Vec::new();
                        let _ = connection.reader().read_to_end(&mut rest);
                        drop(connection);
                        if !rest.is_empty() {
                            self.return_plain(rest);
                        }
                        return Ok(buffer);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => return Err(Err(Truncated)),
                    Err(error) => return Err(Ok(error)),
                }
            }
            self.feed_record().map_err(Ok)?;
            self.flush_pending().map_err(Ok)?;
        }
    }

    fn send(&self, payload: &[u8]) -> std::io::Result<()> {
        let _gate = self.writer.lock().expect("TLS writer lock");
        let mut offset = 0;
        while offset < payload.len() {
            let mut connection = self.connection.lock().expect("TLS lock");
            let end = (offset + 16384).min(payload.len());
            let written = std::io::Write::write(&mut connection.writer(), &payload[offset..end])?;
            if written == 0 {
                return Err(std::io::Error::other("TLS output is closed"));
            }
            offset += written;
            self.flush(connection)?;
        }
        Ok(())
    }

    fn shutdown(&self) -> std::io::Result<()> {
        let _gate = self.reader.lock().expect("TLS reader lock");
        {
            let mut connection = self.connection.lock().expect("TLS lock");
            connection.send_close_notify();
            self.flush(connection)?;
        }
        // Двусторонний shutdown: ждём close_notify пира.
        while !self.eof.load(Ordering::SeqCst) {
            {
                let mut connection = self.connection.lock().expect("TLS lock");
                let mut sink = vec![0u8; CHUNK];
                match connection.reader().read(&mut sink) {
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
                    Err(_) => break,
                }
            }
            if !self.feed_record()? {
                break;
            }
        }
        Ok(())
    }
}

struct Tls {
    engine: Arc<Engine>,
    negotiated: bool,
    authorized: bool,
    closing: bool,
    closed: bool,
    handshake: Option<sched::Future>,
    reading: Option<sched::Future>,
    writing: Option<sched::Future>,
    shutting: Option<sched::Future>,
    peer: Vec<Vec<u8>>,
}

type TlsRef = Rc<RefCell<Tls>>;

fn abort(state: &mut Tls) {
    if state.closed {
        return;
    }
    state.closed = true;
    state.engine.socket.close();
}

fn state(value: &Value, operation: &str) -> Result<TlsRef, Rc<Failure>> {
    let state = handle::<TlsRef>(value).map_err(|_| fail("invalid_input", operation, "Invalid TLS state"))?.clone();
    if state.borrow().closed {
        return Err(fail("already_closed", operation, "TLS owner is closed"));
    }
    Ok(state)
}

fn client(args: &[Value]) -> Result<Value, Error> {
    let operation = symbol("client", 4);
    let (socket, early) = take_tcp(&args[0], &operation).map_err(as_error)?;
    let choice = int(&args[1])?.to_i64().unwrap_or(0);
    let abort_with = |code: &str, message: &str| {
        socket.close();
        as_error(fail(code, &operation, message))
    };
    if !(1..=3).contains(&choice) {
        return Err(abort_with("invalid_input", "invalid TLS version selection"));
    }
    let protocols = items(&args[2])?;
    let server = string(&args[3])?;
    let mut valid = protocols.len() <= 16
        && server.encode_utf16().count() <= 253
        && server.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-');
    let mut names: Vec<String> = Vec::new();
    for protocol in protocols {
        let name = protocol.as_string()?.to_string();
        let units = name.encode_utf16().count();
        if name.is_empty() || units > 255 || name.chars().any(|c| u32::from(c) >= 256) || names.contains(&name) {
            valid = false;
        }
        names.push(name);
    }
    if !valid {
        return Err(abort_with("invalid_input", "invalid TLS client configuration"));
    }
    let provider = Arc::new(provider());
    let versions: &[&'static rustls::SupportedProtocolVersion] = match choice {
        1 => &[&rustls::version::TLS12],
        2 => &[&rustls::version::TLS13],
        _ => &[&rustls::version::TLS13, &rustls::version::TLS12],
    };
    let built = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_protocol_versions(versions)
        .map(|builder| {
            builder
                .dangerous()
                .with_custom_certificate_verifier(Arc::new(Deferred(provider)))
                .with_no_client_auth()
        });
    let Ok(mut config) = built else {
        return Err(abort_with("unsupported", "required TLS engine policy unavailable"));
    };
    config.alpn_protocols = names.iter().map(|name| name.chars().map(|c| c as u8).collect()).collect();
    // SNI — только для имени хоста, как `SNIHostName` JVM.
    let sni = !server.is_empty() && !server.bytes().all(|b| b.is_ascii_digit() || b == b'.');
    let name = if sni {
        ServerName::try_from(server.to_string()).ok()
    } else {
        Some(ServerName::IpAddress(std::net::Ipv4Addr::UNSPECIFIED.into()))
    };
    let Some(name) = name else {
        return Err(abort_with("unsupported", "required TLS engine policy unavailable"));
    };
    let Ok(connection) = ClientConnection::new(Arc::new(config), name) else {
        return Err(abort_with("unsupported", "required TLS engine policy unavailable"));
    };
    let engine = Arc::new(Engine {
        connection: Mutex::new(connection),
        socket,
        incoming: Mutex::new(early),
        plain: Mutex::new(Vec::new()),
        sending: Mutex::new(()),
        reader: Mutex::new(()),
        writer: Mutex::new(()),
        eof: AtomicBool::new(false),
    });
    Ok(new_handle(Rc::new(RefCell::new(Tls {
        engine,
        negotiated: false,
        authorized: false,
        closing: false,
        closed: false,
        handshake: None,
        reading: None,
        writing: None,
        shutting: None,
        peer: Vec::new(),
    }))))
}

fn io_failure(closed: bool, operation: &str, readiness: &str) -> Rc<Failure> {
    if closed {
        fail("already_closed", operation, readiness)
    } else {
        fail("invalid_data", operation, "TLS I/O failed")
    }
}

fn handshake(value: &Value) -> Value {
    let operation = symbol("handshake", 1);
    let state = match state(value, &operation) {
        Ok(state) => state,
        Err(failure) => return resolved(Err(failure)),
    };
    if state.borrow().negotiated {
        return resolved(Ok(Value::Bool(true)));
    }
    if state.borrow().handshake.is_some() {
        return resolved(Err(fail("resource_exhausted", &operation, "TLS handshake already pending")));
    }
    let promise = sched::promise();
    state.borrow_mut().handshake = Some(promise.clone());
    let engine = state.borrow().engine.clone();
    let target = promise.clone();
    let delivered = state.clone();
    let op = sched::offload(
        move || {
            let _gate = engine.reader.lock().expect("TLS reader lock");
            engine.negotiate()
        },
        move |outcome: std::io::Result<()>| match outcome {
            Ok(()) => {
                delivered.borrow_mut().negotiated = true;
                sched::settle(&target, Ok(Value::Bool(true)));
            }
            Err(_) => {
                let failure = if delivered.borrow().closed {
                    fail("already_closed", &operation, "TLS handshake readiness closed")
                } else {
                    fail("invalid_data", &operation, "TLS client handshake failed")
                };
                sched::settle(&target, Err(failure));
            }
        },
    );
    let watched = promise.clone();
    let weak = Rc::downgrade(&state);
    promise.listen(move || {
        op.detach();
        if let Some(state) = weak.upgrade() {
            let mut state = state.borrow_mut();
            state.handshake = None;
            if watched.state() != sched::State::Done {
                abort(&mut state);
            }
        }
    });
    Value::Future(promise)
}

fn authorize(native: Native, args: &[Value]) -> Result<Value, Error> {
    let operation = symbol("authorize", 5);
    let state = state(&args[0], &operation).map_err(as_error)?;
    let attempt = || -> Result<Vec<Vec<u8>>, Error> {
        let roots = items(&args[1])?;
        if roots.len() > 1024 {
            return Err(as_error(fail("resource_exhausted", &operation, "TLS root count limit")));
        }
        let mut total = 0usize;
        for root in roots {
            let size = items(root)?.len();
            total += size;
            if size > 1_048_576 || total > 16_777_216 {
                return Err(as_error(fail("resource_exhausted", &operation, "TLS root byte limit")));
            }
        }
        if !state.borrow().negotiated {
            return Err(as_error(fail("invalid_input", &operation, "TLS peer is not negotiated")));
        }
        let sources: Vec<Vec<u8>> = {
            let borrowed = state.borrow();
            let connection = borrowed.engine.connection.lock().expect("TLS lock");
            connection
                .peer_certificates()
                .map(|certificates| certificates.iter().map(|certificate| certificate.to_vec()).collect())
                .unwrap_or_default()
        };
        if sources.is_empty() {
            return Err(as_error(fail("invalid_data", &operation, "TLS peer certificate is absent")));
        }
        if sources.len() > 32 {
            return Err(as_error(fail("resource_exhausted", &operation, "TLS peer certificate count limit")));
        }
        let mut sum = 0usize;
        for der in &sources {
            sum += der.len();
            if der.len() > 1_048_576 || sum > 8_388_608 {
                return Err(as_error(fail("resource_exhausted", &operation, "TLS peer certificate byte limit")));
            }
        }
        let chain = array(sources.iter().map(|der| byte_array(der.clone())).collect());
        super::x509::execute(
            Native::X509VerifyChain,
            &[chain, args[1].clone(), args[2].clone(), args[3].clone(), args[4].clone(), text("1.3.6.1.5.5.7.3.1")],
        )?;
        Ok(sources)
    };
    let _ = native;
    match attempt() {
        Ok(sources) => {
            let mut borrowed = state.borrow_mut();
            borrowed.peer = sources;
            borrowed.authorized = true;
            Ok(Value::Bool(true))
        }
        Err(error) => {
            abort(&mut state.borrow_mut());
            Err(error)
        }
    }
}

fn unverified(state: &Tls, operation: &str) -> Option<Rc<Failure>> {
    if state.closed {
        return Some(fail("already_closed", operation, "TLS owner is closed"));
    }
    if !state.authorized {
        return Some(fail("permission_denied", operation, "TLS peer is not verified"));
    }
    None
}

fn read(value: &Value, limit: &crate::IntValue) -> Result<Value, Error> {
    let operation = symbol("read", 2);
    if limit.signum() <= 0 {
        return Ok(resolved(Err(fail("invalid_input", &operation, "TLS read limit must be positive"))));
    }
    let state = handle::<TlsRef>(value)?.clone();
    let refused = {
        let borrowed = state.borrow();
        unverified(&borrowed, &operation)
            .or_else(|| borrowed.closing.then(|| fail("already_closed", &operation, "TLS owner is shutting down")))
            .or_else(|| borrowed.reading.is_some().then(|| fail("resource_exhausted", &operation, "TLS operation already pending")))
    };
    if let Some(refused) = refused {
        return Ok(resolved(Err(refused)));
    }
    let limit = limit.to_i64().map_or(CHUNK, |limit| (limit as usize).min(CHUNK));
    let engine = state.borrow().engine.clone();
    if let Some(ready) = engine.take_plain(limit) {
        return Ok(resolved(Ok(byte_array(ready))));
    }
    let promise = sched::promise();
    state.borrow_mut().reading = Some(promise.clone());
    let target = promise.clone();
    let delivered = state.clone();
    let worker = engine.clone();
    let op = sched::offload(
        move || worker.receive(limit),
        move |outcome| {
            if target.state() != sched::State::Running {
                if let Ok(bytes) = outcome {
                    engine.return_plain(bytes);
                }
                return;
            }
            let closed = delivered.borrow().closed;
            let result = match outcome {
                Ok(bytes) => Ok(byte_array(bytes)),
                Err(Err(Truncated)) => Err(fail("invalid_data", &operation, "TLS peer truncated input")),
                Err(Ok(_)) => Err(io_failure(closed, &operation, "TLS I/O readiness closed")),
            };
            sched::settle(&target, result);
        },
    );
    let watched = promise.clone();
    let weak = Rc::downgrade(&state);
    promise.listen(move || {
        op.detach();
        if let Some(state) = weak.upgrade() {
            let mut state = state.borrow_mut();
            state.reading = None;
            if watched.state() == sched::State::Failed && !watched.cancelled() {
                abort(&mut state);
            }
        }
    });
    Ok(Value::Future(promise))
}

fn write(value: &Value, payload: Vec<u8>) -> Result<Value, Error> {
    let operation = symbol("write", 2);
    let state = handle::<TlsRef>(value)?.clone();
    let refused = {
        let borrowed = state.borrow();
        unverified(&borrowed, &operation)
            .or_else(|| borrowed.closing.then(|| fail("already_closed", &operation, "TLS owner is shutting down")))
            .or_else(|| borrowed.writing.is_some().then(|| fail("resource_exhausted", &operation, "TLS operation already pending")))
    };
    if let Some(refused) = refused {
        return Ok(resolved(Err(refused)));
    }
    let promise = sched::promise();
    state.borrow_mut().writing = Some(promise.clone());
    let engine = state.borrow().engine.clone();
    let target = promise.clone();
    let delivered = state.clone();
    let op = sched::offload(
        move || engine.send(&payload),
        move |outcome: std::io::Result<()>| {
            let result = match outcome {
                Ok(()) => Ok(Value::Bool(true)),
                Err(_) => Err(io_failure(delivered.borrow().closed, &operation, "TLS I/O readiness closed")),
            };
            sched::settle(&target, result);
        },
    );
    let watched = promise.clone();
    let weak = Rc::downgrade(&state);
    promise.listen(move || {
        op.detach();
        if let Some(state) = weak.upgrade() {
            let mut state = state.borrow_mut();
            state.writing = None;
            // Как у Perl: незавершённая запись закрывает состояние.
            if watched.state() != sched::State::Done {
                abort(&mut state);
            }
        }
    });
    Ok(Value::Future(promise))
}

fn shutdown(value: &Value) -> Result<Value, Error> {
    let operation = symbol("shutdown", 1);
    let state = handle::<TlsRef>(value)?.clone();
    let refused = {
        let borrowed = state.borrow();
        if borrowed.reading.is_some() || borrowed.writing.is_some() {
            Some(fail("resource_exhausted", &operation, "TLS shutdown requires drained operations"))
        } else {
            unverified(&borrowed, &operation)
                .or_else(|| borrowed.shutting.is_some().then(|| fail("resource_exhausted", &operation, "TLS operation already pending")))
        }
    };
    if let Some(refused) = refused {
        return Ok(resolved(Err(refused)));
    }
    let promise = sched::promise();
    {
        let mut borrowed = state.borrow_mut();
        borrowed.closing = true;
        borrowed.shutting = Some(promise.clone());
    }
    let engine = state.borrow().engine.clone();
    let target = promise.clone();
    let delivered = state.clone();
    let op = sched::offload(
        move || engine.shutdown(),
        move |outcome: std::io::Result<()>| {
            let result = match outcome {
                Ok(()) => Ok(Value::Bool(true)),
                Err(_) => Err(io_failure(delivered.borrow().closed, &operation, "TLS I/O readiness closed")),
            };
            sched::settle(&target, result);
        },
    );
    let weak = Rc::downgrade(&state);
    promise.listen(move || {
        op.detach();
        if let Some(state) = weak.upgrade() {
            let mut state = state.borrow_mut();
            state.shutting = None;
            abort(&mut state);
        }
    });
    Ok(Value::Future(promise))
}

fn verified(value: &Value, operation: &str) -> Result<TlsRef, Error> {
    let state = state(value, operation).map_err(as_error)?;
    if !state.borrow().authorized {
        return Err(as_error(fail("permission_denied", operation, "TLS peer is not verified")));
    }
    Ok(state)
}

/// Имя набора по OpenSSL: TLS 1.3 совпадает с IANA, TLS 1.2 — `ECDHE-RSA-AES128-GCM-SHA256`.
fn openssl_cipher(iana: &str) -> String {
    let Some(rest) = iana.strip_prefix("TLS_") else {
        return iana.to_string();
    };
    let Some((exchange, rest)) = rest.split_once('_') else {
        return iana.to_string();
    };
    let Some((auth, rest)) = rest.split_once("_WITH_") else {
        return iana.to_string();
    };
    if !matches!(exchange, "ECDHE" | "DHE") || !matches!(auth, "RSA" | "ECDSA") {
        return iana.to_string();
    }
    match rest {
        "CHACHA20_POLY1305_SHA256" | "CHACHA20_POLY1305_SHA384" => format!("{exchange}-{auth}-CHACHA20-POLY1305"),
        _ => {
            for bits in ["128", "256"] {
                for hash in ["SHA256", "SHA384"] {
                    if rest == format!("AES_{bits}_GCM_{hash}") {
                        return format!("{exchange}-{auth}-AES{bits}-GCM-{hash}");
                    }
                }
            }
            iana.to_string()
        }
    }
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let name = native.symbol().split("::").nth(1).and_then(|rest| rest.split('/').next()).unwrap_or("");
    match name {
        "client" => client(args),
        "close" => {
            let state = handle::<TlsRef>(&args[0])?.clone();
            abort(&mut state.borrow_mut());
            let operation = symbol("close", 1);
            let pending: Vec<sched::Future> = {
                let borrowed = state.borrow();
                [&borrowed.handshake, &borrowed.reading, &borrowed.writing, &borrowed.shutting]
                    .into_iter()
                    .flatten()
                    .cloned()
                    .collect()
            };
            for future in pending {
                sched::settle(&future, Err(fail("already_closed", &operation, "TLS owner is closed")));
            }
            Ok(Value::Bool(true))
        }
        "closed" => Ok(Value::Bool(handle::<TlsRef>(&args[0])?.borrow().closed)),
        "handshake" => Ok(handshake(&args[0])),
        "authorize" => authorize(native, args),
        "read" => read(&args[0], int(&args[1])?),
        "write" => write(&args[0], bytes(&args[1])?),
        "shutdown" => shutdown(&args[0]),
        "peer" => {
            let state = verified(&args[0], native.symbol())?;
            let peer = state.borrow().peer.clone();
            Ok(array(peer.into_iter().map(byte_array).collect()))
        }
        "version" => {
            let state = verified(&args[0], native.symbol())?;
            let borrowed = state.borrow();
            let connection = borrowed.engine.connection.lock().expect("TLS lock");
            let version = match connection.protocol_version() {
                Some(rustls::ProtocolVersion::TLSv1_3) => "TLSv1_3",
                Some(rustls::ProtocolVersion::TLSv1_2) => "TLSv1_2",
                _ => "NONE",
            };
            Ok(text(version))
        }
        "cipher" => {
            let state = verified(&args[0], native.symbol())?;
            let borrowed = state.borrow();
            let connection = borrowed.engine.connection.lock().expect("TLS lock");
            let suite = connection
                .negotiated_cipher_suite()
                .and_then(|suite| suite.suite().as_str())
                .unwrap_or("SSL_NULL_WITH_NULL_NULL");
            let iana = suite.replacen("TLS13_", "TLS_", 1);
            Ok(text(openssl_cipher(&iana)))
        }
        "protocol" => {
            let state = verified(&args[0], native.symbol())?;
            let borrowed = state.borrow();
            let connection = borrowed.engine.connection.lock().expect("TLS lock");
            let selected = connection.alpn_protocol().map(|bytes| bytes.iter().map(|b| char::from(*b)).collect::<String>());
            Ok(text(selected.unwrap_or_default()))
        }
        _ => Err(Error::Unsupported(format!("native provider is not implemented: {}", native.symbol()))),
    }
}
