//! Native providers of std modules through Sparrow:
//! encodings, bigint, decimal, host crypto, buffer.
#[path = "sparrow_support/mod.rs"]
mod support;
use okkama_compiler::{SourceFile, ir, lowering, syntax};

fn lower(module: &str, natives: &str, body: &str) -> ir::Module {
    let source = SourceFile::new(
        "providers.ok",
        format!("module {module} v1.0.0; {natives} main() is int {{ {body} }}"),
    )
    .unwrap();
    lowering::lower_program(&syntax::parse(&source, &syntax::ParseOptions::default()).unwrap())
        .unwrap()
}

/// Проверка результата Sparrow.
fn both(module: &str, natives: &str, body: &str, expected: &str) {
    let lowered = lower(module, natives, body);

    support::assert_value(&lowered, expected);
}

const HEX: &str = "native hex_encode(origin is @byte) is string;
    native hex_valid_prefix(origin is string) is int;
    native hex_decode(origin is string) is @byte;";

#[test]
fn hex_round_trips_and_rejects_odd_input_like_the_jvm() {
    both(
        "runtime/hex",
        HEX,
        r#"if hex_encode([0.byte(), 171.byte(), 255.byte()]) == "00abff" && hex_decode("00ABff").size() == 3 && hex_valid_prefix("12xz") == 2 { 42; } else { 0; }"#,
        "42",
    );
    let module = lower("runtime/hex", HEX, r#"hex_decode("abc").size();"#);
    assert!(!matches!(support::sparrow(&module), support::Ending::Value(_)));

}

#[test]
fn base64_alphabets_and_padding_follow_rfc_4648() {
    both(
        "runtime/base64",
        "native base64_encode(origin is @byte, alphabet is string) is string;
         native base64_invalid_offset(origin is string, alphabet is string) is int;
         native base64_decode(origin is string, alphabet is string) is @byte;",
        r#"let data = [251.byte(), 255.byte()];
           if base64_encode(data, "standard") == "+/8=" && base64_encode(data, "url") == "-_8="
              && base64_decode("+/8=", "standard").size() == 2 && base64_invalid_offset("Zh==", "standard") == 1 { 42; } else { 0; }"#,
        "42",
    );
}

#[test]
fn bigint_and_decimal_are_exact() {
    both(
        "runtime/bigint",
        "native bigint_product(left is string, right is string) is string;
         native bigint_quotient(left is string, right is string) is string;
         native bigint_remainder(left is string, right is string) is string;
         native bigint_power(base is string, exponent is int, maximum_bits is int) is string;
         native bigint_bits(origin is string) is int;
         native bigint_valid(origin is string) is bool;",
        r#"if bigint_product("99999999999999999999", "99999999999999999999") == "9999999999999999999800000000000000000001"
              && bigint_quotient("-7", "2") == "-3" && bigint_remainder("-7", "2") == "-1"
              && bigint_power("2", 100, 1000) == "1267650600228229401496703205376"
              && bigint_bits("-256") == 9 && bigint_valid("-0012") && !bigint_valid("1e3") { 42; } else { 0; }"#,
        "42",
    );
    both(
        "runtime/decimal",
        "native decimal_canonical(origin is string) is string;
         native decimal_order(left is string, right is string) is int;",
        r#"if decimal_canonical("001.2000") == "1.2" && decimal_canonical("1000") == "1e3"
              && decimal_order("1.10", "1.1") == 0 && decimal_order("-2", "1e-9") == -1 { 42; } else { 0; }"#,
        "42",
    );
    let module = lower(
        "runtime/bigint",
        "native bigint_quotient(left is string, right is string) is string;",
        r#"bigint_quotient("1", "0").length();"#,
    );
    let report = support::report(&module);
    assert!(report.contains("core.bigint") && report.contains("invalid_input"), "{report}");
}

#[test]
fn host_crypto_digests_preserve_branches() {
    let natives = "native object DigestState;
        native digest_create(algorithm is string, key is @byte, length is int) is DigestState;
        native digest_update(origin is DigestState, bytes is @byte) is DigestState;
        native digest_bytes(origin is DigestState, length is int) is @byte;";
    // SHA-256("abc"): ba7816bf…; ветвь с другим продолжением не меняет исходную.
    let body = r#"let empty is @byte = [];
        let base = digest_create("sha256", empty, 32);
        let abc = digest_update(base, [97.byte(), 98.byte(), 99.byte()]);
        let other = digest_update(base, [120.byte()]);
        let first = digest_bytes(abc, 32);
        let again = digest_bytes(digest_update(base, [97.byte(), 98.byte(), 99.byte()]), 32);
        if first[0] == 186.byte() && first[31] == 173.byte() && again == first && digest_bytes(other, 32) != first { 42; } else { 0; }"#;
    both("runtime/host_crypto", natives, body, "42");
}

#[test]
fn buffers_are_mutable_handles_with_typed_close_failures() {
    let natives = "native object BufferState;
        native buffer_state_create(initial is @byte) is BufferState;
        native buffer_state_read(state is BufferState, limit is int) is @byte;
        native buffer_state_append(state is BufferState, value is @byte) is int;
        native buffer_state_size(state is BufferState) is int;
        native buffer_state_close(state is BufferState) is bool;";
    let body = "let buffer = buffer_state_create([1.byte(), 2.byte()]);
        let added = buffer_state_append(buffer, [3.byte()]);
        let head = buffer_state_read(buffer, 2);
        let closed = buffer_state_close(buffer);
        let twice = buffer_state_close(buffer);
        if added == 1 && head.size() == 2 && closed && !twice { buffer_state_size(buffer); } else { 0; }";
    let module = lower("runtime/buffer", natives, body);
    let report = support::report(&module);
    assert!(report.contains("already_closed"), "{report}");
    assert!(report.contains("Byte buffer is already closed"), "{report}");
}
