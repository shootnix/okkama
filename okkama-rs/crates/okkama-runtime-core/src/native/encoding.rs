//! `runtime/hex`, `runtime/base32`, `runtime/base64`, `runtime/base85`,
//! `runtime/quoted_printable`, `runtime/single_byte` — перенос
//! Смещения в
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
    let octets_per_quantum = quantum * width as usize / 8;
    let capacity = octets
        .len()
        .div_ceil(octets_per_quantum)
        .checked_mul(quantum);
    let mut result = String::with_capacity(capacity.unwrap_or(0));
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
    while !result.len().is_multiple_of(quantum) {
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

// Compile-time tables avoid per-symbol alphabet scans (#461).
const fn reverse_alphabet(alphabet: &[u8]) -> [u8; 128] {
    let mut reverse = [u8::MAX; 128];
    let mut at = 0;
    while at < alphabet.len() {
        reverse[alphabet[at] as usize] = at as u8;
        at += 1;
    }
    reverse
}

const BASE32_REVERSE: [u8; 128] = reverse_alphabet(BASE32.as_bytes());
const BASE64_STANDARD_REVERSE: [u8; 128] = reverse_alphabet(BASE64_STANDARD.as_bytes());
const BASE64_URL_REVERSE: [u8; 128] = reverse_alphabet(BASE64_URL.as_bytes());

// Borrow input until validation finishes; only successful parses return bytes (#461).
fn base_parsed(
    source: &str,
    alphabet: &[u8],
    width: u32,
    quantum: usize,
    decode: bool,
) -> Result<Vec<u8>, i64> {
    let reverse = if width == 5 {
        &BASE32_REVERSE
    } else if alphabet == BASE64_URL.as_bytes() {
        &BASE64_URL_REVERSE
    } else {
        &BASE64_STANDARD_REVERSE
    };
    let mut octets = Vec::new();
    if decode
        && let Some(capacity) = (source.len() / quantum).checked_mul(quantum * width as usize / 8)
    {
        octets.reserve_exact(capacity);
    }
    let mut size = 0usize;
    let mut data_end = 0usize;
    let mut padding = 0usize;
    let mut buffer = 0u64;
    let mut bits = 0u32;
    // Scalar enumeration preserves offsets for every non-ASCII error (#461).
    for (offset, symbol) in source.chars().enumerate() {
        size = offset + 1;
        if symbol == '=' {
            padding += 1;
            continue;
        }
        let value = reverse.get(symbol as usize).copied().unwrap_or(u8::MAX);
        if padding != 0 || value == u8::MAX {
            return Err(offset as i64);
        }
        data_end += 1;
        buffer = (buffer << width) | u64::from(value);
        bits += width;
        if bits >= 8 {
            bits -= 8;
            if decode {
                octets.push(((buffer >> bits) & 255) as u8);
            }
        }
        buffer &= (1u64 << bits) - 1;
    }
    // Tail errors must not hide length or padding errors (#461).
    if !size.is_multiple_of(quantum) {
        return Err(size as i64);
    }
    let expected = expected_padding(width, data_end % quantum);
    if expected < 0 || padding != expected as usize {
        return Err(data_end as i64);
    }
    if buffer != 0 {
        return Err(data_end as i64 - 1);
    }
    Ok(octets)
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

// Compile-time tables avoid per-symbol alphabet scans (#551).
const ASCII85_REVERSE: [u8; 128] = {
    let mut reverse = [u8::MAX; 128];
    let mut value = 0;
    while value < 85 {
        reverse[value + 33] = value as u8;
        value += 1;
    }
    reverse
};
const RFC1924_REVERSE: [u8; 128] = reverse_alphabet(RFC1924.as_bytes());

// An optional borrowed sink preserves error order without collecting validation output (#459).
fn base85_parsed(
    source: &str,
    name: &str,
    alphabet: &[u8],
    mut octets: Option<&mut Vec<u8>>,
) -> i64 {
    let reverse = if name == "ascii85" {
        &ASCII85_REVERSE
    } else {
        &RFC1924_REVERSE
    };
    let mut word = 0u64;
    let mut amount = 0usize;
    let mut spelling = [0u8; 5];
    let mut size = 0usize;
    // Scalar enumeration preserves error offsets without materialising the input (#551).
    for (offset, symbol) in source.chars().enumerate() {
        size = offset + 1;
        if name == "ascii85" && symbol == 'z' {
            if amount != 0 {
                return offset as i64;
            }
            if let Some(octets) = octets.as_mut() {
                word_bytes(0, octets, 4);
            }
            continue;
        }
        let value = reverse.get(symbol as usize).copied().unwrap_or(u8::MAX);
        if value == u8::MAX {
            return offset as i64;
        }
        spelling[amount] = symbol as u8;
        amount += 1;
        word = word * 85 + u64::from(value);
        if amount == 5 {
            if word > 0xFFFF_FFFF {
                return offset as i64;
            }
            if let Some(octets) = octets.as_mut() {
                word_bytes(word, octets, 4);
            }
            word = 0;
            amount = 0;
        }
    }
    if amount == 1 {
        return size as i64;
    }
    if amount != 0 {
        for _ in amount..5 {
            word = word * 85 + 84;
        }
        if word > 0xFFFF_FFFF {
            return size as i64 - 1;
        }
        // Fixed buffers keep the canonical tail check allocation-free (#551).
        let partial = (word as u32).to_be_bytes();
        let partial = &partial[..amount - 1];
        let mut padded = [0u8; 4];
        padded[..amount - 1].copy_from_slice(partial);
        let mut word = u32::from_be_bytes(padded);
        let mut canonical = [0u8; 5];
        for digit in canonical.iter_mut().rev() {
            *digit = alphabet[(word % 85) as usize];
            word /= 85;
        }
        if canonical[..amount] != spelling[..amount] {
            return size as i64 - 1;
        }
        if let Some(octets) = octets {
            octets.extend_from_slice(partial);
        }
    }
    -1
}

fn qp_token(octet: u8, trailing: bool) -> ([u8; 3], usize) {
    if octet == 61 || !(32..=126).contains(&octet) || ((octet == 9 || octet == 32) && trailing) {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        (
            [b'=', HEX[usize::from(octet >> 4)], HEX[usize::from(octet & 15)]],
            3,
        )
    } else {
        ([octet, 0, 0], 1)
    }
}

/// Мягкие переносы `=\r\n`: строка не длиннее 76 (75, если токены ещё есть).
fn qp_folded(result: &mut Vec<u8>, column: &mut usize, octet: u8, trailing: bool) {
    let (token, width) = qp_token(octet, trailing);
    let limit = if trailing { 76 } else { 75 };
    if *column + width > limit {
        result.extend_from_slice(b"=\r\n");
        *column = 0;
    }
    result.extend_from_slice(&token[..width]);
    *column += width;
}

fn qp_encode(origin: &[u8]) -> Vec<u8> {
    let mut result = Vec::new();
    let mut column = 0;
    let size = origin.len();
    let mut offset = 0;
    while offset < size {
        if origin[offset] == 13 && offset + 1 < size && origin[offset + 1] == 10 {
            result.extend_from_slice(b"\r\n");
            column = 0;
            offset += 2;
            continue;
        }
        // Lookahead preserves the final-token limit without buffering a line (#460).
        let trailing = offset + 1 == size
            || (offset + 2 < size && origin[offset + 1] == 13 && origin[offset + 2] == 10);
        qp_folded(&mut result, &mut column, origin[offset], trailing);
        offset += 1;
    }
    result
}

fn qp_hex(octet: u8) -> i64 {
    match octet {
        48..=57 => i64::from(octet - 48),
        65..=70 => i64::from(octet - 65 + 10),
        97..=102 => i64::from(octet - 97 + 10),
        _ => -1,
    }
}

// Validation borrows only the input; emission does not change rejection order (#459).
fn qp_parsed(octets: &[u8], mut decoded: Option<&mut Vec<u8>>) -> i64 {
    let size = octets.len();
    let mut offset = 0usize;
    let mut column = 0usize;
    while offset < size {
        let octet = octets[offset];
        if octet == 13 {
            if offset + 1 >= size || octets[offset + 1] != 10 {
                return offset as i64;
            }
            if let Some(decoded) = decoded.as_mut() {
                decoded.extend([13, 10]);
            }
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
            if let Some(decoded) = decoded.as_mut() {
                decoded.push((high * 16 + low) as u8);
            }
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
        if let Some(decoded) = decoded.as_mut() {
            decoded.push(octet);
        }
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

/// Байты по кодовым точкам (младший байт).
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
        Base32InvalidOffset => int(
            base_parsed(args[0].as_string()?, BASE32.as_bytes(), 5, 8, false)
                .err()
                .unwrap_or(-1),
        ),
        Base32Decode => {
            let octets = base_parsed(args[0].as_string()?, BASE32.as_bytes(), 5, 8, true)
                .map_err(|_| failed(native))?;
            byte_array(octets, arrays)
        }
        Base64Encode => {
            let alphabet = base64_alphabet(native, args[1].as_string()?)?;
            text(encoded(&bytes(&args[0], arrays)?, alphabet, 6, 4))
        }
        Base64InvalidOffset => {
            let alphabet = base64_alphabet(native, args[1].as_string()?)?;
            int(
                base_parsed(args[0].as_string()?, alphabet, 6, 4, false)
                    .err()
                    .unwrap_or(-1),
            )
        }
        Base64Decode => {
            let alphabet = base64_alphabet(native, args[1].as_string()?)?;
            let octets = base_parsed(args[0].as_string()?, alphabet, 6, 4, true)
                .map_err(|_| failed(native))?;
            byte_array(octets, arrays)
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
                args[0].as_string()?,
                &name,
                &alphabet,
                None,
            ))
        }
        Base85Decode => {
            let name = args[1].as_string()?.to_string();
            let alphabet = base85_alphabet(native, &name)?;
            let mut octets = Vec::new();
            if base85_parsed(args[0].as_string()?, &name, &alphabet, Some(&mut octets)) != -1 {
                return Err(failed(native));
            }
            byte_array(octets, arrays)
        }
        QuotedPrintableEncode => {
            let encoded = qp_encode(&bytes(&args[0], arrays)?);
            byte_array(encoded, arrays)
        }
        QuotedPrintableInvalidOffset => int(qp_parsed(&bytes(&args[0], arrays)?, None)),
        QuotedPrintableDecode => {
            let mut decoded = Vec::new();
            if qp_parsed(&bytes(&args[0], arrays)?, Some(&mut decoded)) != -1 {
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

    fn base85_result(
        source: &str,
        name: &str,
        alphabet: &[u8],
        out: Option<&mut Vec<u8>>,
    ) -> i64 {
        let invalid = base85_parsed(source, name, alphabet, out);
        assert_eq!(base85_parsed(source, name, alphabet, None), invalid);
        invalid
    }

    fn qp_result(source: &[u8], out: Option<&mut Vec<u8>>) -> i64 {
        let invalid = qp_parsed(source, out);
        assert_eq!(qp_parsed(source, None), invalid);
        invalid
    }

    #[test]
    fn base85_551_alphabet_symbols_round_trip() {
        for name in ["ascii85", "rfc1924"] {
            let alphabet = base85_alphabet(Native::Base85Decode, name).unwrap();
            for (value, symbol) in alphabet.iter().enumerate() {
                let mut digits = [alphabet[0]; 5];
                digits[4] = *symbol;
                let source = std::str::from_utf8(&digits).unwrap();
                let mut out = Vec::new();
                assert_eq!(base85_result(source, name, &alphabet, Some(&mut out)), -1);
                assert_eq!(out, (value as u32).to_be_bytes());
                let encoded = base85_encoded(&out, name, &alphabet);
                if name == "ascii85" && value == 0 {
                    assert_eq!(encoded, "z");
                } else {
                    assert_eq!(encoded, source);
                }
            }
        }
    }

    #[test]
    fn base85_551_error_order_and_offsets() {
        for name in ["ascii85", "rfc1924"] {
            let alphabet = base85_alphabet(Native::Base85Decode, name).unwrap();
            let zero = char::from(alphabet[0]);
            let high = char::from(alphabet[84]);
            for (source, expected) in [
                (format!("{zero}{zero}{zero}{zero}{zero}😀"), 5),
                (format!("{zero}{high}é"), 2),
                (format!("{high}{high}é"), 2),
                (format!("{zero}{zero}{zero}{zero}{zero}{zero}"), 6),
                (format!("{zero}{zero}{zero}{zero}{zero}{high}{high}"), 6),
            ] {
                let mut out = Vec::new();
                assert_eq!(
                    base85_result(&source, name, &alphabet, Some(&mut out)),
                    expected,
                    "{name}: {source}"
                );
            }
        }
        let alphabet = base85_alphabet(Native::Base85Decode, "ascii85").unwrap();
        for (source, expected) in [("zz!!😀", 4), ("z!!z", 3), ("zz!", 3), ("z!u", 2)] {
            let mut out = Vec::new();
            assert_eq!(
                base85_result(source, "ascii85", &alphabet, Some(&mut out)),
                expected
            );
        }
    }

    #[test]
    fn base85_551_large_round_trip() {
        let input: Vec<u8> = (0..100_000).map(|n| (n * 19) as u8).collect();
        for name in ["ascii85", "rfc1924"] {
            let alphabet = base85_alphabet(Native::Base85Decode, name).unwrap();
            let encoded = base85_encoded(&input, name, &alphabet);
            let mut out = Vec::new();
            assert_eq!(base85_result(&encoded, name, &alphabet, Some(&mut out)), -1);
            assert_eq!(out, input);
        }
    }

    #[test]
    fn validity_459_base85() {
        for name in ["ascii85", "rfc1924"] {
            let alphabet = base85_alphabet(Native::Base85Decode, name).unwrap();
            let zero = char::from(alphabet[0]);
            let high = char::from(alphabet[84]);
            for (source, expected) in [
                (String::new(), -1),
                (format!("{zero}"), 1),
                (format!("{zero}é"), 1),
                (format!("{zero}😀"), 1),
                (format!("{high}{high}{high}{high}{high}é"), 4),
                (format!("{high}{high}"), 1),
                (format!("{zero}{high}"), 1),
            ] {
                let mut out = Vec::new();
                assert_eq!(
                    base85_result(&source, name, &alphabet, Some(&mut out)),
                    expected,
                    "{name}: {source}"
                );
            }
            for size in 0..=12 {
                let input: Vec<u8> = (0..size).map(|n| (n * 19) as u8).collect();
                let spelling = base85_encoded(&input, name, &alphabet);
                let mut out = Vec::new();
                assert_eq!(
                    base85_result(&spelling, name, &alphabet, Some(&mut out)),
                    -1
                );
                assert_eq!(out, input);
            }
            for (source, expected) in if name == "ascii85" {
                [("z", -1), ("y", 0), ("!z", 1), ("z!", 2)]
            } else {
                [("z", 1), ("y", 1), ("0000z", -1), ("0000y", -1)]
            } {
                let mut out = Vec::new();
                assert_eq!(
                    base85_result(source, name, &alphabet, Some(&mut out)),
                    expected
                );
                if name == "ascii85" && source == "z" {
                    assert_eq!(out, [0; 4]);
                }
            }
        }
        assert!(base85_alphabet(Native::Base85Decode, "z85").is_err());
    }

    #[test]
    fn validity_459_quoted_printable() {
        for (source, expected) in [
            ("", -1),
            ("a=Q0", 1),
            ("a=", 1),
            ("a=0", 1),
            ("=ff", -1),
            ("a\r", 1),
            ("a\n", 1),
            ("a ", 1),
            ("a\t", 1),
            ("a \r\n", 1),
            ("a=\r\nb", -1),
            ("a\r\nb", -1),
        ] {
            let mut out = Vec::new();
            assert_eq!(
                qp_result(source.as_bytes(), Some(&mut out)),
                expected,
                "{source:?}"
            );
            if source == "=ff" {
                assert_eq!(out, [255]);
            }
        }
        for (source, expected) in [
            ("a".repeat(76), -1),
            ("a".repeat(77), 76),
            (format!("{}=FF", "a".repeat(74)), 76),
            (format!("{}=Q0", "a".repeat(75)), 75),
            (format!("{}=\r\n", "a".repeat(76)), 76),
        ] {
            assert_eq!(
                qp_result(source.as_bytes(), Some(&mut Vec::new())),
                expected
            );
        }
        let input: Vec<u8> = (0..=255).collect();
        let mut out = Vec::new();
        assert_eq!(qp_result(&qp_encode(&input), Some(&mut out)), -1);
        assert_eq!(out, input);
    }

    fn base_result(source: &str, alphabet: &[u8], width: u32, quantum: usize) -> Result<Vec<u8>, i64> {
        let result = base_parsed(source, alphabet, width, quantum, true);
        assert_eq!(
            base_parsed(source, alphabet, width, quantum, false).map(|_| ()),
            result.as_ref().map(|_| ()).map_err(|offset| *offset)
        );
        result
    }

    #[test]
    fn base_codec_vectors() {
        for (input, b64, b32) in [
            ("", "", ""),
            ("f", "Zg==", "MY======"),
            ("fo", "Zm8=", "MZXQ===="),
            ("foo", "Zm9v", "MZXW6==="),
            ("foob", "Zm9vYg==", "MZXW6YQ="),
            ("fooba", "Zm9vYmE=", "MZXW6YTB"),
            ("foobar", "Zm9vYmFy", "MZXW6YTBOI======"),
        ] {
            for name in ["standard", "url"] {
                let alphabet = base64_alphabet(Native::Base64Decode, name).unwrap();
                assert_eq!(encoded(input.as_bytes(), alphabet, 6, 4), b64);
                assert_eq!(base_result(b64, alphabet, 6, 4).unwrap(), input.as_bytes());
            }
            assert_eq!(encoded(input.as_bytes(), BASE32.as_bytes(), 5, 8), b32);
            assert_eq!(
                base_result(b32, BASE32.as_bytes(), 5, 8).unwrap(),
                input.as_bytes()
            );
        }
        for (name, spelling) in [("standard", "+/8="), ("url", "-_8=")] {
            let alphabet = base64_alphabet(Native::Base64Decode, name).unwrap();
            assert_eq!(encoded(&[251, 255], alphabet, 6, 4), spelling);
            assert_eq!(base_result(spelling, alphabet, 6, 4).unwrap(), [251, 255]);
        }
        assert!(base64_alphabet(Native::Base64Decode, "base64url").is_err());
    }

    #[test]
    fn base_codec_invalid_offsets() {
        for name in ["standard", "url"] {
            let alphabet = base64_alphabet(Native::Base64Decode, name).unwrap();
            for (source, offset) in [
                ("Zm!v", 2),
                ("Zm é", 2),
                ("Zm9vé", 4),
                ("Zm9v😀", 4),
                ("Z=g=", 2),
                ("=AAA", 1),
                ("====", 0),
                ("Z===", 1),
                ("Zg======", 2),
                ("Zh==", 1),
                ("Zm9=", 2),
                ("Zh=", 3),
                ("Zh==!", 4),
                ("Z", 1),
                ("Zg", 2),
                ("Zm8", 3),
                ("Zg==\n", 4),
            ] {
                assert_eq!(
                    base_result(source, alphabet, 6, 4),
                    Err(offset),
                    "{name}: {source}"
                );
            }
            let other = if name == "standard" { "-_8=" } else { "+/8=" };
            assert_eq!(base_result(other, alphabet, 6, 4), Err(0));
        }
        for (source, offset) in [
            ("M!======", 1),
            ("MYé=====", 2),
            ("MY😀=====", 2),
            ("My======", 1),
            ("M=Y=====", 2),
            ("=AAAAAAA", 1),
            ("========", 0),
            ("M=======", 1),
            ("MY==============", 2),
            ("MZ======", 1),
            ("MZXR====", 3),
            ("MZXW7===", 4),
            ("MZXW6YR=", 6),
            ("MZ=====", 7),
            ("MZ======!", 8),
        ] {
            assert_eq!(
                base_result(source, BASE32.as_bytes(), 5, 8),
                Err(offset),
                "{source}"
            );
        }
        for size in 1..8 {
            assert_eq!(
                base_result(&"A".repeat(size), BASE32.as_bytes(), 5, 8),
                Err(size as i64)
            );
        }
    }

    #[test]
    fn base_codec_long_round_trip() {
        let input: Vec<u8> = (0u8..=255).cycle().take(100_000).collect();
        for (alphabet, width, quantum) in [
            (BASE32.as_bytes(), 5, 8),
            (BASE64_STANDARD.as_bytes(), 6, 4),
            (BASE64_URL.as_bytes(), 6, 4),
        ] {
            let spelling = encoded(&input, alphabet, width, quantum);
            assert_eq!(
                base_result(&spelling, alphabet, width, quantum).unwrap(),
                input
            );
            let invalid = format!("{spelling}é");
            assert_eq!(
                base_result(&invalid, alphabet, width, quantum),
                Err(spelling.len() as i64)
            );
        }
    }

    #[test]
    fn qp_empty() {
        assert_eq!(qp_encode(b""), b"");
    }

    #[test]
    fn qp_column_boundaries() {
        for size in [75, 76] {
            let input = vec![b'a'; size];
            assert_eq!(qp_encode(&input), input);
            let input = [input, b"\r\n".to_vec()].concat();
            assert_eq!(qp_encode(&input), input);
        }
        assert_eq!(
            qp_encode(&[b'a'; 77]),
            format!("{}=\r\naa", "a".repeat(75)).as_bytes()
        );
        for (prefix, suffix, expected) in [
            (73, "=", format!("{}=3D", "a".repeat(73))),
            (73, "=b", format!("{}=\r\n=3Db", "a".repeat(73))),
            (74, "=", format!("{}=\r\n=3D", "a".repeat(74))),
            (75, "=", format!("{}=\r\n=3D", "a".repeat(75))),
        ] {
            let input = format!("{}{suffix}", "a".repeat(prefix));
            assert_eq!(qp_encode(input.as_bytes()), expected.as_bytes());
        }
    }

    #[test]
    fn qp_whitespace_and_line_breaks() {
        for (input, expected) in [
            ("=", "=3D"),
            ("a b\tc", "a b=09c"),
            ("a  ", "a =20"),
            ("a\t", "a=09"),
            ("a \t", "a =09"),
            ("a  \r\n", "a =20\r\n"),
            ("a\t\r\n", "a=09\r\n"),
            ("\r\na\r\n\r\nb", "\r\na\r\n\r\nb"),
            ("a\nb\rc", "a=0Ab=0Dc"),
            ("\r\r\n\n", "=0D\r\n=0A"),
        ] {
            assert_eq!(qp_encode(input.as_bytes()), expected.as_bytes());
        }
        let input = format!("{}  b", "a".repeat(74));
        assert_eq!(
            qp_encode(input.as_bytes()),
            format!("{} =\r\n b", "a".repeat(74)).as_bytes()
        );
    }

    #[test]
    fn qp_arbitrary_octets() {
        for octet in 0u8..=255 {
            let expected = match octet {
                33..=60 | 62..=126 => char::from(octet).to_string(),
                _ => format!("={octet:02X}"),
            };
            assert_eq!(qp_encode(&[octet]), expected.as_bytes());
        }
    }

    #[test]
    fn qp_long_physical_line() {
        let input: Vec<u8> = b"a= \t\x80\xff\rX\nZ"
            .iter()
            .copied()
            .cycle()
            .take(10_000)
            .collect();
        let encoded = qp_encode(&input);
        let text = std::str::from_utf8(&encoded).unwrap();
        let lines: Vec<_> = text.split("\r\n").collect();
        assert!(lines.len() > 1);
        for (index, line) in lines.iter().enumerate() {
            assert!(line.len() <= 76);
            if index + 1 < lines.len() {
                assert!(line.ends_with('='));
            }
        }
        let mut decoded = Vec::new();
        assert_eq!(qp_parsed(&encoded, Some(&mut decoded)), -1);
        assert_eq!(decoded, input);
    }

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
            assert_eq!(
                base_result(b64, BASE64_STANDARD.as_bytes(), 6, 4).unwrap(),
                input.as_bytes()
            );
        }
        assert_eq!(
            base_result("Zh==", BASE64_STANDARD.as_bytes(), 6, 4),
            Err(1)
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
            base85_parsed(&encoded, "ascii85", &alphabet, Some(&mut out)),
            -1
        );
        assert_eq!(out, b"Man is");
    }

    #[test]
    fn quoted_printable_escapes_and_round_trips() {
        let encoded = qp_encode(b"a=b \xff\r\nend ");
        assert_eq!(encoded, b"a=3Db =FF\r\nend=20");
        let mut decoded = Vec::new();
        assert_eq!(qp_parsed(&encoded, Some(&mut decoded)), -1);
        assert_eq!(decoded, b"a=b \xff\r\nend ");
    }
}
