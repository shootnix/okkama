//! Portable wire encodings. Pure codec algorithms preserve the established
//! alphabet, padding and scalar-offset contracts; storage uses the new heap.
use crate::host::*;
use okkama_runtime_abi::Native;
const BASE32: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
const BASE64_STANDARD: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const BASE64_URL: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const RFC1924: &str =
    "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz!#$%&()*+-;<=>?@^_`{|}~";

fn failed(native: Native) -> Error {
    Error(format!("Native invocation \"{}\" failed", native.symbol()))
}
pub(crate) fn handles(native: Native) -> bool {
    use Native::*;
    matches!(
        native,
        Base32Encode
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
    )
}
impl Heap {
    pub(crate) fn native_encoding(
        &mut self,
        native: Native,
        args: &[Slot],
    ) -> Result<Outcome, Error> {
        use Native::*;
        let value = match native {
            Base32Encode | Base64Encode => {
                let (alphabet, width, quantum) = if native == Base32Encode {
                    (BASE32.as_bytes(), 5, 8)
                } else {
                    (base64_alphabet(native, self.text(args[1])?)?, 6, 4)
                };
                let text = encoded(&self.native_bytes(args[0])?, alphabet, width, quantum);
                self.allocate_text(text)?
            }
            Base32InvalidOffset | Base32Decode | Base64InvalidOffset | Base64Decode => {
                let (alphabet, width, quantum) =
                    if matches!(native, Base32InvalidOffset | Base32Decode) {
                        (BASE32.as_bytes(), 5, 8)
                    } else {
                        (base64_alphabet(native, self.text(args[1])?)?, 6, 4)
                    };
                let source = points(self.text(args[0])?);
                let (invalid, end) = shape(&source, alphabet, width, quantum);
                if matches!(native, Base32InvalidOffset | Base64InvalidOffset) {
                    Slot::small(invalid)
                } else {
                    if invalid != -1 {
                        return Err(failed(native));
                    }
                    self.native_byte_array(decoded(&source, end, alphabet, width))?
                }
            }
            Base85Encode => {
                let name = self.text(args[1])?;
                let alphabet = base85_alphabet(native, name)?;
                let text = base85_encoded(&self.native_bytes(args[0])?, name, &alphabet);
                self.allocate_text(text)?
            }
            Base85InvalidOffset | Base85Decode => {
                let name = self.text(args[1])?;
                let alphabet = base85_alphabet(native, name)?;
                let mut bytes = Vec::new();
                let invalid =
                    base85_parsed(&points(self.text(args[0])?), name, &alphabet, &mut bytes);
                if native == Base85InvalidOffset {
                    Slot::small(invalid)
                } else {
                    if invalid != -1 {
                        return Err(failed(native));
                    }
                    self.native_byte_array(bytes)?
                }
            }
            QuotedPrintableEncode => {
                let bytes = qp_encode(&self.native_bytes(args[0])?);
                self.native_byte_array(bytes)?
            }
            QuotedPrintableInvalidOffset | QuotedPrintableDecode => {
                let mut bytes = Vec::new();
                let invalid = qp_parsed(&self.native_bytes(args[0])?, &mut bytes);
                if native == QuotedPrintableInvalidOffset {
                    Slot::small(invalid)
                } else {
                    if invalid != -1 {
                        return Err(failed(native));
                    }
                    self.native_byte_array(bytes)?
                }
            }
            _ => return Err(Error("invalid encoding provider".into())),
        };
        Ok(Outcome::Value(value))
    }
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
    if !size.is_multiple_of(quantum) {
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(qp_parsed(&encoded, &mut decoded), -1);
        assert_eq!(decoded, input);
    }
}
