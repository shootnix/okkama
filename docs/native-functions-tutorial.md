# Как добавить native-функцию в Okkama

Дата: 19.07.2026
Контракт: portable native runtime through M7 opaque handles

Этот документ — практическая инструкция для добавления низкоуровневой функции,
которую объявляет Okkama-модуль, а реализует host runtime. Архитектурные причины
и будущие расширения описаны отдельно в
`docs/architecture/native-runtime.md`.

## Что уже поддерживается

Текущий исполняемый native ABI поддерживает fixed positional signatures:

```okkama
native add(left is int, right is int) is int;
```

Поддерживается:

- module-level bodyless declaration;
- фиксированное число позиционных аргументов;
- scalar, enum, typed array/map, `@byte` и `any` параметры/результаты;
- обязательный provider descriptor;
- default providers и injected test providers;
- module-qualified canonical symbol;
- рекурсивная проверка descriptor fingerprint, аргументов и результата;
- explicit `NativeValue::*` вместо сырых Perl/Okkama значений;
- defensive snapshots для collections и fresh language storage для results;
- normalized catchable `NativeFailure` outcomes;
- очистка неожиданных host exceptions.
- default `core.fs` provider with opaque `FileHandle` and binary
  `open/read/write/seek/flush/close` primitives.

Пока не поддерживаются named/default/variadic native calls и параметры или
результаты с типом action value `(T, ...) is R`. Anonymous actions, обычные
Okkama objects/roles и `ActionValue` не пересекают native boundary; внешние
ресурсы передаются только как явно объявленные opaque `native object`
capabilities.

## Из каких частей состоит функция

```text
stdlib/runtime/example.ok       Okkama declaration и public ABI
Native/Core/Example.pm          host-private provider implementation
NativeRegistry->default()       composition root
native runtime tests            injected и default end-to-end проверки
```

Okkama source никогда не содержит имя Perl package или callback. Perl provider
никогда не создаёт callable-функцию без видимой Okkama declaration.

## Шаг 1. Сформулировать переносимый контракт

До кода ответьте на вопросы:

- Операция действительно низкоуровневая и переносимая?
- Можно ли одинаково реализовать её в Perl, Rust и C?
- Какова единица измерения и допустимый диапазон результата?
- Что означает результат и чего он намеренно не означает?
- Какие expected host outcomes возвращаются как normalized NativeFailure?

Например, monotonic clock возвращает integer nanoseconds от неизвестной
runtime-local точки отсчёта. Это не календарное и не wall-clock время.

Не добавляйте в `runtime/*` высокоуровневые formatting, I/O buffering, line
splitting, HTTP или ORM операции. Они должны строиться выше, обычно на Okkama.
Узкий `runtime/memory` является generic owned byte-state capability, а не
исключением из этого правила: он знает только snapshot/size/consume/append,
reset и lifecycle. Строки, разделители, prefetch, threshold, wrapped streams и
flush принадлежат `io/buffer` и реализуются на Okkama.
Узкий `runtime/debug.data_dump(any)` является намеренным исключением: это
runtime diagnostic primitive над logical ABI values, а его публичная оболочка
`okkama.Dump` всё равно живёт в Okkama-коде. Так же устроен `debug/dumper`:
natives `data_kind`, `data_type`, `data_size`, `data_leaf`, `data_names`,
`data_keys`, `data_items` только описывают одно значение, а многострочный дамп
с отступами собирает Okkama.

## Шаг 2. Добавить bodyless Okkama declaration

Создайте declaration-only файл, например:

```okkama
// stdlib/runtime/example.ok
module runtime/example v1.0.0;

native sample_counter() is int;
```

Правила:

- declaration разрешена только на module scope;
- имя функции начинается со строчной буквы;
- результат обязателен;
- после сигнатуры ставится `;`, а не `{ ... }`;
- Okkama fallback body запрещён;
- `main`, `byte`, `async`, defaults, varargs, action-value signatures и
  constructors запрещены для native actions;
- `native object FileHandle;` разрешён как отдельная bodyless nominal type
  declaration, но не как native action или source constructor;
- `runtime/*` файл должен оставаться набором низкоуровневых declarations, а не
  реализацией stdlib behavior.

Приложение получает функцию обычным импортом:

```okkama
module app v1.0.0;

import "runtime/example" take sample_counter;

main() is int {
    sample_counter();
}
```

Без `take` native declaration доступна через basename namespace, например
`example.sample_counter()`. Provider сам по себе импорт не заменяет.

## Шаг 3. Определить canonical symbol

Fixed symbol строится так:

```text
module/path@ABI_MAJOR::name/fixed_arity
```

Для примера:

```text
runtime/example@1::sample_counter/0
```

Используйте только общий builder:

```perl
my $symbol = native_symbol(
    module_path    => 'runtime/example',
    module_version => 'v1.0.0',
    name           => 'sample_counter',
    fixed_arity    => 0,
);
```

Не собирайте строку вручную. Builder проверяет module/name/arity и нормализует
версию до ABI major. `v1.2.3` и `v1.9.0` имеют один provider ABI `@1`; `v2.x.x`
имеет другой.

Import path (`runtime/example` или `runtime/example.ok`), alias и имя consumer
module не входят в symbol. Identity берётся из header файла, объявившего
функцию.

## Шаг 4. Реализовать host-private provider

Создайте отдельный package:

```perl
use v5.42;
use utf8;

package Okkama::Runtime::Native::Core::Example;

use Okkama::Runtime::NativeSymbol qw/ native_symbol /;
use Okkama::Runtime::NativeValue;
use Okkama::Runtime::NativeFailure;

sub register($registry) {
    my $symbol = native_symbol(
        module_path    => 'runtime/example',
        module_version => 'v1.0.0',
        name           => 'sample_counter',
        fixed_arity    => 0,
    );

    $registry->register_symbol(
        symbol          => $symbol,
        parameter_types => [],
        return_type     => 'int',
        fixed_arity     => 0,
        variadic        => 0,
        provider_id     => 'core.example.sample-counter',
        handler         => sub ($context) {
            return Okkama::Runtime::NativeValue::Int->new(value => 42);
        },
    );
}

1;
```

Descriptor обязан полностью совпадать с declaration:

- `symbol` — canonical identity;
- `parameter_types` — ordered full parameter fingerprint;
- `return_type` — declared Okkama type;
- `fixed_arity` — число fixed parameters;
- `variadic` — сейчас `0`;
- `provider_id` — стабильное диагностическое имя provider implementation;
- `handler` — host-private callback.

Регистрация двух providers с одним canonical symbol является ошибкой.

## Шаг 5. Соблюдать границу handler

Portable handler получает ровно один `NativeCallContext`:

```perl
handler => sub ($context) {
    my $symbol = $context->symbol;
    my $args = $context->args;
    return Okkama::Runtime::NativeValue::Int->new(value => 42);
},
```

`$args` содержит ordered logical values и возвращается защитной копией. Для
сигнатуры `native add(left is int, right is int) is int;` handler выглядит так:

```perl
handler => sub ($context) {
    my ($left, $right) = $context->args->@*;

    die "wrong logical argument tag\n"
        unless $left->isa(Okkama::Runtime::NativeValue::Int::)
            && $right->isa(Okkama::Runtime::NativeValue::Int::);

    return Okkama::Runtime::NativeValue::Int->new(
        value => $left->value + $right->value,
    );
},
```

Provider никогда не получает обычные Perl scalar/reference как публичный ABI.
Основные формы:

- `Int`, `Float`, `Bool`, `String`, `Char`, `Byte` — `->value`;
- `Rat` — `->numerator`, `->denominator`, `->canonical`; обе части передаются
  десятичными строками, поэтому native boundary не ограничивает точность и не
  раскрывает mutable `Math::BigRat`;
- `Enum` — `->type`, `->case`;
- `Array` — `->element_type`, `->values`;
- `Map` — `->key_type`, `->value_type`, `->entries` (ordered pairs);
- `ByteSequence` — `->length`, `->octets`;
- direct `any` приходит конкретным logical tag; nested `any` остаётся private
  typed envelope и не даёт provider доступ к language box.

Handler не получает:

- interpreter;
- AST declaration/call nodes;
- caller environment или locals;
- receiver/private object fields;
- произвольные Perl package names из Okkama source.

Не добавляйте такие значения в context «для удобства». Сначала должен появиться
переносимый logical ABI contract.

Handler обязан вернуть `Okkama::Runtime::NativeValue::*`, соответствующий
Okkama result type. Runtime не доверяет descriptor и рекурсивно проверяет tag,
collection type metadata, enum case, byte length/octet range и nested values.
Сырой Perl scalar/reference и Okkama runtime object являются malformed provider
result.

Expected failure возвращается отдельно от successful result:

```perl
handler => sub ($context) {
    my $ok = perform_host_operation();

    return Okkama::Runtime::NativeFailure->new(
        domain  => 'core.fs',
        code    => 'permission_denied',
        message => 'operation is not permitted',
    ) unless $ok;

    return Okkama::Runtime::NativeValue::Int->new(value => 1);
},
```

Interpreter сам добавляет canonical `operation`. Okkama ловит outcome так:

```okkama
try {
    low_level_operation();
}
catch failure is NativeFailure {
    failure.domain();
    failure.code();
    failure.operation();
    failure.message();
}
```

Для syscall provider обязан немедленно сохранить numeric errno. Удобный helper
не читает ambient `$!`:

```perl
use Okkama::Runtime::NativeFailure::Errno qw/native_failure_from_errno/;

unless (open my $handle, '<', $path) {
    my $errno = 0 + $!;
    return native_failure_from_errno(
        domain  => 'core.fs',
        errno   => $errno,
        message => 'file cannot be opened',
    );
}
```

Не передавайте helper-у `$!` object или автоматически сформированную строку с
path. Ветвление stdlib выполняет по stable `domain`/`code`, не по `message`.

Для `core.fs` общий normalizer дополнительно сохраняет закрытый неизменяемый
diagnostic payload: операцию файловой системы, точный path, второй path для
rename/link и доступные native code/reason. Это отдельные scalar fields, а не
склейка строки `message`. Числовой `$!` сохраняется до cleanup, reason берётся
из локальной копии сохранённого errno. В normal diagnostics canonical symbol
остаётся как `native_symbol`; публичный `NativeFailure.operation()` по-прежнему
возвращает canonical symbol. Payload не доступен через Okkama reflection.
Текст ОС, его язык и числовые errno могут различаться на разных платформах.

Неожиданный `die` очищается до:

```text
Native invocation "runtime/example@1::sample_counter/0" failed
```

Это programming/provider failure, не recoverable domain error. Malformed/raw
result также остаётся uncatchable и никогда не превращается в NativeFailure.

## Шаг 6. Подключить core provider

Для обязательного bundled provider добавьте package в composition root:

```perl
# Okkama::Runtime::NativeRegistry
use Okkama::Runtime::Native::Core::Example;

sub default($class) {
    my $registry = $class->new();
    # другие providers
    Okkama::Runtime::Native::Core::Example::register($registry);
    return $registry;
}
```

Platform capability проверяется во время регистрации. Если обязательная
операция недоступна, завершайте сборку default registry с ясной ошибкой. Не
подменяйте monotonic clock wall clock, secure random обычным PRNG и т.п.

Optional provider loading и security policy пока не определены. Okkama source
не может самостоятельно загрузить host package.

## Шаг 7. Сначала написать injected test

Injected registry доказывает linker/call contract без зависимости от host:

```perl
my $registry = Okkama::Runtime::NativeRegistry->new();
my $symbol = native_symbol(
    module_path    => 'runtime/example',
    module_version => 'v1.0.0',
    name           => 'sample_counter',
    fixed_arity    => 0,
);

$registry->register_symbol(
    symbol          => $symbol,
    parameter_types => [],
    return_type     => 'int',
    fixed_arity     => 0,
    variadic        => 0,
    provider_id     => 'test.example',
    handler         => sub ($context) {
        return Okkama::Runtime::NativeValue::Int->new(value => 123);
    },
);

my $result = run_program(
    $application_source,
    stdlib_dir      => '../stdlib',
    native_registry => $registry,
);

is($result, 123);
```

Минимальная test matrix:

- canonical symbol имеет ожидаемую строку;
- declaration импортируется с `take`;
- handler вызван ровно один раз;
- context содержит canonical symbol и ordered logical args;
- deterministic result дошёл до Okkama;
- пустой registry даёт `Native symbol "..." is not registered`;
- wrong descriptor fingerprint отклоняется;
- wrong actual result type отклоняется;
- host exception не раскрывает текст/path;
- provider без source declaration не callable;
- ordinary object actions remain independent and always execute Okkama bodies.

После этого добавьте default-provider smoke test. Для времени проверяйте тип,
неотрицательность и неубывание; не проверяйте минимальный elapsed interval.

Focused команда текущей вертикали:

```text
cd okkama-pl
perl -Ilib t/interpreter/24-native-runtime-phase-01.t
```

Полный suite:

```text
cd okkama-pl
prove -Ilib -r t
```

## Шаг 8. Добавить opaque host resource

Если операция возвращает file/socket/process/другой host resource, объявите
отдельный nominal type:

```okkama
native object FileHandle;
native path_encode(path is string) is @byte;
native open(path is @byte) is FileHandle;
native read(handle is FileHandle, size is int) is @byte;
native close(handle is FileHandle) is bool;
```

Provider создаёт и регистрирует одну таблицу своей authority:

```perl
my $table = Okkama::Runtime::NativeHandleTable->new(
    provider_id => 'core.fs',
);
$registry->register_handle_table($table);
```

`open` сохраняет host resource только в таблице и возвращает logical handle:

```perl
return $table->mint(
    nominal_type => 'FileHandle',
    resource     => $perl_filehandle,
    finalizer    => sub ($fh) {
        close $fh or return native_failure_from_errno(
            domain => 'core.fs',
            errno  => 0 + $!,
        );
        return;
    },
);
```

Каждая последующая операция получает logical handle из `$context->args`, а
host resource — только через authority table:

```perl
my ($handle) = $context->args->@*;
my $fh = $table->resolve($handle, 'FileHandle');
return $fh
    if Scalar::Util::blessed($fh)
    && $fh->isa(Okkama::Runtime::NativeFailure::);
```

Закрытие также проходит через таблицу:

```perl
my $failure = $table->close($handle, 'FileHandle');
return $failure if defined $failure;
return Okkama::Runtime::NativeValue::Bool->new(value => 1);
```

Обязательные правила:

- не возвращайте raw Perl reference/filehandle/fd из handler;
- `provider_id` descriptor и таблицы должен описывать одного provider owner;
- не храните resource внутри language value;
- не реализуйте повторный close самостоятельно — table уже обеспечивает
  tombstone и exact-once finalizer;
- зарегистрированная таблица автоматически очищается interpreter shutdown и
  на обычном, и на exceptional exit;
- handles разрешены только на exact nominal boundaries и в typed array/map
  values; `any`, map keys, equality и source construction запрещены.

Эталонные тесты: `t/interpreter/38-native-object-types.t` —
`t/interpreter/44-native-handle-capabilities.t`.

Готовый filesystem-пример расположен в
`Okkama::Runtime::Native::Core::FS`, declarations — в
`stdlib/runtime/fs.ok`, end-to-end проверки — в
`t/interpreter/45-native-basic-byte-io.t` и
`t/interpreter/46-native-filesystem-and-directories.t`. Низкоуровневые пути
всегда имеют тип `@byte`; text caller обязан вызвать fallible `path_encode`, а
имя из `readdir` можно передать в `open/stat/unlink` напрямую. Не копируйте его numeric open flags
в high-level application API: stdlib wrapper должен дать им осмысленные имена
или policy object. `flush` выталкивает runtime-буфер, а `sync` отдельно
запрашивает durable storage. `truncate` не принимает отрицательный размер.
`lock_shared` и `lock_exclusive` — блокирующие advisory whole-file locks;
снимать их следует через `unlock` до `close`.

Над этой границей находится чистый `stdlib/fs/path.ok`. Его `Path` выбирает
POSIX/Windows grammar через `Host` только в экшене, а `PosixPath` и
`WindowsPath` описывают явный foreign syntax. Конструкторы сохраняют строку;
`joined`, `parent`, `normalized`, `absolute`, `string` и `bytes` не вызывают
filesystem provider. Нормализация только лексическая и не делает `stat`,
`realpath` или symlink resolution. `bytes()` кодирует исходную валидную строку
в UTF-8; сырые не-UTF-8 имена из `readdir` остаются `@byte` и не должны
насильно превращаться в `Path`.

Процессный пример расположен в
`Okkama::Runtime::Native::Core::Process`, declarations — в
`stdlib/runtime/process.ok`, end-to-end тест — в
`t/interpreter/47-native-processes-and-pipes.t`. `spawn` получает raw `@byte`
program/argv и три разные PipeHandle capabilities; shell command string здесь
намеренно отсутствует. `spawn_configured` дополнительно принимает raw-byte cwd
и массив пар `@@@byte`, полностью заменяющий environment только в child.
Пустой/NUL-содержащий cwd, не-пары, пустые имена, `=`/NUL в именах, NUL в
значениях и повторные имена должны отклоняться до exec; `%ENV` и cwd родителя
не меняются. Provider обязан отличать exec failure от exit 127,
ставить close-on-exec на остальные descriptors и всегда reaping'овать owned
child при explicit close и runtime shutdown.

Сетевой provider расположен в `Okkama::Runtime::Native::Core::Net`, declarations
— в `stdlib/runtime/net.ok`, loopback end-to-end тест — в
`t/interpreter/48-native-network-and-dns.t`. Не передавайте наружу packed
sockaddr: `runtime/net` использует документированный versioned `@byte` endpoint.
У `resolve(host, port, socket_type)` закрытая portable шкала: `0` не задаёт
transport, `1` ограничивает stream, `2` — datagram. Высокоуровневый `net/dns`
обязан использовать `0`: выбор TCP/UDP принадлежит будущему сокету, а не DNS.
Проверяйте family/type/protocol до host call, ставьте close-on-exec, подавляйте
SIGPIPE только вокруг send и возвращайте partial progress ровно одной попытки.
Blocking операции M11 нельзя незаметно превращать в provider-specific async
API. Готовый readiness provider находится в
`Okkama::Runtime::Native::Core::Poll`, declarations — в
`stdlib/runtime/poll.ok`, end-to-end тест — в
`t/interpreter/49-native-poll-eventing.t`. Новый pollable handle подключается
через `NativeRegistry::register_poll_source`: callback сначала обязан
проверить authority в своей NativeHandleTable и только затем вернуть
provider-private read/write endpoints. Никогда не делайте fd публичным int.

`poll_wait` принимает абсолютный deadline из `monotonic_clock()`, а не
относительные миллисекунды; `-1` означает ожидание без срока. Public masks
фиксированы как readable `1` и writable `2`. Backend-specific epoll/kqueue/
select flags не должны попадать в declaration ABI.

Generic memory provider находится в
`Okkama::Runtime::Native::Core::ByteMemory`, declarations — в
`stdlib/runtime/memory.ok`, focused tests — в
`t/interpreter/native/55-native-byte-memory.t`. `ByteMemoryHandle` является
authority-checked owned capability над точной последовательностью байтов.
Provider умеет только создать состояние, получить snapshot/size, потребить
префикс, дописать байты, очистить, проверить lifecycle и закрыть. Close
идемпотентен и очищает payload. Не добавляйте сюда line read, UTF-8,
buffer capacity, prefetch, partial-write retry или wrapped I/O: эти политики
реализованы в `stdlib/io/buffer.ok`.

Полный time provider находится в `Okkama::Runtime::Native::Core::Time`, а ABI —
в `stdlib/runtime/time.ok`. Все величины — целые наносекунды:
`wall_clock()` возвращает Unix epoch, `monotonic_clock()` — локальную шкалу для
интервалов, `sleep(duration)` — относительную длительность. Не измеряйте время
через wall clock и не подменяйте monotonic fallback'ом на wall clock. Sleep
должен пересчитывать остаток относительно monotonic deadline после EINTR, а не
считать первое прерывание успешным завершением.

Высокоуровневое значение интервала реализовано отдельно на чистом Okkama в
`stdlib/time/duration.ok`. Его конструкторы только сохраняют `int`, а экшены
проверяют signed-64 nanosecond range и точно выполняют арифметику/перевод
именованных единиц. Отрицательный `Duration` допустим как разность или
обратный сдвиг. Нельзя переносить проверку nonnegative из native `sleep` в сам
тип: будущий clock adapter обязан проверить её именно на границе относительного
сна. `InvalidDuration` означает переполнение представления, а не provider
failure. Модуль не должен импортировать `runtime/time`.

Поверх этого ABI уже реализованы `time/instant`, `time/clock` и `time/date`.
`Instant` всегда несёт `ClockIdentity.system` либо `.monotonic`; смешивать их в
`since`/`less` нельзя. `SystemClock` и `MonotonicClock` вызывают provider только
в экшенах, `OffsetClock` является decorator. `Date` считает proleptic Gregorian
календарь на чистом Okkama, а `TimeZone` пока принимает только детерминированный
`UTC`, без неявного host tzdata.

Live-таймеры используют opaque `TimerHandle` из того же `runtime/time`:
`timer_create(delay, interval)`, `timer_wait`, `timer_cancel`,
`timer_cancelled`, `timer_deadline`, `timer_close`. Нулевой interval означает
one-shot, положительный — fixed-rate periodic schedule с пропуском опоздавших
тиков. Handle обязан проходить через `NativeHandleTable`, закрываться
идемпотентно и принадлежать runtime shutdown при утечке.

Обычный код не импортирует эти функции. `ScheduledTimer` и
`ScheduledPeriodicTimer` только сохраняют `Duration`, а их проекция `#projected timer()`
приобретает owned resource. У `Timer`/`PeriodicTimer` команда `cancel()` ничего
не возвращает; состояние спрашивается через `cancelled()`. Это не successor:
таймер является общей live identity, как файл или сокет.

Environment provider находится в
`Okkama::Runtime::Native::Core::Environment`: имена и значения принимаются как
`@byte`, а `environment_all()` возвращает пары `@@@byte`. Не превращайте их в
string/map — на Unix это может уничтожить исходные байты. Помните, что `%ENV`
process-global и наследуется новым child process.

Обычный Okkama-код не должен вызывать этот ABI напрямую. Адаптер
`stdlib/env/var.ok` предоставляет role `Environment`, ambient-реализацию
`Env`, номинальное имя `EnvVar` и snapshot `EnvEntry`. `bytes` сохраняет
байты, а `value`/`EnvEntry.string` выполняют строгий UTF-8 decode и переводят
ошибку в `InvalidEnvEncoding`. Отсутствие представлено `MissingEnvVar`, а не
пустым массивом. Конструкторы ничего не проверяют и не вызывают provider;
проверка происходит в экшенах. В тестах используйте `Environment.fake()`:
он изолирован по экземплярам и вообще не обращается к `%ENV`.

Application argv передаётся при сборке default registry через
`process_arguments => \@arguments`; provider обязан сделать snapshot. Первый
CLI argument — путь программы — туда не входит. Platform/architecture должны
проецироваться на закрытые portable spellings, а secure random обязан читать
OS CSPRNG и падать `unsupported`, если его нет. Fallback на `rand` запрещён.

Пользовательский адаптер argv находится в `stdlib/proc/args.ok`. `Args()` не
читает process state в конструкторе: `bytes()` получает точный `@@byte`, а
`strings()` и `at(index)` используют строгий `encoding/utf8`. Ошибка
декодирования превращается в `InvalidArgumentEncoding` с индексом аргумента;
выход за границы — в `MissingArgument`. Не добавляйте executable/source path,
replacement decoding или locale-зависимое преобразование.

Высокоуровневый `stdlib/random.ok` не требует нового provider. `Random.bytes`
делегирует `SecureRandom`, а `int`, `bool` и `float` вычисляются на Okkama над
полученными байтами. Диапазон `int(minimum, maximum)` включает обе границы и
использует rejection sampling без modulo bias по всему signed 64-bit `int`.
`SeededRandom(seed)` — явно небезопасный чистый детерминированный источник без
native-вызова и скрытого курсора: одинаковый запрос всегда даёт одинаковый
ответ. Для воспроизводимых `Choice` и `Shuffled` внедряйте его через
перегруженный конструктор; не подменяйте им недоступный OS CSPRNG.

Пользовательский слой над identity ABI находится в `stdlib/os.ok`. `Host()`
не кэширует результат в конструкторе: каждый его экшен получает закрытую
runtime-строку и переводит её в `Platform` или `Architecture`. Известный
Windows даёт Windows path style, CRLF и `;`, остальные известные платформы —
POSIX, LF и `:`. Для runtime-значения `unknown` identity остаётся доступной,
но соглашения о путях не угадываются и бросают `UnsupportedPlatform`.
Невозможное написание или `NativeFailure` считается нарушением provider ABI и
превращается в стабильный panic без утечки host diagnostics. `Posix` и
`Windows` нужны для явного foreign/test target и хранят только переданные enum.

Regex provider находится в `Okkama::Runtime::Native::Core::Regex`, declarations
— в `stdlib/runtime/regex.ok`; пользовательский слой реализован на чистом
Okkama в `stdlib/regex.ok`. Не заменяйте portable compiler на `qr//`: Perl dialect,
backtracking и host diagnostics не являются частью ABI. Скомпилированная NFA
обязана оставаться за `RegexHandle`. Кроме bool-операций, `spans` возвращает
полное leftmost-longest совпадение и группы захвата в порядке исходника.
Смещения считаются в Unicode code points; незахваченная группа обозначается
`start = -1, end = -1`, а повторённая хранит последнюю участвовавшую итерацию.
`all_spans` возвращает непересекающиеся совпадения, а `invalid_offset` — `-1`
или точный offset без текста парсера. `Regex(pattern)` только хранит строку;
каждый его экшен создаёт и детерминированно закрывает собственный
operation-scoped handle. Replacement выполняется на Okkama и трактует замену
буквально, без `$1` и других host-соглашений.

Formatting provider находится в
`Okkama::Runtime::Native::Core::Format`, declarations — в
`stdlib/runtime/format.ok`, тесты — в
`t/interpreter/native/52-native-format-crypto.t`. Только этот узкий trusted
boundary форматирует opaque `any`: `concat_values` склеивает display forms,
`format_values` подставляет позиционные `{}`, `format_named` — `{identifier}`,
`sprintf_values` реализует portable percent-subset и точный `%r`, а `{{`/`}}`
дают литеральные скобки. Не объявляйте native overloads: у разных
сигнатур должны быть разные имена.

Обычный код использует `stdlib/format.ok`: `string Formatted(pattern, values...)`,
`string NamedFormatted(pattern, %string:any)` или
`string PrintfFormatted(pattern, values...)`. Эти state-only objects проверяют
грамматику и наличие значений до native-вызова, возвращают `InvalidFormat` с
точным Unicode code-point offset или `MissingFormatValue` с индексом/ключом и
не выпускают provider diagnostics через публичный API.

Cryptography provider находится в
`Okkama::Runtime::Native::Core::Crypto`. ABI принимает только lowercase
`sha256`, `sha384`, `sha512`; digest, ключ, HMAC и результат всегда являются
точными `@byte`. `secure_equal` обязан пройти длину большего операнда и учесть
разницу длин без раннего возврата по содержимому.

TLS provider находится в `Okkama::Runtime::Native::Core::TLS`, declarations —
в `stdlib/runtime/tls.ok`, реальный loopback-тест — в
`t/interpreter/native/53-native-tls.t`. Конструктор сессии потребляет
подключённый stream `SocketHandle`: прежний handle становится closed tombstone,
а ресурсом владеет `TlsHandle`. Клиент всегда проверяет chain и server name;
сервер принимает PEM certificate/key bytes. Handshake явный и идемпотентный,
I/O работает только через TLS-методы, projection сертификата фиксирован, close
и runtime shutdown завершают owned session. Для Perl runtime установите
зависимости из `okkama-pl/cpanfile`; без TLS engine provider сохраняет ABI, но
возвращает нормализованный `unsupported`.

Standard-stream provider находится в
`Okkama::Runtime::Native::Core::Stdio`, declarations — в
`stdlib/runtime/stdio.ok`. `stdin_read(limit)` требует положительный предел,
возвращает не больше указанного числа байт и использует общий внутренний буфер
с `stdin_read_line()`. Поэтому смешанные вызовы не должны терять или повторять
данные. `stdout_write` и `stderr_write` обязаны писать в разные handles и
заканчивать полную запись либо возвращать `NativeFailure`. Процедурные имена и
счётчики остаются в ABI; объекты `Stdin`, `Stdout` и `Stderr` наружу их не
выдают.

Высокоуровневые `Buffer`, `StringBuffer`, `BufferedInput` и `BufferedOutput`
живут в `stdlib/io/buffer.ok`. Именно там определены UTF-8/line semantics,
capacity, prefetch, flush, partial-write retry и владение wrapped stream;
`runtime/memory` этих политик не содержит.

## Диагностика типичных ошибок

### Symbol is not registered

```text
Native symbol "runtime/example@1::sample_counter/0" is not registered
```

Проверьте:

- provider зарегистрирован в registry, который реально передан interpreter;
- module path, ABI major, name и arity совпадают;
- default composition root вызывает `Provider::register`;
- test не инжектирует пустой registry намеренно.

### Provider signature does not match declaration

Проверьте ordered `parameter_types`, `return_type`, `fixed_arity` и `variadic`.
Типы не входят в короткую symbol string, но сравниваются отдельно и полностью.

### Function is undefined despite registered provider

Provider не создаёт source name. Проверьте declaration-файл и import/`take`.

### Import not found

Проверьте, что файл лежит в `stdlib/runtime/...ok`, `stdlib_dir` указывает на
repository stdlib, а import path соответствует относительному пути. Header
module name остаётся отдельным ABI owner и тоже должен быть правильным.

### Returned value incompatible with declared type

Descriptor совпал, но handler вернул фактически неправильное значение. Это
provider bug; не ослабляйте runtime check.

## Чего не делать

Удалённого object-action override API больше нет. Не восстанавливайте методы
`register_action`/`register_variadic_action` и не связывайте ordinary Okkama
action с host callback. Host implementation возможна только для явной bodyless
module-native declaration; отсутствие её provider всегда является ошибкой.

Также не следует:

- писать `panic "native ..."` вместо bodyless declaration;
- передавать Perl callback/package name через Okkama source;
- строить canonical symbol вручную;
- регистрировать provider без полного fingerprint;
- передавать interpreter/AST/environment в portable handler;
- выдавать host exception, `$!`, path или reference пользователю;
- добавлять wall-clock fallback для monotonic API;
- считать parser support доказательством executable ABI support.

## Карта текущей реализации

- syntax/owner AST: `Okkama::Parser`, `Okkama::AST::Decl::NativeAction`;
- symbol builder: `Okkama::Runtime::NativeSymbol`;
- descriptor: `Okkama::Runtime::NativeDescriptor`;
- registry: `Okkama::Runtime::NativeRegistry`;
- handler input: `Okkama::Runtime::NativeCallContext`;
- link/invocation/result boundary: `Okkama::Interpreter`;
- first provider: `Okkama::Runtime::Native::Core::Time`;
- logical values: `Okkama::Runtime::NativeValue`;
- conversion/validation: `Okkama::Runtime::NativeValueAdapter`;
- expected failure descriptor: `Okkama::Runtime::NativeFailure`;
- numeric errno mapper: `Okkama::Runtime::NativeFailure::Errno`;
- first declaration: `stdlib/runtime/time.ok`;
- focused tests: `t/parser/08-native-declarations.t` and
  `t/interpreter/24-native-runtime-phase-01.t`;
- portable values/failures: `t/interpreter/35-native-value-adapter.t`,
  `t/interpreter/36-native-failure.t` and
  `t/interpreter/37-native-errno-mapping.t`.

Перед добавлением нового logical category, failure code/domain или NativeHandle
сначала обновите архитектурный ABI, formal spec и tests, затем adapter/runtime
code, и только после этого добавляйте использующую его declaration в
`runtime/*`.
