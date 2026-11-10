//! Native ABI calls use the same provider through IR and serialized SPB.
mod support;
use okkama_compiler::{SourceFile, codegen, ir, lowering, syntax};
use okkama_runtime::{RuntimeError, execute};

fn lower(body: &str) -> ir::Module {
    let source = SourceFile::new(
        "native.ok",
        format!(
            r#"module runtime/utf v1.0.0;
        native utf8_encode(origin is string) is @byte;
        native utf8_decode(origin is @byte) is string;
        native utf8_valid_prefix(origin is @byte) is int;
        native utf16_encode(origin is string, order is string) is @byte;
        native utf16_decode(origin is @byte, order is string) is string;
        native utf16_valid_prefix(origin is @byte, order is string) is int;
        native utf32_encode(origin is string, order is string) is @byte;
        native utf32_decode(origin is @byte, order is string) is string;
        native utf32_valid_prefix(origin is @byte, order is string) is int;
        main() is int {{ {body} }}"#
        ),
    )
    .unwrap();
    lowering::lower_program(&syntax::parse(&source, &syntax::ParseOptions::default()).unwrap())
        .unwrap()
}

fn both(body: &str, expected: &str) {
    let module = lower(body);
    assert_eq!(execute(&module).unwrap().to_string(), expected);
    support::assert_value(&module, expected);
}

#[test]
fn native_identity_and_signature_are_checked_before_execution() {
    let mut module = lower(r#"utf8_encode("x").size();"#);
    let function = module
        .functions
        .iter_mut()
        .find(|f| f.extern_symbol() == Some("runtime/utf@1::utf8_encode/1"))
        .unwrap();
    let attr = function
        .attrs
        .iter_mut()
        .find(|a| matches!(a, ir::Attr::Extern(_)))
        .unwrap();
    *attr = ir::Attr::Extern("runtime/utf@2::utf8_encode/1".into());
    assert!(matches!(
        execute(&module),
        Err(RuntimeError::Unsupported { .. })
    ));
    // Как IR: неизвестный provider — ошибка запуска, а не генерации.
    assert!(!matches!(support::sparrow(&module), support::Ending::Value(_)));

    let source = SourceFile::new("bad-native.ok", "module runtime/utf v1.0.0; native utf8_encode(text is int) is int; main() is int { utf8_encode(1); }").unwrap();
    let module =
        lowering::lower_program(&syntax::parse(&source, &syntax::ParseOptions::default()).unwrap())
            .unwrap();
    assert!(
        matches!(execute(&module), Err(RuntimeError::InvalidState { message, .. }) if message.contains("signature"))
    );
    // SPB сверяет native с таблицей ABI при проверке программы (#16).
    assert!(
        matches!(codegen::sparrow::generate(&module), Err(error) if error.message.contains("does not match the signature"))
    );
    let module = lower(r#"utf16_encode("x", "middle").size();"#);
    assert!(
        matches!(execute(&module), Err(RuntimeError::Trap { message, .. }) if message == "Native invocation \"runtime/utf@1::utf16_encode/2\" failed")
    );
}

#[test]
fn utf_native_calls_preserve_scalars_and_byte_order() {
    both(r#"utf8_encode("Aé🙂").size();"#, "7");
    for encoding in ["utf8", "utf16", "utf32"] {
        for order in ["big", "little"] {
            let order = if encoding == "utf8" {
                String::new()
            } else {
                format!(", \"{order}\"")
            };
            both(
                &format!(
                    "let text = \"Aé🙂\"; let bytes = {encoding}_encode(text{order}); if {encoding}_decode(bytes{order}) == text {{ {encoding}_valid_prefix(bytes{order}) - bytes.size() + 42; }} else {{ 0; }}"
                ),
                "42",
            );
        }
    }
    both(
        r#"let bs = utf16_encode("A", "big"); bs[0].int() + bs[1].int();"#,
        "65",
    );
    both(
        r#"let bs = utf32_encode("A", "little"); bs[0].int();"#,
        "65",
    );
    both(r#"utf8_valid_prefix(@byte(65, 192, 128));"#, "1");
    both(r#"utf16_valid_prefix(@byte(0, 65, 216, 0), "big");"#, "2");
    both(
        r#"utf32_valid_prefix(@byte(0, 0, 0, 65, 0, 17, 0, 0), "big");"#,
        "4",
    );
}

#[test]
fn malformed_native_data_keeps_structured_failure_fields() {
    for (body, operation, encoding) in [
        (
            r#"utf8_decode(@byte(192, 128)).length();"#,
            "utf8_decode/1",
            "UTF-8",
        ),
        (
            r#"utf16_decode(@byte(216, 0), "big").length();"#,
            "utf16_decode/2",
            "UTF-16BE",
        ),
        (
            r#"utf32_decode(@byte(0, 0, 17, 0), "little").length();"#,
            "utf32_decode/2",
            "UTF-32LE",
        ),
    ] {
        let module = lower(body);
        let Err(RuntimeError::NativeFailure { failure, .. }) = execute(&module) else {
            panic!("expected native failure");
        };
        assert_eq!(failure.domain, "core.utf");
        assert_eq!(failure.code, "invalid_data");
        assert_eq!(failure.operation, format!("runtime/utf@1::{operation}"));
        assert_eq!(failure.message, format!("Bytes are not valid {encoding}"));
        let report = support::report(&module);
        assert!(report.contains(&failure.message) && report.contains(&failure.code), "{report}");
    }
    both(
        r#"if false { utf8_decode(@byte(255)).length(); } else { 42; }"#,
        "42",
    );
}

#[test]
fn ir_native_failure_is_caught_as_concrete_type_or_error_role() {
    for body in [
        r#"try { utf8_decode(@byte(255)); 0; } catch e is NativeFailure { if e.domain() == "core.utf" && e.code() == "invalid_data" && e.operation() == "runtime/utf@1::utf8_decode/1" && e.message() == "Bytes are not valid UTF-8" { 42; } else { 1; } }"#,
        r#"try { utf8_decode(@byte(255)); 0; } catch e is Error { if e.text() == "Bytes are not valid UTF-8" { 42; } else { 1; } }"#,
        r#"try { utf8_decode(@byte(255)); 0; } catch e is NativeFailure { let err is Error = e; if err.text() == e.message() { 42; } else { 1; } }"#,
        r#"try { try { utf8_decode(@byte(255)); 0; } catch e is Error { throw e; 1; } } catch e is NativeFailure { 42; }"#,
        r#"try { try { utf8_decode(@byte(255)); 0; } catch e is NativeFailure { throw; 1; } } catch e is Error { 42; }"#,
    ] {
        assert_eq!(execute(&lower(body)).unwrap().to_string(), "42", "{body}");
        both(body, "42");
    }
}

#[test]
fn ir_native_failure_runs_cleanup_and_preserves_causes() {
    let module = lower(r#"defer utf8_decode(@byte(255)); panic "primary";"#);
    assert_eq!(
        execute(&module).unwrap_err().to_string(),
        "Panic: primary\nSecondary cleanup failure:\n  NativeFailure: Bytes are not valid UTF-8\n  code: invalid_data\n  \
         domain: core.utf\n  operation: runtime/utf@1::utf8_decode/1"
    );
    let module = lower(
        r#"try { utf8_decode(@byte(255)); 0; } catch e is NativeFailure { utf16_decode(@byte(0), "big"); 0; }"#,
    );
    let RuntimeError::Unwind { failure, .. } = execute(&module).unwrap_err() else {
        panic!("expected failure with cause");
    };
    assert!(
        matches!(&failure.kind, okkama_runtime::FailureKind::Native(n) if n.message == "Bytes are not valid UTF-16BE")
    );
    assert!(
        matches!(&failure.cause().unwrap().kind, okkama_runtime::FailureKind::Native(n) if n.message == "Bytes are not valid UTF-8")
    );
}

#[test]
fn sparrow_native_failure_unwinds_cleanup() {
    for body in [
        r#"defer utf8_decode(@byte(255)); panic "primary";"#,
        r#"defer utf16_decode(@byte(0), "big"); utf8_decode(@byte(255)); 0;"#,
    ] {
        let module = lower(body);
        let expected = execute(&module).unwrap_err().to_string();
        assert_eq!(support::report(&module), expected, "{body}");
    }
}
