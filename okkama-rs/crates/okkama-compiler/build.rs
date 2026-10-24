//! Рантайм JVM (`okkama.rt`) для компилятора:
//! - `RUNTIME_FILES`: каждый class-файл `runtime/jvm/classes/okkama/rt`,
//!   встроенный через `include_bytes!`;
//! - `NATIVE_METHODS`: public static методы-providers natives std
//!   (`runtime_json__json_parse_state`), их классы и дескрипторы — из самих
//!   class-файлов, поэтому новый provider не требует правки кода компилятора.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets the manifest dir"));
    let directory = manifest.join("runtime/jvm/classes/okkama/rt");
    println!("cargo:rerun-if-changed={}", directory.display());
    let mut names: Vec<String> = fs::read_dir(&directory)
        .expect("runtime classes are committed")
        .map(|entry| {
            entry
                .expect("readable runtime directory")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort();
    let mut files = format!(
        "pub(super) const RUNTIME_FILES: [(&str, &[u8]); {}] = [\n",
        names.len()
    );
    let mut natives = Vec::new();
    for name in &names {
        let path = directory.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        files.push_str(&format!(
            "    (\"okkama/rt/{name}\", include_bytes!({:?})),\n",
            path.display().to_string()
        ));
        let Some(class) = name.strip_suffix(".class") else {
            continue;
        };
        if class.contains('$') {
            continue;
        }
        let bytes = fs::read(&path).expect("readable runtime class");
        for (method, descriptor) in public_static_methods(&bytes) {
            if method.starts_with("runtime_") && method.contains("__") {
                natives.push((method, format!("okkama/rt/{class}"), descriptor));
            }
        }
    }
    files.push_str("];\n");
    natives.sort();
    natives.dedup_by(|a, b| a.0 == b.0);
    let mut table = format!(
        "pub(super) static NATIVE_METHODS: [(&str, &str, &str); {}] = [\n",
        natives.len()
    );
    for (method, class, descriptor) in &natives {
        table.push_str(&format!("    ({method:?}, {class:?}, {descriptor:?}),\n"));
    }
    table.push_str("];\n");
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    fs::write(out.join("runtime_files.rs"), files).expect("OUT_DIR is writable");
    fs::write(out.join("native_methods.rs"), table).expect("OUT_DIR is writable");
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u1(&mut self) -> u8 {
        let value = self.bytes[self.at];
        self.at += 1;
        value
    }

    fn u2(&mut self) -> u16 {
        u16::from(self.u1()) << 8 | u16::from(self.u1())
    }

    fn u4(&mut self) -> u32 {
        u32::from(self.u2()) << 16 | u32::from(self.u2())
    }

    fn skip(&mut self, count: usize) {
        self.at += count;
    }
}

/// Имена и дескрипторы `public static` методов class-файла.
fn public_static_methods(bytes: &[u8]) -> Vec<(String, String)> {
    let mut reader = Reader { bytes, at: 8 };
    let count = reader.u2();
    let mut utf8 = vec![String::new(); usize::from(count)];
    let mut index = 1;
    while index < count {
        let tag = reader.u1();
        match tag {
            1 => {
                let length = usize::from(reader.u2());
                utf8[usize::from(index)] =
                    String::from_utf8_lossy(&reader.bytes[reader.at..reader.at + length])
                        .into_owned();
                reader.skip(length);
            }
            3 | 4 | 9 | 10 | 11 | 12 | 17 | 18 => reader.skip(4),
            5 | 6 => {
                reader.skip(8);
                index += 1;
            }
            7 | 8 | 16 | 19 | 20 => reader.skip(2),
            15 => reader.skip(3),
            other => panic!("unknown constant pool tag {other}"),
        }
        index += 1;
    }
    reader.skip(6);
    let interfaces = usize::from(reader.u2());
    reader.skip(interfaces * 2);
    let skip_members = |reader: &mut Reader, keep: bool| -> Vec<(String, String)> {
        let mut found = Vec::new();
        for _ in 0..reader.u2() {
            let access = reader.u2();
            let name = usize::from(reader.u2());
            let descriptor = usize::from(reader.u2());
            for _ in 0..reader.u2() {
                reader.skip(2);
                let length = reader.u4() as usize;
                reader.skip(length);
            }
            if keep && access & 0x0009 == 0x0009 {
                found.push((utf8[name].clone(), utf8[descriptor].clone()));
            }
        }
        found
    };
    skip_members(&mut reader, false);
    skip_members(&mut reader, true)
}
