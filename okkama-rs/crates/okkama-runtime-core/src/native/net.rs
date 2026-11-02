//! `runtime/net` (числовые IP-адреса) — часть `NetNatives` JVM: текст
//! `inet_pton`/`inet_ntop` и конверт адреса `'O' 'K' 1 family port(be) address`.
use super::Error;
use super::support::{byte_array, bytes, failure, int, string, text};
use crate::{Value, abi::Native};

pub const IPV4: u8 = 1;
pub const IPV6: u8 = 2;

pub fn parse_ipv4(text: &str) -> Option<[u8; 4]> {
    let parts: Vec<&str> = text.split('.').collect();
    if parts.len() != 4 {
        return None;
    }
    let mut out = [0u8; 4];
    for (index, part) in parts.iter().enumerate() {
        let canonical = *part == "0"
            || (!part.is_empty() && part.len() <= 3 && !part.starts_with('0') && part.bytes().all(|b| b.is_ascii_digit()));
        if !canonical {
            return None;
        }
        out[index] = u8::try_from(part.parse::<u32>().ok()?).ok()?;
    }
    Some(out)
}

/// Группы `h16` через `:`; последняя может быть IPv4 (две группы).
fn groups(text: &str, out: &mut Vec<u16>, last: bool) -> bool {
    if text.is_empty() {
        return true;
    }
    let parts: Vec<&str> = text.split(':').collect();
    for (index, part) in parts.iter().enumerate() {
        if last && index == parts.len() - 1 && part.contains('.') {
            let Some(four) = parse_ipv4(part) else {
                return false;
            };
            out.push(u16::from_be_bytes([four[0], four[1]]));
            out.push(u16::from_be_bytes([four[2], four[3]]));
            continue;
        }
        if part.is_empty() || part.len() > 4 || !part.bytes().all(|b| b.is_ascii_hexdigit()) {
            return false;
        }
        out.push(u16::from_str_radix(part, 16).expect("hex group"));
    }
    true
}

pub fn parse_ipv6(text: &str) -> Option<[u8; 16]> {
    let gap = text.find("::");
    if let Some(at) = gap {
        if text[at + 1..].contains("::") {
            return None;
        }
    }
    let (mut head, mut tail) = (Vec::new(), Vec::new());
    match gap {
        Some(at) => {
            if !groups(&text[..at], &mut head, false) || !groups(&text[at + 2..], &mut tail, true) {
                return None;
            }
            if head.len() + tail.len() > 7 {
                return None;
            }
        }
        None => {
            if !groups(text, &mut head, true) || head.len() != 8 {
                return None;
            }
        }
    }
    let mut out = [0u8; 16];
    for (index, word) in head.iter().enumerate() {
        out[index * 2..index * 2 + 2].copy_from_slice(&word.to_be_bytes());
    }
    let base = 8 - tail.len();
    for (index, word) in tail.iter().enumerate() {
        out[(base + index) * 2..(base + index) * 2 + 2].copy_from_slice(&word.to_be_bytes());
    }
    Some(out)
}

/// `inet_ntop` BSD: самый длинный ряд ≥ 2 нулевых групп — `::`.
pub fn format(raw: &[u8]) -> String {
    if raw.len() == 4 {
        return format!("{}.{}.{}.{}", raw[0], raw[1], raw[2], raw[3]);
    }
    let words: Vec<u16> = (0..8).map(|index| u16::from_be_bytes([raw[index * 2], raw[index * 2 + 1]])).collect();
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

pub fn encode(family: u8, port: u16, address: &[u8]) -> Vec<u8> {
    let mut bytes = vec![79, 75, 1, family, (port >> 8) as u8, port as u8];
    bytes.extend_from_slice(address);
    bytes
}

/// Семейство, порт и адрес конверта.
pub fn decode(native: Native, value: &Value) -> Result<(u8, u16, Vec<u8>), Error> {
    let bytes = bytes(value)?;
    let family = if bytes.len() >= 6 { bytes[3] } else { 0 };
    let expected = match family {
        IPV4 => 10,
        IPV6 => 22,
        _ => 0,
    };
    if bytes.len() < 6 || bytes[..3] != [79, 75, 1] || expected == 0 || bytes.len() != expected {
        return Err(failure("core.net", "invalid_data", native, "Invalid network address envelope"));
    }
    Ok((family, u16::from_be_bytes([bytes[4], bytes[5]]), bytes[6..].to_vec()))
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let operation = native.name();
    match operation {
        "ip_parse" => {
            let kind = match int(&args[0])?.to_i64() {
                Some(1) => IPV4,
                Some(2) => IPV6,
                _ => {
                    return Err(failure(
                        "core.net",
                        "unsupported",
                        native,
                        "IP address family is not supported",
                    ));
                }
            };
            let port = int(&args[2])?
                .to_i64()
                .filter(|port| (0..=65535).contains(port))
                .ok_or_else(|| {
                    failure(
                        "core.net",
                        "invalid_input",
                        native,
                        "Network port must be in range 0..65535",
                    )
                })? as u16;
            let source = string(&args[1])?;
            let invalid = || failure("core.net", "invalid_input", native, "Invalid numeric IP address");
            if kind == IPV4 || source.contains('.') {
                let dotted = source.rsplit(':').next().unwrap_or(source);
                if parse_ipv4(dotted).is_none() {
                    return Err(invalid());
                }
            }
            let address = if kind == IPV4 {
                parse_ipv4(source).map(|a| a.to_vec())
            } else {
                parse_ipv6(source).map(|a| a.to_vec())
            }
            .ok_or_else(invalid)?;
            Ok(byte_array(encode(kind, port, &address)))
        }
        "ip_format" => Ok(text(format(&decode(native, &args[0])?.2))),
        _ => Err(Error::Unsupported(format!(
            "native provider is not implemented: {}",
            native.symbol()
        ))),
    }
}
