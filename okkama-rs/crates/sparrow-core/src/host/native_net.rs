//! Numeric addresses only: no resolver, network I/O or ambient DNS access.
use crate::host::*;
use okkama_runtime_abi::{Native, NativeFailure};
pub(crate) fn handles(native: Native) -> bool {
    matches!(native, Native::NetIpParse | Native::NetIpFormat)
}
impl Heap {
    fn net_failure(&mut self, native: Native, code: &str, message: &str) -> Result<Outcome, Error> {
        let payload = self.native_failure_new(NativeFailure {
            domain: "core.net".into(),
            code: code.into(),
            operation: native.symbol().into(),
            message: message.into(),
        })?;
        self.failure_new(crate::host::FailureKind::Native(payload))
            .map(Outcome::Failure)
    }
    pub(crate) fn native_net(&mut self, native: Native, args: &[Slot]) -> Result<Outcome, Error> {
        if native == Native::NetIpParse {
            let family = match self.int64(args[0])? {
                Some(1) => 1u8,
                Some(2) => 2u8,
                _ => {
                    return self.net_failure(
                        native,
                        "unsupported",
                        "IP address family is not supported",
                    );
                }
            };
            let Some(port) = self
                .int64(args[2])?
                .filter(|n| (0..=65535).contains(n))
            else {
                return self.net_failure(
                    native,
                    "invalid_input",
                    "Network port must be in range 0..65535",
                );
            };
            let source = self.text(args[1])?;
            let address = if family == 1 {
                source
                    .parse::<std::net::Ipv4Addr>()
                    .ok()
                    .map(|a| a.octets().to_vec())
            } else {
                source
                    .parse::<std::net::Ipv6Addr>()
                    .ok()
                    .map(|a| a.octets().to_vec())
            };
            let Some(address) = address else {
                return self.net_failure(native, "invalid_input", "Invalid numeric IP address");
            };
            let mut bytes = vec![79, 75, 1, family, (port >> 8) as u8, port as u8];
            bytes.extend(address);
            self.native_byte_array(bytes).map(Outcome::Value)
        } else {
            let bytes = self.native_bytes(args[0])?;
            let valid = bytes.len() >= 6
                && bytes[..3] == [79, 75, 1]
                && matches!((bytes[3], bytes.len()), (1, 10) | (2, 22));
            if !valid {
                return self.net_failure(
                    native,
                    "invalid_data",
                    "Invalid network address envelope",
                );
            }
            self.allocate_text(format(&bytes[6..])).map(Outcome::Value)
        }
    }
}
/// `inet_ntop` BSD: самый длинный ряд ≥ 2 нулевых групп — `::`.
pub(crate) fn format(raw: &[u8]) -> String {
    if raw.len() == 4 {
        return format!("{}.{}.{}.{}", raw[0], raw[1], raw[2], raw[3]);
    }
    let words: Vec<u16> = (0..8)
        .map(|index| u16::from_be_bytes([raw[index * 2], raw[index * 2 + 1]]))
        .collect();
    let (mut best, mut best_length): (i64, usize) = (-1, 0);
    let mut index = 0;
    while index < 8 {
        if words[index] != 0 {
            index += 1;
            continue;
        }
        let start = index;
        while index < 8 && words[index] == 0 {
            index += 1;
        }
        if index - start > best_length {
            best = start as i64;
            best_length = index - start;
        }
    }
    if best_length < 2 {
        best = -1;
    }
    let mut out = String::new();
    for index in 0..8 {
        if best >= 0 && (index as i64) >= best && (index as i64) < best + best_length as i64 {
            if index as i64 == best {
                out.push(':');
            }
            continue;
        }
        if index != 0 {
            out.push(':');
        }
        if index == 6
            && best == 0
            && (best_length == 6
                || (best_length == 7 && words[7] != 0x0001)
                || (best_length == 5 && words[5] == 0xffff))
        {
            out.push_str(&format!("{}.{}.{}.{}", raw[12], raw[13], raw[14], raw[15]));
            break;
        }
        out.push_str(&format!("{:x}", words[index]));
    }
    if best >= 0 && best as usize + best_length == 8 {
        out.push(':');
    }
    out
}

