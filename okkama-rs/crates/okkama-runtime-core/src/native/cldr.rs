//! `runtime/cldr`: таблицы CLDR как данные рантайма (#223). Генераторы
//! `tools/generate-cldr*.py` пишут их в `cldr-<семейство>.dat` (gzip текста,
//! из `data/`); файл распаковывается и индексируется при первом обращении к
//! любой его таблице. Строка файла — таблица, ключ и поля через табуляцию;
//! имя таблицы начинается с семейства и точки (`numbers.row`). Sparrow берёт
//! этот native через мост (`native_origin`).
use super::Error;
use super::support::{failure, string, string_array};
use crate::data::gunzip;
use crate::{Value, abi::Native};
use std::collections::HashMap;
use std::sync::OnceLock;

/// Таблица → ключ → поля строки (текст после ключа).
type Index = HashMap<&'static str, HashMap<&'static str, &'static str>>;

pub fn handles(native: Native) -> bool {
    matches!(native, Native::CldrCldrFields)
}

pub fn execute(native: Native, args: &[Value]) -> Result<Value, Error> {
    let (table, key) = (string(&args[0])?, string(&args[1])?);
    let Some(rows) = table
        .split_once('.')
        .and_then(|(family, _)| index(family))
        .and_then(|index| index.get(table))
    else {
        return Err(failure(
            "core.cldr",
            "invalid_input",
            native,
            format!("Unknown CLDR table {table:?}"),
        ));
    };
    Ok(match rows.get(key) {
        Some(fields) => string_array(fields.split('\t')),
        None => string_array(std::iter::empty::<&str>()),
    })
}

/// Индекс семейства; `None` — такого семейства нет.
fn index(family: &str) -> Option<&'static Index> {
    macro_rules! families {
        ($($name:literal => $file:literal),* $(,)?) => {
            match family {
                $($name => {
                    static TEXT: OnceLock<String> = OnceLock::new();
                    static INDEX: OnceLock<Index> = OnceLock::new();
                    Some(INDEX.get_or_init(|| {
                        build(TEXT.get_or_init(|| {
                            String::from_utf8(gunzip(include_bytes!(concat!(
                                "../../../okkama-runtime-core/data/",
                                $file
                            ))))
                            .expect("UTF-8 CLDR table")
                        }))
                    }))
                })*
                _ => None,
            }
        };
    }
    families!(
        "numbers" => "cldr-numbers.dat",
        "dates" => "cldr-dates.dat",
        "zones" => "cldr-zones.dat",
        "currencies" => "cldr-currencies.dat",
        "locales" => "cldr-locales.dat",
    )
}

fn build(text: &'static str) -> Index {
    let mut index = Index::new();
    for line in text.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(table), Some(key), Some(fields)) = (parts.next(), parts.next(), parts.next())
        else {
            panic!("malformed CLDR record {line:?}");
        };
        index.entry(table).or_default().insert(key, fields);
    }
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(table: &str, key: &str) -> Option<Vec<&'static str>> {
        let (family, _) = table.split_once('.')?;
        let rows = index(family)?.get(table)?;
        rows.get(key).map(|fields| fields.split('\t').collect())
    }

    #[test]
    fn every_family_answers_its_tables() {
        assert_eq!(fields("numbers.digits", "latn"), Some(vec!["0123456789"]));
        assert_eq!(fields("currencies.digits", "JPY"), Some(vec!["0"]));
        assert_eq!(fields("locales.parent", "en-GB"), Some(vec!["en-001"]));
        assert_eq!(fields("locales.likely", "und"), Some(vec!["en-Latn-US"]));
        // CLDR keys zones by their old names.
        assert_eq!(fields("zones.canonical", "Asia/Kolkata"), Some(vec!["Asia/Calcutta"]));
        assert!(fields("dates.profile", "ru").is_some());
        // A row keeps its empty fields.
        assert_eq!(fields("numbers.row", "en").map(|row| row.len()), Some(7));
        assert_eq!(fields("numbers.digits", "no-such-system"), None);
        assert!(index("nowhere").is_none());
    }
}
