//! System resolution on Tokio's blocking pool with shared admission. A started
//! libc resolver call cannot be aborted: its permit stays owned by the blocking
//! closure until actual completion. No VM data crosses the pool boundary.
use crate::{Error, native::Outcome, native_io::Output, value::*};
use okkama_runtime_abi::{Native, NativeFailure};
use std::{
    collections::HashSet,
    net::{IpAddr, SocketAddr, ToSocketAddrs},
    sync::{Arc, OnceLock},
};

const MAX_SYSTEM_LOOKUPS: usize = 4;
static LOOKUP_SLOTS: OnceLock<Arc<tokio::sync::Semaphore>> = OnceLock::new();

struct AbortQueuedLookup(tokio::task::AbortHandle);
impl Drop for AbortQueuedLookup {
    fn drop(&mut self) {
        // Tokio can cancel queued blocking jobs, but not ones already running.
        self.0.abort();
    }
}

async fn lookup_with_slots<F, T>(
    slots: Arc<tokio::sync::Semaphore>,
    native: Native,
    lookup: F,
) -> Result<T, NativeFailure>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let permit = slots
        .acquire_owned()
        .await
        .map_err(|_| failure(native, "unexpected", "System resolver admission is closed"))?;
    let task = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        lookup()
    });
    let _abort = AbortQueuedLookup(task.abort_handle());
    task.await
        .map_err(|_| failure(native, "unexpected", "System resolver worker failed"))
}

async fn system_lookup<F, T>(native: Native, lookup: F) -> Result<T, NativeFailure>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    let slots =
        LOOKUP_SLOTS.get_or_init(|| Arc::new(tokio::sync::Semaphore::new(MAX_SYSTEM_LOOKUPS)));
    lookup_with_slots(slots.clone(), native, lookup).await
}
pub(crate) fn handles(n: Native) -> bool {
    matches!(n, Native::NetworkDnsAddresses | Native::NetworkDnsName)
}
fn failure(n: Native, code: &str, message: &str) -> NativeFailure {
    NativeFailure {
        domain: "core.net".into(),
        code: code.into(),
        operation: n.symbol().into(),
        message: message.into(),
    }
}
struct Addresses(Vec<Vec<String>>);
impl Output for Addresses {
    fn publish(self: Box<Self>, heap: &mut Heap) -> Result<Slot, Error> {
        let mut addresses = Vec::with_capacity(self.0.len());
        for row in self.0 {
            let values = row
                .into_iter()
                .map(|s| heap.allocate_text(s))
                .collect::<Result<Vec<_>, _>>()?;
            addresses.push(heap.allocate_array(values)?);
        }
        heap.allocate_array(addresses)
    }
}
impl Output for String {
    fn publish(self: Box<Self>, heap: &mut Heap) -> Result<Slot, Error> {
        heap.allocate_text(*self)
    }
}
fn records(n: Native, mut records: Vec<SocketAddr>) -> Result<Addresses, NativeFailure> {
    if records.len() > 256 {
        return Err(failure(
            n,
            "resource_exhausted",
            "System resolution answer exceeds 256 records",
        ));
    }
    records.sort_by_key(SocketAddr::is_ipv6);
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for record in records {
        if !seen.insert(record) {
            continue;
        }
        let (address, scope) = match record {
            SocketAddr::V4(v) => (crate::native_net::format(&v.ip().octets()), String::new()),
            SocketAddr::V6(v) => (
                crate::native_net::format(&v.ip().octets()),
                if v.scope_id() == 0 {
                    String::new()
                } else {
                    v.scope_id().to_string()
                },
            ),
        };
        result.push(vec![address, record.port().to_string(), scope]);
    }
    if result.is_empty() {
        return Err(failure(
            n,
            "not_found",
            "System name resolution returned no usable address",
        ));
    }
    Ok(Addresses(result))
}
impl Heap {
    pub(crate) fn native_dns(&mut self, n: Native, a: &[Slot]) -> Result<Outcome, Error> {
        let source = self.text(a[0])?.to_owned();
        let future = if n == Native::NetworkDnsAddresses {
            let port = self.integer(a[1])?.to_i64();
            self.native_io(async move {
                if source.is_empty()
                    || source.len() > 254
                    || !source
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
                {
                    return Err(failure(
                        n,
                        "invalid_input",
                        "Invalid system resolver hostname",
                    ));
                }
                let port = port
                    .filter(|n| (0..=65535).contains(n))
                    .ok_or_else(|| failure(n, "invalid_input", "Invalid system resolver port"))?;
                if let Ok(address) = source.parse::<IpAddr>() {
                    return records(n, vec![SocketAddr::new(address, port as u16)]);
                }
                let resolved = system_lookup(n, move || {
                    (source.as_str(), port as u16)
                        .to_socket_addrs()
                        .map(|addresses| addresses.take(257).collect())
                })
                .await?
                .map_err(|_| failure(n, "not_found", "Network name was not found"))?;
                records(n, resolved)
            })?
        } else {
            self.native_io(async move {
                if source.is_empty()
                    || source.len() > 45
                    || !source
                        .bytes()
                        .all(|b| b.is_ascii_hexdigit() || b == b':' || b == b'.')
                {
                    return Err(failure(
                        n,
                        "invalid_input",
                        "Reverse lookup requires a bounded numeric IP address",
                    ));
                }
                let address = source.parse::<IpAddr>().map_err(|_| {
                    failure(
                        n,
                        "invalid_input",
                        "Reverse lookup requires a numeric IP address",
                    )
                })?;
                system_lookup(n, move || {
                    let name = dns_lookup::lookup_addr(&address).ok()?;
                    let confirmed = (name.as_str(), 0)
                        .to_socket_addrs()
                        .ok()?
                        .any(|a| a.ip() == address);
                    (confirmed && name != address.to_string()).then_some(name)
                })
                .await?
                .ok_or_else(|| failure(n, "not_found", "Network name was not found"))
            })?
        };
        Ok(Outcome::Value(future))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn numeric_address_keeps_ready_path_without_runtime_or_system_lookup() {
        let mut heap = Heap::new(&[], &[]);
        let host = heap.allocate_text("127.0.0.1".into()).unwrap();
        let Outcome::Value(future) = heap
            .native_dns(Native::NetworkDnsAddresses, &[host, Slot::small(42)])
            .unwrap()
        else {
            panic!("missing address Future");
        };
        let request = heap.future(future).unwrap().io.clone().unwrap();
        assert!(
            request
                .poll(&mut std::task::Context::from_waker(std::task::Waker::noop()))
                .is_ready()
        );
        heap.native_io_settle(future).unwrap();
        let crate::future::State::Ready(Ok(value)) = heap.future(future).unwrap().state else {
            panic!("numeric address unexpectedly requires external resolution");
        };
        let rows = heap.elements(value).unwrap();
        assert_eq!(rows.len(), 1);
        let fields = heap.elements(rows[0]).unwrap();
        assert_eq!(heap.text(fields[0]).unwrap(), "127.0.0.1");
        assert_eq!(heap.text(fields[1]).unwrap(), "42");
    }

    #[test]
    fn cancelled_admission_never_starts_system_lookup() {
        use std::future::Future;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::task::{Context, Waker};
        let slots = Arc::new(tokio::sync::Semaphore::new(1));
        let held = slots.clone().try_acquire_owned().unwrap();
        let started = Arc::new(AtomicBool::new(false));
        let flag = started.clone();
        let mut lookup = Box::pin(lookup_with_slots(
            slots.clone(),
            Native::NetworkDnsName,
            move || {
                flag.store(true, Ordering::SeqCst);
                42
            },
        ));
        assert!(
            lookup
                .as_mut()
                .poll(&mut Context::from_waker(Waker::noop()))
                .is_pending()
        );
        drop(lookup);
        drop(held);
        assert!(!started.load(Ordering::SeqCst));
        assert_eq!(slots.available_permits(), 1);
    }

    #[test]
    fn dropping_running_lookup_keeps_admission_until_work_finishes() {
        use std::future::Future;
        use std::task::{Context, Waker};
        use std::time::Duration;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            let slots = Arc::new(tokio::sync::Semaphore::new(1));
            let (started, begin) = tokio::sync::oneshot::channel();
            let (release, wait) = std::sync::mpsc::channel();
            let mut lookup = Box::pin(lookup_with_slots(
                slots.clone(),
                Native::NetworkDnsName,
                move || {
                    let _ = started.send(());
                    // Bounded even if the test fails; no resolver/network operation.
                    let _ = wait.recv_timeout(Duration::from_secs(1));
                    42
                },
            ));
            assert!(
                lookup
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
            tokio::time::timeout(Duration::from_secs(1), begin)
                .await
                .unwrap()
                .unwrap();
            drop(lookup);
            assert_eq!(slots.available_permits(), 0);
            release.send(()).unwrap();
            let permit =
                tokio::time::timeout(Duration::from_secs(1), slots.clone().acquire_owned())
                    .await
                    .unwrap()
                    .unwrap();
            drop(permit);
            assert_eq!(slots.available_permits(), 1);
        });
    }

    #[test]
    fn dropping_queued_lookup_prevents_its_closure_from_running() {
        use std::future::Future;
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::task::{Context, Waker};
        use std::time::Duration;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .max_blocking_threads(1)
            .build()
            .unwrap();
        runtime.block_on(async {
            let (started, begin) = tokio::sync::oneshot::channel();
            let (release, wait) = std::sync::mpsc::channel();
            let blocker = tokio::task::spawn_blocking(move || {
                let _ = started.send(());
                let _ = wait.recv_timeout(Duration::from_secs(1));
            });
            tokio::time::timeout(Duration::from_secs(1), begin)
                .await
                .unwrap()
                .unwrap();
            let slots = Arc::new(tokio::sync::Semaphore::new(1));
            let ran = Arc::new(AtomicBool::new(false));
            let flag = ran.clone();
            let mut lookup = Box::pin(lookup_with_slots(
                slots.clone(),
                Native::NetworkDnsName,
                move || {
                    flag.store(true, Ordering::SeqCst);
                },
            ));
            assert!(
                lookup
                    .as_mut()
                    .poll(&mut Context::from_waker(Waker::noop()))
                    .is_pending()
            );
            drop(lookup);
            release.send(()).unwrap();
            blocker.await.unwrap();
            let permit = tokio::time::timeout(Duration::from_secs(1), slots.acquire_owned())
                .await
                .unwrap()
                .unwrap();
            drop(permit);
            assert!(!ran.load(Ordering::SeqCst));
        });
    }

    #[test]
    fn answers_preserve_family_order_scope_port_and_remove_duplicates() {
        let a = "127.0.0.1:42".parse().unwrap();
        let b = "[::1]:42".parse().unwrap();
        let scoped = SocketAddr::V6(std::net::SocketAddrV6::new(
            "fe80::1".parse().unwrap(),
            42,
            0,
            7,
        ));
        let result = records(Native::NetworkDnsAddresses, vec![b, a, a, scoped]).unwrap();
        assert_eq!(
            result.0,
            vec![
                vec!["127.0.0.1", "42", ""],
                vec!["::1", "42", ""],
                vec!["fe80::1", "42", "7"]
            ]
        );
        assert!(records(Native::NetworkDnsAddresses, vec![a; 257]).is_err());
        assert!(records(Native::NetworkDnsAddresses, vec![]).is_err());
    }
}
