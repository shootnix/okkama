# POC: вариантные типы с данными, Maybe и Presence

**Статус:** первая версия реализована; актуальный контракт — [спецификация, §15](../okkama-spec-11072026.md#15-enums).
**Дата:** 2 ноября 2026.  
**Основа исходного POC:** Rust-реализация Okkama, `2f669ba7`.

**Обновление:** выбраны явные типы результата и обычные вызовы (§4.5–4.7); директивы и проекции исключены из целевой модели. JSON API в рабочей ветке уже перенесён в `std/json/`; старые примеры Json/get/optional ниже относятся к исходной базе POC. Это иллюстрации семантики, не инструкция миграции нынешнего JsonView. Новый контракт проверяется отдельным адаптером.

Основная модель enum/payload, Maybe/Presence, match и fallback реализована. JSON/SQL API ниже остаются предложениями отдельной миграции. План этапов сохранён как история проектирования; реализация использует существующие инструкции объектов и массивов, без смены формата SPB. Неактивный payload хранится пустым типизированным слотом, активный — одноэлементным; извлечение проверяет тег. Это обеспечивает совместимость с владением, GC и сравнением контейнеров, но пока добавляет аллокации. Worker-передача вариантов, рекурсивные ADT и generic bounds enum явно не поддерживаются.

## 1. Что хотим получить

Описывать несколько допустимых форм значения одним типом, хранить данные каждой формы и безопасно извлекать их через `match`.

```okkama
enum UserInput = (
    name(value is string),
    details(value is UserDetails)
);

let user = UserInput.name("Anton");

let name = match user {
    case UserInput.name(value) { value; }
    case UserInput.details(details) { details.name(); }
};
```

Тем же механизмом выразить отсутствие значения:

```okkama
enum Maybe<T> = (
    some(value is T),
    none
);
```

И отсутствие поля отдельно от явно пустого значения:

```okkama
enum Presence<T> = (
    some(value is T),
    none,
    missing
);
```

Дать короткую запись при создании объектов:

```okkama
let user = User(
    name: json.maybe_text("name") else "anonymous",
    postcode: json.maybe_int("postcode") else 0,
);
```

При этом неправильный JSON-тип, ошибка SQL и сбой операции не должны незаметно превращаться в отсутствие или значение по умолчанию.

## 2. Основные решения POC

| Вопрос | Предложение |
|---|---|
| Общий механизм | Закрытый номинальный enum, варианты которого могут иметь типизированные поля |
| Написание вариантов | Сохраняем существующий lower-case стиль: `Maybe.some`, `Maybe.none` |
| Извлечение данных | Pattern matching с проверкой полноты и типов |
| Отсутствие значения | `Maybe<T>`: `some(T)` / `none` |
| Отсутствие поля | `Presence<T>`: `some(T)` / `none` / `missing` |
| Ошибки | Существующий `throw`; не вариант `Maybe`/`Presence` |
| Fallback | Ленивый `Maybe<T> else T`; прежний индексный fallback сохраняет смысл |
| `Presence` → `Maybe` | Явное преобразование с потерей различия `missing`/`none` |
| `maybe T` | Возможный будущий сахар для `Maybe<T>`; не нужен первому прототипу |
| `maybe expression` | Не вводим в POC; не перехватываем произвольные ошибки |
| Контракт действия | Явный результат T, Maybe<T> или Presence<T>; тело соответствует сигнатуре |
| Вызов | Возвращает объявленный тип, без автоматической распаковки |
| Директивы и проекции | Убираем синтаксис директив `#`, включая `#projected`, и проекционные вызовы |
| Селекторы `?` / `??` | Не вводим; для обработки результата используются match, else и явные преобразования |
| Внутренний `Undef` | Не является пользовательским `none` |
| Произвольные union types, structural subtyping | Вне POC |

**Рекомендация:** сначала проверить общий механизм tagged variants, затем его применение к отсутствию. Не делать `Maybe`, `Presence`, JSON и SQL четырьмя разными системами специальных значений.

## 3. Состояния данных: что различается

Для поля, которое должно содержать строку:

| JSON | `Presence<string>` | `Maybe<string>` при явной политике missing/null → none |
|---|---|---|
| `{}` | `Presence.missing` | `Maybe.none` |
| `{"name": null}` | `Presence.none` | `Maybe.none` |
| `{"name": ""}` | `Presence.some("")` | `Maybe.some("")` |
| `{"name": "Anton"}` | `Presence.some("Anton")` | `Maybe.some("Anton")` |
| `{"name": 42}` | Ошибка типа | Ошибка типа |
| Невалидный JSON-текст | Ошибка парсинга | Ошибка парсинга |

Внешний JSON `null` — данные формата. В доменной модели `Maybe.none` означает отсутствие значения; этот вариант не является универсальным null, допустимым в любом типе.

Нормальные значения не превращаются в отсутствие:

```text
some("")    != none
some(0)     != none
some(false) != none
some([])    != none
```

Пустая строка может быть запрещена доменной валидацией имени, но это отдельное правило. `Maybe<string>` сам по себе не означает «непустая строка».

### 3.1. Формула Presence = Maybe | Missing

Идея пользователя:

```text
Maybe<T>    = Some(T) | None
Presence<T> = Maybe<T> | Missing
```

**Семантически принимается:** Presence имеет состояния Some/None/Missing. Но для реализации важно различать два решения:

1. **Закрытый номинальный enum** с тремя вариантами. Это основной вариант POC: тот же пользовательский набор состояний, минимум новых правил системы типов.
2. **Настоящее объединение типов** `Maybe<T> | Missing`. Оно дополнительно требует правил subtype/coercion, выбора вариантов, вложенных union, inference и совместимости patterns. Это отдельное расширение, не скрытая часть данного POC.

В основном варианте `Maybe.some(1)` и `Presence.some(1)` принадлежат разным типам. Присваивание между ними не становится допустимым только из-за совпадения имён вариантов.

Переходы:

```text
Maybe.some(v) → Presence.some(v)
Maybe.none    → Presence.none

Presence.some(v) → Maybe.some(v)
Presence.none    → Maybe.none
Presence.missing → Maybe.none
```

Первое преобразование не теряет информацию. Второе намеренно теряет различие между явно пустым значением и отсутствующим полем. Предлагаемые стандартные операции: `as_presence()` и `fold()`. В POC они могут быть узкими, определёнными для этих двух стандартных типов операциями; общий механизм методов enum — отдельный вопрос.

**Не вводить неявный fold.** Потеря различия разрешена через явное `.fold()`. Обычное присваивание и fallback этого не делают.

## 4. Декларации и конструирование

### 4.1. Сохраняем существующую форму enum

Сегодня:

```okkama
enum Status = (ready, running, finished);
```

Расширение:

```okkama
enum TaskState = (
    waiting,
    running(progress is int),
    finished(result is string),
    failed(message is string)
);
```

Варианты без payload сохраняют старую запись:

```okkama
TaskState.waiting;
TaskState.running(45);
TaskState.finished("report.pdf");
TaskState.failed("Disk is full");
```

Вызов конструктора варианта не запускает пользовательское тело. Он проверяет типы и создаёт immutable значение. Валидация диапазона progress и доменных инвариантов находится в обычной фабрике/действии.

### 4.2. Правила

- Enum — закрытый набор вариантов. Расширение из другого модуля запрещено.
- Enum номинален: одинаковые поля не делают два enum одним типом.
- Имена вариантов уникальны внутри enum; имена payload-полей уникальны внутри варианта.
- Вариант содержит ноль или более типизированных полей. `void` не разрешён как поле.
- Вариант без полей записывается `Type.case`; вариант с полями — `Type.case(...)`.
- Для первой реализации payload-конструкторы позиционные. Поддержку named arguments можно добавить позже по обычным правилам вызовов Okkama.
- Аргументы вычисляются один раз слева направо. Отказ вычисления аргумента распространяется; частично созданное значение не становится доступным.
- Прямого универсального `.value()` у enum нет: у разных вариантов разные поля. Основной безопасный доступ — через `match`.
- В первом POC нет пользовательских методов, `destroy`, mutable fields и наследования у enum. Payload может содержать объекты по обычным правилам их lifetime.

### 4.3. Generic enum

```okkama
enum Maybe<T> = (some(value is T), none);

let count = Maybe.some(42);                // Maybe<int>
let title is Maybe<string> = Maybe.none;   // тип задан контекстом
let absent = Maybe<int>.none;             // явная специализация
```

Последняя форма требует расширить разрешение qualified enum references на generic специализации. В отсутствие и payload, и ожидаемого типа:

```okkama
let value = Maybe.none;
```

компилятор должен сообщить, что `T` не выводится. Никакого автоматического `Maybe<any>`.

Для POC generic enum инвариантны: `Maybe<A>` нельзя неявно присвоить `Maybe<B>`, даже если между A и B есть преобразование/role relation. Явное отображение содержимого можно добавить библиотечным API позже.

### 4.4. Вложенность

```text
Maybe<Maybe<int>>
    none
    some(none)
    some(some(42))
```

Это три различимых состояния. Автоматического flatten нет. Это важно для композиции lookup и optional значения в контейнере.

Рекурсивные enum, например список с собственным enum в payload, **не входят в первую фазу**. Компилятор должен диагностировать прямую или косвенную рекурсию в описании типов, включая generic expansions, а не бесконечно специализировать их. Поддержка рекурсивных сумм — следующий отдельный эксперимент с явным storage/lifetime контрактом.

### 4.5. Явный тип результата — полный контракт действия

```okkama
object Data {
    Data();

    value() is Maybe<string> {
        Maybe.some("Anton");
    }
}

let optional = Data().value();          // Maybe<string>
let name = Data().value() else "anonymous"; // string
```

Тип результата в сигнатуре описывает весь возвращаемый результат. Если указан `Maybe<string>`, тело возвращает `Maybe<string>` и вызов имеет тот же тип. Голая строка вместо обёртки — ошибка типа. Метод с результатом `string` не может вернуть `Maybe.none`.

| Объявленный результат | Результат обычного вызова |
|---|---|
| T | T |
| Maybe<T> | Maybe<T> |
| Presence<T> | Presence<T> |
| Maybe<Maybe<T>> | Maybe<Maybe<T>> |

Нет автоматического wrap, unwrap, fold или flatten. Вызывающий код обрабатывает варианты через match, применяет `else` к Maybe или явно преобразует Presence в Maybe. Ошибки выполнения распространяются по обычным правилам и не превращаются в отсутствие.

### 4.6. Убираем директивы и проекции

Целевая модель не содержит синтаксиса директив `#`: ни `#maybe`, ни `#presence`, ни существующего `#projected`. Убираем также вызовы проекций вроде `value Data()`: используется обычный `Data().value()`. Если у проекции был алиас `value`, а действие называлось `get`, замена — `Data().get()`.

Селекторы `.?value()` / `.??value()`, префиксы `?value` / `??value` и альтернативный селектор `.!value()` не вводим. Отдельного режима strict-вызова нет: строгая функция явно возвращает T и сама определяет ошибки своего контракта.

Удаление уже реализованной поддержки проекций — отдельная задача реализации: найти декларации и применения, заменить вызовы, удалить обработку директив и проекций из parser/AST/analysis/lowering, обновить spec, grammar, тесты и tooling. При замене сохранить однократное вычисление аргументов и порядок: конструктор и confirm завершаются до вызова метода. Примеры старого синтаксиса должны давать понятную диагностику. Implementation status: projection support has now been removed from parser, AST, analysis and lowering; stdlib and positive conformance examples use ordinary calls.

### 4.7. Статический анализ

Компилятор проверяет тело относительно объявленного результата, а тип вызова получает из сигнатуры с обычной подстановкой generic-параметров. Места вызова не изменяют контракт метода. Ожидаемый тип может участвовать в обычном generic inference, но не переключает метод между T, Maybe<T> и Presence<T>.

Отдельные поля `absence` и `call mode` в AST не нужны. Maybe/Presence проходят как номинальные generic enum; специальная семантика требуется для optional fallback (§6), а не для вызова. Role-контракты и ссылки на действия должны сохранять полный тип результата. Для async действуют существующие правила async-вызова; дополнительной автоматической распаковки Maybe/Presence не появляется.

## 5. Pattern matching

```okkama
let text = match state {
    case TaskState.waiting { "Waiting"; }
    case TaskState.running(progress) { progress.string(); }
    case TaskState.finished(result) { result; }
    case TaskState.failed(message) { message; }
};
```

### 5.1. Минимальные patterns

Для первого POC:

```text
Type.zero_payload_variant
Type.payload_variant(binding, ...)
Type.payload_variant(_, ...)
```

- Payload bindings доступны только в своей ветке.
- Тип binding определяется полем варианта; аннотация не обязательна.
- `_` игнорирует поле, но не отменяет корректное освобождение payload.
- Scrutinee вычисляется ровно один раз.
- В pattern `Type.variant(...)` не выполняется обычный вызов конструктора: это отдельный узел AST pattern.
- Одинаковое binding-имя внутри одного pattern — ошибка.
- Число bindings должно соответствовать payload-полям.
- Несколько альтернатив с извлечением данных в одной ветке, guards, literal/range subpatterns и вложенная деструктуризация отложены. Для вложенного enum используется второй `match`.
- Существующие expression cases обычных match сохраняются; новую pattern-семантику вводим явно для enum, не переосмысляя произвольные вызовы в старых case.

### 5.2. Полнота и тип результата

Для enum с payload компилятор требует все варианты либо `else`:

```okkama
match value {
    case Maybe.some(found) { use(found); }
    // Ошибка: отсутствует ветка Maybe.none.
}
```

Повтор варианта, вариант другого enum и неверная арность pattern — ошибки. Для уже исчерпывающего списка последующий `else` можно диагностировать как unreachable; это поведение следует зафиксировать тестом.

В expression-match все нормально завершающиеся ветви должны давать совместимый результат по текущим правилам Okkama. Не выводить автоматически новый union из разных типов ветвей. `throw` и `panic` не обязаны давать значение результирующего типа.

Добавление варианта в публичный enum делает старые exhaustive matches неполными при перекомпиляции. Это изменение публичного контракта, а не автоматически совместимое расширение.

Для старых payload-free enum не ужесточать все существующие statement matches незаметно: сначала сохранить их текущие правила. Унификация строгой полноты для всех enum требует отдельной проверки совместимости.

### 5.3. Владение

Сопоставление не должно наблюдаемо уничтожать исходный immutable enum:

```okkama
let original = Maybe.some("Anton");
let name = match original {
    case Maybe.some(value) { value; }
    case Maybe.none { "anonymous"; }
};
// original остаётся корректным значением.
```

Реализация может переместить payload вместо retain/copy только при доказанном последнем использовании, включая исключительные пути. Учитывать найденную в аудите проблему liveness на exceptional edges; не строить безопасность новой фичи на этом небезопасном предположении.

## 6. Fallback: Maybe<T> else T

```okkama
let name = candidate else "anonymous";
let name = candidate else load_default_name();
let name = candidate else throw MissingName();
let name = candidate else panic "Name must be present";
```

`MissingName` здесь — пример прикладного Error, который определяет приложение.

Контракт:

1. Вычислить candidate один раз.
2. Для `Maybe.some(value)` вернуть value, не вычисляя правую сторону.
3. Для `Maybe.none` вычислить правую сторону.
4. Правая сторона должна дать T либо завершиться через `throw`/`panic`.
5. Ошибка вычисления candidate распространяется. `else` не является `catch`.
6. `some(0)`, `some(false)`, `some("")`, `some([])` не включают fallback.

В POC стандартный `Maybe` имеет известную компилятору идентичность. Любой пользовательский enum с вариантами `some`/`none` автоматически optional не становится. Общий протокол coalescing пока не вводится.

### 6.1. Приоритет и неоднозначность

- Optional fallback слабее обычных арифметических операций и postfix calls/member access.
- В constructor argument fallback заканчивается на разделяющей запятой аргументов; запятые внутри вложенных вызовов остаются частью этих вызовов.
- Цепочка `a else b else c` в первом POC требует скобок: это позволяет явно выбрать, какой уровень Maybe разворачивается.
- `if ... { ... } else { ... }` сохраняет своё существующее правило; optional fallback поверх целого `if` требует скобок.
- Многооператорный fallback-блок, `return`/`break` как выражения и другие control-flow расширения не входят в POC.

### 6.2. Существующий индексный else

```okkama
values[index] else fallback;
map[key] else fallback;
```

Остаются lookup fallback: реагируют только на отсутствие элемента/ключа.

Если map хранит `Maybe<int>`, успешный lookup возвращает `Maybe<int>`, включая `Maybe.none`. Поэтому:

```okkama
// map имеет тип %string:Maybe<int>
let value = map["age"] else Maybe<int>.none;
let age = value else 0;
```

Первый `else` обрабатывает отсутствие ключа, второй — отсутствие значения. Нельзя автоматически сделать первый оператор одновременно двумя операторами и стереть уровень вложенности.

Для непроиндексированного `candidate else fallback` вводится отдельный AST/THIR узел coalesce. Старый `Index { fallback }` не должен молча сменить семантику. Отсутствие lookup также не реализуется через catch runtime trap.

### 6.3. Presence без скрытой потери информации

В первом POC `Presence<T> else T` — ошибка типа. Разработчик выбирает политику:

```okkama
let name = json.presence_text("name").fold() else "anonymous";
```

или обрабатывает три состояния отдельно. Для PATCH сохраняем Presence до обработки, чтобы различать missing и null. Метод `maybe_text(name) is Maybe<string>` может быть обычной библиотечной обёрткой над `presence_text(name).fold()`; имя не имеет специального значения для компилятора.

## 7. JSON: несколько форм одного значения

Вход:

```json
{"user": "Anton"}
```

```json
{"user": ""}
```

```json
{"user": null}
```

```json
{"user": {"name": "Anton"}}
```

Модель:

```okkama
object UserDetails {
    has name is string;
    UserDetails(:name);
    name() is string { name; }
}

enum UserInput = (
    name(value is string),
    details(value is UserDetails)
);
```

Для всего поля используем `Presence<UserInput>`:

| Вход | Результат |
|---|---|
| `{}` | `Presence<UserInput>.missing` |
| `{"user": null}` | `Presence<UserInput>.none` |
| `{"user": ""}` | `Presence.some(UserInput.name(""))` |
| `{"user": "Anton"}` | `Presence.some(UserInput.name("Anton"))` |
| `{"user": {"name": "Anton"}}` | `Presence.some(UserInput.details(UserDetails("Anton")))` |
| `{"user": 42}` | Ошибка допустимой формы |
| `{"user": {"name": 42}}` | Ошибка типа по пути `/user/name` |
| `{"user": {}}` | Ошибка отсутствующего обязательного `/user/name` |

Преобразование JSON → enum выполняет декодер, а не общий механизм enum. Пример тела декодера непустого значения:

```okkama
match node.kind() {
    case JsonKind.string_value {
        UserInput.name(node.text());
    }
    case JsonKind.object_value {
        UserInput.details(UserDetails(node.text("name")));
    }
    else {
        throw InvalidUserInput(node.pointer());
    }
}
```

`InvalidUserInput` — прикладной Error. Отсутствие поля и JSON null обрабатываются внешним reader до этого декодера. Ошибку внутри объекта не нужно ловить и пробовать строковую форму: форма уже определена видом JSON-узла.

### 7.1. Сохранить форму или нормализовать

Если строка означает поиск существующего пользователя, а объект — создание нового, сохраняем `UserInput` и различаем варианты.

Если обе формы — только два способа записать одно имя, декодер нормализует их в `UserDetails`; приложение получает `Presence<UserDetails>`. Не заставлять доменный код помнить транспортную форму, если она ему не нужна.

### 7.2. Контракт предлагаемых JSON API

| Действие | Результат | Отсутствие/ошибки |
|---|---|---|
| `get(name)` | `Json` | Текущий строгий контракт сохраняется |
| `find(name)` | `Maybe<Json>` | Missing → none; явный JSON null остаётся внутри some(Json) |
| `text(name)` | `string` | Строгий контракт, ошибки задаёт реализация метода |
| `presence_text(name)` | `Presence<string>` | Missing/null/string различаются; другой тип — ошибка |
| `maybe_text(name)` | `Maybe<string>` | Явно объединяет missing и null; другой тип — ошибка |
| `maybe_int(name)` | `Maybe<int>` | Та же политика, с существующими строгими правилами int |
| Схемный reader UserInput | `Presence<UserInput>` | Принимает только объявленные формы |

Таблица описывает предлагаемый API экспериментального адаптера. Каждый метод явно объявляет полный тип результата; названия maybe_text/presence_text — библиотечная договорённость, не механизм языка. Методы могут делегировать общему reader, чтобы не дублировать чтение и проверку типов. Текущий JsonView автоматически не меняем. У Maybe.none нет причины отсутствия, а у Presence нет JSON pointer: строгий reader должен сохранять необходимый контекст для своих ошибок до такого преобразования.

`find` на не-объекте не должен считать ошибку формы отсутствующим полем: сохранить `WrongJsonKind`.

В общем случае параметры именованных JSON readers расширяются и на boolean/number/list, но не нужно создавать все методы для первого вертикального прототипа. Достаточно text/int и одного прикладного reader UserInput.

Текущий `Json.optional()` объединяет missing/null и возвращает специализированный `JsonOptional`. Его не удалять и не менять семантику в POC. Новые API добавляются рядом, миграция проводится отдельно.

## 8. SQL, ошибки и внутренняя незаполненность

```text
Пользователь не найден запросом  → Maybe<User>.none
Найден пользователь            → Maybe.some(user)
SQL NULL в типизированной ячейке → Maybe<T>.none
Значение 0 / false / ""         → Maybe.some(value)
База недоступна                → throw соответствующий Error
Неверное имя колонки           → throw MissingColumn
```

Строка результата SQL с известной схемой обычно не требует Presence для каждого столбца: NULL — нормальное состояние, отсутствующий столбец — ошибка контракта. Если API специально работает с частичной проекцией, он может явно возвращать Presence.

`DbValue` как представление динамического SQL-типа остаётся нужен. Maybe убирает специализированные nullable-обёртки на типизированных границах, но не заменяет discriminant между integer/text/blob у динамических данных.

`Result<T, E>` также можно выразить вариантным типом, однако переход всего языка с throw на Result **не входит** в эту фичу. `Result<Maybe<T>, E>` нужен только там, где ошибки сознательно хранятся/передаются как значения.

Внутренний `Undef` означает, среди прочего, незаписанный регистр или несвязанное поле. Его нельзя присвоить Maybe как none. Иначе use-before-initialization сможет выглядеть как нормальное отсутствие и включать fallback вместо диагностики.

## 9. Равенство, форматирование и сериализация

- Сначала сравниваются номинальная декларация, generic специализация и вариант.
- Разные варианты одного enum не равны.
- Одинаковые payload-free варианты равны.
- Равенство payload-вариантов выполняется по обычным правилам равенства соответствующих полей. Не вводить новую семантику equality объектов ради enum.
- Итеративное сравнение/освобождение и обработка разделяемых ссылок должны распространяться на payload.
- Новые payload enum в POC нельзя использовать ключами map. Старые payload-free enum сохраняют текущую поддержку ключей.
- Для `.string()` новых payload enum не объявлять неявный стабильный wire format. На первом этапе — явный match или debug formatter без гарантии сериализации.
- JSON не обязан содержать тег `name`/`details`: внешнюю форму задаёт adapter.
- При обратной записи Presence.missing означает **пропустить поле объекта**, а Presence.none — записать JSON null. Для элемента массива «пропустить поле» не имеет того же смысла; требуется отдельная политика, нельзя молча удалить позицию.
- `fold()` необратим: после него serializer не может восстановить, был Missing или null.
- Если понадобится worker-передача enum, её формат должен сохранять номинальную идентичность/специализацию и payload. До реализации такой поддержки операция должна отвергать тип явно.

## 10. Синтаксическая дельта и диагностика

Ориентир расширения текущей грамматики, не полная новая EBNF:

```text
enum_decl    := "enum" TypeName generic_params? "=" "(" variants ")" ";"
variant      := name | name "(" typed_fields ")"
typed_field  := name "is" type
pattern      := EnumRef "." variant
             | EnumRef "." variant "(" bindings ")"
binding      := identifier | "_"
coalesce     := expression "else" (expression | throw_form | panic_form)
```

Продукция coalesce нуждается в приоритетах и разграничениях из §6: нельзя просто добавить её в рекурсивный expression parser и считать синтаксис определённым.

Сигнатуры используют обычный `is type`, включая generic enum. Новая грамматика режимов вызова не нужна. Синтаксис директив и проекций удаляется; `.?value()`, `.??value()` и postfix `?` не входят в POC.

Ожидаемые диагностики:

```text
Cannot infer T for Maybe.none; provide an expected type or Maybe<T>.
Variant UserInput.name expects 1 payload field, got 2.
Variant UserInput.name expects string, got int.
Non-exhaustive match: missing UserInput.details.
Duplicate match case: Maybe.none.
Expected a Maybe<T> operand for optional fallback, found Presence<string>.
Fallback must produce int, found string.
Recursive variant payload types are not supported by this POC.
Payload enum UserInput cannot be used as a map key.
Expected Maybe<string> as the action result, found string.
Expected string as the action result, found Maybe<string>.
Action directives are no longer supported; use an explicit result type.
```

Тексты иллюстративны; реальные diagnostics должны использовать обычные spans и стиль compiler diagnostics. Данные JSON должны сохранять pointer/path ошибки.

## 11. План сквозного прототипа

### Этап A. Enum с payload, один backend

1. AST декларации variant fields и отдельные pattern nodes.
2. Анализ номинальных типов, generic enum, inference и полей.
3. Проверка полноты match и типов ветвей.
4. OIR с явной конструкцией, проверкой тега и извлечением payload.
5. IR interpreter: создание, matching, equality и lifetime.
6. Набор семантических тестов, независимый от JSON и SQL.

Временный milestone может поддерживать только IR. Остальные backends обязаны сообщать unsupported, а не генерировать некорректный код. Такой milestone ещё не означает готовность фичи для релиза.

### Этап B. Sparrow и безопасный JIT fallback

- Расширить SPB type/enum schema с типами полей каждого варианта.
- Определить инструкции конструкции, discriminant и payload extraction.
- Verifier проверяет declaration/variant/арность/типы полей и extraction. Для первого прототипа runtime-проверку тега перед extraction не убирать даже при статической верификации.
- При изменении бинарного формата выпустить новую версию SPB; старые executables должны отвергать новую версию явно. Не менять молча значение SPB 9.
- Interpreter реализует новые операции. JIT сначала отказывается компилировать функции с неподдерживаемыми операциями либо использует проверенный helper; fast path добавляется после correctness-тестов.
- Нельзя интерпретировать payload enum как старый scalar enum, который хранит только declaration/case: это сломает владение payload.

### Этап C. Maybe/Presence и fallback

- Стандартные декларации Maybe/Presence, известная компилятору идентичность Maybe.
- Явные преобразования as_presence/fold; их точная API-форма утверждается до реализации.
- `Maybe else` с ленивой веткой без catch.
- Явные generic-типы результата, проверка тела и сохранение типа при обычном вызове.
- Удаление директив и проекций по §4.6, включая миграцию применений и tooling.
- Экспериментальный JSON adapter с явно типизированными readers и прикладной UserInput reader.
- Проверка сохранения JSON path и строгих ошибок в библиотечных adapters.
- SQL adapter для одного nullable типа, без переделки всего db API.

### Этап D. Паритет и критерий готовности

- JVM представление и codegen, совместимые с выбранной семантикой.
- IR/Sparrow off/eager/OSR/JVM differential, проверка ошибок и side effects.
- Worker/FFI либо поддержаны, либо имеют явные ограничения; язык не должен обещать переносимость там, где сериализация ещё не реализована.
- Обновить spec, grammar, conformance, std docs, syntax highlighting и LSP diagnostics.

## 12. Представление и стоимость

Первый прототип должен быть корректным, измеримым и простым для проверки.

- Сохранить существующее представление старого payload-free enum, если это не нарушает общий контракт.
- Для payload допустим tagged reference на immutable storage с явно описанным ownership. Это возможная отправная точка, **не решение о финальном ABI**.
- У Maybe<reference> в некоторых языках отсутствие кодируется свободным pointer value, но для общего Okkama Value и всех T такая оптимизация не гарантирована. Не обещать zero-cost без измерений.
- Не увеличивать размер каждого Value во всей VM случайно ради редких payload enum. Сначала сравнить варианты раскладки и стоимость clone/drop.
- Не кодировать None/Missing числовым sentinel, пустой строкой, пустым массивом или Undef.
- Representation должна сохранять `some(none)` отдельно от `none`, а generic instantiations — от разных номинальных enum.
- Проверить payload с массивом, объектом, замыканием и разделяемым графом. Новая ссылка должна участвовать в tracing/GC и итеративном освобождении по общим правилам runtime.
- JIT не должен читать payload или оптимизировать освобождение только по заявленному типу непроверенного байткода.

Замеры после correctness: создание/сопоставление миллионов some/none, JSON decode с разной долей missing/null, удержание payload после match, allocation counts, peak/retained memory, size_of<Value>, off/JIT и стоимость generic specializations. Сравнение — с нынешним JsonOptional, а не только с искусственным циклом по тэгам.

## 13. Матрица приёмки

| Группа | Обязательные случаи |
|---|---|
| Декларации | Старый enum без payload; несколько payload-типов; duplicate variant/field; неправильная арность |
| Сигнатуры | Тело соответствует полному результату; T вместо Maybe<T> и обратно отвергаются |
| Вызовы | Тип результата сохраняется; добавление потребителя не меняет контракт; receiver/arguments/body вычисляются один раз |
| Обёртки | Нет unwrap/wrap/fold/flatten; nested Maybe сохраняется; throw/panic не становятся none |
| Удалённый синтаксис | Директивы и проекции отвергаются; их применения заменены обычными вызовами; порядок вычисления сохранён |
| Generic | Вывод из some; none с ожидаемым типом; явный Maybe<int>.none; ошибка без контекста; nested Maybe |
| Match | Полный; пропущенный вариант; duplicate; неверный enum; неверное число bindings; область видимости; `_` |
| Тип результата | Совместимые ветви; несовместимые ветви; throw/panic branch; без неявного any/union |
| Fallback | none включает RHS; some не включает; left ровно один раз; RHS side effect; ошибка слева не подавлена |
| Значения | some(0), some(false), some(""), some(empty array) сохраняются |
| Индекс | Missing key и присутствующий Maybe.none различимы; два уровня fallback; прежнее поведение не изменено |
| Presence | missing/none/some различимы; fold явный; as_presence не теряет значение; PATCH semantics |
| JSON | Все строки таблиц §3/§7; неправильный root; неверный nested type; обязательный missing name; сохранение pointer |
| SQL | Нет строки; SQL NULL; 0/false/""; missing column; отказ базы |
| Lifetime | Исходное значение доступно после match; payload переживает ветку; нет double drop/leak; shared DAG |
| Байткод | Неверный variant id/type/arity; extraction чужого варианта; старый/new version; ресурсы ограничены |
| Backend | IR и Sparrow сначала, затем JVM; JIT off/eager/threshold 2/10; сопоставление результатов и отказов |
| Совместимость | Старые enum, map keys, case expressions, index else, Json.optional и generic type syntax |

Минимальный демонстрационный сценарий: прочитать пять форм user (missing, null, пустая строка, строка, объект), получить Presence<UserInput>, обработать все варианты, нормализовать в пользовательское имя и показать lazy fallback отдельно. Отрицательные случаи должны давать ошибки, а не значения по умолчанию.

## 14. Открытые решения до изменения реализации

1. Утвердить основную декларацию payload enum и positional pattern syntax.
2. Утвердить основной вариант Presence: номинальный трёхвариантный enum сейчас или более дорогая настоящая union-композиция отдельным POC. Рекомендация — первый.
3. Утвердить точные имена стандартных вариантов (`some/none/missing`) и явных преобразований.
4. Утвердить generic qualification `Maybe<int>.none` и правила inference.
5. Выбрать storage для первого runtime-прототипа и записать lifetime/GC-инварианты до JIT-оптимизации.
6. Решить, когда ужесточать exhaustiveness старых enum matches: отдельно от вводимой проверки новых payload enum.
7. Выбрать имена и границы библиотечных readers; проверить сохранение строгих ошибок и JSON path без восстановления контекста из none.

Выбрано для POC: полный тип результата явно указан в сигнатуре и сохраняется при вызове. Синтаксис директив, включая #projected, и проекции удаляем. Контракт не выводится из мест вызова; специальных режимов вызова нет.

Отложено: `maybe` как keyword-модификатор типа, `maybe expression`, optional chaining, инфиксный `a ?? b`, arbitrary unions, automatic flatten, recursive ADT, pattern guards, общий custom fallback protocol, автоматические derived codecs.

## 15. Связь с текущим кодом и источники

В исходной базе POC Json.get возвращал Json и бросал MissingJsonValue, а optional объединял missing/null. В текущей рабочей ветке см. [Json](../../std/json.ok) и [JSON API](../std/json.md): get/name-accessors сохраняют WrongJsonKind/MissingJsonValue и JSON pointer; has позволяет проверить наличие поля. [DbValue](../../std/db/value.ok) — динамический SQL-value protocol; [Row](../../std/db/row.ok) различает missing column и SQL NULL.

Основные места будущих изменений:

- [Грамматика](../okkama-grammar-1.0.ebnf), [спецификация](../okkama-spec-11072026.md).
- [Parser declarations](../../okkama-rs/crates/okkama-compiler/src/syntax/parser/decl.rs), [parser expressions](../../okkama-rs/crates/okkama-compiler/src/syntax/parser/expr.rs), [parser types](../../okkama-rs/crates/okkama-compiler/src/syntax/parser/types.rs).
- [THIR](../../okkama-rs/crates/okkama-compiler/src/analysis/thir.rs), [OIR](../../okkama-rs/crates/okkama-compiler/src/ir/mod.rs), [lowering match](../../okkama-rs/crates/okkama-compiler/src/lowering/body/matching.rs), [index fallback](../../okkama-rs/crates/okkama-compiler/src/lowering/body/collections.rs).
- [SPB model](../../okkama-rs/crates/okkama-sparrow/src/lib.rs), [verifier](../../okkama-rs/crates/okkama-sparrow/src/verify.rs), [VM](../../okkama-rs/crates/sparrow-vm/src/machine.rs), [JIT](../../okkama-rs/crates/sparrow-vm/src/jit.rs).
- [Runtime values](../../okkama-rs/crates/okkama-runtime-core/src/value.rs), [heap](../../okkama-rs/crates/okkama-runtime-core/src/heap.rs), [GC](../../okkama-rs/crates/okkama-runtime-core/src/gc.rs).

Внешние ориентиры, использованные в обсуждении:

- [Rust Option](https://doc.rust-lang.org/std/option/): Some(T)/None и явное извлечение значения.
- [Serde double_option](https://docs.rs/serde_with/latest/serde_with/rust/double_option/index.html): различение missing, null и value при JSON-сериализации через два уровня Option.
- [Elm JSON decoder](https://github.com/elm/json/blob/master/src/Json/Decode.elm): nullable сохраняет ошибку неправильного типа; maybe decoder допускает более широкое превращение ошибок декодирования в отсутствие. Для строгих readers Okkama выбран первый принцип.
- [Zig optionals](https://ziglang.org/documentation/master/#Optional-Type): typed optional и отдельный fallback; обработка ошибок — отдельный механизм.

Эти источники подтверждают общие модели, но конкретный синтаксис, выбор номинального Presence, правила совместимости и этапы реализации выше — предложения для Okkama.

## 16. Проверка первой реализации

Рабочий пример исходной JSON-задачи: [examples/variants.ok](../../examples/variants.ok).
Запуск: `okkama run examples/variants.ok`. Он различает missing, null, пустую строку,
имя строкой и объект с именем; fallback применяется явно.

Проверки в ветке `feature/maybe.poc`, 2 ноября 2026:

| Проверка | Результат |
|---|---|
| `cargo test --release --workspace --no-fail-fast` | 526 тестов, 0 ошибок, 0 пропусков |
| Conformance `portable-core`, IR | 725/725 PASS |
| Conformance `portable-stdlib`, IR | 61/61 PASS |
| 12 новых conformance-кейсов + миграция invoice | 13/13 PASS отдельно на IR, Sparrow и JVM |
| 12 новых кейсов, Sparrow `OKKAMA_JIT=eager` | 12/12 PASS |
| `ocs validate` | Метаданные 799 кейсов валидны |

Проверены generic inference, payload разных типов, вложенные обёртки, сравнение в
коллекциях, границы ролей и модулей, ленивость fallback, распространение ошибок,
exhaustiveness и время жизни ресурса с `destroy`. Полная conformance release gate
не заявляется: у общего каталога остаются прежние незакрытые coverage obligations.

## 17. Реализованный компактный выбор

`condition ?? yes !! no` требует bool и две совместимые по типу ветки.
`value ?? .some(v): v !! .none: "anonymous"` — короткий exhaustive match.
Для Presence добавляется `.missing:`; последний `_:` покрывает остальные варианты.
Субъект вычисляется один раз, результаты веток ленивые. Вложенность этих
операторов запрещена в любом операнде, включая скобки и аргументы вызовов.
Используйте отдельные переменные или обычный match. `fold()` заменил `collapse()`.
Подробности и ограничения: [книга, глава 24](../OkkamaBook-RU.md#глава-24-перечисления),
[пример](../../examples/choices.ok).

Дополнительно: у последнего выражения перед `}` точка с запятой необязательна;
`if true { 1 } else { 0 };` допустим. Для `let`, `return`, `throw`, `defer`
и присваиваний правила завершения не меняются. Форматтер сохраняет написание.

Проверки расширения (2 ноября 2026):

| Проверка | Результат |
|---|---|
| Полный release workspace до добавления необязательной `;` | 531 PASS, 0 ошибок |
| Финальный workspace (`RUST_MIN_STACK=16777216 cargo test --workspace --no-fail-fast`) | 532 PASS, 0 ошибок, 0 пропусков |
| Финальная `cargo build --release --workspace` | PASS |
| Runtime-тесты вариантов, компактного выбора и хвостовых выражений | 18 PASS, IR и Sparrow |
| Conformance portable-core после изменения `;` | 740/740 PASS |
| Conformance portable-stdlib после компактного выбора | 61/61 PASS |
| 15 новых conformance-кейсов | 15/15 PASS отдельно на IR, JVM и Sparrow с eager JIT |
| VS Code: правила грамматики / TextMate-токенизация | 226/226 и 49/49 PASS |
| `ocs validate` | 814 кейсов, 726 контрактов, validation OK |
