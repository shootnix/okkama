//! Куча одного запуска. Не знает об инструкциях, регистрах и кадрах.
use crate::{RuntimeError, invalid_state, unsupported, value::Value};
use okkama_compiler::{ir, source::Span};

#[derive(Debug, Default)]
pub(crate) struct State {
    native: std::cell::RefCell<okkama_runtime_core::native::Context>,
    objects: okkama_runtime_core::heap::ObjectHeap,
    arrays: okkama_runtime_core::heap::ArrayHeap,
}

impl State {
    pub(crate) fn set_arguments(&self, arguments: Vec<String>) {
        self.native.borrow_mut().arguments = arguments;
    }

    pub(crate) fn set_enums(&self, module: &ir::Module) {
        self.native.borrow_mut().objects = module
            .objects
            .iter()
            .map(|object| {
                let fields = object.fields.iter().map(|field| field.name.clone()).collect();
                (object.name.clone(), fields)
            })
            .collect();
        self.native.borrow_mut().enums = module
            .enums
            .iter()
            .map(|decl| {
                let name = decl.name.rsplit("::").next().unwrap_or(&decl.name).to_string();
                (name, decl.cases.clone())
            })
            .collect();
    }

    pub(crate) fn equal(
        &self,
        left: &Value,
        right: &Value,
        span: Span,
    ) -> Result<bool, RuntimeError> {
        self.objects
            .equal(&self.arrays, left, right)
            .map_err(|error| match error {
                okkama_runtime_core::heap::EqualityError::InvalidState(error) => {
                    invalid_state(span, error.message)
                }
                okkama_runtime_core::heap::EqualityError::Any => RuntimeError::Trap {
                    span,
                    message: "Cannot compare any without narrowing".into(),
                },
                okkama_runtime_core::heap::EqualityError::MixedTypes => RuntimeError::Trap {
                    span,
                    message: "Cannot compare values of different types".into(),
                },
            })
    }

    /// Объявление объекта; системный `Cancelled` рантайма — объявление
    /// `Cancelled` модуля, если программа его называет.
    pub(crate) fn object_declaration(
        &self,
        module: &ir::Module,
        value: &Value,
        span: Span,
    ) -> Result<usize, RuntimeError> {
        let declaration = self
            .objects
            .declaration(value)
            .map_err(|e| invalid_state(span, e.message))?;
        if declaration != okkama_runtime_core::heap::CANCELLED {
            return Ok(declaration);
        }
        Ok(module
            .objects
            .iter()
            .position(|object| object.builtin && object.name == "Cancelled")
            .unwrap_or(declaration))
    }
    pub(crate) fn role_witness<'a>(
        &self,
        module: &'a ir::Module,
        value: &Value,
        role: ir::RoleId,
        span: Span,
    ) -> Result<Option<&'a ir::Witness>, RuntimeError> {
        let declaration = self.object_declaration(module, value, span)?;
        if declaration == okkama_runtime_core::heap::CANCELLED {
            return Ok(None);
        }
        let object = module
            .objects
            .get(declaration)
            .ok_or_else(|| invalid_state(span, "object declaration is missing"))?;
        Ok(object.does.iter().find(|witness| witness.role == role))
    }
    pub(crate) fn native(
        &self,
        symbol: &str,
        args: &[Value],
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let Some(native) = okkama_runtime_core::abi::Native::resolve(symbol) else {
            // Как JVM: native программы без provider — ошибка вызова; provider
            // `runtime/*` обязан быть у исполнителя.
            if !symbol.starts_with("runtime/") {
                return Err(RuntimeError::Trap {
                    span,
                    message: format!("Native symbol \"{symbol}\" is not registered"),
                });
            }
            return Err(unsupported(
                span,
                format!("native provider is not implemented: {symbol}"),
            ));
        };
        okkama_runtime_core::native::execute(
            native,
            args,
            &mut okkama_runtime_core::heap::ArrayHeap,
            &mut self.native.borrow_mut(),
        )
            .map_err(|error| match error {
                okkama_runtime_core::native::Error::InvalidState(message) => {
                    invalid_state(span, message)
                }
                okkama_runtime_core::native::Error::Unsupported(message) => {
                    unsupported(span, message)
                }
                okkama_runtime_core::native::Error::Trap(message) => {
                    RuntimeError::Trap { span, message }
                }
                okkama_runtime_core::native::Error::Failure(failure) => {
                    RuntimeError::NativeFailure { span, failure }
                }
            })
    }
    pub(crate) fn builder_new(&self) -> Value {
        self.arrays.builder_new()
    }
    pub(crate) fn builder_range(
        &self,
        builder: &Value,
        start: &ir::IntValue,
        end: &ir::IntValue,
        inclusive: bool,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let values =
            okkama_runtime_core::integer::range(start, end, inclusive).map_err(|message| {
                RuntimeError::Trap {
                    span,
                    message: message.into(),
                }
            })?;
        self.arrays
            .builder_extend(builder, values.map(Value::Int))
            .map_err(|e| invalid_state(span, e.message))
    }
    pub(crate) fn builder_push(
        &self,
        builder: &Value,
        value: Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        self.arrays
            .builder_push(builder, value)
            .map_err(|e| invalid_state(span, e.message))
    }
    pub(crate) fn builder_append(
        &self,
        builder: &Value,
        array: &Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        self.arrays
            .builder_append(builder, array)
            .map_err(|e| invalid_state(span, e.message))
    }
    pub(crate) fn builder_finish(
        &self,
        builder: &Value,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        self.arrays
            .builder_finish(builder)
            .map_err(|e| invalid_state(span, e.message))
    }
    pub(crate) fn str_utf8(&self, text: &str) -> Value {
        self.arrays.utf8(text)
    }
    pub(crate) fn array_slice(
        &self,
        array: &Value,
        start: &ir::IntValue,
        end: &ir::IntValue,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        let elements = self.array(array, span)?;
        let range = okkama_runtime_core::integer::slice_range(elements.len(), start, end).map_err(
            |message| RuntimeError::Trap {
                span,
                message: message.into(),
            },
        )?;
        let elements = elements[range].to_vec();
        Ok(self.arrays.allocate(elements))
    }
    pub(crate) fn str_split(&self, text: &str, separator: &str) -> Value {
        self.arrays.split(text, separator)
    }

    pub(crate) fn array_join(
        &self,
        array: &Value,
        separator: &str,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        self.arrays
            .join(array, separator)
            .map(|s| Value::Str(std::rc::Rc::from(s)))
            .map_err(|e| invalid_state(span, e.message))
    }
    pub(crate) fn array_new(&self, elements: Vec<Value>) -> Value {
        self.arrays.allocate(elements)
    }

    fn array<'a>(&self, value: &'a Value, span: Span) -> Result<&'a [Value], RuntimeError> {
        self.arrays
            .elements(value)
            .map_err(|e| invalid_state(span, e.message))
    }

    pub(crate) fn array_len(&self, array: &Value, span: Span) -> Result<Value, RuntimeError> {
        Ok(Value::Int(ir::IntValue::from_u128(
            self.array(array, span)?.len() as u128,
        )))
    }

    pub(crate) fn array_lookup<'a>(
        &self,
        array: &'a Value,
        index: &ir::IntValue,
        span: Span,
    ) -> Result<Option<&'a Value>, RuntimeError> {
        let elements = self.array(array, span)?;
        Ok(index
            .to_i64()
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| elements.get(i)))
    }

    pub(crate) fn array_get(
        &self,
        array: &Value,
        index: &ir::IntValue,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        self.array_lookup(array, index, span)?
            .cloned()
            .ok_or_else(|| RuntimeError::Trap {
                span,
                message: "Array index out of bounds".into(),
            })
    }

    pub(crate) fn object_new(
        &self,
        id: ir::ObjectId,
        declaration: &ir::Object,
        fields: Vec<Value>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if declaration.builtin {
            return Err(invalid_state(span, "runtime objects are created by the runtime"));
        }
        Ok(self.objects.allocate(id.0, fields))
    }

    pub(crate) fn object_get(
        &self,
        value: &Value,
        field: usize,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        if let Value::NativeFailure(failure) = value {
            let text = match field {
                0 => &failure.domain,
                1 => &failure.code,
                2 => &failure.operation,
                3 => &failure.message,
                _ => {
                    return Err(invalid_state(
                        span,
                        "native failure field index out of bounds",
                    ));
                }
            };
            return Ok(Value::Str(std::rc::Rc::from(text.as_str())));
        }
        self.objects
            .field(value, field)
            .map_err(|e| invalid_state(span, e.message))
    }

    pub(crate) fn object_init(
        &self,
        object: &Value,
        field: usize,
        value: Value,
        span: Span,
    ) -> Result<(), RuntimeError> {
        self.objects
            .init(object, field, value)
            .map_err(|e| invalid_state(span, e.message))
    }

    pub(crate) fn object_next(
        &self,
        value: &Value,
        updates: Vec<(usize, Value)>,
        span: Span,
    ) -> Result<Value, RuntimeError> {
        self.objects
            .successor(value, updates)
            .map_err(|e| invalid_state(span, e.message))
    }
}
