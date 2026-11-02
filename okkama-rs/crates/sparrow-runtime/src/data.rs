//! Таблицы рантайма, общие с JVM (`runtime/jvm/src/okkama/rt/*.dat`): gzip
//! текстовых файлов, распаковываются один раз при первом обращении.

/// Содержимое gzip-файла (RFC 1952: заголовок, deflate, CRC и длина).
pub fn gunzip(bytes: &[u8]) -> Vec<u8> {
    assert!(bytes.len() > 18 && bytes[..3] == [0x1f, 0x8b, 8], "gzip data");
    let flags = bytes[3];
    let mut at = 10;
    if flags & 4 != 0 {
        at += 2 + usize::from(u16::from_le_bytes([bytes[at], bytes[at + 1]]));
    }
    for flag in [8, 16] {
        if flags & flag != 0 {
            while bytes[at] != 0 {
                at += 1;
            }
            at += 1;
        }
    }
    if flags & 2 != 0 {
        at += 2;
    }
    miniz_oxide::inflate::decompress_to_vec(&bytes[at..bytes.len() - 8]).expect("valid deflate")
}

macro_rules! table {
    ($name:ident, $file:literal) => {
        pub fn $name() -> &'static str {
            static TEXT: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            TEXT.get_or_init(|| {
                String::from_utf8(gunzip(include_bytes!(concat!(
                    "../../okkama-compiler/runtime/jvm/src/okkama/rt/",
                    $file
                ))))
                .expect("UTF-8 table")
            })
        }
    };
}

table!(unicode, "unicode.dat");
table!(idna, "idna.dat");
table!(public_suffixes, "psl.dat");

/// Именованные ссылки HTML (WHATWG): текстовый файл `имя<TAB>hex…`.
pub fn html_entities() -> &'static str {
    std::str::from_utf8(include_bytes!(
        "../../okkama-compiler/runtime/jvm/src/okkama/rt/html_entities.dat"
    ))
    .expect("UTF-8 table")
}

/// Архив tzdb (zip) — как ресурс JVM.
pub fn time_zone_archive() -> &'static [u8] {
    include_bytes!("../../okkama-compiler/runtime/jvm/src/okkama/rt/tzdb.dat")
}

/// Общая категория Unicode (`Lu`, `Nd`, …) из `unicode.dat`.
pub fn category(point: u32) -> &'static str {
    static RANGES: std::sync::OnceLock<Vec<(u32, u32, &'static str)>> = std::sync::OnceLock::new();
    let ranges = RANGES.get_or_init(|| ranges(unicode(), "G"));
    find(ranges, point).unwrap_or("Cn")
}

/// Диапазоны `<tag> <value> <from> <to>` таблицы (`G`, `C`, `S`, …).
pub fn ranges(text: &'static str, tag: &str) -> Vec<(u32, u32, &'static str)> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split(' ');
            (parts.next()? == tag).then_some(())?;
            let value = parts.next()?;
            let from = u32::from_str_radix(parts.next()?, 16).ok()?;
            let to = u32::from_str_radix(parts.next()?, 16).ok()?;
            Some((from, to, value))
        })
        .collect()
}

/// Значение диапазона, содержащего `point`.
pub fn find(ranges: &[(u32, u32, &'static str)], point: u32) -> Option<&'static str> {
    ranges
        .binary_search_by(|(from, to, _)| {
            if point < *from {
                std::cmp::Ordering::Greater
            } else if point > *to {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Equal
            }
        })
        .ok()
        .map(|index| ranges[index].2)
}

/// Canonical Combining Class (0 — не указан в таблице).
pub fn combining_class(point: u32) -> u32 {
    static RANGES: std::sync::OnceLock<Vec<(u32, u32, &'static str)>> = std::sync::OnceLock::new();
    let ranges = RANGES.get_or_init(|| ranges(unicode(), "C"));
    find(ranges, point).map_or(0, |value| value.parse().unwrap_or(0))
}

#[cfg(test)]
mod tests {
    #[test]
    fn categories_come_from_the_shared_table() {
        assert_eq!(super::category('a' as u32), "Ll");
        assert_eq!(super::category('A' as u32), "Lu");
        assert_eq!(super::category('7' as u32), "Nd");
        assert_eq!(super::category(0x0663), "Nd");
        assert_eq!(super::category(' ' as u32), "Zs");
        assert_eq!(super::category(0x2167), "Nl");
    }
}
