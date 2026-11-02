//! `runtime/hex`, `runtime/base32`, `runtime/base64`, `runtime/base85`,
//! `runtime/quoted_printable`, `runtime/single_byte` — перенос
//! `EncodingNatives`/`TextNatives` JVM-рантайма один к одному. Смещения в
//! строках — в кодовых точках. Сбой provider (неверный алфавит, недопустимый
//! вход декодера) — `Native invocation "…" failed`.

use super::Error;
use crate::{IntValue, Value, abi::Native, heap::ArrayHeap};
use std::rc::Rc;

const BASE32: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const BASE64_STANDARD: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const BASE64_URL: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const RFC1924: &str =
    "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~";

fn failed(native: Native) -> Error {
    Error::Trap(format!("Native invocation \"{}\" failed", native.symbol()))
}

fn bytes(value: &Value, arrays: &ArrayHeap) -> Result<Vec<u8>, Error> {
    Ok(arrays
        .elements(value)?
        .iter()
        .map(Value::as_byte)
        .collect::<Result<Vec<_>, _>>()?)
}

fn byte_array(bytes: Vec<u8>, arrays: &mut ArrayHeap) -> Value {
    arrays.allocate(bytes.into_iter().map(Value::Byte).collect())
}

fn int(value: i64) -> Value {
    Value::Int(IntValue::from_i64(value))
}

fn text(value: String) -> Value {
    Value::Str(Rc::from(value))
}

fn points(text: &str) -> Vec<u32> {
    text.chars().map(u32::from).collect()
}

fn ascii85() -> String {
    (33u8..=117).map(char::from).collect()
}

/// Позиция символа в алфавите (ASCII-алфавиты: байт = символ).
fn index(alphabet: &[u8], symbol: u32) -> i64 {
    if symbol > 0x7f {
        return -1;
    }
    alphabet
        .iter()
        .position(|candidate| u32::from(*candidate) == symbol)
        .map_or(-1, |at| at as i64)
}

fn hex_digit(symbol: u32) -> i64 {
    match char::from_u32(symbol) {
        Some(digit @ '0'..='9') => i64::from(digit as u8 - b'0'),
        Some(digit @ 'a'..='f') => i64::from(digit as u8 - b'a' + 10),
        Some(digit @ 'A'..='F') => i64::from(digit as u8 - b'A' + 10),
        _ => -1,
    }
}

/// Символ на каждые `width` бит, затем `=` до кратности `quantum`.
fn encoded(octets: &[u8], alphabet: &[u8], width: u32, quantum: usize) -> String {
    let mut result = String::new();
    let mut buffer: u64 = 0;
    let mut bits = 0u32;
    let mask = (1u64 << width) - 1;
    for octet in octets {
        buffer = (buffer << 8) | u64::from(*octet);
        bits += 8;
        while bits >= width {
            bits -= width;
            result.push(char::from(alphabet[((buffer >> bits) & mask) as usize]));
        }
        buffer &= (1u64 << bits) - 1;
    }
    if bits > 0 {
        result.push(char::from(
            alphabet[((buffer << (width - bits)) & mask) as usize],
        ));
    }
    while result.len() % quantum != 0 {
        result.push('=');
    }
    result
}

fn expected_padding(width: u32, remainder: usize) -> i64 {
    if width == 5 {
        return match remainder {
            0 => 0,
            2 => 6,
            4 => 4,
            5 => 3,
            7 => 1,
            _ => -1,
        };
    }
    match remainder {
        0 => 0,
        2 => 2,
        3 => 1,
        _ => -1,
    }
}

/// `(первое неверное смещение или -1, конец данных)`, как `shape` JVM.
fn shape(source: &[u32], alphabet: &[u8], width: u32, quantum: usize) -> (i64, usize) {
    let size = source.len();
    if size == 0 {
        return (-1, 0);
    }
    let mut data_end = size;
    let mut padding = 0i64;
    let mut inside = false;
    for (offset, symbol) in source.iter().enumerate() {
        if *symbol == u32::from('=') {
            if !inside {
                inside = true;
                data_end = offset;
            }
            padding += 1;
            continue;
        }
        if inside || index(alphabet, *symbol) < 0 {
            return (offset as i64, 0);
        }
    }
    if size % quantum != 0 {
        return (size as i64, 0);
    }
    let expected = expected_padding(width, data_end % quantum);
    if expected < 0 || padding != expected {
        return (data_end as i64, 0);
    }
    let mut buffer: u64 = 0;
    let mut bits = 0u32;
    for symbol in &source[..data_end] {
        buffer = (buffer << width) | index(alphabet, *symbol) as u64;
        bits += width;
        while bits >= 8 {
            bits -= 8;
        }
        buffer &= (1u64 << bits) - 1;
    }
    if bits > 0 && buffer & ((1u64 << bits) - 1) != 0 {
        return (data_end as i64 - 1, 0);
    }
    (-1, data_end)
}

fn decoded(source: &[u32], data_end: usize, alphabet: &[u8], width: u32) -> Vec<u8> {
    let mut octets = Vec::new();
    let mut buffer: u64 = 0;
    let mut bits = 0u32;
    for symbol in &source[..data_end] {
        buffer = (buffer << width) | index(alphabet, *symbol) as u64;
        bits += width;
        while bits >= 8 {
            bits -= 8;
            octets.push(((buffer >> bits) & 255) as u8);
        }
        buffer &= (1u64 << bits) - 1;
    }
    octets
}

fn base64_alphabet(native: Native, name: &str) -> Result<&'static [u8], Error> {
    match name {
        "standard" => Ok(BASE64_STANDARD.as_bytes()),
        "url" => Ok(BASE64_URL.as_bytes()),
        _ => Err(failed(native)),
    }
}

fn base85_alphabet(native: Native, name: &str) -> Result<Vec<u8>, Error> {
    match name {
        "ascii85" => Ok(ascii85().into_bytes()),
        "rfc1924" => Ok(RFC1924.as_bytes().to_vec()),
        _ => Err(failed(native)),
    }
}

fn base85_encoded(octets: &[u8], name: &str, alphabet: &[u8]) -> String {
    let mut result = String::new();
    for chunk in octets.chunks(4) {
        let amount = chunk.len();
        let mut word: u64 = 0;
        for index in 0..4 {
            word = (word << 8) | u64::from(*chunk.get(index).unwrap_or(&0));
        }
        if name == "ascii85" && amount == 4 && word == 0 {
            result.push('z');
            continue;
        }
        let mut digits = [0u8; 5];
        for index in (0..5).rev() {
            digits[index] = alphabet[(word % 85) as usize];
            word /= 85;
        }
        let take = if amount == 4 { 5 } else { amount + 1 };
        result.extend(digits[..take].iter().map(|digit| char::from(*digit)));
    }
    result
}

fn word_bytes(word: u64, out: &mut Vec<u8>, count: usize) {
    for index in 0..count {
        out.push(((word >> (24 - 8 * index)) & 255) as u8);
    }
}

/// Первое неверное смещение или -1; `octets` — результат.
fn base85_parsed(source: &[u32], name: &str, alphabet: &[u8], octets: &mut Vec<u8>) -> i64 {
    let mut values: Vec<u64> = Vec::new();
    let mut group_start = 0usize;
    let size = source.len();
    for (offset, symbol) in source.iter().enumerate() {
        if name == "ascii85" && *symbol == u32::from('z') {
            if !values.is_empty() {
                return offset as i64;
            }
            word_bytes(0, octets, 4);
            group_start = offset + 1;
            continue;
        }
        let value = index(alphabet, *symbol);
        if value < 0 {
            return offset as i64;
        }
        if values.is_empty() {
            group_start = offset;
        }
        values.push(value as u64);
        if values.len() == 5 {
            let word = values.iter().fold(0u64, |word, value| word * 85 + value);
            if word > 0xFFFF_FFFF {
                return offset as i64;
            }
            word_bytes(word, octets, 4);
            values.clear();
            group_start = offset + 1;
        }
    }
    if values.len() == 1 {
        return size as i64;
    }
    if !values.is_empty() {
        let amount = values.len();
        while values.len() < 5 {
            values.push(84);
        }
        let word = values.iter().fold(0u64, |word, value| word * 85 + value);
        if word > 0xFFFF_FFFF {
            return size as i64 - 1;
        }
        let mut partial = Vec::new();
        word_bytes(word, &mut partial, amount - 1);
        let canonical = base85_encoded(&partial, name, alphabet);
        let end = (group_start + amount).min(size);
        let spelling: String = source[group_start..end]
            .iter()
            .filter_map(|symbol| char::from_u32(*symbol))
            .collect();
        if canonical != spelling {
            return size as i64 - 1;
        }
        octets.extend(partial);
    }
    -1
}

fn qp_token(octet: u8, trailing: bool) -> String {
    if octet == 61 || !(32..=126).contains(&octet) || ((octet == 9 || octet == 32) && trailing) {
        format!("={octet:02X}")
    } else {
        char::from(octet).to_string()
    }
}

/// Мягкие переносы `=\r\n`: строка не длиннее 76 (75, если токены ещё есть).
fn qp_folded(result: &mut String, tokens: &mut Vec<String>, hard_break: bool) {
    let mut line = String::new();
    while !tokens.is_empty() {
        let limit = if tokens.len() > 1 { 75 } else { 76 };
        if line.len() + tokens[0].len() > limit {
            result.push_str(&line);
            result.push_str("=\r\n");
            line.clear();
            continue;
        }
        line.push_str(&tokens.remove(0));
    }
    result.push_str(&line);
    if hard_break {
        result.push_str("\r\n");
    }
}

fn qp_encode(origin: &[u8]) -> Vec<u8> {
    let mut result = String::new();
    let mut tokens = Vec::new();
    let size = origin.len();
    let mut offset = 0;
    while offset < size {
        if origin[offset] == 13 && offset + 1 < size && origin[offset + 1] == 10 {
            qp_folded(&mut result, &mut tokens, true);
            offset += 2;
            continue;
        }
        let trailing = offset + 1 == size
            || (offset + 2 < size && origin[offset + 1] == 13 && origin[offset + 2] == 10);
        tokens.push(qp_token(origin[offset], trailing));
        offset += 1;
    }
    qp_folded(&mut result, &mut tokens, false);
    result.into_bytes()
}

fn qp_hex(octet: u8) -> i64 {
    match octet {
        48..=57 => i64::from(octet - 48),
        65..=70 => i64::from(octet - 65 + 10),
        97..=102 => i64::from(octet - 97 + 10),
        _ => -1,
    }
}

fn qp_parsed(octets: &[u8], decoded: &mut Vec<u8>) -> i64 {
    let size = octets.len();
    let mut offset = 0usize;
    let mut column = 0usize;
    while offset < size {
        let octet = octets[offset];
        if octet == 13 {
            if offset + 1 >= size || octets[offset + 1] != 10 {
                return offset as i64;
            }
            decoded.extend([13, 10]);
            column = 0;
            offset += 2;
            continue;
        }
        if octet == 10 {
            return offset as i64;
        }
        if octet == 61 {
            if offset + 2 < size && octets[offset + 1] == 13 && octets[offset + 2] == 10 {
                if column + 1 > 76 {
                    return offset as i64;
                }
                column = 0;
                offset += 3;
                continue;
            }
            if offset + 2 >= size {
                return offset as i64;
            }
            let (high, low) = (qp_hex(octets[offset + 1]), qp_hex(octets[offset + 2]));
            if high < 0 || low < 0 {
                return offset as i64;
            }
            if column + 3 > 76 {
                return (offset + (76 - column)) as i64;
            }
            decoded.push((high * 16 + low) as u8);
            column += 3;
            offset += 3;
            continue;
        }
        if octet != 9 && octet != 32 && !(33..=126).contains(&octet) {
            return offset as i64;
        }
        if (octet == 9 || octet == 32)
            && (offset + 1 == size
                || (offset + 2 < size && octets[offset + 1] == 13 && octets[offset + 2] == 10))
        {
            return offset as i64;
        }
        if column + 1 > 76 {
            return offset as i64;
        }
        decoded.push(octet);
        column += 1;
        offset += 1;
    }
    -1
}

fn prefix(text: &str, maximum: u32) -> i64 {
    text.chars()
        .take_while(|ch| u32::from(*ch) <= maximum)
        .count() as i64
}

/// Байты по кодовым точкам (младший байт, как `(byte) scalar` JVM).
fn single_byte_encoded(text: &str) -> Vec<u8> {
    text.chars().map(|ch| u32::from(ch) as u8).collect()
}

fn single_byte_decoded(octets: &[u8]) -> String {
    octets.iter().map(|octet| char::from(*octet)).collect()
}

pub fn handles(native: Native) -> bool {
    use Native::*;
    matches!(
        native,
        HexEncode
            | HexValidPrefix
            | HexDecode
            | Base32Encode
            | Base32InvalidOffset
            | Base32Decode
            | Base64Encode
            | Base64InvalidOffset
            | Base64Decode
            | Base85Encode
            | Base85InvalidOffset
            | Base85Decode
            | QuotedPrintableEncode
            | QuotedPrintableInvalidOffset
            | QuotedPrintableDecode
            | AsciiRepresentablePrefix
            | AsciiEncode
            | AsciiValidPrefix
            | AsciiDecode
            | Latin1RepresentablePrefix
            | Latin1Encode
            | Latin1Decode
    )
}

pub fn execute(native: Native, args: &[Value], arrays: &mut ArrayHeap) -> Result<Value, Error> {
    use Native::*;
    Ok(match native {
        HexEncode => text(
            bytes(&args[0], arrays)?
                .iter()
                .map(|octet| format!("{octet:02x}"))
                .collect(),
        ),
        HexValidPrefix => int(points(args[0].as_string()?)
            .iter()
            .take_while(|symbol| hex_digit(**symbol) >= 0)
            .count() as i64),
        HexDecode => {
            let source = points(args[0].as_string()?);
            let mut octets = Vec::new();
            for pair in source.chunks(2) {
                let high = hex_digit(pair[0]);
                let low = pair.get(1).map_or(-1, |symbol| hex_digit(*symbol));
                if high < 0 || low < 0 {
                    return Err(failed(native));
                }
                octets.push((high * 16 + low) as u8);
            }
            byte_array(octets, arrays)
        }
        Base32Encode => text(encoded(&bytes(&args[0], arrays)?, BASE32.as_bytes(), 5, 8)),
        Base32InvalidOffset => int(shape(&points(args[0].as_string()?), BASE32.as_bytes(), 5, 8).0),
        Base32Decode => {
            let source = points(args[0].as_string()?);
            let (invalid, end) = shape(&source, BASE32.as_bytes(), 5, 8);
            if invalid != -1 {
                return Err(failed(native));
            }
            byte_array(decoded(&source, end, BASE32.as_bytes(), 5), arrays)
        }
        Base64Encode => {
            let alphabet = base64_alphabet(native, args[1].as_string()?)?;
            text(encoded(&bytes(&args[0], arrays)?, alphabet, 6, 4))
        }
        Base64InvalidOffset => {
            let alphabet = base64_alphabet(native, args[1].as_string()?)?;
            int(shape(&points(args[0].as_string()?), alphabet, 6, 4).0)
        }
        Base64Decode => {
            let alphabet = base64_alphabet(native, args[1].as_string()?)?;
            let source = points(args[0].as_string()?);
            let (invalid, end) = shape(&source, alphabet, 6, 4);
            if invalid != -1 {
                return Err(failed(native));
            }
            byte_array(decoded(&source, end, alphabet, 6), arrays)
        }
        Base85Encode => {
            let name = args[1].as_string()?.to_string();
            let alphabet = base85_alphabet(native, &name)?;
            text(base85_encoded(&bytes(&args[0], arrays)?, &name, &alphabet))
        }
        Base85InvalidOffset => {
            let name = args[1].as_string()?.to_string();
            let alphabet = base85_alphabet(native, &name)?;
            int(base85_parsed(
                &points(args[0].as_string()?),
                &name,
                &alphabet,
                &mut Vec::new(),
            ))
        }
        Base85Decode => {
            let name = args[1].as_string()?.to_string();
            let alphabet = base85_alphabet(native, &name)?;
            let mut octets = Vec::new();
            if base85_parsed(&points(args[0].as_string()?), &name, &alphabet, &mut octets) != -1 {
                return Err(failed(native));
            }
            byte_array(octets, arrays)
        }
        QuotedPrintableEncode => {
            let encoded = qp_encode(&bytes(&args[0], arrays)?);
            byte_array(encoded, arrays)
        }
        QuotedPrintableInvalidOffset => int(qp_parsed(&bytes(&args[0], arrays)?, &mut Vec::new())),
        QuotedPrintableDecode => {
            let mut decoded = Vec::new();
            if qp_parsed(&bytes(&args[0], arrays)?, &mut decoded) != -1 {
                return Err(failed(native));
            }
            byte_array(decoded, arrays)
        }
        AsciiRepresentablePrefix => int(prefix(args[0].as_string()?, 0x7f)),
        Latin1RepresentablePrefix => int(prefix(args[0].as_string()?, 0xff)),
        AsciiEncode | Latin1Encode => {
            let encoded = single_byte_encoded(args[0].as_string()?);
            byte_array(encoded, arrays)
        }
        AsciiValidPrefix => int(bytes(&args[0], arrays)?
            .iter()
            .take_while(|octet| **octet < 0x80)
            .count() as i64),
        AsciiDecode | Latin1Decode => text(single_byte_decoded(&bytes(&args[0], arrays)?)),
        _ => return Err(Error::InvalidState("not an encoding native".into())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rfc4648_vectors() {
        for (input, b64, b32) in [
            ("", "", ""),
            ("f", "Zg==", "MY======"),
            ("fo", "Zm8=", "MZXQ===="),
            ("foo", "Zm9v", "MZXW6==="),
            ("foob", "Zm9vYg==", "MZXW6YQ="),
            ("fooba", "Zm9vYmE=", "MZXW6YTB"),
            ("foobar", "Zm9vYmFy", "MZXW6YTBOI======"),
        ] {
            assert_eq!(
                encoded(input.as_bytes(), BASE64_STANDARD.as_bytes(), 6, 4),
                b64
            );
            assert_eq!(encoded(input.as_bytes(), BASE32.as_bytes(), 5, 8), b32);
            let source = points(b64);
            let (invalid, end) = shape(&source, BASE64_STANDARD.as_bytes(), 6, 4);
            assert_eq!(invalid, -1);
            assert_eq!(
                decoded(&source, end, BASE64_STANDARD.as_bytes(), 6),
                input.as_bytes()
            );
        }
        assert_eq!(
            shape(&points("Zh=="), BASE64_STANDARD.as_bytes(), 6, 4).0,
            1
        );
    }

    #[test]
    fn ascii85_round_trips_and_zero_groups_shrink() {
        let alphabet = ascii85().into_bytes();
        let encoded = base85_encoded(b"Man is", "ascii85", &alphabet);
        assert_eq!(encoded, "9jqo^Bla");
        assert_eq!(base85_encoded(&[0, 0, 0, 0], "ascii85", &alphabet), "z");
        let mut out = Vec::new();
        assert_eq!(
            base85_parsed(&points(&encoded), "ascii85", &alphabet, &mut out),
            -1
        );
        assert_eq!(out, b"Man is");
    }

    #[test]
    fn quoted_printable_escapes_and_round_trips() {
        let encoded = qp_encode(b"a=b \xff\r\nend ");
        assert_eq!(encoded, b"a=3Db =FF\r\nend=20");
        let mut decoded = Vec::new();
        assert_eq!(qp_parsed(&encoded, &mut decoded), -1);
        assert_eq!(decoded, b"a=b \xff\r\nend ");
    }
}
