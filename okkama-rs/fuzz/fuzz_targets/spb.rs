//! Decoding any bytes as SPB ends in a verified program or an error, and the
//! core's load-time checks (intrinsic contracts) never panic on it.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if okkama_sparrow::decode(data).is_ok() {
        let _ = sparrow_core::check(data);
    }
});
