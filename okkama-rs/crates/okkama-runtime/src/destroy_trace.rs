//! Sparrow-compatible destruction traces (#561).
use okkama_compiler::ir;
use okkama_runtime_core::heap::Object;
use std::collections::HashMap;
use std::io::Write;

#[derive(Debug, Default)]
pub(crate) struct DestroyTrace {
    file: Option<std::fs::File>,
    flags: Vec<bool>,
    serial: u64,
    identities: HashMap<u64, u64>,
}

impl DestroyTrace {
    pub(crate) fn new(module: &ir::Module) -> Self {
        let file = std::env::var_os("OKKAMA_TRACE_DESTROY")
            .and_then(|path| std::fs::OpenOptions::new().create(true).append(true).open(path).ok());
        let flags = if file.is_some() { descriptor_flags(module) } else { Vec::new() };
        Self { file, flags, ..Self::default() }
    }

    pub(crate) fn created(&mut self, object: &Object) {
        if self.flags.get(object.declaration) == Some(&true) {
            self.serial += 1;
            self.identities.insert(object.serial, self.serial);
        }
    }

    pub(crate) fn destroy(&mut self, object: &Object, name: &str) {
        if let Some(file) = &mut self.file
            && let Some(serial) = self.identities.remove(&object.serial)
        {
            let _ = writeln!(file, "{name} {serial}");
        }
    }
}

// The same closed-world fixed point as Sparrow's descriptor_flags (#561).
fn descriptor_flags(module: &ir::Module) -> Vec<bool> {
    let mut flags: Vec<_> = module.objects.iter().map(|object| object.destroy.is_some()).collect();
    if !flags.iter().any(|flag| *flag) {
        return flags;
    }
    loop {
        let mut changed = false;
        for (index, object) in module.objects.iter().enumerate() {
            if !flags[index] && object.fields.iter().any(|field| reaches(&field.ty, module, &flags)) {
                flags[index] = true;
                changed = true;
            }
        }
        if !changed {
            return flags;
        }
    }
}

fn reaches(ty: &ir::Type, module: &ir::Module, flags: &[bool]) -> bool {
    use ir::Type;
    match ty {
        Type::Object(id) | Type::Successor(id) => flags[id.0],
        Type::Array(element) | Type::Map(_, element) => reaches(element, module, flags),
        Type::Role(role) if !module.roles[role.0].builtin => module.objects.iter().enumerate()
            .any(|(index, object)| flags[index] && object.does.iter().any(|witness| witness.role == *role)),
        Type::Variant(variant) => module.variants[variant.0].cases.iter()
            .any(|(_, object)| flags[object.0]),
        Type::Int | Type::Bool | Type::Byte | Type::Char | Type::Float | Type::Rat
        | Type::Str | Type::Enum(_) | Type::Range | Type::Region | Type::Builder(_)
        | Type::MapBuilder(..) | Type::Handle(_) => false,
        Type::Role(_) | Type::Failure | Type::Never | Type::Any | Type::Fn(_)
        | Type::Future(_) | Type::Cap(_) => true,
    }
}

#[cfg(test)]
mod tests {
    use okkama_compiler::{SourceFile, ir, lowering, syntax};

    fn lower(text: &str) -> ir::Module {
        let source = SourceFile::new("destroy_trace.ok", format!("module app v1.0.0; {text}")).unwrap();
        let ast = syntax::parse(&source, &syntax::ParseOptions::default()).unwrap();
        lowering::lower_program(&ast).unwrap()
    }

    #[test]
    fn trace_destroy_programs() {
        const CHILD: &str = "OKKAMA_TEST_DESTROY_TRACE_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let directory = std::env::temp_dir().join(format!("okkama-destroy-{}", std::process::id()));
            std::fs::create_dir(&directory).unwrap();
            let path = directory.join("trace");
            // A separate test process sets the environment without unsafe (#561).
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "destroy_trace::tests::trace_destroy_programs", "--nocapture"])
                .env(CHILD, "1")
                .env("OKKAMA_TRACE_DESTROY", &path)
                .output()
                .unwrap();
            std::fs::remove_dir_all(directory).unwrap();
            assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
            return;
        }
        let path = std::env::var_os("OKKAMA_TRACE_DESTROY").unwrap();
        let cases = [
            (
                "object Res { Res(); destroy {} } object Holder { has res is Res; Holder(:res); destroy {} } main() is int { { let a = Res(); let b = Res(); let h = Holder(res: a); } 0; }",
                "Holder 3\nRes 2\nRes 1\n",
                false,
            ),
            (
                "object Plain { Plain(); } object Res { Res(); destroy {} } object Box { has res is Res; Box(:res); } main() is int { { let p = Plain(); let a = Res(); let box = Box(res: a); let b = Res(); } 0; }",
                "Res 3\nRes 1\n",
                false,
            ),
            (
                "object Res { Res(); async destroy {} } async main() is int { { let a = Res(); let b = Res(); } 0; }",
                "Res 2\nRes 1\n",
                false,
            ),
            (
                "object Res { Res(); destroy { panic \"closed\"; } } main() is int { let a = Res(); let b = Res(); 0; }",
                "Res 2\nRes 1\n",
                true,
            ),
            (
                "object Res { Res(); destroy {} } main() is int { let a = Res(); panic \"stop\"; }",
                "Res 1\n",
                true,
            ),
            (
                "object Res { Res(); destroy {} } object Box { has items is @Res; Box(:items); } main() is int { let box = Box(items: []); let a = Res(); 0; }",
                "Res 2\n",
                false,
            ),
            (
                "role Reading { read() is int; } object Plain does Reading { Plain(); read() is int { 0; } } object Res does Reading { Res(); read() is int { 1; } destroy {} } object Box { has value is Reading; Box(:value); } main() is int { let box = Box(value: Plain()); let a = Res(); 0; }",
                "Res 2\n",
                false,
            ),
            (
                "object Res { Res(); destroy {} } object Box { has value is Maybe<Res>; Box(:value); } main() is int { let box = Box(value: Maybe.none); let a = Res(); 0; }",
                "Res 2\n",
                false,
            ),
            (
                "object Res { has n is int; Res(:n); next() is ^Res { ^n = n + 1; } value() is int { n; } destroy {} } main() is int { let a = Res(n: 0); let b = a.next(); b.value(); }",
                "Res 2\nRes 1\n",
                false,
            ),
            (
                "object Marker { Marker(); destroy {} } object Res { Res(); destroy { let marker = Marker(); } } main() is int { let a = Res(); 0; }",
                "Res 1\nMarker 2\n",
                false,
            ),
            (
                "object Marker { Marker(); async destroy {} } object Res { Res(); async destroy { let marker = Marker(); } } async main() is int { let a = Res(); 0; }",
                "Res 1\nMarker 2\n",
                false,
            ),
            (
                "object Plain { Plain(); } main() is int { let p = Plain(); 0; }",
                "",
                false,
            ),
        ];
        for (source, expected, fails) in cases {
            std::fs::write(&path, "prefix\n").unwrap();
            let module = lower(source);
            // Repeated runs restart numbering but append to the file (#561).
            for _ in 0..2 {
                assert_eq!(crate::execute(&module).is_err(), fails, "{source}");
            }
            assert_eq!(std::fs::read_to_string(&path).unwrap(), format!("prefix\n{expected}{expected}"), "{source}");
        }
        let mut module = lower("object Res { Res(); destroy {} } main() is int { let a = Res(); 0; }");
        module.objects[0].name = "example.resources::Res".into();
        std::fs::write(&path, "").unwrap();
        crate::execute(&module).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "example.resources::Res 1\n");
    }
}
