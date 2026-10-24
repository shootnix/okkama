//! Исполнитель IR для intrinsics рантайма (`okkama_runtime_core::intrinsics`):
//! объекты, поля и вызовы программы по именам, как отражение JVM.
use crate::interpreter::{Engine, cold_call, run_body};
use crate::value::Value;
use okkama_compiler::ir;
use okkama_runtime_core::failure::{Failure, Kind};
use okkama_runtime_core::heap::{ObjectHeap, Ref};
use okkama_runtime_core::intrinsics::Host;
use okkama_runtime_core::sched::Outcome;
use std::rc::Rc;

#[derive(Debug)]
pub(crate) struct IrHost(pub(crate) Rc<Engine>);

fn trap(message: impl Into<String>) -> Rc<Failure> {
    Failure::new(Kind::Program(message.into()))
}

/// Имя Okkama-типа Error-объекта: последний сегмент имени класса JVM
/// (`async.channel::ChannelClosed` → `ChannelClosed`).
pub(crate) fn error_type_name(name: &str) -> String {
    let class = name.replace(['.', ':', '<', '>', ',', ' ', '@', '%', '^'], "$");
    class.rsplit('$').next().unwrap_or(&class).to_string()
}

impl IrHost {
    fn module(&self) -> &ir::Module {
        &self.0.module
    }

    fn declaration(&self, value: &Value) -> Result<usize, Rc<Failure>> {
        match value {
            Value::Object(object) => Ok(object.declaration),
            _ => Err(trap("intrinsic expected an object")),
        }
    }

    fn invoke(&self, function: ir::FuncId, args: Vec<Value>) -> Outcome {
        if self.module().functions[function.0].is_async() {
            return Ok(Value::Future(cold_call(&self.0, function, args)));
        }
        run_body(&self.0, function, args)
    }
}

impl Host for IrHost {
    fn make(&self, declaration: usize, fields: Vec<Value>) -> Value {
        ObjectHeap.allocate(declaration, fields)
    }

    fn thrown(&self, declaration: usize, fields: Vec<Value>) -> Rc<Failure> {
        let Value::Object(object) = self.make(declaration, fields) else {
            unreachable!("allocate makes objects")
        };
        Failure::new(Kind::Thrown {
            object: Ref(object),
            name: error_type_name(&self.module().objects[declaration].name),
            message: None,
        })
    }

    fn field(&self, object: &Value, name: &str) -> Result<Value, Rc<Failure>> {
        let declaration = self.declaration(object)?;
        let index = self.module().objects[declaration]
            .fields
            .iter()
            .position(|field| field.name == name)
            .ok_or_else(|| trap(format!("Cannot read field {name}")))?;
        ObjectHeap
            .field(object, index)
            .map_err(|e| trap(e.message))
    }

    fn call(&self, function: usize, args: Vec<Value>) -> Outcome {
        self.invoke(ir::FuncId(function), args)
    }

    fn call_fn(&self, callee: &Value, args: Vec<Value>) -> Outcome {
        let Value::Fn(closure) = callee else {
            return Err(trap("intrinsic expected a function value"));
        };
        let mut arguments = closure.captures.clone();
        arguments.extend(args);
        self.invoke(ir::FuncId(closure.function), arguments)
    }

    fn call_method(&self, receiver: &Value, name: &str, args: Vec<Value>) -> Outcome {
        let module = self.module();
        let declaration = self.declaration(receiver)?;
        let object = module
            .objects
            .get(declaration)
            .ok_or_else(|| trap(format!("value has no {name}()")))?;
        for witness in &object.does {
            let role = &module.roles[witness.role.0];
            for (slot, method) in role.methods.iter().enumerate() {
                if method.name == name && method.params.len() == args.len() {
                    let mut arguments = vec![receiver.clone()];
                    arguments.extend(args);
                    return self.invoke(witness.slots[slot], arguments);
                }
            }
        }
        Err(trap(format!("Sequencing value has no {name}()")))
    }

    fn sibling(&self, object: &Value, name: &str) -> Option<usize> {
        let declaration = self.declaration(object).ok()?;
        let own = &self.module().objects.get(declaration)?.name;
        let prefix = own.rfind("::").map_or("", |at| &own[..at + 2]);
        let target = format!("{prefix}{name}");
        self.module()
            .objects
            .iter()
            .position(|object| object.name == target)
    }

    fn equal(&self, left: &Value, right: &Value) -> Result<bool, Rc<Failure>> {
        self.0
            .state
            .equal(left, right, self.module().span)
            .map_err(|error| match error {
                crate::RuntimeError::Trap { message, .. } => trap(message),
                other => trap(other.to_string()),
            })
    }

    fn show(&self, value: &Value) -> String {
        crate::tests::show(self.module(), value)
    }

    fn test_failure(&self, message: String) -> Result<(), Rc<Failure>> {
        crate::tests::fail(message)
    }
}
