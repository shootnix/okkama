//! Семантические типы Okkama.
//!
//! В отличие от типов OIR, generic-объявление здесь одно: `Box<T>` и
//! `Box<int>` — это `Object(Box, [Param T])` и `Object(Box, [Int])`.
//! Тела generic-объявлений проверяются один раз, с параметрами как
//! непрозрачными типами; мономорфизация — забота lowering.

/// Объект модуля (включая системные `NativeFailure` и `Cancelled`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId(pub usize);

/// Роль (включая системную `Error`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoleId(pub usize);

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EnumId(pub usize);

/// `native object Name;` — непрозрачный тип рантайма.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct HandleId(pub usize);

/// Параметр generic-объявления (`T` в `Box<T>`).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ParamId(pub usize);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Ty {
    Int,
    Bool,
    Byte,
    Char,
    Float,
    Rat,
    Str,
    /// Явная верхняя граница `any` (§6.2): непрозрачная коробка.
    Any,
    Range,
    /// `@T`; `@never` — нетегированный пустой литерал `[]`.
    Array(Box<Ty>),
    /// `%K:V`; `%never:never` — нетегированный `[:]`.
    Map(Box<Ty>, Box<Ty>),
    /// Объект с аргументами типа (пусто у не-generic).
    Object(ObjectId, Vec<Ty>),
    /// Ограниченная ссылка роли с аргументами типа.
    Role(RoleId, Vec<Ty>),
    Enum(EnumId),
    /// Nominal tagged variants, including generic enums.
    Variant(EnumId, Vec<Ty>),
    Handle(HandleId),
    /// `^C`: обязательство использования результата successor (§11.9).
    Successor(Box<Ty>),
    /// Параметр типа внутри своего объявления.
    Param(ParamId),
    /// Тип action-значения `(A, B) is R` / `async (A) is void`.
    Action(Box<ActionTy>),
    /// Результат вызова async-действия; `None` — async void.
    Future(Option<Box<Ty>>),
    /// `Owner::Cap` — cap объекта (`index` — номер cap владельца).
    Cap(ObjectId, Vec<Ty>, usize),
    /// Variadic-привязка `values is ...T` внутри действия.
    Varargs(Box<Ty>),
    /// Контекстная возможность `test` (§14): доступна во время теста.
    Test,
    /// Нижний тип: элемент пустой нетегированной коллекции.
    Never,
    /// Тип после уже сообщённой ошибки: не порождает новых.
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ActionTy {
    pub is_async: bool,
    pub params: Vec<Ty>,
    /// `None` — `is void`.
    pub result: Option<Ty>,
}

/// Deepest type a local may hold (#49). Written types are bounded by the
/// parser's nesting limit, but a chain of `let`s can deepen a generic type
/// one level per step (`let b2 = b1.deeper();` with `deeper() is Box<@T>`),
/// and lowering keeps a full copy of each value's type: memory grows with
/// the square of the depth (2 000 levels took 840 MB to build).
pub const MAX_TYPE_DEPTH: usize = 64;

impl Ty {
    /// Nesting depth (a scalar is 1), counted without recursion up to
    /// `cap + 1`: a deeper type stops the walk.
    pub fn depth_up_to(&self, cap: usize) -> usize {
        let mut deepest = 0;
        let mut stack = vec![(self, 1usize)];
        while let Some((ty, depth)) = stack.pop() {
            deepest = deepest.max(depth);
            if deepest > cap {
                return deepest;
            }
            let next = depth + 1;
            match ty {
                Ty::Array(inner) | Ty::Successor(inner) | Ty::Varargs(inner) => stack.push((inner, next)),
                Ty::Future(inner) => stack.extend(inner.iter().map(|inner| (&**inner, next))),
                Ty::Map(key, value) => stack.extend([(&**key, next), (&**value, next)]),
                Ty::Object(_, args) | Ty::Role(_, args) | Ty::Variant(_, args) | Ty::Cap(_, args, _) => {
                    stack.extend(args.iter().map(|arg| (arg, next)))
                }
                Ty::Action(action) => {
                    stack.extend(action.params.iter().chain(action.result.iter()).map(|ty| (ty, next)))
                }
                _ => {}
            }
        }
        deepest
    }

    pub fn array(element: Ty) -> Ty {
        Ty::Array(Box::new(element))
    }

    pub fn map(key: Ty, value: Ty) -> Ty {
        Ty::Map(Box::new(key), Box::new(value))
    }

    pub fn object(id: ObjectId) -> Ty {
        Ty::Object(id, Vec::new())
    }

    pub fn role(id: RoleId) -> Ty {
        Ty::Role(id, Vec::new())
    }

    /// Тип после погашения обязательства `^C` → `C`.
    pub fn taken(&self) -> Ty {
        match self {
            Ty::Successor(inner) => (**inner).clone(),
            other => other.clone(),
        }
    }

    pub fn is_successor(&self) -> bool {
        matches!(self, Ty::Successor(_))
    }

    /// `[]` без целевого типа.
    pub fn is_untagged_array(&self) -> bool {
        matches!(self, Ty::Array(element) if **element == Ty::Never)
    }

    /// `[:]` без целевого типа.
    pub fn is_untagged_map(&self) -> bool {
        matches!(self, Ty::Map(key, _) if **key == Ty::Never)
    }

    pub fn is_untagged(&self) -> bool {
        self.is_untagged_array() || self.is_untagged_map()
    }

    /// Ключ словаря (§6.3, D-007: `byte` ключом не бывает).
    pub fn is_map_key(&self) -> bool {
        matches!(
            self,
            Ty::Int | Ty::Bool | Ty::Char | Ty::Float | Ty::Str | Ty::Enum(_)
        )
    }

    pub fn is_numeric(&self) -> bool {
        matches!(self, Ty::Int | Ty::Byte | Ty::Float | Ty::Rat)
    }

    /// Тип содержит уже сообщённую ошибку: проверки с ним молчат.
    pub fn is_error(&self) -> bool {
        match self {
            Ty::Error => true,
            Ty::Array(inner) | Ty::Successor(inner) | Ty::Varargs(inner) => inner.is_error(),
            Ty::Map(key, value) => key.is_error() || value.is_error(),
            Ty::Object(_, args)
            | Ty::Role(_, args)
            | Ty::Variant(_, args)
            | Ty::Cap(_, args, _) => args.iter().any(Ty::is_error),
            Ty::Action(action) => {
                action.params.iter().any(Ty::is_error)
                    || action.result.as_ref().is_some_and(Ty::is_error)
            }
            Ty::Future(Some(inner)) => inner.is_error(),
            _ => false,
        }
    }

    /// Подстановка параметров типа.
    pub fn subst(&self, env: &[(ParamId, Ty)]) -> Ty {
        if env.is_empty() {
            return self.clone();
        }
        match self {
            Ty::Param(param) => env
                .iter()
                .find(|(bound, _)| bound == param)
                .map_or_else(|| self.clone(), |(_, ty)| ty.clone()),
            Ty::Array(inner) => Ty::Array(Box::new(inner.subst(env))),
            Ty::Successor(inner) => Ty::Successor(Box::new(inner.subst(env))),
            Ty::Varargs(inner) => Ty::Varargs(Box::new(inner.subst(env))),
            Ty::Map(key, value) => Ty::Map(Box::new(key.subst(env)), Box::new(value.subst(env))),
            Ty::Variant(id, args) => {
                Ty::Variant(*id, args.iter().map(|arg| arg.subst(env)).collect())
            }
            Ty::Object(id, args) => {
                Ty::Object(*id, args.iter().map(|arg| arg.subst(env)).collect())
            }
            Ty::Role(id, args) => Ty::Role(*id, args.iter().map(|arg| arg.subst(env)).collect()),
            Ty::Cap(id, args, cap) => {
                Ty::Cap(*id, args.iter().map(|arg| arg.subst(env)).collect(), *cap)
            }
            Ty::Action(action) => Ty::Action(Box::new(ActionTy {
                is_async: action.is_async,
                params: action.params.iter().map(|param| param.subst(env)).collect(),
                result: action.result.as_ref().map(|result| result.subst(env)),
            })),
            Ty::Future(Some(inner)) => Ty::Future(Some(Box::new(inner.subst(env)))),
            other => other.clone(),
        }
    }

    /// Встречается ли параметр типа.
    pub fn mentions_param(&self) -> bool {
        match self {
            Ty::Param(_) => true,
            Ty::Array(inner) | Ty::Successor(inner) | Ty::Varargs(inner) => inner.mentions_param(),
            Ty::Map(key, value) => key.mentions_param() || value.mentions_param(),
            Ty::Object(_, args)
            | Ty::Role(_, args)
            | Ty::Variant(_, args)
            | Ty::Cap(_, args, _) => args.iter().any(Ty::mentions_param),
            Ty::Action(action) => {
                action.params.iter().any(Ty::mentions_param)
                    || action.result.as_ref().is_some_and(Ty::mentions_param)
            }
            Ty::Future(Some(inner)) => inner.mentions_param(),
            _ => false,
        }
    }
}
