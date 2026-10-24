//! Главный класс: constant pool, вызовы рантайма `okkama.rt` и launcher.
//!
//! Проверки и операции, которые OIR описывает как контракты инструкций
//! (`%`, битовые слова, индексы, строки по code points, отказы), реализованы в
//! Java-рантайме `okkama.rt.Rt` (`runtime/jvm/src`). Его class-файлы
//! встроены в компилятор и выдаются рядом с программой.

use super::asm::{Asm, VType};
use super::classfile::{ConstantPool, Method};
use super::{BIG, Layout, Result, STRING};
use crate::ir::Type;

pub(super) const RT: &str = "okkama/rt/Rt";
pub(super) const FAILURE: &str = "okkama/rt/Failure";
pub(super) const ERROR: &str = "okkama/rt/Error";
pub(super) const LIST: &str = "java/util/List";
pub(super) const MAP: &str = "java/util/Map";
pub(super) const CASE: &str = "okkama/rt/Case";
pub(super) const OBJ: &str = "okkama/rt/Obj";
pub(super) const REGION: &str = "okkama/rt/Region";
pub(super) const FLOATS: &str = "okkama/rt/Floats";
pub(super) const DOUBLE: &str = "java/lang/Double";
pub(super) const RAT: &str = "okkama/rt/Rat";
pub(super) const KEY_ORDER: &str = "okkama/rt/KeyOrder";
pub(super) const NATIVE_FAILURE: &str = "okkama/rt/NativeFailure";
pub(super) const ANY: &str = "okkama/rt/Any";
pub(super) const RANGE: &str = "okkama/rt/Range";
pub(super) const FN: &str = "okkama/rt/Fn";
pub(super) const FUTURE: &str = "okkama/rt/Future";
pub(super) const CANCELLED: &str = "okkama/rt/Cancelled";
pub(super) const INTRINSICS_CLASS: &str = "okkama/rt/Intrinsics";
pub(super) const SCHEDULER: &str = "okkama/rt/Scheduler";

// Providers natives std: (метод, класс, дескриптор), упорядочены по методу.
// Генерирует build.rs из class-файлов рантайма; метод — символ, где `/`
// заменён на `_`, а `::` — на `__` (`runtime_json__json_parse_state`).
include!(concat!(env!("OUT_DIR"), "/native_methods.rs"));

/// Символ без версии и арности: `runtime/utf@1::utf8_encode/1` →
/// `runtime/utf::utf8_encode` (имя метода provider — по нему).
fn plain_symbol(symbol: &str) -> String {
    let (module, rest) = symbol.split_once("::").unwrap_or((symbol, ""));
    let module = module.split('@').next().unwrap_or(module);
    let name = rest.rsplit_once('/').map_or(rest, |(name, _)| name);
    format!("{module}::{name}")
}

/// Provider native `symbol` (`runtime/utf::utf8_encode`): метод, класс и
/// дескриптор.
fn native_provider(symbol: &str) -> Option<(&'static str, &'static str, &'static str)> {
    let method = symbol.replace('/', "_").replace("::", "__");
    NATIVE_METHODS
        .binary_search_by(|(name, _, _)| (*name).cmp(method.as_str()))
        .ok()
        .map(|found| NATIVE_METHODS[found])
}

const ACC_PUBLIC_STATIC: u16 = 0x0009;

// Встроенные class-файлы рантайма (путь относительно каталога программы);
// список генерирует build.rs: каждый class-файл `runtime/jvm/classes/okkama/rt`.
include!(concat!(env!("OUT_DIR"), "/runtime_files.rs"));

pub(super) struct ClassGen {
    pub pool: ConstantPool,
    pub layout: Layout,
}

impl ClassGen {
    pub fn new(layout: Layout) -> Self {
        Self {
            pool: ConstantPool::default(),
            layout,
        }
    }

    /// `invokestatic okkama/rt/Rt.<name><descriptor>`.
    pub fn call_rt(
        &mut self,
        asm: &mut Asm,
        name: &str,
        descriptor: &str,
        pops: i32,
        pushes: i32,
    ) -> Result<()> {
        self.invoke_static(asm, RT, name, descriptor, pops, pushes)
    }

    pub fn invoke_virtual(
        &mut self,
        asm: &mut Asm,
        owner: &str,
        name: &str,
        descriptor: &str,
        pops: i32,
        pushes: i32,
    ) -> Result<()> {
        let index = self.pool.method_ref(owner, name, descriptor)?;
        asm.op_index(0xb6, index, pops, pushes);
        Ok(())
    }

    pub fn invoke_static(
        &mut self,
        asm: &mut Asm,
        owner: &str,
        name: &str,
        descriptor: &str,
        pops: i32,
        pushes: i32,
    ) -> Result<()> {
        let index = self.pool.method_ref(owner, name, descriptor)?;
        asm.op_index(0xb8, index, pops, pushes);
        Ok(())
    }

    pub fn invoke_interface(
        &mut self,
        asm: &mut Asm,
        owner: &str,
        name: &str,
        descriptor: &str,
        slots: u8,
        pushes: i32,
    ) -> Result<()> {
        let index = self.pool.interface_method_ref(owner, name, descriptor)?;
        asm.invoke_interface(index, slots, i32::from(slots), pushes);
        Ok(())
    }

    /// Класс JVM значения типа OIR в коллекциях (`bool` — `Boolean`, `byte` —
    /// `Byte`, `char` — `Integer`).
    pub fn class_of(&self, ty: &Type) -> String {
        match ty {
            Type::Int => BIG.to_string(),
            Type::Bool => "java/lang/Boolean".to_string(),
            Type::Byte => "java/lang/Byte".to_string(),
            Type::Float => DOUBLE.to_string(),
            Type::Rat => RAT.to_string(),
            Type::Char => "java/lang/Integer".to_string(),
            Type::Str => STRING.to_string(),
            Type::Object(object) | Type::Successor(object) => {
                self.layout.objects[object.0].class.clone()
            }
            Type::Role(role) => self.layout.roles[role.0].class.clone(),
            Type::Enum(_) => CASE.to_string(),
            Type::Region => REGION.to_string(),
            Type::Handle(_) => "java/lang/Object".to_string(),
            Type::Any => ANY.to_string(),
            Type::Range => RANGE.to_string(),
            Type::Fn(_) => FN.to_string(),
            Type::Future(_) => FUTURE.to_string(),
            Type::Cap(id) => self.layout.objects[self.layout.caps[id.0].0].class.clone(),
            Type::Failure => FAILURE.to_string(),
            Type::Array(_) | Type::Builder(_) => LIST.to_string(),
            Type::Map(..) | Type::MapBuilder(..) => MAP.to_string(),
            Type::Never => "java/lang/Object".to_string(),
        }
    }

    pub fn vtype(&mut self, ty: &Type) -> Result<VType> {
        Ok(match ty {
            Type::Bool | Type::Byte | Type::Char => VType::Integer,
            other => {
                let class = self.class_of(other);
                VType::Object(self.pool.class(&class)?)
            }
        })
    }

    pub fn failure_type(&mut self) -> Result<VType> {
        self.vtype(&Type::Failure)
    }

    /// Загрузить строку и бросить программную ошибку с этим текстом.
    pub fn throw_trap(&mut self, asm: &mut Asm, message: &str) -> Result<()> {
        let text = self.pool.string(message)?;
        asm.ldc(text);
        self.call_rt(asm, "trap", "(Ljava/lang/String;)Lokkama/rt/Failure;", 1, 1)?;
        asm.op(&[0xbf], 1, 0); // athrow
        Ok(())
    }

    pub fn function_method(
        &mut self,
        asm: Asm,
        name: &str,
        descriptor: &str,
        max_locals: u16,
    ) -> Result<Method> {
        let assembled = asm
            .finish()
            .map_err(|failure| super::error(format!("{name}: {failure}")))?;
        Ok(Method {
            access: ACC_PUBLIC_STATIC,
            name: self.pool.utf8(name)?,
            descriptor: self.pool.utf8(descriptor)?,
            max_stack: assembled.max_stack,
            max_locals,
            code: assembled.code,
            exceptions: assembled.exceptions,
            stack_map: assembled.stack_map,
        })
    }

    /// `native`: статический метод, передающий параметры `okkama.rt.Natives`.
    pub fn native_method(
        &mut self,
        index: usize,
        function: &crate::ir::Function,
        symbol: &str,
    ) -> Result<Method> {
        let plain = plain_symbol(symbol);
        let (name, descriptor) = self.layout.signatures[index].clone();
        let Some((target, provider, provided)) = native_provider(&plain) else {
            if plain.starts_with("runtime/") {
                return Err(super::error(format!(
                    "native `{plain}` is not available in the JVM runtime"
                )));
            }
            // Native программы без provider — ошибка вызова, как в Perl.
            let mut asm = Asm::default();
            let message = self
                .pool
                .string(&format!("Native symbol \"{symbol}\" is not registered"))?;
            asm.ldc(message);
            self.call_rt(
                &mut asm,
                "trap",
                "(Ljava/lang/String;)Lokkama/rt/Failure;",
                1,
                1,
            )?;
            asm.op(&[0xbf], 1, 0); // athrow
            let locals = u16::try_from(function.params.len())
                .map_err(|_| super::error("native has too many parameters"))?;
            return self.function_method(asm, &name, &descriptor, locals);
        };
        if provided != descriptor {
            return Err(super::error(format!(
                "native `{plain}`: JVM provider `{provider}.{target}{provided}` does not match `{descriptor}`"
            )));
        }
        let mut asm = Asm::default();
        for (slot, param) in function.params.iter().enumerate() {
            let ty = self.vtype(&function.values[param.0])?;
            asm.load(ty, slot as u16);
        }
        let pushes = i32::from(!descriptor.ends_with(")V"));
        let method = self.pool.method_ref(provider, target, &descriptor)?;
        asm.op_index(0xb8, method, function.params.len() as i32, pushes); // invokestatic
        // Async native возвращает Future (`call_result`), а не тип результата.
        let opcode = match &function.call_result() {
            crate::ir::ResultType::Void => 0xb1,
            crate::ir::ResultType::Value(ty) => match self.vtype(ty)? {
                VType::Integer => 0xac,
                _ => 0xb0,
            },
        };
        asm.op(&[opcode], pushes, 0);
        let locals = u16::try_from(function.params.len())
            .map_err(|_| super::error("native has too many parameters"))?;
        self.function_method(asm, &name, &descriptor, locals)
    }

    /// `main(String[])`: `Rt.exit(okkamaMain())`, отказ — `Rt.report`.
    /// Intrinsic: классы объектов, которые рантайм создаёт, и аргументы в
    /// коллекционном представлении (`Object`) — в `okkama.rt.Intrinsics`
    /// (`okkama.channel.Sending.send` → `channel_Sending_send`); результат
    /// распаковывается в тип функции.
    pub fn intrinsic_method(
        &mut self,
        index: usize,
        function: &crate::ir::Function,
        name: &str,
        objects: &[crate::ir::ObjectId],
    ) -> Result<Method> {
        let (method_name, descriptor) = self.layout.signatures[index].clone();
        let mut asm = Asm::default();
        let mut target = String::from("(");
        for object in objects {
            let class = self.layout.objects[object.0].class.clone();
            let index = self.pool.class(&class)?;
            asm.ldc(index);
            target.push_str("Ljava/lang/Object;");
        }
        // Переходники действий программы — значения-функции без захватов.
        let helpers = function.intrinsic_helpers();
        for helper in helpers {
            let fn_layout = self.layout.fn_class(*helper, false);
            let (class_name, ctor) = (
                fn_layout.class.clone(),
                self.layout.fn_ctor_descriptor(fn_layout),
            );
            let class = self.pool.class(&class_name)?;
            asm.op_index(0xbb, class, 0, 1); // new
            asm.op(&[0x59], 0, 1); // dup
            let init = self.pool.method_ref(&class_name, "<init>", &ctor)?;
            asm.op_index(0xb7, init, 1, 0); // invokespecial
            target.push_str("Ljava/lang/Object;");
        }
        for (slot, param) in function.params.iter().enumerate() {
            let ty = &function.values[param.0];
            asm.load(self.vtype(ty)?, slot as u16);
            super::classes::box_value(&mut asm, &mut self.pool, ty)?;
            target.push_str("Ljava/lang/Object;");
        }
        target.push(')');
        let result = function.call_result();
        let pushes = match &result {
            crate::ir::ResultType::Void => {
                target.push('V');
                0
            }
            crate::ir::ResultType::Value(_) => {
                target.push_str("Ljava/lang/Object;");
                1
            }
        };
        let java = name.trim_start_matches("okkama.").replace('.', "_");
        let method = self.pool.method_ref(INTRINSICS_CLASS, &java, &target)?;
        let arity = (objects.len() + helpers.len() + function.params.len()) as i32;
        asm.op_index(0xb8, method, arity, pushes); // invokestatic
        let opcode = match &result {
            crate::ir::ResultType::Void => 0xb1,
            crate::ir::ResultType::Value(ty) => {
                super::classes::unbox_value(&mut asm, &mut self.pool, &self.layout, ty)?;
                match self.vtype(ty)? {
                    VType::Integer => 0xac,
                    _ => 0xb0,
                }
            }
        };
        asm.op(&[opcode], pushes, 0);
        let locals = u16::try_from(function.params.len())
            .map_err(|_| super::error("intrinsic has too many parameters"))?;
        self.function_method(asm, &method_name, &descriptor, locals)
    }

    /// Метод `name` async-функции: холодный Future над её телом и аргументами.
    pub fn async_stub(&mut self, index: usize, function: &crate::ir::Function) -> Result<Method> {
        let (name, descriptor) = self.layout.signatures[index].clone();
        let thunk = self.layout.fn_class(crate::ir::FuncId(index), true);
        let (class_name, ctor) = (thunk.class.clone(), self.layout.fn_ctor_descriptor(thunk));
        let mut asm = Asm::default();
        let class = self.pool.class(&class_name)?;
        asm.op_index(0xbb, class, 0, 1); // new
        asm.op(&[0x59], 0, 1); // dup
        for (slot, param) in function.params.iter().enumerate() {
            let ty = self.vtype(&function.values[param.0])?;
            asm.load(ty, slot as u16);
        }
        let init = self.pool.method_ref(&class_name, "<init>", &ctor)?;
        asm.op_index(0xb7, init, function.params.len() as i32 + 1, 0); // invokespecial
        let cold = self
            .pool
            .method_ref(FUTURE, "cold", "(Lokkama/rt/Fn;)Lokkama/rt/Future;")?;
        asm.op_index(0xb8, cold, 1, 1); // invokestatic
        asm.op(&[0xb0], 1, 0); // areturn
        let locals = u16::try_from(function.params.len())
            .map_err(|_| super::error("async function has too many parameters"))?;
        self.function_method(asm, &name, &descriptor, locals)
    }

    /// `main` тестового профиля: имена и значения-функции тестов —
    /// `okkama.rt.Tests.run`, который печатает отчёт и завершает процесс.
    pub fn test_launcher(&mut self, tests: &[(String, crate::ir::FuncId)]) -> Result<Method> {
        let mut asm = Asm::default();
        asm.op(&[0x2a], 0, 1); // aload_0: String[] args
        self.call_rt(&mut asm, "start", "([Ljava/lang/String;)V", 1, 0)?;
        super::classes::push_int(&mut asm, tests.len())?;
        let string = self.pool.class(STRING)?;
        asm.op_index(0xbd, string, 1, 1); // anewarray String
        for (index, (name, _)) in tests.iter().enumerate() {
            asm.op(&[0x59], 0, 1); // dup
            super::classes::push_int(&mut asm, index)?;
            let text = self.pool.string(name)?;
            asm.ldc(text);
            asm.op(&[0x53], 3, 0); // aastore
        }
        super::classes::push_int(&mut asm, tests.len())?;
        let fn_class = self.pool.class(FN)?;
        asm.op_index(0xbd, fn_class, 1, 1); // anewarray Fn
        for (index, (_, func)) in tests.iter().enumerate() {
            asm.op(&[0x59], 0, 1); // dup
            super::classes::push_int(&mut asm, index)?;
            let class_name = self.layout.fn_class(*func, false).class.clone();
            let class = self.pool.class(&class_name)?;
            asm.op_index(0xbb, class, 0, 1); // new
            asm.op(&[0x59], 0, 1); // dup
            let init = self.pool.method_ref(&class_name, "<init>", "()V")?;
            asm.op_index(0xb7, init, 1, 0); // invokespecial
            asm.op(&[0x53], 3, 0); // aastore
        }
        let run = self.pool.method_ref(
            "okkama/rt/Tests",
            "run",
            "([Ljava/lang/String;[Lokkama/rt/Fn;)V",
        )?;
        asm.op_index(0xb8, run, 2, 0); // invokestatic
        asm.op(&[0xb1], 0, 0); // return
        self.function_method(asm, "main", "([Ljava/lang/String;)V", 1)
    }

    /// `main(String[])`: `Rt.launch` исполняет точку входа в потоке программы
    /// с большим стеком (глубокая рекурсия), печатает отказ и задаёт код выхода.
    pub fn launcher(&mut self, entry: &str, entry_async: bool) -> Result<Method> {
        let main_class = self.layout.main.clone();
        let mut asm = Asm::default();
        asm.op(&[0x2a], 0, 1); // aload_0: String[] args
        let class = self.pool.class(&main_class)?;
        asm.ldc(class);
        let name = self.pool.string(entry)?;
        asm.ldc(name);
        asm.op(&[if entry_async { 0x04 } else { 0x03 }], 0, 1); // iconst_1 / iconst_0
        self.call_rt(&mut asm, "launch", "([Ljava/lang/String;Ljava/lang/Class;Ljava/lang/String;Z)V", 4, 0)?;
        asm.op(&[0xb1], 0, 0); // return
        self.function_method(asm, "main", "([Ljava/lang/String;)V", 1)
    }
}

#[cfg(test)]
mod tests {
    use super::RUNTIME_FILES;

    /// Каждый собранный class-файл рантайма встроен: иначе программа падает
    /// с `NoClassDefFoundError` только на пути, который его загружает.
    #[test]
    fn every_built_runtime_class_is_embedded() {
        let directory = concat!(env!("CARGO_MANIFEST_DIR"), "/runtime/jvm/classes/okkama/rt");
        let mut built: Vec<String> = std::fs::read_dir(directory)
            .expect("runtime classes are committed")
            .map(|entry| format!("okkama/rt/{}", entry.unwrap().file_name().to_string_lossy()))
            .collect();
        built.sort();
        let mut embedded: Vec<String> = RUNTIME_FILES
            .iter()
            .map(|(name, _)| name.to_string())
            .collect();
        embedded.sort();
        assert_eq!(built, embedded);
    }
}
