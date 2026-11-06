# Асинхронность и конкурентность в Okkama

Это подробное руководство по встроенной асинхронности и модулям `async/*`
стандартной библиотеки Okkama. Оно рассчитано на читателя, который раньше не
работал с `Future`, конкурентными задачами, каналами и отменой.

Примеры соответствуют текущему API Stage 19. В руководстве используются:

- встроенные `Future<T>`, `Cancelled`, `async`, `await` и `spawn`;
- `async/cancellation`;
- `async/channel`;
- `async/scope`;
- `async/collect`;
- `async/timeout`;
- `async/lock`;
- `async/semaphore`;
- `async/event`;
- `async/barrier`.

Часть фрагментов полностью самодостаточна и содержит `module`, objects и
`main`. В коротких предметных фрагментах имена вроде `client`, `repository`,
`Response` и `consume` обозначают код приложения: их нужно заменить своими
объектами.

Точка входа асинхронной программы — `async main() is int`, без параметров.
Она возвращает код завершения процесса в диапазоне `0..255`: `0` — успех,
остальные коды определяет приложение. Используйте `return 0;` после основной
работы. `defer`, `destroy` и остановка фоновых задач выполняются до выхода;
ошибка очистки не превращается в успешное завершение.

## Содержание

1. [Самая короткая ментальная модель](#1-самая-короткая-ментальная-модель)
2. [Последовательность, async и parallel](#2-три-разных-понятия-которые-нельзя-смешивать)
3. [Объявление async actions](#3-объявление-async-actions)
4. [Встроенный Future](#4-встроенный-futuret)
5. [Как работает await](#5-await-что-именно-происходит)
6. [Как работает spawn](#6-spawn-немедленный-запуск)
7. [Ошибки и cleanup](#7-ошибки-trycatch-и-defer)
8. [Таблица выбора](#8-как-выбрать-инструмент)
9. [`async/cancellation`](#9-модуль-asynccancellation)
10. [`async/channel`](#10-модуль-asyncchannel)
11. [`async/scope`](#11-модуль-asyncscope)
12. [`async/collect`](#12-модуль-asynccollect)
13. [`async/timeout`](#13-модуль-asynctimeout)
14. [`async/lock`](#14-модуль-asynclock)
15. [`async/semaphore`](#15-модуль-asyncsemaphore)
16. [`async/event`](#16-модуль-asyncevent)
17. [`async/barrier`](#17-модуль-asyncbarrier)
18. [Практические рецепты](#18-практические-рецепты)
19. [Частые ошибки](#19-частые-ошибки)
20. [Гарантии и ограничения](#20-гарантии-и-сознательные-ограничения)
21. [Краткая памятка API](#21-краткая-памятка-api)
22. [Алгоритм выбора](#22-последний-алгоритм-выбора)

## 1. Самая короткая ментальная модель

Запомните четыре правила.

1. Обычный action возвращает готовое значение.
2. `async` action возвращает ленивый `Future<T>`.
3. `await` запускает Future, если он ещё не запущен, приостанавливает только
   текущий async action и затем извлекает `T`.
4. `spawn` запускает работу сразу и возвращает `Future<T>`.

Минимальный пример:

```okkama
module app v1.0.0;

object Answer {
    Answer();

    async value() is int {
        42;
    }
}

async main() is int {
    let pending is Future<int> = Answer().value();
    await pending;
    return 0;
}
```

После `Answer().value()` переменная `pending` содержит не `42`, а обещание
когда-нибудь получить `42`. Тело `value()` ещё не обязано выполняться.
`await pending` запускает Future и получает результат.

Эквивалентный вариант с немедленным запуском:

```okkama
async main() is int {
    let pending is Future<int> = spawn Answer().value();
    await pending;
    return 0;
}
```

## 2. Три разных понятия, которые нельзя смешивать

### 2.1. Последовательное выполнение

Обычный код выполняет действия одно за другим:

```okkama
let first = repository.first();
let second = repository.second();
first + second;
```

Второй вызов начинается после завершения первого.

### 2.2. Кооперативная асинхронность

Обычный `async` action выполняется в том же runtime. Он уступает управление в
точках приостановки: `await`, ожидание канала, таймера, lock и других
асинхронных runtime-операций.

Это не обещание отдельного потока операционной системы.

```okkama
async load() is int {
    let left = await source.left();
    let right = await source.right();
    left + right;
}
```

Этот пример по-прежнему последовательный: `right()` начинается только после
получения `left`. Ключевое слово `async` само по себе не делает два действия
параллельными.

Чтобы запустить оба действия до ожидания результатов:

```okkama
async load() is int {
    let left = spawn source.left();
    let right = spawn source.right();
    (await left) + (await right);
}
```

### 2.3. Параллельная работа в отдельном процессе

`spawn` синхронного bound action отправляет работу в изолированный worker
process. Это предназначено для CPU-bound работы.

```okkama
object Calculator {
    Calculator();

    square(value is int) is int {
        value * value;
    }
}

async main() is int {
    let action is (int) is int = Calculator().square;
    let pending is Future<int> = spawn action(6);
    (await pending) + 6;
    return 0;
}
```

Результат вычисления равен `42`, после чего main возвращает код успеха `0`.
Receiver, аргументы, результат и ошибка синхронной работы
должны переноситься через границу worker process. Непереносимое значение
отвергается до запуска worker.

Итого:

| Форма | Где работает | Когда начинается |
|---|---|---|
| `worker.async_action()` | cooperative runtime | при `await`, `spawn` или другом явном старте |
| `spawn worker.async_action()` | cooperative runtime | немедленно |
| `spawn synchronous_bound_action()` | изолированный process worker | немедленно |

## 3. Объявление async actions

Ключевое слово `async` ставится перед именем action:

```okkama
object UserRepository {
    UserRepository();

    async count() is int {
        42;
    }
}
```

Объявленный результат — внутренний тип результата. Здесь написано `is int`,
но вызов имеет тип `Future<int>`:

```okkama
let pending is Future<int> = UserRepository().count();
```

Так нельзя:

```okkama
// Ошибка типов: справа Future<int>, а слева int.
let count is int = UserRepository().count();
```

Так правильно, но только внутри `async` action:

```okkama
let count is int = await UserRepository().count();
```

`await` запрещён в синхронном action:

```okkama
// Ошибка: main не объявлен async.
main() is int {
    await UserRepository().count();
    return 0;
}
```

Правильный вариант:

```okkama
async main() is int {
    await UserRepository().count();
    return 0;
}
```

## 4. Встроенный `Future<T>`

`Future<T>` — единственный публичный объект асинхронного результата. Второго
типа вроде `Task<T>` в языке нет.

У `Future<T>` нет публичного конструктора. Future появляется после:

- вызова async action;
- `spawn`;
- вызова асинхронной операции стандартной библиотеки.

### 4.1. Публичные actions

```text
cancel()
done() is bool
cancelled() is bool
```

Для извлечения результата нет `result()` или `value()`. Результат извлекается
только оператором `await`.

### 4.2. Future ленивый

```okkama
object Failure {
    Failure();

    async broken() is int {
        panic "это тело не должно запуститься";
    }
}

main() is int {
    let pending = Failure().broken();
    42;
    return 0;
}
```

Программа доходит до выражения `42` и возвращает код успеха `0`: простой
вызов async action создал Future, но не запустил его.

### 4.3. Старт идемпотентен, результат кэшируется

Один Future выполняет свою работу не более одного раза. Его можно ожидать
повторно:

```okkama
async main() is int {
    let pending = Answer().value();
    let first = await pending;
    let second = await pending;
    first + second - 42;
    return 0;
}
```

Оба `await` читают один сохранённый результат.

Если Future завершился ошибкой, сохраняется та же типизированная ошибка.

### 4.4. `done()`

`done()` означает, что Future окончательно завершён одним из трёх способов:

- успешным результатом;
- ошибкой;
- завершённой отменой.

`done()` остаётся `false`, пока выполняется cleanup после отмены.

```okkama
let pending = worker.run();
let before = pending.done(); // false: Future даже не стартовал
let started = spawn Awaiter(origin: pending).run();
let while_running = pending.done();
```

`done()` — наблюдение, а не синхронизация. Нельзя писать логику вида «если
`done()`, тогда безопасно...»: состояние может измениться сразу после проверки.
Для получения результата всегда используйте `await`.

### 4.5. `cancel()` и `cancelled()`

```okkama
let pending = spawn worker.run();
pending.cancel();

try {
    await pending;
}
catch error is Cancelled {
    // Работа завершила отмену и cleanup.
}
```

Правила:

- повторный `cancel()` безопасен;
- отмена незапущенного Future завершает его `Cancelled`, не запуская тело;
- отмена уже завершённого Future не меняет результат;
- `cancelled()` становится `true` только после завершения unwind и `defer`;
- `await` отменённого Future бросает встроенный `Cancelled`;
- отмена кооперативна: runtime доставляет её в точках приостановки.

### 4.6. Отмена ожидающего и отмена ожидаемого — не одно и то же

Если action ожидает чужой, независимо принадлежащий Future, отмена action не
обязана отменять тот чужой Future. Владение должно быть явным.

Структурные объекты `FutureGroup`, `FutureScope`, `AllFutures`, `Raced`,
`FirstSuccessful`, `Within` и `Before` явно владеют дочерними Future и поэтому
распространяют отмену по своим контрактам.

## 5. `await`: что именно происходит

Выражение:

```okkama
let value = await pending;
```

делает следующее:

1. Проверяет, что operand — `Future<T>` или ожидание завершения async void.
2. Идемпотентно запускает Future.
3. Если Future ещё не готов, сохраняет continuation текущего async action.
4. Позволяет runtime выполнять другую готовую работу.
5. После завершения возвращается точно к этому выражению.
6. Выдаёт `T` либо бросает сохранённую ошибку.

Другие async actions не блокируются. Поток операционной системы не обязан
создаваться.

### 5.1. Локальные переменные сохраняются

```okkama
async calculate() is int {
    let base = 40;
    let addition = await source.addition();
    base + addition;
}
```

После возобновления `base` всё ещё равен `40`.

### 5.2. Порядок вычисления сохраняется

Okkama сохраняет обычный порядок слева направо и ленивость логических
выражений. Наличие `await` не разрешает runtime переставлять operands.

```okkama
let result = (await source.left()) + (await source.right());
```

`left()` вычисляется раньше `right()`.

### 5.3. `await` можно использовать внутри выражений

```okkama
let selected = if await flags.enabled() {
    await source.primary();
} else {
    await source.fallback();
};
```

Он также работает в `match`, условиях, циклах, аргументах, `confirm`, `throw`,
rebinding и других async-контекстах.

## 6. `spawn`: немедленный запуск

### 6.1. Async action

```okkama
let pending = spawn worker.run();
```

Для async action `spawn`:

- немедленно запускает его Future;
- возвращает тот же самый `Future<T>`, а не `Future<Future<T>>`;
- регистрирует Future в текущем async root.

### 6.2. Синхронный bound action

```okkama
let work is (int) is int = calculator.square;
let pending is Future<int> = spawn work(7);
```

Синхронная работа исполняется через изолированный worker process. Это
параллелизм для CPU-bound работы, а не обычная cooperative async операция.

### 6.3. First-class async action

```okkama
let work is async () is int = worker.run;
let pending is Future<int> = spawn work();
let answer = await pending;
```

Тип bound async action записывается как:

```text
async (ArgumentType, ...) is ResultType
```

### 6.4. Async root

У верхнего запуска есть неявный async root. Он владеет всеми eagerly spawned
Future текущего дерева выполнения.

При завершении root:

- незаконченная принадлежащая работа получает запрос отмены;
- runtime ждёт её cleanup;
- ненаблюдённая ошибка не теряется и сообщается пользователю;
- ожидаемый `Cancelled` не печатается как случайная ошибка.

Поэтому fire-and-forget не означает «ошибки можно забыть»:

```okkama
async main() is int {
    spawn worker.fail(); // Ошибка будет обнаружена root.
    42;
    return 0;
}
```

Если ошибка ожидаема, сохраните Future и наблюдайте его:

```okkama
async main() is int {
    let child = spawn worker.fail();
    try {
        await child;
        0;
    }
    catch error is ExpectedFailure {
        42;
    }
    return 0;
}
```

## 7. Ошибки, `try/catch` и `defer`

### 7.1. Ошибка Future проявляется в `await`

```okkama
object LoadingFailed does Error {
    LoadingFailed();
    text() is string { "loading failed"; }
}

object Loader {
    Loader();
    async load() is int { throw LoadingFailed(); }
}

async main() is int {
    try {
        await Loader().load();
        0;
    }
    catch error is LoadingFailed {
        42;
    }
    return 0;
}
```

### 7.2. Cleanup через `defer`

Обычный вложенный блок `{ ... }` внутри async-действия сохраняет асинхронное
исполнение: `await` приостанавливает задачу, а после возобновления доступны
её локальные переменные, параметры типов и приватные действия. Это относится
и к `if`/`loop`, используемым как выражения, например в правой части `let`.
Каждый вложенный блок выполняет собственный cleanup при выходе, в том числе
при отмене задачи.

```okkama
async update(lock is Lock) is Worker {
    let guard = await lock.guard();
    defer guard.close();

    await repository.update();
    self;
}
```

`defer` выполняется:

- при успехе, включая досрочный `return`;
- при ошибке;
- при отмене;
- ровно один раз.

В cleanup можно приостанавливаться:

```okkama
async run() is Worker {
    defer await audit.finished();
    await operation.execute();
    self;
}
```

Future не считается завершённым, пока такой cleanup не закончился.
`return await operation.execute();` сначала дожидается результата, затем
выполняет cleanup и передаёт результат вызывающей стороне. Ошибка cleanup
отменяет успешный возврат; отмена во время ожидания тоже запускает очистку.
Если запрос отмены приходит уже во время cleanup, runtime дожидается его
завершения и только затем доставляет отмену. Возвращаемый объект в таком
случае не передаётся вызывающему коду и освобождается.

### 7.3. Основная и вторичные ошибки

Структурные операции могут обнаружить основную ошибку и дополнительные ошибки
cleanup. Основная типизированная ошибка сохраняется для `catch`; вторичные
ошибки остаются в упорядоченной runtime-диагностике и не заменяют причину сбоя.

## 8. Как выбрать инструмент

| Задача | Инструмент |
|---|---|
| Получить один будущий результат | `Future<T>` + `await` |
| Немедленно начать action | `spawn` |
| Явно попросить cooperative-код остановиться | `Cancellation` |
| Передавать сообщения между владельцами | `Channel<T>` |
| Владеть набором дочерних Future | `FutureGroup<T>` |
| Ограничить жизнь дочерних Future телом | `FutureScope<T,R>` |
| Дождаться всех результатов | `AllFutures<T>` |
| Взять первое завершение | `Raced<T>` |
| Взять первый успех | `FirstSuccessful<T>` |
| Обрабатывать Future по мере завершения | `FutureCompletions<T>` |
| Ограничить время одного Future | `Within<T>` или `Before<T>` |
| Защитить один неизбежно общий ресурс | `Lock` |
| Ограничить число одновременных владельцев | `Semaphore` |
| Один раз объявить состояние ready | `Event` |
| Синхронизировать фиксированную группу по фазам | `Barrier` |

Главное практическое правило: если состояние можно отдать одному владельцу и
посылать ему команды через канал, это обычно понятнее lock.

## 9. Модуль `async/cancellation`

```okkama
import "async/cancellation" take Cancellation, Cancelling;
```

### 9.1. Зачем он нужен

`Future.cancel()` отменяет конкретный Future. `Cancellation` — отдельный
предметный сигнал: владелец просит целое cooperating-вычисление остановиться,
а вложенный код сам выбирает безопасные точки проверки.

### 9.2. Объекты и роль

```text
role Cancelling
    requested() is bool
    check()
    async wait()

object Cancellation
    Cancellation()
    cancelling() is Cancelling
    cancel()
```

`Cancellation` хранится у владельца запроса. Сотрудникам передаётся только
`Cancelling`: он позволяет увидеть отмену, но не запросить её.

Конкретный `CancellationToken` реализует `Cancelling` и разделяет состояние со
своим `Cancellation`:

```text
object CancellationToken does Cancelling
    requested() is bool
    check()
    async wait()
```

Полезный token нельзя сконструировать вручную. Единственный корректный путь —
`cancellation.cancelling()`.

### 9.3. CPU-bound цикл

```okkama
object Scanner {
    has cancelling is Cancelling;
    Scanner(:cancelling);

    run(values is @int) is int {
        let total = 0;
        for values -> value {
            cancelling.check();
            total = total + value;
        }
        total;
    }
}
```

`check()` дешёв до отмены и бросает встроенный `Cancelled` после неё.

### 9.4. Асинхронное ожидание отмены

```okkama
object Monitor {
    has cancelling is Cancelling;
    Monitor(:cancelling);

    async run() is int {
        await cancelling.wait();
        42;
    }
}

async main() is int {
    let cancellation = Cancellation();
    let monitor = spawn Monitor(
        cancelling: cancellation.cancelling(),
    ).run();
    cancellation.cancel();
    await monitor;
    return 0;
}
```

Все waiter возобновляются один раз. Новый `wait()` после запроса завершается
сразу. Отмена Future, возвращённого `wait()`, бросает только это ожидание и не
вызывает `Cancellation.cancel()`.

### 9.5. Типичная ошибка

```okkama
let cancellation = Cancellation();
let token = cancellation.cancelling();
cancellation.cancel();

// Само наличие token ничего не остановит.
// Код обязан вызвать token.check() или await token.wait().
```

## 10. Модуль `async/channel`

```okkama
import "async/channel" take
    Channel, Sending, Receiving,
    ChannelClosed, InvalidChannelCapacity;
```

### 10.1. Что такое канал

`Channel<T>` передаёт значения типа `T` от sender к receiver. Одно сообщение
получает ровно один receiver.

```text
Channel<T>.sending()   -> Sending<T>
Channel<T>.receiving() -> Receiving<T>
Sending<T>.send(T)     -> async
Receiving<T>.item()    -> async T
```

Передавайте объекту только нужную capability:

```okkama
object Producer {
    has output is Sending<int>;
    Producer(:output);

    async run() is Producer {
        await output.send(42);
        self;
    }
}

object Consumer {
    has input is Receiving<int>;
    Consumer(:input);

    async run() is int {
        await input.item();
    }
}
```

Producer не может читать, Consumer не может писать.

### 10.2. Буферизованный канал

```okkama
async main() is int {
    let channel is Channel<int> = Channel(capacity: 2);
    await channel.sending().send(20);
    await channel.sending().send(22);
    (await channel.receiving().item())
        + (await channel.receiving().item());
    return 0;
}
```

Буфер принимает не более `capacity` значений. Sender приостанавливается, если
буфер полон.

Отрицательная capacity бросает `InvalidChannelCapacity`. Ноль разрешён.

Ошибка сохраняет отвергнутое значение:

```okkama
try {
    let channel is Channel<int> = Channel(capacity: -1);
}
catch error is InvalidChannelCapacity {
    let rejected = error.capacity(); // -1
    let message = error.text();
}
```

### 10.3. Unbuffered rendezvous

```okkama
async main() is int {
    let channel is Channel<int> = Channel(capacity: 0);
    let producer = spawn Producer(output: channel.sending()).run();
    let value = await channel.receiving().item();
    await producer;
    value;
    return 0;
}
```

При capacity `0` сообщение нигде не хранится. `send()` завершается только после
встречи с `item()`, а `item()` — после встречи с `send()`.

### 10.4. Закрытие

Любая endpoint capability может закрыть весь канал:

```okkama
let output = channel.sending();
defer output.close();
await output.send(42);
```

Закрытие идемпотентно.

После закрытия:

- новые и заблокированные send бросают `ChannelClosed`;
- уже буферизованные сообщения можно дочитать;
- после опустошения `item()` бросает `ChannelClosed`.

Пример чтения до закрытия:

```okkama
async consume(input is Receiving<int>) is int {
    let total = 0;
    loop {
        try {
            total = total + await input.item();
        }
        catch error is ChannelClosed {
            last total;
        }
    };
}
```

Уничтожение или потеря последней переменной endpoint не закрывает канал.
Закрытие всегда явное.

### 10.5. FIFO и отмена

Канал сохраняет FIFO:

- для буферизованных сообщений;
- для заблокированных sender;
- для ожидающих receiver.

Отменённый blocked sender или receiver удаляется из очереди ровно один раз.
Если direct handoff уже состоялся, поздняя отмена не дублирует и не возвращает
сообщение.

Это локальная гарантия очереди. Глобальный порядок выполнения всех async
actions не обещается.

### 10.6. Почему channel types инвариантны

`Channel<T>`, `Sending<T>` и `Receiving<T>` инвариантны. Даже если `Dog`
считается `Animal`, `Channel<Dog>` не превращается автоматически в
`Channel<Animal>`.

Причина на пальцах: через `Sending<Animal>` разрешили бы отправить `Cat` в
настоящий `Channel<Dog>`. Receiver затем получил бы не тот тип. Инвариантность
не даёт такой дыре появиться.

### 10.7. Чего у канала нет

Нет:

- `try_send()`;
- `try_receive()`;
- `closed()`;
- автоматического закрытия;
- heterogeneous `select`.

Такие polling-проверки создают гонку: состояние может измениться сразу после
проверки.

## 11. Модуль `async/scope`

```okkama
import "async/scope" take
    Supervising, FutureGroup, FutureScope, FinishedFutureGroup;
```

Модуль отвечает за структурное владение дочерними Future.

### 11.1. `FutureGroup<T>`

```okkama
let group is FutureGroup<int> = FutureGroup();
let first = Worker(id: 1).run();
let second = Worker(id: 2).run();

group.start(first);
group.start(second);
await group.wait();
```

`start()`:

- регистрирует Future;
- сразу запускает его;
- ничего не возвращает;
- не забирает переменную у caller;
- повторная регистрация той же identity идемпотентна.

`active()` показывает число ещё не завершившихся детей, включая детей в
процессе cancellation cleanup.

`wait()` запечатывает группу: новые `start()` после него бросают
`FinishedFutureGroup`. Повторный `wait()` наблюдает одно сохранённое завершение.
`FinishedFutureGroup.text()` сообщает, что группа уже заканчивается.

`cancel()` также запечатывает группу и просит всех незавершённых детей
отмениться. При прямом использовании после `cancel()` нужно вызвать
`await group.wait()`.

### 11.2. Ошибки группы

Группа наблюдает все дочерние результаты. Независимые ошибки упорядочиваются по
порядку регистрации:

- первая становится основной;
- остальные остаются secondary diagnostics.

### 11.3. `FutureScope<T,R>`

`FutureScope` связывает жизнь группы с одним async body.

```okkama
object Job {
    Job();
    async run() is int { 21; }
}

object Batch {
    Batch();

    async run(group is Supervising<int>) is int {
        group.start(Job().run());
        group.start(Job().run());
        42;
    }
}

async main() is int {
    let scope is FutureScope<int, int> = FutureScope(
        action: Batch().run,
    );
    await scope.result();
    return 0;
}
```

Параметр `T` — единый тип результата детей. `R` — результат body.

Scope гарантирует:

- body вызывается один раз;
- успешный body не публикуется до завершения всех детей;
- при ошибке body незаконченные дети отменяются;
- при отмене scope дети отменяются;
- cleanup детей дожидается завершения;
- дети не переживают `scope.result()`.

Если нужны дети разных типов, используйте вложенные однородные scope.

## 12. Модуль `async/collect`

```okkama
import "async/collect" take
    AllFutures, Raced, FirstSuccessful,
    FutureCompletions, NoFutures;
```

### 12.1. `AllFutures<T>` — дождаться всех

```okkama
let pending is @Future<int> = [
    Worker(id: 1).run(),
    Worker(id: 2).run(),
    Worker(id: 3).run(),
];
let all is AllFutures<int> = AllFutures(origins: pending);
let values is Sequencing<int> = await all.results();
```

Все input Future запускаются. Результаты сохраняют порядок input, а не порядок
фактического завершения.

```text
input:      first, second, third
completion: third, first, second
results:    first, second, third
```

Пустой input немедленно даёт пустую sequence.

Встроенный `@Future<T>` уже удовлетворяет `Sequencing<Future<T>>`, поэтому для
обычного массива не нужен церемониальный wrapper.

При первой ошибке незаконченные siblings отменяются, cleanup дожидается конца,
а исходная ошибка остаётся основной. Повторный `results()` возвращает один и
тот же Future и не перезапускает работу.

### 12.2. `Raced<T>` — первое завершение

```okkama
let race is Raced<Response> = Raced(origins: requests);
let response = await race.result();
```

Побеждает первый завершившийся Future.

Важно: «завершился» означает и успех, и ошибку. Если первым упал Future, race
падает с этой ошибкой и не ждёт потенциальный поздний успех.

Проигравшие отменяются, их cleanup ожидается до публикации победителя.

### 12.3. `FirstSuccessful<T>` — первый успех

```okkama
let attempts is FirstSuccessful<Response> = FirstSuccessful(
    origins: requests,
);
let response = await attempts.result();
```

Ошибки игнорируются, пока остаётся возможный успех. После первого успеха
незаконченные attempts отменяются.

Если упали все:

- первая ошибка по порядку завершения становится основной;
- остальные сохраняются как secondary diagnostics.

### 12.4. Разница `Raced` и `FirstSuccessful`

| Ситуация | `Raced` | `FirstSuccessful` |
|---|---|---|
| Первый Future успешен | возвращает его | возвращает его |
| Первый Future упал, второй позже успешен | сразу падает | ждёт и возвращает второй |
| Все упали | падает первой завершившейся ошибкой | падает после последней попытки |
| Пустой input | `NoFutures` | `NoFutures` |

### 12.5. `FutureCompletions<T>` — обработка по готовности

```okkama
let completions is FutureCompletions<Response> = FutureCompletions(
    origins: requests,
);

while completions.remaining_count() > 0 {
    let completed is Future<Response> = await completions.completion();
    try {
        let response is Response = await completed;
        responses.write(response);
    }
    catch error is RequestFailed {
        failures.write(error);
    }
}
```

Здесь два `await` выполняют разную работу:

1. `await completions.completion()` ждёт следующий завершившийся Future.
2. `await completed` извлекает его значение или ошибку.

Каждая input-позиция возвращается ровно один раз. Даже если один и тот же
Future указан дважды, это две позиции. После исчерпания `completion()` бросает
`NoFutures`. Его `text()` сообщает, что Future больше не осталось.

`remaining_count()` — диагностическое количество ещё не выданных позиций. Это
не `Cursor`, потому что получение следующего Future может приостанавливаться.

## 13. Модуль `async/timeout`

```okkama
import "async/timeout" take Within, Before, TimedOut;
import "time/clock" take Timing, SystemClock, MonotonicInstant;
import "time/duration" take Duration, DurationUnit;
```

### 13.1. `Within<T>` — относительный timeout

```okkama
let clock is Timing = SystemClock();
let bounded is Within<Response> = Within(
    origin: client.request(),
    limit: Duration(
        value: 5.rat(),
        unit: DurationUnit.second,
    ),
    clock: clock,
);

try {
    let response = await bounded.result();
    consume(response);
}
catch error is TimedOut {
    use_cached_response();
}
```

`TimedOut.text()` возвращает `"future timed out"`.

Отрицательная duration отвергается `InvalidArgument`. Нулевая duration означает
немедленно наступивший timeout, если origin ещё не завершён.

### 13.2. `Before<T>` — общий deadline

`Before` полезен, когда несколько последовательных действий должны разделять
один общий budget.

```okkama
let clock is Timing = SystemClock();
let budget = Duration(
    value: 10.rat(),
    unit: DurationUnit.second,
);
let deadline is MonotonicInstant =
    clock.monotonic_instant().plus(budget);

let first_timeout is Before<Response> = Before(
    origin: client.first(),
    deadline: deadline,
    clock: clock,
);
let first = await first_timeout.result();

let second_timeout is Before<Response> = Before(
    origin: client.second(first),
    deadline: deadline,
    clock: clock,
);
let second = await second_timeout.result();
```

Второй action получает только остаток первоначальных десяти секунд.

### 13.3. Почему только monotonic time

Deadline имеет тип `MonotonicInstant`, не civil `Instant`. Системные часы могут
быть переведены пользователем, NTP или операционной системой. Monotonic clock
предназначен для измерения прошедшего runtime-времени и не прыгает вместе с
гражданскими часами.

### 13.4. Победитель и cleanup

- Уже завершённый origin побеждает без запуска clock wait.
- При готовности обеих сторон приоритет детерминированно у origin.
- Если выигрывает deadline, origin отменяется.
- `TimedOut` бросается только после cancellation cleanup origin.
- Если origin успел завершиться значением или ошибкой, они сохраняются.
- Ошибка clock provider не маскируется как `TimedOut`.
- Повторный `result()` возвращает один Future.

Передача `Timing` в constructor нужна ещё и для детерминированных тестов: тест
может подставить controlled fake clock вместо настоящего ожидания.

## 14. Модуль `async/lock`

```okkama
import "async/lock" take Lock, LockGuard;
```

### 14.1. Базовое использование

```okkama
async update(lock is Lock) is Worker {
    let guard is LockGuard = await lock.guard();
    defer guard.close();

    await repository.replace();
    self;
}
```

`guard()` возвращается только после получения владения. `LockGuard` нельзя
создать вручную. `close()` освобождает именно это владение и идемпотентен.

### 14.2. FIFO

Если lock занят, waiter ставятся в FIFO-очередь. При освобождении ownership
передаётся старейшему живому waiter без промежуточного момента, когда новый
caller мог бы его обойти.

Отмена ожидающего `guard()` удаляет request и не потребляет ownership.

### 14.3. `locked()` не даёт права действовать

```okkama
if !lock.locked() {
    // НЕЛЬЗЯ считать, что ресурс теперь наш.
}
```

Между проверкой и следующей строкой lock может получить другой action.
`locked()` нужен для диагностики и тестов. Для владения нужен только
`await lock.guard()`.

### 14.4. Lock не reentrant

Нельзя повторно запросить тот же lock, удерживая guard:

```okkama
let first = await lock.guard();
defer first.close();

// Будет ждать освобождения, которое текущий action сам удерживает.
let second = await lock.guard();
```

### 14.5. Когда lock не нужен

Предпочтительный дизайн:

```text
many callers -> Channel<Command> -> one state-owning async action
```

Один action владеет состоянием, поэтому конкурентная запись невозможна по
конструкции. Lock оставляйте для неизбежно общего внешнего ресурса.

## 15. Модуль `async/semaphore`

```okkama
import "async/semaphore" take
    Semaphore, Permit, InvalidPermitCount;
```

Semaphore похож на lock с несколькими одновременными владельцами.

### 15.1. Ограничение конкурентности

```okkama
let connections is Semaphore = Semaphore(permits: 8);

async request() is Response {
    let permit is Permit = await connections.permit();
    defer permit.close();
    await client.request();
}
```

Одновременно внутрь защищённой области попадут не более восьми action.

Начальное число обязано быть больше нуля. Иначе бросается
`InvalidPermitCount`, у которого есть `permits()` и `text()`.

### 15.2. `Permit`

`Permit`:

- создаётся только успешным `semaphore.permit()`;
- возвращает одну capacity при первом `close()`;
- повторный `close()` ничего не добавляет;
- поддерживает `closed()`;
- безопасен для `defer permit.close()`.

### 15.3. FIFO и direct handoff

При отсутствии capacity waiter стоят FIFO. Закрытый Permit передаётся сразу
старейшему waiter, не увеличивая на мгновение доступный count.

Отменённый waiter удаляется и не потребляет Permit.

### 15.4. `permits()`

`permits()` возвращает текущую доступную capacity, а не исходный limit.

```okkama
let semaphore = Semaphore(permits: 2);
let permit = await semaphore.permit();
let available = semaphore.permits(); // 1
```

Это диагностика, не условие безопасности. После чтения значение может сразу
измениться.

Для rendezvous используйте `Channel(capacity: 0)`, а не semaphore с нулём.

## 16. Модуль `async/event`

```okkama
import "async/event" take Event;
```

Event — одноразовая защёлка готовности.

### 16.1. Обычный event

```okkama
let started is Event = Event();
let first = spawn FirstWorker(signal: started).run();
let second = spawn SecondWorker(signal: started).run();

prepare_shared_state();
started.set();

await first;
await second;
```

Worker:

```okkama
object FirstWorker {
    has signal is Event;
    FirstWorker(:signal);

    async run() is int {
        await signal.wait();
        20;
    }
}
```

Первый `set()`:

- переводит Event в ready навсегда;
- будит всех текущих waiter;
- позволяет всем будущим waiter проходить сразу.

Повторный `set()` безопасен.

### 16.2. Изначально готовый event

```okkama
let event is Event = Event(ready: true);
await event.wait(); // Не блокируется после старта Future.
```

### 16.3. Level-triggered, не counting

```okkama
event.set();
event.set();
event.set();
```

Это не три сообщения. Это одно состояние `ready == true`.

Если три сигнала должны быть получены по одному разу, используйте
`Channel<T>`.

### 16.4. Нет reset

Event намеренно нельзя вернуть в unset. Для нового жизненного цикла создайте
новый объект:

```okkama
let next_cycle is Event = Event();
```

Отмена одного `wait()` не вызывает `set()` и не влияет на остальных waiter.

## 17. Модуль `async/barrier`

```okkama
import "async/barrier" take
    Barrier, BarrierState, BrokenBarrier, InvalidPartyCount;
```

Barrier синхронизирует фиксированное число участников по фазам.

### 17.1. Одна generation

```okkama
let phase is Barrier = Barrier(parties: 3);

object Participant {
    has phase is Barrier;
    Participant(:phase);

    async run() is int {
        prepare();
        let arrival = await phase.wait();
        consume();
        arrival;
    }
}
```

Первые два участника ждут. Третий завершает generation, после чего все трое
продолжают.

Каждый получает отдельный arrival index:

```text
первый пришёл  -> 0
второй пришёл  -> 1
последний      -> parties - 1
```

Последний index удобно использовать для одноразовой работы после rendezvous:

```okkama
let arrival = await phase.wait();
if arrival == phase.state().parties() - 1 {
    publish_metrics();
}
```

### 17.2. Barrier циклический

После успешной generation тот же Barrier автоматически готов собирать
следующую группу участников.

```okkama
for 0..10 -> iteration {
    prepare(iteration);
    await phase.wait();
    consume(iteration);
}
```

### 17.3. `BarrierState`

```okkama
let state is BarrierState = phase.state();
state.parties();
state.waiters();
state.generation();
state.broken();
```

Это неизменяемый снимок одного момента. Старый `state` не меняется после нового
arrival. Для свежего состояния снова вызовите `phase.state()`.

### 17.4. Ошибочная party count

`Barrier(parties: 0)` и отрицательные значения бросают
`InvalidPartyCount`. Ошибка хранит отвергнутое значение в `parties()`.

### 17.5. Cancellation ломает generation

Если один pending waiter отменён:

- его Future завершается `Cancelled`;
- все его peers текущей generation получают `BrokenBarrier`;
- будущие `wait()` тоже получают `BrokenBarrier`;
- Barrier остаётся broken до `reset()`.

Это необходимо: без одного участника оставшиеся никогда не смогут честно
завершить исходную generation.

### 17.6. `abort()`

```okkama
phase.abort();
```

Явно ломает barrier и будит текущих waiter с `BrokenBarrier`. Повторный
`abort()` безопасен.

### 17.7. `reset()`

```okkama
phase.reset();
```

`reset()`:

1. Будит старых waiter с `BrokenBarrier`.
2. Завершает старую generation.
3. Открывает новую unbroken generation.

Старый waiter никогда не переносится в новую группу.

## 18. Практические рецепты

### 18.1. Запустить несколько запросов и сохранить порядок

```okkama
let requests is @Future<Response> = [
    first_client.request(),
    second_client.request(),
    third_client.request(),
];
let all is AllFutures<Response> = AllFutures(origins: requests);
let responses = await all.results();
```

Используйте `AllFutures`, а не последовательные `await`, если запросы можно
запускать одновременно.

### 18.2. Попробовать несколько источников до первого успеха

```okkama
let attempts is @Future<Response> = [
    primary.request(),
    replica.request(),
    cache.request(),
];
let successful is FirstSuccessful<Response> = FirstSuccessful(
    origins: attempts,
);
let response = await successful.result();
```

Если ошибка первого источника должна немедленно завершать операцию, используйте
`Raced`, а не `FirstSuccessful`.

### 18.3. Ограничить число одновременных запросов

```okkama
object LimitedClient {
    has permits is Semaphore;
    has origin is Client;
    LimitedClient(:permits, :origin);

    async request() is Response {
        let permit = await permits.permit();
        defer permit.close();
        await origin.request();
    }
}
```

### 18.4. Общий deadline для нескольких шагов

```okkama
let clock is Timing = SystemClock();
let deadline = clock.monotonic_instant().plus(Duration(
    value: 3.rat(),
    unit: DurationUnit.second,
));

let authorization_timeout is Before<AuthResult> = Before(
    origin: authorization.run(),
    deadline: deadline,
    clock: clock,
);
let authorized = await authorization_timeout.result();

let storage_timeout is Before<StoreResult> = Before(
    origin: storage.write(authorized),
    deadline: deadline,
    clock: clock,
);
let stored = await storage_timeout.result();
```

Оба шага вместе получают три секунды, а не по три секунды каждый.

### 18.5. Producer/consumer с явным закрытием

```okkama
object Producer {
    has output is Sending<int>;
    Producer(:output);

    async run() is Producer {
        defer output.close();
        for 0..100 -> value {
            await output.send(value);
        }
        self;
    }
}

object Consumer {
    has input is Receiving<int>;
    Consumer(:input);

    async run() is int {
        let total = 0;
        loop {
            try {
                total = total + await input.item();
            }
            catch error is ChannelClosed {
                last total;
            }
        };
    }
}

async main() is int {
    let channel is Channel<int> = Channel(capacity: 16);
    let producer = spawn Producer(output: channel.sending()).run();
    let consumer = spawn Consumer(input: channel.receiving()).run();
    await producer;
    await consumer;
    return 0;
}
```

### 18.6. State-owner вместо lock

Плохая исходная идея: много actions напрямую меняют один общий объект под
lock.

Часто лучше передавать команды одному владельцу:

```okkama
object CounterOwner {
    has commands is Receiving<int>;
    CounterOwner(:commands);

    async run() is int {
        let total = 0;
        loop {
            try {
                total = total + await commands.item();
            }
            catch error is ChannelClosed {
                last total;
            }
        };
    }
}
```

Теперь только `CounterOwner` меняет `total`; lock не нужен.

## 19. Частые ошибки

### Ошибка 1. Забыли `await`

```okkama
let value is int = worker.value();
```

Справа `Future<int>`. Нужно:

```okkama
let value is int = await worker.value();
```

### Ошибка 2. Думают, что вызов async action уже начал работу

```okkama
let pending = worker.run(); // Пока только ленивый Future.
```

Для немедленного старта:

```okkama
let pending = spawn worker.run();
```

### Ошибка 3. Последовательные await вместо конкурентного старта

```okkama
let first = await client.first();
let second = await client.second();
```

Если зависимости нет:

```okkama
let first = spawn client.first();
let second = spawn client.second();
let result = combine(await first, await second);
```

### Ошибка 4. Запустили Future и забыли ошибку

```okkama
spawn worker.run();
```

Async root сообщит ненаблюдённую ошибку. Обычно сохраняйте Future, используйте
`FutureScope`, `FutureGroup` или `async/collect`.

### Ошибка 5. Забыли cleanup capability

```okkama
let guard = await lock.guard();
await operation.run(); // Ошибка оставит lock занятым.
guard.close();
```

Правильно:

```okkama
let guard = await lock.guard();
defer guard.close();
await operation.run();
```

То же правило относится к `Permit` и явно закрываемым channel endpoints.

### Ошибка 6. Polling вместо ожидания

```okkama
while !future.done() { }
```

Такой цикл не уступает управление и бессмысленно расходует CPU. Используйте:

```okkama
await future;
```

### Ошибка 7. Используют observation как право на действие

Опасны проверки:

```okkama
if !lock.locked() { ... }
if semaphore.permits() > 0 { ... }
if group.active() == 0 { ... }
```

Это снимки для диагностики. Синхронизация выполняется через `guard()`,
`permit()` и `wait()`.

### Ошибка 8. Путают Event и Channel

- Event: состояние «готово», которое после `set()` остаётся готовым.
- Channel: отдельные сообщения, каждое потребляется один раз.

### Ошибка 9. Путают Raced и FirstSuccessful

- `Raced`: первое завершение, включая ошибку.
- `FirstSuccessful`: первая успешная попытка.

### Ошибка 10. Используют civil time для timeout

Timeout принимает `MonotonicInstant`. Обычный `Instant` предназначен для
гражданского времени и не является безопасным deadline.

### Ошибка 11. Пытаются повторно войти в Lock

Lock не reentrant. Не вызывайте `guard()` повторно, пока не закрыт предыдущий
guard.

### Ошибка 12. Пытаются reset-нуть Event

У Event нет reset. Создайте новый Event. Reset есть у Barrier, потому что его
контракт явно разделён на generations.

### Ошибка 13. Взаимная блокировка двух locks

```text
Action A: взял lock X, ждёт lock Y
Action B: взял lock Y, ждёт lock X
```

Оба ждут друг друга бесконечно. Если нужны два locks, задайте и соблюдайте один
глобальный порядок захвата, либо переделайте состояние в одного owner.

### Ошибка 14. Ждут send без receiver на unbuffered channel

```okkama
let channel is Channel<int> = Channel(capacity: 0);
await channel.sending().send(42); // Action остановится здесь.
let value = await channel.receiving().item(); // До строки не дойдёт.
```

Запустите одну сторону отдельно:

```okkama
let sending = spawn Producer(output: channel.sending()).run();
let value = await channel.receiving().item();
await sending;
```

### Ошибка 15. Barrier никогда не набирает parties

`Barrier(parties: 4)` не завершит generation, если реально `wait()` вызвали
только три участника. Если один участник исчез, владелец должен отменить или
abort-нуть generation и решить, нужен ли `reset()`.

### Ошибка 16. Semaphore Permit утёк

Если не закрыть Permit, доступная capacity навсегда уменьшится. Ставьте
`defer permit.close()` сразу после успешного acquisition.

### Ошибка 17. Считают cancellation мгновенным убийством

Cancellation — запрос и cooperative unwind. Future может оставаться
`done() == false`, пока выполняются `defer`. Наружный код не должен считать
ресурс очищенным раньше времени.

## 20. Гарантии и сознательные ограничения

Okkama гарантирует:

- один запуск и один сохранённый результат на Future identity;
- сохранение continuation, locals и порядка выражений через `await`;
- выполнение `defer` при отмене;
- ожидание cancellation cleanup структурными операциями;
- FIFO внутри документированных очередей channel, lock и semaphore;
- явное владение spawned и scoped Future;
- типизированные ошибки.

Okkama не гарантирует:

- отдельный OS thread для async action;
- строгий глобальный порядок планирования;
- автоматическое закрытие ресурсов по garbage collection;
- thread identity в обычном async-коде;
- публичный scheduler или event loop;
- shared mutable concurrent collections;
- heterogeneous channel `select`;
- незаметный fire-and-forget.

Структурное дерево владения полезно представлять так:

```text
async root
├── spawned Future
├── FutureScope
│   ├── child Future
│   └── child Future
└── collected/timeout operation
    ├── owned origin Future
    └── owned timer or sibling Future
```

При завершении или отмене владелец отвечает за своих непосредственных детей и
ждёт их cleanup. Garbage collector эту ответственность не угадывает.

## 21. Краткая памятка API

### Builtin

```text
Future<T>
    cancel()
    done() is bool
    cancelled() is bool

Cancelled does Error
```

### `async/cancellation`

```text
Cancelling: requested, check, wait
Cancellation: cancelling, cancel
```

### `async/channel`

```text
Channel<T>: sending, receiving
Sending<T>: send, close
Receiving<T>: item, close
Errors: ChannelClosed, InvalidChannelCapacity
```

### `async/scope`

```text
Supervising<T>: start, active, cancel
FutureGroup<T>: start, active, cancel, wait
FutureScope<T,R>: result
Error: FinishedFutureGroup
```

### `async/collect`

```text
AllFutures<T>: results
Raced<T>: result
FirstSuccessful<T>: result
FutureCompletions<T>: remaining_count, completion
Error: NoFutures
```

### `async/timeout`

```text
Within<T>: result
Before<T>: result
Error: TimedOut
```

### `async/lock`

```text
Lock: guard, locked
LockGuard: closed, close
```

### `async/semaphore`

```text
Semaphore: permit, permits
Permit: closed, close
Error: InvalidPermitCount
```

### `async/event`

```text
Event: wait, set, ready
```

### `async/barrier`

```text
Barrier: wait, state, abort, reset
BarrierState: parties, waiters, generation, broken
Errors: InvalidPartyCount, BrokenBarrier
```

## 22. Последний алгоритм выбора

Если непонятно, что использовать, отвечайте по порядку:

1. Нужен один будущий результат? Используйте Future и `await`.
2. Нужно начать его прямо сейчас? Добавьте `spawn`.
3. Нужно владеть несколькими детьми? Используйте scope или group.
4. Нужно собрать несколько результатов? Выберите объект из `async/collect`.
5. Нужно ограничить время? Оберните Future в `Within` или `Before`.
6. Нужно передавать отдельные значения? Используйте Channel.
7. Нужно один раз объявить «готово» всем? Используйте Event.
8. Нужно дождаться фиксированного числа участников? Используйте Barrier.
9. Нужно ограничить число одновременных пользователей? Используйте Semaphore.
10. Нужен эксклюзивный доступ к неизбежно общему ресурсу? Используйте Lock.
11. Можно отдать состояние одному owner action? Сделайте это и обойдитесь без
    Lock.

Если остаётся сомнение между polling action (`done`, `ready`, `locked`,
`permits`, `active`) и suspending action (`await`, `wait`, `guard`, `permit`,
`item`) — почти всегда нужен suspending action.

## Источники актуального контракта

Этот гайд сверен с исполняемыми модулями:

- [`std/async/cancellation.ok`](../std/async/cancellation.ok)
- [`std/async/channel.ok`](../std/async/channel.ok)
- [`std/async/scope.ok`](../std/async/scope.ok)
- [`std/async/collect.ok`](../std/async/collect.ok)
- [`std/async/timeout.ok`](../std/async/timeout.ok)
- [`std/async/lock.ok`](../std/async/lock.ok)
- [`std/async/semaphore.ok`](../std/async/semaphore.ok)
- [`std/async/event.ok`](../std/async/event.ok)
- [`std/async/barrier.ok`](../std/async/barrier.ok)
- [актуальной архитектурой std v4](std/stdlib-roadmap-v4.md)

В сокращённых фрагментах `update(lock)` и `run()` с результатом `Worker`
подразумевается окружающий объект `Worker`, который хранит показанные
зависимости. Явный `self` завершает эти async-действия; это не `async void`.

## Асинхронные команды без результата

```okkama
object Repository {
    Repository();
    async commit() is void {}
}
async main() is int {
    let repository = Repository();
    await repository.commit();
    let pending = repository.commit();
    await pending;
    await pending;
    return 0;
}
```

`await` здесь только ждёт завершения. `let result = await repository.commit();`
и `consume(await repository.commit());` недопустимы. `void` не является типом:
`Future<void>` также недопустим. Исключения и отмена проходят через await
так же, как у действий с результатом. Пример с реальным ожиданием часов:
[completion.ok](../examples/async/completion.ok).
