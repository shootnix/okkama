//! Исполняемый JAR из файлов артефакта: один ZIP с манифестом `Main-Class`,
//! классами программы и рантаймом `okkama.rt`. Записи сжаты DEFLATE
//! (LZ77 + фиксированные коды Хаффмана, RFC 1951), если это короче, иначе
//! `STORED`. Время записей фиксировано — архив детерминирован.

use super::Artifact;

/// 1980-01-01 00:00 в формате MS-DOS.
const DOS_TIME: u16 = 0;
const DOS_DATE: u16 = 0x0021;
/// Бит 11 флагов: имена в UTF-8 (классы программы бывают не ASCII).
const UTF8_NAMES: u16 = 0x0800;

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn u16le(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn u32le(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

/// Ошибка формата: архив вне пределов ZIP без ZIP64.
#[derive(Debug)]
pub struct JarError(pub String);

impl std::fmt::Display for JarError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for JarError {}

impl Artifact {
    /// JAR, запускаемый `java -jar`: главный класс — `main_class`; манифест
    /// разрешает FFM-natives рантайма (`Enable-Native-Access`, JDK 22+).
    pub fn jar(&self, main_class: &str) -> Result<Vec<u8>, JarError> {
        let manifest = format!(
            "Manifest-Version: 1.0\r\nMain-Class: {main_class}\r\nEnable-Native-Access: ALL-UNNAMED\r\nCreated-By: okkama\r\n\r\n"
        );
        let mut entries: Vec<(&str, &[u8])> = vec![("META-INF/MANIFEST.MF", manifest.as_bytes())];
        entries.extend(
            self.files
                .iter()
                .map(|file| (file.name.as_str(), file.bytes.as_slice())),
        );
        let too_large = || JarError("JAR exceeds the 4 GiB / 65535-entry ZIP limits".to_string());
        if entries.len() > usize::from(u16::MAX) {
            return Err(too_large());
        }
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, bytes) in &entries {
            let offset = u32::try_from(out.len()).map_err(|_| too_large())?;
            let size = u32::try_from(bytes.len()).map_err(|_| too_large())?;
            let name_len = u16::try_from(name.len()).map_err(|_| too_large())?;
            let crc = crc32(bytes);
            let deflated = deflate(bytes);
            let (method, data): (u16, &[u8]) = if deflated.len() < bytes.len() {
                (8, &deflated)
            } else {
                (0, bytes)
            };
            let stored = u32::try_from(data.len()).map_err(|_| too_large())?;
            // Локальный заголовок.
            u32le(&mut out, 0x0403_4b50);
            u16le(&mut out, 20);
            u16le(&mut out, UTF8_NAMES);
            u16le(&mut out, method);
            u16le(&mut out, DOS_TIME);
            u16le(&mut out, DOS_DATE);
            u32le(&mut out, crc);
            u32le(&mut out, stored);
            u32le(&mut out, size);
            u16le(&mut out, name_len);
            u16le(&mut out, 0);
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(data);
            // Запись центрального каталога.
            u32le(&mut central, 0x0201_4b50);
            u16le(&mut central, 20);
            u16le(&mut central, 20);
            u16le(&mut central, UTF8_NAMES);
            u16le(&mut central, method);
            u16le(&mut central, DOS_TIME);
            u16le(&mut central, DOS_DATE);
            u32le(&mut central, crc);
            u32le(&mut central, stored);
            u32le(&mut central, size);
            u16le(&mut central, name_len);
            u16le(&mut central, 0);
            u16le(&mut central, 0);
            u16le(&mut central, 0);
            u16le(&mut central, 0);
            u32le(&mut central, 0);
            u32le(&mut central, offset);
            central.extend_from_slice(name.as_bytes());
        }
        let central_offset = u32::try_from(out.len()).map_err(|_| too_large())?;
        let central_size = u32::try_from(central.len()).map_err(|_| too_large())?;
        out.extend_from_slice(&central);
        let count = entries.len() as u16;
        u32le(&mut out, 0x0605_4b50);
        u16le(&mut out, 0);
        u16le(&mut out, 0);
        u16le(&mut out, count);
        u16le(&mut out, count);
        u32le(&mut out, central_size);
        u32le(&mut out, central_offset);
        u16le(&mut out, 0);
        Ok(out)
    }
}

// ---- DEFLATE (RFC 1951): один финальный блок с фиксированными кодами ----

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
const WINDOW: usize = 32 * 1024;
const MAX_MATCH: usize = 258;
const MIN_MATCH: usize = 3;
const HASH_BITS: u32 = 15;
const CHAIN: usize = 128;

/// Поток битов DEFLATE: младшие биты первыми.
struct Bits {
    out: Vec<u8>,
    word: u64,
    count: u32,
}

impl Bits {
    fn put(&mut self, value: u32, width: u32) {
        self.word |= u64::from(value) << self.count;
        self.count += width;
        while self.count >= 8 {
            self.out.push(self.word as u8);
            self.word >>= 8;
            self.count -= 8;
        }
    }

    /// Код Хаффмана: старший бит первым.
    fn code(&mut self, code: u32, width: u32) {
        let reversed = code.reverse_bits() >> (32 - width);
        self.put(reversed, width);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.count > 0 {
            self.out.push(self.word as u8);
        }
        self.out
    }
}

fn literal(bits: &mut Bits, symbol: u32) {
    match symbol {
        0..=143 => bits.code(0x30 + symbol, 8),
        144..=255 => bits.code(0x190 + symbol - 144, 9),
        256..=279 => bits.code(symbol - 256, 7),
        _ => bits.code(0xc0 + symbol - 280, 8),
    }
}

fn matched(bits: &mut Bits, length: usize, distance: usize) {
    let index = LENGTH_BASE
        .iter()
        .rposition(|base| usize::from(*base) <= length)
        .expect("length >= 3");
    literal(bits, 257 + index as u32);
    bits.put(
        (length - usize::from(LENGTH_BASE[index])) as u32,
        u32::from(LENGTH_EXTRA[index]),
    );
    let index = DISTANCE_BASE
        .iter()
        .rposition(|base| usize::from(*base) <= distance)
        .expect("distance >= 1");
    bits.code(index as u32, 5);
    bits.put(
        (distance - usize::from(DISTANCE_BASE[index])) as u32,
        u32::from(DISTANCE_EXTRA[index]),
    );
}

fn hash(data: &[u8], at: usize) -> usize {
    let value = u32::from(data[at]) << 16 | u32::from(data[at + 1]) << 8 | u32::from(data[at + 2]);
    (value.wrapping_mul(0x9e37_79b1) >> (32 - HASH_BITS)) as usize
}

/// Сырой поток DEFLATE: жадный LZ77 по цепочкам хешей, фиксированные коды.
pub(crate) fn deflate(data: &[u8]) -> Vec<u8> {
    let mut bits = Bits {
        out: Vec::with_capacity(data.len() / 2 + 16),
        word: 0,
        count: 0,
    };
    bits.put(1, 1); // BFINAL
    bits.put(1, 2); // BTYPE = 01: фиксированные коды
    let mut head = vec![usize::MAX; 1 << HASH_BITS];
    let mut prev = vec![usize::MAX; WINDOW];
    let insert = |head: &mut Vec<usize>, prev: &mut Vec<usize>, at: usize| {
        if at + MIN_MATCH <= data.len() {
            let slot = hash(data, at);
            prev[at % WINDOW] = head[slot];
            head[slot] = at;
        }
    };
    let mut at = 0;
    while at < data.len() {
        let mut best = 0;
        let mut distance = 0;
        if at + MIN_MATCH <= data.len() {
            let mut candidate = head[hash(data, at)];
            let limit = (data.len() - at).min(MAX_MATCH);
            let mut steps = 0;
            while candidate != usize::MAX && at - candidate <= WINDOW && steps < CHAIN {
                if data[candidate + best] == data[at + best] || best == 0 {
                    let mut length = 0;
                    while length < limit && data[candidate + length] == data[at + length] {
                        length += 1;
                    }
                    if length > best {
                        best = length;
                        distance = at - candidate;
                        if length == limit {
                            break;
                        }
                    }
                }
                let next = prev[candidate % WINDOW];
                if next == usize::MAX || next >= candidate {
                    break;
                }
                candidate = next;
                steps += 1;
            }
        }
        if best >= MIN_MATCH {
            matched(&mut bits, best, distance);
            for position in at..at + best {
                insert(&mut head, &mut prev, position);
            }
            at += best;
        } else {
            literal(&mut bits, u32::from(data[at]));
            insert(&mut head, &mut prev, at);
            at += 1;
        }
    }
    literal(&mut bits, 256);
    bits.finish()
}

#[cfg(test)]
mod tests {
    use super::crc32;

    #[test]
    fn crc32_matches_the_zip_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }
}
