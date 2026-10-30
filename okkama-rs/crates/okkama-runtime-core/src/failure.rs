//! Engine-independent, immutable records carried on exceptional control-flow edges.
use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::Rc,
};

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

/// Место кадра на пути отказа: функция программы и номер инструкции в ней.
/// Нумерация общая для IR и Sparrow: инструкции блоков по порядку, терминатор
/// блока — после его инструкций (кодогенератор Sparrow сохраняет её 1:1).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Site {
    pub function: u32,
    pub pc: u32,
}

/// Стек вызовов отказа: кадры, через которые он прошёл, от места
/// возникновения к корню. Кадр записывается один раз — при первом приходе
/// отказа в него; обработчик региона, перебрасывающий тот же отказ из того же
/// кадра, повтора не даёт. Это диагностика, а не значение: в равенстве
/// отказов не участвует.
#[derive(Clone, Debug, Default)]
pub struct Trace {
    sites: RefCell<Vec<Site>>,
    /// Кадр последней записи: (ключ кадра, функция).
    last: Cell<Option<(usize, u32)>>,
}

impl Trace {
    /// `frame` — ключ живого кадра (адрес его регистров): различает
    /// рекурсивные вызовы одной функции.
    pub fn record(&self, frame: usize, site: Site) {
        let key = Some((frame, site.function));
        if self.last.get() == key {
            return;
        }
        self.last.set(key);
        self.sites.borrow_mut().push(site);
    }

    pub fn sites(&self) -> Vec<Site> {
        self.sites.borrow().clone()
    }
}

impl PartialEq for Trace {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Trace {}

/// Отказ изменяем на месте, как `okkama.rt.Failure` JVM: вторичные отказы
/// и причина дописываются в тот же объект, и это видят все его ссылки.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    pub kind: Kind,
    pub secondary: RefCell<Vec<Rc<Failure>>>,
    pub cause: RefCell<Option<Rc<Failure>>>,
    pub trace: Trace,
}

impl Failure {
    pub fn new(kind: Kind) -> Rc<Self> {
        Rc::new(Self {
            kind,
            secondary: RefCell::new(Vec::new()),
            cause: RefCell::new(None),
            trace: Trace::default(),
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

/// Наибольшее число строк стека в диагностике: длиннее — середина опускается.
const STACK_LINES: usize = 40;

/// Имя функции для стека: символ без номера перегрузки или конструктора
/// (`Bag.new#1` → `Bag.new`); номер анонимного действия остаётся.
pub fn frame_name(symbol: &str) -> &str {
    match symbol.rsplit_once('#') {
        Some((name, number))
            if !name.ends_with("lambda") && !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) =>
        {
            name
        }
        _ => symbol,
    }
}

/// Текст стека вызовов из строк кадров (от места отказа к корню): подряд
/// идущие одинаковые строки (рекурсия) схлопываются, а слишком длинный стек
/// показывается началом и концом.
pub fn stack_text(header: &str, frames: &[String]) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut index = 0;
    while index < frames.len() {
        let repeat = frames[index..].iter().take_while(|frame| **frame == frames[index]).count();
        if repeat > 1 {
            lines.push(format!("  at {}  (\u{00d7}{repeat})", frames[index]));
        } else {
            lines.push(format!("  at {}", frames[index]));
        }
        index += repeat;
    }
    if lines.len() > STACK_LINES {
        let tail = 10;
        let head = STACK_LINES - tail - 1;
        let omitted = lines.len() - head - tail;
        let rest = lines.split_off(lines.len() - tail);
        lines.truncate(head);
        lines.push(format!("  ... {omitted} more frames ..."));
        lines.extend(rest);
    }
    let mut out = String::from(header);
    out.push('\n');
    for line in lines {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_names_drop_overload_numbers_but_keep_lambdas() {
        assert_eq!(frame_name("Bag.new#1"), "Bag.new");
        assert_eq!(frame_name("Box<int>.get"), "Box<int>.get");
        assert_eq!(frame_name("lambda#0"), "lambda#0");
        assert_eq!(frame_name("main"), "main");
    }

    #[test]
    fn trace_records_each_frame_once() {
        let trace = Trace::default();
        trace.record(1, Site { function: 2, pc: 5 });
        // Обработчик региона того же кадра перебрасывает отказ: повтора нет.
        trace.record(1, Site { function: 2, pc: 9 });
        trace.record(2, Site { function: 2, pc: 3 });
        trace.record(3, Site { function: 0, pc: 7 });
        assert_eq!(
            trace.sites(),
            [Site { function: 2, pc: 5 }, Site { function: 2, pc: 3 }, Site { function: 0, pc: 7 }]
        );
    }

    #[test]
    fn stack_text_collapses_recursion_and_elides_long_stacks() {
        let frames = ["f (a.ok:1:1)", "f (a.ok:2:1)", "f (a.ok:2:1)", "main (a.ok:3:1)"].map(String::from);
        assert_eq!(
            stack_text("Stack:", &frames),
            "Stack:\n  at f (a.ok:1:1)\n  at f (a.ok:2:1)  (\u{00d7}2)\n  at main (a.ok:3:1)\n"
        );
        let frames: Vec<String> = (0..100).map(|index| format!("f{index}")).collect();
        let text = stack_text("Stack:", &frames);
        assert_eq!(text.lines().count(), 1 + STACK_LINES);
        assert!(text.contains("  ... 61 more frames ...\n"), "{text}");
        assert!(text.ends_with("  at f99\n"), "{text}");
    }
}
