//! Engine-independent, immutable records carried on exceptional control-flow edges.
use std::{cell::RefCell, fmt, rc::Rc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Native(Rc<crate::abi::NativeFailure>),
    Thrown {
        object: crate::heap::Ref<crate::heap::Object>,
        name: String,
        message: Option<String>,
    },
    Panic(String),
    Program(String),
}

/// Отказ изменяем на месте, как `okkama.rt.Failure` JVM: вторичные отказы
/// и причина дописываются в тот же объект, и это видят все его ссылки.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub kind: Kind,
    pub secondary: RefCell<Vec<Rc<Failure>>>,
    pub cause: RefCell<Option<Rc<Failure>>>,
}

impl Failure {
    pub fn new(kind: Kind) -> Rc<Self> {
        Rc::new(Self {
            kind,
            secondary: RefCell::new(Vec::new()),
            cause: RefCell::new(None),
        })
    }

    /// `Rt.secondary`: отказ cleanup при уже идущем отказе — вторичный.
    pub fn with_secondary(self: &Rc<Self>, cleanup: Rc<Self>) -> Rc<Self> {
        self.secondary.borrow_mut().push(cleanup);
        self.clone()
    }

    pub fn secondaries(&self) -> Vec<Rc<Self>> {
        self.secondary.borrow().clone()
    }

    pub fn cause(&self) -> Option<Rc<Self>> {
        self.cause.borrow().clone()
    }

    /// `Rt.caused`: обработанный отказ — причина в конце цепочки нового.
    pub fn caused(fresh: &Rc<Self>, handled: &Rc<Self>) -> Rc<Self> {
        if !matches!(fresh.kind, Kind::Thrown { .. } | Kind::Native(_)) || Rc::ptr_eq(fresh, handled) {
            return fresh.clone();
        }
        let mut last = fresh.clone();
        loop {
            let next = last.cause();
            match next {
                Some(cause) if Rc::ptr_eq(&cause, handled) => return fresh.clone(),
                Some(cause) => last = cause,
                None => break,
            }
        }
        *last.cause.borrow_mut() = Some(handled.clone());
        fresh.clone()
    }

    pub fn message(&self) -> &str {
        match &self.kind {
            Kind::Native(failure) => &failure.message,
            Kind::Thrown { message, .. } => message.as_deref().unwrap_or(""),
            Kind::Panic(message) | Kind::Program(message) => message,
        }
    }
    pub fn category(&self) -> &'static str {
        match self.kind {
            Kind::Panic(_) => "panic",
            Kind::Program(_) | Kind::Native(_) => "runtime",
            Kind::Thrown { .. } => "uncaught-error",
        }
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match &self.kind {
            Kind::Native(_) => "NativeFailure",
            Kind::Thrown { name, .. } => name.as_str(),
            Kind::Panic(_) => "Panic",
            Kind::Program(_) => "RuntimeError",
        };
        write!(f, "{prefix}: {}", self.message())?;
        for secondary in self.secondary.borrow().iter() {
            write!(f, "\nSecondary cleanup failure:\n  {secondary}")?;
        }
        Ok(())
    }
}

/// Имя Okkama-типа Error-объекта: последний сегмент имени класса JVM
/// (`async.channel::ChannelClosed` → `ChannelClosed`).
pub fn error_type_name(name: &str) -> String {
    let class = name.replace(['.', ':', '<', '>', ',', ' ', '@', '%', '^'], "$");
    class.rsplit('$').next().unwrap_or(&class).to_string()
}

/// `Rt.describe` JVM: текст одного описанного отказа.
pub fn describe(failure: &Failure) -> String {
    match &failure.kind {
        Kind::Native(native) => format!(
            "NativeError: {}\ncode: {}\ndomain: {}\noperation: {}",
            native.message, native.code, native.domain, native.operation
        ),
        Kind::Thrown { name, message, .. } => format!("{name}: {}", message.as_deref().unwrap_or("")),
        Kind::Panic(message) => format!("Panic: {message}"),
        Kind::Program(message) => format!("RuntimeError: {message}"),
    }
}
