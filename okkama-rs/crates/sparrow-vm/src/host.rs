//! Исполнитель Sparrow для intrinsics рантайма (`sparrow_runtime::intrinsics`):
//! объекты, поля и вызовы программы по именам, как отражение JVM.
use crate::machine::{Engine, cold_call, run_body};
use sparrow_runtime::Value;
use sparrow_runtime::failure::{Failure, Kind};
use sparrow_runtime::heap::{ObjectHeap, Ref};
use sparrow_runtime::intrinsics::Host;
use sparrow_runtime::sched::Outcome;
use std::rc::Rc;

#[derive(Debug)]
pub(crate) struct VmHost(pub(crate) Rc<Engine>);

fn trap(message: impl Into<String>) -> Rc<Failure> {
    Failure::new(Kind::Program(message.into()))
}

impl VmHost {
    fn declaration(&self, value: &Value) -> Result<usize, Rc<Failure>> {
        match value {
            Value::Object(object) => Ok(object.declaration),
            _ => Err(trap("intrinsic expected an object")),
        }
    }

    fn invoke(&self, function: usize, args: Vec<Value>) -> Outcome {
        if self.0.program.functions[function].is_async {
            return Ok(Value::Future(cold_call(&self.0, function, args)));
        }
        run_body(&self.0, function, args)
    }
}

impl Host for VmHost {
    fn make(&self, declaration: usize, fields: Vec<Value>) -> Value {
        ObjectHeap.allocate(declaration, fields)
    }

    fn thrown(&self, declaration: usize, fields: Vec<Value>) -> Rc<Failure> {
        let Value::Object(object) = self.make(declaration, fields) else {
            unreachable!("allocate makes objects")
        };
        Failure::new(Kind::Thrown {
            object: Ref(object),
            name: sparrow_runtime::failure::error_type_name(&self.0.program.objects[declaration].name),
            message: None,
        })
    }

    fn field(&self, object: &Value, name: &str) -> Result<Value, Rc<Failure>> {
        let declaration = self.declaration(object)?;
        let index = self.0.program.objects[declaration]
            .fields
            .iter()
            .position(|(field, _)| field == name)
            .ok_or_else(|| trap(format!("Cannot read field {name}")))?;
        ObjectHeap.field(object, index).map_err(|e| trap(e.message))
    }

    fn call(&self, function: usize, args: Vec<Value>) -> Outcome {
        self.invoke(function, args)
    }

    fn call_fn(&self, callee: &Value, args: Vec<Value>) -> Outcome {
        let Value::Fn(closure) = callee else {
            return Err(trap("intrinsic expected a function value"));
        };
        let mut arguments = closure.captures.clone();
        arguments.extend(args);
        self.invoke(closure.function, arguments)
    }

    fn call_method(&self, receiver: &Value, name: &str, args: Vec<Value>) -> Outcome {
        let program = &self.0.program;
        let declaration = self.declaration(receiver)?;
        let object = program.objects.get(declaration).ok_or_else(|| trap(format!("value has no {name}()")))?;
        for witness in &object.does {
            let role = &program.roles[witness.role.0 as usize];
            for (slot, method) in role.methods.iter().enumerate() {
                if method.name == name && method.params.len() == args.len() {
                    let mut arguments = vec![receiver.clone()];
                    arguments.extend(args);
                    return self.invoke(witness.slots[slot].0 as usize, arguments);
                }
            }
        }
        Err(trap(format!("Sequencing value has no {name}()")))
    }

    fn sibling(&self, object: &Value, name: &str) -> Option<usize> {
        let program = &self.0.program;
        let declaration = self.declaration(object).ok()?;
        let own = &program.objects.get(declaration)?.name;
        let prefix = own.rfind("::").map_or("", |at| &own[..at + 2]);
        let target = format!("{prefix}{name}");
        program.objects.iter().position(|object| object.name == target)
    }

    fn equal(&self, left: &Value, right: &Value) -> Result<bool, Rc<Failure>> {
        sparrow_runtime::ops::equal(left, right).map_err(|fault| match fault {
            sparrow_runtime::ops::Fault::Trap(message) => trap(message),
            sparrow_runtime::ops::Fault::Invalid(message) => trap(format!("invalid interpreter state: {message}")),
            sparrow_runtime::ops::Fault::Unsupported(message) => trap(format!("Sparrow: {message}")),
        })
    }

    fn show(&self, value: &Value) -> String {
        crate::tests::show(&self.0, value)
    }

    fn test_failure(&self, message: String) -> Result<(), Rc<Failure>> {
        sparrow_runtime::testing::fail(message)
    }
}
