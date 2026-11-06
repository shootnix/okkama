# OASM 1.0
## Ассемблер и переносимый байткод виртуальной машины Okkama

**Редакция:** 13 сентября 2026. **Форматы:** `.oasm` и `.obc`.
**Статус:** нормативный проект реализации VM; не утверждение, что такая VM уже существует.
**Профиль языка:** `okkama-1`. **Предшественник:** архитектурный проект OASM 0.1.

OASM — линейная запись инструкций и дескрипторов Okkama. Это не второй
прикладной язык: здесь нет выражений с операторами, `if`, `for`, pattern matching,
лямбда-литералов, макросов, вычислений на этапе сборки или отдельной stdlib.
Есть константы, регистры, переходы, вызовы и метаданные, необходимые runtime.

**Минимум исполнения: 43 инструкции core.** Ещё две инструкции доступны только
в явно включённом тестовом режиме. Арифметические операции имеют закрытые
подкоды; стандартные поведения значений используют обычный `call`.

## 0. Основание и границы спецификации

Ссылки `[B §…]` означают разделы приложенного `OkkamaBook.md`, редакция
19 July 2026 с последующими дополнениями. `[D §…]` — предыдущий проект
`okkama-vm-design/README.md` 0.1. Хеши исходных файлов включены в `SOURCES.md`.

Приоритет для настоящего документа:

1. Уточнение автора языка: `defer` выполняется на любом управляемом выходе.
2. Подробные разделы книги о соответствующей возможности.
3. Явно перечисленные ниже решения **профиля VM**, закрывающие пробелы книги.
4. Предыдущий архитектурный проект, если он не противоречит настоящему документу.

OASM не переопределяет синтаксис `.ok`. Совместимость с действующим Perl-
интерпретатором должна быть подтверждена дифференциальными тестами; его
репозиторий и тесты при подготовке этой спецификации не предоставлены.

### 0.1 Зафиксированные решения, а не утверждения об интерпретаторе

| ID | Решение OASM 1.0 | Основание |
|---|---|---|
| D01 | `defer`: все управляемые выходы, LIFO, до `destroy` | Уточнение пользователя; B §15 |
| D02 | Async: lazy Future, suspension; `spawn`: отдельный процесс | B §31.5, а не устаревшая сводка §29 |
| D03 | `int`: signed 64-bit, checked overflow; конечный binary64 `float` | Portable numeric policy VM; книга не закрывает все края |
| D04 | Чтение после `succ.set` видит старые поля; последнее стадирование побеждает | Уточнение staging для VM |
| D05 | Successor проверяется через `fixup`; конструкторы/defaults повторно не выполняются | Новая явная фиксация VM, в книге прямого ответа нет |
| D06 | Неопубликованный construction-self не может покинуть construction/fixup | Новое ограничение безопасной публикации |
| D07 | Временные identities принадлежат текущему scope до экспорта/выхода | Закрытие неопределённого timing временных объектов в B §20.2 |
| D08 | Первичная ошибка тела важнее cleanup; иначе выигрывает первая ошибка defer, затем destroy | Уточнение полного порядка нескольких failures |
| D09 | Невзятый результат worker не отменяет работу; штатное завершение ждёт workers | Политика завершения VM; cancellation не вводится |
| D10 | Сбой worker переносится как нормализованный `NativeFailure`, не сериализует Error object | Закрытие process-boundary без пересылки объектов |
| D11 | Закрытые правила map keys, порядка обхода и недописанных builtin edges | Portable профиль, а не наследование поведения Perl |
| D12 | Async successor сохраняет must-use; transfer-профиль spawn имеет явный список категорий | Уточнение сочетаний возможностей, не обещание совместимости с текущими тестами |

D03–D12 — выбранный контракт нового backend, а не молчаливое исправление книги.
Перед включением его как drop-in замены интерпретатора эти решения надо
согласовать с языковыми compatibility-тестами. Внутри OASM 1.0 они однозначны.

Книга содержит также расхождения по alias imports, implicit self и старой
проекции `&`. OASM хранит уже разрешённые символы и явные result contracts;
эти исходные написания не входят в его грамматику. Подробные B §17.6 и §18.1
имеют приоритет над устаревшими сводками.

### 0.2 Термины обязательности

**ДОЛЖЕН** — требование совместимой реализации; **НЕЛЬЗЯ** — запрещённое
поведение; **МОЖЕТ** — допустимая реализационная свобода. Неизвестные инструкции
или обязательные возможности отвергаются, а не исполняются «как получится».

## 1. Модель модуля и исполнения

```text
Okkama .ok -> compiler ---------+
OASM .oasm -> assembler --------+-> Module -> verifier -> interpreter / JIT
OBC .obc   -> decoder ----------+
```

Сборка текста и загрузка бинарника создают одну и ту же модель `Module`.
Отдельного direct-text evaluator нет. JIT использует тот же semantic runtime.

Модуль содержит: identity и imports; типы; константы; описания объектов,
ролей и callable; тела кода; lexical scopes, slots и handlers; native imports.
Имена разрешаются до исполнения в локальные числовые индексы. В код не попадают
указатели хоста, Perl values, Rust layouts или platform-dependent integer sizes.

Модуль Okkama с несколькими исходными файлами собирается в один модуль OASM.
Внутри зависимостей сохраняются defining-module identities и ABI major.
Imported `main` не вызывается. Циклический граф модулей отвергается. [B §26]

### 1.1 Что остаётся компилятору Okkama

`if`/`match` становятся переходами; `for`/`loop` — переходами и scopes;
`next`/`last` — корректными выходами из scopes. Именованные проекции становятся
`new` + обычный `call`. Shorthand-конструкторы представлены метаданными.
`map/filter/reduce` становятся обходом, телом блока и immutable builder.

Нет инструкций `json.parse`, `http.get`, `sql.query`, `projection`, `map` или
`match`. Нет runtime-парсинга исходных Okkama expressions. Stdlib остаётся на
Okkama поверх объявленных native providers.

### 1.2 CLI-контракт инструментария

```text
oasm assemble input.oasm -o output.obc
oasm disassemble input.obc -o output.oasm
oasm verify input.obc
okkvm run input.obc
okkvm run input.oasm
okkvm test input.obc
```

Это имена проектируемых команд. `run .oasm` сначала собирает Module и выполняет
тот же verifier. `test` — полномочие запуска, а не привилегия, которую файл
может выдать сам себе. Код возврата: 0 — успех; 1 — программный failure или
провал тестов; 2 — ошибка инструмента/загрузки/верификации.

## 2. Текстовый синтаксис

### 2.1 Лексика

Текст — UTF-8 без BOM; допустимы LF и CRLF. EOF без завершающего newline
трактуется как конец последней строки. Пустые строки игнорируются.
Одна инструкция/директива занимает одну физическую строку. Переносов строк
внутри списков нет. `;` начинает комментарий до конца строки, кроме строковых
и символьных литералов. Отступы не значимы.

Имена: `[A-Za-z_][A-Za-z0-9_]*`. Регистр значим. Модульные сегменты соединяются
`/`; квалифицированный символ пишется `alias::Name`. Точка используется только
в именах инструкций и enum constants. Метка — `identifier:` на отдельной строке.
Имена `rN`, `pN`, `cN`, `bN`, `iN`, `s0`, `self`, `_` служебные в соответствующих
позициях. Нет универсальной таблицы ключевых слов для всех контекстов.

`"text"` — строка; `'x'` — ровно один Unicode scalar. Escapes:
`\\`, `\"`, `\'`, `\n`, `\r`, `\t`, `\u{HEX}`. Последний принимает 1–6 hex-цифр;
суррогаты и значения выше U+10FFFF недопустимы. Неэкранированный перевод строки
в литерале запрещён. Это лексика **ассемблера**, не новое правило `.ok`.

### 2.2 Константы

```oasm
const r0, 42
const r1, -7
const r2, 2.5
const r3, 1.0e-3
const r4, byte(255)
const r5, rat(1, 3)
const r6, true
const r7, 'x'
const r8, "hello\n"
const r9, Color.red
```

Целая десятичная запись без точки/exponent означает `int`. Float содержит
точку или exponent. `byte` и `rat` — единственные составные scalar constant
forms; это не вычисляемые вызовы. Числитель/знаменатель константы `rat` могут
быть произвольной точной длины в пределах resource limits загрузчика.

Констант объектов, closures, Futures, handles, `void`, `null`, массивов и maps
нет. Массивы/maps собираются инструкциями builder. Ассемблер сам создаёт пул
констант; отдельной `.const` директивы в 1.0 нет.

### 2.3 Операнды

| Форма | Значение |
|---|---|
| `r0` | Перезаписываемый временный регистр frame |
| `p0` | Read-only параметр текущего тела |
| `c0` | Read-only захват closure/cleanup |
| `self` | Lexical receiver; только там, где он существует |
| `_` | Отбросить обычный результат; не значение |
| `b0` | Affine builder, не Okkama value |
| `i0` | Affine iterator, не Okkama value |
| `name` | Статический slot/field/label/symbol в позиции инструкции |

Результат пишется первым. Константы сначала загружаются в регистр.
В инструкциях нет вложенных вызовов, `r0 + 1`, индексирования или вычисляемых имён.

### 2.4 Списки аргументов

```oasm
call r0, self, total, pos[]
call _, p0, write, pos[r1, r2]
new r3, Cash, named[digit=r0, cents=r1], _
call r4, self, format, pos[r0, forward p1]
```

`pos[...]` — позиционные values; `named[...]` — пары имя=ValueRef. Смешивать
формы, повторять named key или ставить хвост forwarding не последним нельзя.
`pos[forward p0]` допустим для нулевого явного префикса. Forwarding источник —
только текущий variadic `pN`; массив не распаковывается. [B §18.4]

Значения в argument list уже вычислены. Компилятор обязан сначала вычислить
receiver, затем arguments слева направо отдельными инструкциями. Повторно
вычислять receiver/argument при overload selection нельзя.

## 3. Дескрипторы: данные, а не второй синтаксис объектов

В OASM тела инструкций отделены от деклараций. Все тела оформляются одинаково:
`.code ... .end`. Конструктор, method, default, fixup и destroy отличаются
правами и контрактом entry descriptor, а не разными языками тел.

### 3.1 Заголовок и imports

```oasm
.oasm 1.0
.module demo/counter v1.0.0
.profile okkama-1
.import io "io/stdout@1"
.require async.lazy.v1
.entry main
```

Первые три директивы обязательны, строго в указанном порядке. `.import`,
`.require` и единственная необязательная `.entry` идут до деклараций.
`.entry` ссылается на один локальный module action без параметров. Результат
обычного main может быть `void` или value; async main обязан иметь value.
Результат программы не преобразуется неявно в stdout или OS exit code.

Import string — canonical module identity `path@ABI_MAJOR`, не путь поиска
Okkama source. Resolver получает explicit mapping identities в `.obc` artifacts
от launcher/компилятора. Нельзя молча выбирать случайную версию из каталогов.
В одном графе загружается одна конкретная версия каждой такой identity.
Фактические версии и хеши фиксируются отчётом linker; alias — только текстовое имя.

Флаги `.require`: `async.lazy.v1`, `spawn.process.v1`, `test.v1`.
`spawn.process.v1` требует `async.lazy.v1`. JIT не является обязательным флагом.
Модуль с неподдерживаемой требуемой возможностью не запускается.

### 3.2 Типы в дескрипторах

```text
int float rat byte bool char string any
@T   %K:V   range
Name   module_alias::Name   Name<T, U>
Future<T>
(T, U) -> R
```

Последняя форма — exact callable type. В result position дополнительно
разрешены `void` и `^ObjectOrRole`. Ни `void`, ни `^T` не обычные value types.
`Future<void>`, `@void`, `(void)->int` запрещены. `()->void` разрешён.
Только named async actions производят Future; async closures в 1.0 отсутствуют.

`auto` разрешён только в `.local`: тип фиксируется первым `local.bind` данной
активации. `raw`/`infer` — политики builders, не публичные типы.

### 3.3 Роли, объекты и enum

```oasm
.role Counting
    .method number() -> int
    .method advanced() -> ^Counting
.end

.object Counter
    .does Counting
    .field count: int
    .ctor from_count full(value: int) = counter_ctor
    .fixup counter_fixup
    .method number() -> int = counter_number
    .method advanced() -> ^Counter = counter_advanced
.end

.enum Color(red, green, blue)
```

Объект ДОЛЖЕН объявить минимум один `.ctor`; автоматического construction API
нет. `.ctor` label — техническое имя entry, не новая форма вызова `.ok`.

Поле с default:

```oasm
.field count: int = count_default
```

`count_default` — код `-> int` без параметров, receiver и captures. Runtime
вызывает defaults в порядке полей, до constructor body, в defining module.

Конструкторы:

```oasm
.ctor from_count full(value: int) = counter_ctor
.ctor selected named[count] = counter_selected
.ctor many full(first: T, rest: ...T) = many_ctor
```

`full` получает `p0..pN`. `named` авто-связывает перечисленные поля; его тело
читает их через `field.get`, параметров `pN` у него нет. Неуказанные поля не
становятся публичными inputs. Нулевая `named[]` допускает пустой `pos[]`.
Во всех иных shorthand случаях используйте `named[...]`, включая `named[]`
для default-only создания.

Если есть variadic constructor, он единственный. Полный fixed constructor
выбирается по arity; shorthand — по allowed/required names. Пересечение
кандидатов — ambiguity, не предпочтение порядка. [B §17]

Methods:

```oasm
.method number() -> int = number_code
.method secret() -> string private = secret_code
.method loaded(key: string) -> string async = loaded_code
```

Порядок необязательных флагов фиксирован: сначала `async`, затем `private`.
Private successor запрещён. `async -> void` запрещён. Successor-result допустим
только у object method и role method signature; module function, closure и native
не имеют собственного successor receiver и не могут объявить `-> ^T`. Во всех method/function/
closure/native signatures result обязателен. Role method не содержит `= code`.

Дополнительная необязательная метаинформация внутри object:

```oasm
.project value, number
```

Она связывает prefix `value` с единственной zero-argument декларацией `number`.
Префикс уникален внутри объекта. Цель public, synchronous, с обычным value
result, не successor и не own object type. Префиксов может быть несколько.
`.project` не исполняется и не меняет конструктор; source compiler использует
его для lowering, дизассемблер сохраняет metadata. Roles не приобретают
projection-контракты, которых нет в B §17.6.

`.fixup code` и `.destroy code`: максимум по одному, без параметров и результата
значения; ожидаемый конец тела — `ret.void`. Их нельзя вызвать через selector.

### 3.4 Generics

```oasm
.role Comparing<T>
    .method less(other: T) -> bool
.end

.object Holder<T: Comparing<T>>
    .field value: T
    .ctor from_value full(value: T) = holder_ctor
    .method item() -> T = holder_item
.end
```

Type parameters уникальны и лексически принадлежат descriptor. Один nominal
role bound разрешён только object parameter; никаких union/variance/category
bounds. Runtime специализирует template точными equations из constructor inputs
и прямого hint. Role evidence сохраняется, не сужается до случайного implementer.
Hints не передаются внутрь произвольных callee expressions. [B §25]

### 3.5 Module actions и native imports

```oasm
.function main() -> int = main_code
.function task() -> string async = task_code
.native clock "runtime/time@1::monotonic_clock/0" () -> int
```

Для opaque provider type используется отдельный bodyless descriptor:

```oasm
.opaque FileHandle
```

Он не имеет конструктора, полей, методов или права на `new`. Canonical identity
включает defining module/ABI major, как у обычных nominal types.

`Error` и `NativeFailure` — автоматически доступные nominal symbols
`okkama/core@1::Error` и `okkama/core@1::NativeFailure`. Первый задаёт
`text() -> string`; второй создаётся только runtime и удовлетворяет первому.
Переопределение этих двух коротких имён и префикса `okkama/core@1` обычным
модулем запрещено. Это единственные implicit nominal imports.

Module action является разрешённой формой Okkama, не новой global function
системой. Native descriptor — требование к объявленному typed ABI; он не может
заменять код обычного object action. Provider разрешается по canonical identity
и сравнивается с signature. Нельзя вызвать произвольный host symbol.

### 3.6 Closures и deferred code

```oasm
.closure Add(value: int) -> int captures[int] = add_code
.closure Recursive(n: int) -> int captures[] recursive = recursive_code
.cleanup Later origin main_code refs[current] values[] = later_code
```

`captures[...]` — типы snapshot `cN`. Closure code имеет точную signature.
`recursive` добавляет read-only имя `recur` текущей callable identity; это не
cyclic capture environment. Порядок модификаторов: `captures[...]`, затем
необязательный `owner Object`, затем необязательный `recursive`, затем `= code`.
`owner Object` сохраняет
private-field authority; runtime также удерживает lexical self, если код его
читает или создаёт вложенный код, которому self потребуется.

Cleanup `refs[...]` именуют `.local` slots origin code; они читаются **поздно**.
`values[...]` — типы дополнительных стабильных captures. `c0..cR-1` читают refs,
последующие `cN` — values. `defer.push` не вычисляет сам deferred expression.
Код cleanup имеет `-> void`, недоступен обычному `fcall` и не может suspend.
`defer.push` разрешён только в своей `origin` code-активации; ref-slots должны
быть видимы и initialized при регистрации. Они остаются живы до cleanup.
Lexical self/private authority наследуются от origin. Generic parameters в
closure/cleanup descriptors разрешаются в контексте их object owner/origin,
а не объявляются заново и не выводятся из caller.

### 3.7 Код, slots и scopes

```oasm
.code main_code
    .registers 4
    .builders 0
    .iterators 0
    .scope attempt, s0
    .local current, s0, Counter
    const r0, 0
    new r1, Counter, pos[r0], _
    local.bind current, r1
    call r2, r1, number, pos[]
    ret r2
.end
```

Первые три числовые директивы обязательны, включая нули. `s0` — автоматическая
корневая область каждого frame. `.scope child, parent` описывает lexical tree;
само описание не открывает runtime scope. `.local` описывает slot, а не его
значение. Метаданные scopes, locals и catches располагаются до инструкций.

Одно тело `.code` имеет ровно одного declaration owner. Нельзя присоединить
один и тот же body к чужим imported objects или выдать ему новые private-права.
Всякое тело завершается подходящим `ret`, `ret.void`, `ctor.end`, `succ.ret`,
`throw`, `rethrow`, `panic` либо явным переходом. Падение за конец запрещено.

## 4. Frames, значения и границы

Frame хранит PC; r-registers и bitmap initialization; readonly p/c/self;
активные scopes; local slots; handlers; cleanup stack; ownership records;
staging при successor; affine builders и iterators.

Параметры и captures не являются mutable registers. `move r0, p0` копирует
ссылку/значение, не объект. Source-level ограничения на `let` не делают
регистровую VM SSA-машиной: компилятор вправе перезаписывать scratch rN.

Okkama values: scalar, immutable array/map/range, enum, ordinary object,
closure, Future, boxed any, opaque handle. Нет Value для uninitialized, void,
builder, iterator или active exception. Служебные структуры недоступны через
обычную типизацию и provider ABI.

### 4.1 `boundary`

`boundary dst, src, T` использует правила Okkama typed boundary: проверяет
category/nominal identity; collections рекурсивно; roles через declared `does`;
`any` создаёт или сохраняет opaque box. Исходное value не мутирует: tagging
коллекции выражается проверенным immutable view, сохраняющим identities элементов.

В roles и generic специализациях exact type identities включают defining
module, ABI major, declaration и подставленные type arguments. Совпадение
коротких имён не означает совпадения типов.

Closure/Future/handle нельзя упаковать в `any`. Обычный объект можно, однако
затем нельзя читать его поля/вызывать его поведение через box. Нет unbox opcode.

### 4.2 Portable numeric policy D03

`int` — signed two's-complement 64-bit, но арифметика **checked**, не wrapping.
`byte` — 0..255. `float` — конечный IEEE-754 binary64, округление nearest,
ties-to-even; NaN/Infinity не допустимые OASM values. Переполнение/не-конечный
результат — uncatchable numeric failure. Поддерживаются subnormal и signed zero.
`rat` — arbitrary-precision нормализованная дробь, denominator > 0.

| Операнды | `add/sub/mul/rem` | `div` |
|---|---|---|
| int/byte | checked int | exact rat |
| rat с int/byte/rat | exact rat | exact rat |
| float с int/byte/float | float | float |
| rat с float | ошибка | ошибка |

`rem(a,b) = a - trunc(a/b)*b`, trunc направлен к нулю; это не floor modulo.
Делитель 0 во всех категориях — ошибка. Byte-result арифметика не производит.
Unary neg на byte возвращает int. Unary neg минимального int — overflow.
`add` также разрешает ровно `string + string`.

Числовые casts — закрытые builtin actions `float`, `int`, `byte`, `rat`,
`string`; typed boundary не приводит числа. Int→float может округляться,
поэтому фраза «сохраняет любой int точно» не является контрактом VM.
Float→int требует конечного значения, trunc к нулю и попадания результата в i64.
`float.rat()` использует canonical decimal spelling, а не binary fraction.

Canonical float text: кратчайшая десятичная запись, round-trip которой даёт
то же конечное binary64. Среди равнодлинных выбирается ближайшая к exact binary
value, tie — с чётной последней цифрой. Для decimal exponent k от -6 до 20
используется fixed notation, иначе scientific; `e` lowercase, без `+` и ведущих
нулей exponent; лишних дробных нулей нет. Оба нуля печатаются `0`.
Это нормирование D03 для воспроизводимости, не требование использовать host formatter.

### 4.3 Equality и ordering

`eq/ne`: разные scalar categories — ошибка; одинаковые exact scalars сравниваются
по значению. Rat каноничен. Float +0 и -0 равны. Для enums требуется один enum
TypeId; разные enum types — ошибка.

Объекты: разные concrete/applied TypeIds → false; одинаковые — все fields в
порядке объявления, рекурсивно. Arrays — длина и элементы по порядку; maps —
ключи и значения; ranges — start/end/inclusive. Collection tags используются
для type boundaries, но сами по себе не добавляют элементы в deep equality.

Сравнение `any`, handle, closure или Future — ошибка, в том числе при достижении
таких values внутри структур. Нельзя заменять deep equality сравнением pointers.
Допустим обычный short circuit после уже обнаруженного различия.

Ordering `lt/le/gt/ge` — только numeric, по таблице promotion; сравнение точных
integer/rat производится без float. Numeric builtin `less/more` дополнительно
требует одинаковой категории. [B §5, §8.5, §10]

### 4.4 Map keys

Профиль допускает ключи `int`, `float`, `bool`, `char`, `string`, enum; `byte`,
`rat`, `any` и composites запрещены. Исключение `rat` — фиксация VM для ещё
неописанного key edge case. Hash обязан различать категории и быть согласован
с equality; +0/-0 — один float key. Повторный key в map builder заменяет прежнее
значение, не меняя исходных values.

Порядок обхода maps: category rank `bool, int, float, char, string, enum`, затем
естественный порядок внутри category; enum — canonical type identity и порядок
case declarations. Межкатегорийное сравнение здесь внутреннее, не разрешение
Okkama `1 == 1.0`. Это portable порядок VM вместо host hash iteration.

## 5. Construction и закрытые поля

`new dst, Object, arguments, hint`:

1. Проверяет разрешённую call shape и выбирает constructor candidates.
2. Проверяет evaluated arguments, решает exact generic equations и bounds.
3. Создаёт непубличный construction frame и bitmap связанных полей.
4. Выполняет field defaults в declaration order и defining-module scope.
5. Выполняет shorthand bindings, если выбран `named`, затем constructor body.
6. Проверяет полноту и типы полей.
7. Запускает `fixup`, если он есть.
8. Только после успеха создаёт доступную caller identity и пишет `dst`.

`hint` — `_` либо прямой applied object/role target, используемый как evidence
для generic inference. Это не cast и не другой возвращаемый тип конструктора.
Сам `new` всегда возвращает concrete object.

`field.bind` допускается только в constructor body и его обычных вложенных
scopes; в lexical cleanup-контексте он запрещён: cleanup не должен достраивать/изменять объект. Повторные binds
в constructor body разрешены для normalization/default override. Поле читается
через `field.get` только после его binding.

`field.get` использует **implicit lexical self**. Он не принимает произвольный
receiver и не предоставляет reflection. Ordinary/private methods, fixup,
destroy и closures с подтверждённым owner могут читать только собственные fields.

Неполный self нельзя передать в external/native code, удержать в опубликованном
aggregate, closure или Future, либо вернуть. Private synchronous helper на
текущем receiver допустим, если читает уже связанные fields и не делает escape.
Вызов successor во время fixup запрещён, в том числе через цепочку helpers.

Новые object identities, связанные с fields даже из nested constructor scopes,
удерживаются construction frame до публикации или failure. Overwrite field не
уничтожает прежнюю owned dependency немедленно: она остаётся до cleanup frame.

Если constructor/default/fixup падает, объект не публикуется. Cleanup уже
приобретённых owned ресурсов выполняется; `destroy` самого не завершившего
construction объекта не вызывается. Failed `new` не оставляет старое значение
в destination как будто операция успешна.

## 6. Вызовы, overloads и result modes

| Инструкция | Target |
|---|---|
| `call d, v, name, args` | Обычный object или builtin receiver |
| `fcall d, symbol, args` | Объявленный module action |
| `acall d, v, pos[...]` | Anonymous action value |
| `ncall d, native_id, pos[...]` | Объявленный provider function |

Destination — `rN` либо `_`. `_` допускает void и обычный отброшенный value;
не допускает successor. `rN` требует value-producing call. Нельзя получить
`void` в регистр, массив, parameter, any или return. Void body исполняет все свои
инструкции; заканчивается `ret.void`, не отдавая случайный tail value.

Fixed actions выбираются по arity/named shape, runtime argument types и
последовательному dominance: concrete/applied > compatible role > any.
Кандидат обязан быть не хуже по всем параметрам и лучше хотя бы по одному.
Неоднозначность не разрешается порядком деклараций. Opaque any не раскрывается.
Все overloads одного имени имеют совместимый result mode/type. [B §18.3]

Нельзя обойти private: после выбора signature проверяется lexical call authority.
Callsite из closure сохраняет authority объявившего тела, но не получает новых
прав от вызывающего объекта.

Anonymous calls positional-only, exact-arity, exact-signature. Никакого SAM
conversion, currying или передачи closure в native ABI. Native calls не
перегружаются по host types.

Когда выбран async action, `call/fcall` возвращает `Future<R>` без исполнения
body. Отдельного `future.new` opcode нет: это result contract выбранного target.

### 6.1 Проверки до аргументов

Для OASM constructor target и argument shape статичны. Verifier заранее
отвергает named/positional ошибки, дубликаты names, невозможную arity и
неправильный forwarding. Отдельные `call.prepare`/`construct.prepare` из проекта
0.1 не нужны. Значение-зависимые проверки происходят после вычисления аргументов.

Сборщик `.ok` не имеет права перенести допустимый effect аргумента через другой
effect. Ошибка неверной статической shape обнаруживается при загрузке до кода;
фаза диагностики invalid programs может отличаться от tree-walker, но валидные
program traces должны совпадать.

## 7. Successors и must-use

Вход в `-> ^Type` открывает staging context, связанный с текущим concrete self.
`succ.set field, v` проверяет declared field type и сохраняет pending update.
Исходное поле не изменяется. `field.get` всегда читает первоначальный snapshot;
для вычислений от staged значения используются собственные registers/locals.
Повторный `succ.set` одного field заменяет staged value.

`succ.ret` выполняет:

1. Формирует candidate: исходные field references + финальные updates.
2. Проверяет типы и completeness; запускает `fixup` на candidate.
3. При успехе экспортирует новую identity и её owned dependencies.
4. Завершает frame через обычный cleanup, затем публикует результат caller.

Defaults и constructor не запускаются повторно. Candidate fixup не может
вызвать successor. Даже без updates получается новая identity, хотя equality
с прежним объектом может дать true. Вложенные references сохраняются, не
deep-clone. Результат имеет исходный concrete specialization; `^Role` — гарантия
conformance, не разрешение сменить concrete implementation.

Failure на любом шаге отменяет candidate; оставляет original untouched; очищает
staging и owned ресурсы. Уже выполненные внешние эффекты не откатываются.

### 7.1 Must-use без фиктивного использования

Каждый полученный successor result несёт служебное обязательство использования.
`move`, `boundary` и `scope.leave keep[...]` только передают его. Одно настоящее
потребление — typed binding/rebinding, argument, receiver call, `ret`, сохранение
в проверенном field/aggregate или вычисление над value — погашает обязательство.
Результат может иметь aliases; обязательство одно на конкретный результат вызова.

Отбросить его через `_`, затереть все его временные ссылки или нормально выйти
из frame без use нельзя. Просто наличие destination register ничего не доказывает.
Во время уже идущего failure непогашенное обязательство не маскирует первичную
ошибку: запускается cleanup. Verifier доказывает статические случаи; динамические
result modes проверяет runtime.

## 8. Локальные привязки, scopes и контроль потока

`local.bind slot, value` связывает slot один раз в текущей активации его scope.
Все slots неинициализированы при `scope.enter`; повторный вход цикла создаёт
новую активацию. `.local auto` запоминает точный тип первого значения данной
активации. Shadowing компилятор представляет разными SlotIds.

`local.rebind` обновляет уже активный slot, в том числе из descendant scope.
Declared type должен быть **concrete object** с actual successor action;
новое значение должно пройти ту же declared boundary. Scalars, collections,
roles и captured bindings не подходят. RHS не обязан быть непосредственно
successor-call: eligibility принадлежит declared concrete type. [B §12.1a]

Прежняя identity не уничтожается немедленно. Rebinding не меняет source object,
не даёт доступа к fields и не переносит видимость caller locals в callee.

Обычные `jump/branch` разрешены только между точками с одинаковой активной
lexical scope chain. Они не могут войти в середину scope, catch или constructor.
Для выхода сначала исполняются необходимые `scope.leave` от внутреннего к
внешнему, затем `jump`. `ret/throw/panic` выполняют unwind автоматически.

`scope.leave s, keep[...]` закрывает только текущий top scope `s`, никогда `s0`.
Он удерживает экспортируемые values как pending export, выполняет cleanup,
затем переносит необходимые owned identities родителю и делает values доступными
дальше. До успеха cleanup перенос не считается завершённым. Временные
register values, созданные внутри scope и не экспортированные, недоступны после
выхода; verifier/runtime tracking не позволяет использовать разрушенный объект.
Root `s0` закрывается только frame completion/unwind. Write в rN устанавливает
value lifetime текущего scope; `move/local.get` сохраняют identity, но не выдают
право пережить owner. Запись destination выполняется только при успехе операции.
При входе в handler scratch registers, определённые лишь на отдельных failure
путях, не считаются initialized; frontend сохраняет нужные данные в ancestor slots.

Нет VM-инструкций «вернуться из чужого action» или «next цикла caller».

## 9. Ownership и deterministic cleanup

Tracing/RC/другой GC определяет освобождение памяти, но **не момент `destroy`**.
OASM требует identity-based lexical obligations независимо от GC.

### 9.1 Учёт identities

Каждая новая опубликованная object identity получает owner scope текущего frame.
Переименование/копирование ссылки не создаёт вторую ownership obligation.
`pN`, captured references и receiver fields — borrows, пока правило экспорта
явно не переносит удержание. Aggregates сохраняют contained identities.

Практическая реализация сопровождает owner records и retention edges при
создании/экспорте; повторный обход всего живого графа на каждом local.bind
не является допустимым требованием алгоритма. [B §20; D §9]

Для простоты и безопасности временный object в r-register принадлежит тому же
текущему scope (D07). Неиспользуемый temporary очищается на выходе, не произвольно
во время GC. Это закрывает оставленную книгой неопределённость временных объектов.

### 9.2 Экспорт и разделяемые зависимости

Экспорт происходит через `ret`, `scope.leave keep`, binding в ancestor slot,
успешную construction/successor publication, удержание escaping closure/Future.
Переносится только ownership из выходящего scope/его дочерних регионов;
зависимости, уже принадлежащие живому ancestor, остаются borrowed.

Если несколько экспортируемых values разделяют identity, obligation переносится
один раз. Выбирается ближайший общий живой ancestor-owner их удерживающих roots;
при последовательных передачах допускается только promotion наружу, не
понижение в краткоживущий scope. Полученный value не должен ссылаться на identity,
которую этот же выход сейчас уничтожит.

Возвращаемое значение сначала закрепляется как pending export. Cleanup может
сорвать возврат: тогда оно не публикуется caller и временно удержанные новые
ресурсы тоже очищаются. Return не превращает ошибку cleanup в успех.

Native handles имеют отдельного provider owner. Два Okkama wrappers над одним
handle не получают два независимых OS-ресурса. Idempotent close предотвращает
повторное закрытие, но не исправляет преждевременное закрытие общего ресурса.

### 9.3 Порядок

На закрытии scope:

1. Его зарегистрированные defers, LIFO.
2. Его закрытие служебных builders/iterators без публикации результата.
3. Его owned objects, reverse construction-completion order.
4. Удаление bindings/служебного frame-state.

Новый объект регистрируется после построения своих новых dependencies; поэтому
wrapper уничтожается раньше принадлежащих ему зависимостей. Экспортированная
identity не уничтожается в прежнем scope. В `destroy` identity уже помечена
«entered destruction» и не может повторно войти в собственный destroy.

Все cleanup attempts получают шанс выполниться, даже если предыдущий упал.
Новые temporaries, создаваемые самим cleanup, принадлежат его отдельному frame.
`destroy` нельзя вызвать обычным методом; уничтоженное значение не воскресает.

### 9.4 `defer` выполняется всегда — в пределах управляемого исполнения

Нормальный выход, ret, typed throw, panic, программная ошибка, next/last и
управляемый shutdown проходят через cleanup. Async suspension **не** выход.
`defer` не запускается только потому, что action временно ждёт Future.

Гарантия не может охватывать остановку питания, SIGKILL, abort хоста, повреждение
процесса или бесконечно зависший provider. Это не отдельные пути Okkama semantics.

Регистрация lazy: `defer.push` сохраняет code/lexical refs, не вызывает expression.
Сначала поздно читаются slots и выполняются вызовы внутри cleanup body.
Anonymous closure, напротив, хранит snapshot. Замена defer обычной closure с
ранним snapshot меняет семантику и запрещена.

### 9.5 Приоритет failures

Если тело уже заканчивается failure, он остаётся главным; cleanup failures
записываются как secondary diagnostics. Если тело завершается нормально/ret/
next/last, главной становится первая ошибка defer в LIFO; если их нет — первая
ошибка destroy. Остальные записываются вторичными. У главной ошибки сохраняется
канал: catchable Throw или uncatchable Panic/ProgrammingError.

Ошибка cleanup отменяет pending normal return/branch. Все дальнейшие cleanup
исполняются. Catch matching начинается только после cleanup покидаемого try.

## 10. Typed throw, catch и panic

Три failure categories: `Throw(ErrorValue)`, `Panic(string)`,
`ProgrammingError(code, location)`. Catch обрабатывает только первую.
NativeFailure — nominal Error, созданный runtime из ожидаемой ошибки provider.
Непредвиденный host exception не становится автоматически catchable Error.

Catch metadata внутри `.code`:

```oasm
.scope trial, s0
.scope caught, s0
.local error, caught, Error
.catch trial, Error, handle_error, caught, error
```

`trial` — protected scope. Несколько `.catch` этого scope идут в source order.
Catch scope имеет того же parent, что trial. На Throw runtime закрывает scopes
до trial включительно, затем выбирает первый compatible catch и автоматически
активирует `caught`, связывает `error` и переходит на `handle_error:`. Никакого
обычного `scope.enter caught` на этом пути не нужно. Сам thrown Error и его удерживаемые dependencies закрепляются как pending
failure до unwind; их нельзя разрушить вместе с trial до catch. Если handler
выбран, удержание переносится его scope; при дальнейшей propagation — следующему
failure owner. Panic message также остаётся жив до финального diagnostic.
Catch types валидируются до
исполнения: только Error, соответствующий concrete/applied Error, NativeFailure.

Обычный branch не может входить в handler label. Ошибка catch body не возвращается
в собственный trial-handler; обрабатывается enclosing handlers. `rethrow`
разрешён только коду активного lexical catch и сохраняет identity/origin. Вызов
из catch произвольного helper не выдаёт helper право `rethrow`.

## 11. Коллекции и iteration

### 11.1 Builders

```oasm
build.new b0, array, type @int
build.add b0, values[r0]
build.add b0, values[r1]
build.end r2, b0
```

Builder policies:

| Policy | Поведение |
|---|---|
| `raw` | Нет inferred collection tag; сохранять конкретные элементы |
| `infer` | Array: первый элемент задаёт exact T; последующие проверяются |
| `type @T` / `type %K:V` | Рекурсивно проверять и tag результат |
| `like value` | Взять collection tag источника; значения не копировать заранее |

`infer` разрешён только array. Пустой infer-array без contextual evidence —
ошибка: frontend обязан использовать `type` или `like`, когда T известен.
Raw empty array/map допустимы. Map `build.add` получает ровно два значения,
array — одно. Вложенные objects сохраняют identity. Добавление в builder
удерживает новые dependencies до `build.end`/failure; вложенный scope не может
разрушить уже добавленный object. Публикация переносит удержание в итоговую
collection, не копируя эти objects.

bN affine: нельзя копировать, хранить в Value, передавать caller/provider или
использовать после `build.end`. Failure уничтожает builder и очищает принадлежащие
scope ресурсы. В suspended frame builder удерживается, но не разделяется с другим task.
Законченный результат immutable; прежние массивы/maps не изменяются.

### 11.2 Lookup

```oasm
lookup r0, r1, r2, missing
```

На present-edge r0 получает элемент. На missing-edge прыжок `missing`, r0
неинициализирован. `_` вместо label означает hard bounds/missing error.
Fallback код располагается только по missing-edge, не вычисляется заранее.
Неверный тип receiver/key/index — ошибка, не missing. Array index int/byte;
integer за пределами массива, включая отрицательный, — missing.

### 11.3 Iterator tokens

```oasm
iter.open i0, r0
again:
iter.has r1, i0
branch r1, item, finished
item:
iter.value r2, i0
; body
iter.next i0
jump again
finished:
iter.close i0
```

Для массива порядок индексный, для range — возрастающий, для map — §4.4.
Map дополнительно допускает `iter.key`. Для nominal
`collections/iteration@1::Iterable<T>` открытие вызывает `cursor()` один раз;
проверка — `available()`, value — `current()`, next — `advanced()`.
Только совпадения имён методов недостаточно. [B §12.5, §13]

Token states: NeedsCheck → Available/Exhausted. `iter.has` в NeedsCheck делает
реальную проверку и кеширует её; повтор в той же позиции возвращает тот же bool.
Первый `iter.value` Available выполняет current, последующие возвращают уже
полученное значение. `iter.next` Available продвигает и сбрасывает caches;
Exhausted-next/value/key — ошибка. `iter.key` допускается только map token.
`iter.close` потребляет token; следующий open может заново использовать iN.

Failure body не вызывает advanced. `last` закрывает token без advanced. `next`
после cleanup body продвигает ровно один раз. Tokens не Okkama values и не
передаются callbacks. Collection/cursor objects не мутируют.

`map/filter/reduce` компилятор раскладывает через эти инструкции. Пользовательский
functional block остаётся lexical inline code, а не snapshot closure: rebind
внешнего разрешённого slot не теряется из-за неявного closure-conversion.

## 12. Anonymous actions

`closure.new dst, ClosureDecl, values[...]` проверяет capture vector и удерживает
snapshot values + declared types. Изменение outer slot позднее не меняет snapshot.
Captured object сохраняет identity; closure удерживает необходимые owned resources
при escape. Захватывать bN/iN/varargs/ref-slot как обычное value нельзя.

`.closure owner O` даёт только lexical private authority, которую имел enclosing
код O. Verifier разрешает создание такой closure лишь коду той же authority.
Lexical self автоматически удерживается, если он реально нужен declaration;
это вычисляется из body и вложенных closure descriptors, не из обещания автора
недоверенного байткода.

`acall` создаёт собственные s0/locals/defer stack. Нельзя изменить captured `cN`,
прыгнуть в caller loop или получить caller locals. Recursive declaration имеет
`recur` как readonly operand; взаимной recursion через поздние captures нет.

Callable compatibility exact: inputs и result, включая VoidResult. Формы
`(int)->void` и `(int)->int` несовместимы. Covariance/SAM/mutable captures не
вводятся этой VM. [B §18.5]

## 13. Async и process spawn

### 13.1 Lazy Future

Async descriptor объявляет eventual result `R`; call получает `Future<R>`.
Проверки call shape/argument boundaries выполняются при call, body — нет.
Future сохраняет проверенный receiver/arguments и их необходимое удержание.

Состояния: Cold → Running → Succeeded(value) / Failed(Error) / Aborted(panic).
Первый await Cold запускает ровно одно вычисление. Повторные await используют
cached outcome; thrown failure сохраняет identity/origin внутри одного процесса.
Panic никогда не конвертируется в Error. Await самого running вычисления или
цикл зависимостей без внешнего event отвергается как async deadlock.

`await` разрешён только async code. Ready-result может продолжиться без suspension.
Suspension сохраняет PC, values, slots, scopes, handlers, defers, ownership,
staging, builders и iterators. Возобновление продолжает с инструкции после await.
Ни constructor, ни fixup, ни destroy, ни cleanup thunk не могут suspend.

Sync `main` идёт обычным путём. Для async `main` launcher вызывает его, получает
Future и выполняет scheduler до outcome; это не разрешает await в sync source.

Cold Future, утративший владельцев, не запускается ради cleanup: удержанные
новые ресурсы освобождаются по ownership rules. Running Future удерживается
scheduler до terminal outcome. Cached result экспортируется/заимствуется через
обычные ownership rules, не создаёт новую identity при повторном await.

### 13.2 Async successor

Async successor допустим: call → Future<R>, body → `succ.ret`, где R —
тип результата выбранной декларации после удаления `^` и generic substitution.
Фактическое значение остаётся successor конкретного receiver, а не новым
implementer роли. Поле eventual-result Future хранит этот объявленный контракт;
его typed boundary проверяет результат body при завершении.
Future помнит successor result mode. Его discarded call запрещён; связывание
Future удовлетворяет использованию lazy вычисления. При await полученный
successor вновь должен быть реально использован, `_` недопустим. Never-awaited
Cold successor не производит новую object identity и не запускает body.

### 13.3 Нет неявных потоков

Async не означает параллельность, purity или неблокирующий provider. Blocking
native operation блокирует поток VM, пока provider API не предлагает suspension.
Перенос handles на host threads без отдельного authority/affinity-контракта
запрещён. В 1.0 нет cancellation, channels, races, async iterators или async void.

### 13.4 Spawn

```oasm
spawn r0, Worker, pos[r1], number, pos[r2], _
```

Принимает constructor description, готовые constructor arguments и synchronous
method arguments. Создаёт worker process немедленно; receiver строится там.
Async и void targets запрещены. Все values вычислены в parent ровно один раз.

Переносимы int/float/rat/byte/bool/char/string и рекурсивные arrays/maps/ranges/
enums по §17.8. Нет переноса ordinary objects, handles, closures, Futures, any.
Никакого копирования parent heap или видимых `fork`-семантик. [B §31.5.1]

Success memoized. Worker typed failure превращается в parent NativeFailure
`domain=vm.spawn`, `code=worker_error`, operation=canonical target; diagnostic
содержит original error type/text, но не пытается восстановить Error object.
Worker panic/protocol corruption → uncatchable worker failure. Сигнальное/
неожиданное завершение процесса → catchable `worker_terminated`.

Штатный launcher ждёт всех started workers и reap-ит их; unseen failure
делает завершение неуспешным, не исчезает молча. Forced termination может
прервать worker без cleanup и не называется выполнением гарантии defer.

## 14. Native boundary

`ncall` принимает только явно объявленный NativeId. Сверяются canonical symbol,
arity, типы и ownership mode handles. Provider API value-based: int/float/bool/
string/char/byte, enum, typed arrays/maps, @byte и ограниченный any. Rat не
добавляется в existing native ABI автоматически; для него нужен отдельно
версионированный provider contract. [B §27]

Objects, closures, Futures, slots, builders, iterators и VM internals не
пересекают native boundary. Для совместимости с B §27 generic `ncall` не
принимает `rat` даже внутри any/collection: provider ABI должен явно объявить
расширенную версию, прежде чем такая категория станет допустимой.
Native handles разрешены только как объявленные
opaque nominal capability parameters/results. Их нельзя создать constant/new,
читать числом, сравнить или erase в any.

Handle table проверяет kind, authority, token, generation, ownership transfer
и open/closed state на каждом использовании. Close идемпотентен; use-after-close
→ typed NativeFailure `already_closed`. Runtime shutdown финализирует оставшиеся
provider resources ровно один раз после языкового cleanup.

Native failure fields: domain, code, message, operation. Только domain/code
пригодны для ветвления; host errno/exception objects не входят в language API.
Замена Perl-provider на Rust/C не меняет `.obc` contract. Политика consume/borrow
handle является частью зарегистрированного, версионированного provider contract;
её нельзя переопределить строкой assembly. Неизвестный provider не исполняется.
Наличие native imports не делает bytecode sandbox: доступ к FS/network/process
определяется разрешениями launcher и зарегистрированных providers.

## 15. Тестовый профиль

```oasm
.fake Package = PackageFake
.test "open round-trip" = package_test
```

Fake — обычный object descriptor текущего модуля с zero-input constructor,
defaulted fields и conformance enclosing role. `.fake` устанавливает hidden
storage identity и lexical permission; production loader не разрешает fake
construction или test instructions. Наличие `.require test.v1` само по себе
не даёт права: launcher обязан быть в test mode.

Test body — code без параметров, `-> void`, без implicit self даже при
необязательном display owner (`.test "name" owner "Package" = body`).
Root-module tests выполняются, imported tests нет; imported fakes доступны.

`test.check` operations: ok, is, isnt, fail. Assertions не прерывают body;
uncaught failure прерывает только текущий test после cleanup. Test capability
контекстна по dynamic execution, не first-class. Equality через any разрешено
только этой trusted boundary.

`fake.call`: set/get/has/remove/clear/size. Доступ только direct lexical fake
code и его разрешённым captures self; не произвольному receiver. Storage не
field и не value. Get/has/size производят value; set/remove/clear — void.
Missing get/remove — programming error. Successors одной fake сохраняют storage
identity; независимо созданные fakes — нет. [B §23]

## 16. Набор инструкций

`V` — ValueRef; `D` — writable rN или `_` лишь там, где допускается discard;
`T` — TypeId; `S` — StringId; `L` — instruction index метки; `A` — ArgPack.
В таблице binary operands порядок точно соответствует сериализации после header.
Все простые operands — u32. `vec(X)` = u32 count и count элементов X.

| Opcode | Мнемоника | Binary operands | Назначение |
|---|---|---|---|
| `0x0000` | `const` | `dst, ConstId` | Загрузить константу из пула. |
| `0x0001` | `move` | `dst, V` | Скопировать значение/ссылку; не объект и не владение. |
| `0x0002` | `boundary` | `dst, V, T` | Проверить границу типа; создать typed view или any-box. |
| `0x0003` | `unary` | `dst, UnaryOp, V` | Закрытая унарная операция: neg или not. |
| `0x0004` | `binary` | `dst, BinaryOp, V, V` | Закрытая бинарная операция над значениями. |
| `0x0005` | `jump` | `L` | Переход в той же активной цепочке scopes. |
| `0x0006` | `branch` | `V, L, L` | Переход по exact bool. |
| `0x0007` | `scope.enter` | `ScopeId` | Открыть дочернюю область. |
| `0x0008` | `scope.leave` | `ScopeId, vec(V)` | Закрыть верхнюю область с cleanup; экспортировать указанные значения. |
| `0x0009` | `local.bind` | `SlotId, V` | Инициализировать локальную привязку текущей активации scope. |
| `0x000A` | `local.get` | `dst, SlotId` | Прочитать активную привязку. |
| `0x000B` | `local.rebind` | `SlotId, V` | Перепривязать разрешённый конкретный successor-capable local. |
| `0x000C` | `field.get` | `dst, FieldIndex` | Прочитать поле lexical self. |
| `0x000D` | `field.bind` | `FieldIndex, V` | Связать поле только в construction frame. |
| `0x000E` | `new` | `dst, ObjectSymbol:S, Hint:T-or-NONE, A` | Выбрать конструктор, собрать и проверить объект. |
| `0x000F` | `call` | `D, V, Selector:S, A` | Обычный member dispatch; async создаёт lazy Future. |
| `0x0010` | `fcall` | `D, FunctionSymbol:S, A` | Вызвать объявленное module action. |
| `0x0011` | `acall` | `D, V, A` | Вызвать anonymous action с exact signature. |
| `0x0012` | `ncall` | `D, NativeDeclId, A` | Вызвать объявленный provider ABI. |
| `0x0013` | `ret` | `V` | Вернуть значение после проверки и cleanup. |
| `0x0014` | `ret.void` | `—` | Завершить void action/hook/cleanup без значения. |
| `0x0015` | `ctor.end` | `—` | Завершить constructor body; проверку и fixup ведёт new. |
| `0x0016` | `succ.set` | `FieldIndex, V` | Стадировать поле будущей версии receiver. |
| `0x0017` | `succ.ret` | `—` | Построить, проверить и вернуть successor. |
| `0x0018` | `throw` | `V` | Поднять номинальный Error. |
| `0x0019` | `rethrow` | `—` | Повторить активную ошибку текущего lexical catch. |
| `0x001A` | `panic` | `V` | Uncatchable failure с exact string. |
| `0x001B` | `defer.push` | `CleanupDeclId, vec(V)` | Зарегистрировать deferred code с поздним чтением local refs. |
| `0x001C` | `closure.new` | `dst, ClosureDeclId, vec(V)` | Создать callable со snapshot captures. |
| `0x001D` | `await` | `D, V` | Запустить cold Future либо дождаться running; получить cached outcome. |
| `0x001E` | `spawn` | `dst, ObjectSymbol:S, Hint:T-or-NONE, Selector:S, A, A` | Начать synchronous action в отдельном процессе. |
| `0x001F` | `build.new` | `BuilderId, Kind, Policy, PolicyOperand` | Открыть непубликуемый builder array/map. |
| `0x0020` | `build.add` | `BuilderId, vec(V)` | Добавить элемент либо пару key/value. |
| `0x0021` | `build.end` | `dst, BuilderId` | Потребить builder и получить immutable collection. |
| `0x0022` | `lookup` | `dst, V, V, L-or-NONE` | Индексирование; отдельное missing-ребро или trap. |
| `0x0023` | `range.new` | `dst, V, V, Inclusive` | Создать диапазон exact integer bounds. |
| `0x0024` | `varargs.array` | `dst, ParameterIndex` | Материализовать varargs<T> в @T. |
| `0x0025` | `iter.open` | `IteratorId, V` | Открыть служебный обход array/map/range/Iterable. |
| `0x0026` | `iter.has` | `dst, IteratorId` | Проверить наличие текущего элемента. |
| `0x0027` | `iter.key` | `dst, IteratorId` | Получить ключ map-итерации. |
| `0x0028` | `iter.value` | `dst, IteratorId` | Получить текущий элемент. |
| `0x0029` | `iter.next` | `IteratorId` | Продвинуть служебный обход; для Iterable вызвать advanced. |
| `0x002A` | `iter.close` | `IteratorId` | Закрыть служебный обход без мутации коллекции. |
| `0x0100` | `test.check` | `TestOp, vec(V)` | Контекстные assertions активного теста. |
| `0x0101` | `fake.call` | `D, FakeOp, vec(V)` | Операция storage текущей fake identity. |

Textual arity, optional operands и grammar приведены в `OASM-1.0.ebnf`;
списки `values[]`/`keep[]` в binary становятся `vec(V)`. Регистры rN не имеют
особого нулевого регистра: r0 — обычная writable ячейка.

Номера фиксированы. 0x002B..0x00FF и 0x0102..0xFFFF зарезервированы, их
исполнение запрещено. Использовать незнакомый opcode как NOP нельзя.

### 16.1 Подкоды и special operands

`UnaryOp`: 0=neg, 1=not.
`BinaryOp`: 0=add, 1=sub, 2=mul, 3=div, 4=rem, 5=eq, 6=ne,
7=lt, 8=le, 9=gt, 10=ge. `&&/||` компилируются short-circuit branches,
не являются eager binary opcodes.

Builder Kind: 0=array, 1=map. Policy: 0=raw, 1=infer, 2=type, 3=like.
PolicyOperand: 0 для raw/infer; TypeId для type; ValueRef для like.
Inclusive: 0 либо 1, другие значения invalid.

`TestOp`: 0=ok (1 argument), 1=is (2), 2=isnt (2), 3=fail (1).
`FakeOp`: 0=set (2, void), 1=get (1, value), 2=has (1, bool),
3=remove (1, void), 4=clear (0, void), 5=size (0, int).

`NONE = 0xFFFFFFFF` обозначает отсутствие metadata operand, не Okkama value.
D с таким значением означает `_`. Dest обычного value-producing opcode никогда
не NONE. Бинарный `new/spawn` хранит hint до ArgPack, хотя текстовый hint идёт
последним; assembler выполняет это однозначное переупорядочение метаданных.

### 16.2 Закрытая таблица builtin dispatch

Для non-object receiver `call` разрешает только таблицу ниже. Все вызовы
positional-only, exact arity. Объект с таким же selector использует собственное
поведение, не builtin. Несуществующее имя — programming error.

| Receiver | Selector(args) | Результат/правило |
|---|---|---|
| array | `size()` | int, число элементов |
| array | `slice(start,end)` | Новый array/view, end-exclusive, hard bounds |
| @string | `join(separator)` | string; raw empty без @string target не подходит |
| string | `length()` | Число Unicode scalars |
| string | `slice(start,end)` | string, code-point offsets, hard bounds |
| string | `split(separator)` | @string, literal split, trailing empty сохраняются |
| string | `replace(target,replacement)` | string, все non-overlapping совпадения |
| string | `replace_first(target,replacement)` | string, первое совпадение |
| string | `trim()` | string, убрать крайние ASCII space/tab/LF/CR/VT/FF |
| string | `chomp()` | string, убрать один завершающий CRLF, LF или CR |
| map | `size()` / `has(key)` | int / bool |
| map | `keys()` / `values()` | Array в порядке §4.4; сохраняет known element tag |
| numeric | `less(x)`, `less_or_equal(x)`, `more(x)`, `more_or_equal(x)` | bool, одинаковая category |
| int/byte/float/rat | `int/float/byte/rat/string()` | Только определённые conversions §4.2 |
| bool | `string()` | `"1"` или `"0"` |

Numeric conversion to own category сохраняет value; float/rat byte conversion
truncates и проверяет диапазон. Byte->rat exact. Char, enum, range, any, Future
не получают дополнительные builtin methods. Generic user objects не получают
автоматическую stringification.

Пустой split separator разбивает строку на code points; пустой replacement target
соответствует boundaries, включая начало и конец; replace_first с пустым target
добавляет префикс. Bounds принимают int/byte, не float. Trim/chomp и недописанные
края builtin table здесь зафиксированы как portable решения профиля, не как
проверенная копия host behavior.

## 17. Бинарный формат OBC 1.0

### 17.1 Базовые кодировки

Все числовые поля little-endian, без native alignment внутри records.
`u8/u16/u32/u64/i64/f64` имеют указанный размер. `vec<X>` = u32 count + X[count].
`blob` = u32 byte_length + bytes. `record` = u32 payload_length + payload.
Все длины проверяются **до** allocation; умножение/сложение размеров checked.

`StringId/TypeId/ConstId/DeclId/CodeId` — u32 индексы соответствующих таблиц,
zero-based. Отсутствующий optional reference — NONE. Смешение пространств IDs
запрещено. Никакой pointer relocation между host address spaces нет.

### 17.2 Header и directory

Header, ровно 24 bytes:

| Offset | Поле | Размер |
|---|---|---|
| 0 | ASCII `OKBC` | 4 |
| 4 | format_major = 1 | u16 |
| 6 | format_minor = 0 | u16 |
| 8 | flags = 0 | u32 |
| 12 | section_count | u32 |
| 16 | directory_offset | u64 |

Directory entry, ровно 24 bytes:

| Поле | Размер |
|---|---|
| kind | u16 |
| flags | u16 |
| offset | u64 |
| byte_length | u64 |
| entry_count | u32 |

Flags bit0=REQUIRED; остальные 0. Каждая section начинается по 4-byte boundary;
внешнее padding заполнено нулями. Header, directory и sections не пересекаются.
Нельзя ссылаться за конец файла или переполнять размер арифметикой. Unknown
REQUIRED section — reject; unknown optional section можно пропустить.

### 17.3 Секции

| Kind | Section | Обязательность | Формат |
|---|---|---|---|
| 1 | MODULE | required, 1 entry | §17.4 |
| 2 | STRINGS | required | `record` с exact UTF-8 bytes |
| 3 | TYPES | required | `record` type descriptors |
| 4 | CONSTANTS | required | `record` constants |
| 5 | DECLARATIONS | required | `record` declarations |
| 6 | CODE | required | `record` bodies |
| 7 | DEBUG | optional | §17.9 |

Одного kind может быть не более одного. Entry_count таблицы — из directory;
он не дублируется внутри payload. Нулевая таблица имеет byte_length=0.
Содержимое каждой известной section потребляется ровно до конца; trailing
неописанные bytes запрещены.

### 17.4 MODULE

```text
name:StringId
major:u32 minor:u32 patch:u32
profile:u32                 ; 1 = okkama-1
features:u32                ; bit0 async, bit1 process-spawn, bit2 test
entry_symbol:StringId|NONE
imports:vec<StringId>        ; "module/path@major"
```

Unknown profile/features отвергаются. Name — canonical module path без `@`.
Version компонентов — без ведущих нулей в canonical text. Для import alias в
binary отдельной роли нет: declaration references полностью квалифицированы.
Disassembler вправе назначить a0/a1 aliases при восстановлении текста.

### 17.5 TYPES и signatures

Type record начинается `tag:u8`, далее:

| Tag | Вид | Payload |
|---|---|---|
| 0 | int | — |
| 1 | float | — |
| 2 | rat | — |
| 3 | byte | — |
| 4 | bool | — |
| 5 | char | — |
| 6 | string | — |
| 7 | any | — |
| 8 | range | — |
| 9 | array | element:TypeId |
| 10 | map | key:TypeId, value:TypeId |
| 11 | nominal | symbol:StringId, arguments:vec<TypeId> |
| 12 | parameter | owner_symbol:StringId, parameter_index:u32 |
| 13 | callable | inputs:vec<TypeId>, result:Result |
| 14 | future | eventual:TypeId |

Bare generic object construction target — StringId в `new`, а не неправильно
применённый nominal Type. TypeId на `List<T>` уже имеет exact arity.
Raw untagged collections не требуют отдельного Type entry; runtime category
сохраняется до explicit boundary. Structural type graph не может содержать
бесконечного цикла; nominal recursion через fields легальна.

`Result` = tag:u8 + payload: 0=Value(TypeId), 1=Void(no payload),
2=Successor(TypeId). Callable result Successor в 1.0 запрещён: anonymous action
не имеет собственного receiver. Async eventual Void запрещён; future type
содержит только value TypeId, successor obligation хранится в Future metadata.

`Param` = name:StringId, type:TypeId, flags:u8 (bit0 variadic).
`Signature` = parameters:vec<Param>, result:Result.
`GenericParam` = name:StringId, bound:TypeId|NONE.
`Flags` methods/functions: bit0 async, bit1 private; другие bits 0.

### 17.6 CONSTANTS и DECLARATIONS

Constant tags:

| Tag | Вид | Payload |
|---|---|---|
| 0 | int | i64 |
| 1 | float | f64 bits, finite only |
| 2 | byte | u8 |
| 3 | bool | u8, 0/1 |
| 4 | char | u32 Unicode scalar |
| 5 | string | StringId |
| 6 | rat | sign:u8 (0/1), numerator:blob, denominator:blob |
| 7 | enum | TypeId, case_index:u32 |

Rat magnitudes big-endian unsigned, minimal: zero = empty numerator; denominator
nonempty; no leading zero byte, denominator >0, gcd=1, negative zero запрещён.
Отличие endian magnitudes от little-endian fixed integers намеренное и фиксированное.

Declaration record: tag:u8, symbol:StringId, payload. Symbol включает
`module/path@major::Name`, для unnamed helper descriptor всё равно есть уникальный
assembly label. Tags и payload:

**0 Object**

```text
generics:vec<GenericParam>
roles:vec<TypeId>
fields:vec<Field>
constructors:vec<Constructor>
methods:vec<Method>
fixup:CodeId|NONE
destroy:CodeId|NONE
projections:vec<Projection>
```

Field = name:StringId, type:TypeId, default_code:CodeId|NONE.
Constructor = label:StringId, kind:u8 (0 full, 1 named),
parameters:vec<Param>, allowed_fields:vec<u32>, code:CodeId.
Full имеет пустой allowed_fields; named имеет пустой parameters, field indexes
задают allow-list. Required fields выводятся из наличия defaults, не отдельного
противоречивого bitmap.
Method = name:StringId, signature:Signature, flags:u8, code:CodeId.
Projection = prefix:StringId, method_index:u32.

**1 Role**: generics:vec<GenericParam> (все bounds NONE), methods:vec<RoleMethod>.
RoleMethod = name:StringId, signature:Signature, flags:u8 (private запрещён).

**2 Enum**: cases:vec<StringId>.
**3 Function**: signature:Signature, flags:u8 (private запрещён), code:CodeId.
**4 Native**: provider_symbol:StringId, signature:Signature.
**5 Closure**: signature:Signature, captures:vec<TypeId>, owner:TypeId|NONE,
flags:u8 (bit0 recursive), code:CodeId.
**6 Cleanup**: origin:CodeId, ref_slots:vec<u32>, values:vec<TypeId>, code:CodeId.
**7 Test**: display_name:StringId, display_owner:StringId|NONE, code:CodeId.
**8 FakeMapping**: role:TypeId, object:TypeId.
**9 Opaque**: no payload.

Native signature fixed-only, normal Value/Void result, никогда Successor.
Cleanup/Test/default/fixup/destroy signatures выводятся из descriptor и не
дублируются. CodeIds one-owner, но code origin/ref access графы разрешены.
Для Test/FakeMapping без собственного имени в тексте assembler назначает
служебный symbol `__oasm_test_N` / `__oasm_fake_N` в порядке деклараций; имена с
префиксом `__oasm_` пользователь объявлять не может. Остальные unnamed bodies
всё равно имеют явный code label. Внутренние symbols не становятся public API.

FakeMapping относится к local synthetic object без generic argument evidence;
generic fake template использует собственные TypeParams и отображается
symbolically на параметры enclosing role. Его конкретная specialization задаётся
hint при `new`, как у source `Role.fake()`.

### 17.7 CODE и инструкции

Code record:

```text
register_count:u32
builder_count:u32
iterator_count:u32
scopes:vec<Scope>           ; включая implicit s0 с parent NONE
slots:vec<Slot>
catches:vec<Catch>
instruction_count:u32
bytecode:blob
```

Scope = name:StringId, parent:u32|NONE. Root индекс 0.
Slot = name:StringId, scope:u32, declared_type:TypeId|NONE (NONE=auto).
Catch = protected_scope:u32, error_type:TypeId, target_instruction:u32,
catch_scope:u32, slot:u32. Порядок записей значим.

Каждая инструкция:

```text
opcode:u16
word_count:u16             ; включая header word
operands:u32[word_count-1]
```

Instruction index — порядковый номер, не byte offset. Bytecode blob содержит
ровно instruction_count инструкций; word_count не может быть 0, decoder сверяет
его со схемой opcode, включая lengths всех vectors. Максимум одной инструкции
65535 words — предел формата, не разрешение слепо выделить столько памяти.

ValueRef:u32 = kind в bits31..29, index в bits28..0:
0=register rN, 1=parameter pN, 2=capture cN, 3=self (index=0),
4=recur (index=0). 5..7 invalid. D — отдельное поле raw register index либо
NONE; writable p/c/self не существует. Ref index меньше объявленного count.

ArgPack состоит из:

```text
mode:u32                ; 0 positional, 1 named
count:u32
entries                  ; pos: ValueRef[count]; named: (StringId,ValueRef)[count]
forward_parameter:u32    ; ParameterIndex или NONE
```

Для named forwarding всегда NONE. Явные аргументы precede forwarded tail.
Все value operands допускают только initialized registers и разрешённые readonly
sources. Varargs pN нельзя читать как ordinary V, кроме dedicated forwarding,
`varargs.array` и `iter.open` для for-unpack.

### 17.8 Worker wire contract

Это отдельный protocol, не сериализованный host heap. Message envelope:
ASCII `OWP1`, version:u16=1, kind:u16, payload_length:u64, payload bytes.
Kinds: 0 START, 1 SUCCESS, 2 THROWN, 3 PANICKED. Pipes/process exit передают
EOF/abnormal completion отдельно. Все поля little-endian, length checked.

START: module_identity:text, graph_hash:32 bytes, object_symbol:text,
constructor_mode:u8, ctor_args:vec<WireArg>, action_name:text,
action_mode:u8, action_args:vec<WireArg>, hint:optional TypeText.
`text` = UTF-8 blob; WireArg для named содержит text-name, для positional нет.
Hint optional = u8 present + text при present=1. TypeText — canonical OASM type
без aliases; descriptor registry worker проверяет его, не исполняя исходный код.
Worker загружает тот же module dependency closure; несовпадение hash — failure.
`graph_hash` = SHA-256 от конкатенации отсортированных по UTF-8 identity записей
`text(module_identity) || SHA-256(exact_obc_bytes)`. Это проверка одинакового
набора образов, не цифровая подпись и не доказательство доверия. Материалы
загружает launcher из своего resolved graph, не по произвольным путям worker payload.

SUCCESS содержит одно WireValue. THROWN: origin_type:text, diagnostic:text;
PANICKED: diagnostic:text, origin_location:text. Никакого arbitrary class loading
для восстановления Error из origin_type не производится.

WireValue = tag:u8 + payload: int(i64), float(f64), rat(sign+two blobs), byte(u8),
bool(u8), char(u32), string(blob), enum(canonical type text+case text),
range(i64 start,i64 end,u8 inclusive), array(tag_text_optional+vec<WireValue>),
map(key_tag_optional,value_tag_optional,vec<(WireValue,WireValue)>).
Tags по порядку перечисления 0..10. Optional type text кодируется как hint.

Перед transfer весь граф проверяется рекурсивно. Даже пустая typed collection с
нетрансферным element type (`@User`, `@Future<int>`) отвергается: пустота не
снимает контракт типа. Depth/size quotas обязательны. Результат worker проверяется
против вычисленного Future result contract. Незнакомые type identities не
угадываются.

### 17.9 Debug и canonical round-trip

DEBUG payload: `files:vec<StringId>`, затем `locations:vec<Location>`.
Location = CodeId, instruction_index:u32, file_index:u32, line:u32, column:u32.
Все строки/колонки one-based. Код без debug остаётся исполнимым.

Canonical serialization: sections по kind; imports/nominal declarations по
canonical symbol; methods/fields/parameters/catches в semantic declaration order;
strings уникальные, byte-lexicographic; type descriptors topologically ordered
с lexicographic tie-break canonical type text; constants deduplicated и sorted по
canonical tagged binary payload. При сортировке все IDs перенумеровываются.
Code order — порядок owner records и вложенных bodies; labels нормализуются в
instruction indices. Все reserved bits/padding zero.

`assemble(disassemble(B))` обязан сохранять semantic module. Для canonical B без
DEBUG результат byte-identical. Исходные комментарии, aliases, пробелы и имена
r-registers не обязаны восстанавливаться. Forwarders могут сохранять unknown
optional sections, но canonical reassembly вправе удалить их с явным отчётом.

## 18. Verifier и диагностика

### 18.1 Обязательные проверки до исполнения

Проверяются длины/индексы/лимиты/unknown features; type reference validity,
nominal/generic arity; unique owners; role conformance; signatures и result
modes; missing constructors; duplicate fields/cases/projection prefixes;
constructor call shape; declared native contract.

По CFG: register definite initialization; ровно одна согласованная active scope
chain в каждой точке; отсутствие jump внутрь catch/child scope; разрешённые
readonly operands; slot visibility; terminal instruction; field-bind/staging
placement; private authority; affine b/i states; statically known void misuse;
must-use obligations, где они выводимы из descriptors; test privileges.

На failure edges result register не считается initialized. Нельзя считать код
безопасным только потому, что проблемная ветка сейчас не достигается.

### 18.2 Runtime checks остаются обязательными

Exact dynamic value-type mismatch; generic equations; overload winner;
unknown receiver method; numeric overflow; bounds; native handle authority;
future state; dynamic must-use; complete fields; forbidden successor during
fixup — проверяются также в интерпретаторе/JIT.

Verifier не обещает статически доказать всё, что зависит от данных. Handwritten
OASM не даёт возможности обойти immutable fields или opaque handles, но может
использовать обычные mutable scratch registers, отсутствующие в `.ok`.

### 18.3 Ошибки инструмента и программы

Стабильные семейства:
`ASM_SYNTAX`, `FORMAT_INVALID`, `LINK_UNRESOLVED`, `VERIFY_TYPE`,
`VERIFY_CONTROL`, `VERIFY_AUTHORITY`, `VERIFY_AFFINE`, `VOID_VALUE`,
`SUCCESSOR_UNUSED`, `RUNTIME_TYPE`, `NUMERIC_RANGE`, `BOUNDS`,
`NATIVE_UNAVAILABLE`, `ASYNC_STATE`, `TEST_MODE_REQUIRED`.

Диагностика содержит module identity, CodeId/symbol, instruction index и,
если доступно, source location. Wrong bytecode не должен провоцировать host UB,
out-of-bounds access или попытку вызвать произвольный host адрес.

## 19. Контракт JIT

JIT необязателен. Интерпретатор той же verified Module — semantic reference.
Ни `.obc`, ни OASM не зависят от Cranelift/LLVM или register allocation backend.

Первый JIT может компилировать method-at-a-time и вызывать runtime helpers.
Он ДОЛЖЕН сохранять: evaluation order, exact categories, overload resolution,
error channel/origin, object identity/aliasing, scope/defer/destroy order,
staging atomicity, snapshot captures, Future memoization и provider boundaries.

Обязательные logical safepoints: allocations, language/native call boundaries,
backedges, await suspend/resume. Живые roots, locals и ownership state доступны
runtime через stack maps либо консервативное pinned frame представление.
Baseline не обязан иметь speculative deoptimization/OSR; если они добавлены,
reconstruction восстанавливает также EH/defer/staging/ownership, не только rN.

Нельзя считать action pure из-за immutable self, имени `value/number` или
projection directive. Нельзя автоматически memoize обычные methods. Нельзя
заменять successor in-place mutation без доказанного сохранения всех наблюдаемых
identities и cleanup. Любая оптимизация имеет интерпретируемый эквивалент.

Host crashes и performance не являются частью гарантии equivalence. JIT cache
отдельный, keyed по hash Module, OASM/profile/runtime ABI, target и CPU features;
он никогда не заменяет portable `.obc`.

## 20. Соответствие Okkama без раздувания OASM

| Okkama | Представление OASM |
|---|---|
| typed let / exact inference | `.local` + `local.bind` + boundary rules |
| ограниченный rebind | `local.rebind` |
| constructors/defaults/fixup | object descriptor + `new`, `field.bind`, `ctor.end` |
| private fields/actions | lexical owner + verifier + dispatch |
| successors | `succ.set`, `succ.ret` |
| void | Result::Void + `_` call destination + `ret.void` |
| roles/generics/enums | nominal descriptors/type templates |
| projection prefixes | metadata + `new` и обычный `call` |
| arrays/maps | builders + lookup + boundaries |
| ranges | `range.new` + iteration |
| for/loop/if/match | branches/scopes/iteration, без high-level opcodes |
| map/filter/reduce | inline block + iterator + builder/accumulator |
| anonymous actions | closure descriptor + `closure.new`, `acall` |
| try/catch expressions | catch metadata + exported branch value |
| throw/panic | разные failure channels одного unwinder |
| defer/destroy | cleanup descriptor / lifecycle code |
| lazy async/await | async descriptor + normal call + `await` |
| process spawn | `spawn` + restricted value wire |
| native/std | typed provider boundary + normal object code |
| tests/fakes | test profile + 2 restricted opcodes |

**Сахар находится в frontend. Гарантии находятся в runtime/verifier.
Алгоритмы stdlib не дублируются инструкциями.**

## 21. Проверка реализации и критерий завершения

Реализация считается соответствующей OASM 1.0 после прохождения матрицы
`CONFORMANCE.md`: text/binary round-trip, malformed modules, numeric edges,
construction/fixup, successors, void, roles/generics, scopes и все cleanup exits,
closures, iteration, native handles, Future/spawn и test privilege boundaries.

Для выпуска совместимого Okkama backend дополнительно нужны differential tests
against reference interpreter по valid program traces и отдельно утверждённые
тесты решений D03–D12. Сравниваются values/types, output, selected errors и cleanup
order; различия нельзя скрывать изменением expected outputs под новый runtime.

JIT включается только после interpreter conformance и проходит те же tests.
Скорость не заменяет корректность. Размер этой спецификации не означает, что
все subsystem нужно реализовать в первом коммите: opcode namespace и contracts
фиксированы, а реализация строится вертикальными слоями.

## 22. Короткий цельный модуль

```oasm
.oasm 1.0
.module example/answer v1.0.0
.profile okkama-1
.entry main
.function main() -> int = answer
.code answer
    .registers 1
    .builders 0
    .iterators 0
    const r0, 42
    ret r0
.end
```

У него один constant, один function descriptor, один code body и две
инструкции. Та же `.code` форма используется для методов, конструкторов и
cleanup; высокоуровневых выражений в ассемблере нет.

## 23. Комплект документов

- `OASM-1.0.md` / `.pdf` — настоящий нормативный проект.
- `OASM-1.0.ebnf` — полная текстовая grammar.
- `schema/opcodes.json` — числовая ISA и binary operand layouts.
- `examples/*.oasm` — целые assembly modules без пропущенных тел.
- `CONFORMANCE.md` — обязательные сценарии проверки реализации.
- `DECISIONS.md` — фиксации и отличия от эскиза 0.1.
- `VALIDATION.md` — что действительно проверено при подготовке комплекта.
- `SOURCES.md` — исходные документы и привязка решений к разделам.

В комплекте нет готового production assembler, VM или JIT. Примеры —
спецификационные fixtures; их синтаксис проверен вспомогательным parser check,
исполнение остаётся задачей реализации.
