# Коллекции Okkama: подробное руководство

Этот текст объясняет пользовательскую модель модулей `collections/*`: как обходить
данные, когда можно обращаться по индексу, какие преобразования ленивы и почему
большинство операций возвращает новый объект вместо изменения старого.

Примеры соответствуют текущему API в `std/collections`. Внутренние курсоры,
поисковые объекты и детали представления в справочник не включены: прикладной код
не должен создавать их напрямую.

## 1. Самая важная идея

В Okkama нет одного универсального типа «коллекция». Есть несколько контрактов с
разными обещаниями.

| Контракт | Что обещает | Чего не обещает |
|---|---|---|
| `Iterating<T>` | можно начать последовательный обход | размер, повторный обход, индекс |
| `Cursor<T>` | можно посмотреть текущий элемент и получить следующий курсор | мутацию позиции |
| `Sizing` | известен `size()` | обход или индекс |
| `Sequencing<T>` | обход, размер, `item(index)`, `first()`, `last()` | дешёвую вставку |
| `List<T>` | голова, хвост и дешёвый `prefix()` | случайный доступ |
| `Set<T>` | уникальность и проверку принадлежности | сортировку |

Правило выбора простое:

- функция только читает элементы по порядку — принимайте `Iterating<T>`;
- нужен индекс или известная длина — принимайте `Sequencing<T>`;
- нужен persistent-список с дешёвой головой — принимайте `List<T>`;
- нужна уникальность — принимайте `Set<T>`.

Не требуйте более сильный контракт «на всякий случай». Это оставляет вызывающему
коду больше свободы.

## 2. Неизменяемость и ленивость

Коллекционные декораторы не меняют источник:

```okkama
let positives = Filtered(origin: numbers, condition: (number is int) is bool {
    number > 0;
});
let doubled = Mapped(origin: positives, mapping: (number is int) is int {
    number * 2;
});
```

`numbers` остаётся прежним. `positives` и `doubled` описывают вычисление. Элементы
запрашиваются только при обходе. Это позволяет работать с большими и даже
бесконечными источниками, но означает, что ошибка внутри `mapping` может возникнуть
не в конструкторе `Mapped`, а во время чтения элемента.

Материализующие операции отмечены отдельно. Например, `Sorted`, `OrderedSet` и
`Frequencies` обязаны прочитать весь вход, чтобы построить результат.

## 3. `collections/iteration`

### `Iterating<T>`

У объекта одна операция:

```okkama
cursor() is Cursor<T>
```

Обычный `for` является удобной формой обхода `Iterating`:

```okkama
for values -> value {
    Stdout().writeln(value.string());
}
```

### `Cursor<T>`

Курсор неизменяем. Он не «двигается»; `successor()` создаёт представление следующей
позиции:

```okkama
let first = values.cursor();
if first.available() {
    Stdout().writeln(first.item().string());
    let second = first.successor();
    // first по-прежнему указывает на первый элемент.
}
```

Контракт:

- сначала проверяйте `available()`;
- `item()` читает текущий элемент;
- `successor()` возвращает следующий курсор;
- `item()` и `successor()` на исчерпанном курсоре бросают `IterationEnded`.

`CursorItems(cursor: cursor)` адаптирует уже полученный курсор обратно к
`Iterating<T>`. Это полезно, когда первые элементы разобраны вручную, а остаток надо
передать общей функции.

## 4. `collections/sequence`

`Sequencing<T>` предоставляет пять операций:

```okkama
cursor() is Cursor<T>
size() is int
item(index is int) is T
first() is T
last() is T
```

Индексы нулевые. Выход за границы обычно приводит к `OutOfRange`; `first()` и
`last()` пустой последовательности — к `EmptySequence`.

Принимайте `Sequencing` только когда алгоритм действительно использует индекс,
размер либо оба конца. Для одного прохода достаточно `Iterating`.

## 5. `collections/enumeration`

`Enumerated<T>` добавляет к элементам номера, не материализуя вход.

```okkama
let numbered = Enumerated(origin: names, start: 1);
for numbered -> entry {
    Stdout().writeln(
        entry.index().string(),
        ". ",
        entry.item(),
    );
}
```

Без `start` нумерация начинается с нуля. Каждый элемент имеет тип `Indexed<T>` с
`index()` и `item()`. Переполнение `int` при переходе к следующему номеру приводит
к `EnumerationOverflow`.

Используйте модуль для номеров строк, позиций в отчёте и сохранения исходного
порядка после последующей сортировки.

## 6. `collections/combining`

### `Pair<A,B>`

Небольшое типизированное значение с `first()` и `second()`:

```okkama
let answer = Pair(first: "answer", second: 42);
```

### `Zipped<A,B>`

Обходит два источника одновременно и заканчивается вместе с более коротким:

```okkama
for Zipped(left: names, right: scores) -> pair {
    Stdout().writeln(pair.first(), ": ", pair.second().string());
}
```

Берите `Zipped`, только если лишний хвост допустимо игнорировать.

### `StrictlyZipped<A,B>`

Имеет ту же форму, но бросает `UnequalSizes`, когда закончился ровно один источник.
Это правильный выбор для связанных столбцов, координат и входных данных, где
различная длина означает повреждение.

### `Chained<T>`

Лениво разворачивает один уровень `Iterating<Iterating<T>>`:

```okkama
let rows = Chained(origins: pages);
for rows -> row {
    consume(row);
}
```

Пустые внутренние источники пропускаются. Более глубокие уровни автоматически не
разворачиваются.

## 7. `collections/views`

Это основной набор ленивых декораторов.

### Преобразование и фильтрация

```okkama
let names = Mapped<User,string>(
    origin: users,
    mapping: (user is User) is string { user.name(); },
);
let active = Filtered(
    origin: users,
    condition: (user is User) is bool { user.active(); },
);
```

`Mapped<A,B>` меняет тип каждого элемента. `Filtered<T>` сохраняет только элементы,
для которых условие вернуло `true`.

### Ограничение и пропуск

```okkama
let page = Taken(
    origin: Dropped(origin: rows, count: 100),
    count: 25,
);
```

`Taken` отдаёт не более `count` элементов, `Dropped` пропускает первые `count`.
Отрицательный `count` недопустим. Эти объекты не знают общей длины источника.

`TakenWhile` отдаёт элементы до первого ложного условия. `DroppedWhile` пропускает
начальный ложный префикс, после чего отдаёт весь остаток без повторной проверки:

```okkama
let headerless = DroppedWhile(origin: lines, condition: (line is string) is bool {
    line.starts_with("#");
});
```

### Порции и выравнивание

`Chunked(origin, size)` группирует элементы в массивы `@T`. Последняя порция может
быть короче; `size <= 0` запрещён.

```okkama
for Chunked(origin: records, size: 500) -> batch {
    database.insert(batch);
}
```

`Flattened` снимает ровно один уровень вложенности. `Pairwise` строит соседние пары:

```okkama
// [a,b,c] -> (a,b), (b,c)
for Pairwise(origin: points) -> edge {
    draw(edge.first(), edge.second());
}
```

Для пустого или одноэлементного входа `Pairwise` пуст.

### Накопление и свёртка

`Accumulated<T,R>` выдаёт промежуточные состояния и первым элементом обязательно
выдаёт явное начальное значение:

```okkama
let totals = Accumulated(
    origin: prices,
    initial: 0,
    action: (total is int, price is int) is int { total + price; },
);
// Для [2,3,5] получится [0,2,5,10].
```

`Reduced<T,R>` сворачивает весь источник в одно значение:

```okkama
let total is int = value Reduced(
    origin: prices,
    initial: 0,
    action: (sum is int, price is int) is int { sum + price; },
);
```

`Reduced` читает источник целиком при проекции `value` и потому не подходит для
бесконечного входа.

### Обратный порядок и срез

`Reversed<T>` требует `Sequencing<T>` и сам остаётся последовательностью. Он
перенаправляет индексы, а не обязан копировать элементы.

`Sliced(origin, start, stop, step)` — ленивый полуоткрытый срез. `start` и `stop`
неотрицательны, а `step` строго положителен. Текущая модель предназначена для прямого обхода; для обратного порядка
у индексируемого источника используйте `Reversed`.

### Цена цепочек

Цепочка декораторов удобна и сохраняет потоковость:

```okkama
let result = Taken(
    origin: Mapped(
        origin: Filtered(origin: source, condition: wanted),
        mapping: converted,
    ),
    count: 10,
);
```

Каждый элемент проходит через несколько объектов. Для горячего участка измеряйте
цепочку и сравнивайте с нативной bulk-операцией. Не материализуйте данные заранее
без измерения: это может оказаться значительно дороже по памяти.

## 8. `collections/repetition`

### `Repeating<T>`

Бесконечно повторяет одно значение. Его курсор всегда доступен:

```okkama
let first_hundred = Taken(origin: Repeating(value: 0), count: 100);
```

Никогда не сворачивайте `Repeating` без ограничения.

### `Repeated<T>`

Конечная индексируемая последовательность из `count` одинаковых значений:

```okkama
let padding = Repeated(value: " ", count: 4);
padding.size();
padding.item(2);
```

Отрицательный размер запрещён. У пустого результата `first()` и `last()` бросают
`EmptySequence`.

### `Cycled<T>`

Повторяет конечную `Sequencing<T>`. Пустой источник даёт пустой результат,
непустой — бесконечный:

```okkama
for Taken(origin: Cycled(origin: workers), count: jobs.size()) -> worker {
    assign(worker);
}
```

## 9. `collections/list`

`List<T>` — неизменяемый односвязный список. Он намеренно не имеет индекса.

```okkama
let empty is List<string> = EmptyList();
let one = empty.prefix("one");
let two = one.prefix("two");

two.first();         // "two"
two.tail().first();  // "one"
one.first();         // всё ещё "one"
```

`prefix()` работает за O(1) и разделяет хвост с прежним списком. `size()` хранится
в структуре. Поиск n-го элемента — последовательный, поэтому маскировать его под
`item(index)` нельзя.

`first()` и `tail()` у `EmptyList` бросают `EmptyListAccess` с названием операции.

## 10. `collections/set`

`OrderedSet<T>` сохраняет порядок первого появления и удаляет повторы:

```okkama
let tags = OrderedSet(values: input_tags);
if tags.membership("stable") {
    enable_stable_mode();
}
let extended = tags.addition("portable");
let reduced = extended.difference("legacy");
```

`addition` и `difference` возвращают новые множества. Старые остаются пригодными.

Операции над двумя множествами:

```okkama
let either = Union(left: a, right: b);
let both = Intersection(left: a, right: b);
let only_a = Difference(left: a, right: b);
let exactly_one = SymmetricDifference(left: a, right: b);
```

Порядок детерминирован: `Union` сначала идёт по левому, затем по новым элементам
правого; остальные операции сохраняют порядок соответствующих исходных множеств.

Важно: текущая реализация не притворяется хеш-таблицей. Пока у рантайма нет общего
hash-контракта, принадлежность проверяется строгим равенством линейно. Для больших
множеств это надо учитывать в профиле производительности.

## 11. `collections/deque`

`Deque<T>` — неизменяемая двусторонняя очередь. Концы представлены отдельными
capability-объектами:

```okkama
let queue = Deque(values: jobs);
let with_urgent = queue.front().addition(urgent);
let ready = with_urgent.back().addition(background);

let job = ready.front().value();
let remaining = ready.front().remainder();
```

Для стека используйте один и тот же конец:

```okkama
let pushed = stack.front().addition(value);
let top = pushed.front().value();
let popped = pushed.front().remainder();
```

`front()` и `back()` сами допустимы на пустой очереди, но `value()` и `remainder()`
тогда бросают `EmptyDequeAccess`. `item(index)` существует, но середина стоит O(n).
Отдельные `Stack` и `Queue` не нужны без более узкого доменного контракта.

## 12. `collections/heap`

`Heap<T: Comparing<T>>` — persistent min-heap:

```okkama
let heap = Heap(values: jobs);
let next = heap.top();
let remainder = heap.remainder();
let expanded = remainder.addition(new_job);
```

- `size()` и `top()` — O(1);
- `addition()` и `remainder()` — O(log n);
- пустой `top()` или `remainder()` бросает `EmptySequence`;
- порядка обхода нет и внутреннее расположение не открыто.

Если нужны все элементы по возрастанию, повторяйте `top()`/`remainder()`. Если нужен
один отсортированный снимок с индексами, используйте `Sorted`.

## 13. `collections/frequencies`

```okkama
let counts = Frequencies(values: words);
counts.size();             // число различных значений
counts.total();            // число всех входных элементов
counts.amount("error");   // 0, если такого значения нет

for counts -> frequency {
    Stdout().writeln(
        frequency.item(),
        ": ",
        frequency.amount().string(),
    );
}
```

Порядок — первое появление значения. `Frequencies` читает вход один раз и хранит
результат. Как и `OrderedSet`, до появления hash-контракта он использует линейное
сравнение: построение делает O(n*d) сравнений, где `d` — число различных
значений, но не пересобирает промежуточные списки, поэтому сотни различных
значений на тысячах элементов считаются за миллисекунды.

## 14. `collections/ordering`

### Сортировка

`Sorted<T>` материализует источник и стабильно сортирует по `Comparing<T>`:

```okkama
let ordered = Sorted(origin: values);
Stdout().writeln(ordered.first().string());
```

`SortedBy<T,K>` вычисляет ключ один раз для каждого элемента и сохраняет исходный
порядок равных ключей:

```okkama
let jobs = SortedBy<Job,int>(
    origin: pending,
    key: (job is Job) is int { job.priority(); },
);
```

`Ranked<T,K>` связывает значение с отдельным ключом и сравнивает только ключи. Он
полезен, когда доменная сортировка отличается от естественного порядка объекта.

### Двоичный поиск

```okkama
let first is int = value LowerBound(origin: ordered, item: needle);
let after is int = value UpperBound(origin: ordered, item: needle);
let exact is int = value PositionOf(origin: ordered, item: needle);
```

- `LowerBound` — первая позиция, куда можно вставить значение;
- `UpperBound` — позиция после всех равных значений;
- `PositionOf` — первый точный элемент или `MissingItem`.

Источник обязан быть отсортирован тем же правилом сравнения. Каждый запрос не
проверяет это за O(n), иначе двоичный поиск потерял бы смысл.

## 15. `collections/combinatorics`

Все результаты ленивы, но каждый отдельный результат материализуется как `@T`.

```okkama
let pairs = Combinations(origin: values, size: 2);
let repeated_pairs = CombinationsWithReplacement(origin: values, size: 2);
let arrangements = Permutations(origin: values, size: 2);
let all_arrangements = Permutations(origin: values);
let grid = CartesianProduct(origins: dimensions);
```

Семантика:

- `Combinations`: позиции различны, порядок источника сохраняется;
- `CombinationsWithReplacement`: одну позицию можно выбирать повторно;
- `Permutations`: порядок выбора имеет значение;
- `CartesianProduct`: один выбор из каждого измерения, быстрее всего меняется
  правое измерение.

Отрицательный размер запрещён. Размер ноль даёт один пустой результат. Комбинация
без повторений размером больше источника пуста. Произведение без измерений содержит
один пустой результат; пустое измерение делает всё произведение пустым.

Ленивость спасает память, но не время. Число результатов растёт взрывным образом.
Обычно сразу ограничивайте перебор:

```okkama
for Taken(origin: Permutations(origin: candidates), count: 1000) -> candidate {
    inspect(candidate);
}
```

## 16. Практические рецепты

### Первые десять подходящих значений

```okkama
let result = Taken(
    origin: Filtered(origin: source, condition: wanted),
    count: 10,
);
```

Фильтр не обязан читать весь источник: обход прекращается после десятого результата.

### Нумерованные порции

```okkama
let batches = Enumerated(
    origin: Chunked(origin: records, size: 500),
    start: 1,
);
for batches -> batch {
    Stdout().writeln("batch ", batch.index().string());
    upload(batch.item());
}
```

### Проверка параллельных входов

```okkama
for StrictlyZipped(left: identifiers, right: payloads) -> pair {
    save(pair.first(), pair.second());
}
```

### Очередь приоритетов без мутации

```okkama
let pending = Heap(values: initial_jobs);
let current = pending.top();
let next_state = pending.remainder().addition(retry);
```

Старое состояние `pending` остаётся пригодным для журналирования или отката.

## 17. Частые ошибки

1. Не вызывайте `item()` курсора без `available()`.
2. Не запускайте полный `for` по `Repeating` или непустому `Cycled`.
3. Не ожидайте, что ленивый декоратор выполнил пользовательскую функцию в момент
   конструктора.
4. Не используйте `Zipped`, если потеря хвоста является ошибкой — берите
   `StrictlyZipped`.
5. Не используйте linked list как массив.
6. Не ожидайте O(1) от текущего `OrderedSet` и `Frequencies`.
7. Не передавайте неотсортированную последовательность в binary-search объекты.
8. Не путайте `Accumulated` (поток промежуточных состояний) и `Reduced` (одно
   окончательное значение).

## 18. Краткая карта выбора

| Задача | Объект |
|---|---|
| пронумеровать элементы | `Enumerated` |
| пройти два входа параллельно | `Zipped` / `StrictlyZipped` |
| склеить несколько потоков | `Chained` |
| преобразовать / отфильтровать | `Mapped` / `Filtered` |
| ограничить / пропустить | `Taken` / `Dropped` |
| разбить на порции | `Chunked` |
| снять один уровень вложенности | `Flattened` |
| получить соседние пары | `Pairwise` |
| накопить состояния / свернуть | `Accumulated` / `Reduced` |
| повторять значение | `Repeating` / `Repeated` |
| крутить последовательность | `Cycled` |
| persistent-голова/хвост | `List` |
| уникальные значения | `OrderedSet` |
| двусторонняя очередь | `Deque` |
| постоянно брать минимум | `Heap` |
| посчитать появления | `Frequencies` |
| отсортировать | `Sorted` / `SortedBy` |
| искать границу | `LowerBound` / `UpperBound` |
| комбинаторный перебор | `Combinations` / `Permutations` / `CartesianProduct` |
