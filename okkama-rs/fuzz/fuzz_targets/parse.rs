//! Parsing any text ends in a program or diagnostics; the program is dropped
//! (deep trees must not overflow the stack), and formatted source parses again.
#![no_main]
use libfuzzer_sys::fuzz_target;
use okkama_compiler::SourceFile;
use okkama_compiler::syntax::{self, ParseOptions};

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else { return };
    let Ok(source) = SourceFile::new("fuzz.ok", text) else { return };
    let Ok(program) = syntax::parse(&source, &ParseOptions::default()) else { return };
    drop(program);
    if let Ok(formatted) = syntax::format::format(&source) {
        let again = SourceFile::new("fuzz.ok", formatted).expect("formatted source fits");
        assert!(syntax::parse(&again, &ParseOptions::default()).is_ok(), "formatted source does not parse");
    }
});
