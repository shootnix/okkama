# Okkama: стандартная библиотека — практическая книга


Кукбук на русском языке. Локальный срез `std/` от 18 октября 2026 года.

Начните с задачи: что нужно прочитать, сохранить, проверить или передать. В каждом разделе приведены подходящие модули, объяснение выбора и полные программы. В таблицах указан практический смысл **каждого прикладного модуля**; ссылка «рецепт» ведёт к коду его использования или публичного потребителя. Общие протокольные роли раскрываются рядом с задачей, которую они обслуживают.

Книга охватывает **506 модулей** текущего дерева: **351 прикладной**, **95 внутренних** и **60 runtime-модулей**. Служебные слои разобраны в приложениях вместе с их потребителями. API сверяется с исходниками; дополнительные сигнатуры собраны в [справочнике std](std/stdlib-api-v4.md). Правила языка описывает [книга Okkama](OkkamaBook-RU.md).

Готовые программы и отдельные веб-проекты: [учебный набор](../examples/cookbook/README.md). Все 158 рецептов доступны как [отдельные лаборатории](../examples/cookbook/LABS.md).

## Команды Rust-инструментов

```sh
okkama run test.ok
okkama build test.ok --target jvm -o out.class
okkama build test.ok --target sparrow -o out.spb
sparrow out.spb
```

`run` исполняет исходник Rust-интерпретатором. `build` создаёт выбранный
байткод; отдельный Sparrow исполняет `.spb`. `--backend` остаётся алиасом
`--target`. `sparrow check out.spb` проверяет файл без выполнения.
Инструкции по сборке инструментов и таблица поддержки рантаймов находятся в
[руководстве Rust-реализации](../okkama-rs/README.md).

## Подготовить примитивные значения для библиотечного API

Задача: собрать двоичный пакет из чисел и подписать его числовым идентификатором.
Преобразования примитивов встроены в язык, отдельный импорт для них не нужен:

```okkama
let identifier is string = string(101);
let packet is @byte = @byte(122, 11, 22);
let empty = @byte();
let whole is int = int(-3.9); // -3
```

`T(value)` возвращает одно значение; `@T(...)` — иммутабельный массив,
включая пустой и одноэлементный. Доступны `int`, `float`, `byte`, `string`.
Каждый элемент преобразуется по тем же правилам, что и одиночный аргумент.
`@byte(111, 256, 113)` завершается ошибкой на аргументе 2, частичного
результата нет. Дробные числа усекаются к нулю при преобразовании в `int`
и `byte`; диапазон проверяется после усечения. `float` округляется до
binary64, поэтому большие целые могут терять точность.

Если числа уже находятся в массиве, передайте их через явную распаковку:

```okkama
let nums is @int = [1, 2, 3, 4, 5];
let bytes = @byte(nums...);
let framed = @byte(0, nums..., 255);
```

Для обычного вариадического API распаковка передаётся последним позиционным
аргументом после фиксированных параметров; каждый элемент сохраняет строгую
проверку типа. В `@T(...)` разрешены несколько распаковок в любом месте.
Результат выражения тоже можно распаковать сразу: `out <- @string(1, 2, 3)... <- endl;`
пишет `123` и перевод строки. Массив вычисляется один раз, строки записываются
по порядку через обычный протокол IO.
[Правила и полный пример](array-spread.md) включают пустые массивы и генерики.

Для текста из файла или CLI выбирайте объект разбора из главы о числах;
для строкового содержимого сетевого пакета — кодировку из главы 5.
`int("101")` и `@byte("abc")` такие задачи не выполняют.
Полные [правила преобразований](OkkamaBook-RU.md#511-одиночные-преобразования-и-массивы-t)
и [запускаемый пример](../examples/primitive-casts.ok) показывают обе формы.

## Как запускать примеры

Сохраните выбранный блок в отдельный файл, например `recipe.ok`, и запускайте из корня репозитория на Perl 5.42:

```sh
OKKAMA_STDLIB_DIR="$PWD/std" ./okkama-pl/okkama recipe.ok
```

Для блоков с `test "..."` используйте тестовый режим:

```sh
OKKAMA_STDLIB_DIR="$PWD/std" ./okkama-pl/okkama -t recipe.ok
```

Файлы рецептов лучше хранить в отдельном каталоге. Импорты разрешаются относительно файла и затем через каталог `std`; собственные имена модулей приложения выбирайте явно. В сетевых и интерактивных рецептах условия запуска указаны перед кодом. `confirm` в примерах закрепляет ожидаемый результат.

### Четыре правила чтения кода

1. `let` связывает значение. Операции persistent-коллекции возвращают следующую версию, которую связывают с новым именем.
2. `#projected` позволяет краткую запись `value object` или `string object`; обычный вызов именованной проекции также виден в рецептах.
3. `async`-действие возвращает `Future`; `await` получает результат. Связанной фоновой работой владеет группа или scope.
4. Файл, соединение и сессия представляют внешний ресурс. `defer owner.close()` связывает освобождение с областью работы; для асинхронного завершения используются предусмотренные владельцем `settled`/`wait`.

## Содержание

- [1. Общие контракты и предметные значения](#chapter-basics)
- [2. Коллекции и преобразование данных](#chapter-collections)
- [3. Строки, Unicode и регулярные выражения](#chapter-text)
- [4. Числа, деньги и статистика](#chapter-math)
- [5. Кодировки и двоичные форматы](#chapter-encoding)
- [6. IO: потоки, буферы и консоль](#chapter-io)
- [7. Файлы, каталоги и наблюдение изменений](#chapter-files)
- [8. Операционная система и внешние программы](#chapter-system)
- [9. Командная строка и конфигурация](#chapter-config)
- [10. Интерактивный терминал](#chapter-terminal)
- [11. Даты, время, часовые пояса и календарь](#chapter-time)
- [12. Случайность и идентификаторы](#chapter-random)
- [13. Криптография, сертификаты и контроль целостности](#chapter-crypto)
- [14. JSON, CSV, TOML, plist, XML и HTML](#chapter-formats)
- [15. URL, параметры запроса и формы](#chapter-url)
- [16. Шаблоны писем, страниц и отчётов](#chapter-templates)
- [17. Архивы и потоковые преобразования](#chapter-archives)
- [18. Базы данных: SQL, SQLite, транзакции и пул](#chapter-database)
- [19. Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network)
- [20. HTTP: сообщения, поля и представление данных](#chapter-http-values)
- [21. HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client)
- [22. HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire)
- [23. HTTP-сервер, веб-приложение и потоковые события](#chapter-web)
- [24. Почта и MIME](#chapter-mail)
- [25. Локализация и переводы](#chapter-locale)
- [26. Асинхронность и параллельная обработка](#chapter-async)
- [27. Журналы событий](#chapter-logging)
- [28. Тесты, свойства, снимки и отчёты](#chapter-testing)
- [Внутренние модули и таблицы данных](#internal-modules)
- [Граница с runtime](#runtime-modules)
- [Алфавитный указатель всех модулей](#all-modules)
- [Проверка примеров](#verification)

<a id="chapter-basics"></a>

## 1. Общие контракты и предметные значения

Отказывайте в неверном вводе там, где известен предметный смысл. Маленькие роли позволяют алгоритму получить ровно нужное наблюдение: порядок, размер или текст ошибки.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`compare`](../std/compare.ok) | Упорядочить заявки по приоритету: реализуйте `Comparing<T>` с результатами Order.less, equal и greater. | `Order`, `Comparing` | [Рецепт](#recipe-fundamentals) |
| [`error`](../std/error.ok) | Отказать в обработке отрицательной цены: выберите InvalidArgument и передайте понятное сообщение. | `OutOfRange` | [Рецепт](#recipe-fundamentals) |
| [`size`](../std/size.ok) | Принять коллекцию любого вида в отчёт: зависите от Sizing и наблюдайте только size(). | `Sizing` | [Рецепт](#recipe-fundamentals) |

<a id="recipe-fundamentals"></a>

### 1.1. Проверить сумму и упорядочить счета

**Задача.** Предметное значение должно отвергать отрицательную сумму и сортироваться через узкий контракт.

**Какие библиотеки использовать.** [`error`](../std/error.ok), [`compare`](../std/compare.ok), [`size`](../std/size.ok), [`collections/ordering`](../std/collections/ordering.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Invoice сохраняет свой инвариант в конструкторе. Sorted использует `Comparing<Invoice>`, а Report зависит только от Sizing и наблюдает размер подготовленной подборки.

```okkama
module cookbook/fundamentals v1.0.0;
import "error" take InvalidArgument;
import "compare" take Comparing,Order;
import "size" take Sizing;
import "collections/ordering" take Sorted;
import "io/stdout" take Stdout;

object Invoice does Comparing<Invoice> {
    has total is int;
    Invoice(amount is int) {
        confirm amount >= 0 else throw InvalidArgument("сумма должна быть неотрицательной");
        total = amount;
    }
    amount() is int { total; }
    order(other is Invoice) is Order { total.order(other.amount()); }
}
object Report {
    has rows is Sizing;
    Report(rows is Sizing) { self.rows = rows; }
    heading() is string { "Счетов: " + rows.size().string(); }
}

main() is int {
    let invoices = Sorted(origin: [Invoice(120), Invoice(40)]);
    confirm invoices.first().amount() == 40;
    Stdout().writeln(Report(invoices).heading());
    0;
}
```

<a id="chapter-collections"></a>

## 2. Коллекции и преобразование данных

История заказа, очередь заданий и подборка адресатов требуют разных представлений. Выбирайте коллекцию по операциям: индекс, последовательный обход, минимум, принадлежность или добавление на одном из концов. В рецептах новые значения сохраняют прежние версии.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`collections/combinatorics`](../std/collections/combinatorics.ok) | Перебрать комплектации заказа: Combinations, Permutations и CartesianProduct строят ленивые варианты. | `Combinations`, `CombinationCursor`, `CombinationsWithReplacement`, `ReplacementCombinationCursor`, `Permutations` | [Рецепт](#recipe-views) |
| [`collections/combining`](../std/collections/combining.ok) | Сопоставить артикулы с остатками: Zipped связывает пары, StrictlyZipped проверяет равную длину, Chained объединяет обходы. | `Pair`, `Zipped`, `ZippedCursor`, `StrictlyZipped`, `StrictlyZippedCursor` | [Рецепт](#recipe-views) |
| [`collections/deque`](../std/collections/deque.ok) | Поставить срочную задачу в начало очереди: выбирайте front или back и получайте новую очередь через addition. | `Deque`, `DequeFront`, `DequeBack`, `DequeItems`, `ConcatenatedDequeCursor` | [Рецепт](#recipe-lists) |
| [`collections/enumeration`](../std/collections/enumeration.ok) | Показать номера строк с ошибками импорта: Enumerated добавляет стабильный индекс к каждому элементу. | `Indexed`, `Enumerated`, `EnumerationCursor`, `EnumerationOverflow` | [Рецепт](#recipe-views) |
| [`collections/frequencies`](../std/collections/frequencies.ok) | Посчитать обращения по категориям: Frequencies возвращает количество каждого значения в порядке первого появления. | `Frequency`, `Frequencies`, `FrequencyBuilding`, `FrequencyRevision` | [Рецепт](#recipe-lists) |
| [`collections/heap`](../std/collections/heap.ok) | Доставать ближайший дедлайн первым: `Heap<T>` хранит минимальный элемент по `Comparing<T>`. | `Heap`, `HeapBuilding`, `HeapRepresenting`, `HeapTreeNode`, `HeapChildren` | [Рецепт](#recipe-lists) |
| [`collections/iteration`](../std/collections/iteration.ok) | Обойти большую подборку заявок: `Iterating<T>` и `Cursor<T>` задают последовательный обход с отдельным successor. | `Cursor`, `Iterating`, `IterationEnded`, `CursorItems` | [Рецепт](#recipe-lists) |
| [`collections/list`](../std/collections/list.ok) | Сохранить историю действий: prefix создаёт новый List, а прежняя версия истории остаётся доступной. | `List`, `LinkedList`, `LinkedListCursor` | [Рецепт](#recipe-lists) |
| [`collections/ordering`](../std/collections/ordering.ok) | Отсортировать товары по цене: SortedBy вычисляет ключ, а LowerBound и UpperBound ищут диапазон одинаковых цен. | `Sorted`, `SortedBy`, `Ranked`, `PositionOf`, `LowerBound` | [Рецепт](#recipe-lists) |
| [`collections/repetition`](../std/collections/repetition.ok) | Повторить образец расписания: Repeated задаёт конечное повторение, Cycled обходит исходный цикл. | `Repeating`, `RepeatingCursor`, `Repeated`, `RepeatedCursor`, `Cycled` | [Рецепт](#recipe-views) |
| [`collections/sequence`](../std/collections/sequence.ok) | Прочитать строку таблицы по позиции: `Sequencing<T>` задаёт размер, item, первый и последний элементы. | `Sequencing` | [Рецепт](#recipe-lists) |
| [`collections/set`](../std/collections/set.ok) | Убрать повторные адреса рассылки: OrderedSet и операции множеств сохраняют детерминированный обход. | `Set`, `OrderedSet`, `Union`, `Intersection`, `Difference` | [Рецепт](#recipe-sets) |
| [`collections/views`](../std/collections/views.ok) | Читать записи до STOP и выбирать оплаченные: сочетайте TakenWhile, Filtered и Mapped над исходным обходом. | `CursorStep`, `CursorSteps`, `CursorStepCursor`, `Mapped`, `MappedCursorSearch` | [Рецепт](#recipe-views) |

### Что учитывать при сборке решения

- `collections/deque`: Выбирайте front/back по требуемому концу. Средний item(index) обходит элементы; прежние версии очереди сохраняются.
- `collections/frequencies`: Порядок частот — первое появление. Для d различных значений построение занимает O(n·d), поиск amount — O(d).
- `collections/heap`: top наблюдает минимум, remainder возвращает новую кучу. size и top выполняются за O(1), addition и remainder — за O(log n).
- `collections/ordering`: SortedBy вычисляет ключ для каждого входного элемента один раз. Поиск LowerBound/UpperBound применяйте к последовательности с тем же порядком.

<a id="recipe-lists"></a>

### 2.1. Сохранить прежнюю версию истории

**Задача.** В отчёте нужна история изменений; несколько ветвей обработки должны видеть свои версии.

**Какие библиотеки использовать.** [`collections/list`](../std/collections/list.ok), [`collections/sequence`](../std/collections/sequence.ok), [`collections/iteration`](../std/collections/iteration.ok), [`collections/deque`](../std/collections/deque.ok), [`collections/heap`](../std/collections/heap.ok), [`collections/ordering`](../std/collections/ordering.ok), [`collections/frequencies`](../std/collections/frequencies.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** prefix, addition и remainder возвращают новое значение. В примере очередь, куча и отсортированная последовательность — разные представления одной задачи; прежняя history остаётся доступной.

```okkama
module cookbook/lists v1.0.0;
import "collections/list" take List,EmptyList;
import "collections/sequence" take Sequencing;
import "collections/iteration" take Iterating;
import "collections/deque" take Deque;
import "collections/heap" take Heap;
import "collections/ordering" take Sorted;
import "collections/frequencies" take Frequencies;
import "io/stdout" take Stdout;

main() is int {
    let empty is List<int> = EmptyList();
    let history = empty.prefix(30).prefix(10).prefix(20);
    let sorted = Sorted(origin: history);
    let source is Sequencing<int> = sorted;
    let traversal is Iterating<int> = sorted;
    confirm source.item(0) == traversal.cursor().item();
    let queue = Deque(values: history);
    let urgent = queue.front().addition(5);
    let priorities = Heap(values: history).addition(5);
    confirm queue.size() == 3 && urgent.size() == 4;
    confirm priorities.top() == 5 && sorted.first() == 10;
    confirm history.first() == 20;
    let counts = Frequencies(values: ["оплата", "доставка", "оплата"]);
    confirm counts.amount("оплата") == 2;
    Stdout().writeln(counts.total().string());
    0;
}
```

<a id="recipe-views"></a>

### 2.2. Отбирать строки до маркера STOP

**Задача.** Нужно обработать только положительные суммы до завершающей записи и сохранить номера строк.

**Какие библиотеки использовать.** [`collections/views`](../std/collections/views.ok), [`collections/enumeration`](../std/collections/enumeration.ok), [`collections/combining`](../std/collections/combining.ok), [`collections/repetition`](../std/collections/repetition.ok), [`collections/combinatorics`](../std/collections/combinatorics.ok), [`strings/parse`](../std/strings/parse.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** TakenWhile прекращает обход на первом STOP. Mapped и Filtered передают значения дальше по цепочке. Enumerated нумерует уже отобранные суммы; для номеров исходного файла его следует поставить перед фильтром.

```okkama
module cookbook/views v1.0.0;
import "collections/views" take TakenWhile,Filtered,Mapped;
import "collections/enumeration" take Enumerated;
import "collections/combining" take Zipped;
import "collections/repetition" take Repeated;
import "collections/combinatorics" take Combinations;
import "strings/parse" take ParsedInt;
import "io/stdout" take Stdout;

main() is int {
    let rows = ["12", "0", "8", "STOP", "не читать"];
    let prefix = TakenWhile(origin: rows, condition: (row is string) is bool { row != "STOP"; });
    let amounts = Mapped(origin: prefix, mapping: (row is string) is int { ParsedInt(row).value(); });
    let paid = Filtered(origin: amounts, condition: (amount is int) is bool { amount > 0; });
    for Enumerated(origin: paid, start: 1) -> line {
        Stdout().writeln(line.index().string() + ": " + line.item().string());
    }
    for Zipped(left: ["A", "B"], right: [3, 7]) -> pair {
        Stdout().writeln(pair.first() + "=" + pair.second().string());
    }
    confirm Repeated(value: "-", count: 4).size() == 4;
    for Combinations(origin: ["чай", "кофе", "сок"], size: 2) -> pair {
        Stdout().writeln(pair.item(0) + " + " + pair.item(1));
    }
    0;
}
```

<a id="recipe-sets"></a>

### 2.3. Убрать повторные адреса рассылки

**Задача.** Отделы прислали списки адресатов; каждому получателю нужно отправить одно письмо.

**Какие библиотеки использовать.** [`collections/set`](../std/collections/set.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** OrderedSet сохраняет детерминированный порядок. Union формирует объединение адресатов, Intersection выбирает общих получателей; источники остаются прежними.

```okkama
module cookbook/sets v1.0.0;
import "collections/set" take OrderedSet,Union,Intersection;
import "io/stdout" take Stdout;

main() is int {
    let sales = OrderedSet(values: ["ada@example.test", "bob@example.test", "ada@example.test"]);
    let support = OrderedSet(values: ["bob@example.test", "eve@example.test"]);
    confirm sales.size() == 2;
    for Union(left: sales, right: support) -> address { Stdout().writeln(address); }
    confirm Intersection(left: sales, right: support).membership("bob@example.test");
    0;
}
```

<a id="chapter-text"></a>

## 3. Строки, Unicode и регулярные выражения

Текст поступает из форм, команд и файлов. Сначала определите, какие части имеют предметный смысл: число, имя, строка отчёта либо совпадение шаблона. Для каждой операции выберите явное представление.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`regex/escape`](../std/regex/escape.ok) | Найти пользовательский текст буквально: EscapedRegex превращает метасимволы в литералы шаблона. | `EscapedRegex` | [Рецепт](#recipe-regex) |
| [`regex/match`](../std/regex/match.ok) | Прочитать группы из найденного артикула: RegexMatching и RegexCapture сохраняют текст и координаты захватов. | `RegexMatching`, `RegexMatch`, `RegexCapture`, `UnmatchedRegexCapture` | [Рецепт](#recipe-regex) |
| [`regex/pattern`](../std/regex/pattern.ok) | Задать формат артикула: Regex связывает Dialect 1, флаги и бюджет сопоставления. | `RegexFlag`, `RegexLimit`, `Regex`, `RegexQuery`, `RegexMatchObservation` | [Рецепт](#recipe-regex) |
| [`regex/range`](../std/regex/range.ok) | Ограничить поиск одной колонкой текста: RegexRange задаёт полуоткрытый интервал Unicode-позиций. | `RegexRange` | [Рецепт](#recipe-regex) |
| [`regex/replace`](../std/regex/replace.ok) | Скрыть номера в журнале: RegexSubstitution использует буквальную либо раскрываемую замену. | `ReplacingRegexMatch`, `LiteralRegexReplacement`, `ExpandedRegexReplacement`, `ReplacementToken`, `ReplacementTokens` | [Рецепт](#recipe-regex) |
| [`regex/search`](../std/regex/search.ok) | Найти номер заказа в строке: FirstRegexMatch, EntireRegexMatch и RegexMatches выбирают режим поиска. | `FirstRegexMatch`, `PrefixRegexMatch`, `EntireRegexMatch`, `RegexMatches`, `RegexMatchCursor` | [Рецепт](#recipe-regex) |
| [`regex/split`](../std/regex/split.ok) | Разобрать список с неодинаковыми пробелами: RegexParts обходит части по регулярному разделителю. | `RegexParts`, `RegexPartsCursor` | [Рецепт](#recipe-regex) |
| [`strings/format`](../std/strings/format.ok) | Собрать короткое уведомление с подстановками: Formatted связывает шаблон и значения слотов {}. | `Formatted` | [Рецепт](#example-rendering) |
| [`strings/join`](../std/strings/join.ok) | Собрать строку адресатов через запятую: Joined фиксирует строки, разделитель и бюджет результата. | `Joined`, `JoinedParts`, `JoinLimit` | [Рецепт](#example-joined) |
| [`strings/parse`](../std/strings/parse.ok) | Разобрать число из аргумента команды: ParsedInt, ParsedFloat, ParsedRat, ParsedBool и ParsedByte выполняют явную проекцию. | `ParsedInt`, `ParsedFloat`, `ParsedRat`, `ParsedBool`, `ParsedByte` | [Рецепт](#recipe-views) |
| [`text/layout`](../std/text/layout.ok) | Подготовить текст справки: Indented, Dedented, Wrapped и Shortened управляют отступами и длиной строк. | `Indented`, `Dedented`, `Wrapped`, `Shortened` | [Рецепт](#recipe-text-unicode) |
| [`unicode/character`](../std/unicode/character.ok) | Классифицировать символ из импортированного имени: UnicodeCharacter наблюдает свойства одного Unicode scalar. | `UnicodeCategory`, `UnicodeCharacter`, `NonNumericCharacter` | [Рецепт](#recipe-text-unicode) |
| [`unicode/normalization`](../std/unicode/normalization.ok) | Сопоставить разные Unicode-записи одного имени: Normalized строит выбранную нормальную форму. | `NormalizationForm`, `Normalized` | [Рецепт](#recipe-text-unicode) |

<a id="recipe-regex"></a>

### 3.1. Извлечь номер заказа и скрыть цифры

**Задача.** В строке журнала требуется найти номер заказа, затем подготовить обезличенную копию.

**Какие библиотеки использовать.** [`regex/pattern`](../std/regex/pattern.ok), [`regex/search`](../std/regex/search.ok), [`regex/replace`](../std/regex/replace.ok), [`regex/escape`](../std/regex/escape.ok), [`regex/split`](../std/regex/split.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** RegexMatching предоставляет capture/group и абсолютный RegexRange. Буквальная замена сохраняет текст замены буквально, а EscapedRegex нужен при включении пользовательской строки в шаблон.

```okkama
module cookbook/regex v1.0.0;
import "regex/pattern" take Regex;
import "regex/search" take FirstRegexMatch,RegexMatches;
import "regex/replace" take RegexSubstitution,LiteralRegexReplacement;
import "regex/escape" take EscapedRegex;
import "regex/split" take RegexParts;
import "io/stdout" take Stdout;

main() is int {
    let source = "order=4711; amount=120";
    let number = FirstRegexMatch(source, Regex("order=(?<id>\\d+)"));
    confirm number.group("id").text() == "4711";
    let safe = RegexSubstitution(source, Regex("\\d+"), LiteralRegexReplacement("***")).string();
    Stdout().writeln(safe);
    let literal = Regex(EscapedRegex("a+b").string());
    confirm RegexMatches("a+b", literal).found();
    for RegexParts("чай, кофе;сок", Regex("[,;]\\s*")) -> part {
        Stdout().writeln(part);
    }
    0;
}
```

<a id="recipe-text-unicode"></a>

### 3.2. Подготовить имена и текст справки

**Задача.** Импорт содержит составные Unicode-символы; справку нужно разбить на удобные строки.

**Какие библиотеки использовать.** [`unicode/normalization`](../std/unicode/normalization.ok), [`unicode/character`](../std/unicode/character.ok), [`text/layout`](../std/text/layout.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Нормализация меняет представление строки по явно выбранной форме. Свойства UnicodeCharacter относятся к одному scalar; Wrapped форматирует текст по словам.

```okkama
module cookbook/text_unicode v1.0.0;
import "unicode/normalization" take Normalized,NormalizationForm;
import "unicode/character" take UnicodeCharacter;
import "text/layout" take Wrapped,Indented;
import "io/stdout" take Stdout;

main() is int {
    let name = Normalized(origin: "é", form: NormalizationForm.nfc).string();
    confirm name == "é";
    let letter = UnicodeCharacter(origin: 'Ж');
    Stdout().writeln(letter.name());
    let help = Wrapped(origin: "Импортировать товары из проверенного CSV файла", width: 24).string();
    Stdout().writeln(Indented(origin: help, prefix: "  ").string());
    0;
}
```

<a id="example-rendering"></a>

### 3.3. Собрать уведомление и JSON-сумму заказа

**Задача.** Собрать короткое уведомление с подстановками: Formatted связывает шаблон и значения слотов {}.

**Какие библиотеки использовать.** [`strings/format`](../std/strings/format.ok), [`json/value`](../std/json/value.ok), [`json/write`](../std/json/write.ok), [`math/decimal`](../std/math/decimal.ok).

**Как использовать.** Formatted получает строки для слотов, затем JsonObject связывает сообщение с точным Decimal. CompactJson сериализует готовое дерево.

```okkama
module example v1.0.0;

import "strings/format" take Formatted;
import "json/value" take JsonObject, JsonMember, JsonText, JsonNumber;
import "json/write" take CompactJson;
import "math/decimal" take Decimal;

object ExampleResult {
    ExampleResult();
    value() is string {
        let greeting = Formatted(pattern: "Hello, {}!", values: ["Ada"]);
        CompactJson(origin: JsonObject(members: [
            JsonMember(name: "greeting", value: JsonText(greeting.string())),
            JsonMember(name: "amount", value: JsonNumber(Decimal("12.50"))),
        ])).string();
    }
}

main() is int {
    ExampleResult().value();
    return 0;
}
```

[Исходный пример](../examples/std-v4/rendering.ok).

<a id="example-joined"></a>

### 3.4. Собрать список адресатов через разделитель

**Задача.** Собрать строку адресатов через запятую: Joined фиксирует строки, разделитель и бюджет результата.

**Какие библиотеки использовать.** [`strings/join`](../std/strings/join.ok), [`collections/list`](../std/collections/list.ok).

**Как использовать.** Передайте разделитель и источник строк в Joined. Наблюдайте string() после создания ограниченного снимка.

```okkama
module app v1.0.0;
import "strings/join" take Joined;
import "collections/list" take List, EmptyList;
object ExampleResult {
    ExampleResult();
    value() is string {
        let empty is List<string> = EmptyList();
        let names = empty.prefix("Grace").prefix("Ada");
        string Joined(separator: " / ", values: names, maximum_items: 2, maximum_length: 11);
    }
}

main() is int {
    ExampleResult().value();
    return 0;
}
```

[Исходный пример](../examples/std-v4/joined.ok).

<a id="chapter-math"></a>

## 4. Числа, деньги и статистика

Суммы платежей, большие счётчики и измерения имеют разные требования к точности. Decimal подходит для десятичного представления, BigInt — для больших целых, BigRational — для точных отношений. Статистика принимает конечные float-значения.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`math/aggregate`](../std/math/aggregate.ok) | Подвести итог измерений: Sum, AccurateSum, Minimum и Maximum вычисляют агрегаты по источнику значений. | `Minimum`, `Maximum`, `Sum`, `AccurateSum`, `AccurateTotal` | [Рецепт](#recipe-measurements) |
| [`math/constants`](../std/math/constants.ok) | Рассчитать длину окружности: Pi, Tau и Euler предоставляют именованные математические значения. | `Pi`, `Tau`, `Euler` | [Рецепт](#recipe-measurements) |
| [`math/decimal`](../std/math/decimal.ok) | Хранить цену и рассчитывать округление: Decimal сохраняет десятичные коэффициент и показатель степени. | `Decimal` | [Рецепт](#recipe-decimal-price) |
| [`math/rounding`](../std/math/rounding.ok) | Согласовать правило округления суммы: Rounding явно выбирает half_even, half_up и направленные режимы. | `Rounding` | [Рецепт](#recipe-decimal-price) |
| [`numbers/bigint`](../std/numbers/bigint.ok) | Сложить счётчики больше машинного int: BigInt и BigSum сохраняют точные целые произвольной длины. | `BigInt`, `BigDifference`, `BigSum`, `BigProduct`, `BigQuotient` | [Рецепт](#example-bigrational) |
| [`numbers/bigrational`](../std/numbers/bigrational.ok) | Разделить общий расход точно: BigRational и операции над ним хранят несократимую дробь. | `BigRational`, `BigRationalSum`, `BigRationalDifference`, `BigRationalProduct`, `BigRationalQuotient` | [Рецепт](#example-bigrational) |
| [`numbers/float`](../std/numbers/float.ok) | Записать измерение компактной десятичной строкой: ShortestFloat выбирает представление binary64. | `ShortestFloat`, `Binary64RoundingInterval`, `BinaryPowerRatio`, `FloatDecimalCandidate` | [Рецепт](#recipe-decimal-price) |
| [`statistics`](../std/statistics.ok) | Описать время ответа сервиса: Mean, Median, Quantiles и другие наблюдения работают над выборкой float. | `QuantileMethod`, `FloatSample`, `Mean`, `GeometricMean`, `HarmonicMean` | [Рецепт](#recipe-measurements) |

<a id="recipe-measurements"></a>

### 4.1. Посчитать среднее и медиану времени ответа

**Задача.** Нужно включить статистику времени ответа в дневной отчёт.

**Какие библиотеки использовать.** [`math/aggregate`](../std/math/aggregate.ok), [`math/constants`](../std/math/constants.ok), [`statistics`](../std/statistics.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Среднее учитывает все значения, медиана характеризует середину отсортированной выборки. Режим inclusive явно выбирает трактовку границ квантилей.

```okkama
module cookbook/measurements v1.0.0;
import "math/aggregate" take AccurateSum,Maximum;
import "math/constants" take Pi;
import "statistics" take Mean,Median,Quantiles,QuantileMethod;
import "io/stdout" take Stdout;

main() is int {
    let elapsed = [10.0, 20.0, 20.0, 100.0];
    Stdout().writeln("сумма: " + AccurateSum(values: elapsed).value().string());
    Stdout().writeln("среднее: " + Mean(values: elapsed).value().string());
    Stdout().writeln("медиана: " + Median(values: elapsed).value().string());
    confirm Maximum(values: elapsed).value() == 100.0;
    let quartiles = Quantiles(values: elapsed, count: 4, method: QuantileMethod.inclusive).values();
    confirm quartiles.size() == 3;
    confirm Pi().value() > 3.14;
    0;
}
```

<a id="recipe-decimal-price"></a>

### 4.2. Показать цену с выбранным округлением

**Задача.** В печатном отчёте цена должна иметь две десятичные цифры по правилу half_even.

**Какие библиотеки использовать.** [`math/decimal`](../std/math/decimal.ok), [`math/rounding`](../std/math/rounding.ok), [`locale/tag`](../std/locale/tag.ok), [`locale/number`](../std/locale/number.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Decimal сохраняет исходное точное десятичное значение. NumberPrecision задаёт число дробных цифр и округление, а LocalizedDecimal применяет выбранное локализованное представление.

```okkama
module cookbook/decimal_price v1.0.0;
import "math/decimal" take Decimal;
import "math/rounding" take Rounding;
import "locale/tag" take Locale;
import "locale/number" take LocalizedDecimal,NumberFormat,NumberNotation,NumberGrouping,NumberSign,NumberPrecision;
import "io/stdout" take Stdout;

main() is int {
    let price = Decimal("1234.565");
    let format = NumberFormat(NumberNotation.standard, NumberGrouping.automatic, NumberSign.automatic, NumberPrecision(1, 2, 2, Rounding.half_even));
    let text = LocalizedDecimal(price, Locale("en"), format).string();
    confirm text == "1,234.56";
    Stdout().writeln(text);
    0;
}
```

<a id="example-bigrational"></a>

### 4.3. Сохранить точное отношение сумм

**Задача.** Сложить счётчики больше машинного int: BigInt и BigSum сохраняют точные целые произвольной длины.

**Какие библиотеки использовать.** [`numbers/bigint`](../std/numbers/bigint.ok), [`numbers/bigrational`](../std/numbers/bigrational.ok).

**Как использовать.** BigRational получает числитель и знаменатель как BigInt. Арифметические представления возвращают каноническую дробь через value().

```okkama
module app v1.0.0;
import "numbers/bigint" take BigInt;
import "numbers/bigrational" take BigRational, BigRationalSum;
object ExampleResult {
    ExampleResult();
    value() is string {
        let third = BigRational(BigInt("1"), BigInt("3"));
        let sixth = BigRational(BigInt("1"), BigInt("6"));
        BigRationalSum(left: third, right: sixth).value().string();
    }
}

main() is int {
    ExampleResult().value();
    return 0;
}
```

[Исходный пример](../examples/std-v4/bigrational.ok).

<a id="chapter-encoding"></a>

## 5. Кодировки и двоичные форматы

На внешней границе нужны байты. Выбирайте отдельно кодировку текста, текстовую упаковку байтов и числовой wire-формат. Эти три решения выражаются разными объектами.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`encoding/base32`](../std/encoding/base32.ok) | Выдать удобный буквенный код из байтов: Base32Encoded использует алфавит RFC 4648. | `Base32Encoded`, `Base32Decoded` | [Рецепт](#recipe-encodings) |
| [`encoding/base64`](../std/encoding/base64.ok) | Передать байты в текстовом поле API: Base64Encoded и Base64Decoded выбирают обычный или URL-safe алфавит. | `Base64Alphabet`, `Base64Encoded`, `Base64Decoded` | [Рецепт](#recipe-text-bytes) |
| [`encoding/base85`](../std/encoding/base85.ok) | Упаковать двоичные данные в компактный текст: Base85Encoded явно выбирает ASCII85 или RFC 1924. | `Base85Alphabet`, `Base85Encoded`, `Base85Decoded` | [Рецепт](#recipe-encodings) |
| [`encoding/binary`](../std/encoding/binary.ok) | Прочитать числовое поле бинарного протокола: IntegerFromBytes и FloatFromBytes фиксируют ширину и порядок байтов. | `Signedness`, `FloatWidth`, `IntegerFromBytes`, `IntegerBytes`, `FloatFromBytes` | [Рецепт](#recipe-encodings) |
| [`encoding/hex`](../std/encoding/hex.ok) | Показать двоичный идентификатор в журнале: HexEncoded выдаёт каноническую hex-строку, HexDecoded восстанавливает байты. | `HexEncoded`, `HexDecoded` | [Рецепт](#recipe-text-bytes) |
| [`encoding/pem`](../std/encoding/pem.ok) | Прочитать текстовый контейнер сертификата: PemBlocks разбирает подписанные метками блоки, PemEncoded сериализует их. | `PemEncoded`, `PemBlock`, `PemBlocks`, `PemSourcesCursor`, `PemBlockCursor` | [Рецепт](#example-certificate-observers) |
| [`encoding/quoted_printable`](../std/encoding/quoted_printable.ok) | Передать письмо через текстовый транспорт: QuotedPrintableEncoded и Decoded преобразуют байты MIME. | `QuotedPrintableEncoded`, `QuotedPrintableDecoded` | [Рецепт](#recipe-imap-assignment) |
| [`encoding/single_byte`](../std/encoding/single_byte.ok) | Обменяться текстом с ASCII или Latin-1 системой: выбирайте соответствующую пару Encoded и Decoded. | `AsciiEncoded`, `AsciiDecoded`, `Latin1Encoded`, `Latin1Decoded`, `UnrepresentableAsciiCharacter` | [Рецепт](#example-hpack-primitives) |
| [`encoding/utf`](../std/encoding/utf.ok) | Прочитать Unicode-текст из байтов: Utf8Decoded и UTF-16/32-кодеки делают кодировку и порядок байтов явными. | `ByteOrder`, `Utf8Encoded`, `Utf8Decoded`, `Utf16Encoded`, `Utf16Decoded` | [Рецепт](#recipe-text-bytes) |

<a id="recipe-encodings"></a>

### 5.1. Перенести двоичный идентификатор в текст

**Задача.** Пакет содержит двухбайтовое поле и байтовый идентификатор, который нужно передать текстом.

**Какие библиотеки использовать.** [`encoding/binary`](../std/encoding/binary.ok), [`encoding/utf`](../std/encoding/utf.ok), [`encoding/base32`](../std/encoding/base32.ok), [`encoding/base85`](../std/encoding/base85.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Числовой wire-формат задаётся шириной и ByteOrder. Base32/Base85 меняют представление тех же байтов; декодирование восстанавливает packet.

```okkama
module cookbook/encodings v1.0.0;
import "encoding/binary" take IntegerBytes,IntegerFromBytes,Signedness;
import "encoding/utf" take ByteOrder;
import "encoding/base32" take Base32Encoded,Base32Decoded;
import "encoding/base85" take Base85Encoded,Base85Decoded,Base85Alphabet;
import "io/stdout" take Stdout;

main() is int {
    let packet = IntegerBytes(value: 513, size: 2, signedness: Signedness.unsigned, order: ByteOrder.big).bytes();
    confirm packet == [2.byte(), 1.byte()];
    confirm IntegerFromBytes(packet, Signedness.unsigned, ByteOrder.big).value() == 513;
    let token = Base32Encoded(packet).string();
    confirm Base32Decoded(token).bytes() == packet;
    let compact = Base85Encoded(packet, Base85Alphabet.ascii85).string();
    confirm Base85Decoded(compact, Base85Alphabet.ascii85).bytes() == packet;
    Stdout().writeln(token);
    0;
}
```

<a id="recipe-text-bytes"></a>

### 5.2. Передать Unicode-текст в байтовом поле API

**Задача.** Имя клиента нужно превратить в UTF-8 bytes, упаковать в Base64 и восстановить на другой стороне.

**Какие библиотеки использовать.** [`encoding/utf`](../std/encoding/utf.ok), [`encoding/base64`](../std/encoding/base64.ok), [`encoding/hex`](../std/encoding/hex.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** UTF-8 связывает текст с байтами. Base64 и hex представляют те же байты текстом. Эти шаги разделены, поэтому кодировка исходного текста видна в месте обмена.

```okkama
module cookbook/text_bytes v1.0.0;
import "encoding/utf" take Utf8Encoded,Utf8Decoded;
import "encoding/base64" take Base64Encoded,Base64Decoded;
import "encoding/hex" take HexEncoded;
import "io/stdout" take Stdout;

main() is int {
    let original = "Ада";
    let bytes = Utf8Encoded(original).bytes();
    let transfer = Base64Encoded(bytes).string();
    let restored = Utf8Decoded(Base64Decoded(transfer).bytes()).string();
    confirm restored == original;
    Stdout().writeln("base64: " + transfer);
    Stdout().writeln("hex: " + HexEncoded(bytes).string());
    0;
}
```

<a id="chapter-io"></a>

## 6. IO: потоки, буферы и консоль

Алгоритм чтения принимает Reading, алгоритм записи — Writing или WritingText. Владельца ресурса держите в области выполнения и закрывайте через defer; байтовое чтение завершается пустым блоком, текстовый ввод имеет собственное завершение.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`io/buffer`](../std/io/buffer.ok) | Собрать небольшой двоичный пакет в памяти: ByteBuffer предоставляет отдельные reading, writing и content. | `ByteBuffer`, `BufferReading`, `BufferWriting`, `BufferClosing`, `BufferContent` | [Рецепт](#recipe-buffer-output) |
| [`io/decorators`](../std/io/decorators.ok) | Ограничить входящий документ по байтам: LimitedReading, BufferedReading и ChainedReading составляют поток из декораторов. | `BufferedReading`, `LimitedReading`, `ChainedReading`, `ByteSnapshot`, `TeeWriting` | [Рецепт](#example-mime-transfer) |
| [`io/stdin`](../std/io/stdin.ok) | Считать строку из консольного ввода: Stdin и Utf8Input связывают стандартный поток с UTF-8 чтением. | `Stdin`, `Utf8Input` | [Рецепт](#recipe-stdin) |
| [`io/stdout`](../std/io/stdout.ok) | Вывести результат и диагностику раздельно: Stdout и Stderr предоставляют стандартные выходные потоки. | `Stdout`, `Stderr` | [Рецепт](#recipe-stdin) |
| [`io/stream`](../std/io/stream.ok) | Принять файл или буфер одним алгоритмом: Reading, Writing, ByteSource и Closing ограничивают нужные возможности. | `ByteSource`, `Writing`, `Reading`, `WritingText`, `ReadingText` | [Рецепт](#example-smtp-reading) |
| [`io/text`](../std/io/text.ok) | Писать Unicode-текст в байтовый приёмник: Utf8Output кодирует его явно через Writing. | `Utf8Output` | [Рецепт](#recipe-buffer-output) |

<a id="recipe-stdin"></a>

### 6.1. Прочитать имена из stdin и вывести приветствия

**Задача.** Утилита получает по одному имени на строку из терминала либо конвейера.

**Какие библиотеки использовать.** [`io/stdin`](../std/io/stdin.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** line снимает разделитель строки. Завершение входа выражено EndOfInput. Оператор <- последовательно пишет части, а endl задаёт перевод строки.

```okkama
module cookbook/stdin v1.0.0;
import "io/stdin" take Stdin,Utf8Input,EndOfInput;
import "io/stdout" take Stdout;

main() is int {
    let input = Utf8Input(origin: Stdin());
    defer input.close();
    let out = Stdout();
    loop {
        try {
            let name = input.line();
            out <- "Здравствуйте, " <- name <- endl;
        }
        catch ended is EndOfInput { last; }
    }
    0;
}
```

<a id="recipe-buffer-output"></a>

### 6.2. Собрать UTF-8 документ в памяти

**Задача.** Перед отправкой небольшого отчёта нужно получить готовые bytes из нескольких текстовых записей.

**Какие библиотеки использовать.** [`io/buffer`](../std/io/buffer.ok), [`io/text`](../std/io/text.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ByteBuffer владеет памятью. Utf8Output получает только Writing и преобразует текст в UTF-8. content().bytes() возвращает снимок подготовленного документа.

```okkama
module cookbook/buffer_output v1.0.0;
import "io/buffer" take ByteBuffer;
import "io/text" take Utf8Output;
import "encoding/utf" take Utf8Decoded;
import "io/stdout" take Stdout;

main() is int {
    let buffer = ByteBuffer();
    defer buffer.closing().close();
    let output = Utf8Output(origin: buffer.writing());
    output.writeln("Заказ 7");
    output.write("Готов к выдаче");
    let document = buffer.content().bytes();
    Stdout().writeln(Utf8Decoded(document).string());
    0;
}
```

<a id="chapter-files"></a>

## 7. Файлы, каталоги и наблюдение изменений

Путь — значение, открытый файл — ресурс. Собирайте пути отдельно от чтения и записи. Временная рабочая область удобна для импорта, теста и промежуточного отчёта; корневые capability полезны для обработки внешних имён.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`fs/directory`](../std/fs/directory.ok) | Обойти каталог загрузок: Directory, DirectoryEntries и Descendants представляют каталог и его содержимое. | `DirectoryLocating`, `Directory`, `DirectoryEntry`, `DirectoryEntries`, `DirectoryEntryCursor` | [Рецепт](#recipe-file-discovery) |
| [`fs/file`](../std/fs/file.ok) | Сохранить счёт и прочитать его обратно: File связывает путь, режим и байтовое либо текстовое представление. | `FileMode`, `FileLocking`, `File`, `FileBytes`, `FileText` | [Рецепт](#recipe-file-report) |
| [`fs/glob`](../std/fs/glob.ok) | Найти месячные CSV-отчёты: MatchingPaths выбирает пути по шаблону относительно заданного корня. | `MatchingPaths`, `MatchingPathCursor` | [Рецепт](#recipe-file-discovery) |
| [`fs/link`](../std/fs/link.ok) | Создать дополнительное имя для локального файла: SymbolicLink и HardLink выполняют явную операцию с ссылкой. | `SymbolicLink`, `HardLink`, `NotSymbolicLink` | [Рецепт](#recipe-imap-assignment) |
| [`fs/lock`](../std/fs/lock.ok) | Сериализовать запись в общий локальный файл: FileLock задаёт режим и политику ожидания advisory-блокировки. | `FileLockMode`, `FileLockWaiting`, `FileLock`, `FileLockModeCode`, `FileLockWaitingCode` | [Рецепт](#recipe-imap-assignment) |
| [`fs/path`](../std/fs/path.ok) | Построить путь к отчёту: Path соединяет части переносимо и работает с явной грамматикой путей и дочерними компонентами. | `PathStyle`, `Path`, `NativePathStyle`, `PathStyleCode` | [Рецепт](#recipe-file-discovery) |
| [`fs/root`](../std/fs/root.ok) | Раздавать файлы относительно разрешённого каталога: FileRoot и RootedEntry удерживают файловые дескрипторы корня и детей. | `FileRoot`, `RootedEntry` | [Рецепт](#example-http-static) |
| [`fs/stat`](../std/fs/stat.ok) | Проверить размер и права файла перед обработкой: FileStat, Permissions и FileTimes описывают наблюдение метаданных. | `FileKind`, `FileStat`, `FileTimes`, `FileIdentity`, `Permissions` | [Рецепт](#recipe-logging-file) |
| [`fs/temp`](../std/fs/temp.ok) | Создать рабочую область импорта: TemporaryFile и TemporaryDirectory владеют временными ресурсами. | `TemporaryFile`, `TemporaryFilePath`, `TemporaryFilePreservation`, `TemporaryDirectory` | [Рецепт](#recipe-file-report) |
| [`fs/watch`](../std/fs/watch.ok) | Заметить обновление конфигурации: FileWatch выдаёт FileEvent для наблюдаемого пути. | `FileEventKind`, `FileEvent`, `FileWatch` | [Рецепт](#recipe-file-discovery) |

<a id="recipe-file-discovery"></a>

### 7.1. Найти CSV и дождаться изменения каталога

**Задача.** Папка обмена получает новые выгрузки, которые требуется обнаруживать по расширению.

**Какие библиотеки использовать.** [`fs/glob`](../std/fs/glob.ok), [`fs/watch`](../std/fs/watch.ok), [`fs/directory`](../std/fs/directory.ok), [`fs/path`](../std/fs/path.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Сначала MatchingPaths выдаёт отсортированный снимок существующих путей. Затем event ожидает изменение наблюдаемого каталога: после запуска создайте или измените файл в нём.

```okkama
module cookbook/file_discovery v1.0.0;
import "fs/glob" take MatchingPaths;
import "fs/watch" take FileWatch;
import "fs/directory" take Directory;
import "fs/path" take Path;
import "io/stdout" take Stdout;

async main() is int {
    let incoming = Directory(Path("."));
    for MatchingPaths(root: incoming, pattern: "**/*.csv") -> path {
        Stdout().writeln(path.string());
    }
    let watch = FileWatch(path: incoming.path());
    defer watch.close();
    let event = await watch.event();
    Stdout().writeln(event.path().string());
    0;
}
```

<a id="recipe-file-report"></a>

### 7.2. Сохранить текстовый отчёт и прочитать его обратно

**Задача.** После расчёта нужно записать UTF-8 файл и проверить его содержимое.

**Какие библиотеки использовать.** [`fs/path`](../std/fs/path.ok), [`fs/file`](../std/fs/file.ok), [`fs/directory`](../std/fs/directory.ok), [`fs/temp`](../std/fs/temp.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Сначала явно закройте writer, затем откройте файл для чтения. Текстовая capability работает с выбранным файловым ресурсом. Временный каталог ограничивает время жизни промежуточного отчёта.

```okkama
module cookbook/file_report v1.0.0;
import "fs/path" take Path;
import "fs/file" take File,FileMode;
import "fs/directory" take Directory;
import "fs/temp" take TemporaryDirectory;
import "io/stdout" take Stdout;

main() is int {
    let scratch = TemporaryDirectory(Directory(Path(".")), "cookbook-report-");
    defer scratch.close();
    let path = scratch.path().child("report.txt");
    let output = File(mode: FileMode.writing, path: path);
    output.text().writeln("Заказов: 7");
    output.close();
    let input = File(mode: FileMode.reading, path: path);
    defer input.close();
    let text = input.text().line();
    confirm text == "Заказов: 7";
    Stdout().writeln(text);
    0;
}
```

<a id="chapter-system"></a>

## 8. Операционная система и внешние программы

Окружение, рабочий каталог и argv становятся явными зависимостями команды. Для результата небольшого процесса удобно захватить stdout и stderr; для длительного процесса используется отдельный владелец и его потоковые capability.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`os/account`](../std/os/account.ok) | Найти домашний каталог учётной записи: User, Group и GroupUsers представляют системные аккаунты. | `User`, `Group`, `GroupUsers`, `GroupUserCursor` | [Рецепт](#recipe-system) |
| [`os/command`](../std/os/command.ok) | Запустить программу с точными аргументами: Command описывает исполняемый файл и последовательность argv. | `Commanding`, `Command`, `InheritedCommand` | [Рецепт](#recipe-commands) |
| [`os/command_capture`](../std/os/command_capture.ok) | Прочитать вывод конвертера документов: CommandCapture собирает ограниченные stdout и stderr вместе с результатом завершения. | `CapturedStream`, `CommandCapture`, `CompletedCommand` | [Рецепт](#recipe-commands) |
| [`os/directory`](../std/os/directory.ok) | Разрешить пользовательский относительный путь: WorkingDirectory наблюдает рабочий каталог процесса. | `WorkingDirectory` | [Рецепт](#recipe-system) |
| [`os/environment`](../std/os/environment.ok) | Получить настройки запуска: Environment фиксирует снимок переменных, HomeDirectory и SearchPath дают предметные проекции. | `Environment`, `HomeDirectory`, `SearchPath`, `SearchPathCursor` | [Рецепт](#recipe-proxy-environment) |
| [`os/pipeline_capture`](../std/os/pipeline_capture.ok) | Передать вывод одной программы другой: PipelineCapture соединяет команды и ограничивает захваченные данные. | `PipelineStream`, `PipelineCapture`, `CompletedPipeline` | [Рецепт](#recipe-pipeline) |
| [`os/platform`](../std/os/platform.ok) | Подписать диагностический отчёт сведениями о машине: Platform и Host предоставляют явные наблюдения среды. | `OperatingSystem`, `Platform`, `Host` | [Рецепт](#recipe-system) |
| [`os/process`](../std/os/process.ok) | Управлять длительной внешней командой: Process владеет запуском, потоками, ожиданием exit и сигналами. | `Signal`, `ExitKind`, `Exit`, `Process`, `ProcessInput` | [Рецепт](#recipe-commands) |
| [`os/storage`](../std/os/storage.ok) | Оценить место для выгрузки архива: Storage и StorageUsage наблюдают ёмкость и доступное пространство тома. | `Storage`, `StorageUsage` | [Рецепт](#recipe-system) |

<a id="recipe-commands"></a>

### 8.1. Получить вывод внешнего конвертера

**Задача.** Команда должна вернуть stdout, stderr и статус, а каждый аргумент должен сохранить свою границу.

**Какие библиотеки использовать.** [`os/command`](../std/os/command.ok), [`os/command_capture`](../std/os/command_capture.ok), [`fs/path`](../std/fs/path.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Command хранит отдельные argv. CommandCapture одновременно обслуживает потоки дочернего процесса и возвращает CompletedCommand после их завершения.

**Запуск.** В примере используются пути к стандартным POSIX-программам; для своей системы задайте пути к тем же командам.

```okkama
module cookbook/commands v1.0.0;
import "os/command" take InheritedCommand;
import "os/command_capture" take CommandCapture;
import "fs/path" take Path;
import "encoding/utf" take Utf8Decoded;
import "io/stdout" take Stdout;

async main() is int {
    let command = InheritedCommand(program: Path("/usr/bin/printf"), arguments: ["%s", "Отчёт готов"]);
    let completed = await CommandCapture(origin: command, limit: 4096).completion();
    Stdout().writeln(Utf8Decoded(completed.output()).string());
    0;
}
```

<a id="recipe-system"></a>

### 8.2. Собрать сведения для локального диагностического отчёта

**Задача.** Пользователь прислал проблему с импортом; в отчёт нужны архитектура и текущий каталог.

**Какие библиотеки использовать.** [`os/platform`](../std/os/platform.ok), [`os/directory`](../std/os/directory.ok), [`os/account`](../std/os/account.ok), [`os/storage`](../std/os/storage.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Platform — снимок сведений платформы. User предоставляет предметное наблюдение учётной записи; Storage привязывается к пути интересующего тома.

```okkama
module cookbook/system v1.0.0;
import "os/platform" take Platform;
import "os/directory" take WorkingDirectory;
import "os/account" take User;
import "os/storage" take Storage;
import "io/stdout" take Stdout;

main() is int {
    Stdout().writeln(User().name());
    Stdout().writeln(Storage(WorkingDirectory().path()).usage().available().string());
    Stdout().writeln(Platform().architecture());
    Stdout().writeln(WorkingDirectory().path().string());
    0;
}
```

<a id="recipe-pipeline"></a>

### 8.3. Преобразовать вывод программы следующей командой

**Задача.** Нужен простой конвейер обработки текста с отдельными argv каждой программы.

**Какие библиотеки использовать.** [`os/command`](../std/os/command.ok), [`os/pipeline_capture`](../std/os/pipeline_capture.ok), [`fs/path`](../std/fs/path.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** PipelineCapture соединяет stdout предыдущей команды со stdin следующей. Результат содержит общий конечный stdout, статусы команд и отдельные stderr.

**Запуск.** В примере используются пути к стандартным POSIX-программам; для своей системы задайте пути к тем же командам.

```okkama
module cookbook/pipeline v1.0.0;
import "os/command" take InheritedCommand;
import "os/pipeline_capture" take PipelineCapture;
import "fs/path" take Path;
import "encoding/utf" take Utf8Decoded;
import "io/stdout" take Stdout;

async main() is int {
    let producer = InheritedCommand(program: Path("/usr/bin/printf"), arguments: ["%s", "order-ready"]);
    let consumer = InheritedCommand(program: Path("/usr/bin/tr"), arguments: ["a-z", "A-Z"]);
    let completed = await PipelineCapture(commands: [producer, consumer], limit: 4096).completion();
    confirm Utf8Decoded(completed.output()).string() == "ORDER-READY";
    Stdout().writeln(Utf8Decoded(completed.output()).string());
    0;
}
```

<a id="chapter-config"></a>

## 9. Командная строка и конфигурация

Пример операторской программы с `Utf8Arguments` и `ParsedInt` —
[инспекция и сверка возврата](refund-reconciliation.md): сначала команда показывает
состояние, затем принимает ожидаемые поколение и статус. Авторизация, исходный
ключ операции и SQL-проверки относятся к приложению; стандартные аргументы лишь
дают проверенный ввод для этого протокола.

Опишите синтаксис команды, разберите аргументы и сведите источники к SettingReading. Один и тот же предметный ключ связывает defaults, документ, переменные окружения и CLI. Так приоритет настроек виден в месте сборки.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`cli/arguments`](../std/cli/arguments.ok) | Получить уже разделённые аргументы запуска: ProcessArguments и Utf8Arguments представляют argv, ArgumentSnapshot удерживает снимок. | `ProcessArguments`, `ArgumentSnapshot`, `ArgumentParts`, `Utf8Arguments` | [Рецепт](#example-application-cli) |
| [`cli/help`](../std/cli/help.ok) | Напечатать --help и дополнения оболочки: Usage и ShellCompletion строят представление из спецификации команды. | `CompletionShell`, `Usage`, `ShellCompletion`, `CompletionWords`, `CompletionNames` | [Рецепт](#example-application-cli) |
| [`cli/parse`](../std/cli/parse.ok) | Разобрать --input и позиционные пути: ParsedCommand возвращает Invocation с настройками и выбранной командой. | `ParsedCommand`, `Invocation`, `CliOperandToken`, `CliParsing` | [Рецепт](#example-application-cli) |
| [`cli/spec`](../std/cli/spec.ok) | Описать команду импорта: CliCommand, CommandSyntax, Option, Flag и Operand задают синтаксис команды. | `OperandCardinality`, `OptionValue`, `Option`, `Flag`, `Operand` | [Рецепт](#example-application-cli) |
| [`config/document`](../std/config/document.ok) | Использовать JSON, TOML или INI как слой настроек: JsonSettings, TomlSettings и IniSettings переводят документы в SettingEntry. | `SettingDocumentEntries`, `JsonSettings`, `JsonSettingVisitor`, `JsonSettingScalar`, `TomlSettings` | [Рецепт](#recipe-json-settings) |
| [`config/environment`](../std/config/environment.ok) | Привязать APP_PORT к предметному ключу: EnvironmentBinding и EnvironmentSettings явно описывают переменные. | `EnvironmentBinding`, `EnvironmentSettings` | [Рецепт](#example-application-cli) |
| [`config/ini`](../std/config/ini.ok) | Сохранить привычный INI-файл приложения: IniDocument, ParsedIni и IniString представляют структуру и текст. | `IniContent`, `IniWriting`, `IniEntry`, `IniSection`, `IniDocument` | [Рецепт](#example-ini-codec) |
| [`config/key`](../std/config/key.ok) | Отличить вложенный server.port от буквального имени: SettingKey хранит последовательность точных сегментов ключа. | `SettingKey`, `SettingKeyFromString` | [Рецепт](#recipe-json-settings) |
| [`config/source`](../std/config/source.ok) | Соединить defaults, файл и аргументы: Settings и LayeredSettings сохраняют приоритеты и происхождение значений. | `SettingSourceKind`, `SettingSource`, `SettingEntry`, `SettingReading`, `Settings` | [Рецепт](#recipe-json-settings) |
| [`config/value`](../std/config/value.ok) | Прочитать один параметр после объединения слоёв: ScalarSetting и ScalarSettingOr выбирают скалярный результат. | `Setting`, `ScalarSetting`, `ScalarSettingOr`, `NonScalarSetting` | [Рецепт](#recipe-json-settings) |

<a id="recipe-json-settings"></a>

### 9.1. Использовать JSON как слой настроек

**Задача.** Настройки порта заданы документом и должны читаться тем же способом, что аргументы команды.

**Какие библиотеки использовать.** [`config/document`](../std/config/document.ok), [`config/source`](../std/config/source.ok), [`config/key`](../std/config/key.ok), [`config/value`](../std/config/value.ok), [`json/value`](../std/json/value.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Адаптер переводит документ в SettingReading. После этого файл можно включить в LayeredSettings вместе с defaults, EnvironmentSettings и настройками Invocation.

```okkama
module cookbook/json_settings v1.0.0;
import "config/document" take JsonSettings;
import "config/source" take SettingSource,SettingSourceKind;
import "config/key" take SettingKey;
import "config/value" take ScalarSetting;
import "json/value" take JsonObject,JsonMember,JsonText;
import "io/stdout" take Stdout;

main() is int {
    let document = JsonObject([JsonMember("port", JsonText("8080"))]);
    let settings = JsonSettings(document, SettingSource(SettingSourceKind.document, "service.json"));
    let port = ScalarSetting(settings, SettingKey(["port"])).value();
    confirm port == "8080";
    Stdout().writeln(port);
    0;
}
```

<a id="example-application-cli"></a>

### 9.2. Собрать CLI с defaults, окружением и --help

**Задача.** Получить уже разделённые аргументы запуска: ProcessArguments и Utf8Arguments представляют argv, ArgumentSnapshot удерживает снимок.

**Какие библиотеки использовать.** [`cli/arguments`](../std/cli/arguments.ok), [`cli/spec`](../std/cli/spec.ok), [`cli/parse`](../std/cli/parse.ok), [`cli/help`](../std/cli/help.ok), [`config/key`](../std/config/key.ok), [`config/source`](../std/config/source.ok), [`config/value`](../std/config/value.ok), [`config/environment`](../std/config/environment.ok), [`os/environment`](../std/os/environment.ok), [`terminal/session`](../std/terminal/session.ok), [`terminal/capability`](../std/terminal/capability.ok), [`terminal/style`](../std/terminal/style.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Опишите greeting/name как SettingKey. ParsedCommand выдаёт слой CLI, EnvironmentSettings — слой переменных; LayeredSettings выбирает итоговое имя.

```okkama
module example v1.0.0;
import "cli/arguments" take ProcessArguments,Utf8Arguments;
import "cli/spec" take CliCommand,CommandSyntax,Option,OptionValue,Flag;
import "cli/parse" take ParsedCommand,CliFailure;
import "cli/help" take Usage;
import "config/key" take SettingKey;
import "config/source" take Settings,SettingEntry,SettingSource,SettingSourceKind,LayeredSettings;
import "config/value" take ScalarSetting;
import "config/environment" take EnvironmentSettings,EnvironmentBinding;
import "os/environment" take Environment;
import "terminal/session" take OpenedStandardTerminal,TerminalOutputKind;
import "terminal/capability" take ColorPolicy;
import "terminal/style" take StyledText,TextStyle,NamedColor,DefaultColor,BasicColor;
import "io/stdout" take Stderr;

async main() is int{
    let name=SettingKey(["greeting","name"]);
    let help=SettingKey(["help"]);
    let command=CliCommand("greet","Greet one explicitly configured person",CommandSyntax(
        [Option(name,["--name","-n"],OptionValue("NAME",false),"Person to greet")],
        [Flag(help,["--help","-h"],"true","Show this help")],[]),[]);
    let arguments=Utf8Arguments(ProcessArguments());
    let invocation=try{ParsedCommand(command,arguments).value();}
        catch failure is CliFailure{Stderr().writeln("argument "+failure.index().string()+": "+failure.text());return 2;};
    let environment=Environment();
    let session=await OpenedStandardTerminal(TerminalOutputKind.standard_output,ColorPolicy.automatic,environment).session();
    defer session.close();
    if invocation.settings().membership(help){session.output().write(Usage(command,"greet",80).string());return 0;}
    let defaults=Settings([SettingEntry(name,["world"],SettingSource(SettingSourceKind.defaults,"application"))]);
    let variables=EnvironmentSettings(environment,[EnvironmentBinding("OKKAMA_NAME",name)],SettingSource(SettingSourceKind.environment,"process"));
    let settings=LayeredSettings([defaults,variables,invocation.settings()]);
    let person=ScalarSetting(settings,name).value();
    session.output().writeln(StyledText("Hello, "+person+"!",TextStyle(NamedColor(BasicColor.green),DefaultColor(),[])));
    0;
}
```

[Исходный пример](../examples/std-v4/application-cli.ok).

<a id="example-ini-codec"></a>

### 9.3. Прочитать и записать INI-конфигурацию

**Задача.** Сохранить привычный INI-файл приложения: IniDocument, ParsedIni и IniString представляют структуру и текст.

**Какие библиотеки использовать.** [`config/ini`](../std/config/ini.ok).

**Как использовать.** ParsedIni разбирает строку в IniDocument. Изменения описываются новым документом; IniString строит текст выбранного значения.

```okkama
module app v1.0.0;
import "config/ini" take ParsedIni, IniString;
object ExampleResult {
    ExampleResult();
    value() is string {
        let settings = ParsedIni("# explicit Okkama INI\n[settings]\nname=Ada\npath=C:\\tmp");
        IniString(settings).string();
    }
}

main() is int {
    ExampleResult().value();
    return 0;
}
```

[Исходный пример](../examples/std-v4/ini-codec.ok).

<a id="chapter-terminal"></a>

## 10. Интерактивный терминал

Откройте TerminalSession, выберите представление по её capability и удерживайте режимы ввода через lease. Ширина отображения, цвет и управление экраном решают разные задачи интерфейса.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`terminal/capability`](../std/terminal/capability.ok) | Выбрать цветной или простой вывод: TerminalCapabilities и ColorPolicy используют явное наблюдение терминала. | `TerminalColorDepth`, `ColorPolicy`, `TerminalSize`, `TerminalCapabilities`, `DetectedTerminalCapabilities` | [Рецепт](#recipe-terminal-prompt) |
| [`terminal/error`](../std/terminal/error.ok) | Обработать отказ интерактивного ввода: TerminalFailure, TerminalClosed и TerminalInputLimit различают причины. | `TerminalUnavailable`, `TerminalClosed`, `TerminalInputLimit`, `TerminalProblem` | [Рецепт](#recipe-terminal-prompt) |
| [`terminal/mode`](../std/terminal/mode.ok) | Временно включить специальный режим ввода: AppliedTerminalMode и TerminalModeLease ограничивают время действия режима. | `TerminalMode`, `TerminalModeApplying`, `AppliedTerminalMode`, `TerminalModeLease` | [Рецепт](#recipe-terminal-prompt) |
| [`terminal/prompt`](../std/terminal/prompt.ok) | Запросить имя и пароль в CLI: PromptedLine и PromptedSecret используют явно открытый terminal session. | `PromptedLine`, `PromptedSecret`, `SecretPromptCleanup` | [Рецепт](#recipe-terminal-prompt) |
| [`terminal/screen`](../std/terminal/screen.ok) | Обновить интерактивный экран: TerminalScreen выполняет операции экрана, TerminalResizes наблюдает изменение размера. | `ClearRegion`, `TerminalScreen`, `TerminalScreenApplying`, `TerminalResizeOpening`, `TerminalResizeReading` | [Рецепт](#recipe-terminal-prompt) |
| [`terminal/session`](../std/terminal/session.ok) | Открыть консоль для интерактивной команды: OpenedStandardTerminal возвращает TerminalSession и её узкие capability. | `TerminalOutputKind`, `OpenedStandardTerminal`, `OpenedControllingTerminal`, `TerminalSession`, `TerminalObservation` | [Рецепт](#recipe-terminal-prompt) |
| [`terminal/style`](../std/terminal/style.ok) | Выделить статус заказа цветом: StyledText и TextStyle связывают текст с цветами и атрибутами. | `BasicColor`, `TextAttribute`, `TerminalColor`, `DefaultColor`, `NamedColor` | [Рецепт](#example-application-cli) |
| [`terminal/width`](../std/terminal/width.ok) | Выровнять колонки с emoji и кириллицей: DisplayWidth, TruncatedDisplay и WrappedDisplay учитывают grapheme boundaries. | `DisplayWidth`, `TruncatedDisplay`, `DisplayPrefix`, `WrappedDisplay`, `DisplayLines` | [Рецепт](#recipe-display-columns) |

<a id="recipe-terminal-prompt"></a>

### 10.1. Запросить имя и секрет в интерактивной команде

**Задача.** Утилита входа должна показать приглашение и временно отключить отображение секрета.

**Какие библиотеки использовать.** [`terminal/session`](../std/terminal/session.ok), [`terminal/capability`](../std/terminal/capability.ok), [`terminal/prompt`](../std/terminal/prompt.ok).

**Как использовать.** Запускайте пример в интерактивном терминале. PromptedSecret удерживает lease режима ввода до окончания чтения; закрытие session восстанавливает терминальное состояние.

```okkama
module cookbook/terminal_prompt v1.0.0;
import "terminal/session" take OpenedControllingTerminal;
import "terminal/capability" take ColorPolicy;
import "terminal/prompt" take PromptedLine,PromptedSecret;

async main() is int {
    let session = await OpenedControllingTerminal(ColorPolicy.automatic).session();
    defer session.close();
    let name = await PromptedLine(session, "Имя: ", 256).string();
    let password = await PromptedSecret(session, "Пароль: ", 4096).string();
    session.output().writeln("
Здравствуйте, " + name);
    0;
}
```

<a id="recipe-display-columns"></a>

### 10.2. Ограничить ширину колонки с Unicode

**Задача.** В консольной таблице название товара должно занимать заданное число экранных колонок.

**Какие библиотеки использовать.** [`terminal/width`](../std/terminal/width.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** DisplayWidth измеряет экранное представление. TruncatedDisplay сохраняет целые grapheme clusters и оставляет место для выбранного многоточия.

```okkama
module cookbook/display_columns v1.0.0;
import "terminal/width" take DisplayWidth,TruncatedDisplay;
import "io/stdout" take Stdout;

main() is int {
    confirm DisplayWidth("界").value() == 2;
    let label = TruncatedDisplay("Длинное название товара", 12, "…").string();
    confirm DisplayWidth(label).value() <= 12;
    Stdout().writeln(label);
    0;
}
```

<a id="chapter-time"></a>

## 11. Даты, время, часовые пояса и календарь

Время доставки в форме — LocalDateTime, событие журнала — Instant, замер длительности — монотонные часы. Часовой пояс передаётся при связи календарного времени с абсолютным событием.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`calendar`](../std/calendar.ok) | Показать календарь доставки на месяц: YearMonth, MonthDates, CalendarMonth и IsoWeek строят календарные представления. | `YearMonth`, `MonthDates`, `MonthDateCursor`, `CalendarWeek`, `CalendarWeekCursor` | [Рецепт](#recipe-calendar) |
| [`time/clock`](../std/time/clock.ok) | Измерить время операции и получить отметку журнала: SystemClock различает календарный Instant и MonotonicInstant. | `Instant`, `MonotonicInstant`, `Timing`, `SystemClock`, `IncompatibleClock` | [Рецепт](#recipe-test-tools) |
| [`time/date`](../std/time/date.ok) | Рассчитать дату следующей поставки: Date, ShiftedDate и DaysBetween работают с григорианскими датами. | `Weekday`, `Date`, `ShiftedDate`, `DaysBetween`, `DateOrdinal` | [Рецепт](#recipe-calendar) |
| [`time/day`](../std/time/day.ok) | Записать часы работы пункта выдачи: TimeOfDay представляет время внутри гражданских суток. | `TimeOfDay` | [Рецепт](#recipe-calendar) |
| [`time/duration`](../std/time/duration.ok) | Задать таймаут и интервал повтора: Duration хранит точную длительность с явно выбранной единицей. | `DurationUnit`, `Duration`, `ScaledDuration`, `DurationString` | [Рецепт](#recipe-time-api) |
| [`time/format`](../std/time/format.ok) | Выпустить отчёт в оговорённом формате даты: FormattedDate и DateFromFormat используют явный шаблон. | `FormattedDate`, `DateFromFormat`, `FormattedLocalDateTime`, `LocalDateTimeFromFormat` | [Рецепт](#recipe-calendar) |
| [`time/iso`](../std/time/iso.ok) | Прочитать дату из календарного поля: DateFromIso и LocalDateTimeFromIso разбирают ISO-представления. | `IsoDate`, `DateFromIso`, `IsoLocalDateTime`, `LocalDateTimeFromIso`, `IsoFraction` | [Рецепт](#recipe-calendar) |
| [`time/local_datetime`](../std/time/local_datetime.ok) | Сохранить введённое клиентом время встречи: LocalDateTime связывает Date и TimeOfDay. | `LocalDateTime` | [Рецепт](#recipe-calendar) |
| [`time/rfc3339`](../std/time/rfc3339.ok) | Передать отметку времени через API: Rfc3339 и InstantFromRfc3339 связывают строку с Instant. | `Rfc3339`, `InstantFromRfc3339` | [Рецепт](#recipe-time-api) |
| [`time/zone`](../std/time/zone.ok) | Назначить встречу в конкретном часовом поясе: Zone, ZonedDateTime и InstantFromLocal учитывают правила зоны. | `Fold`, `TimeZone`, `Zone`, `FixedZone`, `ZonedDateTime` | [Рецепт](#example-localized-date-time) |

<a id="recipe-calendar"></a>

### 11.1. Построить календарь поставок

**Задача.** На странице заказа нужны все даты выбранного месяца и единое строковое представление.

**Какие библиотеки использовать.** [`calendar`](../std/calendar.ok), [`time/iso`](../std/time/iso.ok), [`time/format`](../std/time/format.ok), [`time/day`](../std/time/day.ok), [`time/local_datetime`](../std/time/local_datetime.ok), [`time/date`](../std/time/date.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** MonthDates перечисляет только выбранный месяц. LocalDateTime подходит для календарного ввода; перевод в абсолютный Instant выполняют с явно выбранной зоной.

```okkama
module cookbook/calendar v1.0.0;
import "calendar" take YearMonth,MonthDates;
import "time/iso" take IsoDate;
import "time/format" take FormattedDate;
import "time/day" take TimeOfDay;
import "time/local_datetime" take LocalDateTime;
import "time/date" take Date;
import "io/stdout" take Stdout;

main() is int {
    let dates = MonthDates(origin: YearMonth(year: 2026, month: 10));
    confirm dates.size() == 31;
    for dates -> date { Stdout().writeln(IsoDate(date).string()); }
    let delivery = LocalDateTime(Date(2026, 10, 18), TimeOfDay(14, 30, 0, 0));
    Stdout().writeln(FormattedDate(origin: delivery.date(), pattern: "%d.%m.%Y").string());
    0;
}
```

<a id="recipe-time-api"></a>

### 11.2. Передать отметку времени через API

**Задача.** Нужно принять RFC 3339 время и вывести ту же абсолютную отметку в UTC.

**Какие библиотеки использовать.** [`time/rfc3339`](../std/time/rfc3339.ok), [`time/duration`](../std/time/duration.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Instant хранит абсолютную отметку, а offset участвует в строковом представлении. Нулевое смещение даёт канонический UTC-суффикс Z.

```okkama
module cookbook/time_api v1.0.0;
import "time/rfc3339" take InstantFromRfc3339,Rfc3339;
import "time/duration" take Duration,DurationUnit;
import "io/stdout" take Stdout;

main() is int {
    let received = InstantFromRfc3339("2026-10-18T14:30:00+05:00").value();
    let canonical = Rfc3339(received, Duration(0.rat(), DurationUnit.second)).string();
    confirm canonical == "2026-10-18T09:30:00Z";
    Stdout().writeln(canonical);
    0;
}
```

<a id="chapter-random"></a>

## 12. Случайность и идентификаторы

Для воспроизводимых данных задавайте seed источника PcgRandom. Для nonce и случайного UUID выбирайте SecureRandom. Распределение получает источник как зависимость, поэтому сценарий легко воспроизвести.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`random/distribution`](../std/random/distribution.ok) | Смоделировать поток заказов: распределения используют переданный Randomizing и выдают sample(). | `Distribution`, `UniformIntegers`, `UniformFloats`, `NormalDistribution`, `StandardNormal` | [Рецепт](#recipe-random-ids) |
| [`random/source`](../std/random/source.ok) | Воспроизвести тестовую выборку либо получить nonce: PcgRandom задаёт seed, SecureRandom получает системную энтропию. | `Randomizing`, `PcgRandom`, `SecureRandom`, `SecureInteger`, `SecureBytes` | [Рецепт](#recipe-random-ids) |
| [`uuid/generation`](../std/uuid/generation.ok) | Создать идентификатор документа: Uuid4s использует источник случайности, Uuid3 и Uuid5 связывают namespace с именем. | `Uuid3`, `Uuid5`, `Uuid8`, `Uuid4s` | [Рецепт](#recipe-random-ids) |
| [`uuid/value`](../std/uuid/value.ok) | Проверить идентификатор из внешнего API: Uuid и UuidFromBytes связывают 128 бит с каноническим текстом. | `UuidVariant`, `Uuid`, `UuidFromBytes` | [Рецепт](#recipe-random-ids) |

<a id="recipe-random-ids"></a>

### 12.1. Создать идентификатор и воспроизводимую выборку

**Задача.** Документу нужен UUID, а тестовым задержкам — стабильная случайная последовательность.

**Какие библиотеки использовать.** [`random/source`](../std/random/source.ok), [`random/distribution`](../std/random/distribution.ok), [`uuid/generation`](../std/uuid/generation.ok), [`uuid/value`](../std/uuid/value.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Seed фиксирует PCG-последовательность. UUID v4 здесь получает байты от явно переданного SecureRandom; диапазон UniformIntegers полуоткрытый.

```okkama
module cookbook/random_ids v1.0.0;
import "random/source" take PcgRandom,SecureRandom;
import "random/distribution" take UniformIntegers;
import "uuid/generation" take Uuid4s;
import "uuid/value" take Uuid;
import "io/stdout" take Stdout;

main() is int {
    let generator = PcgRandom(seed: 42);
    let delay = UniformIntegers(random: generator, start: 10, stop: 20);
    let sampled = delay.sample();
    confirm sampled >= 10 && sampled < 20;
    let id = Uuid4s(random: SecureRandom()).sample();
    confirm Uuid(id.string()) == id;
    Stdout().writeln(id.string());
    0;
}
```

<a id="chapter-crypto"></a>

## 13. Криптография, сертификаты и контроль целостности

Секрет, открытый ключ, сертификат и доверие — разные сущности. При проверке сервера связывайте корни доверия, идентичность и время. MD5/SHA-1 в этой главе используются для совместимости со старыми полями форматов; CRC и Adler-32 — для контрольных сумм.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`checksum/adler32`](../std/checksum/adler32.ok) | Сверить Adler-32 из заголовка формата: Adler32 вычисляет контрольную сумму с явным начальным значением. | `Adler32` | [Рецепт](#example-checksum-adler32) |
| [`checksum/crc`](../std/checksum/crc.ok) | Проверить CRC переданного пакета: Crc32 и Crc16Ccitt дают числовую сумму и продолжение по блокам. | `Crc32`, `Crc16Ccitt` | [Рецепт](#example-checksum-crc) |
| [`checksum/state`](../std/checksum/state.ok) | Принять любой потоковый checksum в проверку файла: Checksumming задаёт value и иммутабельное продолжение state. | `Checksumming` | [Рецепт](#example-checksum-adler32) |
| [`crypto/key`](../std/crypto/key.ok) | Передать открытый ключ проверяющему коду: PublicKey задаёт DER SubjectPublicKeyInfo через bytes(). | `PublicKey` | [Рецепт](#example-x509-certificate) |
| [`crypto/secret`](../std/crypto/secret.ok) | Удерживать токен доступа как отдельный ресурс: SecretBytes явно ограничивает экспорт и время жизни секрета. | `SecretBytes`, `ExportedSecret`, `SecretClosed`, `SecretLimit` | [Рецепт](#example-http-proxy) |
| [`crypto/x509/certificate`](../std/crypto/x509/certificate.ok) | Разобрать DER-сертификат партнёра: Certificate предоставляет имя, ключ, расширения и цепочку наблюдений. | `ParsedCertificate`, `Certificate`, `CertificateName`, `CertificatePublicKey`, `CertificateNames` | [Рецепт](#example-x509-certificate) |
| [`crypto/x509/name`](../std/crypto/x509/name.ok) | Сравнить структурированные имена сертификатов: DistinguishedName сохраняет группы атрибутов X.509. | `NameAttribute`, `RelativeDistinguishedName`, `DistinguishedName` | [Рецепт](#example-x509-name) |
| [`crypto/x509/trust`](../std/crypto/x509/trust.ok) | Проверить цепочку для выбранного сервера: явный TrustStore и VerifiedCertificateChain связывают корни, имя и время. | `TrustStore`, `CertificateIdentityKind`, `CertificateIdentity`, `DnsCertificateIdentity`, `IpCertificateIdentity` | [Рецепт](#example-certificate-observers) |
| [`crypto/x509/value`](../std/crypto/x509/value.ok) | Проверить календарный интервал сертификата: CertificateValidity и CertificateConstraints представляют его условия. | `CertificateValidity`, `CertificateConstraints` | [Рецепт](#example-x509-values) |
| [`digest/blocks`](../std/digest/blocks.ok) | Реализовать обработку блоков собственного digest: DigestBlocks64 и DigestPadding64 выделяют общую блочную инфраструктуру. | `DigestCompression64`, `DigestBlocks64`, `DigestPadding64`, `DigestInputTooLong` | [Рецепт](#example-digest-legacy) |
| [`digest/legacy`](../std/digest/legacy.ok) | Сверить поле MD5 или SHA-1 старого формата обмена: Md5 и Sha1 вычисляют совместимое байтовое представление. | `Md5`, `Sha1`, `Md5Compression`, `Sha1Compression` | [Рецепт](#example-digest-legacy) |
| [`digest/state`](../std/digest/state.ok) | Обработать файл частями при вычислении digest: Digesting возвращает новый state для очередного блока байтов. | `Digesting` | [Рецепт](#example-digest-legacy) |

### Что учитывать при сборке решения

- `crypto/secret`: Экспорт байтов выполняется явно через ExportedSecret; владельца SecretBytes закройте по завершении операции.
- `crypto/x509/certificate`: Разбор DER предоставляет сведения сертификата. VerifiedCertificateChain — отдельная операция доверия с выбранными корнями и идентичностью.
- `digest/legacy`: Сверяйте известное поле старого формата с байтовым результатом, отобразив его через HexEncoded.

<a id="recipe-sha256"></a>

### 13.1. Сверить SHA-256 полученного документа

**Задача.** У поставщика опубликован hex SHA-256; загрузчик должен вычислить digest тех же байтов.

**Какие библиотеки использовать.** [`runtime/crypto`](../std/runtime/crypto.ok), [`encoding/utf`](../std/encoding/utf.ok), [`encoding/hex`](../std/encoding/hex.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Здесь явно используется модуль границы runtime/crypto. digest принимает имя алгоритма и исходные байты, HexEncoded формирует hex-представление. Для файла подставьте его байтовое содержимое.

```okkama
module cookbook/sha256 v1.0.0;
import "runtime/crypto" as crypto;
import "encoding/utf" take Utf8Encoded;
import "encoding/hex" take HexEncoded;
import "io/stdout" take Stdout;

main() is int {
    let bytes = Utf8Encoded("abc").bytes();
    let digest = crypto.digest("sha256", bytes);
    let text = HexEncoded(digest).string();
    confirm text == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    confirm crypto.secure_equal(digest, crypto.digest("sha256", bytes));
    Stdout().writeln(text);
    0;
}
```

<a id="example-checksum-adler32"></a>

### 13.2. Сверить контрольную сумму импортированного блока

**Задача.** Сверить Adler-32 из заголовка формата: Adler32 вычисляет контрольную сумму с явным начальным значением.

**Какие библиотеки использовать.** [`checksum/adler32`](../std/checksum/adler32.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Вычислите Adler32 над байтами и при необходимости продолжите сумму через state. Вход и предыдущая версия суммы сохраняются.

```okkama
module app v1.0.0;
import "checksum/adler32" take Adler32;
import "encoding/utf" take Utf8Encoded;
import "io/stdout" take Stdout;

main() is int {
    let prefix = Adler32(Utf8Encoded("Wiki").bytes());
    let whole = prefix.state(Utf8Encoded("pedia").bytes());
    Stdout().writeln(whole.value().string());
    Stdout().writeln(prefix.value().string());
    0;
}
```

[Исходный пример](../examples/std-v4/checksum-adler32.ok).

<a id="example-checksum-crc"></a>

### 13.3. Проверить CRC сетевого пакета

**Задача.** Проверить CRC переданного пакета: Crc32 и Crc16Ccitt дают числовую сумму и продолжение по блокам.

**Какие библиотеки использовать.** [`checksum/crc`](../std/checksum/crc.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Crc32 и Crc16Ccitt получают bytes и явный seed. value() возвращает числовую сумму; state добавляет очередной chunk в новую версию.

```okkama
module app v1.0.0;
import "checksum/crc" take Crc32, Crc16Ccitt;
import "encoding/utf" take Utf8Encoded;
import "io/stdout" take Stdout;

main() is int {
    let first = Crc32(Utf8Encoded("1234").bytes());
    let whole = first.state(Utf8Encoded("56789").bytes());
    let digits = Utf8Encoded("123456789").bytes();
    Stdout().writeln(whole.value().string());
    Stdout().writeln(Crc16Ccitt(digits, 65535).value().string());
    Stdout().writeln(Crc16Ccitt(digits, 0).value().string());
    0;
}
```

[Исходный пример](../examples/std-v4/checksum-crc.ok).

<a id="example-x509-certificate"></a>

### 13.4. Прочитать сведения DER-сертификата

**Задача.** Разобрать DER-сертификат партнёра: Certificate предоставляет имя, ключ, расширения и цепочку наблюдений.

**Какие библиотеки использовать.** [`crypto/x509/certificate`](../std/crypto/x509/certificate.ok), [`encoding/base64`](../std/encoding/base64.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Certificate связывает DER-байты. Имя, ключ и интервалы предоставляются предметными наблюдениями сертификата.

```okkama
module app v1.0.0;
import "crypto/x509/certificate" take ParsedCertificate;
import "encoding/base64" take Base64Decoded;
import "io/stdout" take Stdout;
main() is int {
    let c=ParsedCertificate(Base64Decoded("MIIDOjCCAiKgAwIBAgIBBzANBgkqhkiG9w0BAQsFADASMRAwDgYDVQQKDAdFeGFtcGxlMB4XDTIzMTExNDIyMTMyMFoXDTI3MDExNTA4MDAwMFowMTEQMA4GA1UECgwHRXhhbXBsZTEdMAoGA1UECwwDVExTMA8GA1UEAwwIb25lLnRlc3QwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQC0AVC8f6+i1jHzanXSfK2FMi3bYKF1ad9otQDiozQGBrjHWxvvjjbZkTZxVRZfhu7Tv0jutLhO1ZhjIH11dmoRVq77R/9xW3ox4nhl1/lxntoSP9MIU5zJzxrTh/1HZW5IAOC2uhsBP4EFvT6jl5eEzbPFHhRZ0IKN91h4snoEyLcdYI7bLN0cKsbnFVsuMkxsi+1V9w7BC6xIyschDhQh3oYUcpF9Oo3Nm4H3r/1U26DS97o7iIuRbVmcOK8GzfAbAwlH12sjglRf6luK9qE/e7Bb/KyKyws58NTxpNIEBaCKHssSqjnM7LwxJECeT4Lrr3rLwx4uCTN8oNZb1uTVAgMBAAGjfDB6MB0GA1UdDgQWBBSFl6gGd/HXDSNXcbW9tr4pQeQxFjAJBgNVHSMEAjAAMAwGA1UdEwEB/wQCMAAwDgYDVR0PAQH/BAQDAgWgMBEGCWCGSAGG+EIBAQQEAwIGwDAdBgNVHSUEFjAUBggrBgEFBQcDAgYIKwYBBQUHAwEwDQYJKoZIhvcNAQELBQADggEBAJEoEe/bTTyK43RdY5pJjtYA6rTnt0lZlBJCpybQM8o7vBUQp3H0Qi/B1kDnQxFDOt+7RtdjwqnnNQn9+AbUH0lKSZJHNJNbyU/AlG0ONedyUkMkDmP8t0SzzH5INF+qdo/06wZnzDOGHhYH/gcCfjdfTHETGXe2eWaqz3f5Z6102K0mO6ZjkH8nJJ0ynhBbcTJFQMMXQDkfafZjQZHFhydYQ9XK2yd3kKd9kYvUfZGh2bLaCPsTU7OTofBGIoHWgc23w8W0RatEJIFMZUINF5qx4PmVomy1qVh+XJPtSQq6vvWJgVDSOzzpBIbKXua3cQ7wzkTkoziGEG3qQydEg9A=").bytes()).value();
    confirm c.subject().relative_names().size()==2;
    confirm c.subject().relative_names().item(1).attributes().size()==2;
    confirm c.validity().starts().unix_duration().seconds()==1700000000.rat();
    confirm c.public_key().bytes().size()>200;
    Stdout().writeln("DER, grouped certificate names, validity and SPKI verified");0;
}
```

[Исходный пример](../examples/std-v4/x509-certificate.ok).

<a id="example-x509-name"></a>

### 13.5. Представить имя организации в X.509

**Задача.** Сравнить структурированные имена сертификатов: DistinguishedName сохраняет группы атрибутов X.509.

**Какие библиотеки использовать.** [`crypto/x509/name`](../std/crypto/x509/name.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** NameAttribute образует атрибут, RelativeDistinguishedName группирует связанные атрибуты, DistinguishedName сохраняет последовательность групп.

```okkama
module app v1.0.0;
import "crypto/x509/name" take NameAttribute,RelativeDistinguishedName,DistinguishedName;
import "io/stdout" take Stdout;
main() is int {
    let organization=RelativeDistinguishedName([NameAttribute("2.5.4.10","example")]);
    let identity=RelativeDistinguishedName([NameAttribute("2.5.4.3","example.com"),NameAttribute("2.5.4.3","other.example")]);
    let name=DistinguishedName([organization,identity]);
    confirm name.relative_names().size()==2 && name.relative_names().item(1).attributes().size()==2;
    Stdout().writeln("grouped distinguished-name snapshots verified");0;
}
```

[Исходный пример](../examples/std-v4/x509-name.ok).

<a id="example-x509-values"></a>

### 13.6. Проверить срок действия сертификата

**Задача.** Проверить календарный интервал сертификата: CertificateValidity и CertificateConstraints представляют его условия.

**Какие библиотеки использовать.** [`crypto/x509/value`](../std/crypto/x509/value.ok), [`time/clock`](../std/time/clock.ok), [`time/duration`](../std/time/duration.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** CertificateValidity получает две явные временные границы. Наблюдайте принадлежность выбранного Instant этому интервалу.

```okkama
module app v1.0.0;
import "crypto/x509/value" take CertificateValidity,CertificateConstraints,MissingCertificateDepth;
import "time/clock" take Instant;
import "time/duration" take Duration,DurationUnit;
import "io/stdout" take Stdout;
main() is int {
    let period=CertificateValidity(Instant(Duration(0.rat(),DurationUnit.second)),Instant(Duration(10.rat(),DurationUnit.second)));
    let unlimited=CertificateConstraints(true);let direct=CertificateConstraints(true,0);
    confirm period.starts()!=period.ends() && direct.maximum_depth()==0;
    confirm try{unlimited.maximum_depth();false;}catch e is MissingCertificateDepth{true;};
    Stdout().writeln("certificate metadata values verified; trust is a separate operation");0;
}
```

[Исходный пример](../examples/std-v4/x509-values.ok).

<a id="example-digest-legacy"></a>

### 13.7. Сверить MD5 или SHA-1 старой выгрузки

**Задача.** Сверить поле MD5 или SHA-1 старого формата обмена: Md5 и Sha1 вычисляют совместимое байтовое представление.

**Какие библиотеки использовать.** [`digest/legacy`](../std/digest/legacy.ok), [`encoding/utf`](../std/encoding/utf.ok), [`encoding/hex`](../std/encoding/hex.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Передайте байты в Md5/Sha1, затем HexEncoded преобразует байтовый digest в строку для сопоставления с полем внешнего формата.

```okkama
module app v1.0.0;
import "digest/legacy" take Md5, Sha1;
import "encoding/utf" take Utf8Encoded;
import "encoding/hex" take HexEncoded;
import "io/stdout" take Stdout;

main() is int {
    let prefix = Md5(Utf8Encoded("ab").bytes());
    let complete = prefix.state(Utf8Encoded("c").bytes());
    Stdout().writeln(HexEncoded(complete.bytes()).string());
    Stdout().writeln(HexEncoded(Sha1(Utf8Encoded("abc").bytes()).bytes()).string());
    Stdout().writeln(HexEncoded(prefix.bytes()).string());
    0;
}
```

[Исходный пример](../examples/std-v4/digest-legacy.ok).

<a id="example-certificate-observers"></a>

### 13.8. Прочитать SAN, ключ и расширения сертификата

**Задача.** Разобрать DER-сертификат партнёра: Certificate предоставляет имя, ключ, расширения и цепочку наблюдений.

**Какие библиотеки использовать.** [`crypto/x509/certificate`](../std/crypto/x509/certificate.ok), [`crypto/x509/trust`](../std/crypto/x509/trust.ok), [`encoding/base64`](../std/encoding/base64.ok), [`encoding/pem`](../std/encoding/pem.ok), [`net/ip`](../std/net/ip.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Предметные наблюдатели выбирают соответствующее расширение или public key из сертификата. Байтовое представление ключа остаётся DER SubjectPublicKeyInfo.

```okkama
module app v1.0.0;
import "crypto/x509/certificate" take Certificate,CertificateNames,CertificateChain;
import "crypto/x509/trust" take TrustStore,PemTrustStore,TrustedCertificates;
import "encoding/base64" take Base64Decoded;
import "encoding/pem" take PemEncoded;
import "net/ip" take IpAddress;
import "io/stdout" take Stdout;
main() is int {
    let certificate=Certificate(Base64Decoded("MIIDXTCCAkWgAwIBAgIBBzANBgkqhkiG9w0BAQsFADASMRAwDgYDVQQKDAdFeGFtcGxlMB4XDTIzMTExNDIyMTMyMFoXDTI3MDExNTA4MDAwMFowMTEQMA4GA1UECgwHRXhhbXBsZTEdMAoGA1UECwwDVExTMA8GA1UEAwwIb25lLnRlc3QwggEiMA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQC1PT/xZlx3qpz+hOUpep7NNz+mzYhS+HimbPSNo9QbEfHBFLvrvjzCmgVImyPL0FOzwgzKs7hSHB/gL0ml2VTt98hZhoOzT6zaH0A90OwM1rq22OkGHzxmELJl9t2tFzbb6DtVr198Vj7bVpt8RCp+mLEkmsOP37W/7NW4ilPX+c2oMBvJj/MITXCeqLgp58KZRZn92VlJESzcRUuEjYR1TDQeyYIMO0DpaCL2+2nC2sg4Uldk6MSLWo+fEOBKSIf53xNXlc2gR/oG55tZV29gFwLseyMaluKo6b8xqpBa3AqJTmL7u4C2pTNkAkm9SLzq4vFbKT2767HcLIiueQ6JAgMBAAGjgZ4wgZswHQYDVR0OBBYEFICI7Y+9QxWMMKjkvhdeVAer1O9EMAkGA1UdIwQCMAAwHwYDVR0RBBgwFoIOKi5leGFtcGxlLnRlc3SHBMAAAgcwDAYDVR0TAQH/BAIwADAOBgNVHQ8BAf8EBAMCBaAwEQYJYIZIAYb4QgEBBAQDAgbAMB0GA1UdJQQWMBQGCCsGAQUFBwMCBggrBgEFBQcDATANBgkqhkiG9w0BAQsFAAOCAQEAZOm1/tL+Q6lb3mxS6kMVU6NqIwVAcCq7cEaZGRqa+GF0ITgMi+bayJ2+KYzF/+sCfdqBgRVVOCN2uMOrnQPD9J6aJsUUWsl13v4kCNk9Ao1xMrM/y6ihiZfLnV528P5aTXqgMaaED32cN14US6zLXNH4NaifjYMOwENFwOUa8o5LOiuRsJDStfRgttiia0VceeCd2pMtMeedmDj086QHELQJXpUtd1GOI8+aF1hROE2LGN1Uml51zrS04mpIgC3CR6wQ34vz68SeegOlxB7G/uqRygcAsLvQ5C0nHku3uoHkzFjAdeK40eu9LRAFwTwEXgD9+IOeSai4Lqkkp234hg==").bytes());
    let names=CertificateNames(certificate);
    confirm names.dns().item(0)=="*.example.test";
    confirm names.ips().item(0)==IpAddress("192.0.2.7");
    let chain=CertificateChain([certificate]);confirm chain.certificates().size()==1;
    let empty is TrustStore=TrustedCertificates([]);confirm empty.certificates().size()==0;
    let roots is TrustStore=PemTrustStore(PemEncoded("CERTIFICATE",certificate.der()).string());
    confirm roots.certificates().item(0)==certificate;
    Stdout().writeln("SAN, supplied chain and explicit PEM roots verified as snapshots");0;
}
```

[Исходный пример](../examples/std-v4/certificate-observers.ok).

<a id="chapter-formats"></a>

## 14. JSON, CSV, TOML, plist, XML и HTML

Внешний формат задаёт свои типы и границы. Разделяйте разбор документа, проверку предметных полей и его представление на выходе. Для большого CSV выбирайте потребление записей, для небольшого ответа — готовое дерево значений.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`csv/input`](../std/csv/input.ok) | Прочитать ограниченный CSV-документ из байтов: CsvInput выполняет явное UTF-8 декодирование и разбор. | `CsvInput` | [Рецепт](#example-csv-input) |
| [`csv/stream`](../std/csv/stream.ok) | Импортировать большой CSV по одной записи: CsvStream потребляет текущую строку и сигнализирует EndOfCsv. | `CsvStream` | [Рецепт](#recipe-csv-stream) |
| [`csv`](../std/csv.ok) | Описать CSV-выгрузку товаров: CsvField, CsvRecord, CsvRows и CsvString задают записи и строковое представление. | `CsvString`, `CsvRecord`, `CsvField`, `CsvRows`, `CsvMode` | [Рецепт](#example-csv) |
| [`html/entities`](../std/html/entities.ok) | Показать пользовательское имя в HTML: EscapedHtml кодирует специальные символы, UnescapedHtml декодирует ссылки. | `EscapedHtml`, `UnescapedHtml` | [Рецепт](#example-html-entities) |
| [`html/events`](../std/html/events.ok) | Извлечь события из готового HTML: HtmlEvent и HandlingHtmlEvent дают последовательность тегов, текста и комментариев. | `HtmlEventKind`, `HtmlAttribute`, `StartingHtmlTag`, `EndingHtmlTag`, `ContainingHtmlText` | [Рецепт](#recipe-html-events) |
| [`json/parse`](../std/json/parse.ok) | Прочитать JSON-тело запроса: ParsedJson откладывает разбор до value и возвращает JsonValue. | `ParsedJson`, `JsonNode` | [Рецепт](#recipe-json-read) |
| [`json/read`](../std/json/read.ok) | Извлечь id и имя из JSON: JsonView предоставляет строгие проекции и путь до ошибочного значения. | `JsonView`, `JsonPointerToken`, `JsonPayload`, `JsonKindName`, `JsonLocation` | [Рецепт](#recipe-json-read) |
| [`json/value`](../std/json/value.ok) | Собрать JSON-ответ о заказе: JsonObject, JsonMember, JsonArray и скаляры образуют типизированное дерево. | `JsonKind`, `JsonWriting`, `JsonValue`, `JsonMember`, `JsonNull` | [Рецепт](#recipe-json-settings) |
| [`json/write`](../std/json/write.ok) | Вернуть компактный API-ответ или читаемый файл: CompactJson и PrettyJson выбирают представление одного JsonValue. | `CompactJson`, `JsonTextOutput`, `PrettyJson`, `JsonBoundedText`, `PrettyJsonOutput` | [Рецепт](#recipe-runner-report) |
| [`plist/parse`](../std/plist/parse.ok) | Прочитать XML или binary plist: ParsedPlist получает байты и возвращает типизированное дерево. | `ParsedPlist`, `PlistParsedItems`, `PlistParsedNode`, `PlistParsePosition`, `PlistXmlSource` | [Рецепт](#example-plist-parse) |
| [`plist/value`](../std/plist/value.ok) | Представить настройки приложения Apple: PlistValue, PlistMember и скаляры сохраняют типы property list. | `PlistKind`, `PlistValue`, `PlistWriting`, `PlistBoolean`, `PlistInteger` | [Рецепт](#example-plist-write) |
| [`plist/write`](../std/plist/write.ok) | Записать настройки в нужном plist-представлении: EncodedPlist выбирает PlistFormat и выдаёт bytes. | `PlistFormat`, `EncodedPlist`, `PlistWireSession`, `PlistWireValue`, `PlistWrittenNames` | [Рецепт](#example-plist-write) |
| [`toml/parse`](../std/toml/parse.ok) | Загрузить TOML-конфигурацию сервиса: ParsedToml разбирает текст с явными бюджетами. | `ParsedToml`, `TomlNode` | [Рецепт](#example-toml-parse) |
| [`toml/value`](../std/toml/value.ok) | Построить типизированную конфигурацию: TomlTable, TomlArray и скаляры сохраняют значения TOML. | `TomlValue`, `TomlVisitor`, `TomlString`, `TomlInteger`, `TomlFloat` | [Рецепт](#example-toml-special-floats) |
| [`xml/name`](../std/xml/name.ok) | Проверить имя элемента экспортируемого XML: XmlName и XmlNCName отделяют правила именования от текста. | `XmlName`, `XmlNCName` | [Рецепт](#recipe-xml-invoice) |
| [`xml/node`](../std/xml/node.ok) | Собрать XML-документ накладной: XmlElement, XmlAttribute и XmlDocument составляют дерево узлов. | `XmlNode`, `XmlText`, `XmlComment`, `XmlProcessingInstruction`, `XmlAttribute` | [Рецепт](#recipe-xml-invoice) |
| [`xml/write`](../std/xml/write.ok) | Экранировать текст для XML-экспорта: EscapedXml и XmlCharacters применяют правила выбранного контекста. | `XmlCharacters`, `EscapedXml` | [Рецепт](#recipe-xml-invoice) |

### Что учитывать при сборке решения

- `csv/stream`: Предметная обработка видит строки последовательно. Перед фиксацией полного импорта дождитесь EndOfCsv.

<a id="recipe-csv-stream"></a>

### 14.1. Импортировать CSV по одной записи

**Задача.** Каталог товаров поступает байтовым потоком; строки нужно обрабатывать по мере чтения.

**Какие библиотеки использовать.** [`csv/stream`](../std/csv/stream.ok), [`io/buffer`](../std/io/buffer.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** CsvStream потребляет записи, а исходным Reading продолжает владеть вызывающий код. Полную загрузку завершают после EndOfCsv; предметную проверку артикула выполняют на каждой строке.

```okkama
module cookbook/csv_stream v1.0.0;
import "csv/stream" take CsvStream,EndOfCsv;
import "io/buffer" take ByteBuffer;
import "encoding/utf" take Utf8Encoded;
import "io/stdout" take Stdout;

main() is int {
    let buffer = ByteBuffer(Utf8Encoded("sku,name\r\nA1,Чай\r\nB2,Кофе\r\n").bytes());
    defer buffer.closing().close();
    let csv = CsvStream(buffer.reading());
    defer csv.close();
    loop {
        try {
            let row = csv.row();
            Stdout().writeln(row[0] + ": " + row[1]);
        }
        catch ended is EndOfCsv { last; }
    }
    0;
}
```

<a id="recipe-json-read"></a>

### 14.2. Проверить JSON-поля заказа

**Задача.** Из API пришли номер заказа и имя клиента; значения следует прочитать со строгими типами.

**Какие библиотеки использовать.** [`json/parse`](../std/json/parse.ok), [`json/read`](../std/json/read.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** JsonView сохраняет путь вложенного значения. member выбирает объект, а int/text задают ожидаемое представление конкретного поля.

```okkama
module cookbook/json_read v1.0.0;
import "json/parse" take ParsedJson;
import "json/read" take JsonView;
import "io/stdout" take Stdout;

main() is int {
    let order = JsonView(ParsedJson("{\"id\":4711,\"customer\":{\"name\":\"Ада\"}}"));
    confirm order.int("id") == 4711;
    Stdout().writeln(order.member("customer").text("name"));
    0;
}
```

<a id="recipe-html-events"></a>

Для HTTP используйте `web/request.RequestJson`, полученный через
`await request.json(4096)`: он связывает ошибки чтения с HTTP-статусами400/422.
Проверяйте права до разбора тела, связывайте поля с конструкторами доменных
значений и формируйте ответ через выбранное представление и `JsonObject`.
[Практический рецепт с адресом клиента](json-http-codecs.md) включает реальный
codec, обработчик, OpenAPI и проверки Unicode, вложенных полей и точных сумм.

### 14.3. Перебрать теги полученной HTML-страницы

**Задача.** Импортёр описаний товаров должен реагировать на начало тегов и текстовые события.

**Какие библиотеки использовать.** [`html/events`](../std/html/events.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** HtmlEvents выдаёт события в порядке исходника. Для предметной обработки вызывайте dispatch с объектом HandlingHtmlEvent; роли отдельных событий передают нужные данные обработчику.

```okkama
module cookbook/html_events v1.0.0;
import "html/events" take HtmlEvents,HtmlEventKind;
import "io/stdout" take Stdout;

main() is int {
    let events = HtmlEvents(origin: "<p>Чай &amp; кофе</p>");
    let kinds = events.map() -> event { event.kind(); };
    confirm kinds[0] == HtmlEventKind.start_tag;
    confirm kinds[1] == HtmlEventKind.text;
    confirm kinds[2] == HtmlEventKind.end_tag;
    Stdout().writeln(kinds.size().string());
    0;
}
```

<a id="recipe-xml-invoice"></a>

### 14.4. Сформировать XML-накладную

**Задача.** Партнёр принимает идентификатор и название позиции в XML-документе.

**Какие библиотеки использовать.** [`xml/name`](../std/xml/name.ok), [`xml/node`](../std/xml/node.ok), [`xml/write`](../std/xml/write.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** XmlName проверяет имя отдельно от значения. XmlText и XmlAttribute выполняют контекстное экранирование при построении узла; XmlDocument добавляет документное представление.

```okkama
module cookbook/xml_invoice v1.0.0;
import "xml/name" take XmlName;
import "xml/node" take XmlElement,XmlText,XmlAttribute,XmlDocument;
import "xml/write" take EscapedXml;
import "io/stdout" take Stdout;

main() is int {
    let item = XmlElement(XmlName("item"), [XmlAttribute(XmlName("id"), "A-7")], [XmlText("Чай & кофе")]);
    let document = XmlDocument(XmlElement(XmlName("invoice"), [item]));
    Stdout().writeln(document.xml());
    confirm EscapedXml("A&B", false).string() == "A&amp;B";
    0;
}
```

<a id="example-csv-input"></a>

### 14.5. Прочитать UTF-8 CSV из байтового источника

**Задача.** Прочитать ограниченный CSV-документ из байтов: CsvInput выполняет явное UTF-8 декодирование и разбор.

**Какие библиотеки использовать.** [`csv/input`](../std/csv/input.ok), [`csv`](../std/csv.ok), [`io/buffer`](../std/io/buffer.ok), [`encoding/utf`](../std/encoding/utf.ok).

**Как использовать.** CsvInput получает Reading и ограничения документа. Разделите чтение, разбор CSV и предметную проверку полей товара.

```okkama
module app v1.0.0;
import "csv/input" take CsvInput;
import "csv" take CsvDialect, CsvString;
import "io/buffer" take ByteBuffer;
import "encoding/utf" take Utf8Encoded;

object ExampleResult {
    ExampleResult();
    value() is string {
        let source = ByteBuffer(Utf8Encoded("name;note\nAda;'one; two'\n").bytes());
        let dialect = CsvDialect(";", "'", "\n");
        let rows = CsvInput(source.reading(), dialect).read();
        CsvString(rows, dialect).string();
    }
}

main() is int {
    ExampleResult().value();
    return 0;
}
```

[Исходный пример](../examples/std-v4/csv-input.ok).

<a id="example-csv"></a>

### 14.6. Выгрузить строки таблицы в CSV

**Задача.** Описать CSV-выгрузку товаров: CsvField, CsvRecord, CsvRows и CsvString задают записи и строковое представление.

**Какие библиотеки использовать.** [`csv`](../std/csv.ok).

**Как использовать.** Соберите CsvField и CsvRecord, затем CsvString выполняет quoting и выбранное текстовое представление записи.

```okkama
module app v1.0.0;
import "csv" take CsvString;
object ExampleResult {
    ExampleResult();
    value() is string {
        let rows is @@string = [["name", "note"], ["Ada", "commas, and quotes: \""]];
        CsvString(rows: rows).string();
    }
}

main() is int {
    ExampleResult().value();
    return 0;
}
```

[Исходный пример](../examples/std-v4/csv-basic.ok).

<a id="example-html-entities"></a>

### 14.7. Вставить имя клиента в HTML

**Задача.** Показать пользовательское имя в HTML: EscapedHtml кодирует специальные символы, UnescapedHtml декодирует ссылки.

**Какие библиотеки использовать.** [`html/entities`](../std/html/entities.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** EscapedHtml получает исходную строку и политику кавычек. Вызывайте string() в точке формирования текстового HTML-фрагмента.

```okkama
module app v1.0.0;

import "html/entities" take EscapedHtml;
import "io/stdout" take Stdout;

main() is int {
    let output = Stdout();
    output.writeln(EscapedHtml("<Ada & Bob>").string());
    output.writeln(EscapedHtml("\"Ada's\"", true).string());
    0;
}
```

[Исходный пример](../examples/std-v4/html-entities.ok).

<a id="example-plist-parse"></a>

### 14.8. Импортировать настройки property list

**Задача.** Прочитать XML или binary plist: ParsedPlist получает байты и возвращает типизированное дерево.

**Какие библиотеки использовать.** [`plist/parse`](../std/plist/parse.ok), [`plist/value`](../std/plist/value.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Передайте байты в ParsedPlist и получите дерево PlistValue. Узлы сохраняют типы строк, данных, чисел и дат.

```okkama
module app v1.0.0;
import "plist/parse" take ParsedPlist;
import "plist/value" take PlistKind;
import "encoding/utf" take Utf8Encoded;
import "io/stdout" take Stdout;
main() is int {
    let source="<plist version=\"1.0\"><dict><key>enabled</key><true/></dict></plist>";
    let xml=ParsedPlist(Utf8Encoded(source).bytes()).value();
    confirm xml.kind()==PlistKind.dictionary_value;
    Stdout().writeln("XML dictionary parsed");
    let binary=ParsedPlist([byte(98),byte(112),byte(108),byte(105),byte(115),byte(116),byte(48),byte(48),byte(9),byte(8),byte(0),byte(0),byte(0),byte(0),byte(0),byte(0),byte(1),byte(1),byte(0),byte(0),byte(0),byte(0),byte(0),byte(0),byte(0),byte(1),byte(0),byte(0),byte(0),byte(0),byte(0),byte(0),byte(0),byte(0),byte(0),byte(0),byte(0),byte(0),byte(0),byte(0),byte(0),byte(9)]).value();
    confirm binary.kind()==PlistKind.boolean_value;
    Stdout().writeln("binary boolean parsed");
    0;
}
```

[Исходный пример](../examples/std-v4/plist-parse.ok).

<a id="example-plist-write"></a>

### 14.9. Записать настройки в XML либо binary plist

**Задача.** Представить настройки приложения Apple: PlistValue, PlistMember и скаляры сохраняют типы property list.

**Какие библиотеки использовать.** [`plist/value`](../std/plist/value.ok), [`plist/write`](../std/plist/write.ok), [`encoding/utf`](../std/encoding/utf.ok), [`encoding/hex`](../std/encoding/hex.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** EncodedPlist получает дерево и PlistFormat. bytes() формирует ограниченное wire-представление готового значения.

```okkama
module app v1.0.0;
import "plist/value" take PlistDictionary, PlistMember, PlistString, PlistBoolean, PlistData;
import "plist/write" take EncodedPlist, PlistFormat;
import "encoding/utf" take Utf8Decoded;
import "encoding/hex" take HexEncoded;
import "io/stdout" take Stdout;

main() is int {
    let value=PlistDictionary([
        PlistMember("name",PlistString("Okkama & Ω")),
        PlistMember("enabled",PlistBoolean(true)),
        PlistMember("data",PlistData([byte(0),byte(255)])),
    ]);
    Stdout().writeln(Utf8Decoded(EncodedPlist(value,PlistFormat.xml).bytes()).string());
    Stdout().writeln(HexEncoded(EncodedPlist(value,PlistFormat.binary).bytes()).string());
    0;
}
```

[Исходный пример](../examples/std-v4/plist-write.ok).

<a id="example-toml-parse"></a>

### 14.10. Прочитать TOML-настройки запуска

**Задача.** Загрузить TOML-конфигурацию сервиса: ParsedToml разбирает текст с явными бюджетами.

**Какие библиотеки использовать.** [`toml/parse`](../std/toml/parse.ok).

**Как использовать.** ParsedToml откладывает разбор до value(). Полученная таблица и скаляры позволяют продолжить предметную проверку параметров.

```okkama
module app v1.0.0;
import "toml/parse" take ParsedToml;
object ExampleResult {
    ExampleResult();
    value() is @string {
        let settings = ParsedToml("title='Okkama'\n[database]\nport=5432\nenabled=true").value();
        let names is @string = settings.members().map() -> member { member.name(); };
        names;
    }
}

main() is int {
    ExampleResult().value();
    return 0;
}
```

[Исходный пример](../examples/std-v4/toml-parse.ok).

<a id="example-toml-special-floats"></a>

### 14.11. Сохранить специальные числовые значения TOML

**Задача.** Построить типизированную конфигурацию: TomlTable, TomlArray и скаляры сохраняют значения TOML.

**Какие библиотеки использовать.** [`toml/value`](../std/toml/value.ok).

**Как использовать.** Используйте отдельные TomlInfinity/TomlNaN внутри модели TOML. Visitor обрабатывает их как значения данного внешнего формата.

```okkama
module app v1.0.0;
import "toml/value" take TomlInfinity, TomlNaN;
object ExampleResult {
    ExampleResult();
    value() is @string {
        [TomlInfinity(false).string(), TomlInfinity(true).string(), TomlNaN().string()];
    }
}

main() is int {
    ExampleResult().value();
    return 0;
}
```

[Исходный пример](../examples/std-v4/toml-special-floats.ok).

<a id="chapter-url"></a>

## 15. URL, параметры запроса и формы

Разделяйте адрес целиком и данные его отдельных компонентов. Значения query кодируются отдельно; повторяющиеся параметры сохраняют порядок. Относительная ссылка разрешается относительно явно выбранного базового адреса.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`url/form`](../std/url/form.ok) | Отправить обычную HTML-форму: FormUrlEncoded и ParsedForm используют правила application/x-www-form-urlencoded. | `FormUrlEncoded`, `ParsedForm` | [Рецепт](#example-url-parameters) |
| [`url/percent`](../std/url/percent.ok) | Вставить пользовательское значение в компонент URL: PercentEncoded и PercentDecoded учитывают выбранный UrlComponent. | `UrlComponent`, `PercentEncoded`, `PercentDecoded` | [Рецепт](#example-url-percent) |
| [`url/query`](../std/url/query.ok) | Прочитать повторяющиеся фильтры ?tag=a&tag=b: QueryParameters сохраняет имена, значения и порядок. | `QueryParameter`, `QueryParameters`, `ParsedQueryParameters`, `ParameterSnapshot`, `ParameterParsing` | [Рецепт](#example-url-parameters) |
| [`url/value`](../std/url/value.ok) | Разобрать адрес API и разрешить относительную ссылку: ParsedUrl, Url и UrlParts сохраняют компоненты адреса. | `UrlQuery`, `UrlFragment`, `UrlComponentText`, `UrlHostKind`, `UrlHost` | [Рецепт](#example-url-parameters) |

### Что учитывать при сборке решения

- `url/query`: Повторные query-параметры имеют собственные позиции; выбирайте first/все значения по контракту конкретной формы.

<a id="example-url-parameters"></a>

### 15.1. Сохранить повторные query-параметры

**Задача.** Разобрать адрес API и разрешить относительную ссылку: ParsedUrl, Url и UrlParts сохраняют компоненты адреса.

**Какие библиотеки использовать.** [`url/value`](../std/url/value.ok), [`url/query`](../std/url/query.ok), [`url/form`](../std/url/form.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** QueryParameter и QueryParameters хранят каждую пару отдельно. Выберите нужное наблюдение имени либо все одноимённые значения.

```okkama
module app v1.0.0;
import "url/value" take UrlQuery, UrlFragment;
import "url/query" take ParsedQueryParameters;
import "url/form" take FormUrlEncoded, ParsedForm;
import "io/stdout" take Stdout;

main() is int {
    let query=UrlQuery("flag&name=чай&name=a+b");
    let entries=ParsedQueryParameters(query).value();
    Stdout().writeln(query.string());
    Stdout().writeln(entries.values("name").item(1));
    let form=FormUrlEncoded(entries);
    Stdout().writeln(form.string());
    Stdout().writeln(ParsedForm(form.bytes()).parameters().first("name"));
    Stdout().writeln(UrlFragment("%7e/часть").string());
    0;
}
```

[Исходный пример](../examples/std-v4/url-parameters.ok).

<a id="example-url-percent"></a>

### 15.2. Передать пользовательский текст в URL

**Задача.** Вставить пользовательское значение в компонент URL: PercentEncoded и PercentDecoded учитывают выбранный UrlComponent.

**Какие библиотеки использовать.** [`url/percent`](../std/url/percent.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** PercentEncoded получает значение и UrlComponent. Выбирайте контекст отдельного query-значения или другой нужной части адреса.

```okkama
module app v1.0.0;
import "url/percent" take PercentEncoded, PercentDecoded, UrlComponent, InvalidPercentEncoding;
import "io/stdout" take Stdout;

main() is int {
    let encoded=PercentEncoded("чай & кофе",UrlComponent.data).string();
    Stdout().writeln(encoded);
    Stdout().writeln(PercentDecoded(encoded).string());
    Stdout().writeln(PercentDecoded("%252F+%2F").string());
    try {PercentDecoded("чай%XZ").bytes();}
    catch failure is InvalidPercentEncoding {Stdout().writeln(failure.text());}
    0;
}
```

[Исходный пример](../examples/std-v4/url-percent.ok).

<a id="chapter-templates"></a>

## 16. Шаблоны писем, страниц и отчётов

Текст шаблона, именованные данные и назначение вывода передаются отдельно. Строковые bindings удобны для писем; JSON-адаптеры и секции — для списков позиций. Для HTML выбирайте контекстное представление.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`template/compile`](../std/template/compile.ok) | Составить текст письма по шаблону: TextTemplate связывает текст с именованными подстановками. | `TextTemplate`, `TemplateInstruction`, `TemplateProgram`, `TemplateScanner`, `TemplateTextParts` | [Рецепт](#example-template-json) |
| [`template/html`](../std/template/html.ok) | Сформировать HTML-карточку клиента: HtmlTemplate применяет экранирование по контексту подстановки. | `HtmlTemplate`, `HtmlTemplateInstruction`, `HtmlTemplateProgram`, `HtmlTemplatePlan`, `HtmlTemplateContext` | [Рецепт](#example-template-html) |
| [`template/html_library`](../std/template/html_library.ok) | Переиспользовать HTML-фрагменты по именам: HtmlTemplateLibrary хранит NamedHtmlTemplate для рендеринга partials. | `NamedHtmlTemplate`, `HtmlTemplateLibrary`, `HtmlTemplateLibrarySnapshot`, `DuplicateHtmlTemplate` | [Рецепт](#example-template-html-library) |
| [`template/json`](../std/template/json.ok) | Подать JSON в текстовый шаблон: JsonTemplateStrings адаптирует скаляры через протокол JsonWriting. | `JsonTemplateStrings`, `JsonTemplateObservation`, `JsonTemplateVisitor`, `NonScalarTemplateValue` | [Рецепт](#example-template-json) |
| [`template/library`](../std/template/library.ok) | Собрать набор текстовых писем по именам: TemplateLibrary и NamedTemplate удерживают именованные шаблоны. | `NamedTemplate`, `TemplateLibrary`, `TemplateLibrarySnapshot`, `DuplicateTemplate` | [Рецепт](#example-template-partials) |
| [`template/model`](../std/template/model.ok) | Передать имя клиента и номер заказа в шаблон: TemplateBindings удерживает явные строковые значения по путям. | `TemplateStrings`, `TemplateBinding`, `TemplateBindings`, `TemplateBindingSnapshot`, `DuplicateTemplateValue` | [Рецепт](#example-template-library) |
| [`template/path`](../std/template/path.ok) | Назвать вложенное значение шаблона: TemplatePath сохраняет точные сегменты user.name. | `TemplatePath`, `TemplatePathFromString`, `TemplatePathSnapshot`, `TemplatePathParts` | [Рецепт](#example-template-path) |
| [`template/render`](../std/template/render.ok) | Записать шаблон в переданный текстовый поток: TemplateOutput соединяет программу, данные и назначение. | `TemplateOutput` | [Рецепт](#example-template-output) |
| [`template/sections`](../std/template/sections.ok) | Повторить строки счёта и подключить partial: SectionTemplate выполняет ограниченные секции над JSON. | `SectionTemplate`, `TemplateIterationBudget`, `TemplateSectionRun`, `TemplateOutputMeter`, `TemplateSectionOutput` | [Рецепт](#example-template-sections) |
| [`template/tree`](../std/template/tree.ok) | Посмотреть структуру шаблона перед обработкой: TemplateTree представляет узлы и вложенные секции. | `TemplateTreeKind`, `TemplateTree`, `TemplateTreeNode`, `TemplateTreeFrame`, `TemplateTreeBuilder` | [Рецепт](#example-template-tree) |

<a id="example-template-json"></a>

### 16.1. Составить письмо из JSON-данных

**Задача.** Составить текст письма по шаблону: TextTemplate связывает текст с именованными подстановками.

**Какие библиотеки использовать.** [`template/compile`](../std/template/compile.ok), [`template/json`](../std/template/json.ok), [`json/parse`](../std/json/parse.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** JsonTemplateStrings адаптирует явно переданный JsonValue. TextTemplate разрешает именованные пути через этот интерфейс.

```okkama
module app v1.0.0;

import "template/compile" take TextTemplate;
import "template/json" take JsonTemplateStrings;
import "json/parse" take ParsedJson;
import "io/stdout" take Stdout;

main() is int {
    let data = JsonTemplateStrings(ParsedJson("{\"user\":{\"name\":\"Ada\"},\"active\":true}"));
    Stdout().writeln(TextTemplate("Hello {{user.name}}! Active: {{active}}").string(data));
    0;
}
```

[Исходный пример](../examples/std-v4/template-json.ok).

<a id="example-template-html"></a>

### 16.2. Сформировать HTML-карточку заказа

**Задача.** Сформировать HTML-карточку клиента: HtmlTemplate применяет экранирование по контексту подстановки.

**Какие библиотеки использовать.** [`template/html`](../std/template/html.ok), [`json/value`](../std/json/value.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** HtmlTemplate получает шаблон и данные. Контекст подстановки задаёт представление текста внутри HTML.

```okkama
module app v1.0.0;
import "template/html" take HtmlTemplate;
import "json/value" take JsonText;
import "io/stdout" take Stdout;
main() is int {
    HtmlTemplate("<p title=\"{{.}}\">Hello {{.}}</p>\n").write(JsonText("Ada & <friends>"),Stdout());
    0;
}
```

[Исходный пример](../examples/std-v4/template-html.ok).

<a id="example-template-html-library"></a>

### 16.3. Переиспользовать HTML-фрагменты страницы

**Задача.** Сформировать HTML-карточку клиента: HtmlTemplate применяет экранирование по контексту подстановки.

**Какие библиотеки использовать.** [`template/html`](../std/template/html.ok), [`template/html_library`](../std/template/html_library.ok), [`json/value`](../std/json/value.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** NamedHtmlTemplate связывает имя с фрагментом. HtmlTemplateLibrary передаётся при рендеринге повторно используемых HTML partials.

```okkama
module app v1.0.0;
import "template/html" take HtmlTemplate;
import "template/html_library" take HtmlTemplateLibrary, NamedHtmlTemplate;
import "json/value" take JsonText;
import "io/stdout" take Stdout;
main() is int {
    let library=HtmlTemplateLibrary([NamedHtmlTemplate("item",HtmlTemplate("<b>{{.}}</b>"))]);
    Stdout().writeln(library.value("item").string(JsonText("Ada & Grace")));
    0;
}
```

[Исходный пример](../examples/std-v4/template-html-library.ok).

<a id="example-template-context"></a>

### 16.4. Различить текст и HTML при подстановке

**Задача.** Подать JSON в текстовый шаблон: JsonTemplateStrings адаптирует скаляры через протокол JsonWriting.

**Какие библиотеки использовать.** [`template/json`](../std/template/json.ok), [`json/value`](../std/json/value.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Один набор значений применяется к выбранному виду шаблона. Контекст HTML проверяется до выдачи результата.

```okkama
module app v1.0.0;
import "template/json" take JsonTemplateStrings;
import "json/value" take JsonText, JsonObject, JsonMember;
import "io/stdout" take Stdout;

main() is int {
    let item=JsonTemplateStrings(JsonText("Ada"));
    let user=JsonTemplateStrings(JsonObject([JsonMember("name",JsonText("Grace"))]));
    Stdout().writeln(item.string("."));
    Stdout().writeln(user.string("name"));
    0;
}
```

[Исходный пример](../examples/std-v4/template-context.ok).

<a id="example-template-partials"></a>

### 16.5. Подключить общий фрагмент письма

**Задача.** Повторить строки счёта и подключить partial: SectionTemplate выполняет ограниченные секции над JSON.

**Какие библиотеки использовать.** [`template/sections`](../std/template/sections.ok), [`template/library`](../std/template/library.ok), [`template/compile`](../std/template/compile.ok), [`json/parse`](../std/json/parse.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Соберите именованные шаблоны в TemplateLibrary. SectionTemplate получает библиотеку вместе с данными и бюджетом исполнения.

```okkama
module app v1.0.0;
import "template/sections" take SectionTemplate;
import "template/library" take TemplateLibrary, NamedTemplate;
import "template/compile" take TextTemplate;
import "json/parse" take ParsedJson;
import "io/stdout" take Stdout;
main() is int {
    let partials=TemplateLibrary([NamedTemplate("row",TextTemplate("- {{name}}\n"))]);
    let template=SectionTemplate("Users:\n{{#users}}{{>row}}{{/users}}",partials);
    Stdout().write(template.string(ParsedJson("{\"users\":[{\"name\":\"Ada\"},{\"name\":\"Grace\"}]}")));
    0;
}
```

[Исходный пример](../examples/std-v4/template-partials.ok).

<a id="example-template-library"></a>

### 16.6. Выбрать текстовый шаблон по имени

**Задача.** Собрать набор текстовых писем по именам: TemplateLibrary и NamedTemplate удерживают именованные шаблоны.

**Какие библиотеки использовать.** [`template/library`](../std/template/library.ok), [`template/compile`](../std/template/compile.ok), [`template/path`](../std/template/path.ok), [`template/model`](../std/template/model.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** NamedTemplate задаёт имя и TextTemplate. TemplateLibrary удерживает явный снимок доступных писем.

```okkama
module app v1.0.0;

import "template/library" take NamedTemplate, TemplateLibrary;
import "template/compile" take TextTemplate;
import "template/path" take TemplatePath;
import "template/model" take TemplateBinding, TemplateBindings;
import "io/stdout" take Stdout;

main() is int {
    let library = TemplateLibrary([
        NamedTemplate("greeting", TextTemplate("Hello {{name}}!")),
        NamedTemplate("footer", TextTemplate("Goodbye."))]);
    let data = TemplateBindings([TemplateBinding(TemplatePath(["name"]), "Ada")]);
    Stdout().writeln(library.value("greeting").string(data));
    Stdout().writeln(library.names().item(1));
    0;
}
```

[Исходный пример](../examples/std-v4/template-library.ok).

<a id="example-template-path"></a>

### 16.7. Адресовать имя клиента в шаблоне

**Задача.** Назвать вложенное значение шаблона: TemplatePath сохраняет точные сегменты user.name.

**Какие библиотеки использовать.** [`template/path`](../std/template/path.ok), [`compare`](../std/compare.ok).

**Как использовать.** TemplatePath хранит точные сегменты вложенного имени. При разборе строковой формы используйте TemplatePathFromString.

```okkama
module app v1.0.0;
import "template/path" take TemplatePath, TemplatePathFromString;
import "compare" take Order;
object ExampleResult {
    ExampleResult();
    value() is any {
        let name = TemplatePathFromString("user.name").value();
        let literal = TemplatePath(["user.name"]);
        [name.string(), name.segments().size(), literal.order(name) == Order.greater];
    }
}

main() is int {
    ExampleResult().value();
    return 0;
}
```

[Исходный пример](../examples/std-v4/template-path.ok).

<a id="example-template-output"></a>

### 16.8. Записать готовый шаблон в поток

**Задача.** Составить текст письма по шаблону: TextTemplate связывает текст с именованными подстановками.

**Какие библиотеки использовать.** [`template/compile`](../std/template/compile.ok), [`template/render`](../std/template/render.ok), [`template/model`](../std/template/model.ok), [`template/path`](../std/template/path.ok), [`io/buffer`](../std/io/buffer.ok), [`io/text`](../std/io/text.ok), [`encoding/utf`](../std/encoding/utf.ok).

**Как использовать.** TemplateOutput связывает программу, TemplateStrings и WritingText. Явная команда выводит результат в переданное назначение.

```okkama
module app v1.0.0;
import "template/compile" take TextTemplate;
import "template/render" take TemplateOutput;
import "template/model" take TemplateBinding, TemplateBindings;
import "template/path" take TemplatePath;
import "io/buffer" take ByteBuffer;
import "io/text" take Utf8Output;
import "encoding/utf" take Utf8Decoded;
object ExampleResult {
    ExampleResult();
    value() is string {
        let out=ByteBuffer();
        let data=TemplateBindings([TemplateBinding(TemplatePath(["name"]),"Ada")]);
        TemplateOutput(TextTemplate("Hello {{name}}!"),data,Utf8Output(origin:out.writing())).write();
        Utf8Decoded(out.content().bytes()).string();
    }
}

main() is int {
    ExampleResult().value();
    return 0;
}
```

[Исходный пример](../examples/std-v4/template-output.ok).

<a id="example-template-sections"></a>

### 16.9. Вывести все позиции счёта

**Задача.** Повторить строки счёта и подключить partial: SectionTemplate выполняет ограниченные секции над JSON.

**Какие библиотеки использовать.** [`template/sections`](../std/template/sections.ok), [`json/parse`](../std/json/parse.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** SectionTemplate получает JSON-данные, а секция повторяет её тело для элементов. Задайте бюджеты дерева, обхода и результата.

```okkama
module app v1.0.0;
import "template/sections" take SectionTemplate;
import "json/parse" take ParsedJson;
import "io/stdout" take Stdout;

main() is int {
    let source="{{#users}}{{name}}:{{#tags}} {{.}}{{/tags}}{{^tags}} no tags{{/tags}}\n{{/users}}";
    let users=ParsedJson("{\"users\":[{\"name\":\"Ada\",\"tags\":[\"math\",\"code\"]},{\"name\":\"Grace\",\"tags\":[]}]}");
    Stdout().write(SectionTemplate(source).string(users));
    0;
}
```

[Исходный пример](../examples/std-v4/template-sections.ok).

<a id="example-template-tree"></a>

### 16.10. Разобрать структуру шаблона для инструмента проверки

**Задача.** Посмотреть структуру шаблона перед обработкой: TemplateTree представляет узлы и вложенные секции.

**Какие библиотеки использовать.** [`template/tree`](../std/template/tree.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** TemplateTree представляет имена и вложенные секции как узлы. Этот слой удобен для инспектора либо собственного исполнения шаблона.

```okkama
module app v1.0.0;
import "template/tree" take TemplateTree;
import "io/stdout" take Stdout;

main() is int {
    let nodes=TemplateTree("{{#users}}Hello {{name}}!{{/users}}{{>footer}}").nodes();
    Stdout().writeln(nodes[0].text());
    Stdout().writeln(nodes[0].children().size().string());
    Stdout().writeln(nodes[1].text());
    0;
}
```

[Исходный пример](../examples/std-v4/template-tree.ok).

<a id="chapter-archives"></a>

## 17. Архивы и потоковые преобразования

Сначала представьте элемент архива: безопасное имя, вид, содержимое и метаданные. Затем выберите формат и поток. Бюджеты задаются в месте приёма данных, а кодек передаётся как отдельная capability.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`archive/entry`](../std/archive/entry.ok) | Представить файл, каталог или ссылку внутри бэкапа: ArchiveEntry и конкретные типы отделяют содержимое от формата. | `ArchiveEntryKind`, `ArchiveEntry`, `ArchiveEntryVisitor`, `ArchiveFile`, `ArchiveDirectory` | [Рецепт](#example-archive-zip-entry) |
| [`archive/limits`](../std/archive/limits.ok) | Ограничить загрузку архива: ArchiveLimits и ArchiveBudget проверяют число элементов, размеры и длины путей. | `ArchiveLimits`, `ArchiveBudget` | [Рецепт](#example-archive-limits) |
| [`archive/metadata`](../std/archive/metadata.ok) | Сохранить права, владельца и время в архиве: ArchiveMetadata и ArchiveOwnership представляют явные метаданные. | `ArchiveOwnership`, `ArchiveMetadata`, `ArchiveModificationFromEpoch` | [Рецепт](#example-archive-metadata) |
| [`archive/path`](../std/archive/path.ok) | Назвать элемент архива относительно его корня: ArchivePath проверяет и удерживает переносимое имя. | `ArchivePath`, `UnsafeArchivePath` | [Рецепт](#example-archive-path) |
| [`archive/stream`](../std/archive/stream.ok) | Обработать архив по одному элементу: ReadingArchive и WritingArchive задают потребляющий интерфейс и завершение. | `ReadingArchive`, `WritingArchive`, `ArchiveReading`, `ArchiveWriting` | [Рецепт](#example-archive-tar-stream) |
| [`archive/tar`](../std/archive/tar.ok) | Сохранить набор документов в TAR и прочитать обратно: TarArchive и TarWriting работают с явными потоками и форматом. | `TarFormat`, `TarArchive`, `TarWriting`, `DuplicateArchivePath`, `TarMemberWriting` | [Рецепт](#example-archive-tar-reading) |
| [`archive/zip`](../std/archive/zip.ok) | Выбрать параметры ZIP-записи: ZipCompression связывает метод с уровнем, ZipArchiveEntry удерживает выбор элемента. | `ZipMethod`, `ZipCompression`, `ZipArchiveEntry` | [Рецепт](#example-archive-zip-compression) |
| [`compression/read`](../std/compression/read.ok) | Читать преобразованный поток частями: CompressedReading и DecompressedReading соединяют кодек с исходным Reading. | `CompressedReading`, `DecompressedReading`, `CompressionReading`, `CompressionFlow`, `CompressingFlow` | [Рецепт](#example-compression-read) |
| [`compression/state`](../std/compression/state.ok) | Подставить потоковый кодек в обработку данных: Compressing, Decompressing и DecompressionBudget описывают взаимодействие. | `Compressing`, `Decompressing`, `CompressionOutput`, `DecompressionBudget` | [Рецепт](#example-compression-state) |

<a id="example-archive-zip-entry"></a>

### 17.1. Связать элемент бэкапа с параметрами ZIP

**Задача.** Выбрать параметры ZIP-записи: ZipCompression связывает метод с уровнем, ZipArchiveEntry удерживает выбор элемента.

**Какие библиотеки использовать.** [`archive/zip`](../std/archive/zip.ok), [`archive/entry`](../std/archive/entry.ok), [`archive/path`](../std/archive/path.ok), [`archive/metadata`](../std/archive/metadata.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Создайте ArchiveEntry, выберите ZipCompression и свяжите их через ZipArchiveEntry. Наблюдения сохраняют исходный элемент и выбор метода.

```okkama
module app v1.0.0;
import "archive/zip" take ZipArchiveEntry, ZipCompression, ZipMethod;
import "archive/entry" take ArchiveFile;
import "archive/path" take ArchivePath;
import "archive/metadata" take ArchiveMetadata, ArchiveModificationFromEpoch;
import "encoding/utf" take Utf8Encoded;
import "io/stdout" take Stdout;

main() is int {
    let metadata=ArchiveMetadata(ArchiveModificationFromEpoch(0.rat()).value(),"");
    let file=ArchiveFile(ArchivePath("hello.txt"),metadata,Utf8Encoded("hello\n").bytes());
    let stored=ZipArchiveEntry(file,ZipCompression(ZipMethod.stored));
    let deflated=ZipArchiveEntry(file,ZipCompression(ZipMethod.deflated,9));
    confirm stored.entry()==deflated.entry() else panic "entry binding changed";
    Stdout().writeln(stored.entry().path().string());
    Stdout().writeln("stored level: "+stored.compression().level().string());
    Stdout().writeln("deflate choice: "+deflated.compression().level().string());
    0;
}
```

[Исходный пример](../examples/std-v4/archive-zip-entry.ok).

<a id="example-archive-limits"></a>

### 17.2. Проверить бюджет принимаемого архива

**Задача.** Ограничить загрузку архива: ArchiveLimits и ArchiveBudget проверяют число элементов, размеры и длины путей.

**Какие библиотеки использовать.** [`archive/limits`](../std/archive/limits.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ArchiveLimits задаёт число элементов, размер одного и общий размер, длину имени. ArchiveBudget возвращает следующую допустимую версию учёта.

```okkama
module app v1.0.0;
import "archive/limits" take ArchiveLimits, ArchiveLimitExceeded;
import "io/stdout" take Stdout;

main() is int {
    let initial = ArchiveLimits(2, 4, 6, 64).budget();
    let first = initial.entered("a.bin", 4).expanded(2).expanded(2).completed();
    let second = first.entered("b.bin", 0).expanded(2);
    Stdout().writeln(second.path().string());
    Stdout().writeln(second.total_remaining().string());
    try { second = second.expanded(1); }
    catch error is ArchiveLimitExceeded { Stdout().writeln(error.text()); }
    let ready = second.completed();
    Stdout().writeln(ready.entries_remaining().string());
    0;
}
```

[Исходный пример](../examples/std-v4/archive-limits.ok).

<a id="example-archive-metadata"></a>

### 17.3. Сохранить владельца, права и дату в архиве

**Задача.** Сохранить права, владельца и время в архиве: ArchiveMetadata и ArchiveOwnership представляют явные метаданные.

**Какие библиотеки использовать.** [`archive/metadata`](../std/archive/metadata.ok), [`numbers/bigint`](../std/numbers/bigint.ok), [`fs/stat`](../std/fs/stat.ok), [`time/iso`](../std/time/iso.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ArchiveOwnership и ArchiveMetadata получают явные координаты. Передавайте их элементу отдельно от содержимого и архивного формата.

```okkama
module app v1.0.0;
import "archive/metadata" take ArchiveMetadata, ArchiveOwnership, ArchiveModificationFromEpoch, MissingArchiveMode;
import "numbers/bigint" take BigInt;
import "fs/stat" take PermissionMode;
import "time/iso" take IsoLocalDateTime;
import "io/stdout" take Stdout;

main() is int {
    let modified = ArchiveModificationFromEpoch(rat(-1, 2)).value();
    let owner = ArchiveOwnership("builder", "build", BigInt("1000"), BigInt("1000"));
    let member = ArchiveMetadata(PermissionMode(420), modified, owner, "exact timestamp");
    Stdout().writeln(IsoLocalDateTime(member.modification()).string());
    Stdout().writeln(member.ownership().user());
    Stdout().writeln(member.comment());
    let plain = ArchiveMetadata(modified, "mode absent");
    try { plain.mode(); }
    catch error is MissingArchiveMode { Stdout().writeln(error.text()); }
    0;
}
```

[Исходный пример](../examples/std-v4/archive-metadata.ok).

<a id="example-archive-path"></a>

### 17.4. Сохранить переносимое имя внутри архива

**Задача.** Назвать элемент архива относительно его корня: ArchivePath проверяет и удерживает переносимое имя.

**Какие библиотеки использовать.** [`archive/path`](../std/archive/path.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ArchivePath получает относительное имя. Пользуйтесь его проекциями вместо смешивания архивного имени с host-путём файла.

```okkama
module app v1.0.0;
import "archive/path" take ArchivePath, UnsafeArchivePath, MissingArchiveParent;
import "io/stdout" take Stdout;

main() is int {
    let member = ArchivePath("assets/иконка.png");
    Stdout().writeln(member.string());
    Stdout().writeln(member.parent().string());
    Stdout().writeln(member.name());
    try { ArchivePath("assets/../outside"); }
    catch error is UnsafeArchivePath { Stdout().writeln(error.text()); }
    try { member.parent().parent(); }
    catch error is MissingArchiveParent { Stdout().writeln(error.text()); }
    0;
}
```

[Исходный пример](../examples/std-v4/archive-path.ok).

<a id="example-archive-tar-stream"></a>

### 17.5. Записать TAR в переданный поток

**Задача.** Сохранить набор документов в TAR и прочитать обратно: TarArchive и TarWriting работают с явными потоками и форматом.

**Какие библиотеки использовать.** [`archive/tar`](../std/archive/tar.ok), [`archive/stream`](../std/archive/stream.ok), [`archive/entry`](../std/archive/entry.ok), [`archive/path`](../std/archive/path.ok), [`archive/metadata`](../std/archive/metadata.ok), [`archive/limits`](../std/archive/limits.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/buffer`](../std/io/buffer.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Соберите элементы, формат и Writing. После записи завершите архив выбранной командой владельца.

```okkama
module app v1.0.0;
import "archive/tar" take TarWriting, TarReading;
import "archive/stream" take EndOfArchive;
import "archive/entry" take ArchiveFile;
import "archive/path" take ArchivePath;
import "archive/metadata" take ArchiveMetadata, ArchiveModificationFromEpoch;
import "archive/limits" take ArchiveLimits;
import "encoding/utf" take Utf8Encoded;
import "io/buffer" take ByteBuffer;
import "io/stdout" take Stdout;

main() is int {
    let wire=ByteBuffer();defer wire.closing().close();
    let metadata=ArchiveMetadata(ArchiveModificationFromEpoch(rat(0,1)).value(), "");
    let writer=TarWriting(wire.writing());
    writer.write(ArchiveFile(ArchivePath("hello.txt"),metadata,Utf8Encoded("hello\n").bytes()));
    writer.write(ArchiveFile(ArchivePath("empty.txt"),metadata,[]));
    writer.finish();writer.close();

    let reader=TarReading(wire.reading(),ArchiveLimits(2,1024,2048,255));
    let first=reader.entry();Stdout().writeln(first.path().string());
    let second=reader.entry();Stdout().writeln(second.path().string());
    confirm !reader.finished() else panic "EOF observed too early";
    let complete=try {reader.entry();false;} catch end is EndOfArchive {true;};
    confirm complete && reader.finished() else panic "expected normal EOF";
    reader.close();
    confirm !wire.closing().closed() else panic "borrowed input was closed";
    Stdout().writeln("retained after close: "+first.path().string());
    0;
}
```

[Исходный пример](../examples/std-v4/archive-tar-stream.ok).

<a id="example-archive-tar-reading"></a>

### 17.6. Прочитать элементы TAR по очереди

**Задача.** Сохранить набор документов в TAR и прочитать обратно: TarArchive и TarWriting работают с явными потоками и форматом.

**Какие библиотеки использовать.** [`archive/tar`](../std/archive/tar.ok), [`archive/entry`](../std/archive/entry.ok), [`archive/path`](../std/archive/path.ok), [`archive/metadata`](../std/archive/metadata.ok), [`archive/limits`](../std/archive/limits.ok), [`collections/list`](../std/collections/list.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** TarArchive связывает Reading и policy. Читайте текущий элемент, обрабатывайте его и продолжайте до EndOfArchive.

```okkama
module app v1.0.0;
import "archive/tar" take TarArchive, ParsedTar;
import "archive/entry" take ArchiveEntry, ArchiveFile;
import "archive/path" take ArchivePath;
import "archive/metadata" take ArchiveMetadata, ArchiveModificationFromEpoch;
import "archive/limits" take ArchiveLimits;
import "collections/list" take List, EmptyList;
import "encoding/utf" take Utf8Encoded;
import "io/stdout" take Stdout;

main() is int {
    let metadata=ArchiveMetadata(ArchiveModificationFromEpoch(rat(-1,2)).value(), "example");
    let file=ArchiveFile(ArchivePath("документы/hello.txt"),metadata,Utf8Encoded("hello\n").bytes());
    let empty is List<ArchiveEntry> = EmptyList();
    let wire=TarArchive(empty.prefix(file)).bytes();
    let parsed=ParsedTar(wire,ArchiveLimits(4,1024,4096,255));
    let entries=parsed.entries();
    Stdout().writeln("members: "+entries.size().string());
    Stdout().writeln(entries.first().path().string());
    confirm TarArchive(entries).bytes()==wire else panic "TAR round trip differs";
    Stdout().writeln("PAX round trip agrees");
    0;
}
```

[Исходный пример](../examples/std-v4/archive-tar-reading.ok).

<a id="example-archive-zip-compression"></a>

### 17.7. Выбрать метод и уровень ZIP-представления

**Задача.** Выбрать параметры ZIP-записи: ZipCompression связывает метод с уровнем, ZipArchiveEntry удерживает выбор элемента.

**Какие библиотеки использовать.** [`archive/zip`](../std/archive/zip.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ZipCompression нормализует выбранный метод и уровень. Повторное использование параметров сохраняет один явный профиль каждого элемента.

```okkama
module app v1.0.0;
import "archive/zip" take ZipMethod, ZipCompression;
import "io/stdout" take Stdout;

main() is int {
    let stored=ZipCompression(ZipMethod.stored);
    let deflate=ZipCompression(ZipMethod.deflated);
    let fast=ZipCompression(ZipMethod.zstandard,-1);
    Stdout().writeln("stored level: "+stored.level().string());
    Stdout().writeln("deflate default: "+deflate.level().string());
    Stdout().writeln("zstandard choice: "+fast.level().string());
    confirm ZipCompression(ZipMethod.zstandard,0)==ZipCompression(ZipMethod.zstandard)
        else panic "default alias did not resolve";
    0;
}
```

[Исходный пример](../examples/std-v4/archive-zip-compression.ok).

<a id="example-compression-read"></a>

### 17.8. Проверить чтение потока через codec

**Задача.** Подставить потоковый кодек в обработку данных: Compressing, Decompressing и DecompressionBudget описывают взаимодействие.

**Какие библиотеки использовать.** [`compression/state`](../std/compression/state.ok), [`compression/read`](../std/compression/read.ok), [`io/buffer`](../std/io/buffer.ok).

**Как использовать.** CompressedReading связывает Reading с codec capability. Здесь Compressing.fake() — тестовый двойник: проверяются выдача chunks, finish и владение ресурсами. Выполните файл в режиме -t.

```okkama
module app v1.0.0;
import "compression/state" take Compressing;
import "compression/read" take CompressedReading;
import "io/buffer" take ByteBuffer;

object ReadingExample {
    ReadingExample();
    run() is void {
        let source = ByteBuffer([byte(1), byte(2), byte(255)]);
        let codec = Compressing.fake();
        let stream = CompressedReading(source.reading(), codec, 1);
        test.is(stream.bytes(1), [byte(1)]);
        test.is(stream.bytes(8), [byte(2), byte(255)]);
        test.is(stream.bytes(1).size(), 0);
        test.ok(codec.finished());
        stream.close();
        test.ok(!codec.closed());
        test.ok(!source.closing().closed());
        codec.close();
        source.closing().close();
    }
}
test "bounded reading with a caller-owned codec and source" {
    ReadingExample().run();
}
```

[Исходный пример](../examples/std-v4/compression-read.ok).

<a id="example-compression-state"></a>

### 17.9. Связать потоковый codec с бюджетом

**Задача.** Подставить потоковый кодек в обработку данных: Compressing, Decompressing и DecompressionBudget описывают взаимодействие.

**Какие библиотеки использовать.** [`compression/state`](../std/compression/state.ok), [`encoding/hex`](../std/encoding/hex.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Compressing и Decompressing описывают выдачу блоков и finish. DecompressionBudget проверяет рост результата при обработке входа.

```okkama
module app v1.0.0;
import "compression/state" take CompressionOutput, DecompressionBudget;
import "encoding/hex" take HexEncoded;
import "io/stdout" take Stdout;

main() is int {
    let budget = DecompressionBudget(3).spent(3);
    let output = CompressionOutput([byte(1), byte(2), byte(255)]);
    Stdout().writeln(HexEncoded(output.bytes(2)).string());
    Stdout().writeln(HexEncoded(output.bytes(2)).string());
    Stdout().writeln(budget.remaining().string());
    output.close();
    0;
}
```

[Исходный пример](../examples/std-v4/compression-state.ok).

<a id="chapter-database"></a>

## 18. Базы данных: SQL, SQLite, транзакции и пул

Statement связывает SQL с отдельными DbValue-параметрами. Строка результата — immutable snapshot, который RowReader переводит в строгие скалярные проекции; предметный объект создаётся своим конструктором. Соединения, строки, подготовленные запросы и транзакции имеют отдельное время жизни.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`db/acquisition`](../std/db/acquisition.ok) | Передать коннектор в пул соединений: DbAcquiring и DbAcquisition явно связывают переносимую и дополнительную capability. | `DbAcquiring`, `DbAcquisition`, `PortableDbAcquiring` | [Рецепт](#example-database-stack) |
| [`db/connection`](../std/db/connection.ok) | Написать repository с узкой зависимостью: DbExecuting задаёт execution и rows, DbConnection владеет соединением. | `DbExecuting`, `DbConnection`, `DbConnecting` | [Рецепт](#example-database-stack) |
| [`db/error`](../std/db/error.ok) | Различить constraint и busy при записи: DbFailure сохраняет переносимый вид отказа, операцию и код provider. | `DbOperation`, `DbOperationText`, `AffectedRowsUnknown`, `DbClosed`, `DbBusy` | [Рецепт](#recipe-db-row-read) |
| [`db/pool`](../std/db/pool.ok) | Поделить соединения между обработчиками запросов: DbPool и DbPreparedPool выдают lease с бюджетами ожидания. | `PoolOptions`, `PoolSnapshot`, `DbChecking`, `UncheckedDbConnections`, `DbPool` | [Рецепт](#example-database-stack) |
| [`db/prepared`](../std/db/prepared.ok) | Повторить один SQL с разными значениями: Prepared и PreparedStatement удерживают подготовленный запрос и срок его жизни. | `DbPreparing`, `DbPreparedOpening`, `PreparedStatement`, `Prepared`, `DbPreparedTransaction` | [Рецепт](#example-database-stack) |
| [`db/read`](../std/db/read.ok) | Восстановить предметные поля из результата SQL: RowReader строго читает int, text и integer-флаги 0/1 по имени. | `RowReader` | [Рецепт](#recipe-db-row-read) |
| [`db/row`](../std/db/row.ok) | Прочитать найденную запись: Row хранит immutable snapshot, Column задаёт метаданные, RowReading потребляет строки. | `Column`, `DbColumns`, `Row`, `RowReading`, `RowsEnded` | [Рецепт](#recipe-db-row-read) |
| [`db/sqlite/backup`](../std/db/sqlite/backup.ok) | Сделать согласованную копию рабочей SQLite: SqliteBackup возвращает BackupReport о скопированных страницах. | `SqliteBackupMaking`, `SqliteBackup`, `BackupReport` | [Рецепт](#example-database-stack) |
| [`db/sqlite/blob`](../std/db/sqlite/blob.ok) | Записать большой двоичный объект частями: OpenedSqliteBlob открывает адресуемый blob с явным смещением. | `SqliteBlobOpening`, `SqliteBlob`, `OpenedSqliteBlob` | [Рецепт](#example-database-stack) |
| [`db/sqlite/script`](../std/db/sqlite/script.ok) | Применить известную последовательность DDL: SqliteScript выполняет явно заданный SQL-скрипт. | `SqliteScripting`, `SqliteScript` | [Рецепт](#example-database-stack) |
| [`db/sqlite`](../std/db/sqlite.ok) | Открыть локальную базу приложения: SqliteDatabase и SqliteMemory приобретают SQLite-соединение с явными options. | `SqliteJournal`, `SqliteOptions`, `SqliteDatabase`, `SqliteMemory`, `SqliteConnector` | [Рецепт](#example-database-stack) |
| [`db/statement`](../std/db/statement.ok) | Выполнить запрос с параметрами: Statement хранит SQL отдельно от значений, Execution описывает affected rows. | `DbLimits`, `DbValueSnapshot`, `Statement`, `DbSqlText`, `Execution` | [Рецепт](#example-db-values) |
| [`db/transaction`](../std/db/transaction.ok) | Сохранить заказ и позиции атомарно: TransactionOptions и Transaction разделяют выполнение, commit и rollback. | `Isolation`, `TransactionOptions`, `Transaction` | [Рецепт](#example-database-stack) |
| [`db/value`](../std/db/value.ok) | Передать цену и имя в SQL как параметры: DbInteger, DbText и другие DbValue публикуют типизированное значение. | `DbValueKind`, `DbValue`, `DbValueWriting`, `DbNull`, `DbBoolean` | [Рецепт](#recipe-db-row-read) |

В PostgreSQL-адаптере примеров целый NUMERIC читается как `DbInteger` с `BigInt`,
дробный — как `DbDecimal`. Для суммы за пределами int64 используйте
`ObservedDbValue(row.value("total")).integer()`; `RowReader.int` сохраняет строгий
диапазон. [Точный контракт NUMERIC и аналитический пример](sql-numeric-transport.md).

### Что учитывать при сборке решения

- `db/pool`: Верните lease через close, завершите работу пула через settled. Время пользования rows и prepared statement входит в управление соединением.
- `db/read`: int проверяет signed 64-bit диапазон, flag принимает integer 0/1. RowValueFailure сохраняет column, expected, actual_kind и cause.
- `db/transaction`: На отказе откатывайте активную транзакцию через defer. Commit выполняется после всех предметных проверок.

<a id="recipe-db-row-read"></a>

### 18.1. Прочитать id, имя и флаг из SQL

**Задача.** После SELECT нужно получить проверенные скалярные значения и передать их предметному конструктору.

**Какие библиотеки использовать.** [`db/read`](../std/db/read.ok), [`db/row`](../std/db/row.ok), [`db/value`](../std/db/value.ok), [`numbers/bigint`](../std/numbers/bigint.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Row здесь построен в памяти, поэтому пример запускается независимо от provider. В реальном запросе тот же RowReader получает await rows.row(); RowValueFailure сохраняет имя столбца, ожидаемую проекцию, actual_kind и cause.

`int` принимает SQL integer в диапазоне от −9223372036854775808 до 9223372036854775807. `flag` читает именно integer 0/1; SQL BOOLEAN остаётся отдельным типом. NULL и неверный тип дают RowValueFailure, а отсутствующее или неоднозначное имя колонки — MissingColumn или AmbiguousColumn. Предметные метки проверяйте явным разбором: например, USD/EUR должны давать соответствующий Currency, а неизвестная метка — ошибку. В billing-примере такой разбор дополняет RowReader контекстом колонки и ожидаемого доменного типа.

```okkama
module cookbook/db_row_read v1.0.0;
import "db/read" take RowReader;
import "db/row" take Row,Column;
import "db/value" take DbInteger,DbText;
import "numbers/bigint" take BigInt;
import "io/stdout" take Stdout;

main() is int {
    let row = Row(
        [Column(0,"id","INTEGER"),Column(1,"name","TEXT"),Column(2,"verified","INTEGER")],
        [DbInteger(BigInt("7")),DbText("Ада"),DbInteger(BigInt("1"))]
    );
    let fields = RowReader(row);
    confirm fields.int("id") == 7 && fields.flag("verified");
    Stdout().writeln(fields.text("name"));
    0;
}
```

<a id="example-database-stack"></a>

### 18.2. Сохранить запись, blob и резервную копию SQLite

**Задача.** Открыть локальную базу приложения: SqliteDatabase и SqliteMemory приобретают SQLite-соединение с явными options.

**Какие библиотеки использовать.** [`db/sqlite`](../std/db/sqlite.ok), [`db/transaction`](../std/db/transaction.ok), [`db/statement`](../std/db/statement.ok), [`db/value`](../std/db/value.ok), [`db/prepared`](../std/db/prepared.ok), [`db/pool`](../std/db/pool.ok), [`db/sqlite/blob`](../std/db/sqlite/blob.ok), [`db/sqlite/backup`](../std/db/sqlite/backup.ok), [`db/sqlite/script`](../std/db/sqlite/script.ok), [`numbers/bigint`](../std/numbers/bigint.ok), [`fs/temp`](../std/fs/temp.ok), [`fs/directory`](../std/fs/directory.ok), [`fs/path`](../std/fs/path.ok), [`time/clock`](../std/time/clock.ok), [`time/duration`](../std/time/duration.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Откройте SQLite, выполните SQL внутри транзакции и закройте blob до commit. Подготовленный запрос, backup и pool получают собственные явные capability.

```okkama
module example v1.0.0;
import "db/sqlite" take SqliteMemory,SqliteDatabase,SqliteAcquiring,SqliteOptions,SqliteTransactions,SqliteTransactionMode;
import "db/transaction" take TransactionOptions;
import "db/statement" take Statement;
import "db/value" take DbInteger,DbText,ObservedDbValue;
import "db/prepared" take Prepared;
import "db/pool" take DbPreparedPool,PooledTransactions,PoolOptions;
import "db/sqlite/blob" take OpenedSqliteBlob;
import "db/sqlite/backup" take SqliteBackup;
import "db/sqlite/script" take SqliteScript;
import "numbers/bigint" take BigInt;
import "fs/temp" take TemporaryDirectory;
import "fs/directory" take Directory;
import "fs/path" take Path;
import "time/clock" take SystemClock;
import "time/duration" take Duration,DurationUnit;
import "io/stdout" take Stdout;

async main() is int{
    let scratch=TemporaryDirectory(Directory(Path("/tmp")),"okkama-db-");
    defer scratch.close();
    let connection=await SqliteMemory().connection();
    defer await connection.settled();
    defer connection.close();
    await SqliteScript(connection,"CREATE TABLE customers(id INTEGER PRIMARY KEY,name TEXT,payload BLOB);").executions();
    let transaction=await SqliteTransactions(connection,SqliteTransactionMode.immediate).transaction(TransactionOptions());
    await transaction.execution(Statement("INSERT INTO customers VALUES(?,?,zeroblob(4))",[DbInteger(BigInt("7")),DbText("Ada")]));
    let blob=await OpenedSqliteBlob(transaction,"main","customers","payload",BigInt("7"),true).blob();
    await blob.write(BigInt("0"),[65.byte(),0.byte(),66.byte()]);
    blob.close();
    await transaction.commit();
    let prepared=await Prepared(connection,"SELECT name FROM customers WHERE id=?").statement();
    defer prepared.close();
    let rows=await prepared.rows([DbInteger(BigInt("7"))]);
    let row=await rows.row();
    rows.close();
    Stdout().writeln("customer: "+ObservedDbValue(row.value("name")).text());
    let copy=Path(scratch.path().string()+"/copy.sqlite");
    let report=await SqliteBackup(connection,copy,1,SystemClock(),Duration(2.rat(),DurationUnit.second)).result();
    Stdout().writeln("backup pages: "+report.copies().string()+"/"+report.total().string());
    let options=PoolOptions(1,2,Duration(30.rat(),DurationUnit.second),Duration(60.rat(),DurationUnit.second),Duration(2.rat(),DurationUnit.second));
    let pool=DbPreparedPool(SqliteAcquiring(SqliteDatabase(copy,SqliteOptions())),options,SystemClock());
    defer await pool.settled();
    defer pool.close();
    let lease=await pool.connection();
    let pooled_transaction=await PooledTransactions(lease).transaction(TransactionOptions());
    let pooled_prepared=await Prepared(pooled_transaction,"SELECT count(*) AS total FROM customers").statement();
    let selected=await pooled_prepared.rows([]);
    let count=await selected.row();
    selected.close();
    pooled_prepared.close();
    await pooled_transaction.commit();
    lease.close();
    await pool.settled();
    Stdout().writeln("pooled customers: "+ObservedDbValue(count.value("total")).integer().string());
    0;
}
```

[Исходный пример](../examples/std-v4/database-stack.ok).

<a id="example-db-values"></a>

### 18.3. Привязать SQL-параметр и прочитать имя результата

**Задача.** Передать цену и имя в SQL как параметры: DbInteger, DbText и другие DbValue публикуют типизированное значение.

**Какие библиотеки использовать.** [`db/value`](../std/db/value.ok), [`db/read`](../std/db/read.ok), [`db/statement`](../std/db/statement.ok), [`db/row`](../std/db/row.ok), [`numbers/bigint`](../std/numbers/bigint.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Statement хранит текст и DbValue отдельно. RowReader читает по имени столбца, сохраняя строгую проекцию text.

```okkama
module example v1.0.0;
import "db/value" take DbInteger,DbText;
import "db/read" take RowReader;
import "db/statement" take Statement;
import "db/row" take Row,Column;
import "numbers/bigint" take BigInt;
import "io/stdout" take Stdout;
main() is int{
    let statement=Statement("SELECT id, name FROM customers WHERE id = ?",[DbInteger(BigInt("7"))]);
    let row=Row([Column(0,"id","INTEGER"),Column(1,"name","TEXT")],[DbInteger(BigInt("7")),DbText("Ada")]);
    Stdout().writeln(statement.values().size().string()+" binding: "+RowReader(row).text("name"));0;
}
```

[Исходный пример](../examples/std-v4/db-values.ok).

<a id="chapter-network"></a>

## 19. Сети: адреса, DNS, TCP, UDP, Unix и TLS

Имя узла, IP-адрес и endpoint с портом представляют разные стадии подключения. Resolver выдаёт кандидатов, политика проверяет адресата, а соединение владеет асинхронными потоками. UDP сохраняет границу каждой датаграммы.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`net/dns`](../std/net/dns.ok) | Получить адреса сервера или подставить тестовый DNS: SystemResolution и FixedResolution реализуют Resolving. | `Resolving`, `HostAddresses`, `AddressName`, `FixedResolution`, `SystemResolution` | [Рецепт](#example-system-resolution) |
| [`net/endpoint`](../std/net/endpoint.ok) | Связать сервер с портом: HostEndpoint и IpEndpoint отделяют DNS-имя от числового адреса. | `HostName`, `HostEndpoint`, `IpEndpoint` | [Рецепт](#example-host-policy) |
| [`net/failure`](../std/net/failure.ok) | Обработать таймаут подключения отдельно от DNS: NetworkFailure сохраняет вид и операцию отказа. | `NetworkOperation`, `NetworkClosed` | [Рецепт](#example-network-failure) |
| [`net/interface`](../std/net/interface.ok) | Выбрать локальный сетевой интерфейс: NetworkInterface даёт явное системное наблюдение. | `NetworkInterface` | [Рецепт](#example-network-interface) |
| [`net/ip`](../std/net/ip.ok) | Проверить адрес и принадлежность подсети: IpAddress и IpNetwork работают с IPv4 и IPv6. | `IpVersion`, `IpProperty`, `IpAddress`, `IpNetwork`, `IpInterface` | [Рецепт](#example-ip-interface) |
| [`net/ip_registry`](../std/net/ip_registry.ok) | Классифицировать специальный диапазон IP: IpClassification использует встроенные сведения о назначении префиксов. | `IpClassification`, `PackedIpPrefix` | [Рецепт](#example-ip-interface) |
| [`net/multicast`](../std/net/multicast.ok) | Получать объявления в локальной сети: MulticastOptions и MulticastMembership описывают участие в группе. | `MulticastOptions`, `MulticastJoining`, `MulticastMembership` | [Рецепт](#example-multicast-options) |
| [`net/public_suffix`](../std/net/public_suffix.ok) | Выделить регистрируемый домен сайта: PublicSuffixList учитывает встроенные правила доменных суффиксов. | `PublicSuffixList` | [Рецепт](#example-host-policy) |
| [`net/stream`](../std/net/stream.ok) | Принять TCP либо TLS в одном протоколе: AsyncReading, AsyncWriting и StreamConnection ограничивают транспортный контракт. | `AsyncReading`, `AsyncWriting`, `ConnectionEndpoints`, `StreamConnection`, `TlsTcpOrigin` | [Рецепт](#example-smtp-reading) |
| [`net/tcp`](../std/net/tcp.ok) | Открыть TCP-соединение или слушающий порт: TcpConnecting и TcpListening возвращают владельцев асинхронного транспорта. | `TcpOptions`, `TcpListeningOptions`, `TcpConnection`, `TcpConnecting`, `TcpListening` | [Рецепт](#example-tcp-options) |
| [`net/tcp_racing`](../std/net/tcp_racing.ok) | Быстро выбрать доступный адрес сервера: TcpRacing управляет несколькими проверенными кандидатами подключения. | `TcpRaceEndpoints`, `TcpRacing`, `TcpRaceWin`, `TcpRaceAttempt`, `TcpRaceDelay` | [Рецепт](#example-tcp-racing) |
| [`net/tls`](../std/net/tls.ok) | Подключиться к серверу с проверкой TLS: TlsClientPolicy связывает доверие, версии протокола и ALPN. | `TlsVersion`, `TlsClientPolicy`, `TlsPhase`, `TlsClientHandshake`, `TlsUnpublished` | [Рецепт](#recipe-https-request) |
| [`net/udp`](../std/net/udp.ok) | Отправить датаграмму и получить адрес отправителя: UdpSocket, UdpConnection и Datagram сохраняют границу пакета. | `Datagram`, `DatagramTooLarge`, `UdpOptions`, `UdpBinding`, `UdpMembership` | [Рецепт](#example-udp-options) |
| [`net/unix`](../std/net/unix.ok) | Соединить локальные процессы через Unix socket: UnixAddress, UnixConnecting, UnixListening и UnixPaired задают транспорт. | `UnixAddress`, `UnixSocketsUnsupported`, `UnixEndpoint`, `UnixEndpointBytes`, `UnixConnection` | [Рецепт](#example-http1-writing) |

### Что учитывать при сборке решения

- `net/tls`: Выбор TrustStore, идентичности сервера, версий TLS и ALPN выполняется в месте сборки политики.

<a id="example-udp-receive"></a>

### 19.1. Получить датаграмму и адрес отправителя

**Задача.** Проверить адрес и принадлежность подсети: IpAddress и IpNetwork работают с IPv4 и IPv6.

**Какие библиотеки использовать.** [`net/ip`](../std/net/ip.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/udp`](../std/net/udp.ok), [`async/timeout`](../std/async/timeout.ok), [`time/clock`](../std/time/clock.ok), [`time/duration`](../std/time/duration.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Откройте UDP-владельца на выбранном endpoint. Результат receive сохраняет payload, адрес отправителя и наблюдение truncation.

```okkama
module app v1.0.0;
import "net/ip" take IpAddress;
import "net/endpoint" take IpEndpoint;
import "net/udp" take UdpBinding, UdpOptions, Datagram;
import "async/timeout" take Within;
import "time/clock" take SystemClock;
import "time/duration" take Duration, DurationUnit;
import "io/stdout" take Stdout;

async main() is int {
    let socket=await UdpBinding(IpEndpoint(IpAddress("127.0.0.1"),0),UdpOptions()).socket();
    Stdout().writeln(socket.local_endpoint().port().string());
    let request is Within<Datagram>=Within(socket.datagram(4),
        Duration(5.rat(),DurationUnit.second),SystemClock());
    let packet=await request.result();
    socket.close();
    Stdout().writeln(packet.source().address().string());
    Stdout().writeln(packet.bytes().size().string());
    Stdout().writeln(if packet.truncated() {"truncated";} else {"complete";});
    0;
}
```

[Исходный пример](../examples/std-v4/udp-receive.ok).

<a id="example-system-resolution"></a>

### 19.2. Разрешить DNS-имя через системный resolver

**Задача.** Получить адреса сервера или подставить тестовый DNS: SystemResolution и FixedResolution реализуют Resolving.

**Какие библиотеки использовать.** [`net/dns`](../std/net/dns.ok), [`net/endpoint`](../std/net/endpoint.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** SystemResolution предоставляет Resolving. Передайте HostEndpoint и обработайте полученные числовые кандидаты в своей политике подключения.

```okkama
module app v1.0.0;
import "net/dns" take SystemResolution;
import "net/endpoint" take HostName,HostEndpoint;
import "io/stdout" take Stdout;
async main() is int {
    let resolver=SystemResolution();let result=await resolver.addresses(HostEndpoint(HostName("localhost"),80));
    confirm result.size()>0;
    for (0..result.size()) -> index {confirm result.item(index).port()==80;}
    Stdout().writeln("system resolver worker and requested ports verified");0;
}
```

[Исходный пример](../examples/std-v4/system-resolution.ok).

<a id="example-network-failure"></a>

### 19.3. Различить сетевые отказы по операции

**Задача.** Обработать таймаут подключения отдельно от DNS: NetworkFailure сохраняет вид и операцию отказа.

**Какие библиотеки использовать.** [`net/failure`](../std/net/failure.ok), [`runtime/network`](../std/runtime/network.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** NetworkFailure сохраняет переносимый вид и операцию. Выбирайте обработчик по этим наблюдениям вместо разбора текста provider.

```okkama
module app v1.0.0;
import "net/failure" take NetworkOperation, NetworkFailureKind, NetworkFailure, NetworkError;
import "runtime/network" take socket;
import "io/stdout" take Stdout;

main() is int {
    let result=try {
        try {socket(99,1,1);"unexpected success";}
        catch failure is NativeFailure {
            let translated=NetworkError(NetworkOperation.binding,failure);
            if translated.applicable() {throw translated.value();}
            throw;
        }
    } catch failure is NetworkFailure {
        confirm failure.operation()==NetworkOperation.binding
            && failure.kind()==NetworkFailureKind.unsupported else panic "wrong classification";
        failure.text();
    };
    Stdout().writeln(result);
    0;
}
```

[Исходный пример](../examples/std-v4/network-failure.ok).

<a id="example-network-interface"></a>

### 19.4. Получить сведения локального интерфейса

**Задача.** Проверить адрес и принадлежность подсети: IpAddress и IpNetwork работают с IPv4 и IPv6.

**Какие библиотеки использовать.** [`net/ip`](../std/net/ip.ok), [`net/interface`](../std/net/interface.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** NetworkInterface наблюдает имя и связанные с интерфейсом данные. Полученный результат можно передать в выбор multicast либо endpoint.

```okkama
module app v1.0.0;
import "net/ip" take IpAddress, IpInterface;
import "net/interface" take NetworkInterface;
import "io/stdout" take Stdout;

main() is int {
    let addresses is @IpInterface=[IpInterface(IpAddress("192.0.2.7"),24),
        IpInterface(IpAddress("2001:db8::8"),64)];
    let interface=NetworkInterface("example0",2,addresses,true,false);
    Stdout().writeln(interface.name()+" #"+interface.index().string());
    for interface.addresses() -> address {Stdout().writeln(address.string());}
    0;
}
```

[Исходный пример](../examples/std-v4/network-interface.ok).

<a id="example-ip-interface"></a>

### 19.5. Связать адрес интерфейса с префиксом сети

**Задача.** Проверить адрес и принадлежность подсети: IpAddress и IpNetwork работают с IPv4 и IPv6.

**Какие библиотеки использовать.** [`net/ip`](../std/net/ip.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Постройте IP-представления из числовой строки и длины префикса. Используйте нормализованный network для проверки membership.

```okkama
module app v1.0.0;
import "net/ip" take IpAddress, IpInterface;
import "io/stdout" take Stdout;

main() is int {
    let interface=IpInterface(IpAddress("2001:0DB8::7"),64);
    Stdout().writeln(interface.string());
    Stdout().writeln(interface.network().network().string());
    0;
}
```

[Исходный пример](../examples/std-v4/ip-interface.ok).

<a id="example-multicast-options"></a>

### 19.6. Выбрать параметры локальной multicast-группы

**Задача.** Получать объявления в локальной сети: MulticastOptions и MulticastMembership описывают участие в группе.

**Какие библиотеки использовать.** [`net/multicast`](../std/net/multicast.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** MulticastOptions задаёт группу и интерфейсные координаты. Membership удерживает время участия открытого UDP-ресурса.

```okkama
module app v1.0.0;
import "net/multicast" take MulticastOptions;
import "io/stdout" take Stdout;

main() is int {
    let options=MulticastOptions(hop_limit:1,loopback:false);
    Stdout().writeln("hop limit: "+options.hop_limit().string());
    Stdout().writeln(if options.loopback() {"local delivery requested";} else {"local delivery disabled";});
    0;
}
```

[Исходный пример](../examples/std-v4/multicast-options.ok).

<a id="example-host-policy"></a>

### 19.7. Ограничить сетевые адреса клиента

**Задача.** Связать сервер с портом: HostEndpoint и IpEndpoint отделяют DNS-имя от числового адреса.

**Какие библиотеки использовать.** [`net/endpoint`](../std/net/endpoint.ok), [`net/public_suffix`](../std/net/public_suffix.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Представьте IP-сеть и передайте её ListedNetworks. Политика получает конкретных числовых кандидатов resolver.

```okkama
module app v1.0.0;
import "net/endpoint" take HostName, HostEndpoint;
import "net/public_suffix" take PublicSuffixList;
import "io/stdout" take Stdout;
main() is int {
    let host=HostName("BÜCHER.example");
    Stdout().writeln(HostEndpoint(host,443).string());
    let rules=PublicSuffixList();
    Stdout().writeln(rules.version());
    Stdout().writeln(rules.registrable(HostName("a.www.ck")).ascii());
    0;
}
```

[Исходный пример](../examples/std-v4/host-policy.ok).

<a id="example-tcp-options"></a>

### 19.8. Настроить параметры TCP-подключения

**Задача.** Открыть TCP-соединение или слушающий порт: TcpConnecting и TcpListening возвращают владельцев асинхронного транспорта.

**Какие библиотеки использовать.** [`net/tcp`](../std/net/tcp.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** TcpOptions и listening options являются явными значениями. Передавайте их соответствующей connecting/listening capability.

```okkama
module app v1.0.0;
import "net/tcp" take TcpOptions, TcpListeningOptions;
import "io/stdout" take Stdout;

main() is int {
    let connection=TcpOptions(true,true);
    let listener=TcpListeningOptions(false,false,128);
    Stdout().writeln(if connection.no_delay() {"no-delay requested";} else {"normal packet delay";});
    Stdout().writeln("requested backlog: "+listener.backlog().string());
    0;
}
```

[Исходный пример](../examples/std-v4/tcp-options.ok).

<a id="example-tcp-racing"></a>

### 19.9. Выбрать доступный адрес из нескольких кандидатов

**Задача.** Открыть TCP-соединение или слушающий порт: TcpConnecting и TcpListening возвращают владельцев асинхронного транспорта.

**Какие библиотеки использовать.** [`net/tcp`](../std/net/tcp.ok), [`net/tcp_racing`](../std/net/tcp_racing.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/ip`](../std/net/ip.ok), [`io/decorators`](../std/io/decorators.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** TcpRacing принимает проверенные endpoints и политику задержки. Победивший транспорт публикуется после завершения проигравших попыток.

```okkama
module app v1.0.0;
import "net/tcp" take TcpListening,TcpAcceptance,TcpConnection;
import "net/tcp_racing" take TcpRacing;
import "net/endpoint" take IpEndpoint;
import "net/ip" take IpAddress;
import "io/decorators" take ByteSnapshot;
import "io/stdout" take Stdout;
async main() is int {
    let listener=await TcpListening(IpEndpoint(IpAddress("127.0.0.1"),0)).acceptance();defer listener.close();
    let accepting is async () is TcpConnection=listener.connection;let pending=spawn accepting();
    let client=await TcpRacing([listener.local_endpoint()]).connection();defer client.close();
    let peer=await pending;defer peer.close();
    await client.output().write(ByteSnapshot([7.byte()]));
    confirm (await peer.input().bytes(1))[0].int()==7;
    Stdout().writeln("owned TCP racing connection verified");0;
}
```

[Исходный пример](../examples/std-v4/tcp-racing.ok).

<a id="example-udp-options"></a>

### 19.10. Настроить UDP перед обменом пакетами

**Задача.** Отправить датаграмму и получить адрес отправителя: UdpSocket, UdpConnection и Datagram сохраняют границу пакета.

**Какие библиотеки использовать.** [`net/udp`](../std/net/udp.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** UdpOptions задаёт параметры сокета, а UdpBinding связывает локальный endpoint с владельцем датаграмм.

```okkama
module app v1.0.0;
import "net/udp" take UdpOptions;
import "io/stdout" take Stdout;

main() is int {
    let options=UdpOptions(address_reuse:true,broadcast:false,dual_stack:false);
    Stdout().writeln(if options.address_reuse() {"reuse requested";} else {"reuse disabled";});
    Stdout().writeln(if options.broadcast() {"broadcast requested";} else {"broadcast disabled";});
    Stdout().writeln(if options.dual_stack() {"dual stack requested";} else {"dual stack disabled";});
    0;
}
```

[Исходный пример](../examples/std-v4/udp-options.ok).

<a id="chapter-http-values"></a>

## 20. HTTP: сообщения, поля и представление данных

Request и Response связывают метаданные с BodyOpening. Заголовки, media type, даты, validators и предпочтения клиента имеют свои типизированные представления; это позволяет проверять данные до сетевого эффекта.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`http/body`](../std/http/body.ok) | Передать байтовое или одноразовое тело: BodyOpening и BodyReading делают время потребления явным. | `BodyOpening`, `BodyReading`, `ByteBody`, `OneShotBody`, `StreamBodyReading` | [Рецепт](#example-http-body) |
| [`http/conditional`](../std/http/conditional.ok) | Проверить If-None-Match и диапазон байтов: EntityTag, условия и range-представления работают над метаданными. | `EntityTag`, `ConditionalLimits`, `EntityTags`, `ParsedEntityTags`, `EntityTagSnapshot` | [Рецепт](#example-http-byte-ranges) |
| [`http/content_coding`](../std/http/content_coding.ok) | Обозначить Content-Encoding: ContentCoding представляет каноническое имя преобразования содержимого. | `ContentCoding` | [Рецепт](#example-http-content-coding) |
| [`http/date`](../std/http/date.ok) | Разобрать Date или Last-Modified: HttpDate и ParsedHttpDate используют представление HTTP-времени. | `HttpDate`, `ParsedHttpDate`, `HttpDateLocal`, `HttpDateFields`, `HttpDateClock` | [Рецепт](#example-http-cookie-date) |
| [`http/form`](../std/http/form.ok) | Принять загрузку multipart-формы: потоковый разбор отделяет текущую часть и контролирует бюджеты формы. | `FormLimits`, `MultipartFormEnded`, `MultipartFormClosed`, `MultipartPartUnread`, `FormPart` | [Рецепт](#example-http-form) |
| [`http/header`](../std/http/header.ok) | Передать заголовки запроса: HeaderName, Header и Headers сохраняют имена, значения и множественность полей. | `HeaderName`, `Header`, `HttpFieldValue`, `HttpFieldScan`, `HttpFieldRange` | [Рецепт](#recipe-https-request) |
| [`http/media_type`](../std/http/media_type.ok) | Прочитать Content-Type API: HttpMediaType сохраняет параметры по HTTP-грамматике. | `HttpMediaParameter`, `HttpMediaParameters`, `HttpMediaSnapshot`, `HttpMediaType`, `HttpMediaLimits` | [Рецепт](#example-http-media-type) |
| [`http/message`](../std/http/message.ok) | Собрать запрос и ответ: Request и Response удерживают метод, адресат, заголовки и открываемое тело. | `Request`, `Response`, `RequestCoordinates` | [Рецепт](#recipe-https-request) |
| [`http/method`](../std/http/method.ok) | Задать смысл операции API: Method хранит точное имя метода, MethodSemantics даёт протокольные свойства. | `Method`, `MethodSemantics` | [Рецепт](#recipe-https-request) |
| [`http/negotiation`](../std/http/negotiation.ok) | Выбрать язык и тип ответа по Accept: качества и Accepted-представления сохраняют точные предпочтения клиента. | `Quality`, `ParsedQuality`, `WeightedValue`, `NegotiationLimits`, `AcceptedContentCodings` | [Рецепт](#example-http-accepted-charsets) |
| [`http/status`](../std/http/status.ok) | Классифицировать ответ сервера: Status хранит код и предоставляет класс ответа. | `StatusClass`, `Status` | [Рецепт](#example-http-status) |
| [`http/target`](../std/http/target.ok) | Сформировать request-target: OriginTarget, AbsoluteTarget, AuthorityTarget и AsteriskTarget разделяют формы адресации. | `RequestTargetKind`, `RequestTarget`, `OriginTarget`, `AbsoluteTarget`, `AuthorityTarget` | [Рецепт](#recipe-https-request) |
| [`http/version`](../std/http/version.ok) | Выбрать HTTP/1.1 или HTTP/2: HttpVersion и HTTP/1-текстовые представления задают версию сообщения. | `HttpVersion`, `Http1VersionText`, `ParsedHttp1Version` | [Рецепт](#recipe-https-request) |
| [`http/xmlrpc`](../std/http/xmlrpc.ok) | Обменяться данными со старым XML-RPC сервисом: типизированные значения, вызовы и результаты сохраняют wire-представление. | `XmlRpcKind`, `XmlRpcResultKind`, `XmlRpcHttpFailed`, `XmlRpcValue`, `XmlRpcWriting` | [Рецепт](#example-http-xmlrpc) |

<a id="example-http-body"></a>

### 20.1. Передать небольшое либо одноразовое HTTP-тело

**Задача.** Передать байтовое или одноразовое тело: BodyOpening и BodyReading делают время потребления явным.

**Какие библиотеки использовать.** [`http/body`](../std/http/body.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ByteBody открывает чтение байтового снимка; OneShotBody передаёт уже выделенное потребление. Владейте каждым BodyReading в области обработки.

```okkama
module app v1.0.0;
import "http/body" take ByteBody;
import "encoding/utf" take Utf8Encoded, Utf8Decoded;
import "io/stdout" take Stdout;
async main() is int {
    let body=ByteBody(Utf8Encoded("hello").bytes());let reader=body.reading();defer reader.close();
    Stdout().writeln("length: "+body.length().string());
    loop {let chunk=await reader.bytes(2);last if chunk.size()==0;Stdout().writeln(Utf8Decoded(chunk).string());}
    Stdout().writeln("trailers: "+reader.trailers().entries().size().string());0;
}
```

[Исходный пример](../examples/std-v4/http-body.ok).

<a id="example-http-byte-ranges"></a>

### 20.2. Выбрать часть файла по Range

**Задача.** Проверить If-None-Match и диапазон байтов: EntityTag, условия и range-представления работают над метаданными.

**Какие библиотеки использовать.** [`http/conditional`](../std/http/conditional.ok), [`numbers/bigint`](../std/numbers/bigint.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Разберите byte-range представление и сопоставьте с размером содержимого. Результат определяет выбранный диапазон ответа.

```okkama
module app v1.0.0;
import "http/conditional" take ByteRanges;
import "numbers/bigint" take BigInt;
import "io/stdout" take Stdout;
main() is int {
    let plan=ByteRanges("bytes=500-700, 601-999, -100");
    Stdout().writeln(plan.string());
    let ranges=plan.ranges(BigInt("1000"));
    for ranges -> range {Stdout().writeln(range.string()+" ("+range.size().string()+" bytes)");}
    0;
}
```

[Исходный пример](../examples/std-v4/http-byte-ranges.ok).

<a id="example-http-content-coding"></a>

### 20.3. Представить Content-Encoding ответа

**Задача.** Обозначить Content-Encoding: ContentCoding представляет каноническое имя преобразования содержимого.

**Какие библиотеки использовать.** [`http/content_coding`](../std/http/content_coding.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ContentCoding проверяет и канонизирует имя преобразования. Выбранный codec связывается с потоковым чтением отдельным сотрудником.

```okkama
module app v1.0.0;
import "http/content_coding" take ContentCoding;
import "io/stdout" take Stdout;
main() is int {
    Stdout().writeln(ContentCoding("GZip").string());
    Stdout().writeln(ContentCoding("X-Custom").string());0;
}
```

[Исходный пример](../examples/std-v4/http-content-coding.ok).

<a id="example-http-form"></a>

### 20.4. Разобрать multipart-загрузку документов

**Задача.** Принять загрузку multipart-формы: потоковый разбор отделяет текущую часть и контролирует бюджеты формы.

**Какие библиотеки использовать.** [`http/form`](../std/http/form.ok), [`http/body`](../std/http/body.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Передайте boundary, лимиты и поток формы. Дочитывайте текущую часть перед переходом к следующей.

```okkama
module example v1.0.0;
import "http/form" take ParsedMultipartForm,FormLimits,FormText,MultipartFormEnded;
import "http/body" take ByteBody;
import "encoding/utf" take Utf8Encoded;
import "io/stdout" take Stdout;
async main() is int{
    let source=ByteBody(Utf8Encoded("--demo\r\nContent-Disposition: form-data; name=customer\r\nContent-Type: text/plain; charset=UTF-8\r\n\r\nБарак\r\n--demo--\r\n").bytes()).reading();
    let parts=ParsedMultipartForm(source,"demo",FormLimits()).parts();defer parts.close();
    let part=await parts.part();Stdout().writeln(part.name()+": "+await FormText(part).text());
    try{await parts.part();}catch e is MultipartFormEnded{}0;
}
```

[Исходный пример](../examples/std-v4/http-form.ok).

<a id="example-http-media-type"></a>

### 20.5. Прочитать Content-Type с параметрами

**Задача.** Прочитать Content-Type API: HttpMediaType сохраняет параметры по HTTP-грамматике.

**Какие библиотеки использовать.** [`http/media_type`](../std/http/media_type.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** HttpMediaType сохраняет HTTP-параметры. Charset имеет собственную token-идентичность, остальные значения остаются точными.

```okkama
module app v1.0.0;
import "http/media_type" take HttpMediaType;
import "io/stdout" take Stdout;
main() is int {
    let type=HttpMediaType("Text/PLAIN; format=flowed; Charset=\"UTF-8\"");
    Stdout().writeln(type.string());Stdout().writeln(type.parameters().first("charset"));
    0;
}
```

[Исходный пример](../examples/std-v4/http-media-type.ok).

<a id="example-http-accepted-charsets"></a>

### 20.6. Выбрать кодировку по Accept-Charset

**Задача.** Выбрать язык и тип ответа по Accept: качества и Accepted-представления сохраняют точные предпочтения клиента.

**Какие библиотеки использовать.** [`http/negotiation`](../std/http/negotiation.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Разберите веса и сопоставьте их с доступными кодировками приложения. Quality хранит точные тысячные доли предпочтения.

```okkama
module app v1.0.0;
import "http/negotiation" take AcceptedCharsets, CharsetName;
import "io/stdout" take Stdout;
main() is int {
    let accepted=AcceptedCharsets("utf-8;q=0,*;q=0.5,us-ascii;q=0.8");
    Stdout().writeln(accepted.string());
    Stdout().writeln(accepted.preference([CharsetName("UTF-8"),CharsetName("US-ASCII")]).string());
    0;
}
```

[Исходный пример](../examples/std-v4/http-accepted-charsets.ok).

<a id="example-http-status"></a>

### 20.7. Выбрать обработчик по классу HTTP-ответа

**Задача.** Классифицировать ответ сервера: Status хранит код и предоставляет класс ответа.

**Какие библиотеки использовать.** [`http/status`](../std/http/status.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Status удерживает числовой код. Используйте его класс и публичные наблюдения при выборе поведения клиента.

```okkama
module app v1.0.0;
import "http/status" take Status, StatusClass;
import "io/stdout" take Stdout;
main() is int {
    let status=Status(422);Stdout().writeln(status.code().string()+" "+status.phrase());
    let extension=Status(599);
    Stdout().writeln("extension class is server_error: "+(extension.class()==StatusClass.server_error).string());
    0;
}
```

[Исходный пример](../examples/std-v4/http-status.ok).

<a id="example-http-xmlrpc"></a>

### 20.8. Вызвать XML-RPC сервис учётной системы

**Задача.** Обменяться данными со старым XML-RPC сервисом: типизированные значения, вызовы и результаты сохраняют wire-представление.

**Какие библиотеки использовать.** [`http/xmlrpc`](../std/http/xmlrpc.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Соберите типизированные значения и вызов. Кодек сохраняет параметры и различает успешный результат с fault-представлением.

```okkama
module example v1.0.0;
import "http/xmlrpc" take XmlRpcCall,XmlRpcInteger,XmlRpcText,XmlRpcCallWire,ParsedXmlRpcCall,XmlRpcLimits;
import "io/stdout" take Stdout;
main() is int{
    let limits=XmlRpcLimits();let call=XmlRpcCall("customer.save",[XmlRpcInteger(7),XmlRpcText("Ada")]);
    let parsed=ParsedXmlRpcCall(XmlRpcCallWire(call,limits).bytes(),limits).call();
    Stdout().writeln(parsed.method()+" "+parsed.parameters().size().string());0;
}
```

[Исходный пример](../examples/std-v4/http-xmlrpc.ok).

<a id="chapter-http-client"></a>

## 21. HTTP-клиент: соединения, авторизация, proxy, cookies и кеш

Соберите базовый Exchanging из resolver, TLS, часов и политик. Затем добавьте необходимые декораторы: перенаправления, авторизацию, cookies или кеш. Прочитанное до корректного EOF тело определяет завершение обмена и возможность повторного использования ресурса.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`http/auth`](../std/http/auth.ok) | Добавить Basic или Bearer на выбранный scope: scoped-декораторы связывают credentials с разрешённым адресатом. | `BasicCredentials`, `HttpAuthorizing`, `BasicAuthorization`, `BearerAuthorization`, `BasicExchanging` | [Рецепт](#example-http-proxy) |
| [`http/auth_challenge`](../std/http/auth_challenge.ok) | Разобрать WWW-Authenticate: AuthenticationChallenges сохраняет параметры и границы отдельных challenge. | `AuthenticationParameter`, `AuthenticationChallenge`, `AuthenticationChallenges`, `AuthenticationChallengeScanning` | [Рецепт](#example-http-digest-https) |
| [`http/cache`](../std/http/cache.ok) | Подключить кеш к готовому клиенту: Caching публикует ответ после корректного завершения чтения тела. | `Caching`, `CacheRequestSelection`, `CacheResponseSelection`, `CacheBodyReading`, `CacheReaderAnchor` | [Рецепт](#example-http-cache) |
| [`http/cache_admission`](../std/http/cache_admission.ok) | Согласовать параллельные обращения к кешу: CacheAdmissions выдаёт короткоживущую аренду admission. | `CacheAdmissionKey`, `CacheAdmissions`, `CacheAdmissionSlot`, `CacheAdmissionBooking`, `CacheAdmissionLease` | [Рецепт](#example-http-cache) |
| [`http/cache_control`](../std/http/cache_control.ok) | Прочитать директивы Cache-Control: CacheControl сохраняет параметры, порядок и повторные директивы. | `CacheDirective`, `CacheControl`, `CacheDeltaSeconds`, `CacheControlFields`, `CacheControlScanning` | [Рецепт](#example-http-cache) |
| [`http/cache_freshness`](../std/http/cache_freshness.ok) | Рассчитать свежесть ответа: CacheFreshness разделяет HTTP-даты и монотонное прошедшее время. | `CacheResponseTiming`, `CacheFreshness`, `CacheFreshnessMetadata`, `CacheFreshnessObservations` | [Рецепт](#example-http-cache) |
| [`http/cache_memory`](../std/http/cache_memory.ok) | Удерживать завершённые варианты ответов: MemoryHttpCache ограничивает объём сохранённых байтов и учитывает Vary. | `ResponseCaching`, `CachedHttpEntry`, `MemoryHttpCache`, `CacheStorageSelection`, `CacheStoragePosition` | [Рецепт](#example-http-cache) |
| [`http/cache_validation`](../std/http/cache_validation.ok) | Обновить сохранённый ответ после 304: CacheValidation проверяет validator и объединяет допустимые поля. | `CacheValidation`, `Cache304Fields`, `CacheMergedFields`, `CacheFieldState`, `CacheRequestFreshness` | [Рецепт](#example-http-cache) |
| [`http/client`](../std/http/client.ok) | Переиспользовать соединения к API: HttpClient реализует Exchanging с явными resolver, TLS и политикой. | `Exchanging`, `HttpClient`, `HttpClientKey`, `HttpClientIdle`, `HttpClientState` | [Рецепт](#recipe-https-request) |
| [`http/client_policy`](../std/http/client_policy.ok) | Настроить бюджеты HTTP-клиента: ClientPolicy, HttpLimits и ClientTimeouts объединяют ограничения вызова. | `HttpLimits`, `ClientTimeouts`, `ClientPolicy`, `Http1ClientProtocols` | [Рецепт](#recipe-https-request) |
| [`http/client_protocols`](../std/http/client_protocols.ok) | Согласовать разрешённые транспорты клиента: HttpClientProtocols связывает версии HTTP с политикой TLS. | `HttpClientProtocols` | [Рецепт](#recipe-proxy-environment) |
| [`http/client_request`](../std/http/client_request.ok) | Подготовить прямой запрос к отправке: HttpDirectRequest проверяет координаты запроса перед DNS и соединением. | `HttpDirectRequest` | [Рецепт](#recipe-proxy-environment) |
| [`http/connection`](../std/http/connection.ok) | Открыть HTTP/1-разговор на проверенном маршруте: Http1Connecting связывает resolver, policy и владение транспортом. | `Http1Connecting`, `Http1Acquisition`, `Http1AcquisitionDisposal`, `HttpAcquisitionCleanup`, `HttpTcpDisposal` | [Рецепт](#example-http1-connecting) |
| [`http/conversation`](../std/http/conversation.ok) | Подставить протокольный разговор в клиент: HttpConversation и Connecting задают обмен и время жизни приобретённого транспорта. | `HttpConversation`, `HttpConversationConnecting`, `HttpConversationRouting`, `HttpConversationAcquisition`, `HttpConversationExchange` | [Рецепт](#recipe-proxy-environment) |
| [`http/cookie`](../std/http/cookie.ok) | Представить Cookie и Set-Cookie: типы cookie сохраняют атрибуты и отдельное абсолютное либо сессионное время жизни. | `CookieExpiry`, `CookieFlag`, `SameSite`, `CookieAttributes`, `Cookie` | [Рецепт](#example-http-cookie-values) |
| [`http/cookie_client`](../std/http/cookie_client.ok) | Подключить cookie к Exchanging: CookieExchanging выбирает cookies для каждого конкретного HTTP-hop. | `CookieContextualizing`, `FirstPartyCookies`, `CallerCookiePolicy`, `CookieExchanging`, `CookieRequestPairs` | [Рецепт](#example-http-cookie-client) |
| [`http/cookie_date`](../std/http/cookie_date.ok) | Разобрать Expires из Set-Cookie: ParsedCookieDate применяет специальную грамматику cookie-date. | `ParsedCookieDate`, `CookieDateReading`, `CookieDateMonth`, `CookieDateDecimal` | [Рецепт](#example-http-cookie-date) |
| [`http/cookie_jar`](../std/http/cookie_jar.ok) | Сохранить cookie между запросами: MemoryCookieJar применяет правила выбора и сроков к явно переданным наблюдениям. | `CookieStoring`, `CookieLimits`, `MemoryCookieJar`, `CookieJarState`, `CookieRecord` | [Рецепт](#example-http-cookie-jar) |
| [`http/cookie_policy`](../std/http/cookie_policy.ok) | Решить, подходит ли cookie текущему запросу: CookieContext и политики учитывают origin, path и site. | `CookieContext`, `CookieOrigin`, `CookieDefaultPath`, `CookiePathMatch`, `CookieDomainMatch` | [Рецепт](#example-http-cookie-jar) |
| [`http/digest`](../std/http/digest.ok) | Ответить на Digest challenge партнёра: DigestAuthorization и связанные credentials реализуют явный профиль алгоритма. | `DigestCredentials`, `DigestChallenge`, `DigestFlag`, `DigestAuthorization`, `DigestAscii` | [Рецепт](#example-http-digest-https) |
| [`http/endpoint`](../std/http/endpoint.ok) | Разрешить только нужные адреса партнёра: PublicEndpoints, ListedNetworks и выбор endpoint проверяют числовые кандидаты. | `EndpointAllowing`, `AllEndpoints`, `PublicEndpoints`, `ListedNetworks`, `HttpEndpointSelection` | [Рецепт](#recipe-https-request) |
| [`http/environment_proxy`](../std/http/environment_proxy.ok) | Прочитать HTTP_PROXY и NO_PROXY из заданного Environment: EnvironmentProxy интерпретирует снимок настроек. | `EnvironmentProxy`, `ProxyEnvironmentValue` | [Рецепт](#recipe-proxy-environment) |
| [`http/exchange`](../std/http/exchange.ok) | Прочитать ответ вместе со сведениями о соединении: ClientResponse, ExchangeDetails и ExchangeHop сохраняют результат вызова. | `ExchangeDetails`, `PlainHttpHasNoTls`, `ExchangeHop`, `ClientResponse` | [Рецепт](#example-http-digest-https) |
| [`http/proxy`](../std/http/proxy.ok) | Отправить запрос через proxy: Proxied собирает клиент с явно заданной ProxyPolicy. | `Proxied` | [Рецепт](#example-http-proxy) |
| [`http/proxy_connection`](../std/http/proxy_connection.ok) | Открыть proxy-маршрут для клиента: HttpProxyRouting и HttpProxyConnecting управляют выбранным транспортом. | `HttpProxyRouting`, `HttpProxyConnecting`, `HttpProxySettlement` | [Рецепт](#example-http-proxy) |
| [`http/proxy_policy`](../std/http/proxy_policy.ok) | Настроить корпоративный HTTP proxy: ProxyPolicy удерживает адрес и выбранную авторизацию proxy. | `ProxyPolicy`, `HttpProxyEndpoint`, `ProxyAuthorizationConflict`, `ProxyConnectRejected` | [Рецепт](#example-http-proxy) |
| [`http/proxy_request`](../std/http/proxy_request.ok) | Сформировать forward-запрос или CONNECT: HttpForwardRequest и HttpConnectRequest разделяют формы обращения к proxy. | `HttpForwardRequest`, `HttpConnectRequest`, `ProxyRequestFields` | [Рецепт](#recipe-proxy-environment) |
| [`http/proxy_tunnel`](../std/http/proxy_tunnel.ok) | Перейти от CONNECT к туннелю: HttpProxyTunnel сохраняет точную границу ответа proxy и следующих байтов. | `HttpProxyTunnel`, `HttpProxySettlement` | [Рецепт](#example-http-proxy) |
| [`http/redirect`](../std/http/redirect.ok) | Пройти перенаправления по явной политике: Redirecting сохраняет историю и проверяет очередной адресат. | `RedirectPolicy`, `Redirecting`, `RedirectState`, `RedirectHistory`, `RedirectDestination` | [Рецепт](#example-http-redirect) |
| [`http/retry`](../std/http/retry.ok) | Повторить допустимый вызов после stale-соединения: Retrying использует подтверждённое состояние транспортного отказа. | `Retrying`, `HttpRetrySettlement` | [Рецепт](#example-http-retry) |
| [`http/robots`](../std/http/robots.ok) | Соблюдать правила сайта при сборе каталога: представления robots выбирают правила для user-agent и пути. | `RobotsFetchFailed`, `RobotsTrim`, `RobotsRecord`, `RobotsPolicy`, `RobotsAgent` | [Рецепт](#example-http-robots) |
| [`http/selected_connection`](../std/http/selected_connection.ok) | Выбрать разговор по фактически согласованному протоколу: HttpSelectedConnecting возвращает подходящий HTTP transport. | `HttpSelectedConnecting`, `HttpConversationAdopting` | [Рецепт](#example-http-selected-connecting) |

### Что учитывать при сборке решения

- `http/cache_admission`: Admission связывает параллельные обращения; ключ кеша и проверка Vary остаются обязанностями сохранённого представления.
- `http/cache_freshness`: Для Date используются wall-clock отметки, для delay и residence — монотонное время.
- `http/client`: Дочитайте body до EOF либо явно закройте его. Это завершает время владения конкретным ответом.
- `http/cookie_client`: Для применения cookies на каждом перенаправлении собирайте Redirecting поверх CookieExchanging.
- `http/redirect`: Политика перенаправления принимает очередной URL; credentials и endpoint-права проверяются для нового адресата.
- `http/retry`: Повторная отправка связана с подтверждённым состоянием stale before-response и допустимостью конкретного запроса.

<a id="recipe-proxy-environment"></a>

### 21.1. Выбрать proxy из настроек запуска

**Задача.** Сервис получает proxy и список исключений в явно переданном снимке Environment.

**Какие библиотеки использовать.** [`http/environment_proxy`](../std/http/environment_proxy.ok), [`os/environment`](../std/os/environment.ok), [`url/value`](../std/url/value.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Используйте выбранную ProxyPolicy при создании Proxied. Нижний регистр http_proxy и no_proxy в этом примере задаёт всю конфигурацию независимо от окружения процесса.

```okkama
module cookbook/proxy_environment v1.0.0;
import "http/environment_proxy" take EnvironmentProxy;
import "os/environment" take Environment;
import "url/value" take ParsedUrl;
import "io/stdout" take Stdout;

main() is int {
    let environment = Environment(values: ["http_proxy": "http://proxy.example:3128", "no_proxy": "localhost"]);
    let policy = EnvironmentProxy(environment).proxy(ParsedUrl("http://api.example/orders").value());
    Stdout().writeln("Выбрана политика proxy");
    0;
}
```

<a id="recipe-https-request"></a>

### 21.2. Получить JSON по HTTPS

**Задача.** Приложение обращается к опубликованному API с системными доверенными корнями и явной политикой адресатов.

**Какие библиотеки использовать.** [`http/client`](../std/http/client.ok), [`http/client_policy`](../std/http/client_policy.ok), [`http/endpoint`](../std/http/endpoint.ok), [`http/version`](../std/http/version.ok), [`http/message`](../std/http/message.ok), [`http/method`](../std/http/method.ok), [`http/header`](../std/http/header.ok), [`http/target`](../std/http/target.ok), [`net/dns`](../std/net/dns.ok), [`net/tls`](../std/net/tls.ok), [`crypto/x509/trust`](../std/crypto/x509/trust.ok), [`collections/set`](../std/collections/set.ok), [`time/clock`](../std/time/clock.ok), [`url/value`](../std/url/value.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Подставьте адрес вашего API. HttpClient получает resolver, trust policy, endpoint policy и часы отдельно. Тело читается до EOF; его владелец закрывается через defer. Для JSON накапливайте байты с ограничением и передайте декодированный текст в ParsedJson.

```okkama
module cookbook/https_request v1.0.0;
import "http/client" take HttpClient;
import "http/client_policy" take ClientPolicy,HttpLimits,ClientTimeouts;
import "http/endpoint" take PublicEndpoints;
import "http/version" take HttpVersion;
import "http/message" take Request;
import "http/method" take Method;
import "http/header" take Headers;
import "http/target" take AbsoluteTarget;
import "net/dns" take SystemResolution;
import "net/tls" take TlsClientPolicy;
import "crypto/x509/trust" take SystemTrustStore;
import "collections/set" take Set,OrderedSet;
import "time/clock" take SystemClock;
import "url/value" take ParsedUrl;
import "io/stdout" take Stdout;

async main() is int {
    let versions is Set<HttpVersion> = OrderedSet([HttpVersion.http_1_1]);
    let policy = ClientPolicy(versions, HttpLimits(), ClientTimeouts(), 2, PublicEndpoints());
    let client = HttpClient(SystemResolution(), TlsClientPolicy(SystemTrustStore().result()), policy, SystemClock());
    defer client.close();
    let request = Request(Method("GET"), AbsoluteTarget(ParsedUrl("https://example.org/").value()), Headers([]));
    let result = await client.response(request);
    Stdout().writeln("HTTP " + result.response().status().code().string());
    let body = result.response().body().reading();
    defer body.close();
    loop {
        let chunk = await body.bytes(4096);
        last if chunk.size() == 0;
    }
    0;
}
```

<a id="example-http-proxy"></a>

### 21.3. Обратиться к API через HTTP proxy

**Задача.** Отправить запрос через proxy: Proxied собирает клиент с явно заданной ProxyPolicy.

**Какие библиотеки использовать.** [`http/proxy`](../std/http/proxy.ok), [`http/proxy_policy`](../std/http/proxy_policy.ok), [`http/auth`](../std/http/auth.ok), [`crypto/secret`](../std/crypto/secret.ok), [`encoding/single_byte`](../std/encoding/single_byte.ok), [`os/environment`](../std/os/environment.ok), [`net/dns`](../std/net/dns.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/ip`](../std/net/ip.ok), [`net/tls`](../std/net/tls.ok), [`crypto/x509/trust`](../std/crypto/x509/trust.ok), [`collections/set`](../std/collections/set.ok), [`http/client_policy`](../std/http/client_policy.ok), [`http/endpoint`](../std/http/endpoint.ok), [`time/clock`](../std/time/clock.ok), [`http/version`](../std/http/version.ok), [`http/message`](../std/http/message.ok), [`http/method`](../std/http/method.ok), [`http/header`](../std/http/header.ok), [`http/target`](../std/http/target.ok), [`url/value`](../std/url/value.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ProxyPolicy связывает proxy-адрес и credentials. Proxied получает её вместе с resolver, TLS и обычной client policy.

**Входы примера.** Передайте через окружение `OKKAMA_PROXY_CASE`, `OKKAMA_PROXY_URL`. Адреса и файлы задают подготовленный стенд; проверяемые ответы видны в `confirm`.

```okkama
module app v1.0.0;
import "http/proxy" take Proxied;
import "http/proxy_policy" take ProxyPolicy,ProxyAuthorizationConflict;
import "http/auth" take BasicCredentials;
import "crypto/secret" take SecretBytes;
import "encoding/single_byte" take AsciiEncoded;
import "os/environment" take Environment;
import "net/dns" take FixedResolution,HostAddresses;
import "net/endpoint" take HostName;
import "net/ip" take IpAddress,IpNetwork;
import "net/tls" take TlsClientPolicy;
import "crypto/x509/trust" take TrustedCertificates;
import "collections/set" take Set,OrderedSet;
import "http/client_policy" take ClientPolicy,HttpLimits,ClientTimeouts;
import "http/endpoint" take ListedNetworks,EndpointDenied;
import "time/clock" take SystemClock;
import "http/version" take HttpVersion;
import "http/message" take Request;
import "http/method" take Method;
import "http/header" take Headers,Header,HeaderName;
import "http/target" take AbsoluteTarget;
import "url/value" take ParsedUrl;
import "io/stdout" take Stdout;
async main() is int {
    let environment=Environment();let scenario=environment.variable("OKKAMA_PROXY_CASE");let url=ParsedUrl(environment.variable("OKKAMA_PROXY_URL")).value();
    let password=SecretBytes(AsciiEncoded("proxy-pass").bytes());defer password.close();
    let proxy=if scenario=="auth"{ProxyPolicy(url,BasicCredentials("proxy-user",password));}else{ProxyPolicy(url);};
    let versions is Set<HttpVersion>=OrderedSet([HttpVersion.http_1_1]);let ip=if scenario=="denied"{IpAddress("192.0.2.0");}else{IpAddress("127.0.0.0");};let width=if scenario=="denied"{24;}else{8;};
    let policy=ClientPolicy(versions,HttpLimits(),ClientTimeouts(),1,ListedNetworks([IpNetwork(ip,width)]));
    let resolver=FixedResolution([HostAddresses(HostName("proxy.test"),[IpAddress("127.0.0.1")])]);
    let client=Proxied(proxy,resolver,TlsClientPolicy(TrustedCertificates([])),policy,SystemClock());defer client.close();
    let headers=if scenario=="conflict"{Headers([Header(HeaderName("Proxy-Authorization"),"caller")]);}else{Headers([Header(HeaderName("Authorization"),"Bearer origin")]);};
    let request=Request(Method("GET"),AbsoluteTarget(ParsedUrl("http://origin.test/x?number=1").value()),headers);
    if scenario=="denied" || scenario=="conflict"{
        let rejected=try{await client.response(request);false;}catch e is EndpointDenied{scenario=="denied";}catch e is ProxyAuthorizationConflict{scenario=="conflict";};confirm rejected;
    }else{
        let first=await client.response(request);confirm first.response().status().code()==204;
        let result=await client.response(request);confirm result.response().status().code()==200 && result.details().endpoints().remote_endpoint().port()==url.parts().authority().port();
        confirm !result.request().headers().membership(HeaderName("Proxy-Authorization"));let reader=result.response().body().reading();defer reader.close();confirm await reader.bytes(8)==[66.byte()];confirm (await reader.bytes(8)).size()==0;
    }
    Stdout().writeln("proxy case "+scenario+" passed");0;
}
```

[Исходный пример](../examples/std-v4/http-proxy.ok).

<a id="example-http-cache"></a>

### 21.4. Повторно использовать завершённый ответ API

**Задача.** Подключить кеш к готовому клиенту: Caching публикует ответ после корректного завершения чтения тела.

**Какие библиотеки использовать.** [`http/cache`](../std/http/cache.ok), [`http/cache_memory`](../std/http/cache_memory.ok), [`http/client`](../std/http/client.ok), [`http/client_policy`](../std/http/client_policy.ok), [`http/endpoint`](../std/http/endpoint.ok), [`net/ip`](../std/net/ip.ok), [`net/dns`](../std/net/dns.ok), [`net/tls`](../std/net/tls.ok), [`crypto/x509/trust`](../std/crypto/x509/trust.ok), [`collections/set`](../std/collections/set.ok), [`http/version`](../std/http/version.ok), [`http/message`](../std/http/message.ok), [`http/method`](../std/http/method.ok), [`http/header`](../std/http/header.ok), [`http/target`](../std/http/target.ok), [`url/value`](../std/url/value.ok), [`os/environment`](../std/os/environment.ok), [`time/clock`](../std/time/clock.ok), [`numbers/bigint`](../std/numbers/bigint.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** MemoryHttpCache получает clock и бюджет. Caching оборачивает Exchanging и публикует сохранённое тело после корректного EOF.

**Входы примера.** Передайте через окружение `OKKAMA_CACHE_URL`. Адреса и файлы задают подготовленный стенд; проверяемые ответы видны в `confirm`.

```okkama
module app v1.0.0;
import "http/cache" take Caching;
import "http/cache_memory" take MemoryHttpCache;
import "http/client" take HttpClient;
import "http/client_policy" take ClientPolicy,HttpLimits,ClientTimeouts;
import "http/endpoint" take ListedNetworks;
import "net/ip" take IpAddress,IpNetwork;
import "net/dns" take FixedResolution;
import "net/tls" take TlsClientPolicy;
import "crypto/x509/trust" take TrustedCertificates;
import "collections/set" take Set,OrderedSet;
import "http/version" take HttpVersion;
import "http/message" take Request;
import "http/method" take Method;
import "http/header" take Headers,HeaderName;
import "http/target" take AbsoluteTarget;
import "url/value" take ParsedUrl;
import "os/environment" take Environment;
import "time/clock" take SystemClock;
import "numbers/bigint" take BigInt;
import "io/stdout" take Stdout;
async main() is int{
    let clock=SystemClock();let cache=MemoryHttpCache(clock,BigInt("8192"));defer cache.close();
    let versions is Set<HttpVersion>=OrderedSet([HttpVersion.http_1_1]);let policy=ClientPolicy(versions,HttpLimits(),ClientTimeouts(),1,ListedNetworks([IpNetwork(IpAddress("127.0.0.0"),8)]));
    let direct=HttpClient(FixedResolution([]),TlsClientPolicy(TrustedCertificates([])),policy,clock);defer direct.close();let client=Caching(direct,cache,clock);
    let request=Request(Method("GET"),AbsoluteTarget(ParsedUrl(Environment().variable("OKKAMA_CACHE_URL")).value()),Headers([]));
    for (0..2) -> index{let result=await client.response(request);confirm result.response().status().code()==200;let body=result.response().body().reading();defer body.close();confirm await body.bytes(1)==[65.byte()];confirm (await body.bytes(1)).size()==0;if index==1{confirm result.response().headers().membership(HeaderName("Age"));}}
    confirm cache.size()==1;Stdout().writeln("HTTP cache passed");0;
}
```

[Исходный пример](../examples/std-v4/http-cache.ok).

<a id="example-http-cookie-values"></a>

### 21.5. Представить атрибуты cookie

**Задача.** Представить Cookie и Set-Cookie: типы cookie сохраняют атрибуты и отдельное абсолютное либо сессионное время жизни.

**Какие библиотеки использовать.** [`http/cookie`](../std/http/cookie.ok), [`collections/set`](../std/collections/set.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Cookie-представления сохраняют имя, значение, expiry и policy-атрибуты. Используйте отдельные session и absolute expiry-наблюдения.

```okkama
module app v1.0.0;
import "http/cookie" take Cookie, CookieAttributes, CookieExpiry, CookieFlag, SameSite;
import "collections/set" take OrderedSet;
import "io/stdout" take Stdout;
main() is int {
    let attributes=CookieAttributes("","/",CookieExpiry(),SameSite.lax,OrderedSet([CookieFlag.secure,CookieFlag.http_only]));
    let cookie=Cookie("__Host-session","opaque-token",attributes);
    Stdout().writeln(cookie.diagnostic());
    confirm cookie.string()=="__Host-session=opaque-token; Path=/; SameSite=Lax; Secure; HttpOnly";
    0;
}
```

[Исходный пример](../examples/std-v4/http-cookie-values.ok).

<a id="example-http-cookie-client"></a>

### 21.6. Добавить cookie jar к клиенту

**Задача.** Переиспользовать соединения к API: HttpClient реализует Exchanging с явными resolver, TLS и политикой.

**Какие библиотеки использовать.** [`http/client`](../std/http/client.ok), [`http/redirect`](../std/http/redirect.ok), [`http/cookie_client`](../std/http/cookie_client.ok), [`http/cookie_jar`](../std/http/cookie_jar.ok), [`http/client_policy`](../std/http/client_policy.ok), [`http/message`](../std/http/message.ok), [`http/target`](../std/http/target.ok), [`http/method`](../std/http/method.ok), [`http/status`](../std/http/status.ok), [`http/header`](../std/http/header.ok), [`http/http1_reading`](../std/http/http1_reading.ok), [`http/http1_message_writing`](../std/http/http1_message_writing.ok), [`http/endpoint`](../std/http/endpoint.ok), [`http/version`](../std/http/version.ok), [`url/value`](../std/url/value.ok), [`net/tcp`](../std/net/tcp.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/ip`](../std/net/ip.ok), [`net/dns`](../std/net/dns.ok), [`net/tls`](../std/net/tls.ok), [`crypto/x509/trust`](../std/crypto/x509/trust.ok), [`collections/set`](../std/collections/set.ok), [`time/clock`](../std/time/clock.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** CookieExchanging связывает jar, clock и контекст выбора. Для каждого HTTP-hop выполняется собственный отбор cookies.

```okkama
module app v1.0.0;
import "http/client" take HttpClient;
import "http/redirect" take Redirecting,RedirectPolicy;
import "http/cookie_client" take CookieExchanging,FirstPartyCookies;
import "http/cookie_jar" take MemoryCookieJar;
import "http/client_policy" take ClientPolicy,HttpLimits,ClientTimeouts;
import "http/message" take Request,Response;
import "http/target" take AbsoluteTarget;
import "http/method" take Method;
import "http/status" take Status;
import "http/header" take Headers,Header,HeaderName;
import "http/http1_reading" take Http1RequestHeadReading;
import "http/http1_message_writing" take Http1ResponseWriting;
import "http/endpoint" take AllEndpoints;
import "http/version" take HttpVersion;
import "url/value" take ParsedUrl;
import "net/tcp" take TcpListening,TcpAcceptance;
import "net/endpoint" take IpEndpoint;
import "net/ip" take IpAddress;
import "net/dns" take FixedResolution;
import "net/tls" take TlsClientPolicy;
import "crypto/x509/trust" take TrustedCertificates;
import "collections/set" take Set,OrderedSet;
import "time/clock" take SystemClock;
import "io/stdout" take Stdout;
object Replying {
 has listener is TcpAcceptance;Replying(listener is TcpAcceptance){self.listener=listener;}
 async run() is bool{
  let connection=await listener.connection();defer connection.close();
  let first=await Http1RequestHeadReading(connection.input()).result();
  let response=Response(Status(303),Headers([Header(HeaderName("Location"),"/second"),Header(HeaderName("Content-Length"),"0"),Header(HeaderName("Set-Cookie"),"x=1; Path=/")]));
  await Http1ResponseWriting(connection.output(),response,first.head().method()).result();
  let second=await Http1RequestHeadReading(connection.input()).result();
  await Http1ResponseWriting(connection.output(),Response(Status(204),Headers([])),second.head().method()).result();
  first.head().target().string()=="/first?a=1" && second.head().target().string()=="/second" && first.head().method().string()=="POST" && second.head().method().string()=="GET" && second.head().headers().first(HeaderName("Cookie"))=="x=1";
 }
}

async main() is int{
  let listener=await TcpListening(IpEndpoint(IpAddress("127.0.0.1"),0)).acceptance();defer listener.close();
  let replying=Replying(listener);let reply is async () is bool=replying.run;let serving=spawn reply();
  let versions is Set<HttpVersion>=OrderedSet([HttpVersion.http_1_1]);
  let client=HttpClient(FixedResolution([]),TlsClientPolicy(TrustedCertificates([])),ClientPolicy(versions,HttpLimits(),ClientTimeouts(),1,AllEndpoints()),SystemClock());defer client.close();
  let root="http://127.0.0.1:"+listener.local_endpoint().port().string();
  let request=Request(Method("POST"),AbsoluteTarget(ParsedUrl(root+"/first?a=1").value()),Headers([]));
  let jar=MemoryCookieJar(SystemClock());defer jar.close();
  let result=await Redirecting(CookieExchanging(client,jar,FirstPartyCookies()),RedirectPolicy()).response(request);client.close();
  let reader=result.response().body().reading();
  confirm result.request().target().string()==root+"/second" && result.response().status().code()==204;
  confirm result.history().size()==1 && result.history().item(0).response().status().code()==303;
  confirm (await reader.bytes(1)).size()==0;confirm await serving;
  Stdout().writeln("redirect selected Cookie x=1 on the next request");0;
}
```

[Исходный пример](../examples/std-v4/http-cookie-client.ok).

<a id="example-http-cookie-date"></a>

### 21.7. Разобрать Expires в ответе сервера

**Задача.** Разобрать Expires из Set-Cookie: ParsedCookieDate применяет специальную грамматику cookie-date.

**Какие библиотеки использовать.** [`http/cookie_date`](../std/http/cookie_date.ok), [`http/date`](../std/http/date.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ParsedCookieDate применяет token-грамматику cookie-date. Дата становится абсолютным наблюдением, пригодным для expiry-политики.

```okkama
module app v1.0.0;
import "http/cookie_date" take ParsedCookieDate;
import "http/date" take HttpDate;
import "io/stdout" take Stdout;
main() is int {
    let expiry=ParsedCookieDate("Wednesday, 09-Jun-21 10:18:14 GMT").value();
    Stdout().writeln(HttpDate(expiry).string());
    confirm HttpDate(expiry).string()=="Wed, 09 Jun 2021 10:18:14 GMT";
    0;
}
```

[Исходный пример](../examples/std-v4/http-cookie-date.ok).

<a id="example-http-cookie-jar"></a>

### 21.8. Выбирать cookie по адресу и времени

**Задача.** Сохранить cookie между запросами: MemoryCookieJar применяет правила выбора и сроков к явно переданным наблюдениям.

**Какие библиотеки использовать.** [`http/cookie_jar`](../std/http/cookie_jar.ok), [`http/cookie_policy`](../std/http/cookie_policy.ok), [`http/header`](../std/http/header.ok), [`http/method`](../std/http/method.ok), [`url/value`](../std/url/value.ok), [`time/clock`](../std/time/clock.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** MemoryCookieJar получает Set-Cookie-наблюдение, а policy проверяет domain/path/site. Отправка использует явный CookieContext.

```okkama
module app v1.0.0;
import "http/cookie_jar" take MemoryCookieJar;
import "http/cookie_policy" take CookieContext;
import "http/header" take Header, HeaderName, Headers;
import "http/method" take Method;
import "url/value" take ParsedUrl;
import "time/clock" take SystemClock;
import "io/stdout" take Stdout;
main() is int {
    let jar=MemoryCookieJar(SystemClock());defer jar.close();
    let origin=ParsedUrl("https://example.com/a/b").value();
    let context=CookieContext(origin,true,Method("GET"));
    jar.store(origin,Headers([Header(HeaderName("Set-Cookie"),"session=opaque; Secure; HttpOnly; SameSite=Lax; Path=/")]),context);
    let cookies=jar.cookies(origin,context);confirm cookies.size()==1;
    Stdout().writeln(cookies.item(0).diagnostic());
    0;
}
```

[Исходный пример](../examples/std-v4/http-cookie-jar.ok).

<a id="example-http-digest-https"></a>

### 21.9. Выполнить запрос с Digest challenge

**Задача.** Переиспользовать соединения к API: HttpClient реализует Exchanging с явными resolver, TLS и политикой.

**Какие библиотеки использовать.** [`os/environment`](../std/os/environment.ok), [`fs/path`](../std/fs/path.ok), [`fs/file`](../std/fs/file.ok), [`net/ip`](../std/net/ip.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/tls`](../std/net/tls.ok), [`crypto/x509/certificate`](../std/crypto/x509/certificate.ok), [`crypto/x509/trust`](../std/crypto/x509/trust.ok), [`collections/set`](../std/collections/set.ok), [`http/client`](../std/http/client.ok), [`http/digest`](../std/http/digest.ok), [`http/auth`](../std/http/auth.ok), [`http/exchange`](../std/http/exchange.ok), [`crypto/secret`](../std/crypto/secret.ok), [`encoding/single_byte`](../std/encoding/single_byte.ok), [`encoding/utf`](../std/encoding/utf.ok), [`random/source`](../std/random/source.ok), [`http/client_policy`](../std/http/client_policy.ok), [`time/clock`](../std/time/clock.ok), [`http/endpoint`](../std/http/endpoint.ok), [`url/value`](../std/url/value.ok), [`net/dns`](../std/net/dns.ok), [`http/body`](../std/http/body.ok), [`http/message`](../std/http/message.ok), [`http/method`](../std/http/method.ok), [`http/target`](../std/http/target.ok), [`http/header`](../std/http/header.ok), [`http/version`](../std/http/version.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Подготовьте credentials и область авторизации. Декоратор получает challenge от сервера и строит следующий запрос с явным Digest-профилем.

**Входы примера.** Передайте через окружение `OKKAMA_DIGEST_CASE`, `OKKAMA_HTTP_URL`, `OKKAMA_TLS_ROOT`. Адреса и файлы задают подготовленный стенд; проверяемые ответы видны в `confirm`.

```okkama
module app v1.0.0;
import "os/environment" take Environment;
import "fs/path" take Path;
import "fs/file" take File,FileMode;
import "net/ip" take IpAddress;
import "net/endpoint" take HostName;
import "net/tls" take TlsClientPolicy,TlsVersion;
import "crypto/x509/certificate" take Certificate;
import "crypto/x509/trust" take TrustedCertificates;
import "collections/set" take Set,OrderedSet;
import "http/client" take HttpClient;
import "http/digest" take DigestCredentials,DigestExchanging;
import "http/auth" take InvalidAuthentication;
import "http/exchange" take ClientResponse;
import "crypto/secret" take SecretBytes;
import "encoding/single_byte" take AsciiEncoded;
import "encoding/utf" take Utf8Encoded;
import "random/source" take SecureRandom;
import "http/client_policy" take ClientPolicy,HttpLimits,ClientTimeouts;
import "time/clock" take SystemClock;
import "http/endpoint" take ListedNetworks;
import "url/value" take ParsedUrl,Url,UrlParts,UrlFragment,UrlQuery;
import "net/dns" take FixedResolution,HostAddresses;
import "net/ip" take IpNetwork;
import "http/body" take EmptyBody;
import "http/message" take Request;
import "http/method" take Method;
import "http/target" take AbsoluteTarget;
import "http/header" take Headers,HeaderName;
import "http/version" take HttpVersion;
import "io/stdout" take Stdout;

async main() is int {
    let environment=Environment();
    let file=File(mode:FileMode.reading,path:Path(environment.variable("OKKAMA_TLS_ROOT")),binary:true);defer file.close();
    let roots=TrustedCertificates([Certificate(file.binary().bytes(1048577))]);
    let versions is Set<TlsVersion>=OrderedSet([TlsVersion.tls1_2,TlsVersion.tls1_3]);
    let policy=TlsClientPolicy(roots,versions,["http/1.1"]);
    let url=ParsedUrl(environment.variable("OKKAMA_HTTP_URL")).value();
    let resolver=FixedResolution([HostAddresses(HostName("example.test"),[IpAddress("127.0.0.1")]),HostAddresses(HostName("wrong.test"),[IpAddress("127.0.0.1")])]);
    let http_versions is Set<HttpVersion>=OrderedSet([HttpVersion.http_1_1]);
    let client_policy=ClientPolicy(http_versions,HttpLimits(),ClientTimeouts(),1,ListedNetworks([IpNetwork(IpAddress("127.0.0.0"),8)]));
    let client=HttpClient(resolver,policy,client_policy,SystemClock());defer client.close();
    let parts=url.parts();let target=Url(UrlParts(parts.scheme(),parts.authority(),parts.path(),parts.query(),UrlFragment()));
    let request=Request(Method("GET"),AbsoluteTarget(target),Headers([]),EmptyBody(),HttpVersion.http_1_1);
    let scenario=environment.variable("OKKAMA_DIGEST_CASE");let password=if scenario=="unicode"{Utf8Encoded("Secret, or not?").bytes();}else{AsciiEncoded("Circle of Life").bytes();};
    let username=if scenario=="unicode"{"Jäsøn Doe";}else{"Mufasa";};
    let secret=SecretBytes(password);defer secret.close();let credentials=DigestCredentials(username,secret);
    let scope=Url(UrlParts("https",parts.authority(),"/",UrlQuery(),UrlFragment()));
    let expected=if scenario=="wrong"{401;}else{200;};
    let outcomes is @ClientResponse=try{[await DigestExchanging(client,scope,credentials,SecureRandom()).response(request)];}catch e is InvalidAuthentication{confirm scenario=="badproof";let empty is @ClientResponse=[];empty;};
    if scenario=="badproof"{confirm outcomes.size()==0;Stdout().writeln("HTTPS Digest "+scenario+" passed");return 0;}
    confirm outcomes.size()==1;let result=outcomes[0];
    confirm result.response().status().code()==expected && result.details().secure();confirm !result.request().headers().membership(HeaderName("Authorization"));
    let reader=result.response().body().reading();defer reader.close();confirm await reader.bytes(8)==[66.byte()];confirm (await reader.bytes(8)).size()==0;
    Stdout().writeln("HTTPS Digest "+scenario+" passed");0;
}
```

[Исходный пример](../examples/std-v4/http-digest-https.ok).

<a id="example-http-redirect"></a>

### 21.10. Пройти цепочку перенаправлений API

**Задача.** Переиспользовать соединения к API: HttpClient реализует Exchanging с явными resolver, TLS и политикой.

**Какие библиотеки использовать.** [`http/client`](../std/http/client.ok), [`http/redirect`](../std/http/redirect.ok), [`http/client_policy`](../std/http/client_policy.ok), [`http/message`](../std/http/message.ok), [`http/target`](../std/http/target.ok), [`http/method`](../std/http/method.ok), [`http/status`](../std/http/status.ok), [`http/header`](../std/http/header.ok), [`http/http1_reading`](../std/http/http1_reading.ok), [`http/http1_message_writing`](../std/http/http1_message_writing.ok), [`http/endpoint`](../std/http/endpoint.ok), [`http/version`](../std/http/version.ok), [`url/value`](../std/url/value.ok), [`net/tcp`](../std/net/tcp.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/ip`](../std/net/ip.ok), [`net/dns`](../std/net/dns.ok), [`net/tls`](../std/net/tls.ok), [`crypto/x509/trust`](../std/crypto/x509/trust.ok), [`collections/set`](../std/collections/set.ok), [`time/clock`](../std/time/clock.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Redirecting получает базовый Exchanging и RedirectPolicy. Итог содержит историю hop и ответ выбранного адресата.

```okkama
module app v1.0.0;
import "http/client" take HttpClient;
import "http/redirect" take Redirecting,RedirectPolicy;
import "http/client_policy" take ClientPolicy,HttpLimits,ClientTimeouts;
import "http/message" take Request,Response;
import "http/target" take AbsoluteTarget;
import "http/method" take Method;
import "http/status" take Status;
import "http/header" take Headers,Header,HeaderName;
import "http/http1_reading" take Http1RequestHeadReading;
import "http/http1_message_writing" take Http1ResponseWriting;
import "http/endpoint" take AllEndpoints;
import "http/version" take HttpVersion;
import "url/value" take ParsedUrl;
import "net/tcp" take TcpListening,TcpAcceptance;
import "net/endpoint" take IpEndpoint;
import "net/ip" take IpAddress;
import "net/dns" take FixedResolution;
import "net/tls" take TlsClientPolicy;
import "crypto/x509/trust" take TrustedCertificates;
import "collections/set" take Set,OrderedSet;
import "time/clock" take SystemClock;
import "io/stdout" take Stdout;
object Replying {
 has listener is TcpAcceptance;Replying(listener is TcpAcceptance){self.listener=listener;}
 async run() is bool{
  let connection=await listener.connection();defer connection.close();
  let first=await Http1RequestHeadReading(connection.input()).result();
  let response=Response(Status(303),Headers([Header(HeaderName("Location"),"/second"),Header(HeaderName("Content-Length"),"0")]));
  await Http1ResponseWriting(connection.output(),response,first.head().method()).result();
  let second=await Http1RequestHeadReading(connection.input()).result();
  await Http1ResponseWriting(connection.output(),Response(Status(204),Headers([])),second.head().method()).result();
  first.head().target().string()=="/first?a=1" && second.head().target().string()=="/second" && first.head().method().string()=="POST" && second.head().method().string()=="GET";
 }
}

async main() is int{
  let listener=await TcpListening(IpEndpoint(IpAddress("127.0.0.1"),0)).acceptance();defer listener.close();
  let replying=Replying(listener);let reply is async () is bool=replying.run;let serving=spawn reply();
  let versions is Set<HttpVersion>=OrderedSet([HttpVersion.http_1_1]);
  let client=HttpClient(FixedResolution([]),TlsClientPolicy(TrustedCertificates([])),ClientPolicy(versions,HttpLimits(),ClientTimeouts(),1,AllEndpoints()),SystemClock());defer client.close();
  let root="http://127.0.0.1:"+listener.local_endpoint().port().string();
  let request=Request(Method("POST"),AbsoluteTarget(ParsedUrl(root+"/first?a=1").value()),Headers([]));
  let result=await Redirecting(client,RedirectPolicy()).response(request);client.close();
  let reader=result.response().body().reading();
  confirm result.request().target().string()==root+"/second" && result.response().status().code()==204;
  confirm result.history().size()==1 && result.history().item(0).response().status().code()==303;
  confirm (await reader.bytes(1)).size()==0;confirm await serving;
  Stdout().writeln("followed POST to GET with one redirect hop");0;
}
```

[Исходный пример](../examples/std-v4/http-redirect.ok).

<a id="example-http-retry"></a>

### 21.11. Повторить запрос после закрытого idle-соединения

**Задача.** Переиспользовать соединения к API: HttpClient реализует Exchanging с явными resolver, TLS и политикой.

**Какие библиотеки использовать.** [`os/environment`](../std/os/environment.ok), [`http/client`](../std/http/client.ok), [`http/retry`](../std/http/retry.ok), [`http/client_policy`](../std/http/client_policy.ok), [`http/endpoint`](../std/http/endpoint.ok), [`http/version`](../std/http/version.ok), [`http/message`](../std/http/message.ok), [`http/method`](../std/http/method.ok), [`http/header`](../std/http/header.ok), [`http/target`](../std/http/target.ok), [`http/body`](../std/http/body.ok), [`collections/set`](../std/collections/set.ok), [`net/dns`](../std/net/dns.ok), [`net/tls`](../std/net/tls.ok), [`crypto/x509/trust`](../std/crypto/x509/trust.ok), [`net/ip`](../std/net/ip.ok), [`time/clock`](../std/time/clock.ok), [`io/stdout`](../std/io/stdout.ok), [`url/value`](../std/url/value.ok), [`numbers/bigint`](../std/numbers/bigint.ok), [`async/atomic`](../std/async/atomic.ok), [`error`](../std/error.ok).

**Как использовать.** Оберните Exchanging в Retrying. Подготовленный стенд закрывает транспорт до ответа и позволяет проверить ровно допустимый повтор.

**Входы примера.** Передайте через окружение `OKKAMA_HTTP_URL`, `OKKAMA_RETRY_CASE`. Адреса и файлы задают подготовленный стенд; проверяемые ответы видны в `confirm`.

```okkama
module app v1.0.0;
import "os/environment" take Environment;
import "http/client" take HttpClient,HttpStaleConnection;
import "http/retry" take Retrying;
import "http/client_policy" take ClientPolicy,HttpLimits,ClientTimeouts;
import "http/endpoint" take ListedNetworks;
import "http/version" take HttpVersion;
import "http/message" take Request,InvalidHttpMessage;
import "http/method" take Method;
import "http/header" take Headers;
import "http/target" take AbsoluteTarget;
import "http/body" take EmptyBody,OneShotBody,ByteBody,BodyOpening;
import "collections/set" take Set,OrderedSet;
import "net/dns" take FixedResolution;
import "net/tls" take TlsClientPolicy;
import "crypto/x509/trust" take TrustedCertificates;
import "net/ip" take IpAddress,IpNetwork;
import "time/clock" take SystemClock;
import "io/stdout" take Stdout;
import "url/value" take ParsedUrl;
import "http/body" take BodyReading;
import "numbers/bigint" take BigInt;
import "async/atomic" take Atomic;
import "error" take InvalidState;
object NonReplayableEmpty does BodyOpening{
    has opened is Atomic<bool>;NonReplayableEmpty(){opened=Atomic(false);}
    length() is BigInt{BigInt("0");}replayable() is bool{false;}
    reading() is BodyReading{confirm opened.compare_and_replace(false,true) else throw InvalidState("fixture body opened twice");EmptyBody().reading();}
}
async main() is int{
    let environment=Environment();let scenario=environment.variable("OKKAMA_RETRY_CASE");let url=ParsedUrl(environment.variable("OKKAMA_HTTP_URL")).value();
    let enabled is @HttpVersion=if scenario=="mixed"{[HttpVersion.http_1_1,HttpVersion.http_2];}else{[HttpVersion.http_1_1];};let versions is Set<HttpVersion>=OrderedSet(enabled);let policy=ClientPolicy(versions,HttpLimits(),ClientTimeouts(),1,ListedNetworks([IpNetwork(IpAddress("127.0.0.0"),8)]));
    let client=HttpClient(FixedResolution([]),TlsClientPolicy(TrustedCertificates([])),policy,SystemClock());defer client.close();let retry=Retrying(client);
    let warm=Request(Method("GET"),AbsoluteTarget(url),Headers([]));if scenario!="fresh"{let first=await client.response(warm);confirm first.response().status().code()==204;}
    let method=if scenario=="post"{"POST";}else{"GET";};
    let body is BodyOpening=if scenario=="oneshot"{NonReplayableEmpty();}else{EmptyBody();};
    let request=Request(Method(method),AbsoluteTarget(url),Headers([]),body);
    if scenario=="retry" || scenario=="mixed"{let result=await retry.response(request);let reader=result.response().body().reading();defer reader.close();confirm await reader.bytes(8)==[66.byte()];confirm (await reader.bytes(8)).size()==0;}
    else{confirm try{await retry.response(request);false;}catch e is HttpStaleConnection{scenario=="post" || scenario=="oneshot";}catch e is InvalidHttpMessage{scenario=="partial" || scenario=="informational" || scenario=="fresh";};}
    Stdout().writeln("retry case "+scenario+" passed");0;
}
```

[Исходный пример](../examples/std-v4/http-retry.ok).

<a id="example-http-robots"></a>

### 21.12. Выбрать правила обхода каталога сайта

**Задача.** Соблюдать правила сайта при сборе каталога: представления robots выбирают правила для user-agent и пути.

**Какие библиотеки использовать.** [`http/robots`](../std/http/robots.ok), [`url/value`](../std/url/value.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Представьте robots-документ и user-agent. Проверка пути выбирает применимую группу и наиболее подходящее правило.

```okkama
module example v1.0.0;
import "http/robots" take RobotsPolicy;
import "url/value" take ParsedUrl;
import "io/stdout" take Stdout;
main() is int{
    let policy=RobotsPolicy("User-agent: *\nDisallow: /private\nAllow: /private/public\n");
    let denied=policy.allowed("ExampleBot",ParsedUrl("https://example.test/private").value());
    let allowed=policy.allowed("ExampleBot",ParsedUrl("https://example.test/private/public").value());
    Stdout().writeln(denied.string()+" "+allowed.string());0;
}
```

[Исходный пример](../examples/std-v4/http-robots.ok).

<a id="example-http-selected-connecting"></a>

### 21.13. Согласовать HTTP transport по выбранному протоколу

**Задача.** Выбрать разговор по фактически согласованному протоколу: HttpSelectedConnecting возвращает подходящий HTTP transport.

**Какие библиотеки использовать.** [`os/environment`](../std/os/environment.ok), [`fs/path`](../std/fs/path.ok), [`fs/file`](../std/fs/file.ok), [`net/ip`](../std/net/ip.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/tls`](../std/net/tls.ok), [`crypto/x509/certificate`](../std/crypto/x509/certificate.ok), [`crypto/x509/trust`](../std/crypto/x509/trust.ok), [`collections/set`](../std/collections/set.ok), [`http/selected_connection`](../std/http/selected_connection.ok), [`http/client_policy`](../std/http/client_policy.ok), [`time/clock`](../std/time/clock.ok), [`http/endpoint`](../std/http/endpoint.ok), [`url/value`](../std/url/value.ok), [`net/dns`](../std/net/dns.ok), [`http/body`](../std/http/body.ok), [`http/message`](../std/http/message.ok), [`http/method`](../std/http/method.ok), [`http/target`](../std/http/target.ok), [`http/header`](../std/http/header.ok), [`http/version`](../std/http/version.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Выбор соединения связывает client protocols с resolver и TLS. Наблюдайте фактически приобретённый разговор и его версию.

**Входы примера.** Передайте через окружение `OKKAMA_HTTP_URL`, `OKKAMA_TLS_ROOT`. Адреса и файлы задают подготовленный стенд; проверяемые ответы видны в `confirm`.

```okkama
module app v1.0.0;
import "os/environment" take Environment;
import "fs/path" take Path;
import "fs/file" take File,FileMode;
import "net/ip" take IpAddress;
import "net/endpoint" take HostName;
import "net/tls" take TlsClientPolicy,TlsVersion;
import "crypto/x509/certificate" take Certificate;
import "crypto/x509/trust" take TrustedCertificates;
import "collections/set" take Set,OrderedSet;
import "http/selected_connection" take HttpSelectedConnecting;
import "http/client_policy" take ClientPolicy,HttpLimits,ClientTimeouts;
import "time/clock" take SystemClock;
import "http/endpoint" take HttpEndpointSelection,ListedNetworks;
import "url/value" take ParsedUrl,Url,UrlParts,UrlFragment;
import "net/dns" take FixedResolution,HostAddresses;
import "net/ip" take IpNetwork;
import "http/body" take EmptyBody;
import "http/message" take Request;
import "http/method" take Method;
import "http/target" take AbsoluteTarget;
import "http/header" take Headers;
import "http/version" take HttpVersion;
import "io/stdout" take Stdout;

async main() is int {
    let environment=Environment();
    let file=File(mode:FileMode.reading,path:Path(environment.variable("OKKAMA_TLS_ROOT")),binary:true);defer file.close();
    let roots=TrustedCertificates([Certificate(file.binary().bytes(1048577))]);
    let versions is Set<TlsVersion>=OrderedSet([TlsVersion.tls1_2,TlsVersion.tls1_3]);
    let policy=TlsClientPolicy(roots,versions,["h2","http/1.1"]);
    let url=ParsedUrl(environment.variable("OKKAMA_HTTP_URL")).value();
    let resolver=FixedResolution([HostAddresses(HostName("example.test"),[IpAddress("127.0.0.1")]),HostAddresses(HostName("wrong.test"),[IpAddress("127.0.0.1")])]);
    let route=HttpEndpointSelection(url,resolver,ListedNetworks([IpNetwork(IpAddress("127.0.0.0"),8)]));
    let enabled is Set<HttpVersion>=OrderedSet([HttpVersion.http_1_1,HttpVersion.http_2]);let client=ClientPolicy(enabled,HttpLimits(),ClientTimeouts(),1,ListedNetworks([IpNetwork(IpAddress("127.0.0.0"),8)]));
    let acquired=await HttpSelectedConnecting(route,policy,client,SystemClock()).acquisition();let session=acquired.session();defer await session.settled();defer session.close();
    let parts=url.parts();let target=Url(UrlParts(parts.scheme(),parts.authority(),parts.path(),parts.query(),UrlFragment()));
    let request=Request(Method("GET"),AbsoluteTarget(target),Headers([]),EmptyBody(),HttpVersion.http_2);
    let exchange=await session.exchange(request);
    confirm exchange.response().status().code()==204;
    let version=if acquired.details().version()==HttpVersion.http_2{"HTTP2";}else{"HTTP1";};Stdout().writeln("verified negotiated "+version+" session returned 204");0;
}
```

[Исходный пример](../examples/std-v4/http-selected-connecting.ok).

<a id="chapter-http-wire"></a>

## 22. HTTP/1 и HTTP/2: протокольные слои

При разработке собственного транспорта разделяйте head, framing, body и session. HTTP/1 использует последовательный обмен с телом-владельцем admission; HTTP/2 распределяет несколько потоков и их окна внутри одного выбранного транспорта.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`http/http1_body`](../std/http/http1_body.ok) | Прочитать HTTP/1-тело по выбранному плану: Http1BodyReading связывает framing с транспортом и владельцем. | `Http1BodyReading`, `Http1IncomingBody`, `Http1BodyInput`, `Http1BodyStep`, `Http1LineTail` | [Рецепт](#example-http1-body) |
| [`http/http1_body_check`](../std/http/http1_body_check.ok) | Проверить исходящее тело перед head: Http1BodyCheck выполняет отложенную согласованную проверку метаданных. | `Http1BodyCheck` | [Рецепт](#example-http1-message-writing) |
| [`http/http1_body_writing`](../std/http/http1_body_writing.ok) | Потоково отправить HTTP/1-тело: Http1BodyWriting соблюдает framing и ожидает завершения операций записи. | `Http1BodyWriting`, `Http1BodyWriteOperation`, `Http1BodyWriteCleanup`, `Http1ReadSettlement`, `Http1WriteSettlement` | [Рецепт](#example-http1-body-writing) |
| [`http/http1_chunk`](../std/http/http1_chunk.ok) | Разобрать очередной chunk и trailers: ParsedHttp1ChunkLine и ParsedHttp1Trailers проверяют wire-грамматику. | `ParsedHttp1ChunkLine`, `ParsedHttp1Trailers` | [Рецепт](#example-http1-chunks) |
| [`http/http1_continue`](../std/http/http1_continue.ok) | Обработать Expect: 100-continue: Http1RequestStart и состояние продолжения разделяют head, interim и чтение body. | `Http1RequestStart`, `Http1ContinueHead`, `Http1ContinueState` | [Рецепт](#example-https-http1-session) |
| [`http/http1_framing`](../std/http/http1_framing.ok) | Определить границу HTTP/1-тела: request/response framing выбирает chunked, длину и правила завершения. | `Http1BodyKind`, `Http1BodyPlan`, `Http1RequestFraming`, `Http1ResponseFraming`, `Http1FramingFields` | [Рецепт](#example-http1-framing) |
| [`http/http1_head`](../std/http/http1_head.ok) | Разобрать стартовую строку и поля HTTP/1: ParsedHttp1RequestHead и ParsedHttp1ResponseHead проверяют head и бюджеты. | `Http1Limits`, `ParsedHttp1RequestHead`, `ParsedHttp1ResponseHead`, `Http1HeadLines`, `Http1RequestHead` | [Рецепт](#example-http1-framing) |
| [`http/http1_message_writing`](../std/http/http1_message_writing.ok) | Отправить сообщение HTTP/1 целиком: Http1RequestWriting и Http1ResponseWriting связывают head и body. | `Http1RequestWriting`, `Http1ResponseWriting`, `Http1MessageOutput`, `Http1MessageWriteOperation` | [Рецепт](#example-http1-message-writing) |
| [`http/http1_reading`](../std/http/http1_reading.ok) | Получить один head из асинхронного потока: head-reading операции удерживают точный остаток прочитанных байтов. | `Http1RequestHeadReading`, `Http1ResponseHeadReading`, `Http1HeadRead`, `Http1RequestHeadDecoding`, `Http1ResponseHeadDecoding` | [Рецепт](#example-http1-writing) |
| [`http/http1_session`](../std/http/http1_session.ok) | Обменяться запросами в одном HTTP/1-соединении: Http1Session удерживает admission до завершения тела ответа. | `Http1Session`, `Http1Exchange`, `Http1SessionHeads`, `Http1SessionLease`, `Http1WritingSettlement` | [Рецепт](#example-http1-session) |
| [`http/http1_writing`](../std/http/http1_writing.ok) | Записать head HTTP/1 в транспорт: head-writing операции сначала формируют проверенные канонические байты. | `Http1RequestHeadWriting`, `Http1ResponseHeadWriting`, `Http1HeadOutput`, `Http1HeadWriteOperation`, `Http1HeadWire` | [Рецепт](#example-http1-writing) |
| [`http/http2_connection`](../std/http/http2_connection.ok) | Открыть HTTP/2-соединение: Http2Connecting выбирает явный prior knowledge либо подтверждённый h2 ALPN. | `Http2Connecting`, `Http2Acquisition`, `Http2AcquisitionDisposal` | [Рецепт](#example-http2-connecting) |
| [`http/http2_server`](../std/http/http2_server.ok) | Обслуживать несколько HTTP/2-потоков: Http2ServerSession связывает выбранный транспорт с обработчиком приложения. | `Http2ServerSession`, `Http2ServerState`, `Http2ServerStream`, `Http2ServerBodySending`, `Http2ServerBody` | [Рецепт](#example-http2-server) |
| [`http/http2_session`](../std/http/http2_session.ok) | Параллельно обмениваться HTTP/2-запросами: Http2Session владеет потоками и выдаёт отдельные тела ответов. | `Http2Session`, `Http2Exchange`, `Http2SessionState`, `Http2StreamHolding`, `Http2HoldingSignal` | [Рецепт](#example-http2-session) |
| [`http/http2_settings`](../std/http/http2_settings.ok) | Задать локальные лимиты HTTP/2: Http2Settings описывает receive-окна, таблицу и параллельные потоки. | `Http2Settings`, `Http2SettingsWire`, `Http2Setting` | [Рецепт](#example-http2-settings) |
| [`http/http2_tls`](../std/http/http2_tls.ok) | Подтвердить HTTP/2 поверх TLS: Http2TlsProtocol принимает фактически согласованный h2. | `Http2TlsProtocol` | [Рецепт](#example-http2-connecting) |
| [`http/tls_session`](../std/http/tls_session.ok) | Связать проверенный TLS с HTTP/1: Http1TlsProtocol и Http1TlsSession проверяют фактический ALPN. | `Http1TlsProtocol`, `Http1TlsSession` | [Рецепт](#example-https-http1-session) |
| [`http/transport_observation`](../std/http/transport_observation.ok) | Различить отказ до ответа и частичный ответ: HttpResponseObservation отслеживает полученные транспортные байты. | `HttpResponseObservation`, `HttpPeerGoneBeforeResponse`, `HttpObservedReading`, `HttpObservedWriting`, `HttpObservationSettlement` | [Рецепт](#example-http1-connecting) |

### Что учитывать при сборке решения

- `http/http2_session`: Тела разных stream читаются независимо; session управляет общим транспортом и flow-control.

<a id="example-http2-preface"></a>

### 22.1. Согласовать начало HTTP/2-разговора

**Задача.** Задать локальные лимиты HTTP/2: Http2Settings описывает receive-окна, таблицу и параллельные потоки.

**Какие библиотеки использовать.** [`http/http2_settings`](../std/http/http2_settings.ok), [`http/internal/http2_preface`](../std/http/internal/http2_preface.ok), [`http/internal/http2_frame`](../std/http/internal/http2_frame.ok), [`http/internal/http2_reading`](../std/http/internal/http2_reading.ok), [`net/tcp`](../std/net/tcp.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/ip`](../std/net/ip.ok), [`io/decorators`](../std/io/decorators.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok), [`async/atomic`](../std/async/atomic.ok).

**Как использовать.** Preface связывает байтовые потоки с локальными settings. Результат начального обмена передаётся session.

```okkama
module app v1.0.0;
import "http/http2_settings" take Http2Settings,Http2SettingsWire;
import "http/internal/http2_preface" take Http2ClientPreface;
import "http/internal/http2_frame" take Http2Frame,Http2FrameHeader;
import "http/internal/http2_reading" take Http2FrameReading;
import "net/tcp" take TcpListening,TcpConnecting,TcpAcceptance;
import "net/endpoint" take IpEndpoint;
import "net/ip" take IpAddress;
import "io/decorators" take ByteSnapshot;
import "encoding/utf" take Utf8Encoded;
import "io/stdout" take Stdout;
import "async/atomic" take Atomic;

object Peer {
    has listener is TcpAcceptance;Peer(listener is TcpAcceptance){self.listener=listener;}
    async run() is bool{
        let connection=await listener.connection();defer connection.close();
        let magic=Utf8Encoded("PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").bytes();let offset=Atomic(0);
        loop{
            if offset.value()==magic.size(){last;}
            let chunk=await connection.input().bytes(magic.size()-offset.value());
            confirm chunk.size()>0;
            for chunk -> octet{confirm octet==magic[offset.value()];offset.replace(offset.value()+1);}
        }
        let client=await Http2FrameReading(connection.input()).result();
        confirm client.header().kind().int()==4 && client.header().flags().int()==0;
        let payload=Http2SettingsWire(Http2Settings(0,0,32768,0),false).bytes();
        await connection.output().write(ByteSnapshot(Http2Frame(Http2FrameHeader(4.byte(),0.byte(),0,payload.size()),payload).bytes()));
        let acknowledgment=await Http2FrameReading(connection.input()).result();
        acknowledgment.header().kind().int()==4 && acknowledgment.header().flags().int()==1 && acknowledgment.payload().size()==0;
    }
}
async main() is int{
    let listener=await TcpListening(IpEndpoint(IpAddress("127.0.0.1"),0)).acceptance();defer listener.close();
    let peer=Peer(listener);let action is async () is bool=peer.run;let serving=spawn action();
    let connection=await TcpConnecting(listener.local_endpoint()).connection();defer connection.close();
    let settings=try{await Http2ClientPreface(connection.input(),connection.output(),Http2Settings()).result();}
        catch failure is Error{await serving;throw;};
    confirm settings.frame_size()==32768 && settings.initial_window()==0 && settings.streams()==0;
    confirm await serving;Stdout().writeln("HTTP2 preface and SETTINGS ACK over one loopback connection");0;
}
```

[Исходный пример](../examples/std-v4/http2-preface.ok).

<a id="example-hpack-primitives"></a>

### 22.2. Проверить кодирование полей HTTP/2

**Задача.** Показать двоичный идентификатор в журнале: HexEncoded выдаёт каноническую hex-строку, HexDecoded восстанавливает байты.

**Какие библиотеки использовать.** [`http/internal/hpack_primitives`](../std/http/internal/hpack_primitives.ok), [`encoding/hex`](../std/encoding/hex.ok), [`encoding/single_byte`](../std/encoding/single_byte.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Пример обращается к bounded HPACK primitives. Эти операции полезны при проверке собственного транспорта и побайтовых тестах wire-формата.

```okkama
module app v1.0.0;
import "http/internal/hpack_primitives" take HpackInteger,ParsedHpackInteger,HpackHuffman;
import "encoding/hex" take HexDecoded;
import "encoding/single_byte" take Latin1Decoded;
import "io/stdout" take Stdout;
main() is int{
    confirm ParsedHpackInteger(HpackInteger(1337,5,0).bytes(),0,5).value()==1337;
    let octets=HpackHuffman(HexDecoded("f1e3c2e5f23a6ba0ab90f4ff").bytes(),15).bytes();
    confirm Latin1Decoded(octets).string()=="www.example.com";
    Stdout().writeln("bounded HPACK integer and Huffman RFC vectors verified");0;
}
```

[Исходный пример](../examples/std-v4/hpack-primitives.ok).

<a id="example-http1-connecting"></a>

### 22.3. Подключить собственный HTTP/1 transport

**Задача.** Разобрать стартовую строку и поля HTTP/1: ParsedHttp1RequestHead и ParsedHttp1ResponseHead проверяют head и бюджеты.

**Какие библиотеки использовать.** [`os/environment`](../std/os/environment.ok), [`fs/path`](../std/fs/path.ok), [`fs/file`](../std/fs/file.ok), [`net/ip`](../std/net/ip.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/tls`](../std/net/tls.ok), [`crypto/x509/certificate`](../std/crypto/x509/certificate.ok), [`crypto/x509/trust`](../std/crypto/x509/trust.ok), [`collections/set`](../std/collections/set.ok), [`http/connection`](../std/http/connection.ok), [`http/endpoint`](../std/http/endpoint.ok), [`url/value`](../std/url/value.ok), [`net/dns`](../std/net/dns.ok), [`http/http1_head`](../std/http/http1_head.ok), [`http/body`](../std/http/body.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Http1Connecting получает resolver, endpoint policy и часы. Приобретённый разговор объединяет обмен с владением транспортом.

**Входы примера.** Передайте через окружение `OKKAMA_HTTP_URL`, `OKKAMA_TLS_ROOT`. Адреса и файлы задают подготовленный стенд; проверяемые ответы видны в `confirm`.

```okkama
module app v1.0.0;
import "os/environment" take Environment;
import "fs/path" take Path;
import "fs/file" take File,FileMode;
import "net/ip" take IpAddress;
import "net/endpoint" take HostName;
import "net/tls" take TlsClientPolicy,TlsVersion;
import "crypto/x509/certificate" take Certificate;
import "crypto/x509/trust" take TrustedCertificates;
import "collections/set" take Set,OrderedSet;
import "http/connection" take Http1Connecting;
import "http/endpoint" take HttpEndpointSelection,ListedNetworks;
import "url/value" take ParsedUrl;
import "net/dns" take FixedResolution,HostAddresses;
import "net/ip" take IpNetwork;
import "http/http1_head" take ParsedHttp1RequestHead;
import "http/body" take EmptyBody;
import "io/stdout" take Stdout;

async main() is int {
    let environment=Environment();
    let file=File(mode:FileMode.reading,path:Path(environment.variable("OKKAMA_TLS_ROOT")),binary:true);defer file.close();
    let roots=TrustedCertificates([Certificate(file.binary().bytes(1048577))]);
    let versions is Set<TlsVersion>=OrderedSet([TlsVersion.tls1_2,TlsVersion.tls1_3]);
    let policy=TlsClientPolicy(roots,versions,["http/1.1"]);
    let url=ParsedUrl(environment.variable("OKKAMA_HTTP_URL")).value();
    let resolver=FixedResolution([HostAddresses(HostName("example.test"),[IpAddress("127.0.0.1")]),HostAddresses(HostName("wrong.test"),[IpAddress("127.0.0.1")])]);
    let route=HttpEndpointSelection(url,resolver,ListedNetworks([IpNetwork(IpAddress("127.0.0.0"),8)]));
    let session=await Http1Connecting(route,policy).session();defer session.close();
    let parts=url.parts();let path=if parts.path()==""{"/";}else{parts.path();};
    let target=path+if parts.query().present(){"?"+parts.query().string();}else{"";};
    let request=ParsedHttp1RequestHead("GET "+target+" HTTP/1.1\r\nHost: "+parts.authority().string()+"\r\n\r\n").value().request(EmptyBody());
    let exchange=await session.exchange(request);
    confirm exchange.response().status().code()==204;
    Stdout().writeln("verified HTTPS session returned 204");0;
}
```

[Исходный пример](../examples/std-v4/http1-connecting.ok).

<a id="example-http1-body"></a>

### 22.4. Прочитать HTTP/1-тело по framing-плану

**Задача.** Прочитать HTTP/1-тело по выбранному плану: Http1BodyReading связывает framing с транспортом и владельцем.

**Какие библиотеки использовать.** [`http/http1_body`](../std/http/http1_body.ok), [`http/http1_head`](../std/http/http1_head.ok), [`http/http1_framing`](../std/http/http1_framing.ok), [`http/header`](../std/http/header.ok), [`http/method`](../std/http/method.ok), [`net/unix`](../std/net/unix.ok), [`io/stream`](../std/io/stream.ok), [`io/buffer`](../std/io/buffer.ok), [`io/decorators`](../std/io/decorators.ok), [`encoding/single_byte`](../std/encoding/single_byte.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Http1BodyReading получает body plan и точный префикс. Пустой блок после корректного завершения освобождает выбранного владельца.

```okkama
module app v1.0.0;
import "http/http1_body" take Http1BodyReading,Http1IncomingBody;
import "http/http1_head" take ParsedHttp1ResponseHead;
import "http/http1_framing" take Http1ResponseFraming;
import "http/header" take HeaderName;
import "http/method" take Method;
import "net/unix" take UnixPaired,UnixConnection;
import "io/stream" take Closing;
import "io/buffer" take ByteBuffer;
import "io/decorators" take ByteSnapshot;
import "encoding/single_byte" take Latin1Encoded,Latin1Decoded;
import "io/stdout" take Stdout;
object ConnectionOwner does Closing {
    has origin is UnixConnection;ConnectionOwner(origin is UnixConnection){self.origin=origin;}
    closed() is bool{origin.closed();}close() is void{origin.close();}
}
async main() is int {
    let pair=await UnixPaired().result();let input=pair.first();let output=pair.second();defer input.close();defer output.close();
    let head=ParsedHttp1ResponseHead("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n").value();
    await output.output().write(ByteSnapshot(Latin1Encoded("4\r\nbody\r\n0\r\nX-Checksum: completed\r\n\r\n").bytes()));
    let reader=Http1BodyReading(input.input(),ConnectionOwner(input),Http1ResponseFraming(head,Method("GET")).plan(),[],head.headers());
    let body=Http1IncomingBody(reader);let reading=body.reading();defer reading.close();
    let content=ByteBuffer();defer content.closing().close();
    loop {let chunk=await reading.bytes(2);last if chunk.size()==0;content.writing().write(ByteSnapshot(chunk));}
    Stdout().writeln(Latin1Decoded(content.content().bytes()).string());
    Stdout().writeln(reading.trailers().first(HeaderName("X-Checksum")));0;
}
```

[Исходный пример](../examples/std-v4/http1-body.ok).

<a id="example-http1-body-writing"></a>

### 22.5. Отправить HTTP/1-тело частями

**Задача.** Записать head HTTP/1 в транспорт: head-writing операции сначала формируют проверенные канонические байты.

**Какие библиотеки использовать.** [`http/http1_writing`](../std/http/http1_writing.ok), [`http/http1_body_writing`](../std/http/http1_body_writing.ok), [`http/http1_reading`](../std/http/http1_reading.ok), [`http/http1_body`](../std/http/http1_body.ok), [`http/http1_head`](../std/http/http1_head.ok), [`http/http1_framing`](../std/http/http1_framing.ok), [`http/method`](../std/http/method.ok), [`http/body`](../std/http/body.ok), [`net/unix`](../std/net/unix.ok), [`io/stream`](../std/io/stream.ok), [`io/buffer`](../std/io/buffer.ok), [`io/decorators`](../std/io/decorators.ok), [`encoding/single_byte`](../std/encoding/single_byte.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Http1BodyWriting связывает body opening, framing и sink. Каждая запись ожидается до перехода к следующему блоку.

```okkama
module app v1.0.0;
import "http/http1_writing" take Http1ResponseHeadWriting;
import "http/http1_body_writing" take Http1BodyWriting;
import "http/http1_reading" take Http1ResponseHeadReading;
import "http/http1_body" take Http1BodyReading;
import "http/http1_head" take ParsedHttp1ResponseHead;
import "http/http1_framing" take Http1ResponseFraming;
import "http/method" take Method;
import "http/body" take ByteBody;
import "net/unix" take UnixPaired,UnixConnection;
import "io/stream" take Closing;
import "io/buffer" take ByteBuffer;
import "io/decorators" take ByteSnapshot;
import "encoding/single_byte" take Latin1Encoded,Latin1Decoded;
import "io/stdout" take Stdout;
object ConnectionOwner does Closing {
    has origin is UnixConnection;ConnectionOwner(origin is UnixConnection){self.origin=origin;}
    closed() is bool{origin.closed();}close() is void{origin.close();}
}
async main() is int {
    let pair=await UnixPaired().result();let input=pair.first();let output=pair.second();defer input.close();defer output.close();
    let head=ParsedHttp1ResponseHead("HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n").value();
    let plan=Http1ResponseFraming(head,Method("GET")).plan();
    await Http1ResponseHeadWriting(output.output(),head,Method("GET")).result();
    await Http1BodyWriting(output.output(),ByteBody(Latin1Encoded("body").bytes()),plan,head.headers()).result();
    let parsed=await Http1ResponseHeadReading(input.input()).result();
    let reader=Http1BodyReading(input.input(),ConnectionOwner(input),plan,parsed.remainder(),parsed.head().headers());defer reader.close();
    let buffer=ByteBuffer();defer buffer.closing().close();
    loop {let chunk=await reader.bytes(2);last if chunk.size()==0;buffer.writing().write(ByteSnapshot(chunk));}
    Stdout().writeln(Latin1Decoded(buffer.content().bytes()).string());0;
}
```

[Исходный пример](../examples/std-v4/http1-body-writing.ok).

<a id="example-http1-chunks"></a>

### 22.6. Разобрать длину chunk и trailers

**Задача.** Разобрать очередной chunk и trailers: ParsedHttp1ChunkLine и ParsedHttp1Trailers проверяют wire-грамматику.

**Какие библиотеки использовать.** [`http/http1_chunk`](../std/http/http1_chunk.ok), [`http/header`](../std/http/header.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ParsedHttp1ChunkLine проверяет заголовок chunk. Завершающие поля проходят отдельную проверку ParsedHttp1Trailers.

```okkama
module app v1.0.0;
import "http/http1_chunk" take ParsedHttp1ChunkLine,ParsedHttp1Trailers;
import "http/header" take Headers,HeaderName;
import "io/stdout" take Stdout;
main() is int {
    let size=ParsedHttp1ChunkLine("a; comment=\"ignored; safely\"").size();
    let trailers=ParsedHttp1Trailers("X-Checksum: completed\r\n\r\n",Headers([])).value();
    Stdout().writeln(size.string());Stdout().writeln(trailers.first(HeaderName("X-Checksum")));0;
}
```

[Исходный пример](../examples/std-v4/http1-chunks.ok).

<a id="example-http1-framing"></a>

### 22.7. Определить длину и способ передачи тела

**Задача.** Разобрать стартовую строку и поля HTTP/1: ParsedHttp1RequestHead и ParsedHttp1ResponseHead проверяют head и бюджеты.

**Какие библиотеки использовать.** [`http/http1_head`](../std/http/http1_head.ok), [`http/http1_framing`](../std/http/http1_framing.ok), [`http/method`](../std/http/method.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** RequestFraming и ResponseFraming получают проверенные метаданные. Выбранный body plan используется читающим или пишущим слоем.

```okkama
module app v1.0.0;
import "http/http1_head" take ParsedHttp1ResponseHead;
import "http/http1_framing" take Http1ResponseFraming,Http1BodyKind;
import "http/method" take Method;
import "io/stdout" take Stdout;
main() is int {
    let head=ParsedHttp1ResponseHead("HTTP/1.1 200 OK\r\nContent-Length: 4\r\nConnection: close\r\n\r\n").value();
    let plan=Http1ResponseFraming(head,Method("GET")).plan();
    confirm plan.kind()==Http1BodyKind.fixed && !plan.reuse_after_eof();
    Stdout().writeln(plan.length().string());0;
}
```

[Исходный пример](../examples/std-v4/http1-framing.ok).

<a id="example-http1-message-writing"></a>

### 22.8. Отправить HTTP/1 head и body одной операцией

**Задача.** Отправить сообщение HTTP/1 целиком: Http1RequestWriting и Http1ResponseWriting связывают head и body.

**Какие библиотеки использовать.** [`http/http1_message_writing`](../std/http/http1_message_writing.ok), [`http/http1_reading`](../std/http/http1_reading.ok), [`http/http1_body`](../std/http/http1_body.ok), [`http/http1_framing`](../std/http/http1_framing.ok), [`http/message`](../std/http/message.ok), [`http/status`](../std/http/status.ok), [`http/header`](../std/http/header.ok), [`http/method`](../std/http/method.ok), [`http/body`](../std/http/body.ok), [`net/unix`](../std/net/unix.ok), [`io/stream`](../std/io/stream.ok), [`io/buffer`](../std/io/buffer.ok), [`io/decorators`](../std/io/decorators.ok), [`encoding/single_byte`](../std/encoding/single_byte.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Message-writing сотрудник сначала связывает метаданные и framing, затем согласованно пишет head и открытое тело.

```okkama
module app v1.0.0;
import "http/http1_message_writing" take Http1ResponseWriting;
import "http/http1_reading" take Http1ResponseHeadReading;
import "http/http1_body" take Http1BodyReading;
import "http/http1_framing" take Http1ResponseFraming;
import "http/message" take Response;
import "http/status" take Status;
import "http/header" take Headers,Header,HeaderName;
import "http/method" take Method;
import "http/body" take ByteBody;
import "net/unix" take UnixPaired,UnixConnection;
import "io/stream" take Closing;
import "io/buffer" take ByteBuffer;
import "io/decorators" take ByteSnapshot;
import "encoding/single_byte" take Latin1Encoded,Latin1Decoded;
import "io/stdout" take Stdout;
object ConnectionOwner does Closing {
    has origin is UnixConnection;ConnectionOwner(origin is UnixConnection){self.origin=origin;}
    closed() is bool{origin.closed();}close() is void{origin.close();}
}
async main() is int {
    let pair=await UnixPaired().result();let input=pair.first();let output=pair.second();defer input.close();defer output.close();
    let response=Response(Status(200),Headers([Header(HeaderName("Content-Length"),"4")]),ByteBody(Latin1Encoded("body").bytes()));
    await Http1ResponseWriting(output.output(),response,Method("GET")).result();
    let parsed=await Http1ResponseHeadReading(input.input()).result();let head=parsed.head();let plan=Http1ResponseFraming(head,Method("GET")).plan();
    let reader=Http1BodyReading(input.input(),ConnectionOwner(input),plan,parsed.remainder(),head.headers());defer reader.close();
    let buffer=ByteBuffer();defer buffer.closing().close();
    loop {let chunk=await reader.bytes(2);last if chunk.size()==0;buffer.writing().write(ByteSnapshot(chunk));}
    Stdout().writeln(Latin1Decoded(buffer.content().bytes()).string());0;
}
```

[Исходный пример](../examples/std-v4/http1-message-writing.ok).

<a id="example-http1-writing"></a>

### 22.9. Сериализовать head HTTP/1

**Задача.** Записать head HTTP/1 в транспорт: head-writing операции сначала формируют проверенные канонические байты.

**Какие библиотеки использовать.** [`http/http1_writing`](../std/http/http1_writing.ok), [`http/http1_reading`](../std/http/http1_reading.ok), [`http/http1_head`](../std/http/http1_head.ok), [`net/unix`](../std/net/unix.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Head-writing операция получает проверенные метаданные и borrowed sink. Канонический head формируется до начала отправки.

```okkama
module app v1.0.0;
import "http/http1_writing" take Http1RequestHeadWriting;
import "http/http1_reading" take Http1RequestHeadReading;
import "http/http1_head" take ParsedHttp1RequestHead;
import "net/unix" take UnixPaired;
import "io/stdout" take Stdout;
async main() is int {
    let pair=await UnixPaired().result();let client=pair.first();let server=pair.second();defer client.close();defer server.close();
    let head=ParsedHttp1RequestHead("GET / HTTP/1.1\r\nHost: example.com\r\n\r\n").value();
    await Http1RequestHeadWriting(client.output(),head).result();
    let received=await Http1RequestHeadReading(server.input()).result();
    Stdout().writeln(received.head().host().string());0;
}
```

[Исходный пример](../examples/std-v4/http1-writing.ok).

<a id="example-http1-session"></a>

### 22.10. Переиспользовать один HTTP/1-разговор

**Задача.** Обменяться запросами в одном HTTP/1-соединении: Http1Session удерживает admission до завершения тела ответа.

**Какие библиотеки использовать.** [`net/tcp`](../std/net/tcp.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/ip`](../std/net/ip.ok), [`http/http1_session`](../std/http/http1_session.ok), [`http/http1_reading`](../std/http/http1_reading.ok), [`http/http1_message_writing`](../std/http/http1_message_writing.ok), [`http/http1_head`](../std/http/http1_head.ok), [`http/message`](../std/http/message.ok), [`http/status`](../std/http/status.ok), [`http/header`](../std/http/header.ok), [`http/body`](../std/http/body.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Session связывает input/output и владельца транспорта. Следующий запрос допускается после корректного завершения body предыдущего ответа.

```okkama
module app v1.0.0;
import "net/tcp" take TcpListening,TcpConnecting,TcpAcceptance;
import "net/endpoint" take IpEndpoint;
import "net/ip" take IpAddress;
import "http/http1_session" take Http1Session;
import "http/http1_reading" take Http1RequestHeadReading;
import "http/http1_message_writing" take Http1ResponseWriting;
import "http/http1_head" take ParsedHttp1RequestHead;
import "http/message" take Response;
import "http/status" take Status;
import "http/header" take Headers,Header,HeaderName;
import "http/body" take ByteBody,EmptyBody;
import "io/stdout" take Stdout;
object Replying {
    has listener is TcpAcceptance;Replying(listener is TcpAcceptance){self.listener=listener;}
    async run() is void {
        let connection=await listener.connection();defer connection.close();
        let first=await Http1RequestHeadReading(connection.input()).result();
        let response=Response(Status(200),Headers([Header(HeaderName("Content-Length"),"3")]),ByteBody([97.byte(),98.byte(),99.byte()]));
        await Http1ResponseWriting(connection.output(),response,first.head().method()).result();
        let second=await Http1RequestHeadReading(connection.input()).result();
        await Http1ResponseWriting(connection.output(),Response(Status(204),Headers([])),second.head().method()).result();
    }
}
async main() is int {
    let listener=await TcpListening(IpEndpoint(IpAddress("127.0.0.1"),0)).acceptance();defer listener.close();
    let replying=Replying(listener);let active=spawn replying.run();let connection=await TcpConnecting(listener.local_endpoint()).connection();
    let session=Http1Session(connection.input(),connection.output(),connection);defer session.close();
    let request=ParsedHttp1RequestHead("GET / HTTP/1.1\r\nHost: example.com\r\n\r\n").value().request(EmptyBody());
    let first=await session.exchange(request);let reader=first.response().body().reading();
    confirm (await reader.bytes(1))[0].int()==97 && (await reader.bytes(1))[0].int()==98 && (await reader.bytes(1))[0].int()==99;
    confirm (await reader.bytes(1)).size()==0;
    let second=await session.exchange(request);confirm second.response().status().code()==204;await active;
    Stdout().writeln("two HTTP1 exchanges on one TCP session verified");0;
}
```

[Исходный пример](../examples/std-v4/http1-session.ok).

<a id="example-http2-connecting"></a>

### 22.11. Открыть выбранное HTTP/2-соединение

**Задача.** Открыть HTTP/2-соединение: Http2Connecting выбирает явный prior knowledge либо подтверждённый h2 ALPN.

**Какие библиотеки использовать.** [`os/environment`](../std/os/environment.ok), [`fs/path`](../std/fs/path.ok), [`fs/file`](../std/fs/file.ok), [`net/ip`](../std/net/ip.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/tls`](../std/net/tls.ok), [`crypto/x509/certificate`](../std/crypto/x509/certificate.ok), [`crypto/x509/trust`](../std/crypto/x509/trust.ok), [`collections/set`](../std/collections/set.ok), [`http/http2_connection`](../std/http/http2_connection.ok), [`http/endpoint`](../std/http/endpoint.ok), [`url/value`](../std/url/value.ok), [`net/dns`](../std/net/dns.ok), [`http/body`](../std/http/body.ok), [`http/message`](../std/http/message.ok), [`http/method`](../std/http/method.ok), [`http/target`](../std/http/target.ok), [`http/header`](../std/http/header.ok), [`http/version`](../std/http/version.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Сначала задайте protocol policy и проверенные адреса. Соединение выполняет H2 preface и передаёт готовую session в обмен.

**Входы примера.** Передайте через окружение `OKKAMA_HTTP_URL`, `OKKAMA_TLS_ROOT`. Адреса и файлы задают подготовленный стенд; проверяемые ответы видны в `confirm`.

```okkama
module app v1.0.0;
import "os/environment" take Environment;
import "fs/path" take Path;
import "fs/file" take File,FileMode;
import "net/ip" take IpAddress;
import "net/endpoint" take HostName;
import "net/tls" take TlsClientPolicy,TlsVersion;
import "crypto/x509/certificate" take Certificate;
import "crypto/x509/trust" take TrustedCertificates;
import "collections/set" take Set,OrderedSet;
import "http/http2_connection" take Http2Connecting;
import "http/endpoint" take HttpEndpointSelection,ListedNetworks;
import "url/value" take ParsedUrl,Url,UrlParts,UrlFragment;
import "net/dns" take FixedResolution,HostAddresses;
import "net/ip" take IpNetwork;
import "http/body" take EmptyBody;
import "http/message" take Request;
import "http/method" take Method;
import "http/target" take AbsoluteTarget;
import "http/header" take Headers;
import "http/version" take HttpVersion;
import "io/stdout" take Stdout;

async main() is int {
    let environment=Environment();
    let file=File(mode:FileMode.reading,path:Path(environment.variable("OKKAMA_TLS_ROOT")),binary:true);defer file.close();
    let roots=TrustedCertificates([Certificate(file.binary().bytes(1048577))]);
    let versions is Set<TlsVersion>=OrderedSet([TlsVersion.tls1_2,TlsVersion.tls1_3]);
    let policy=TlsClientPolicy(roots,versions,["h2"]);
    let url=ParsedUrl(environment.variable("OKKAMA_HTTP_URL")).value();
    let resolver=FixedResolution([HostAddresses(HostName("example.test"),[IpAddress("127.0.0.1")]),HostAddresses(HostName("wrong.test"),[IpAddress("127.0.0.1")])]);
    let route=HttpEndpointSelection(url,resolver,ListedNetworks([IpNetwork(IpAddress("127.0.0.0"),8)]));
    let session=await Http2Connecting(route,policy).session();defer await session.settled();defer session.close();
    let parts=url.parts();let target=Url(UrlParts(parts.scheme(),parts.authority(),parts.path(),parts.query(),UrlFragment()));
    let request=Request(Method("GET"),AbsoluteTarget(target),Headers([]),EmptyBody(),HttpVersion.http_2);
    let exchange=await session.exchange(request);
    confirm exchange.response().status().code()==204;
    Stdout().writeln("verified HTTP2 session returned 204");0;
}
```

[Исходный пример](../examples/std-v4/http2-connecting.ok).

<a id="example-http2-server"></a>

### 22.12. Обслужить несколько HTTP/2 stream

**Задача.** Обслуживать несколько HTTP/2-потоков: Http2ServerSession связывает выбранный транспорт с обработчиком приложения.

**Какие библиотеки использовать.** [`http/http2_server`](../std/http/http2_server.ok), [`http/server_policy`](../std/http/server_policy.ok), [`http/client_policy`](../std/http/client_policy.ok), [`http/handler`](../std/http/handler.ok), [`http/request_context`](../std/http/request_context.ok), [`http/message`](../std/http/message.ok), [`http/body`](../std/http/body.ok), [`http/status`](../std/http/status.ok), [`http/header`](../std/http/header.ok), [`http/version`](../std/http/version.ok), [`http/exchange`](../std/http/exchange.ok), [`collections/set`](../std/collections/set.ok), [`net/tcp`](../std/net/tcp.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/ip`](../std/net/ip.ok), [`net/stream`](../std/net/stream.ok), [`encoding/utf`](../std/encoding/utf.ok), [`time/clock`](../std/time/clock.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Http2ServerSession получает выбранный транспорт и Handling. Потоки имеют отдельные запросы, ответы и правила завершения body.

```okkama
module app v1.0.0;
import "http/http2_server" take Http2ServerSession;
import "http/server_policy" take ServerPolicy,ServerTimeouts;
import "http/client_policy" take HttpLimits;
import "http/handler" take Handling,HandlingResult,ResponseHandling;
import "http/request_context" take HttpRequestContext;
import "http/message" take Request,Response;
import "http/body" take ByteBody;
import "http/status" take Status;
import "http/header" take Headers;
import "http/version" take HttpVersion;
import "http/exchange" take ExchangeDetails;
import "collections/set" take Set,OrderedSet;
import "net/tcp" take TcpListening;
import "net/endpoint" take IpEndpoint;
import "net/ip" take IpAddress;
import "net/stream" take ConnectionEndpoints;
import "encoding/utf" take Utf8Encoded;
import "time/clock" take SystemClock;
import "io/stdout" take Stdout;
object Application does Handling{
    Application();
    async handling(request is Request,context is HttpRequestContext) is HandlingResult{
        ResponseHandling(Response(Status(200),Headers([]),ByteBody(Utf8Encoded("hello").bytes()),HttpVersion.http_2));
    }
}
async main() is int{
    let listener=await TcpListening(IpEndpoint(IpAddress("127.0.0.1"),0)).acceptance();defer listener.close();
    Stdout().writeln(listener.local_endpoint().port().string());let peer=await listener.connection();defer peer.close();
    let endpoints=peer.endpoints();
    let protocols is Set<HttpVersion>=OrderedSet([HttpVersion.http_2]);let policy=ServerPolicy(protocols,HttpLimits(),ServerTimeouts(),1,1);
    let session=Http2ServerSession(peer.input(),peer.output(),peer,Application(),ExchangeDetails(endpoints,HttpVersion.http_2),policy,SystemClock());
    await session.serve();await session.settled();Stdout().writeln("HTTP2 server settled");0;
}
```

[Исходный пример](../examples/std-v4/http2-server.ok).

<a id="example-http2-session"></a>

### 22.13. Выполнить два параллельных запроса HTTP/2

**Задача.** Задать локальные лимиты HTTP/2: Http2Settings описывает receive-окна, таблицу и параллельные потоки.

**Какие библиотеки использовать.** [`http/internal/http2_preface`](../std/http/internal/http2_preface.ok), [`http/http2_settings`](../std/http/http2_settings.ok), [`http/internal/http2_settings`](../std/http/internal/http2_settings.ok), [`http/internal/http2_reading`](../std/http/internal/http2_reading.ok), [`http/internal/http2_frame`](../std/http/internal/http2_frame.ok), [`http/internal/http2_headers`](../std/http/internal/http2_headers.ok), [`http/internal/http2_input`](../std/http/internal/http2_input.ok), [`http/internal/http2_output`](../std/http/internal/http2_output.ok), [`http/internal/http2_sending`](../std/http/internal/http2_sending.ok), [`http/internal/http2_receiving`](../std/http/internal/http2_receiving.ok), [`http/internal/http2_body`](../std/http/internal/http2_body.ok), [`http/internal/http2_fields`](../std/http/internal/http2_fields.ok), [`http/internal/http2_flow`](../std/http/internal/http2_flow.ok), [`http/internal/http2_controls`](../std/http/internal/http2_controls.ok), [`http/internal/hpack`](../std/http/internal/hpack.ok), [`http/internal/hpack_tables`](../std/http/internal/hpack_tables.ok), [`http/body`](../std/http/body.ok), [`http/http2_session`](../std/http/http2_session.ok), [`http/client_policy`](../std/http/client_policy.ok), [`http/message`](../std/http/message.ok), [`http/method`](../std/http/method.ok), [`http/target`](../std/http/target.ok), [`http/version`](../std/http/version.ok), [`url/value`](../std/url/value.ok), [`net/tcp`](../std/net/tcp.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/ip`](../std/net/ip.ok), [`io/decorators`](../std/io/decorators.ok), [`encoding/utf`](../std/encoding/utf.ok), [`async/atomic`](../std/async/atomic.ok), [`async/event`](../std/async/event.ok), [`numbers/bigint`](../std/numbers/bigint.ok), [`http/header`](../std/http/header.ok), [`async/scope`](../std/async/scope.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Пример поднимает локального protocol peer, выполняет preface и обменивается stream 1 и 3. Чтение body проверяет отдельные потоки и общий flow-control.

```okkama
module app v1.0.0;
import "http/internal/http2_preface" take Http2ClientPreface;
import "http/http2_settings" take Http2Settings,Http2SettingsWire;
import "http/internal/http2_settings" take Http2PeerSettings;
import "http/internal/http2_reading" take Http2FrameReading;
import "http/internal/http2_frame" take Http2Frame,Http2FrameHeader,Http2Fault,Http2FaultScope;
import "http/internal/http2_headers" take Http2HeaderBlockReading,Http2HeaderBlock;
import "http/internal/http2_input" take Http2ConnectionInput,Http2ReceivingEvents;
import "http/internal/http2_output" take Http2ConnectionOutput;
import "http/internal/http2_sending" take Http2DataScheduler;
import "http/internal/http2_receiving" take Http2ReceiveFlow;
import "http/internal/http2_body" take Http2BodyBuffer,Http2BodyLength;
import "http/internal/http2_fields" take ParsedHttp2Response,ParsedHttp2Request;
import "http/internal/http2_flow" take Http2DataPayload,Http2WindowIncrement;
import "http/internal/http2_controls" take Http2Reset,Http2GoAway;
import "http/internal/hpack" take HpackDecoder,HpackBudget;
import "http/internal/hpack_tables" take HpackField;
import "http/body" take EmptyBody,ByteBody;
import "http/http2_session" take Http2Session,Http2Exchange;
import "http/client_policy" take HttpLimits;
import "http/message" take Request;
import "http/method" take Method;
import "http/target" take AbsoluteTarget;
import "http/version" take HttpVersion;
import "url/value" take ParsedUrl;
import "net/tcp" take TcpListening,TcpConnecting,TcpAcceptance;
import "net/endpoint" take IpEndpoint;
import "net/ip" take IpAddress;
import "io/decorators" take ByteSnapshot;
import "encoding/utf" take Utf8Encoded;
import "async/atomic" take Atomic;
import "async/event" take Event;
import "numbers/bigint" take BigInt;
import "http/header" take Headers,Header,HeaderName;
import "async/scope" take FutureGroup;
import "io/stdout" take Stdout;

object Peer {
    has listener is TcpAcceptance;Peer(listener is TcpAcceptance){self.listener=listener;}
    async run() is bool{
        let connection=await listener.connection();defer connection.close();
        let magic=Utf8Encoded("PRI * HTTP/2.0\r\n\r\nSM\r\n\r\n").bytes();let offset=Atomic(0);
        loop{
            if offset.value()==magic.size(){last;}
            let bytes=await connection.input().bytes(magic.size()-offset.value());confirm bytes.size()>0;
            for bytes -> octet{confirm octet==magic[offset.value()];offset.replace(offset.value()+1);}
        }
        let settings=await Http2FrameReading(connection.input()).result();confirm settings.header().kind().int()==4;
        let payload=Http2SettingsWire(Http2Settings(),false).bytes();
        await connection.output().write(ByteSnapshot(Http2Frame(Http2FrameHeader(4.byte(),0.byte(),0,payload.size()),payload).bytes()));
        await connection.output().write(ByteSnapshot(Http2Frame(Http2FrameHeader(4.byte(),1.byte(),0,0),[]).bytes()));
        let acknowledgment=await Http2FrameReading(connection.input()).result();confirm acknowledgment.header().flags().int()==1;
        let context=Atomic(HpackDecoder());
        let head1=Event();let head3=Event();let end1=Event();let end3=Event();let count1=Atomic(0);let count3=Atomic(0);
        loop{
            if end1.ready() && end3.ready(){last;}
            let frame=await Http2FrameReading(connection.input()).result();let stream=frame.header().stream();confirm stream==1 || stream==3;
            if frame.header().kind().int()==1{
                let block=await Http2HeaderBlockReading(frame,connection.input(),context.value(),HpackBudget()).result();context.replace(block.context());confirm !block.end_stream();
                let request=ParsedHttp2Request(block.fields(),stream).request(EmptyBody());confirm request.method().string()=="POST";
                if stream==1{confirm !head1.ready();head1.set();}else{confirm !head3.ready();head3.set();}
            }else{
                let data=Http2DataPayload(frame);let expected=if stream==1{65;}else{66;};
                confirm (if stream==1{head1.ready() && !end1.ready();}else{head3.ready() && !end3.ready();});
                for data.bytes() -> octet{confirm octet.int()==expected;}
                if stream==1{count1.replace(count1.value()+data.bytes().size());if data.end_stream(){end1.set();}}
                else{count3.replace(count3.value()+data.bytes().size());if data.end_stream(){end3.set();}}
            }
        }
        confirm count1.value()==1 && count3.value()==1;
        let output=Http2ConnectionOutput(connection.output(),8,65536);
        output.frame(Http2Frame(Http2FrameHeader(6.byte(),0.byte(),0,8),[1.byte(),2.byte(),3.byte(),4.byte(),5.byte(),6.byte(),7.byte(),8.byte()]));
        let second=output.headers(3,[HpackField(":status","200"),HpackField("content-length","2")],HpackBudget(),4,false);
        let first=output.headers(1,[HpackField(":status","200"),HpackField("content-length","1")],HpackBudget(),4,false);
        output.frame(Http2Frame(Http2FrameHeader(0.byte(),9.byte(),3,4),[1.byte(),65.byte(),66.byte(),0.byte()]));
        output.frame(Http2Frame(Http2FrameHeader(0.byte(),1.byte(),1,1),[67.byte()]));output.finish();await output.run();await first.result();await second.result();
        let pong=Event();let credit=Atomic(0);
        for (0..4) -> ignored{
            let frame=await Http2FrameReading(connection.input()).result();
            if frame.header().kind().int()==6{confirm !pong.ready() && frame.header().flags().int()==1 && frame.payload()[7].int()==8;pong.set();}
            else{let increment=Http2WindowIncrement(frame);confirm increment.stream()==0;credit.replace(credit.value()+increment.amount());}
        }
        confirm pong.ready() && credit.value()==5;true;
    }
}
async main() is int{
    let listener=await TcpListening(IpEndpoint(IpAddress("127.0.0.1"),0)).acceptance();defer listener.close();
    let peer=Peer(listener);let action is async () is bool=peer.run;let serving=spawn action();
    let connection=await TcpConnecting(listener.local_endpoint()).connection();defer connection.close();let local=Http2Settings(4096,4,16384,8);
    let settings=await Http2ClientPreface(connection.input(),connection.output(),local).result();
    let session=Http2Session(connection.input(),connection.output(),connection,local,settings,HttpLimits());defer session.close();
    let url=ParsedUrl("http://127.0.0.1:"+listener.local_endpoint().port().string()+"/").value();let fields=Headers([Header(HeaderName("Content-Length"),"1")]);
    let request1=Request(Method("POST"),AbsoluteTarget(url),fields,ByteBody([65.byte()]),HttpVersion.http_2);
    let request3=Request(Method("POST"),AbsoluteTarget(url),fields,ByteBody([66.byte()]),HttpVersion.http_2);
    let first=session.exchange(request1);let third=session.exchange(request3);let group is FutureGroup<Http2Exchange>=FutureGroup();group.start(first);group.start(third);
    let exchange1=await first;let exchange3=await third;confirm exchange1.response().status().code()==200 && exchange3.response().status().code()==200;
    let body3=exchange3.response().body().reading();let body1=exchange1.response().body().reading();
    confirm await body3.bytes(8)==[65.byte(),66.byte()];confirm (await body3.bytes(8)).size()==0;
    confirm await body1.bytes(8)==[67.byte()];confirm (await body1.bytes(8)).size()==0;
    confirm await serving;session.close();await session.settled();
    Stdout().writeln("HTTP2 session: multiplexed typed exchanges and body ownership verified");0;
}
```

[Исходный пример](../examples/std-v4/http2-session.ok).

<a id="example-http2-settings"></a>

### 22.14. Задать окна и лимиты HTTP/2

**Задача.** Задать локальные лимиты HTTP/2: Http2Settings описывает receive-окна, таблицу и параллельные потоки.

**Какие библиотеки использовать.** [`http/http2_settings`](../std/http/http2_settings.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Http2Settings представляет локально объявляемые границы. Wire-представление передаётся протокольному слою SETTINGS.

```okkama
module app v1.0.0;
import "http/http2_settings" take Http2Settings;
import "io/stdout" take Stdout;

main() is int{
    let settings=Http2Settings(4096,65535,16384,100);
    confirm settings.frame_size()==16384 && settings.streams()==100;
    Stdout().writeln("HTTP2 receive frame limit: ",settings.frame_size().string());0;
}
```

[Исходный пример](../examples/std-v4/http2-settings.ok).

<a id="example-https-http1-session"></a>

### 22.15. Использовать HTTP/1 поверх проверенного TLS

**Задача.** Связать проверенный TLS с HTTP/1: Http1TlsProtocol и Http1TlsSession проверяют фактический ALPN.

**Какие библиотеки использовать.** [`os/environment`](../std/os/environment.ok), [`fs/path`](../std/fs/path.ok), [`fs/file`](../std/fs/file.ok), [`strings/parse`](../std/strings/parse.ok), [`net/ip`](../std/net/ip.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/tcp`](../std/net/tcp.ok), [`net/tls`](../std/net/tls.ok), [`crypto/x509/certificate`](../std/crypto/x509/certificate.ok), [`crypto/x509/trust`](../std/crypto/x509/trust.ok), [`collections/set`](../std/collections/set.ok), [`http/tls_session`](../std/http/tls_session.ok), [`http/http1_head`](../std/http/http1_head.ok), [`http/body`](../std/http/body.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Передайте проверенный TLS owner в HTTP/1 protocol/session. Фактический ALPN определяет допустимую адаптацию транспорта.

**Входы примера.** Передайте через окружение `OKKAMA_TLS_PORT`, `OKKAMA_TLS_ROOT`. Адреса и файлы задают подготовленный стенд; проверяемые ответы видны в `confirm`.

```okkama
module app v1.0.0;
import "os/environment" take Environment;
import "fs/path" take Path;
import "fs/file" take File,FileMode;
import "strings/parse" take ParsedInt;
import "net/ip" take IpAddress;
import "net/endpoint" take IpEndpoint,HostName;
import "net/tcp" take TcpConnecting;
import "net/tls" take TlsClientPolicy,TlsClientHandshake,TlsVersion;
import "crypto/x509/certificate" take Certificate;
import "crypto/x509/trust" take TrustedCertificates,DnsCertificateIdentity;
import "collections/set" take Set,OrderedSet;
import "http/tls_session" take Http1TlsSession;
import "http/http1_head" take ParsedHttp1RequestHead;
import "http/body" take EmptyBody;
import "io/stdout" take Stdout;

async main() is int {
    let environment=Environment();
    let file=File(mode:FileMode.reading,path:Path(environment.variable("OKKAMA_TLS_ROOT")),binary:true);defer file.close();
    let roots=TrustedCertificates([Certificate(file.binary().bytes(1048577))]);
    let versions is Set<TlsVersion>=OrderedSet([TlsVersion.tls1_2,TlsVersion.tls1_3]);
    let policy=TlsClientPolicy(roots,versions,["http/1.1"]);
    let tcp=await TcpConnecting(IpEndpoint(IpAddress("127.0.0.1"),ParsedInt(environment.variable("OKKAMA_TLS_PORT")).value())).connection();
    let connection=await TlsClientHandshake(tcp,DnsCertificateIdentity(HostName("example.test")),policy).connection();defer connection.close();
    confirm tcp.closed();
    let session=Http1TlsSession(connection);defer session.close();
    let request=ParsedHttp1RequestHead("GET / HTTP/1.1\r\nHost: example.test\r\n\r\n").value().request(EmptyBody());
    let exchange=await session.exchange(request);
    confirm exchange.response().status().code()==204;
    Stdout().writeln("verified HTTPS session returned 204");0;
}
```

[Исходный пример](../examples/std-v4/https-http1-session.ok).

<a id="chapter-web"></a>

## 23. HTTP-сервер, веб-приложение и потоковые события

Для приложения связывайте bound actions контроллера с маршрутом. На нижнем уровне Handling получает Request и HttpRequestContext. StaticFiles, SSE и WebSocket выбираются как отдельные способы обслуживания запроса.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`http/handler`](../std/http/handler.ok) | Вернуть обычный ответ или upgrade: Handling и HandlingResult задают контракт обработчика сервера. | `HandlingKind`, `HandlingResult`, `Handling`, `WebSocketHandling`, `ResponseHandling` | [Рецепт](#recipe-web) |
| [`http/request_context`](../std/http/request_context.ok) | Получить адрес клиента и отмену в обработчике: HttpRequestContext предоставляет транспортные факты запроса. | `HttpRequestContext`, `HttpContextParameters` | [Рецепт](#example-http-static) |
| [`http/routing`](../std/http/routing.ok) | Извлечь параметр /orders/:id: маршрутизация декодирует сегменты и удерживает именованные параметры. | `RouteSegmentKind`, `RoutedPath`, `RouteLiteral`, `RouteParameterName`, `RouteSegment` | [Рецепт](#example-http-routing) |
| [`http/server`](../std/http/server.ok) | Обслужить API на слушающем TCP-порту: HttpServer владеет listener, вызовами обработчика и завершением работы. | `HttpServer`, `HttpServerState`, `HttpServerCleanup`, `HttpServerPeer`, `HttpServerBodyLease` | [Рецепт](#example-http-server) |
| [`http/server_policy`](../std/http/server_policy.ok) | Выставить бюджеты приёма запросов: ServerPolicy и ServerTimeouts задают ограничения HTTP-сервера. | `ServerTimeouts`, `ServerPolicy` | [Рецепт](#example-http-server) |
| [`http/sse`](../std/http/sse.ok) | Передавать клиенту события готовности заказа: ServerEvent и потоковые представления используют text/event-stream. | `EventStreamEnded`, `EventStreamBusy`, `EventStreamLimit`, `ServerEvent`, `ServerEventData` | [Рецепт](#example-http-sse) |
| [`http/static`](../std/http/static.ok) | Раздать документы из разрешённого каталога: StaticFiles использует FileRoot и явную StaticPolicy. | `StaticPolicy`, `StaticIndexes`, `StaticCoordinate`, `StaticPath`, `StaticFiles` | [Рецепт](#example-http-static) |
| [`http/websocket`](../std/http/websocket.ok) | Представить текстовое или двоичное сообщение чата: типы WebSocket фиксируют payload, close-коды и бюджеты. | `WebSocketMessageKind`, `WebSocketMessage`, `TextWebSocketMessage`, `BinaryWebSocketMessage`, `WebSocketLimits` | [Рецепт](#example-websocket-client) |
| [`http/websocket_client`](../std/http/websocket_client.ok) | Подключить клиента к каналу обновлений: WebSocketConnecting открывает HTTP/1 Upgrade с явной политикой. | `WebSocketPolicy`, `WebSocketConnecting`, `WebSocketOpening` | [Рецепт](#example-websocket-client) |
| [`http/websocket_handshake`](../std/http/websocket_handshake.ok) | Проверить WebSocket Upgrade: handshake-типы связывают nonce, accept и выбранные subprotocols. | `WebSocketAccept`, `WebSocketProtocols`, `WebSocketHandshakeFields`, `WebSocketClientHandshake`, `WebSocketServerHandshake` | [Рецепт](#recipe-web) |
| [`http/websocket_origin`](../std/http/websocket_origin.ok) | Разрешить подключение браузерного чата: SameOriginWebSockets сравнивает Origin с доверенным адресатом сервера. | `WebSocketAllowing`, `SameOriginWebSockets`, `WebSocketOriginCoordinates` | [Рецепт](#recipe-web) |
| [`http/websocket_server`](../std/http/websocket_server.ok) | Передать серверное соединение WebSocket-обработчику: WebSocketUpgrade удерживает политику Origin и subprotocols. | `WebSocketUpgrade`, `WebSocketAccepting`, `WebSocketServerOpening`, `WebSocketServerProtocol`, `WebSocketServerUnpublished` | [Рецепт](#recipe-web) |
| [`http/websocket_session`](../std/http/websocket_session.ok) | Читать и отправлять сообщения после Upgrade: WebSocketSession владеет транспортом и graceful close. | `WebSocketSession`, `WebSocketSessionState`, `WebSocketSessionFinish` | [Рецепт](#recipe-web) |
| [`web/app`](../std/web/app.ok) | Поднять небольшое HTTP-приложение: WebApplication связывает таблицу маршрутов с listener. | `WebApplication` | [Рецепт](#recipe-web) |
| [`web/exception`](../std/web/exception.ok) | Вернуть клиенту предметную HTTP-ошибку: Exception участвует и как Error, и как ответ endpoint. | `Exception` | [Рецепт](#recipe-web) |
| [`web/reply`](../std/web/reply.ok) | Вернуть текст, HTML, JSON или redirect: Text, Html, Json и другие ответы реализуют HandlingResult. | `Text`, `Html`, `Json`, `Created`, `Redirect` | [Рецепт](#recipe-web) |
| [`web/request`](../std/web/request.ok) | Получить id, query и JSON в endpoint: WebRequest предоставляет параметры и ограниченное чтение body. | `WebRequest`, `RequestJson` | [Рецепт](#recipe-web) |
| [`web/routing`](../std/web/routing.ok) | Привязать методы контроллера к маршрутам: Get, Post, Scope и WebRoutes связывают bound actions. | `ExceptionScope`, `WebRouting`, `Get`, `Post`, `Put` | [Рецепт](#recipe-web) |

### Что учитывать при сборке решения

- `http/static`: Корень FileRoot передаётся StaticFiles как capability; выбранный ответ удерживает собственный файловый дескриптор.
- `http/websocket_origin`: Доверенный адресат передаётся серверной конфигурацией; политика Origin применяется перед подтверждением Upgrade.

<a id="recipe-web"></a>

### 23.1. Поднять API проверки готовности

**Задача.** Сервис должен отвечать на /health и возвращать понятный ответ для остальных путей.

**Какие библиотеки использовать.** [`web/app`](../std/web/app.ok), [`web/routing`](../std/web/routing.ok), [`web/request`](../std/web/request.ok), [`web/reply`](../std/web/reply.ok), [`web/exception`](../std/web/exception.ok), [`http/handler`](../std/http/handler.ok).

**Как использовать.** Методы контроллера передаются как bound actions. WebRequest представляет параметры запроса, а Text и Exception уже являются HandlingResult. Запустите сервер и обратитесь к http://127.0.0.1:8080/health.

**Запуск.** Программа обслуживает порт до остановки; запросите указанный в коде маршрут из другого процесса.

```okkama
module cookbook/web v1.0.0;
import "web/app" take WebApplication;
import "web/routing" take WebRoutes,Get;
import "web/request" take WebRequest;
import "web/reply" take Text;
import "web/exception" take Exception;
import "http/handler" take HandlingResult;

object Health {
    Health();
    async ready(request is WebRequest) is HandlingResult { Text(200, "готов"); }
    async missing(request is WebRequest) is HandlingResult { Exception(404, "Маршрут не найден"); }
}

async main() is int {
    let endpoints = Health();
    let routes = WebRoutes([Get("/health", endpoints.ready)], endpoints.missing);
    await WebApplication(routes).listen("127.0.0.1", 8080);
    0;
}
```

<a id="example-http-static"></a>

### 23.2. Раздать документы из выбранного каталога

**Задача.** Раздать документы из разрешённого каталога: StaticFiles использует FileRoot и явную StaticPolicy.

**Какие библиотеки использовать.** [`fs/root`](../std/fs/root.ok), [`fs/directory`](../std/fs/directory.ok), [`fs/path`](../std/fs/path.ok), [`http/static`](../std/http/static.ok), [`mime/types`](../std/mime/types.ok), [`http/message`](../std/http/message.ok), [`http/method`](../std/http/method.ok), [`http/header`](../std/http/header.ok), [`http/target`](../std/http/target.ok), [`http/request_context`](../std/http/request_context.ok), [`http/exchange`](../std/http/exchange.ok), [`http/version`](../std/http/version.ok), [`net/stream`](../std/net/stream.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/ip`](../std/net/ip.ok), [`async/cancellation`](../std/async/cancellation.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Откройте FileRoot и передайте его StaticFiles вместе с policy и источником media types. Ответ удерживает выбранный файловый ресурс.

```okkama
module example v1.0.0;
import "fs/root" take FileRoot;
import "fs/directory" take Directory;
import "fs/path" take Path;
import "http/static" take StaticFiles,StaticPolicy;
import "mime/types" take StandardMediaTypes;
import "http/message" take Request;
import "http/method" take Method;
import "http/header" take Headers,HeaderName;
import "http/target" take ParsedRequestTarget;
import "http/request_context" take HttpRequestContext;
import "http/exchange" take ExchangeDetails;
import "http/version" take HttpVersion;
import "net/stream" take ConnectionEndpoints;
import "net/endpoint" take IpEndpoint;
import "net/ip" take IpAddress;
import "async/cancellation" take Cancellation;
import "io/stdout" take Stdout;
async main() is int{
    let root=FileRoot(Directory(Path(".")));defer root.close();
    let handler=StaticFiles(root,StandardMediaTypes(),StaticPolicy());
    let endpoint=IpEndpoint(IpAddress("127.0.0.1"),8080);
    let ends is ConnectionEndpoints<IpEndpoint>=ConnectionEndpoints(endpoint,endpoint);
    let context=HttpRequestContext(ExchangeDetails(ends,HttpVersion.http_1_1),Cancellation().cancelling());
    let method=Method("HEAD");let request=Request(method,ParsedRequestTarget("/README.md",method).value(),Headers([]));
    let response=(await handler.handling(request,context)).response();
    Stdout().writeln(response.status().code().string()+" "+response.headers().first(HeaderName("Content-Type")));0;
}
```

[Исходный пример](../examples/std-v4/http-static.ok).

<a id="example-http-sse"></a>

### 23.3. Передать событие готовности заказа

**Задача.** Передавать клиенту события готовности заказа: ServerEvent и потоковые представления используют text/event-stream.

**Какие библиотеки использовать.** [`http/sse`](../std/http/sse.ok), [`async/channel`](../std/async/channel.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** SSE-представления связывают data, id, тип события и retry. Поток читает и пишет события по установленному framing.

```okkama
module example v1.0.0;
import "http/sse" take ServerEvent,EventBody,ServerEvents,EventStreamEnded;
import "async/channel" take Channel;
import "io/stdout" take Stdout;
async main() is int{
    let queue is Channel<ServerEvent>=Channel(2);
    await queue.sending().send(ServerEvent("alive",true));
    await queue.sending().send(ServerEvent("customer updated"));queue.sending().close();
    let events=ServerEvents(EventBody(queue.receiving()).reading());defer events.close();
    let event=await events.event();Stdout().writeln(event.event()+": "+event.data());
    try{await events.event();}catch e is EventStreamEnded{}0;
}
```

[Исходный пример](../examples/std-v4/http-sse.ok).

<a id="example-http-routing"></a>

### 23.4. Выбрать endpoint по сегментам пути

**Задача.** Извлечь параметр /orders/:id: маршрутизация декодирует сегменты и удерживает именованные параметры.

**Какие библиотеки использовать.** [`http/routing`](../std/http/routing.ok), [`http/handler`](../std/http/handler.ok), [`http/request_context`](../std/http/request_context.ok), [`http/message`](../std/http/message.ok), [`http/method`](../std/http/method.ok), [`http/status`](../std/http/status.ok), [`http/header`](../std/http/header.ok), [`http/target`](../std/http/target.ok), [`http/version`](../std/http/version.ok), [`http/body`](../std/http/body.ok), [`http/exchange`](../std/http/exchange.ok), [`net/stream`](../std/net/stream.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/ip`](../std/net/ip.ok), [`async/cancellation`](../std/async/cancellation.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Маршрут задаёт литералы и параметры. Полученные именованные сегменты передаются в HttpRequestContext выбранного обработчика.

```okkama
module app v1.0.0;
import "http/routing" take Routes,Route,RoutePattern;
import "http/handler" take Handling,HandlingResult,ResponseHandling;
import "http/request_context" take HttpRequestContext;
import "http/message" take Request,Response;
import "http/method" take Method;
import "http/status" take Status;
import "http/header" take Headers,Header,HeaderName;
import "http/target" take ParsedRequestTarget;
import "http/version" take HttpVersion;
import "http/body" take EmptyBody;
import "http/exchange" take ExchangeDetails;
import "net/stream" take ConnectionEndpoints;
import "net/endpoint" take IpEndpoint;
import "net/ip" take IpAddress;
import "async/cancellation" take Cancellation;
import "io/stdout" take Stdout;

object CustomerPage does Handling{
    CustomerPage();

    async handling(request is Request, context is HttpRequestContext) is HandlingResult {
        ResponseHandling(
            Response(
                Status(200),
                Headers(
                    [
                        Header(
                            HeaderName("X-Customer"),
                            context.parameters()["id"]
                        )
                    ]
                )
            )
        );
    }
}

object MissingPage does Handling {
    MissingPage();

    async handling(request is Request, context is HttpRequestContext) is HandlingResult {
        ResponseHandling(
            Response(Status(404), Headers([]))
        );
    }
}
async main() is int{
    let routes = Routes(
        [
            Route(
                Method("GET"),
                RoutePattern("/customer/:id"),
                CustomerPage()
            )
        ],
        MissingPage()
    );
    let endpoint = IpEndpoint(
        IpAddress("127.0.0.1"),
        8080
    );
    let endpoints is ConnectionEndpoints<IpEndpoint> = ConnectionEndpoints(endpoint, endpoint);
    let context = HttpRequestContext(
        ExchangeDetails(
            endpoints,
            HttpVersion.http_1_1
        ),
        Cancellation().cancelling()
    );
    let method = Method("HEAD");
    let request = Request(
        method,
        ParsedRequestTarget("/customer/a%2Fb",method).value(),
        Headers([]),
        EmptyBody()
    );
    let result = await routes.handling(request,context);
    Stdout().writeln(
        result.response().status().code().string()
    );

    Stdout().writeln(result.response().headers().first(HeaderName("X-Customer")));

    0;
}
```

[Исходный пример](../examples/std-v4/http-routing.ok).

<a id="example-http-server"></a>

### 23.5. Завершить HTTP-сервер после обслуженного запроса

**Задача.** Обслужить API на слушающем TCP-порту: HttpServer владеет listener, вызовами обработчика и завершением работы.

**Какие библиотеки использовать.** [`http/server`](../std/http/server.ok), [`http/server_policy`](../std/http/server_policy.ok), [`http/handler`](../std/http/handler.ok), [`http/request_context`](../std/http/request_context.ok), [`http/message`](../std/http/message.ok), [`http/status`](../std/http/status.ok), [`http/header`](../std/http/header.ok), [`net/tcp`](../std/net/tcp.ok), [`net/endpoint`](../std/net/endpoint.ok), [`net/ip`](../std/net/ip.ok), [`async/event`](../std/async/event.ok), [`async/scope`](../std/async/scope.ok), [`time/clock`](../std/time/clock.ok), [`time/duration`](../std/time/duration.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Listener принадлежит HttpServer. Пример запускает serve в группе, затем ServerDrain завершает начатую обработку и освобождает транспорт.

```okkama
module app v1.0.0;
import "http/server" take HttpServer,ServerDrain;
import "http/server_policy" take ServerPolicy;
import "http/handler" take Handling,HandlingResult,ResponseHandling;
import "http/request_context" take HttpRequestContext;
import "http/message" take Request,Response;
import "http/status" take Status;
import "http/header" take Headers,Header,HeaderName;
import "net/tcp" take TcpListening;
import "net/endpoint" take IpEndpoint;
import "net/ip" take IpAddress;
import "async/event" take Event;
import "async/scope" take FutureGroup;
import "time/clock" take SystemClock;
import "time/duration" take Duration,DurationUnit;
import "io/stdout" take Stdout;
object Application does Handling {
    has entered is Event;

    Application(entered is Event) {
        self.entered = entered;
    }

    async handling(request is Request, context is HttpRequestContext) is HandlingResult {
        entered.set();

        ResponseHandling(
            Response(
                Status(204),
                Headers(
                    [
                        Header(
                            HeaderName("X-Peer"),
                            context.endpoints().remote_endpoint().address().string()
                        )
                    ]
                )
            )
        );
    }
}
async main() is int{
    let listener = await TcpListening(IpEndpoint(IpAddress("127.0.0.1"), 0)).acceptance();
    let entered = Event();
    let clock = SystemClock();
    let server = HttpServer(
        listener,
        Application(entered),
        ServerPolicy(),
        clock
    );
    let workers is FutureGroup<HttpServer> = FutureGroup();
    workers.start(server.serve());
    Stdout().writeln(server.local_endpoint().port().string());
    await entered.wait();
    await ServerDrain(
        server,
        Duration(5.rat(), DurationUnit.second),
        clock
    ).drain();
    await workers.wait();
    Stdout().writeln("HTTP server drained");

    0;
}
```

[Исходный пример](../examples/std-v4/http-server.ok).

<a id="example-websocket-client"></a>

### 23.6. Получать события через WebSocket

**Задача.** Подключить клиента к каналу обновлений: WebSocketConnecting открывает HTTP/1 Upgrade с явной политикой.

**Какие библиотеки использовать.** [`http/websocket_client`](../std/http/websocket_client.ok), [`http/websocket`](../std/http/websocket.ok), [`http/header`](../std/http/header.ok), [`http/endpoint`](../std/http/endpoint.ok), [`url/value`](../std/url/value.ok), [`net/dns`](../std/net/dns.ok), [`net/tls`](../std/net/tls.ok), [`crypto/x509/trust`](../std/crypto/x509/trust.ok), [`os/environment`](../std/os/environment.ok), [`time/clock`](../std/time/clock.ok), [`time/duration`](../std/time/duration.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** WebSocketConnecting получает URL, policy, resolver и TLS. После Upgrade читайте message, отправляйте типизированное сообщение и завершайте close-handshake.

**Входы примера.** Передайте через окружение `OKKAMA_WEBSOCKET_URL`. Адреса и файлы задают подготовленный стенд; проверяемые ответы видны в `confirm`.

```okkama
module app v1.0.0;
import "http/websocket_client" take WebSocketConnecting,WebSocketPolicy;
import "http/websocket" take WebSocketLimits,TextWebSocketMessage;
import "http/header" take Headers;
import "http/endpoint" take AllEndpoints;
import "url/value" take ParsedUrl;
import "net/dns" take FixedResolution;
import "net/tls" take TlsClientPolicy;
import "crypto/x509/trust" take TrustedCertificates;
import "os/environment" take Environment;
import "time/clock" take SystemClock;
import "time/duration" take Duration,DurationUnit;
import "io/stdout" take Stdout;
async main() is int{
    let url=ParsedUrl(Environment().variable("OKKAMA_WEBSOCKET_URL")).value();let clock=SystemClock();
    let policy=WebSocketPolicy(WebSocketLimits(),Duration(10.rat(),DurationUnit.second),Duration(2.rat(),DurationUnit.second),AllEndpoints());
    let connecting=WebSocketConnecting(url,[],Headers([]),FixedResolution([]),TlsClientPolicy(TrustedCertificates([])),policy,clock);
    let connection=await connecting.connection();let message=await connection.message();confirm message.text()=="Hi";
    await connection.send(TextWebSocketMessage("bye"));await connection.close(1000,"done");confirm connection.closed();Stdout().writeln("WebSocket passed");0;
}
```

[Исходный пример](../examples/std-v4/websocket-client.ok).

<a id="chapter-mail"></a>

## 24. Почта и MIME

Письмо состоит из заголовков и MIME-частей; транспортная сессия и локальное хранилище живут отдельно. Для IMAP удерживайте UID вместе с областью аккаунта, папкой и UIDVALIDITY. Рецепты protocol session используют явно подготовленный транспорт.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`mail/imap`](../std/mail/imap.ok) | Узнать возможности аутентифицированного IMAP-сервера: ImapCapabilities удерживает CAPABILITY-рекламу. | `ImapFeature`, `ImapCapabilityLimits`, `ImapCapability`, `ImapCapabilities`, `ParsedImapCapabilities` | [Рецепт](#example-imap-capabilities) |
| [`mail/imap_changes`](../std/mail/imap_changes.ok) | Сопоставить UID после COPY или APPEND: результаты UIDPLUS сохраняют соответствие исходных и новых идентификаторов. | `ImapAssignedUids`, `ImapCopiedUids`, `ImapUidPlus`, `CopiedMail`, `MovedMail` | [Рецепт](#recipe-imap-assignment) |
| [`mail/imap_command`](../std/mail/imap_command.ok) | Сформировать UID FETCH, STORE либо APPEND: команды связывают предметные параметры с wire-представлением. | `ImapSimpleCommand`, `ImapMailboxCommand`, `ImapDatum`, `FlagChange`, `ImapCommand` | [Рецепт](#example-imap-session) |
| [`mail/imap_command_reading`](../std/mail/imap_command_reading.ok) | Получить завершение конкретной IMAP-команды: чтение ожидает нужный tag и собирает unsolicited-наблюдения. | `ImapContinuationWriting`, `ImapResponseObserving`, `ImapNoObservation`, `ImapReplyMode`, `ImapNoContinuation` | [Рецепт](#example-imap-session) |
| [`mail/imap_data`](../std/mail/imap_data.ok) | Разобрать вложенные данные untagged-ответа: ParsedImapData сохраняет списки, токены, NIL и literal-байты. | `ImapValueKind`, `ImapTokenKind`, `ImapDataLimits`, `ImapValue`, `ParsedImapData` | [Рецепт](#recipe-imap-assignment) |
| [`mail/imap_exchange`](../std/mail/imap_exchange.ok) | Сериализовать IMAP-обмен и продолжения literal: ImapCommandChannel владеет транспортом и назначением тегов. | `ImapCommandChannel`, `ImapCommandExchange`, `ImapExchangeState`, `ImapExchangeCleanup`, `ImapLiteralWriting` | [Рецепт](#example-imap-session) |
| [`mail/imap_message`](../std/mail/imap_message.ok) | Прочитать FETCH-наблюдение письма: ImapMessage различает наличие UID, заголовков и body. | `ImapFetchAttribute`, `ImapFetch`, `ImapMessage` | [Рецепт](#recipe-imap-assignment) |
| [`mail/imap_metadata`](../std/mail/imap_metadata.ok) | Прочитать FLAGS, STATUS и UID-диапазоны: предметные представления проверяют форму protocol metadata. | `ImapStatusCode`, `ImapMailbox`, `ImapUnsigned`, `ImapFlagsValue`, `ImapUidRanges` | [Рецепт](#recipe-imap-assignment) |
| [`mail/imap_name`](../std/mail/imap_name.ok) | Назвать IMAP-папку с кириллицей: ImapMailboxName и modified UTF-7 кодеки задают wire-имя. | `ImapNameLimits`, `ImapUtf7Encoded`, `ImapUtf7Decoded`, `ImapMailboxName`, `ImapUtf7Encoder` | [Рецепт](#example-imap-names) |
| [`mail/imap_reading`](../std/mail/imap_reading.ok) | Прочитать IMAP-ответ из асинхронного потока: ImapResponseReading удерживает точный остаток. | `ImapResponseReading`, `ImapResponseRead`, `ImapResponseInput` | [Рецепт](#example-imap-reading) |
| [`mail/imap_response`](../std/mail/imap_response.ok) | Отделить один IMAP-ответ с literal-данными: response-представления проверяют теги, части и длины. | `ImapResponseKind`, `ImapResponsePartKind`, `ImapResponseLimits`, `ImapResponsePart`, `ImapResponse` | [Рецепт](#example-imap-responses) |
| [`mail/imap_search`](../std/mail/imap_search.ok) | Найти письма по дате и флагам: MailMatching и типизированные критерии строят проверенный search key. | `MailMatching`, `ImapSearchAtom`, `ImapSearchOperator`, `ImapSearchLimits`, `ImapSearchKey` | [Рецепт](#example-imap-session) |
| [`mail/imap_session`](../std/mail/imap_session.ok) | Выбрать папку и получить письма: ImapSession, SearchedMail и FetchedMail используют выбранный mailbox. | `ImapSession`, `SelectedMailbox`, `ImapSelectionToken`, `ImapSelectedState`, `ImapSessionState` | [Рецепт](#example-imap-session) |
| [`mail/imap_state`](../std/mail/imap_state.ok) | Обновить наблюдение выбранной папки после EXPUNGE: ImapMailboxState сохраняет согласованные счётчики и флаги. | `ImapMailboxState`, `ImapSelectionProgress` | [Рецепт](#recipe-imap-assignment) |
| [`mail/imap_uid`](../std/mail/imap_uid.ok) | Сохранить ссылку на письмо между операциями: ImapUid и ImapUidValidity отличают UID от sequence number. | `ImapUid`, `ImapUidValidity`, `ImapSequence`, `ImapUidSetLimits`, `ImapUidSet` | [Рецепт](#recipe-imap-assignment) |
| [`mail/mailbox`](../std/mail/mailbox.ok) | Сохранить письмо и флаги в Maildir: Maildir и представления флагов связывают сообщения с локальным хранилищем. | `MailFlagKind`, `SystemMailFlag`, `MailFlag`, `SystemFlag`, `KeywordFlag` | [Рецепт](#example-mail-flags) |
| [`mail/mbox`](../std/mail/mbox.ok) | Сформировать экспорт почтового архива: MboxRecord и MboxArchive представляют mboxrd-данные. | `MboxSeparator`, `MboxRecord`, `MboxLimits`, `MboxArchive`, `ParsedMbox` | [Рецепт](#example-mbox) |
| [`mail/mbox_store`](../std/mail/mbox_store.ok) | Редактировать локальный mbox как сессию: MboxSession удерживает блокировку и явно фиксирует результат. | `Mbox`, `MboxSession`, `MboxConflict`, `MboxLease`, `MboxFileBytes` | [Рецепт](#example-mbox-session) |
| [`mail/message`](../std/mail/message.ok) | Подготовить MIME-письмо к доставке: MailMessage и DeliveredMail представляют сообщение и явную подготовку заголовков. | `MailMessage`, `ParsedMail`, `DeliveredMail`, `MessageId`, `MailDate` | [Рецепт](#example-mail-message) |
| [`mail/pop3`](../std/mail/pop3.ok) | Сопоставить LIST и UIDL: PopMessage и связанные типы различают позицию сообщения и идентификатор maildrop. | `PopFeature`, `PopNumber`, `PopIdentifier`, `PopListingLimits`, `PopMessage` | [Рецепт](#example-pop3-session) |
| [`mail/pop3_reading`](../std/mail/pop3_reading.ok) | Дочитать один POP3-ответ из потока: PopReplyReading сохраняет байты следующего ответа. | `PopReplyReading`, `PopReplyRead`, `PopReplyInput` | [Рецепт](#example-pop3-replies) |
| [`mail/pop3_reply`](../std/mail/pop3_reply.ok) | Разобрать POP3 status и multiline body: ParsedPopReply снимает dot-stuffing по выбранному режиму. | `PopReplyMode`, `PopReplyLimits`, `PopReply`, `PopReplyDecoder`, `ParsedPopReply` | [Рецепт](#example-pop3-replies) |
| [`mail/pop3_session`](../std/mail/pop3_session.ok) | Прочитать почту в аутентифицированной POP3-сессии: PopSession сериализует команды и управляет delete/reset. | `PopSession`, `PopMailbox`, `PreviewedPopMail`, `PopDeleteReset`, `PopSimpleCommand` | [Рецепт](#example-pop3-session) |
| [`mail/smtp`](../std/mail/smtp.ok) | Разобрать EHLO сервера перед отправкой письма: SmtpCapabilities представляет объявленные расширения. | `SmtpFeature`, `SmtpExtension`, `SmtpCapabilities`, `SmtpKnownFeature`, `SmtpSizeNumber` | [Рецепт](#example-smtp-wire) |
| [`mail/smtp_data`](../std/mail/smtp_data.ok) | Подготовить сообщение к SMTP DATA: PreparedSmtpData проверяет формат, dot-stuffing и advertised-возможности. | `SmtpBodyMode`, `SmtpDataLimits`, `PreparedSmtpData`, `SmtpDataValue`, `SmtpDataMime` | [Рецепт](#example-smtp-wire) |
| [`mail/smtp_reading`](../std/mail/smtp_reading.ok) | Дочитать один SMTP-ответ из транспорта: SmtpReplyReading возвращает ответ и точный remainder. | `SmtpReplyReading`, `SmtpReplyRead`, `SmtpReplyInput` | [Рецепт](#example-smtp-reading) |
| [`mail/smtp_reply`](../std/mail/smtp_reply.ok) | Прочитать SMTP-код и многострочный ответ: ParsedSmtpReply и SmtpReplyDecoder сохраняют рамки ответа. | `SmtpReplyLimits`, `SmtpReply`, `ParsedSmtpReply`, `SmtpReplyDecoder`, `SmtpReplyValue` | [Рецепт](#example-smtp-wire) |
| [`message/header`](../std/message/header.ok) | Собрать заголовки письма без потери повторов: MessageHeaders и RenderedMessageHeaders работают с mail-грамматикой. | `MessageHeaderName`, `MessageHeader`, `MessageHeaders`, `RenderedMessageHeaders`, `UnfoldedMessageHeader` | [Рецепт](#example-mime-word) |
| [`mime/disposition`](../std/mime/disposition.ok) | Указать имя скачиваемого файла: ContentDisposition представляет inline либо attachment с параметрами. | `ContentDisposition` | [Рецепт](#example-mime-disposition) |
| [`mime/media_type`](../std/mime/media_type.ok) | Разобрать тип вложения с MIME-параметрами: MediaType сохраняет расширенные имена и значения параметров. | `MimeParameter`, `MimeParameters`, `MediaType`, `MediaTypeLimits`, `MimeCharacter` | [Рецепт](#example-mime-disposition) |
| [`mime/part`](../std/mime/part.ok) | Собрать письмо с текстом и вложениями: MimePart, TextBody и BinaryBody задают MIME-структуру. | `MimeBody`, `BinaryBody`, `TextBody`, `MimePart`, `MultipartBody` | [Рецепт](#example-mime-part) |
| [`mime/transfer`](../std/mime/transfer.ok) | Выбрать кодирование тела письма: TransferEncodedBody и TransferDecodedBody связывают bytes с Content-Transfer-Encoding. | `TransferEncoding`, `TransferEncodedBody`, `TransferDecodedBody`, `TransferPosition`, `TransferProfile` | [Рецепт](#example-mime-transfer) |
| [`mime/types`](../std/mime/types.ok) | Определить тип файла по расширению: StandardMediaTypes, MappedMediaTypes и SystemMediaTypes дают явный источник соответствий. | `MediaIdentifying`, `MediaTypeExtension`, `MappedMediaTypes`, `StandardMediaTypes`, `SystemMediaTypes` | [Рецепт](#example-mime-types) |
| [`mime/word`](../std/mime/word.ok) | Закодировать тему с кириллицей: EncodedWord и DecodedWords работают с encoded-word заголовков. | `EncodedWordContext`, `EncodedWord`, `DecodedWords`, `WordPrevious`, `WordCharset` | [Рецепт](#example-mime-word) |

<a id="recipe-imap-assignment"></a>

### 24.1. Проверить наличие назначенных IMAP UID

**Задача.** После APPEND либо COPY приложение сохраняет полученные идентификаторы, когда сервер их сообщил.

**Какие библиотеки использовать.** [`mail/imap_changes`](../std/mail/imap_changes.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** UIDPLUS-результаты отделяют успешную операцию от наличия отображения UID. Когда assignment.known() истинно, используйте uids() и связанную UIDVALIDITY для последующих действий.

```okkama
module cookbook/imap_assignment v1.0.0;
import "mail/imap_changes" take ImapAssignedUids;
import "io/stdout" take Stdout;

main() is int {
    let assignment = ImapAssignedUids();
    if assignment.known() {
        Stdout().writeln("Сервер сообщил назначенные UID");
    } else {
        Stdout().writeln("Сохранить результат операции без назначения UID");
    }
    0;
}
```

<a id="example-pop3-session"></a>

### 24.2. Прочитать почтовый ящик через POP3

**Задача.** Сопоставить LIST и UIDL: PopMessage и связанные типы различают позицию сообщения и идентификатор maildrop.

**Какие библиотеки использовать.** [`mail/pop3`](../std/mail/pop3.ok), [`mail/pop3_reply`](../std/mail/pop3_reply.ok), [`mail/pop3_session`](../std/mail/pop3_session.ok), [`net/stream`](../std/net/stream.ok), [`io/stream`](../std/io/stream.ok), [`io/buffer`](../std/io/buffer.ok), [`io/stdout`](../std/io/stdout.ok), [`encoding/utf`](../std/encoding/utf.ok), [`async/event`](../std/async/event.ok).

**Как использовать.** Пример передаёт готовый authenticated transport в PopSession. Mailbox-операции связывают LIST, UIDL и чтение сообщения.

```okkama
module app v1.0.0;
import "mail/pop3" take PopCapabilities, PopListingLimits;
import "mail/pop3_reply" take ParsedPopReply, PopReplyMode, PopReplyLimits;
import "mail/pop3_session" take PopSession, PopDeleteReset;
import "net/stream" take AsyncReading, AsyncWriting;
import "io/stream" take Reading, ByteSource, Closing;
import "io/buffer" take ByteBuffer;
import "io/stdout" take Stdout;
import "encoding/utf" take Utf8Encoded, Utf8Decoded;
import "async/event" take Event;

object Input does AsyncReading {
    has origin is Reading;Input(origin is Reading){self.origin=origin;}
    async bytes(limit is int) is @byte {origin.bytes(if limit<3{limit;}else{3;});}
}
object Output does AsyncWriting {
    has origin is ByteBuffer;Output(origin is ByteBuffer){self.origin=origin;}
    async write(source is ByteSource) is AsyncWriting {origin.writing().write(source);self;}
}
object Owner does Closing {
    has stopped is Event;Owner(){stopped=Event();}
    closed() is bool {stopped.ready();}close() is void {stopped.set();}
}
async main() is int {
    let input=ByteBuffer(Utf8Encoded("+OK 1 20\r\n+OK\r\n1 20\r\n.\r\n+OK\r\n1 stable-id\r\n.\r\n+OK marked\r\n+OK reset\r\n+OK done\r\n").bytes());
    let output=ByteBuffer();defer input.closing().close();defer output.closing().close();
    let caps=PopCapabilities(ParsedPopReply(Utf8Encoded("+OK\r\nUIDL\r\n.\r\n").bytes(),PopReplyMode.multiline).reply());
    let session=PopSession(Input(input.reading()),Output(output),Owner(),caps,[],PopReplyLimits(),PopListingLimits());defer session.close();
    let mailbox=session.mailbox();let state=await mailbox.state();let messages=await mailbox.messages();
    Stdout().writeln("messages: "+state.count().string()+", bytes: "+state.size().string());
    Stdout().writeln("identifier: "+messages.item(0).identifier().string());
    await mailbox.delete(messages.item(0).number());await PopDeleteReset(session).reset();await session.quit();
    Stdout().write(Utf8Decoded(output.content().bytes()).string());0;
}
```

[Исходный пример](../examples/std-v4/pop3-session.ok).

<a id="example-mime-transfer"></a>

### 24.3. Закодировать вложение для письма

**Задача.** Выбрать кодирование тела письма: TransferEncodedBody и TransferDecodedBody связывают bytes с Content-Transfer-Encoding.

**Какие библиотеки использовать.** [`mime/transfer`](../std/mime/transfer.ok), [`io/decorators`](../std/io/decorators.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** TransferEncodedBody получает MIME-body и выбранную TransferEncoding. Обратное представление восстанавливает исходные байты.

```okkama
module app v1.0.0;
import "mime/transfer" take TransferEncoding, TransferEncodedBody, TransferDecodedBody;
import "io/decorators" take ByteSnapshot;
import "encoding/utf" take Utf8Encoded, Utf8Decoded;
import "io/stdout" take Stdout;
main() is int {
    let source=ByteSnapshot(Utf8Encoded("Привет, MIME!\r\n").bytes());
    let encoded=TransferEncodedBody(source,TransferEncoding.base64);
    Stdout().writeln(Utf8Decoded(encoded.bytes()).string());
    Stdout().writeln(Utf8Decoded(TransferDecodedBody(encoded,TransferEncoding.base64).bytes()).string());
    0;
}
```

[Исходный пример](../examples/std-v4/mime-transfer.ok).

<a id="example-pop3-replies"></a>

### 24.4. Разобрать POP3 status и тело ответа

**Задача.** Дочитать один POP3-ответ из потока: PopReplyReading сохраняет байты следующего ответа.

**Какие библиотеки использовать.** [`mail/pop3_reading`](../std/mail/pop3_reading.ok), [`mail/pop3_reply`](../std/mail/pop3_reply.ok), [`net/stream`](../std/net/stream.ok), [`io/stream`](../std/io/stream.ok), [`io/buffer`](../std/io/buffer.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Выберите reply mode по отправленной команде. Decoder сохраняет границы multiline и снимает dot-stuffing.

```okkama
module app v1.0.0;
import "mail/pop3_reading" take PopReplyReading;
import "mail/pop3_reply" take PopReplyMode;
import "net/stream" take AsyncReading;
import "io/stream" take Reading;
import "io/buffer" take ByteBuffer;
import "encoding/utf" take Utf8Encoded, Utf8Decoded;
import "io/stdout" take Stdout;

object Fragments does AsyncReading {
    has origin is Reading;Fragments(origin is Reading) {self.origin=origin;}
    async bytes(limit is int) is @byte {origin.bytes(if limit<2 {limit;} else {2;});}
}
async main() is int {
    let buffer=ByteBuffer(Utf8Encoded("+OK message follows\r\nSubject: example\r\n\r\n..body\r\n.\r\n-ERR unavailable\r\n").bytes());
    defer buffer.closing().close();let input=Fragments(buffer.reading());
    let first=await PopReplyReading(input,PopReplyMode.multiline).result();
    let second=await PopReplyReading(input,PopReplyMode.single,first.remainder()).result();
    Stdout().writeln(first.reply().text());Stdout().writeln(Utf8Decoded(first.reply().body()).string());
    Stdout().writeln(second.reply().text());0;
}
```

[Исходный пример](../examples/std-v4/pop3-replies.ok).

<a id="example-imap-capabilities"></a>

### 24.5. Узнать расширения IMAP сервера

**Задача.** Узнать возможности аутентифицированного IMAP-сервера: ImapCapabilities удерживает CAPABILITY-рекламу.

**Какие библиотеки использовать.** [`mail/imap`](../std/mail/imap.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ImapCapabilities сохраняет объявленные токены. Выбирайте supported feature и механизм AUTH по этим наблюдениям.

```okkama
module app v1.0.0;
import "mail/imap" take ParsedImapCapabilities, ImapFeature;
import "encoding/utf" take Utf8Encoded;
import "io/stdout" take Stdout;

main() is int {
    let caps=ParsedImapCapabilities(Utf8Encoded("* CAPABILITY IMAP4rev1 STARTTLS AUTH=PLAIN IDLE X-Local=Value\r\n").bytes()).capabilities();
    Stdout().writeln(caps.authentication().first());
    Stdout().writeln(caps.tokens().last().string());
    Stdout().writeln(if caps.features().membership(ImapFeature.start_tls) {"STARTTLS advertised";} else {"STARTTLS absent";});
    0;
}
```

[Исходный пример](../examples/std-v4/imap-capabilities.ok).

<a id="example-imap-session"></a>

### 24.6. Выбрать папку и прочитать тело письма по UID

**Задача.** Выбрать папку и получить письма: ImapSession, SearchedMail и FetchedMail используют выбранный mailbox.

**Какие библиотеки использовать.** [`mail/imap_session`](../std/mail/imap_session.ok), [`mail/imap_exchange`](../std/mail/imap_exchange.ok), [`mail/imap_command_reading`](../std/mail/imap_command_reading.ok), [`mail/imap_command`](../std/mail/imap_command.ok), [`mail/imap`](../std/mail/imap.ok), [`mail/imap_name`](../std/mail/imap_name.ok), [`mail/imap_search`](../std/mail/imap_search.ok), [`net/stream`](../std/net/stream.ok), [`io/stream`](../std/io/stream.ok), [`io/buffer`](../std/io/buffer.ok), [`io/stdout`](../std/io/stdout.ok), [`encoding/utf`](../std/encoding/utf.ok), [`async/event`](../std/async/event.ok).

**Как использовать.** Готовый transcript передаётся в ImapCommandChannel и ImapSession. Затем mailbox, SearchedMail и FetchedMail работают внутри выбранной папки.

```okkama
module app v1.0.0;
import "mail/imap_session" take ImapSession, SearchedMail, FetchedMail;
import "mail/imap_exchange" take ImapCommandChannel;
import "mail/imap_command_reading" take ImapCommandLimits;
import "mail/imap_command" take ImapDatum;
import "mail/imap" take ImapCapabilities;
import "mail/imap_name" take ImapMailboxName;
import "mail/imap_search" take AllMail;
import "net/stream" take AsyncReading, AsyncWriting;
import "io/stream" take Reading, ByteSource, Closing;
import "io/buffer" take ByteBuffer;
import "io/stdout" take Stdout;
import "encoding/utf" take Utf8Encoded, Utf8Decoded;
import "async/event" take Event;

object Transcript does AsyncReading, AsyncWriting, Closing {
    has input is Reading;has output is ByteBuffer;has stopped is Event;
    Transcript(input is Reading, output is ByteBuffer) {self.input=input;self.output=output;stopped=Event();}
    async bytes(limit is int) is @byte {input.bytes(if limit<3 {limit;} else {3;});}
    async write(source is ByteSource) is AsyncWriting {output.writing().write(source);self;}
    closed() is bool {stopped.ready();}
    close() is void {stopped.set();}
}
async main() is int {
    let input=ByteBuffer(Utf8Encoded("* FLAGS ()\r\n* 1 EXISTS\r\n* OK [UIDVALIDITY 7] epoch\r\nO1 OK selected\r\n* SEARCH 42\r\nO2 OK found\r\n* 1 FETCH (UID 42 BODY[TEXT] {5}\r\nHello)\r\nO3 OK fetched\r\n").bytes());
    let output=ByteBuffer();defer input.closing().close();defer output.closing().close();
    let transcript=Transcript(input.reading(),output);
    let channel=ImapCommandChannel(transcript,transcript,transcript,[],ImapCommandLimits());
    let session=ImapSession(channel,ImapCapabilities(["IMAP4rev1"]),"example-account");defer session.close();
    let selected=await session.mailbox(ImapMailboxName("INBOX"));
    let ids=await SearchedMail(selected,AllMail()).uids();
    let messages=await FetchedMail(selected,ids,[ImapDatum.body]).messages();
    Stdout().writeln("UID "+messages.item(0).uid().string()+": "+Utf8Decoded(messages.item(0).body()).string());
    Stdout().writeln(Utf8Decoded(output.content().bytes()).string());
    0;
}
```

[Исходный пример](../examples/std-v4/imap-session.ok).

<a id="example-imap-names"></a>

### 24.7. Сформировать имя папки с Unicode

**Задача.** Назвать IMAP-папку с кириллицей: ImapMailboxName и modified UTF-7 кодеки задают wire-имя.

**Какие библиотеки использовать.** [`mail/imap_name`](../std/mail/imap_name.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ImapMailboxName представляет имя папки, а modified UTF-7 кодеки задают его IMAP4rev1-представление.

```okkama
module app v1.0.0;
import "mail/imap_name" take ImapMailboxName, ImapUtf7Decoded;
import "io/stdout" take Stdout;

main() is int {
    let name=ImapMailboxName("~peter/mail/台北/日本語");
    Stdout().writeln(name.wire_name());
    Stdout().writeln(name.quoted());
    Stdout().writeln(ImapUtf7Decoded(name.wire_name()).string());
    0;
}
```

[Исходный пример](../examples/std-v4/imap-names.ok).

<a id="example-imap-reading"></a>

### 24.8. Прочитать ответ IMAP из небольших chunks

**Задача.** Прочитать IMAP-ответ из асинхронного потока: ImapResponseReading удерживает точный остаток.

**Какие библиотеки использовать.** [`mail/imap_reading`](../std/mail/imap_reading.ok), [`net/stream`](../std/net/stream.ok), [`io/stream`](../std/io/stream.ok), [`io/buffer`](../std/io/buffer.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ImapResponseReading получает borrowed AsyncReading и остаток предыдущего ответа. result и remainder разделяют текущую запись с последующими байтами.

```okkama
module app v1.0.0;
import "mail/imap_reading" take ImapResponseReading;
import "net/stream" take AsyncReading;
import "io/stream" take Reading;
import "io/buffer" take ByteBuffer;
import "encoding/utf" take Utf8Encoded;
import "io/stdout" take Stdout;

object Fragments does AsyncReading {
    has origin is Reading;
    Fragments(origin is Reading) {self.origin=origin;}
    async bytes(limit is int) is @byte {origin.bytes(if limit<3 {limit;} else {3;});}
}
async main() is int {
    let buffer=ByteBuffer(Utf8Encoded("* 1 FETCH (BODY[] {5}\r\na\r\nbc)\r\nA1 OK done\r\n").bytes());
    defer buffer.closing().close();let input=Fragments(buffer.reading());
    let first=await ImapResponseReading(input).result();
    let second=await ImapResponseReading(input,first.remainder()).result();
    Stdout().writeln(first.response().name());
    Stdout().writeln(first.response().parts().item(1).bytes().size().string());
    Stdout().writeln(second.response().tag()+" "+second.response().name());
    0;
}
```

[Исходный пример](../examples/std-v4/imap-reading.ok).

<a id="example-imap-responses"></a>

### 24.9. Разобрать tagged, untagged и literal-ответы

**Задача.** Отделить один IMAP-ответ с literal-данными: response-представления проверяют теги, части и длины.

**Какие библиотеки использовать.** [`mail/imap_response`](../std/mail/imap_response.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Response-представления выделяют статус и части ответа. Literal payload сохраняется как точная байтовая последовательность.

```okkama
module app v1.0.0;
import "mail/imap_response" take ImapResponseDecoder;
import "encoding/utf" take Utf8Encoded, Utf8Decoded;
import "io/stdout" take Stdout;

main() is int {
    let decoder=ImapResponseDecoder();
    decoder=decoder.fed(Utf8Encoded("* 7 FETCH (BODY[] {5}\r\nA\r").bytes());
    decoder=decoder.fed(Utf8Encoded("\nBC)\r\nA1 OK complete\r\n").bytes());
    let response=decoder.finish();
    Stdout().writeln(response.name());
    Stdout().writeln(response.parts().item(1).bytes().size().string());
    Stdout().write(Utf8Decoded(decoder.remainder()).string());
    0;
}
```

[Исходный пример](../examples/std-v4/imap-responses.ok).

<a id="example-mail-flags"></a>

### 24.10. Сохранить Seen и предметные флаги письма

**Задача.** Сохранить письмо и флаги в Maildir: Maildir и представления флагов связывают сообщения с локальным хранилищем.

**Какие библиотеки использовать.** [`mail/mailbox`](../std/mail/mailbox.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Типы MailFlag отличают системный флаг от keyword. Снимок флагов участвует в mailbox-операции как явное значение.

```okkama
module app v1.0.0;
import "mail/mailbox" take SystemFlag, SystemMailFlag, KeywordFlag;
import "io/stdout" take Stdout;
main() is int {
    Stdout().writeln(SystemFlag(SystemMailFlag.seen).string());
    Stdout().writeln(KeywordFlag("$CustomLabel").keyword());
    0;
}
```

[Исходный пример](../examples/std-v4/mail-flags.ok).

<a id="example-mbox"></a>

### 24.11. Собрать почтовый экспорт mboxrd

**Задача.** Сформировать экспорт почтового архива: MboxRecord и MboxArchive представляют mboxrd-данные.

**Какие библиотеки использовать.** [`mail/mbox`](../std/mail/mbox.ok), [`mail/message`](../std/mail/message.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** MboxSeparator и MboxRecord связывают явную envelope-строку с MailMessage. Архив применяет обратимое From-quoting.

```okkama
module app v1.0.0;
import "mail/mbox" take MboxSeparator, MboxRecord, MboxArchive, ParsedMbox;
import "mail/message" take ParsedMail;
import "encoding/utf" take Utf8Encoded, Utf8Decoded;
import "io/stdout" take Stdout;

main() is int {
    let message=ParsedMail(Utf8Encoded("Subject: example\r\n\r\nFrom sender\r\n>From quoted\r\n").bytes()).message();
    let record=MboxRecord(MboxSeparator("From sender@example.test Thu Jan  1 00:00:00 1970"),message);
    let archive=MboxArchive([record]).bytes();
    Stdout().write(Utf8Decoded(archive).string());
    let restored=ParsedMbox(archive).records().first();
    Stdout().write(Utf8Decoded(restored.message().body().bytes()).string());
    0;
}
```

[Исходный пример](../examples/std-v4/mbox.ok).

<a id="example-mbox-session"></a>

### 24.12. Изменить локальный mbox под блокировкой

**Задача.** Редактировать локальный mbox как сессию: MboxSession удерживает блокировку и явно фиксирует результат.

**Какие библиотеки использовать.** [`mail/mbox_store`](../std/mail/mbox_store.ok), [`mail/mbox`](../std/mail/mbox.ok), [`mail/message`](../std/mail/message.ok), [`fs/path`](../std/fs/path.ok), [`fs/file`](../std/fs/file.ok), [`fs/directory`](../std/fs/directory.ok), [`fs/temp`](../std/fs/temp.ok), [`io/decorators`](../std/io/decorators.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** MboxSession удерживает выбранный архив и фиксирует подготовленную следующую версию. Завершайте владение через close.

```okkama
module app v1.0.0;
import "mail/mbox_store" take Mbox;
import "mail/mbox" take MboxRecord, MboxSeparator;
import "mail/message" take ParsedMail;
import "fs/path" take Path;
import "fs/file" take File, FileMode;
import "fs/directory" take Directory;
import "fs/temp" take TemporaryDirectory;
import "io/decorators" take ByteSnapshot;
import "encoding/utf" take Utf8Encoded, Utf8Decoded;
import "io/stdout" take Stdout;

main() is int {
    let directory=TemporaryDirectory(Directory(Path(".")),"mbox-example-");defer directory.close();
    let path=directory.path().child("archive");
    let file=File(mode:FileMode.writing,path:path,binary:true);file.binary().write(ByteSnapshot([]));file.close();
    let session=Mbox(path).session();defer session.close();
    let message=ParsedMail(Utf8Encoded("Subject: example\r\n\r\nFrom sender\r\n").bytes()).message();
    session=session.add(MboxRecord(MboxSeparator("From sender@example.test Thu Jan  1 00:00:00 1970"),message));
    let saved=session.messages().first();
    Stdout().writeln(Utf8Decoded(saved.message().body().bytes()).string());
    session=session.remove(saved.identifier());
    Stdout().writeln(session.messages().size().string());
    0;
}
```

[Исходный пример](../examples/std-v4/mbox-session.ok).

<a id="example-mail-message"></a>

### 24.13. Подготовить письмо к доставке

**Задача.** Подготовить MIME-письмо к доставке: MailMessage и DeliveredMail представляют сообщение и явную подготовку заголовков.

**Какие библиотеки использовать.** [`mail/message`](../std/mail/message.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** MailMessage объединяет MessageHeaders и MIME-body. DeliveredMail выполняет выбранную подготовку перед передачей транспортному сотруднику.

```okkama
module app v1.0.0;
import "mail/message" take ParsedMail, DeliveredMail, MessageId, MailDate, ParsedMailDate;
import "encoding/utf" take Utf8Encoded, Utf8Decoded;
import "io/stdout" take Stdout;

main() is int {
    let message=ParsedMail(Utf8Encoded("To: reader@example.test\r\nBcc: private@example.test\r\nSubject: hello\r\n\r\nExample body").bytes()).message();
    Stdout().writeln(Utf8Decoded(DeliveredMail(message).bytes()).string());
    Stdout().writeln(MessageId("<example.1@example.test>").string());
    let date=ParsedMailDate("Fri, 21 Nov 97 09:55 -0000");
    Stdout().writeln(MailDate(date.value(),date.offset_known()).string());
    0;
}
```

[Исходный пример](../examples/std-v4/mail-message.ok).

<a id="example-smtp-wire"></a>

### 24.14. Подготовить SMTP DATA по EHLO сервера

**Задача.** Прочитать SMTP-код и многострочный ответ: ParsedSmtpReply и SmtpReplyDecoder сохраняют рамки ответа.

**Какие библиотеки использовать.** [`mail/smtp_reply`](../std/mail/smtp_reply.ok), [`mail/smtp`](../std/mail/smtp.ok), [`mail/smtp_data`](../std/mail/smtp_data.ok), [`mail/message`](../std/mail/message.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Сначала SmtpCapabilities разбирает расширения. PreparedSmtpData учитывает объявленные возможности, применяет dot-stuffing и wire-завершение.

```okkama
module app v1.0.0;
import "mail/smtp_reply" take SmtpReplyDecoder;
import "mail/smtp" take SmtpCapabilities;
import "mail/smtp_data" take PreparedSmtpData;
import "mail/message" take ParsedMail;
import "encoding/utf" take Utf8Encoded, Utf8Decoded;
import "io/stdout" take Stdout;

main() is int {
    let decoder=SmtpReplyDecoder();decoder=decoder.fed(Utf8Encoded("250-mail.example\r\n250-SIZE 4096\r").bytes());
    decoder=decoder.fed(Utf8Encoded("\n250 8BITMIME\r\n").bytes());
    let capabilities=SmtpCapabilities(decoder.finish());
    Stdout().writeln(capabilities.maximum_size().string());
    let message=ParsedMail(Utf8Encoded("Subject: example\r\n\r\n.line\r\n").bytes()).message();
    let prepared=PreparedSmtpData(message,capabilities);
    Stdout().writeln(prepared.message_size().string());
    Stdout().write(Utf8Decoded(prepared.bytes()).string());
    0;
}
```

[Исходный пример](../examples/std-v4/smtp-wire.ok).

<a id="example-smtp-reading"></a>

### 24.15. Дочитать многострочный SMTP-ответ

**Задача.** Дочитать один SMTP-ответ из транспорта: SmtpReplyReading возвращает ответ и точный remainder.

**Какие библиотеки использовать.** [`mail/smtp_reading`](../std/mail/smtp_reading.ok), [`net/stream`](../std/net/stream.ok), [`io/stream`](../std/io/stream.ok), [`io/buffer`](../std/io/buffer.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Reply-reading операция получает поток и prefix. Используйте один result и передавайте remainder следующей операции чтения.

```okkama
module app v1.0.0;
import "mail/smtp_reading" take SmtpReplyReading;
import "net/stream" take AsyncReading;
import "io/stream" take Reading;
import "io/buffer" take ByteBuffer;
import "encoding/utf" take Utf8Encoded;
import "io/stdout" take Stdout;

object Fragments does AsyncReading {
    has origin is Reading;
    Fragments(origin is Reading) {self.origin=origin;}
    async bytes(limit is int) is @byte {origin.bytes(if limit<3 {limit;} else {3;});}
}
async main() is int {
    let buffer=ByteBuffer(Utf8Encoded("220 ready\r\n250-mail.example\r\n250 SIZE 4096\r\n").bytes());
    defer buffer.closing().close();let input=Fragments(buffer.reading());
    let greeting=await SmtpReplyReading(input).result();
    let ehlo=await SmtpReplyReading(input,greeting.remainder()).result();
    Stdout().writeln(greeting.reply().text());
    Stdout().writeln(ehlo.reply().text());
    0;
}
```

[Исходный пример](../examples/std-v4/smtp-reading.ok).

<a id="example-mime-word"></a>

### 24.16. Закодировать кириллицу в теме письма

**Задача.** Закодировать тему с кириллицей: EncodedWord и DecodedWords работают с encoded-word заголовков.

**Какие библиотеки использовать.** [`mime/word`](../std/mime/word.ok), [`message/header`](../std/message/header.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** EncodedWord выбирает представление заголовочного текста. DecodedWords восстанавливает его для показа пользователю.

```okkama
module app v1.0.0;
import "mime/word" take EncodedWord, DecodedWords;
import "message/header" take MessageHeader, MessageHeaderName;
import "io/stdout" take Stdout;
main() is int {
    let text="Письмо про чай и MIME";
    let encoded=EncodedWord(text).string();
    Stdout().writeln(encoded);
    let header=MessageHeader(MessageHeaderName("Subject"),encoded);
    Stdout().writeln(DecodedWords(header.value()).string());
    0;
}
```

[Исходный пример](../examples/std-v4/mime-word.ok).

<a id="example-mime-disposition"></a>

### 24.17. Задать имя файла во вложении

**Задача.** Указать имя скачиваемого файла: ContentDisposition представляет inline либо attachment с параметрами.

**Какие библиотеки использовать.** [`mime/disposition`](../std/mime/disposition.ok), [`mime/media_type`](../std/mime/media_type.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ContentDisposition хранит disposition и параметры имени. Передайте его в заголовки соответствующей MIME-части.

```okkama
module app v1.0.0;
import "mime/disposition" take ContentDisposition;
import "mime/media_type" take MimeParameters, MimeParameter;
import "io/stdout" take Stdout;
main() is int {
    let value=ContentDisposition("attachment",MimeParameters([MimeParameter("filename","отчёт.txt")]));
    Stdout().writeln(value.string());
    Stdout().writeln(ContentDisposition(value.string()).parameters().first("filename"));
    0;
}
```

[Исходный пример](../examples/std-v4/mime-disposition.ok).

<a id="example-mime-part"></a>

### 24.18. Собрать письмо с текстом и вложением

**Задача.** Собрать письмо с текстом и вложениями: MimePart, TextBody и BinaryBody задают MIME-структуру.

**Какие библиотеки использовать.** [`mime/part`](../std/mime/part.ok), [`message/header`](../std/message/header.ok), [`encoding/utf`](../std/encoding/utf.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** TextBody и BinaryBody задают содержимое, MimePart связывает его с заголовками и границами multipart.

```okkama
module app v1.0.0;
import "mime/part" take MimePart, TextBody, MultipartBody, ParsedMime, MimeContent;
import "message/header" take MessageHeader, MessageHeaderName, MessageHeaders;
import "encoding/utf" take Utf8Decoded;
import "io/stdout" take Stdout;
main() is int {
    let leaf_headers=MessageHeaders([
        MessageHeader(MessageHeaderName("Content-Type"),"text/plain; charset=UTF-8"),
        MessageHeader(MessageHeaderName("Content-Transfer-Encoding"),"base64")]);
    let leaf=MimePart(leaf_headers,TextBody("Привет, MIME!","UTF-8"));
    let body=MultipartBody("mixed","okkama-example",[leaf]);
    let headers=MessageHeaders([MessageHeader(MessageHeaderName("Content-Type"),"multipart/mixed; boundary=okkama-example")]);
    let message=MimePart(headers,body);let wire=message.bytes();
    Stdout().writeln(Utf8Decoded(wire).string());
    Stdout().writeln(MimeContent(ParsedMime(wire).part()).type().string());
    0;
}
```

[Исходный пример](../examples/std-v4/mime-part.ok).

<a id="example-mime-types"></a>

### 24.19. Определить media type по имени файла

**Задача.** Определить тип файла по расширению: StandardMediaTypes, MappedMediaTypes и SystemMediaTypes дают явный источник соответствий.

**Какие библиотеки использовать.** [`mime/types`](../std/mime/types.ok), [`mime/media_type`](../std/mime/media_type.ok), [`fs/path`](../std/fs/path.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Выберите явный источник правил: StandardMediaTypes либо собственный MappedMediaTypes. Наблюдайте MediaType для нужного расширения.

```okkama
module app v1.0.0;
import "mime/types" take StandardMediaTypes, MappedMediaTypes, MediaTypeExtension;
import "mime/media_type" take MediaType;
import "fs/path" take Path;
import "io/stdout" take Stdout;
main() is int {
    let standard=StandardMediaTypes();
    Stdout().writeln(standard.type(Path("not-opened/archive.TAR.GZ")).string());
    let personal=MappedMediaTypes([MediaTypeExtension("ok",MediaType("text/plain"))],standard);
    Stdout().writeln(personal.type(Path("source.ok")).string());
    let extensions=personal.extensions(MediaType("text/plain")).map() -> extension {extension;};
    Stdout().writeln(extensions.join(", "));
    0;
}
```

[Исходный пример](../examples/std-v4/mime-types.ok).

<a id="chapter-locale"></a>

## 25. Локализация и переводы

Locale задаёт язык и регион как значение. Отдельно выбираются подходящая локаль, формат чисел и дат, правила множественного числа и текст каталога. Все эти решения можно передать в пользовательский интерфейс явно.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`i18n/catalog`](../std/i18n/catalog.ok) | Выдать перевод с контекстом и количеством: MessageCatalog связывает MessageKey с singular и plural формами. | `MessageKey`, `MessageEntry`, `PluralForm`, `SingularMessage`, `PluralMessage` | [Рецепт](#example-locale-i18n) |
| [`i18n/gettext`](../std/i18n/gettext.ok) | Подключить переводы из PO или MO: gettext-представления разбирают каталог и сохраняют правила форм. | `GettextListArray`, `GettextAscii`, `GettextSpace`, `GettextControl`, `GettextContains` | [Рецепт](#example-gettext-po) |
| [`i18n/operands`](../std/i18n/operands.ok) | Сохранить значение 1.0 при выборе множественного числа: PluralOperands удерживает различимые числовые операнды правила. | `PluralOperands` | [Рецепт](#example-gettext-po) |
| [`i18n/plural`](../std/i18n/plural.ok) | Выбрать файл, файла или файлов: PluralNumber и PluralCategoryOf определяют категорию количества. | `PluralCategory`, `PluralKind`, `PluralNumber`, `CanonicalPluralNumber`, `PluralCategoryOf` | [Рецепт](#example-locale-i18n) |
| [`locale/collation`](../std/locale/collation.ok) | Отсортировать имена по языковым правилам: CollatedString и CollationOptions задают силу и сравнение. | `CollationStrength`, `CollationCaseFirst`, `CollationOptions`, `CollationDataVersion`, `CollatedString` | [Рецепт](#example-localized-collation) |
| [`locale/currency`](../std/locale/currency.ok) | Оформить цену с кодом или символом валюты: Currency и LocalizedCurrency выбирают представление суммы. | `CurrencyDisplay`, `Currency`, `CurrencyFormat`, `CurrencyDataVersion`, `CurrencyLocaleData` | [Рецепт](#example-localized-currencies) |
| [`locale/data`](../std/locale/data.ok) | Найти данные форматирования через fallback: LocaleFallbacks задаёт явную цепочку локалей. | `LocaleDataVersion`, `LocaleDataKey`, `LocaleFallbacks`, `LocaleDataChain` | [Рецепт](#example-locale-i18n) |
| [`locale/date_time`](../std/locale/date_time.ok) | Показать дату доставки привычным способом: локализованные представления связывают стиль, Locale и часовой пояс. | `DateStyle`, `TimeStyle`, `DateTimeStyle`, `DateStyleIndex`, `TimeStyleIndex` | [Рецепт](#example-localized-date-time) |
| [`locale/likely`](../std/locale/likely.ok) | Развернуть неполный язык интерфейса: LikelyLocale применяет add-likely-subtags к заданному Locale. | `LikelyLocale` | [Рецепт](#example-locale-i18n) |
| [`locale/list`](../std/locale/list.ok) | Написать естественный список товаров: LocalizedList выбирает союзы и шаблон перечисления языка. | `ListKind`, `ListStyle`, `LocalizedList`, `LocalizedListPattern` | [Рецепт](#recipe-locale-list) |
| [`locale/match`](../std/locale/match.ok) | Выбрать язык из клиентского списка: BasicLanguageRange и LookedUpLocale сопоставляют доступные варианты. | `BasicLanguageRange`, `LanguagePrefixes`, `BasicRangeSubtag`, `LocaleSelectionInputs`, `LocaleRangeIndex` | [Рецепт](#example-locale-i18n) |
| [`locale/number`](../std/locale/number.ok) | Показать число в формате клиента: LocalizedDecimal и LocalizedInteger используют локальные разделители, цифры и точное округление. | `NumberNotation`, `NumberGrouping`, `NumberSign`, `PercentScale`, `NumberSpecial` | [Рецепт](#example-localized-currencies) |
| [`locale/system`](../std/locale/system.ok) | Принять языковые предпочтения ОС как вход настройки: SystemLanguagePreferences и PreferredLocales дают упорядоченный выбор. | `PreferredLanguageReading`, `FixedLanguagePreferences`, `SystemLanguagePreferences`, `PreferredLocales`, `PreferredLocalePolicy` | [Рецепт](#recipe-locale-list) |
| [`locale/tag`](../std/locale/tag.ok) | Сохранить язык интерфейса ru-KZ: Locale разбирает BCP 47 и удерживает канонические координаты. | `Locale`, `LocaleFromLanguageTag`, `LocaleVariantSnapshot`, `LocaleSubtag`, `LanguageTagValue` | [Рецепт](#recipe-locale-list) |

### Что учитывать при сборке решения

- `locale/tag`: BCP 47-локаль является явным значением; форматирование получает её как зависимость.

<a id="recipe-locale-list"></a>

### 25.1. Сформировать естественное перечисление

**Задача.** В письме клиенту нужно перечислить заказанные товары с союзом выбранного языка.

**Какие библиотеки использовать.** [`locale/list`](../std/locale/list.ok), [`locale/tag`](../std/locale/tag.ok), [`locale/system`](../std/locale/system.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Locale передаётся вместе с видом списка и стилем. Языковые предпочтения можно получить через SystemLanguagePreferences либо задать FixedLanguagePreferences как явный вход.

```okkama
module cookbook/locale_list v1.0.0;
import "locale/list" take LocalizedList,ListKind,ListStyle;
import "locale/tag" take Locale;
import "locale/system" take FixedLanguagePreferences;
import "io/stdout" take Stdout;

main() is int {
    let preferences = FixedLanguagePreferences(["ru-KZ", "en"], true);
    confirm preferences.tags().item(0) == "ru-KZ";
    let products = ["чай", "кофе", "сок"];
    let text = LocalizedList(products, Locale("ru"), ListKind.conjunction, ListStyle.long).string();
    Stdout().writeln(text);
    0;
}
```

<a id="example-locale-i18n"></a>

### 25.2. Выбрать язык и форму слова файл

**Задача.** Сохранить язык интерфейса ru-KZ: Locale разбирает BCP 47 и удерживает канонические координаты.

**Какие библиотеки использовать.** [`locale/tag`](../std/locale/tag.ok), [`locale/match`](../std/locale/match.ok), [`locale/data`](../std/locale/data.ok), [`locale/likely`](../std/locale/likely.ok), [`i18n/plural`](../std/i18n/plural.ok), [`i18n/catalog`](../std/i18n/catalog.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** LookedUpLocale выбирает доступную локаль. MessageCatalog получает plural-формы, а PluralNumber сохраняет текстовое число для правила категории.

```okkama
module example v1.0.0;
import "locale/tag" take Locale;
import "locale/match" take BasicLanguageRange,LookedUpLocale;
import "locale/data" take LocaleFallbacks;
import "locale/likely" take LikelyLocale;
import "i18n/plural" take PluralNumber,PluralCategory;
import "i18n/catalog" take MessageKey,PluralForm,PluralMessage,MessageCatalog;
import "io/stdout" take Stdout;
main() is int {
    let chosen=LookedUpLocale([BasicLanguageRange("ru-KZ")],[Locale("en"),Locale("ru")],Locale("en")).value();
    Stdout().writeln(LikelyLocale(chosen).value().string());
    let key=MessageKey("","files");
    let catalog=MessageCatalog(chosen,[PluralMessage(key,[PluralForm(PluralCategory.one,"файл"),PluralForm(PluralCategory.few,"файла"),PluralForm(PluralCategory.other,"файлов")])]);
    for ["1","2","11","1.0"]->text {Stdout().writeln(text+" "+catalog.plural(key,PluralNumber(text)));}
    let fallback=LocaleFallbacks(Locale("en","AU"));
    for (0..fallback.size())->index {Stdout().writeln(fallback.item(index).string());}
    0;
}
```

[Исходный пример](../examples/std-v4/locale-i18n.ok).

<a id="example-gettext-po"></a>

### 25.3. Загрузить переводы из PO

**Задача.** Подключить переводы из PO или MO: gettext-представления разбирают каталог и сохраняют правила форм.

**Какие библиотеки использовать.** [`i18n/gettext`](../std/i18n/gettext.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Gettext-кодек читает каталог с контекстами и правилами plural. Передайте полученное представление в выбор сообщения интерфейса.

```okkama
module example v1.0.0;
import "i18n/gettext" take ParsedPo,PoString;
import "io/stdout" take Stdout;
main() is int {
    let source="# translator\nmsgctxt \"menu\"\nmsgid \"Open\"\nmsgstr \"Открыть\"\n";
    let document=ParsedPo(source).value();
    Stdout().writeln(document.entries().item(0).translations()[0]);
    Stdout().writeln(PoString(document).string());
    0;
}
```

[Исходный пример](../examples/std-v4/gettext-po.ok).

<a id="example-localized-collation"></a>

### 25.4. Отсортировать имена по правилам языка

**Задача.** Отсортировать имена по языковым правилам: CollatedString и CollationOptions задают силу и сравнение.

**Какие библиотеки использовать.** [`locale/collation`](../std/locale/collation.ok), [`locale/tag`](../std/locale/tag.ok), [`compare`](../std/compare.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** CollatedString связывает исходный текст с Locale и CollationOptions. Sorted использует это Comparing-представление.

```okkama
module example v1.0.0;
import "locale/collation" take CollatedString,CollationOptions,CollationStrength,CollationCaseFirst;
import "locale/tag" take Locale;
import "compare" take Order;
import "io/stdout" take Stdout;
main() is int {
    let options=CollationOptions(CollationStrength.tertiary,CollationCaseFirst.locale_default,true,true);
    let earlier=CollatedString("file2",Locale("en"),options);
    let later=CollatedString("file10",Locale("en"),options);
    Stdout().writeln(earlier.string());
    confirm earlier.order(later)==Order.less else throw InvalidExample();
    0;
}
object InvalidExample does Error {InvalidExample();text() is string{"unexpected numeric ordering";}}
```

[Исходный пример](../examples/std-v4/localized-collation.ok).

<a id="example-localized-currencies"></a>

### 25.5. Показать цену с символом валюты

**Задача.** Оформить цену с кодом или символом валюты: Currency и LocalizedCurrency выбирают представление суммы.

**Какие библиотеки использовать.** [`locale/currency`](../std/locale/currency.ok), [`locale/number`](../std/locale/number.ok), [`locale/tag`](../std/locale/tag.ok), [`math/decimal`](../std/math/decimal.ok), [`math/rounding`](../std/math/rounding.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Currency задаёт валюту, CurrencyFormat — представление, LocalizedCurrency — сумму и локаль. Округление передаётся явно.

```okkama
module example v1.0.0;
import "locale/currency" take Currency,CurrencyFormat,CurrencyDisplay,LocalizedCurrency;
import "locale/number" take NumberFormat,NumberNotation,NumberGrouping,NumberSign,NumberPrecision;
import "locale/tag" take Locale;
import "math/decimal" take Decimal;
import "math/rounding" take Rounding;
import "io/stdout" take Stdout;
main() is int {
    let numbers=NumberFormat(NumberNotation.standard,NumberGrouping.automatic,NumberSign.automatic,NumberPrecision(1,0,2,Rounding.half_even));
    let format=CurrencyFormat(numbers,CurrencyDisplay.symbol,true,true);
    Stdout().writeln(LocalizedCurrency(Decimal("-1234.5"),Currency("USD"),Locale("en"),format).string());
    Stdout().writeln(LocalizedCurrency(Decimal("1234.5"),Currency("EUR"),Locale("fr"),format).string());
    0;
}
```

[Исходный пример](../examples/std-v4/localized-currencies.ok).

<a id="example-localized-date-time"></a>

### 25.6. Показать дату и время доставки клиенту

**Задача.** Показать дату доставки привычным способом: локализованные представления связывают стиль, Locale и часовой пояс.

**Какие библиотеки использовать.** [`locale/date_time`](../std/locale/date_time.ok), [`locale/tag`](../std/locale/tag.ok), [`time/date`](../std/time/date.ok), [`time/clock`](../std/time/clock.ok), [`time/duration`](../std/time/duration.ok), [`time/zone`](../std/time/zone.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Локализованное представление получает Locale и стиль. Для Instant явно передайте TimeZone.

```okkama
module example v1.0.0;
import "locale/date_time" take LocalizedDate,LocalizedInstant,DateStyle,TimeStyle,DateTimeStyle;
import "locale/tag" take Locale;
import "time/date" take Date;
import "time/clock" take Instant;
import "time/duration" take Duration,DurationUnit;
import "time/zone" take Zone;
import "io/stdout" take Stdout;
main() is int {
    Stdout().writeln(LocalizedDate(Date(2026,8,11),Locale("fr"),DateStyle.full).string());
    let instant=Instant(Duration(1786467845.rat(),DurationUnit.second));
    Stdout().writeln(LocalizedInstant(instant,Zone("America/New_York"),Locale("en"),DateTimeStyle(DateStyle.full,TimeStyle.full)).string());
    0;
}
```

[Исходный пример](../examples/std-v4/localized-date-time.ok).

<a id="chapter-async"></a>

## 26. Асинхронность и параллельная обработка

Для фонового задания время ожидания и аренда в БД решают разные задачи.
`SystemClock.wait` вместе с `Raced<T>` позволяет запускать heartbeat и отменять
его с ожиданием очистки. Общий срок аренды проверяйте по часам БД, а номер
поколения — внутри SQL-запроса; локальный `MonotonicInstant` нельзя переносить
между процессами как общий timestamp. Пример композиции и правила сохранения
исходного платёжного ключа описаны в [протоколе аренды воркера](operational-worker-leases.md).
`WorkLease` в этом примере — адаптер приложения, а `Raced` и часы — стандартная библиотека.

Future представляет отложенную операцию, а группа или scope владеет её выполнением и завершением. Выбирайте канал для передачи заданий, семафор для ограничения внешнего параллелизма и отмену для остановки связанной работы.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`async/atomic`](../std/async/atomic.ok) | Опубликовать новое наблюдение общей инфраструктуры: Atomic удерживает явную ячейку и compare_and_replace. | `Atomic`, `AtomicClosed` | [Рецепт](#example-http2-preface) |
| [`async/barrier`](../std/async/barrier.ok) | Начать этап после готовности всех участников: Barrier синхронизирует заданное число ожидающих задач. | `Barrier`, `BarrierState`, `BrokenBarrier` | [Рецепт](#recipe-async-phases) |
| [`async/cancellation`](../std/async/cancellation.ok) | Остановить обработку по запросу владельца: Cancellation выдаёт узкий Cancelling для потребителей. | `Cancelling`, `Cancellation`, `CancellationToken` | [Рецепт](#example-http-static) |
| [`async/channel`](../std/async/channel.ok) | Передавать задания от производителя обработчику: Channel разделяет Sending и Receiving. | `Channel`, `Sending`, `Receiving`, `ChannelClosed` | [Рецепт](#example-http-sse) |
| [`async/collect`](../std/async/collect.ok) | Выбрать первый удачный ответ зеркала: FirstSuccessful, Raced и AllFutures задают способ ожидания Future. | `AllFutures`, `Raced`, `FirstSuccessful`, `FutureCompletions` | [Рецепт](#recipe-async-phases) |
| [`async/event`](../std/async/event.ok) | Дождаться готовности фонового подключения: Event предоставляет явный сигнал ready, set и wait. | `Event` | [Рецепт](#example-pop3-session) |
| [`async/lock`](../std/async/lock.ok) | Сериализовать доступ к внешнему ресурсу: Lock выдаёт LockGuard с ограниченным временем владения. | `Lock`, `LockGuard` | [Рецепт](#recipe-async-phases) |
| [`async/scope`](../std/async/scope.ok) | Дождаться всех фоновых операций запроса: FutureGroup и FutureScope явно владеют запущенной работой. | `Supervising`, `FutureGroup`, `FutureScope`, `FinishedFutureGroup` | [Рецепт](#recipe-logging-file) |
| [`async/semaphore`](../std/async/semaphore.ok) | Ограничить число одновременных запросов: Semaphore выдаёт Permit на время операции. | `Semaphore`, `Permit` | [Рецепт](#recipe-async-phases) |
| [`async/timeout`](../std/async/timeout.ok) | Ограничить время ответа партнёра: Within и Before соединяют Future с длительностью либо deadline. | `Within`, `Before`, `TimedOut` | [Рецепт](#example-udp-receive) |
| [`parallel/map`](../std/parallel/map.ok) | Преобразовать независимые документы ограниченным числом workers: ParallelMapped сохраняет порядок результатов. | `ParallelMapped`, `ParallelMapRun`, `ParallelMapProgress` | [Рецепт](#example-parallel-map) |

### Что учитывать при сборке решения

- `async/atomic`: Это явная инфраструктурная capability общей ячейки. В рецептах построения коллекций используйте возвращаемые immutable значения.
- `async/scope`: Scope ожидает завершения дочерних операций и cleanup перед публикацией результата.

<a id="recipe-async-phases"></a>

### 26.1. Дождаться готовности двух обработчиков

**Задача.** Две операции готовят свои части, затем одновременно переходят к общему этапу.

**Какие библиотеки использовать.** [`async/barrier`](../std/async/barrier.ok), [`async/collect`](../std/async/collect.ok), [`async/lock`](../std/async/lock.ok), [`async/semaphore`](../std/async/semaphore.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Вызовы async создают Future. AllFutures запускает обе операции, а Barrier освобождает поколение после двух wait. Ограничение внешнего параллелизма выражают Semaphore/Permit, эксклюзивный доступ — Lock/LockGuard.

```okkama
module cookbook/async_phases v1.0.0;
import "async/barrier" take Barrier;
import "async/collect" take AllFutures;
import "async/lock" take Lock;
import "async/semaphore" take Semaphore;
import "io/stdout" take Stdout;

object Worker {
    has phase is Barrier;
    Worker(phase is Barrier) { self.phase = phase; }
    async run() is int { await phase.wait(); }
}

async main() is int {
    let phase = Barrier(parties: 2);
    let first = Worker(phase).run();
    let second = Worker(phase).run();
    let pending is @Future<int> = [first, second];
    let results = await AllFutures(origins: pending).results();
    confirm results.size() == 2;
    Stdout().writeln("оба обработчика готовы");
    0;
}
```

<a id="example-parallel-map"></a>

### 26.2. Параллельно преобразовать независимые записи

**Задача.** Преобразовать независимые документы ограниченным числом workers: ParallelMapped сохраняет порядок результатов.

**Какие библиотеки использовать.** [`parallel/map`](../std/parallel/map.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** ParallelMapped связывает входы, action и число workers. Результирующая последовательность сохраняет исходный порядок.

```okkama
module app v1.0.0;
import "parallel/map" take ParallelMapped;
import "io/stdout" take Stdout;

object Worker {
    has seed is int;
    Worker(:seed);
    run(value is int) is int { seed + value; }
}

async main() is int {
    let mapped is ParallelMapped<int,int> = ParallelMapped(
        origins: [5,1,9,2,4], action: Worker(seed: 10).run, workers: 2);
    let results = await mapped.results();
    for results -> result { Stdout().writeln(result.string()); }
    0;
}
```

[Исходный пример](../examples/std-v4/parallel-map.ok).

<a id="chapter-logging"></a>

## 27. Журналы событий

Соберите запись из имени подсистемы, уровня, времени и полей. Декораторы добавляют контекст, фильтруют и скрывают значения; назначения формируют строку, файл либо сообщение syslog. Завершение очереди доставки — отдельный этап остановки сервиса.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`logging/enrichment`](../std/logging/enrichment.ok) | Добавить общий tenant_id в дочерний logger: FieldLogging и ChildLogging обогащают контекст. | `FieldLogging`, `ChildLogging` | [Рецепт](#recipe-logging-policy) |
| [`logging/field`](../std/logging/field.ok) | Добавить order_id к записи: LogFields хранит типизированные JSON-поля контекста. | `LogFields`, `LogFieldSnapshot`, `CombinedLogFields` | [Рецепт](#recipe-logging-policy) |
| [`logging/file`](../std/logging/file.ok) | Сохранить журнал в локальный файл: LogFile и FileLogOptions задают путь, права и правила записи. | `FileLogOptions`, `LogPermissionBits`, `LogFileState`, `LogFile` | [Рецепт](#recipe-logging-file) |
| [`logging/filter`](../std/logging/filter.ok) | Оставить warning для шумной подсистемы: LevelThresholds и AllowedLogs задают явный отбор. | `LogAllowing`, `LevelOverride`, `LevelThresholds`, `LogThresholdChoice`, `AllowedLogs` | [Рецепт](#recipe-logging-policy) |
| [`logging/level`](../std/logging/level.ok) | Разделить trace, info и error события: LogLevel и его представления сохраняют уровень записи. | `LogLevel`, `LogLevelName`, `LogLevelFromName`, `LogLevelOrder`, `LogLevelRank` | [Рецепт](#recipe-logging-policy) |
| [`logging/line`](../std/logging/line.ok) | Сериализовать одну запись журнала: TextLogLine и JsonLogLine формируют физическую строку. | `TextLogLine`, `JsonLogLine`, `BoundedLogJson`, `PhysicalLogLine`, `LogLineLimit` | [Рецепт](#recipe-logging-policy) |
| [`logging/name`](../std/logging/name.ok) | Назвать подсистему обработки платежей: LogName и ChildLogName удерживают иерархию компонентов. | `LogName`, `ChildLogName` | [Рецепт](#recipe-logging-policy) |
| [`logging/named`](../std/logging/named.ok) | Писать события от имени сервиса: NamedLogging объединяет часы, process id, имя и назначение. | `LogProcessReading`, `CurrentLogProcess`, `Logging`, `NamedLogging` | [Рецепт](#recipe-logging-policy) |
| [`logging/queue`](../std/logging/queue.ok) | Развязать обработчик и доставку журнала: очередь задаёт бюджет, политику переполнения и draining. | `LogOverflow`, `LogDelivering`, `LogDraining`, `LogQueueOptions`, `SynchronousLogDelivery` | [Рецепт](#recipe-logging-file) |
| [`logging/record`](../std/logging/record.ok) | Передать событие нескольким получателям: LogRecord связывает время, уровень, сообщение, контекст и явную причину. | `LogContext`, `LogRecord` | [Рецепт](#recipe-logging-policy) |
| [`logging/redact`](../std/logging/redact.ok) | Скрыть token в структурированных полях: RedactedLogs и RedactedCauseLogs преобразуют запись перед передачей. | `RedactedLogs`, `RedactedCauseLogs`, `RedactedLogCause`, `ChangedLogRecord` | [Рецепт](#example-logging-stack) |
| [`logging/rotation`](../std/logging/rotation.ok) | Менять файл журнала по размеру или времени: SizeRotating и IntervalRotating выбирают архивный переход. | `Rotating`, `SizeRotating`, `IntervalRotating`, `LogArchive`, `LogArchiveCatalog` | [Рецепт](#recipe-logging-file) |
| [`logging/stream`](../std/logging/stream.ok) | Направить записи в stdout: TextStreamLogs и JsonStreamLogs используют переданный текстовый поток. | `LogStyle`, `LogStyles`, `TextStreamLogs`, `JsonStreamLogs`, `TextTerminalLogs` | [Рецепт](#recipe-logging-file) |
| [`logging/syslog`](../std/logging/syslog.ok) | Передать событие в syslog: SyslogOptions и syslog-представления задают facility, severity и transport. | `SyslogFacility`, `SyslogFacilityCode`, `SyslogSeverity`, `SyslogHeaderName`, `SyslogOptions` | [Рецепт](#recipe-logging-policy) |
| [`logging/write`](../std/logging/write.ok) | Дублировать журнал в несколько направлений: BroadcastLogs, FallbackLogs и CollectedLogs составляют LogWriting. | `LogWriting`, `DiscardingLogs`, `CollectedLogs`, `BroadcastLogs`, `FallbackLogs` | [Рецепт](#recipe-logging-policy) |

### Что учитывать при сборке решения

- `logging/queue`: При остановке сначала drain доставляет принятые события; затем закройте очередь и дождитесь группы workers.
- `logging/redact`: Настраивайте RedactedLogs до сериализации и доставки записи в выбранное назначение.

<a id="recipe-logging-policy"></a>

### 27.1. Отфильтровать события и добавить tenant_id

**Задача.** Для отчёта нужны только warning и выше, с общим контекстом клиента.

**Какие библиотеки использовать.** [`logging/write`](../std/logging/write.ok), [`logging/filter`](../std/logging/filter.ok), [`logging/enrichment`](../std/logging/enrichment.ok), [`logging/named`](../std/logging/named.ok), [`logging/name`](../std/logging/name.ok), [`logging/level`](../std/logging/level.ok), [`logging/field`](../std/logging/field.ok), [`logging/syslog`](../std/logging/syslog.ok), [`json/value`](../std/json/value.ok), [`time/clock`](../std/time/clock.ok).

**Как использовать.** Декораторы выполняются в порядке композиции: FieldLogging добавляет контекст, AllowedLogs отбирает записи, CollectedLogs удерживает результат. SyslogMessage показывает байтовое представление той же записи для syslog.

```okkama
module cookbook/logging_policy v1.0.0;
import "logging/write" take CollectedLogs;
import "logging/filter" take AllowedLogs,LevelThresholds;
import "logging/enrichment" take FieldLogging;
import "logging/named" take NamedLogging;
import "logging/name" take LogName;
import "logging/level" take LogLevel;
import "logging/field" take LogFields;
import "logging/syslog" take SyslogMessage,SyslogOptions,SyslogFacility;
import "json/value" take JsonMember,JsonText;
import "time/clock" take SystemClock;

main() is int {
    let records = CollectedLogs();
    let sink = AllowedLogs(records, LevelThresholds(LogLevel.warning, []));
    let named = NamedLogging(LogName(["orders"]), sink, SystemClock(), LogFields([]));
    let logger = FieldLogging(named, LogFields([JsonMember("tenant_id", JsonText("shop-7"))]));
    logger.log(LogLevel.info, "запрос принят");
    logger.log(LogLevel.warning, "запрошен повтор");
    confirm records.size() == 1;
    let packet = SyslogMessage(records.records().item(0), SyslogOptions(SyslogFacility.local0, "orders-host", "orders", 4096));
    confirm packet.bytes().size() > 0;
    0;
}
```

<a id="recipe-logging-file"></a>

### 27.2. Записать журнал с ротацией и дождаться доставки

**Задача.** Сервису требуется ограничить размер рабочего log-файла и корректно завершить очередь.

**Какие библиотеки использовать.** [`logging/file`](../std/logging/file.ok), [`logging/rotation`](../std/logging/rotation.ok), [`logging/queue`](../std/logging/queue.ok), [`logging/stream`](../std/logging/stream.ok), [`logging/named`](../std/logging/named.ok), [`logging/name`](../std/logging/name.ok), [`logging/level`](../std/logging/level.ok), [`logging/field`](../std/logging/field.ok), [`fs/stat`](../std/fs/stat.ok), [`fs/temp`](../std/fs/temp.ok), [`fs/directory`](../std/fs/directory.ok), [`fs/path`](../std/fs/path.ok), [`async/scope`](../std/async/scope.ok), [`numbers/bigint`](../std/numbers/bigint.ok), [`time/clock`](../std/time/clock.ok).

**Как использовать.** Сначала drain ожидает доставку принятых записей, затем close завершает очередь, а workers.wait дожидается её работы. Размер ротации и число архивов передаются явно.

```okkama
module cookbook/logging_file v1.0.0;
import "logging/file" take FileLogOptions;
import "logging/rotation" take RotatingLogFile,SizeRotating;
import "logging/queue" take QueuedLogs,LogOverflow;
import "logging/stream" take JsonStreamLogs;
import "logging/named" take NamedLogging;
import "logging/name" take LogName;
import "logging/level" take LogLevel;
import "logging/field" take LogFields;
import "fs/stat" take PermissionMode,Permissions;
import "fs/temp" take TemporaryDirectory;
import "fs/directory" take Directory;
import "fs/path" take Path;
import "async/scope" take FutureGroup;
import "numbers/bigint" take BigInt;
import "time/clock" take SystemClock;

async main() is int {
    let scratch = TemporaryDirectory(Directory(Path(".")), "cookbook-log-");
    defer scratch.close();
    let clock = SystemClock();
    let mode = PermissionMode(384);
    let file = RotatingLogFile(scratch.path().child("orders.log"), FileLogOptions(mode,true,false), SizeRotating(BigInt("1024")), 3, clock);
    defer file.close();
    let workers is FutureGroup<int> = FutureGroup();
    let queue = QueuedLogs(JsonStreamLogs(file), 32, LogOverflow.reject, workers);
    let logger = NamedLogging(LogName(["orders"]), queue, clock, LogFields([]));
    logger.log(LogLevel.info, "заказ сохранён");
    await queue.drain();
    queue.close();
    await workers.wait();
    0;
}
```

<a id="example-logging-stack"></a>

### 27.3. Написать структурированный журнал с redaction

**Задача.** Писать события от имени сервиса: NamedLogging объединяет часы, process id, имя и назначение.

**Какие библиотеки использовать.** [`logging/named`](../std/logging/named.ok), [`logging/name`](../std/logging/name.ok), [`logging/level`](../std/logging/level.ok), [`logging/field`](../std/logging/field.ok), [`logging/stream`](../std/logging/stream.ok), [`logging/redact`](../std/logging/redact.ok), [`json/value`](../std/json/value.ok), [`time/clock`](../std/time/clock.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** NamedLogging получает имя, sink, clock и общие поля. RedactedLogs заменяет выбранные поля до JsonStreamLogs.

```okkama
module example v1.0.0;
import "logging/named" take NamedLogging;
import "logging/name" take LogName;
import "logging/level" take LogLevel;
import "logging/field" take LogFields;
import "logging/stream" take JsonStreamLogs;
import "logging/redact" take RedactedLogs;
import "json/value" take JsonMember,JsonText;
import "time/clock" take SystemClock;
import "io/stdout" take Stdout;

main() is int{
    let destination=RedactedLogs(JsonStreamLogs(Stdout()),["token"],JsonText("[redacted]"));
    let logger=NamedLogging(LogName(["example"]),destination,SystemClock(),LogFields([JsonMember("service",JsonText("demo"))]));
    logger.log(LogLevel.info,"started");
    logger.child("worker").log(LogLevel.warning,"retry\nrequested",LogFields([JsonMember("token",JsonText("private"))]));
    0;
}
```

[Исходный пример](../examples/std-v4/logging-stack.ok).

<a id="chapter-testing"></a>

## 28. Тесты, свойства, снимки и отчёты

Проверяйте предметное поведение на конкретных входах и воспроизводимых генерациях. Тестовый контекст предоставляет assertions, двойники и управляемое время. Результаты набора тестов становятся данными для удобного отчёта или CI.

### Задачи и выбор модулей

| Модуль | Задача и способ применения | Основные имена | Код |
|---|---|---|---|
| [`diff`](../std/diff.ok) | Объяснить изменение текстового отчёта: UnifiedDiff и сравнения последовательностей показывают отличающиеся строки. | `DiffListValues`, `DiffLines`, `DiffEditKind`, `DiffEdit`, `DiffSearching` | [Рецепт](#recipe-test-tools) |
| [`testing/check`](../std/testing/check.ok) | Проверить текст, байты и погрешность расчёта: TextEquals, BytesEqual и CloseTo выполняют предметные assertions. | `Checking`, `AsyncChecking`, `CheckContext`, `CloseTo`, `CheckWindow` | [Рецепт](#example-testing-values) |
| [`testing/context`](../std/testing/context.ok) | Выполнить проверку внутри запущенного теста: TestContext представляет контекст assertion-операций. | `TestContext` | [Рецепт](#recipe-test-tools) |
| [`testing/double`](../std/testing/double.ok) | Записать вызовы почтового транспорта: RecordedCalls и Stub составляют явный тестовый двойник. | `Recorded`, `RecordedCalls`, `Stub`, `StubExhausted` | [Рецепт](#recipe-test-tools) |
| [`testing/error`](../std/testing/error.ok) | Проверить ожидаемый типизированный отказ: Throwing и AsyncThrowing соединяют действие с ErrorMatching. | `Throwing`, `AsyncThrowing` | [Рецепт](#recipe-test-tools) |
| [`testing/generation`](../std/testing/generation.ok) | Получать воспроизводимые входы для свойства: IntegerGeneration и другие Generation строят значения и уменьшения. | `Generation`, `Generated`, `ReductionEntry`, `DistinctReductions`, `GenerationSize` | [Рецепт](#example-testing-values) |
| [`testing/property`](../std/testing/property.ok) | Проверить закон для множества входов: Property и AsyncProperty запускают генерацию с seed и shrinking-бюджетом. | `PropertyReproduction`, `PropertyObservation`, `Property`, `AsyncProperty` | [Рецепт](#example-testing-values) |
| [`testing/report`](../std/testing/report.ok) | Опубликовать результаты в CI: HumanTestReport, TapTestReport, JsonTestReport и JUnitTestReport выбирают формат. | `TestStatusName`, `ReportText`, `HumanTestReport`, `TapTestReport`, `JsonTestReport` | [Рецепт](#recipe-runner-report) |
| [`testing/result`](../std/testing/result.ok) | Передать результаты набора тестов: TestResult, TestFailure и TestReport представляют типизированный отчёт. | `TestStatus`, `TestPosition`, `TestOutput`, `TestResult`, `TestCounts` | [Рецепт](#example-junit-report) |
| [`testing/runner`](../std/testing/runner.ok) | Прочитать wire-отчёт встроенного runner: RecordedTestReport превращает явные данные runner в наблюдения. | `RecordedTestReport`, `RecordedTestResult`, `RecordedTestCause` | [Рецепт](#recipe-runner-report) |
| [`testing/snapshot`](../std/testing/snapshot.ok) | Сравнить JSON-отчёт с эталоном: SnapshotFile, JsonSnapshot и Snapshot разделяют проверку и явное обновление. | `SnapshotFile`, `TextSnapshot`, `JsonSnapshot`, `RawSnapshot`, `Snapshot` | [Рецепт](#example-testing-snapshots) |
| [`testing/time`](../std/testing/time.ok) | Проверить таймаут без ожидания реального времени: ManualClock даёт управляемые test-only часы. | `ManualClock`, `ManualClockWaiter`, `ManualClockFrame`, `ManualClockClosed` | [Рецепт](#recipe-test-tools) |

### Что учитывать при сборке решения

- `testing/snapshot`: Обновление эталона — отдельная явная операция SnapshotUpdate; обычный Snapshot выполняет сравнение.

<a id="recipe-test-tools"></a>

### 28.1. Проверить вызовы, отказ и управляемое время

**Задача.** Тест должен записать отправку уведомления, проверить отказ и продвинуть часы на две секунды.

**Какие библиотеки использовать.** [`testing/double`](../std/testing/double.ok), [`testing/error`](../std/testing/error.ok), [`testing/time`](../std/testing/time.ok), [`time/clock`](../std/time/clock.ok), [`time/duration`](../std/time/duration.ok), [`error`](../std/error.ok).

**Как использовать.** Запускайте программу с -t. RecordedCalls и ManualClock создаются внутри test-контекста; предметный двойник может делегировать им запись и выдачу заранее заданных ответов.

```okkama
module cookbook/test_tools v1.0.0;
import "testing/double" take RecordedCalls,Stub;
import "testing/error" take Throwing,ErrorTextMatching;
import "testing/time" take ManualClock;
import "time/clock" take Instant;
import "time/duration" take Duration,DurationUnit;
import "error" take InvalidArgument;

main() is int {

    0;
}

test "уведомления и время" {
    let calls is RecordedCalls<string> = RecordedCalls();
    calls.record("order-7");
    test.is(calls.records().item(0).value(), "order-7");
    let answers = Stub([200, 503]);
    test.is(answers.next(), 200);
    let rejection is Throwing<int> = Throwing(() is int { throw InvalidArgument("сумма"); }, ErrorTextMatching("сумма"));
    rejection.check();
    let clock = ManualClock(Instant(Duration(0.rat(), DurationUnit.second)));
    defer clock.close();
    clock.advance(Duration(2.rat(), DurationUnit.second));
    test.is(clock.monotonic_instant().reading().seconds(), 2.rat());
}
```

<a id="recipe-runner-report"></a>

### 28.2. Прочитать отчёт runner и получить JUnit

**Задача.** Промежуточный JSON-отчёт нужно превратить в типизированный результат и сериализовать для CI.

**Какие библиотеки использовать.** [`testing/runner`](../std/testing/runner.ok), [`testing/report`](../std/testing/report.ok), [`json/value`](../std/json/value.ok), [`json/write`](../std/json/write.ok), [`math/decimal`](../std/math/decimal.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** RecordedTestReport читает явный runner wire-format. JUnitTestReport получает тот же TestReport, который можно представить через HumanTestReport, TapTestReport или JsonTestReport.

```okkama
module cookbook/runner_report v1.0.0;
import "testing/runner" take RecordedTestReport;
import "testing/report" take JUnitTestReport;
import "json/value" take JsonObject,JsonMember,JsonText,JsonNumber,JsonArray;
import "json/write" take CompactJson;
import "math/decimal" take Decimal;
import "io/stdout" take Stdout;

main() is int {
    let wire = CompactJson(origin: JsonObject([
        JsonMember("start_ns", JsonText("0")),
        JsonMember("duration_ns", JsonText("0")),
        JsonMember("seed", JsonNumber(Decimal("42"))),
        JsonMember("results", JsonArray([]))
    ])).string();
    let report = RecordedTestReport(wire).value();
    Stdout().writeln(JUnitTestReport(report, "cookbook").string());
    0;
}
```

<a id="example-testing-values"></a>

### 28.3. Проверить закон на воспроизводимой выборке

**Задача.** Получать воспроизводимые входы для свойства: IntegerGeneration и другие Generation строят значения и уменьшения.

**Какие библиотеки использовать.** [`testing/generation`](../std/testing/generation.ok), [`testing/property`](../std/testing/property.ok), [`collections/sequence`](../std/collections/sequence.ok), [`testing/check`](../std/testing/check.ok), [`random/source`](../std/random/source.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** Generation задаёт входы, Property — seed, число случаев и бюджет shrinking. Проверка выполняется внутри test-контекста.

```okkama
module example v1.0.0;
import "testing/generation" take IntegerGeneration,SequenceGeneration;
import "testing/property" take Property;
import "collections/sequence" take Sequencing;
import "testing/check" take TextEquals;
import "random/source" take PcgRandom;
import "io/stdout" take Stdout;
main() is int {
    let generated=IntegerGeneration(-10,11).sample(PcgRandom(42),10);
    Stdout().writeln("seed=42 value="+generated.string());
    0;
}
test "configured sequence property" {
    let generation is SequenceGeneration<int>=SequenceGeneration(IntegerGeneration(-10,11),0,8);
    let property is Property<Sequencing<int>>=Property(generation,42,30,8,64,(values is Sequencing<int>) is bool{values.size()<=8;});
    property.check();
}
test "configured integer property" {
    TextEquals("same","same").check();
    let property is Property<int>=Property(IntegerGeneration(-10,11),42,100,10,64,(value is int) is bool{value>=-10 && value<=10;});
    property.check();
}
```

[Исходный пример](../examples/std-v4/testing-values.ok).

<a id="example-junit-report"></a>

### 28.4. Подготовить JUnit XML для CI

**Задача.** Передать результаты набора тестов: TestResult, TestFailure и TestReport представляют типизированный отчёт.

**Какие библиотеки использовать.** [`testing/result`](../std/testing/result.ok), [`testing/report`](../std/testing/report.ok), [`time/clock`](../std/time/clock.ok), [`time/duration`](../std/time/duration.ok), [`io/stdout`](../std/io/stdout.ok).

**Как использовать.** TestResult и TestReport являются значениями отчёта. JUnitTestReport получает suite name и сериализует выбранный профиль XML.

```okkama
module example v1.0.0;
import "testing/result" take TestReport,TestResult,TestStatus,TestOutput;
import "testing/report" take JUnitTestReport;
import "time/clock" take Instant;
import "time/duration" take Duration,DurationUnit;
import "io/stdout" take Stdout;
main() is int {
    let elapsed=Duration(1.rat(),DurationUnit.nanosecond);
    let report=TestReport(Instant(elapsed),elapsed,42,[
        TestResult("explicit report",TestStatus.passed,elapsed,[],TestOutput([byte(255)],[],false)),
    ]);
    Stdout().writeln(JUnitTestReport(report,"example").string());
    report.exit_status();
}
```

[Исходный пример](../examples/std-v4/junit-report.ok).

<a id="example-testing-snapshots"></a>

### 28.5. Проверить JSON-снимок отчёта

**Задача.** Сравнить JSON-отчёт с эталоном: SnapshotFile, JsonSnapshot и Snapshot разделяют проверку и явное обновление.

**Какие библиотеки использовать.** [`testing/snapshot`](../std/testing/snapshot.ok), [`fs/temp`](../std/fs/temp.ok), [`fs/directory`](../std/fs/directory.ok), [`fs/path`](../std/fs/path.ok), [`json/value`](../std/json/value.ok).

**Как использовать.** SnapshotFile указывает эталон, JsonSnapshot задаёт нормализованное представление результата. Update и check вызываются явно и отдельно.

```okkama
module example v1.0.0;
import "testing/snapshot" take SnapshotFile,TextSnapshot,JsonSnapshot,Snapshot,SnapshotUpdate;
import "fs/temp" take TemporaryDirectory;
import "fs/directory" take Directory;
import "fs/path" take Path;
import "json/value" take JsonObject,JsonMember,JsonText;
main() is int {0;}
test "explicit snapshot update and ordinary check" {
    let scratch=TemporaryDirectory(Directory(Path(".")),"okkama-snapshots-");
    defer scratch.close();
    let directory=Directory(scratch.path());
    let expected=SnapshotFile(directory,"invoice.json");
    let actual=JsonSnapshot(JsonObject([JsonMember("customer",JsonText("Ada"))]));
    SnapshotUpdate(actual,expected,4096).update();
    Snapshot(actual,expected,4096).check();
    test.is(TextSnapshot("first\r\nsecond\r").bytes(),TextSnapshot("first\nsecond\n").bytes());
}
```

[Исходный пример](../examples/std-v4/testing-snapshots.ok).

<a id="internal-modules"></a>

## Приложение A. Внутренние модули и таблицы данных

Прикладной рецепт выбирает публичный объект; тот использует перечисленных ниже сотрудников. Здесь показана задача каждого файла и путь к месту его применения. Протокольные главы полезны при реализации транспорта, а таблицы locale — при разборе происхождения локализованного вывода.

| Модуль | Какая работа выполняется | Публичное применение |
|---|---|---|
| [`http/internal/hpack`](../std/http/internal/hpack.ok) | Кодировать и декодировать ограниченные HPACK header blocks. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/hpack_primitives`](../std/http/internal/hpack_primitives.ok) | Выполнять HPACK integer/string/Huffman операции. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/hpack_static`](../std/http/internal/hpack_static.ok) | Предоставлять фиксированную HPACK static table. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/hpack_tables`](../std/http/internal/hpack_tables.ok) | Удерживать поля и индексированные таблицы HPACK-контекста. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/http2_body`](../std/http/internal/http2_body.ok) | Хранить ожидающие body-байты stream и согласовывать их длину. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/http2_controls`](../std/http/internal/http2_controls.ok) | Представлять RST_STREAM, GOAWAY и управляющие сообщения. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/http2_fields`](../std/http/internal/http2_fields.ok) | Переводить pseudo-fields HTTP/2 в Request/Response и обратно. | [`http/client`](#recipe-https-request), [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/http2_flow`](../std/http/internal/http2_flow.ok) | Проверять WINDOW_UPDATE и DATA с padding для flow-control. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/http2_frame`](../std/http/internal/http2_frame.ok) | Представлять и проверять заголовок и payload HTTP/2 frame. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/http2_headers`](../std/http/internal/http2_headers.ok) | Собирать HEADERS/CONTINUATION в один ограниченный блок заголовков. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/http2_input`](../std/http/internal/http2_input.ok) | Разбирать входные события одного HTTP/2-соединения и направлять их потокам. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/http2_life`](../std/http/internal/http2_life.ok) | Сохранять протокольное время жизни HTTP/2 stream. | [`http/http2_session`](#example-http2-session) |
| [`http/internal/http2_output`](../std/http/internal/http2_output.ok) | Сериализовать очереди исходящих frame в один транспорт. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/http2_preface`](../std/http/internal/http2_preface.ok) | Выполнить HTTP/2 client/server preface и начальное SETTINGS-согласование. | [`http/http2_connection`](#example-http2-connecting), [`http/http2_server`](#example-http2-server), [`http/selected_connection`](#example-http-selected-connecting) |
| [`http/internal/http2_reading`](../std/http/internal/http2_reading.ok) | Дочитать frame из асинхронного байтового потока. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/http2_receiving`](../std/http/internal/http2_receiving.ok) | Принимать DATA с учётом receive-credit соединения и отдельных stream. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/http2_sending`](../std/http/internal/http2_sending.ok) | Планировать отправку DATA при доступных окнах и завершать stream. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/http2_settings`](../std/http/internal/http2_settings.ok) | Применять SETTINGS партнёра и проверять отдельные значения. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`http/internal/trailers`](../std/http/internal/trailers.ok) | Проверять отложенные поля trailers при завершении тела. | [`http/http1_chunk`](#example-http1-chunks) |
| [`http/internal/websocket_frame`](../std/http/internal/websocket_frame.ok) | Проверять frame, masking, opcode и контрольные ограничения. | [`http/websocket_session`](#recipe-web) |
| [`http/internal/websocket_messages`](../std/http/internal/websocket_messages.ok) | Собирать fragmented frame в текстовое либо двоичное сообщение. | [`http/websocket_session`](#recipe-web) |
| [`http/internal/websocket_output`](../std/http/internal/websocket_output.ok) | Сериализовать исходящие WebSocket frame через выделенный writer. | [`http/server`](#example-http-server), [`http/websocket_client`](#example-websocket-client), [`http/websocket_server`](#recipe-web), [`http/websocket_session`](#recipe-web) |
| [`http/internal/websocket_reading`](../std/http/internal/websocket_reading.ok) | Дочитывать WebSocket frame из транспортного потока. | [`http/websocket_session`](#recipe-web) |
| [`http/internal/websocket_upgrade`](../std/http/internal/websocket_upgrade.ok) | Передавать проверенный HTTP/1 Upgrade и точный непрочитанный хвост. | [`http/websocket_client`](#example-websocket-client), [`http/websocket_server`](#recipe-web) |
| [`http/internal/wire_bytes`](../std/http/internal/wire_bytes.ok) | Обрабатывать небольшие проверенные байтовые представления wire-формата. | [`http/websocket_session`](#recipe-web) |
| [`locale/internal/currencies`](../std/locale/internal/currencies.ok) | Предоставлять сведения о кодах и minor units валют. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_0`](../std/locale/internal/currency_names_0.ok) | Предоставлять локализованные названия валют: фрагмент 0, 23 локалей от `aa` до `az-Cyrl`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_1`](../std/locale/internal/currency_names_1.ok) | Предоставлять локализованные названия валют: фрагмент 1, 7 локалей от `bal-Latn` до `bgn`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_10`](../std/locale/internal/currency_names_10.ok) | Предоставлять локализованные названия валют: фрагмент 10, 12 локалей от `gd` до `he`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_11`](../std/locale/internal/currency_names_11.ok) | Предоставлять локализованные названия валют: фрагмент 11, 9 локалей от `hi` до `ia`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_12`](../std/locale/internal/currency_names_12.ok) | Предоставлять локализованные названия валют: фрагмент 12, 16 локалей от `id` до `kea`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_13`](../std/locale/internal/currency_names_13.ok) | Предоставлять локализованные названия валют: фрагмент 13, 10 локалей от `kgp` до `ko`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_14`](../std/locale/internal/currency_names_14.ok) | Предоставлять локализованные названия валют: фрагмент 14, 16 локалей от `kok` до `lb`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_15`](../std/locale/internal/currency_names_15.ok) | Предоставлять локализованные названия валют: фрагмент 15, 18 локалей от `lg` до `mfe`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_16`](../std/locale/internal/currency_names_16.ok) | Предоставлять локализованные названия валют: фрагмент 16, 11 локалей от `mg` до `mr`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_17`](../std/locale/internal/currency_names_17.ok) | Предоставлять локализованные названия валют: фрагмент 17, 22 локалей от `ms` до `no`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_18`](../std/locale/internal/currency_names_18.ok) | Предоставлять локализованные названия валют: фрагмент 18, 16 локалей от `nqo` до `pl`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_19`](../std/locale/internal/currency_names_19.ok) | Предоставлять локализованные названия валют: фрагмент 19, 17 локалей от `ps` до `ro-MD`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_2`](../std/locale/internal/currency_names_2.ok) | Предоставлять локализованные названия валют: фрагмент 2, 8 локалей от `blo` до `brx`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_20`](../std/locale/internal/currency_names_20.ok) | Предоставлять локализованные названия валют: фрагмент 20, 22 локалей от `rof` до `sg`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_21`](../std/locale/internal/currency_names_21.ok) | Предоставлять локализованные названия валют: фрагмент 21, 12 локалей от `shi` до `so-KE`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_22`](../std/locale/internal/currency_names_22.ok) | Предоставлять локализованные названия валют: фрагмент 22, 12 локалей от `sq` до `sv`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_23`](../std/locale/internal/currency_names_23.ok) | Предоставлять локализованные названия валют: фрагмент 23, 12 локалей от `sw` до `teo-KE`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_24`](../std/locale/internal/currency_names_24.ok) | Предоставлять локализованные названия валют: фрагмент 24, 14 локалей от `tg` до `tzm`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_25`](../std/locale/internal/currency_names_25.ok) | Предоставлять локализованные названия валют: фрагмент 25, 11 локалей от `ug` до `vec`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_26`](../std/locale/internal/currency_names_26.ok) | Предоставлять локализованные названия валют: фрагмент 26, 12 локалей от `vi` до `yo-BJ`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_27`](../std/locale/internal/currency_names_27.ok) | Предоставлять локализованные названия валют: фрагмент 27, 13 локалей от `yrl` до `zu`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_3`](../std/locale/internal/currency_names_3.ok) | Предоставлять локализованные названия валют: фрагмент 3, 8 локалей от `bs` до `ccp`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_4`](../std/locale/internal/currency_names_4.ok) | Предоставлять локализованные названия валют: фрагмент 4, 10 локалей от `ce` до `cv`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_5`](../std/locale/internal/currency_names_5.ok) | Предоставлять локализованные названия валют: фрагмент 5, 12 локалей от `cy` до `dz`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_6`](../std/locale/internal/currency_names_6.ok) | Предоставлять локализованные названия валют: фрагмент 6, 105 локалей от `ebu` до `es-SV`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_7`](../std/locale/internal/currency_names_7.ok) | Предоставлять локализованные названия валют: фрагмент 7, 9 локалей от `es-US` до `ff`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_8`](../std/locale/internal/currency_names_8.ok) | Предоставлять локализованные названия валют: фрагмент 8, 21 локалей от `ff-Adlm` до `fil`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_names_9`](../std/locale/internal/currency_names_9.ok) | Предоставлять локализованные названия валют: фрагмент 9, 22 локалей от `fo` до `ga`. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/currency_rows`](../std/locale/internal/currency_rows.ok) | Находить перевод названия и символов валюты в упакованной строке данных. | [`locale/currency`](#example-localized-currencies) |
| [`locale/internal/day_periods`](../std/locale/internal/day_periods.ok) | Выбирать названия частей суток в локализованном времени. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/gregorian`](../std/locale/internal/gregorian.ok) | Предоставлять календарные шаблоны, месяцы и дни недели. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/iana`](../std/locale/internal/iana.ok) | Канонизировать языковые теги по встроенному IANA registry. | [`locale/data`](#example-locale-i18n), [`locale/tag`](#recipe-locale-list) |
| [`locale/internal/likely`](../std/locale/internal/likely.ok) | Расширять неполную локаль по данным likely-subtags. | [`locale/likely`](#example-locale-i18n) |
| [`locale/internal/lists`](../std/locale/internal/lists.ok) | Предоставлять шаблоны союзов и перечислений для LocalizedList. | [`locale/list`](#recipe-locale-list) |
| [`locale/internal/numbers`](../std/locale/internal/numbers.ok) | Предоставлять числовые символы, системы цифр и шаблоны форматирования. | [`locale/number`](#example-localized-currencies) |
| [`locale/internal/parents`](../std/locale/internal/parents.ok) | Выбирать явных родителей локали при поиске CLDR-данных. | [`locale/data`](#example-locale-i18n) |
| [`locale/internal/plurals`](../std/locale/internal/plurals.ok) | Предоставлять правила категорий множественного числа. | [`i18n/plural`](#example-locale-i18n) |
| [`locale/internal/zone_meta`](../std/locale/internal/zone_meta.ok) | Сопоставлять IANA-зоны с CLDR-метазонами для локализованной даты. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_name_rows`](../std/locale/internal/zone_name_rows.ok) | Находить метазону или конкретную зону в упакованном ряду названий. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names`](../std/locale/internal/zone_names.ok) | Выбирать локализованный ряд названий часовых поясов. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_0`](../std/locale/internal/zone_names_0.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 0, 6 локалей от `af` до `ast`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_1`](../std/locale/internal/zone_names_1.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 1, 9 локалей от `az` до `blo`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_10`](../std/locale/internal/zone_names_10.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 10, 6 локалей от `it` до `kgp`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_11`](../std/locale/internal/zone_names_11.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 11, 5 локалей от `kk` до `ko-KP`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_12`](../std/locale/internal/zone_names_12.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 12, 8 локалей от `kok` до `kxv-Deva`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_13`](../std/locale/internal/zone_names_13.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 13, 7 локалей от `kxv-Orya` до `lmo`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_14`](../std/locale/internal/zone_names_14.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 14, 6 локалей от `lo` до `mi`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_15`](../std/locale/internal/zone_names_15.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 15, 5 локалей от `mk` до `mni`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_16`](../std/locale/internal/zone_names_16.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 16, 9 локалей от `mr` до `nl-SR`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_17`](../std/locale/internal/zone_names_17.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 17, 8 локалей от `nn` до `os`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_18`](../std/locale/internal/zone_names_18.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 18, 9 локалей от `pa` до `pt-PT`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_19`](../std/locale/internal/zone_names_19.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 19, 16 локалей от `qu` до `se`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_2`](../std/locale/internal/zone_names_2.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 2, 5 локалей от `bn` до `bs-Cyrl`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_20`](../std/locale/internal/zone_names_20.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 20, 6 локалей от `se-FI` до `sq`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_21`](../std/locale/internal/zone_names_21.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 21, 9 локалей от `sr` до `sw-KE`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_22`](../std/locale/internal/zone_names_22.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 22, 7 локалей от `syr` до `tg`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_23`](../std/locale/internal/zone_names_23.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 23, 7 локалей от `th` до `tr`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_24`](../std/locale/internal/zone_names_24.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 24, 6 локалей от `tt` до `uz`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_25`](../std/locale/internal/zone_names_25.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 25, 8 локалей от `uz-Cyrl` до `xnr`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_26`](../std/locale/internal/zone_names_26.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 26, 12 локалей от `yo` до `zh-Hant-HK`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_27`](../std/locale/internal/zone_names_27.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 27, 1 локалей от `zu` до `zu`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_3`](../std/locale/internal/zone_names_3.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 3, 5 локалей от `ca` до `chr`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_4`](../std/locale/internal/zone_names_4.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 4, 9 локалей от `cs` до `dsb`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_5`](../std/locale/internal/zone_names_5.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 5, 57 локалей от `dz` до `es-VE`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_6`](../std/locale/internal/zone_names_6.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 6, 5 локалей от `et` до `fi`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_7`](../std/locale/internal/zone_names_7.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 7, 12 локалей от `fil` до `gsw`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_8`](../std/locale/internal/zone_names_8.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 8, 7 локалей от `gu` до `hr`. | [`locale/date_time`](#example-localized-date-time) |
| [`locale/internal/zone_names_9`](../std/locale/internal/zone_names_9.ok) | Предоставлять локализованные названия часовых поясов: фрагмент 9, 9 локалей от `hsb` до `is`. | [`locale/date_time`](#example-localized-date-time) |
| [`web/internal/reply`](../std/web/internal/reply.ok) | Строить согласованные HTTP response metadata и body для Text, Html, Json и Exception. | [`web/exception`](#recipe-web), [`web/reply`](#recipe-web) |

<a id="runtime-modules"></a>

## Приложение B. Граница с runtime

Эти модули связывают объекты stdlib с реализацией исполнения. Для каждой границы показана её работа и библиотечный объект, через который она участвует в рецепте. Например, `encoding/utf` использует `runtime/utf`, а `db/sqlite` — `runtime/database`.

| Модуль | Работа границы | Использование в stdlib |
|---|---|---|
| [`runtime/account`](../std/runtime/account.ok) | Получать переносимые наблюдения учётных записей пользователя и групп. | [`os/account`](#recipe-system) |
| [`runtime/application`](../std/runtime/application.ok) | Владеть host-эффектами terminal session, режимов ввода и экрана. | [`terminal/mode`](#recipe-terminal-prompt), [`terminal/screen`](#recipe-terminal-prompt), [`terminal/session`](#recipe-terminal-prompt) |
| [`runtime/base32`](../std/runtime/base32.ok) | Выполнять строгий байтовый кодек Base32. | [`encoding/base32`](#recipe-encodings) |
| [`runtime/base64`](../std/runtime/base64.ok) | Выполнять Base64 с выбранным алфавитом. | [`encoding/base64`](#recipe-text-bytes) |
| [`runtime/base85`](../std/runtime/base85.ok) | Выполнять ASCII85 и RFC 1924 Base85. | [`encoding/base85`](#recipe-encodings) |
| [`runtime/bigint`](../std/runtime/bigint.ok) | Сохранять и вычислять точные целые с заданным bit budget. | [`numbers/bigint`](#example-bigrational) |
| [`runtime/binary`](../std/runtime/binary.ok) | Преобразовывать числовые wire-поля и фиксированные маски байтов. | [`encoding/binary`](#recipe-encodings) |
| [`runtime/buffer`](../std/runtime/buffer.ok) | Удерживать непрозрачное хранилище байтового буфера IO. | [`io/buffer`](#recipe-buffer-output), [`io/decorators`](#example-mime-transfer) |
| [`runtime/collation`](../std/runtime/collation.ok) | Выполнять закреплённое языковое сравнение Unicode. | [`locale/collation`](#example-localized-collation) |
| [`runtime/collections`](../std/runtime/collections.ok) | Создавать типизированные immutable snapshots для ограниченных коллекций. | [`http/robots`](#example-http-robots), [`http/routing`](#example-http-routing) |
| [`runtime/crypto`](../std/runtime/crypto.ok) | Вычислять digest по имени алгоритма и выполнять secure_equal над байтами. | [`http/cache_admission`](#example-http-cache), [`http/digest`](#example-http-digest-https) |
| [`runtime/csv_state`](../std/runtime/csv_state.ok) | Удерживать состояние ограниченного CSV-сканера текущей записи. | [`csv/stream`](#recipe-csv-stream) |
| [`runtime/database`](../std/runtime/database.ok) | Владеть host-соединением SQLite, worker-операциями и дополнительными DB capability. | [`db/sqlite`](#example-database-stack) |
| [`runtime/debug`](../std/runtime/debug.ok) | Предоставлять явное диагностическое представление значения. | [`testing/check`](#example-testing-values) |
| [`runtime/decimal`](../std/runtime/decimal.ok) | Выполнять точное десятичное разложение и арифметические наблюдения. | [`math/decimal`](#recipe-decimal-price) |
| [`runtime/diff`](../std/runtime/diff.ok) | Вычислять ограниченные операции сравнения последовательностей. | [`diff`](#recipe-test-tools) |
| [`runtime/environment`](../std/runtime/environment.ok) | Снимать и преобразовывать явные снимки переменных окружения. | [`os/environment`](#recipe-proxy-environment) |
| [`runtime/format`](../std/runtime/format.ok) | Выполнять низкоуровневые типизированные форматные представления. | [`numbers/float`](#recipe-decimal-price) |
| [`runtime/fs`](../std/runtime/fs.ok) | Выполнять операции файлов, путей, каталогов, links, lock и watch. | [`db/sqlite`](#example-database-stack), [`fs/directory`](#recipe-file-discovery), [`fs/file`](#recipe-file-report), [`fs/glob`](#recipe-file-discovery) |
| [`runtime/fs_root`](../std/runtime/fs_root.ok) | Владеть файловыми дескрипторами корня и открытых относительно него элементов. | [`fs/root`](#example-http-static) |
| [`runtime/hex`](../std/runtime/hex.ok) | Кодировать байты и декодировать проверенные hex-пары. | [`encoding/hex`](#recipe-text-bytes) |
| [`runtime/hpack`](../std/runtime/hpack.ok) | Выполнять ограниченный bulk-примитив HPACK. | [`http/http2_server`](#example-http2-server), [`http/http2_session`](#example-http2-session) |
| [`runtime/html`](../std/runtime/html.ok) | Хранить лексические HTML-события и данные character references. | [`html/entities`](#example-html-entities), [`html/events`](#recipe-html-events) |
| [`runtime/idna`](../std/runtime/idna.ok) | Выполнять ограниченное UTS46-представление доменного имени. | [`net/endpoint`](#example-host-policy) |
| [`runtime/ini`](../std/runtime/ini.ok) | Хранить проверенное дерево INI и диагностические координаты. | [`config/ini`](#example-ini-codec) |
| [`runtime/input_state`](../std/runtime/input_state.ok) | Удерживать undecoded bytes и состояние одного UTF-8 input. | [`io/stdin`](#recipe-stdin) |
| [`runtime/io_decorators`](../std/runtime/io_decorators.ok) | Удерживать счётчики и состояние потоковых IO-декораторов. | [`io/decorators`](#example-mime-transfer) |
| [`runtime/json`](../std/runtime/json.ok) | Хранить проверенное точное дерево JSON и его диагностику. | [`json/parse`](#recipe-json-read) |
| [`runtime/layout`](../std/runtime/layout.ok) | Выполнять bulk-операции отступов и разбиения текста. | [`text/layout`](#recipe-text-unicode) |
| [`runtime/locale`](../std/runtime/locale.ok) | Получать системный снимок языковых предпочтений. | [`locale/system`](#recipe-locale-list) |
| [`runtime/math_aggregate`](../std/runtime/math_aggregate.ok) | Выполнять finite-float primitive для устойчивых числовых агрегатов. | [`math/aggregate`](#recipe-measurements), [`statistics`](#recipe-measurements) |
| [`runtime/net`](../std/runtime/net.ok) | Преобразовывать числовые IP-адреса и их packed-представления. | [`net/ip`](#example-ip-interface), [`net/multicast`](#example-multicast-options), [`net/tcp`](#example-tcp-options), [`net/udp`](#example-udp-options) |
| [`runtime/network`](../std/runtime/network.ok) | Владеть nonblocking socket, resolver и транспортными host-операциями. | [`net/dns`](#example-system-resolution), [`net/multicast`](#example-multicast-options), [`net/stream`](#example-smtp-reading), [`net/tcp`](#example-tcp-options) |
| [`runtime/ordering`](../std/runtime/ordering.ok) | Выполнять стабильное bulk-упорядочение типизированного снимка. | [`collections/ordering`](#recipe-lists) |
| [`runtime/pem`](../std/runtime/pem.ok) | Преобразовывать текстовые PEM-контейнеры и байтовые блоки. | [`crypto/x509/trust`](#example-certificate-observers), [`encoding/pem`](#example-certificate-observers) |
| [`runtime/process`](../std/runtime/process.ok) | Владеть дочерними процессами, argv, pipe и захватом вывода. | [`cli/arguments`](#example-application-cli), [`logging/named`](#recipe-logging-policy), [`os/command_capture`](#recipe-commands), [`os/pipeline_capture`](#recipe-pipeline) |
| [`runtime/public_suffix`](../std/runtime/public_suffix.ok) | Предоставлять поиск по закреплённым правилам public suffix. | [`net/public_suffix`](#example-host-policy) |
| [`runtime/quoted_printable`](../std/runtime/quoted_printable.ok) | Кодировать и декодировать MIME quoted-printable. | [`encoding/quoted_printable`](#recipe-imap-assignment) |
| [`runtime/random`](../std/runtime/random.ok) | Удерживать PCG32-состояние и fixed-width преобразование выборки. | [`random/source`](#recipe-random-ids) |
| [`runtime/random_distribution`](../std/runtime/random_distribution.ok) | Выполнять числовые primitives алгоритмов распределений. | [`random/distribution`](#recipe-random-ids) |
| [`runtime/regex`](../std/runtime/regex.ok) | Компилировать ограниченный NFA и удерживать memoized search-наблюдения. | [`regex/escape`](#recipe-regex), [`regex/match`](#recipe-regex), [`regex/pattern`](#recipe-regex), [`regex/replace`](#recipe-regex) |
| [`runtime/secret`](../std/runtime/secret.ok) | Владеть непрозрачной памятью секрета, экспортом и очисткой ресурса. | [`crypto/secret`](#example-http-proxy) |
| [`runtime/single_byte`](../std/runtime/single_byte.ok) | Преобразовывать ASCII и ISO-8859-1. | [`encoding/single_byte`](#example-hpack-primitives), [`http/http1_chunk`](#example-http1-chunks), [`http/http1_head`](#example-http1-framing) |
| [`runtime/snapshot`](../std/runtime/snapshot.ok) | Выполнять операции эталонных файлов тестовых снимков. | [`testing/snapshot`](#example-testing-snapshots) |
| [`runtime/statistics`](../std/runtime/statistics.ok) | Выполнять численно чувствительные статистические вычисления. | [`statistics`](#recipe-measurements) |
| [`runtime/stdio`](../std/runtime/stdio.ok) | Читать и писать байты стандартных потоков процесса. | [`io/stdin`](#recipe-stdin), [`io/stdout`](#recipe-stdin) |
| [`runtime/system`](../std/runtime/system.ok) | Получать системную идентичность, storage-наблюдения и энтропию. | [`fs/path`](#recipe-file-discovery), [`locale/system`](#recipe-locale-list), [`os/platform`](#recipe-system), [`os/storage`](#recipe-system) |
| [`runtime/template`](../std/runtime/template.ok) | Предоставлять bounded primitives plain-template программы. | [`template/compile`](#example-template-json), [`template/json`](#example-template-json), [`template/sections`](#example-template-sections) |
| [`runtime/testing`](../std/runtime/testing.ok) | Предоставлять контекст assertion-операций и ограниченное test-состояние. | [`testing/context`](#recipe-test-tools), [`testing/double`](#recipe-test-tools) |
| [`runtime/time`](../std/runtime/time.ok) | Получать wall/monotonic отсчёты и выполнять ожидание. | [`testing/time`](#recipe-test-tools), [`time/clock`](#recipe-test-tools) |
| [`runtime/time_format`](../std/runtime/time_format.ok) | Форматировать и разбирать календарные значения по явному шаблону. | [`time/format`](#recipe-calendar) |
| [`runtime/time_zone`](../std/runtime/time_zone.ok) | Получать правила встроенной базы IANA для выбранной зоны. | [`time/zone`](#example-localized-date-time) |
| [`runtime/tls_transport`](../std/runtime/tls_transport.ok) | Владеть TLS transport, handshake и проверкой peer. | [`net/stream`](#example-smtp-reading), [`net/tls`](#recipe-https-request) |
| [`runtime/toml`](../std/runtime/toml.ok) | Удерживать дерево TOML и типизированные наблюдения узлов. | [`toml/parse`](#example-toml-parse) |
| [`runtime/unicode_character`](../std/runtime/unicode_character.ok) | Предоставлять свойства Unicode Character Database. | [`logging/line`](#recipe-logging-policy), [`terminal/width`](#recipe-display-columns), [`unicode/character`](#recipe-text-unicode) |
| [`runtime/unicode_normalization`](../std/runtime/unicode_normalization.ok) | Выполнять выбранную нормальную форму Unicode. | [`unicode/normalization`](#recipe-text-unicode) |
| [`runtime/url_text`](../std/runtime/url_text.ok) | Выполнять профилированные bulk-операции текстового представления URL. | [`url/value`](#example-url-parameters) |
| [`runtime/utf`](../std/runtime/utf.ok) | Кодировать и декодировать UTF с выбранным порядком байтов. | [`encoding/utf`](#recipe-text-bytes), [`http/sse`](#example-http-sse) |
| [`runtime/uuid`](../std/runtime/uuid.ok) | Проверять и форматировать канонические 128-bit UUID. | [`uuid/generation`](#recipe-random-ids), [`uuid/value`](#recipe-random-ids) |
| [`runtime/x509`](../std/runtime/x509.ok) | Разбирать сертификаты, проверять цепочки и явно читать системные trust anchors. | [`crypto/x509/certificate`](#example-x509-certificate), [`crypto/x509/trust`](#example-certificate-observers) |

<a id="all-modules"></a>

## Приложение C. Алфавитный указатель всех модулей

Каждый путь соответствует одному `.ok`-файлу текущего `std/`. Прикладные ссылки ведут к главе с выбором и кодом; внутренние и runtime-пути — к их разбору выше.

| Модуль | Раздел книги |
|---|---|
| [`archive/entry`](../std/archive/entry.ok) | [Архивы и потоковые преобразования](#chapter-archives) |
| [`archive/limits`](../std/archive/limits.ok) | [Архивы и потоковые преобразования](#chapter-archives) |
| [`archive/metadata`](../std/archive/metadata.ok) | [Архивы и потоковые преобразования](#chapter-archives) |
| [`archive/path`](../std/archive/path.ok) | [Архивы и потоковые преобразования](#chapter-archives) |
| [`archive/stream`](../std/archive/stream.ok) | [Архивы и потоковые преобразования](#chapter-archives) |
| [`archive/tar`](../std/archive/tar.ok) | [Архивы и потоковые преобразования](#chapter-archives) |
| [`archive/zip`](../std/archive/zip.ok) | [Архивы и потоковые преобразования](#chapter-archives) |
| [`async/atomic`](../std/async/atomic.ok) | [Асинхронность и параллельная обработка](#chapter-async) |
| [`async/barrier`](../std/async/barrier.ok) | [Асинхронность и параллельная обработка](#chapter-async) |
| [`async/cancellation`](../std/async/cancellation.ok) | [Асинхронность и параллельная обработка](#chapter-async) |
| [`async/channel`](../std/async/channel.ok) | [Асинхронность и параллельная обработка](#chapter-async) |
| [`async/collect`](../std/async/collect.ok) | [Асинхронность и параллельная обработка](#chapter-async) |
| [`async/event`](../std/async/event.ok) | [Асинхронность и параллельная обработка](#chapter-async) |
| [`async/lock`](../std/async/lock.ok) | [Асинхронность и параллельная обработка](#chapter-async) |
| [`async/scope`](../std/async/scope.ok) | [Асинхронность и параллельная обработка](#chapter-async) |
| [`async/semaphore`](../std/async/semaphore.ok) | [Асинхронность и параллельная обработка](#chapter-async) |
| [`async/timeout`](../std/async/timeout.ok) | [Асинхронность и параллельная обработка](#chapter-async) |
| [`calendar`](../std/calendar.ok) | [Даты, время, часовые пояса и календарь](#chapter-time) |
| [`checksum/adler32`](../std/checksum/adler32.ok) | [Криптография, сертификаты и контроль целостности](#chapter-crypto) |
| [`checksum/crc`](../std/checksum/crc.ok) | [Криптография, сертификаты и контроль целостности](#chapter-crypto) |
| [`checksum/state`](../std/checksum/state.ok) | [Криптография, сертификаты и контроль целостности](#chapter-crypto) |
| [`cli/arguments`](../std/cli/arguments.ok) | [Командная строка и конфигурация](#chapter-config) |
| [`cli/help`](../std/cli/help.ok) | [Командная строка и конфигурация](#chapter-config) |
| [`cli/parse`](../std/cli/parse.ok) | [Командная строка и конфигурация](#chapter-config) |
| [`cli/spec`](../std/cli/spec.ok) | [Командная строка и конфигурация](#chapter-config) |
| [`collections/combinatorics`](../std/collections/combinatorics.ok) | [Коллекции и преобразование данных](#chapter-collections) |
| [`collections/combining`](../std/collections/combining.ok) | [Коллекции и преобразование данных](#chapter-collections) |
| [`collections/deque`](../std/collections/deque.ok) | [Коллекции и преобразование данных](#chapter-collections) |
| [`collections/enumeration`](../std/collections/enumeration.ok) | [Коллекции и преобразование данных](#chapter-collections) |
| [`collections/frequencies`](../std/collections/frequencies.ok) | [Коллекции и преобразование данных](#chapter-collections) |
| [`collections/heap`](../std/collections/heap.ok) | [Коллекции и преобразование данных](#chapter-collections) |
| [`collections/iteration`](../std/collections/iteration.ok) | [Коллекции и преобразование данных](#chapter-collections) |
| [`collections/list`](../std/collections/list.ok) | [Коллекции и преобразование данных](#chapter-collections) |
| [`collections/ordering`](../std/collections/ordering.ok) | [Коллекции и преобразование данных](#chapter-collections) |
| [`collections/repetition`](../std/collections/repetition.ok) | [Коллекции и преобразование данных](#chapter-collections) |
| [`collections/sequence`](../std/collections/sequence.ok) | [Коллекции и преобразование данных](#chapter-collections) |
| [`collections/set`](../std/collections/set.ok) | [Коллекции и преобразование данных](#chapter-collections) |
| [`collections/views`](../std/collections/views.ok) | [Коллекции и преобразование данных](#chapter-collections) |
| [`compare`](../std/compare.ok) | [Общие контракты и предметные значения](#chapter-basics) |
| [`compression/read`](../std/compression/read.ok) | [Архивы и потоковые преобразования](#chapter-archives) |
| [`compression/state`](../std/compression/state.ok) | [Архивы и потоковые преобразования](#chapter-archives) |
| [`config/document`](../std/config/document.ok) | [Командная строка и конфигурация](#chapter-config) |
| [`config/environment`](../std/config/environment.ok) | [Командная строка и конфигурация](#chapter-config) |
| [`config/ini`](../std/config/ini.ok) | [Командная строка и конфигурация](#chapter-config) |
| [`config/key`](../std/config/key.ok) | [Командная строка и конфигурация](#chapter-config) |
| [`config/source`](../std/config/source.ok) | [Командная строка и конфигурация](#chapter-config) |
| [`config/value`](../std/config/value.ok) | [Командная строка и конфигурация](#chapter-config) |
| [`crypto/key`](../std/crypto/key.ok) | [Криптография, сертификаты и контроль целостности](#chapter-crypto) |
| [`crypto/secret`](../std/crypto/secret.ok) | [Криптография, сертификаты и контроль целостности](#chapter-crypto) |
| [`crypto/x509/certificate`](../std/crypto/x509/certificate.ok) | [Криптография, сертификаты и контроль целостности](#chapter-crypto) |
| [`crypto/x509/name`](../std/crypto/x509/name.ok) | [Криптография, сертификаты и контроль целостности](#chapter-crypto) |
| [`crypto/x509/trust`](../std/crypto/x509/trust.ok) | [Криптография, сертификаты и контроль целостности](#chapter-crypto) |
| [`crypto/x509/value`](../std/crypto/x509/value.ok) | [Криптография, сертификаты и контроль целостности](#chapter-crypto) |
| [`csv/input`](../std/csv/input.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`csv/stream`](../std/csv/stream.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`csv`](../std/csv.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`db/acquisition`](../std/db/acquisition.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`db/connection`](../std/db/connection.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`db/error`](../std/db/error.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`db/pool`](../std/db/pool.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`db/prepared`](../std/db/prepared.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`db/read`](../std/db/read.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`db/row`](../std/db/row.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`db/sqlite/backup`](../std/db/sqlite/backup.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`db/sqlite/blob`](../std/db/sqlite/blob.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`db/sqlite/script`](../std/db/sqlite/script.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`db/sqlite`](../std/db/sqlite.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`db/statement`](../std/db/statement.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`db/transaction`](../std/db/transaction.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`db/value`](../std/db/value.ok) | [Базы данных: SQL, SQLite, транзакции и пул](#chapter-database) |
| [`diff`](../std/diff.ok) | [Тесты, свойства, снимки и отчёты](#chapter-testing) |
| [`digest/blocks`](../std/digest/blocks.ok) | [Криптография, сертификаты и контроль целостности](#chapter-crypto) |
| [`digest/legacy`](../std/digest/legacy.ok) | [Криптография, сертификаты и контроль целостности](#chapter-crypto) |
| [`digest/state`](../std/digest/state.ok) | [Криптография, сертификаты и контроль целостности](#chapter-crypto) |
| [`encoding/base32`](../std/encoding/base32.ok) | [Кодировки и двоичные форматы](#chapter-encoding) |
| [`encoding/base64`](../std/encoding/base64.ok) | [Кодировки и двоичные форматы](#chapter-encoding) |
| [`encoding/base85`](../std/encoding/base85.ok) | [Кодировки и двоичные форматы](#chapter-encoding) |
| [`encoding/binary`](../std/encoding/binary.ok) | [Кодировки и двоичные форматы](#chapter-encoding) |
| [`encoding/hex`](../std/encoding/hex.ok) | [Кодировки и двоичные форматы](#chapter-encoding) |
| [`encoding/pem`](../std/encoding/pem.ok) | [Кодировки и двоичные форматы](#chapter-encoding) |
| [`encoding/quoted_printable`](../std/encoding/quoted_printable.ok) | [Кодировки и двоичные форматы](#chapter-encoding) |
| [`encoding/single_byte`](../std/encoding/single_byte.ok) | [Кодировки и двоичные форматы](#chapter-encoding) |
| [`encoding/utf`](../std/encoding/utf.ok) | [Кодировки и двоичные форматы](#chapter-encoding) |
| [`error`](../std/error.ok) | [Общие контракты и предметные значения](#chapter-basics) |
| [`fs/directory`](../std/fs/directory.ok) | [Файлы, каталоги и наблюдение изменений](#chapter-files) |
| [`fs/file`](../std/fs/file.ok) | [Файлы, каталоги и наблюдение изменений](#chapter-files) |
| [`fs/glob`](../std/fs/glob.ok) | [Файлы, каталоги и наблюдение изменений](#chapter-files) |
| [`fs/link`](../std/fs/link.ok) | [Файлы, каталоги и наблюдение изменений](#chapter-files) |
| [`fs/lock`](../std/fs/lock.ok) | [Файлы, каталоги и наблюдение изменений](#chapter-files) |
| [`fs/path`](../std/fs/path.ok) | [Файлы, каталоги и наблюдение изменений](#chapter-files) |
| [`fs/root`](../std/fs/root.ok) | [Файлы, каталоги и наблюдение изменений](#chapter-files) |
| [`fs/stat`](../std/fs/stat.ok) | [Файлы, каталоги и наблюдение изменений](#chapter-files) |
| [`fs/temp`](../std/fs/temp.ok) | [Файлы, каталоги и наблюдение изменений](#chapter-files) |
| [`fs/watch`](../std/fs/watch.ok) | [Файлы, каталоги и наблюдение изменений](#chapter-files) |
| [`html/entities`](../std/html/entities.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`html/events`](../std/html/events.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`http/auth`](../std/http/auth.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/auth_challenge`](../std/http/auth_challenge.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/body`](../std/http/body.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`http/cache`](../std/http/cache.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/cache_admission`](../std/http/cache_admission.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/cache_control`](../std/http/cache_control.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/cache_freshness`](../std/http/cache_freshness.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/cache_memory`](../std/http/cache_memory.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/cache_validation`](../std/http/cache_validation.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/client`](../std/http/client.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/client_policy`](../std/http/client_policy.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/client_protocols`](../std/http/client_protocols.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/client_request`](../std/http/client_request.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/conditional`](../std/http/conditional.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`http/connection`](../std/http/connection.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/content_coding`](../std/http/content_coding.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`http/conversation`](../std/http/conversation.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/cookie`](../std/http/cookie.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/cookie_client`](../std/http/cookie_client.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/cookie_date`](../std/http/cookie_date.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/cookie_jar`](../std/http/cookie_jar.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/cookie_policy`](../std/http/cookie_policy.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/date`](../std/http/date.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`http/digest`](../std/http/digest.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/endpoint`](../std/http/endpoint.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/environment_proxy`](../std/http/environment_proxy.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/exchange`](../std/http/exchange.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/form`](../std/http/form.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`http/handler`](../std/http/handler.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`http/header`](../std/http/header.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`http/http1_body`](../std/http/http1_body.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http1_body_check`](../std/http/http1_body_check.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http1_body_writing`](../std/http/http1_body_writing.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http1_chunk`](../std/http/http1_chunk.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http1_continue`](../std/http/http1_continue.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http1_framing`](../std/http/http1_framing.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http1_head`](../std/http/http1_head.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http1_message_writing`](../std/http/http1_message_writing.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http1_reading`](../std/http/http1_reading.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http1_session`](../std/http/http1_session.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http1_writing`](../std/http/http1_writing.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http2_connection`](../std/http/http2_connection.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http2_server`](../std/http/http2_server.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http2_session`](../std/http/http2_session.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http2_settings`](../std/http/http2_settings.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/http2_tls`](../std/http/http2_tls.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/internal/hpack`](../std/http/internal/hpack.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/hpack_primitives`](../std/http/internal/hpack_primitives.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/hpack_static`](../std/http/internal/hpack_static.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/hpack_tables`](../std/http/internal/hpack_tables.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_body`](../std/http/internal/http2_body.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_controls`](../std/http/internal/http2_controls.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_fields`](../std/http/internal/http2_fields.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_flow`](../std/http/internal/http2_flow.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_frame`](../std/http/internal/http2_frame.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_headers`](../std/http/internal/http2_headers.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_input`](../std/http/internal/http2_input.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_life`](../std/http/internal/http2_life.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_output`](../std/http/internal/http2_output.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_preface`](../std/http/internal/http2_preface.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_reading`](../std/http/internal/http2_reading.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_receiving`](../std/http/internal/http2_receiving.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_sending`](../std/http/internal/http2_sending.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/http2_settings`](../std/http/internal/http2_settings.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/trailers`](../std/http/internal/trailers.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/websocket_frame`](../std/http/internal/websocket_frame.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/websocket_messages`](../std/http/internal/websocket_messages.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/websocket_output`](../std/http/internal/websocket_output.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/websocket_reading`](../std/http/internal/websocket_reading.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/websocket_upgrade`](../std/http/internal/websocket_upgrade.ok) | [Внутренние модули](#internal-modules) |
| [`http/internal/wire_bytes`](../std/http/internal/wire_bytes.ok) | [Внутренние модули](#internal-modules) |
| [`http/media_type`](../std/http/media_type.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`http/message`](../std/http/message.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`http/method`](../std/http/method.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`http/negotiation`](../std/http/negotiation.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`http/proxy`](../std/http/proxy.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/proxy_connection`](../std/http/proxy_connection.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/proxy_policy`](../std/http/proxy_policy.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/proxy_request`](../std/http/proxy_request.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/proxy_tunnel`](../std/http/proxy_tunnel.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/redirect`](../std/http/redirect.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/request_context`](../std/http/request_context.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`http/retry`](../std/http/retry.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/robots`](../std/http/robots.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/routing`](../std/http/routing.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`http/selected_connection`](../std/http/selected_connection.ok) | [HTTP-клиент: соединения, авторизация, proxy, cookies и кеш](#chapter-http-client) |
| [`http/server`](../std/http/server.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`http/server_policy`](../std/http/server_policy.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`http/sse`](../std/http/sse.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`http/static`](../std/http/static.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`http/status`](../std/http/status.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`http/target`](../std/http/target.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`http/tls_session`](../std/http/tls_session.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/transport_observation`](../std/http/transport_observation.ok) | [HTTP/1 и HTTP/2: протокольные слои](#chapter-http-wire) |
| [`http/version`](../std/http/version.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`http/websocket`](../std/http/websocket.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`http/websocket_client`](../std/http/websocket_client.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`http/websocket_handshake`](../std/http/websocket_handshake.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`http/websocket_origin`](../std/http/websocket_origin.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`http/websocket_server`](../std/http/websocket_server.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`http/websocket_session`](../std/http/websocket_session.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`http/xmlrpc`](../std/http/xmlrpc.ok) | [HTTP: сообщения, поля и представление данных](#chapter-http-values) |
| [`i18n/catalog`](../std/i18n/catalog.ok) | [Локализация и переводы](#chapter-locale) |
| [`i18n/gettext`](../std/i18n/gettext.ok) | [Локализация и переводы](#chapter-locale) |
| [`i18n/operands`](../std/i18n/operands.ok) | [Локализация и переводы](#chapter-locale) |
| [`i18n/plural`](../std/i18n/plural.ok) | [Локализация и переводы](#chapter-locale) |
| [`io/buffer`](../std/io/buffer.ok) | [IO: потоки, буферы и консоль](#chapter-io) |
| [`io/decorators`](../std/io/decorators.ok) | [IO: потоки, буферы и консоль](#chapter-io) |
| [`io/stdin`](../std/io/stdin.ok) | [IO: потоки, буферы и консоль](#chapter-io) |
| [`io/stdout`](../std/io/stdout.ok) | [IO: потоки, буферы и консоль](#chapter-io) |
| [`io/stream`](../std/io/stream.ok) | [IO: потоки, буферы и консоль](#chapter-io) |
| [`io/text`](../std/io/text.ok) | [IO: потоки, буферы и консоль](#chapter-io) |
| [`json/parse`](../std/json/parse.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`json/read`](../std/json/read.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`json/value`](../std/json/value.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`json/write`](../std/json/write.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`locale/collation`](../std/locale/collation.ok) | [Локализация и переводы](#chapter-locale) |
| [`locale/currency`](../std/locale/currency.ok) | [Локализация и переводы](#chapter-locale) |
| [`locale/data`](../std/locale/data.ok) | [Локализация и переводы](#chapter-locale) |
| [`locale/date_time`](../std/locale/date_time.ok) | [Локализация и переводы](#chapter-locale) |
| [`locale/internal/currencies`](../std/locale/internal/currencies.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_0`](../std/locale/internal/currency_names_0.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_1`](../std/locale/internal/currency_names_1.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_10`](../std/locale/internal/currency_names_10.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_11`](../std/locale/internal/currency_names_11.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_12`](../std/locale/internal/currency_names_12.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_13`](../std/locale/internal/currency_names_13.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_14`](../std/locale/internal/currency_names_14.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_15`](../std/locale/internal/currency_names_15.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_16`](../std/locale/internal/currency_names_16.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_17`](../std/locale/internal/currency_names_17.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_18`](../std/locale/internal/currency_names_18.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_19`](../std/locale/internal/currency_names_19.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_2`](../std/locale/internal/currency_names_2.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_20`](../std/locale/internal/currency_names_20.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_21`](../std/locale/internal/currency_names_21.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_22`](../std/locale/internal/currency_names_22.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_23`](../std/locale/internal/currency_names_23.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_24`](../std/locale/internal/currency_names_24.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_25`](../std/locale/internal/currency_names_25.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_26`](../std/locale/internal/currency_names_26.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_27`](../std/locale/internal/currency_names_27.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_3`](../std/locale/internal/currency_names_3.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_4`](../std/locale/internal/currency_names_4.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_5`](../std/locale/internal/currency_names_5.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_6`](../std/locale/internal/currency_names_6.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_7`](../std/locale/internal/currency_names_7.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_8`](../std/locale/internal/currency_names_8.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_names_9`](../std/locale/internal/currency_names_9.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/currency_rows`](../std/locale/internal/currency_rows.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/day_periods`](../std/locale/internal/day_periods.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/gregorian`](../std/locale/internal/gregorian.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/iana`](../std/locale/internal/iana.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/likely`](../std/locale/internal/likely.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/lists`](../std/locale/internal/lists.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/numbers`](../std/locale/internal/numbers.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/parents`](../std/locale/internal/parents.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/plurals`](../std/locale/internal/plurals.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_meta`](../std/locale/internal/zone_meta.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_name_rows`](../std/locale/internal/zone_name_rows.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names`](../std/locale/internal/zone_names.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_0`](../std/locale/internal/zone_names_0.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_1`](../std/locale/internal/zone_names_1.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_10`](../std/locale/internal/zone_names_10.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_11`](../std/locale/internal/zone_names_11.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_12`](../std/locale/internal/zone_names_12.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_13`](../std/locale/internal/zone_names_13.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_14`](../std/locale/internal/zone_names_14.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_15`](../std/locale/internal/zone_names_15.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_16`](../std/locale/internal/zone_names_16.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_17`](../std/locale/internal/zone_names_17.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_18`](../std/locale/internal/zone_names_18.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_19`](../std/locale/internal/zone_names_19.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_2`](../std/locale/internal/zone_names_2.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_20`](../std/locale/internal/zone_names_20.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_21`](../std/locale/internal/zone_names_21.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_22`](../std/locale/internal/zone_names_22.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_23`](../std/locale/internal/zone_names_23.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_24`](../std/locale/internal/zone_names_24.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_25`](../std/locale/internal/zone_names_25.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_26`](../std/locale/internal/zone_names_26.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_27`](../std/locale/internal/zone_names_27.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_3`](../std/locale/internal/zone_names_3.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_4`](../std/locale/internal/zone_names_4.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_5`](../std/locale/internal/zone_names_5.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_6`](../std/locale/internal/zone_names_6.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_7`](../std/locale/internal/zone_names_7.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_8`](../std/locale/internal/zone_names_8.ok) | [Внутренние модули](#internal-modules) |
| [`locale/internal/zone_names_9`](../std/locale/internal/zone_names_9.ok) | [Внутренние модули](#internal-modules) |
| [`locale/likely`](../std/locale/likely.ok) | [Локализация и переводы](#chapter-locale) |
| [`locale/list`](../std/locale/list.ok) | [Локализация и переводы](#chapter-locale) |
| [`locale/match`](../std/locale/match.ok) | [Локализация и переводы](#chapter-locale) |
| [`locale/number`](../std/locale/number.ok) | [Локализация и переводы](#chapter-locale) |
| [`locale/system`](../std/locale/system.ok) | [Локализация и переводы](#chapter-locale) |
| [`locale/tag`](../std/locale/tag.ok) | [Локализация и переводы](#chapter-locale) |
| [`logging/enrichment`](../std/logging/enrichment.ok) | [Журналы событий](#chapter-logging) |
| [`logging/field`](../std/logging/field.ok) | [Журналы событий](#chapter-logging) |
| [`logging/file`](../std/logging/file.ok) | [Журналы событий](#chapter-logging) |
| [`logging/filter`](../std/logging/filter.ok) | [Журналы событий](#chapter-logging) |
| [`logging/level`](../std/logging/level.ok) | [Журналы событий](#chapter-logging) |
| [`logging/line`](../std/logging/line.ok) | [Журналы событий](#chapter-logging) |
| [`logging/name`](../std/logging/name.ok) | [Журналы событий](#chapter-logging) |
| [`logging/named`](../std/logging/named.ok) | [Журналы событий](#chapter-logging) |
| [`logging/queue`](../std/logging/queue.ok) | [Журналы событий](#chapter-logging) |
| [`logging/record`](../std/logging/record.ok) | [Журналы событий](#chapter-logging) |
| [`logging/redact`](../std/logging/redact.ok) | [Журналы событий](#chapter-logging) |
| [`logging/rotation`](../std/logging/rotation.ok) | [Журналы событий](#chapter-logging) |
| [`logging/stream`](../std/logging/stream.ok) | [Журналы событий](#chapter-logging) |
| [`logging/syslog`](../std/logging/syslog.ok) | [Журналы событий](#chapter-logging) |
| [`logging/write`](../std/logging/write.ok) | [Журналы событий](#chapter-logging) |
| [`mail/imap`](../std/mail/imap.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_changes`](../std/mail/imap_changes.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_command`](../std/mail/imap_command.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_command_reading`](../std/mail/imap_command_reading.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_data`](../std/mail/imap_data.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_exchange`](../std/mail/imap_exchange.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_message`](../std/mail/imap_message.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_metadata`](../std/mail/imap_metadata.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_name`](../std/mail/imap_name.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_reading`](../std/mail/imap_reading.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_response`](../std/mail/imap_response.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_search`](../std/mail/imap_search.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_session`](../std/mail/imap_session.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_state`](../std/mail/imap_state.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/imap_uid`](../std/mail/imap_uid.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/mailbox`](../std/mail/mailbox.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/mbox`](../std/mail/mbox.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/mbox_store`](../std/mail/mbox_store.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/message`](../std/mail/message.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/pop3`](../std/mail/pop3.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/pop3_reading`](../std/mail/pop3_reading.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/pop3_reply`](../std/mail/pop3_reply.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/pop3_session`](../std/mail/pop3_session.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/smtp`](../std/mail/smtp.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/smtp_data`](../std/mail/smtp_data.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/smtp_reading`](../std/mail/smtp_reading.ok) | [Почта и MIME](#chapter-mail) |
| [`mail/smtp_reply`](../std/mail/smtp_reply.ok) | [Почта и MIME](#chapter-mail) |
| [`math/aggregate`](../std/math/aggregate.ok) | [Числа, деньги и статистика](#chapter-math) |
| [`math/constants`](../std/math/constants.ok) | [Числа, деньги и статистика](#chapter-math) |
| [`math/decimal`](../std/math/decimal.ok) | [Числа, деньги и статистика](#chapter-math) |
| [`math/rounding`](../std/math/rounding.ok) | [Числа, деньги и статистика](#chapter-math) |
| [`message/header`](../std/message/header.ok) | [Почта и MIME](#chapter-mail) |
| [`mime/disposition`](../std/mime/disposition.ok) | [Почта и MIME](#chapter-mail) |
| [`mime/media_type`](../std/mime/media_type.ok) | [Почта и MIME](#chapter-mail) |
| [`mime/part`](../std/mime/part.ok) | [Почта и MIME](#chapter-mail) |
| [`mime/transfer`](../std/mime/transfer.ok) | [Почта и MIME](#chapter-mail) |
| [`mime/types`](../std/mime/types.ok) | [Почта и MIME](#chapter-mail) |
| [`mime/word`](../std/mime/word.ok) | [Почта и MIME](#chapter-mail) |
| [`net/dns`](../std/net/dns.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`net/endpoint`](../std/net/endpoint.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`net/failure`](../std/net/failure.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`net/interface`](../std/net/interface.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`net/ip`](../std/net/ip.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`net/ip_registry`](../std/net/ip_registry.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`net/multicast`](../std/net/multicast.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`net/public_suffix`](../std/net/public_suffix.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`net/stream`](../std/net/stream.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`net/tcp`](../std/net/tcp.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`net/tcp_racing`](../std/net/tcp_racing.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`net/tls`](../std/net/tls.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`net/udp`](../std/net/udp.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`net/unix`](../std/net/unix.ok) | [Сети: адреса, DNS, TCP, UDP, Unix и TLS](#chapter-network) |
| [`numbers/bigint`](../std/numbers/bigint.ok) | [Числа, деньги и статистика](#chapter-math) |
| [`numbers/bigrational`](../std/numbers/bigrational.ok) | [Числа, деньги и статистика](#chapter-math) |
| [`numbers/float`](../std/numbers/float.ok) | [Числа, деньги и статистика](#chapter-math) |
| [`os/account`](../std/os/account.ok) | [Операционная система и внешние программы](#chapter-system) |
| [`os/command`](../std/os/command.ok) | [Операционная система и внешние программы](#chapter-system) |
| [`os/command_capture`](../std/os/command_capture.ok) | [Операционная система и внешние программы](#chapter-system) |
| [`os/directory`](../std/os/directory.ok) | [Операционная система и внешние программы](#chapter-system) |
| [`os/environment`](../std/os/environment.ok) | [Операционная система и внешние программы](#chapter-system) |
| [`os/pipeline_capture`](../std/os/pipeline_capture.ok) | [Операционная система и внешние программы](#chapter-system) |
| [`os/platform`](../std/os/platform.ok) | [Операционная система и внешние программы](#chapter-system) |
| [`os/process`](../std/os/process.ok) | [Операционная система и внешние программы](#chapter-system) |
| [`os/storage`](../std/os/storage.ok) | [Операционная система и внешние программы](#chapter-system) |
| [`parallel/map`](../std/parallel/map.ok) | [Асинхронность и параллельная обработка](#chapter-async) |
| [`plist/parse`](../std/plist/parse.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`plist/value`](../std/plist/value.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`plist/write`](../std/plist/write.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`random/distribution`](../std/random/distribution.ok) | [Случайность и идентификаторы](#chapter-random) |
| [`random/source`](../std/random/source.ok) | [Случайность и идентификаторы](#chapter-random) |
| [`regex/escape`](../std/regex/escape.ok) | [Строки, Unicode и регулярные выражения](#chapter-text) |
| [`regex/match`](../std/regex/match.ok) | [Строки, Unicode и регулярные выражения](#chapter-text) |
| [`regex/pattern`](../std/regex/pattern.ok) | [Строки, Unicode и регулярные выражения](#chapter-text) |
| [`regex/range`](../std/regex/range.ok) | [Строки, Unicode и регулярные выражения](#chapter-text) |
| [`regex/replace`](../std/regex/replace.ok) | [Строки, Unicode и регулярные выражения](#chapter-text) |
| [`regex/search`](../std/regex/search.ok) | [Строки, Unicode и регулярные выражения](#chapter-text) |
| [`regex/split`](../std/regex/split.ok) | [Строки, Unicode и регулярные выражения](#chapter-text) |
| [`runtime/account`](../std/runtime/account.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/application`](../std/runtime/application.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/base32`](../std/runtime/base32.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/base64`](../std/runtime/base64.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/base85`](../std/runtime/base85.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/bigint`](../std/runtime/bigint.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/binary`](../std/runtime/binary.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/buffer`](../std/runtime/buffer.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/collation`](../std/runtime/collation.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/collections`](../std/runtime/collections.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/crypto`](../std/runtime/crypto.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/csv_state`](../std/runtime/csv_state.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/database`](../std/runtime/database.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/debug`](../std/runtime/debug.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/decimal`](../std/runtime/decimal.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/diff`](../std/runtime/diff.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/environment`](../std/runtime/environment.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/format`](../std/runtime/format.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/fs`](../std/runtime/fs.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/fs_root`](../std/runtime/fs_root.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/hex`](../std/runtime/hex.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/hpack`](../std/runtime/hpack.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/html`](../std/runtime/html.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/idna`](../std/runtime/idna.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/ini`](../std/runtime/ini.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/input_state`](../std/runtime/input_state.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/io_decorators`](../std/runtime/io_decorators.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/json`](../std/runtime/json.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/layout`](../std/runtime/layout.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/locale`](../std/runtime/locale.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/math_aggregate`](../std/runtime/math_aggregate.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/net`](../std/runtime/net.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/network`](../std/runtime/network.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/ordering`](../std/runtime/ordering.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/pem`](../std/runtime/pem.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/process`](../std/runtime/process.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/public_suffix`](../std/runtime/public_suffix.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/quoted_printable`](../std/runtime/quoted_printable.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/random`](../std/runtime/random.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/random_distribution`](../std/runtime/random_distribution.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/regex`](../std/runtime/regex.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/secret`](../std/runtime/secret.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/single_byte`](../std/runtime/single_byte.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/snapshot`](../std/runtime/snapshot.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/statistics`](../std/runtime/statistics.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/stdio`](../std/runtime/stdio.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/system`](../std/runtime/system.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/template`](../std/runtime/template.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/testing`](../std/runtime/testing.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/time`](../std/runtime/time.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/time_format`](../std/runtime/time_format.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/time_zone`](../std/runtime/time_zone.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/tls_transport`](../std/runtime/tls_transport.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/toml`](../std/runtime/toml.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/unicode_character`](../std/runtime/unicode_character.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/unicode_normalization`](../std/runtime/unicode_normalization.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/url_text`](../std/runtime/url_text.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/utf`](../std/runtime/utf.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/uuid`](../std/runtime/uuid.ok) | [Граница с runtime](#runtime-modules) |
| [`runtime/x509`](../std/runtime/x509.ok) | [Граница с runtime](#runtime-modules) |
| [`size`](../std/size.ok) | [Общие контракты и предметные значения](#chapter-basics) |
| [`statistics`](../std/statistics.ok) | [Числа, деньги и статистика](#chapter-math) |
| [`strings/format`](../std/strings/format.ok) | [Строки, Unicode и регулярные выражения](#chapter-text) |
| [`strings/join`](../std/strings/join.ok) | [Строки, Unicode и регулярные выражения](#chapter-text) |
| [`strings/parse`](../std/strings/parse.ok) | [Строки, Unicode и регулярные выражения](#chapter-text) |
| [`template/compile`](../std/template/compile.ok) | [Шаблоны писем, страниц и отчётов](#chapter-templates) |
| [`template/html`](../std/template/html.ok) | [Шаблоны писем, страниц и отчётов](#chapter-templates) |
| [`template/html_library`](../std/template/html_library.ok) | [Шаблоны писем, страниц и отчётов](#chapter-templates) |
| [`template/json`](../std/template/json.ok) | [Шаблоны писем, страниц и отчётов](#chapter-templates) |
| [`template/library`](../std/template/library.ok) | [Шаблоны писем, страниц и отчётов](#chapter-templates) |
| [`template/model`](../std/template/model.ok) | [Шаблоны писем, страниц и отчётов](#chapter-templates) |
| [`template/path`](../std/template/path.ok) | [Шаблоны писем, страниц и отчётов](#chapter-templates) |
| [`template/render`](../std/template/render.ok) | [Шаблоны писем, страниц и отчётов](#chapter-templates) |
| [`template/sections`](../std/template/sections.ok) | [Шаблоны писем, страниц и отчётов](#chapter-templates) |
| [`template/tree`](../std/template/tree.ok) | [Шаблоны писем, страниц и отчётов](#chapter-templates) |
| [`terminal/capability`](../std/terminal/capability.ok) | [Интерактивный терминал](#chapter-terminal) |
| [`terminal/error`](../std/terminal/error.ok) | [Интерактивный терминал](#chapter-terminal) |
| [`terminal/mode`](../std/terminal/mode.ok) | [Интерактивный терминал](#chapter-terminal) |
| [`terminal/prompt`](../std/terminal/prompt.ok) | [Интерактивный терминал](#chapter-terminal) |
| [`terminal/screen`](../std/terminal/screen.ok) | [Интерактивный терминал](#chapter-terminal) |
| [`terminal/session`](../std/terminal/session.ok) | [Интерактивный терминал](#chapter-terminal) |
| [`terminal/style`](../std/terminal/style.ok) | [Интерактивный терминал](#chapter-terminal) |
| [`terminal/width`](../std/terminal/width.ok) | [Интерактивный терминал](#chapter-terminal) |
| [`testing/check`](../std/testing/check.ok) | [Тесты, свойства, снимки и отчёты](#chapter-testing) |
| [`testing/context`](../std/testing/context.ok) | [Тесты, свойства, снимки и отчёты](#chapter-testing) |
| [`testing/double`](../std/testing/double.ok) | [Тесты, свойства, снимки и отчёты](#chapter-testing) |
| [`testing/error`](../std/testing/error.ok) | [Тесты, свойства, снимки и отчёты](#chapter-testing) |
| [`testing/generation`](../std/testing/generation.ok) | [Тесты, свойства, снимки и отчёты](#chapter-testing) |
| [`testing/property`](../std/testing/property.ok) | [Тесты, свойства, снимки и отчёты](#chapter-testing) |
| [`testing/report`](../std/testing/report.ok) | [Тесты, свойства, снимки и отчёты](#chapter-testing) |
| [`testing/result`](../std/testing/result.ok) | [Тесты, свойства, снимки и отчёты](#chapter-testing) |
| [`testing/runner`](../std/testing/runner.ok) | [Тесты, свойства, снимки и отчёты](#chapter-testing) |
| [`testing/snapshot`](../std/testing/snapshot.ok) | [Тесты, свойства, снимки и отчёты](#chapter-testing) |
| [`testing/time`](../std/testing/time.ok) | [Тесты, свойства, снимки и отчёты](#chapter-testing) |
| [`text/layout`](../std/text/layout.ok) | [Строки, Unicode и регулярные выражения](#chapter-text) |
| [`time/clock`](../std/time/clock.ok) | [Даты, время, часовые пояса и календарь](#chapter-time) |
| [`time/date`](../std/time/date.ok) | [Даты, время, часовые пояса и календарь](#chapter-time) |
| [`time/day`](../std/time/day.ok) | [Даты, время, часовые пояса и календарь](#chapter-time) |
| [`time/duration`](../std/time/duration.ok) | [Даты, время, часовые пояса и календарь](#chapter-time) |
| [`time/format`](../std/time/format.ok) | [Даты, время, часовые пояса и календарь](#chapter-time) |
| [`time/iso`](../std/time/iso.ok) | [Даты, время, часовые пояса и календарь](#chapter-time) |
| [`time/local_datetime`](../std/time/local_datetime.ok) | [Даты, время, часовые пояса и календарь](#chapter-time) |
| [`time/rfc3339`](../std/time/rfc3339.ok) | [Даты, время, часовые пояса и календарь](#chapter-time) |
| [`time/zone`](../std/time/zone.ok) | [Даты, время, часовые пояса и календарь](#chapter-time) |
| [`toml/parse`](../std/toml/parse.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`toml/value`](../std/toml/value.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`unicode/character`](../std/unicode/character.ok) | [Строки, Unicode и регулярные выражения](#chapter-text) |
| [`unicode/normalization`](../std/unicode/normalization.ok) | [Строки, Unicode и регулярные выражения](#chapter-text) |
| [`url/form`](../std/url/form.ok) | [URL, параметры запроса и формы](#chapter-url) |
| [`url/percent`](../std/url/percent.ok) | [URL, параметры запроса и формы](#chapter-url) |
| [`url/query`](../std/url/query.ok) | [URL, параметры запроса и формы](#chapter-url) |
| [`url/value`](../std/url/value.ok) | [URL, параметры запроса и формы](#chapter-url) |
| [`uuid/generation`](../std/uuid/generation.ok) | [Случайность и идентификаторы](#chapter-random) |
| [`uuid/value`](../std/uuid/value.ok) | [Случайность и идентификаторы](#chapter-random) |
| [`web/app`](../std/web/app.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`web/exception`](../std/web/exception.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`web/internal/reply`](../std/web/internal/reply.ok) | [Внутренние модули](#internal-modules) |
| [`web/reply`](../std/web/reply.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`web/request`](../std/web/request.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`web/routing`](../std/web/routing.ok) | [HTTP-сервер, веб-приложение и потоковые события](#chapter-web) |
| [`xml/name`](../std/xml/name.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`xml/node`](../std/xml/node.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |
| [`xml/write`](../std/xml/write.ok) | [JSON, CSV, TOML, plist, XML и HTML](#chapter-formats) |

<a id="verification"></a>

## Приложение D. Проверка примеров

Все пути модулей сопоставлены с файлами `std/`; прикладному модулю назначены задача и рецепт. Полные блоки книги проверены лексером, синтаксическим и семантическим анализаторами Okkama в контексте установленного `std`. Локальные программы дополнительно проверены запуском; интерактивные, серверные и стендовые рецепты имеют описанные входы.

Проверено на локальном срезе 18 октября 2026 года, Perl 5.42.2.

| Проверка | Результат |
|---|---|
| Файлы `std/` и алфавитный указатель | 506 из 506 |
| Прикладные задачи с переходом к рецепту | 351 из 351 |
| Внутренние и runtime-модули с назначением и связями | 155 из 155 |
| Полные Okkama-блоки: синтаксический и семантический анализ | 158 из 158 |
| Выполненные локальные программы | 111 из 111 |
| Локальные ссылки и якоря | 3241, все корректны |

Запуски включают текст и Unicode, persistent-коллекции, regex, числовые представления, CSV/JSON/TOML/plist/XML, шаблоны, архивные потоки, MIME, локализацию, журналирование и тестовые инструменты. В отдельных примерах `confirm` и `test` проверяют результат, сохранение прежних значений и завершение ресурсов. Сетевые стенды и интерактивный ввод задаются в условиях соответствующего рецепта.
