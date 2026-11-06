# Операционная система Okkama: подробное руководство

Модули `os/*` дают переносимый доступ к окружению, платформе, учётным записям,
рабочему каталогу, запуску процессов и наблюдению за хранилищем. Платформенные
различия остаются внутри runtime provider-а.

## 1. Общая модель

В API различаются три класса вещей:

1. Неизменяемые описания: `Environment`, `Command`, `InheritedCommand`.
2. Наблюдения меняющегося мира: `Platform`, `Host`, `StorageUsage`.
3. Ресурсы с жизненным циклом: `Process` и его потоки.

Создание описания не выполняет эффект. `Command(...)` не запускает программу.
`Process(...)` тоже запускается лениво при первом lifecycle/stream действии.

## 2. `os/environment`

### Снимок окружения процесса

```okkama
let environment = Environment();
if environment.membership("APPLICATION_TOKEN") {
    let token = environment.variable("APPLICATION_TOKEN");
}
```

`Environment()` снимает immutable snapshot. Последующие изменения внешнего процесса
не меняют объект. `variable(name)` бросает `MissingEnvironmentVariable`, если имени
нет; это лучше, чем смешивать отсутствие с пустой строкой.

### Явное окружение

```okkama
let environment = Environment(values: [
    "LANG": "C.UTF-8",
    "APPLICATION_ENV": "production",
]);
```

Входная map копируется. `variables()` также возвращает независимый immutable
snapshot.

### Производные окружения

```okkama
let child_environment = Environment()
    .addition("LANG", "C.UTF-8")
    .addition("APPLICATION_ENV", "production")
    .difference("SSH_AUTH_SOCK")
    .difference("APPLICATION_TOKEN");
```

`add` и `diff` не меняют receiver. Это позволяет безопасно получить окружение
дочернего процесса, не меняя process-global environment.

Имя не может быть пустым и содержать `=` или NUL. Значение не может содержать NUL;
нарушение даёт `InvalidEnvironmentVariable`.

### Домашний каталог и PATH

```okkama
let environment = Environment();
let home is Directory = value HomeDirectory(environment: environment);
let paths = SearchPath(environment: environment);
```

`HomeDirectory` использует платформенную политику и бросает `MissingHomeDirectory`,
если достоверный путь определить нельзя.

`SearchPath` — `Sequencing<Path>` в исходном порядке. Повторы сохраняются. Пустая
компонента означает `Path(".")`; отсутствующий `PATH` даёт пустую последовательность.
Разделитель выбирает runtime provider, пользователь не проверяет ОС вручную.

## 3. `os/platform`

```okkama
let platform = Platform();
match platform.operating_system() {
    case OperatingSystem.windows { configure_windows_integration(); }
    case OperatingSystem.macos { configure_macos_integration(); }
    case OperatingSystem.linux { configure_linux_integration(); }
    case OperatingSystem.freebsd { configure_freebsd_integration(); }
    else { configure_portable_fallback(); }
}

let architecture = platform.architecture();
let byte_order = platform.byte_order();
```

Проверяйте платформу только когда домен действительно различается. Для путей,
PATH, процессов и блокировок политика уже должна быть внутри runtime provider-а.

`Host()` предоставляет `name()` и положительный `cpu_count()`. Это текущее
наблюдение, не стабильная идентичность машины:

```okkama
let workers = Host().cpu_count();
```

Недоступная информация приводит к `UnavailableHostInformation`, а не к фиктивной
строке или нулю.

## 4. `os/account`

API избегает фабрик `CurrentUser.value()` и `UserNamed.value()`. Сам `User` знает,
какую запись представляет:

```okkama
let current = User();
let alice = User(name: "alice");

Stdout().writeln(current.name());
Stdout().writeln(current.identity());
Stdout().writeln(current.home().path().string());
```

`User()` означает текущего пользователя, `User(name)` — lookup по имени. Нативная
машинерия остаётся внутри действий и может быть ленивой; конструктор не превращён в
мешок системных вызовов.

Группа:

```okkama
let developers = Group(name: "developers");
Stdout().writeln(developers.identity());
for developers.users() -> user {
    Stdout().writeln(user.name());
}
```

Identity — платформенно-нейтральная строка, а не обещание универсального UID/GID.
Возможные ошибки: `MissingUser`, `MissingGroup`, `MissingUserHome`,
`InvalidAccountName`, `UnavailableAccountInformation`.

Не используйте имя пользователя как криптографическую идентичность и не считайте
список членов группы окончательной проверкой авторизации.

## 5. `os/directory`

`WorkingDirectory` сам выполняет узкую роль `DirectoryLocating`:

```okkama
let working = WorkingDirectory();
Stdout().writeln(working.path().string());
for DirectoryEntries(origin: working) -> entry {
    inspect(entry);
}
```

Церемонии `value WorkingDirectory()` нет. Process-global `chdir` не предоставляется:
рабочий каталог передаётся конкретной команде. Так параллельные задачи не меняют
глобальное состояние друг у друга.

## 6. `os/command`

### `Commanding`

Это общий неизменяемый контракт:

```okkama
program() is Path
arguments() is @string
environment() is Environment
directory() is DirectoryLocating
string() is string
```

Потребители (`Process`, `CommandCapture`, pipeline) принимают роль, а не конкретный
класс.

### Полностью явная команда

```okkama
let command is Commanding = Command(
    program: Path(origin: "/usr/bin/git"),
    arguments: ["status", "--short"],
    environment: Environment().addition("LANG", "C.UTF-8"),
    directory: Directory(path: Path(origin: "/work/project")),
);
```

Такой объект воспроизводим: рабочий каталог и окружение заданы явно.

### Команда с унаследованным контекстом

```okkama
let command is Commanding = InheritedCommand(
    program: Path(origin: "git"),
    arguments: ["--version"],
);
```

`InheritedCommand` получает snapshot окружения и текущий каталог через свои
действия. Это короткий API для утилит и интерактивных сценариев. Для тестов, build
и серверного кода предпочитайте явный `Command`.

### Никакого неявного shell

Каждый элемент `arguments` передаётся одним аргументом:

```okkama
InheritedCommand(
    program: Path(origin: "printf"),
    arguments: ["%s", "hello world"],
);
```

`"hello world"` не разбивается. `*`, `$HOME`, `|`, `>`, кавычки и `;` не получают
shell-смысла. Это безопаснее и переносимее.

Bare `Path("git")` ищется в `PATH` именно окружения команды. Путь с компонентом
каталога запускается напрямую.

`string()` предназначен для диагностики, не для round trip через shell. Он может
содержать секреты в аргументах, поэтому не логируйте его бездумно.

## 7. `os/process`

### Запуск и ожидание

```okkama
let process = Process(command: command);
defer process.close();

let pid = process.id();        // здесь процесс лениво стартует
let exit = await process.exit();
if !exit.success() {
    report_failure(exit.kind());
}
```

Первое `id`, `running`, `exit`, `signal` или действие stream запускает процесс ровно
один раз. `close()` не запускает ещё не начатый процесс; для работающего он
останавливает, reap-ит и отбрасывает непрочитанные pipe-данные.

`Exit.kind()` возвращает `exited`, `interrupted`, `terminated`, `killed` или `other`.
`code()` допустим только для `exited`, иначе бросает `MissingExitCode`. `success()`
истинен только для обычного exit code 0.

### Сигналы

```okkama
process.signal(Signal.interrupt);
process.signal(Signal.terminate);
process.signal(Signal.kill);
```

Набор намеренно переносимый и мал. Неподдерживаемое действие даёт
`UnsupportedSignal`.

### stdin/stdout/stderr процесса

```okkama
let process = Process(command: command);
defer process.close();

let input = ProcessInput(origin: process);
let output = ProcessOutput(origin: process);
let errors = ProcessErrors(origin: process);

input.write(ByteSnapshot(origin: request));
input.close(); // послать EOF ребёнку, не закрывая весь Process

let first_stdout = output.bytes(65536);
let first_stderr = errors.bytes(65536);
let exit = await process.exit();
```

Если ребёнок пишет много, stdout и stderr надо дренировать одновременно с
ожиданием. Последовательное «сначала wait, потом read» может зависнуть на полном
pipe. Для обычного запуска с захватом используйте `CommandCapture`.

## 8. `os/command_capture`

Это безопасный bounded-сценарий: стартовать, при необходимости записать stdin,
закрыть его, одновременно собрать stdout/stderr и дождаться exit.

```okkama
let completed = await CommandCapture(
    origin: command,
    limit: 16 * 1024 * 1024,
).completion();

if completed.exit().success() {
    consume(completed.output());
}
else {
    diagnose(completed.errors());
}
```

С бинарным stdin:

```okkama
let completed = await CommandCapture(
    origin: command,
    input: request_bytes,
    limit: 16 * 1024 * 1024,
).completion();
```

Лимит положителен и применяется независимо к stdout и stderr. Переполнение даёт
`CaptureLimitExceeded`, где `stream()` показывает `output` или `errors`. Это защита
от процесса, который печатает неограниченный объём.

Результаты остаются `@byte`; декодируйте явно:

```okkama
let text = Utf8Decoded(origin: completed.output()).string();
```

Не предполагайте, что дочерняя программа пишет UTF-8.

## 9. `os/pipeline_capture`

Pipeline соединяет stdout каждой команды со stdin следующей без shell:

```okkama
let completed = await PipelineCapture(
    commands: [producer, transformer, consumer],
    limit: 16 * 1024 * 1024,
).completion();
```

Вариант с бинарным входом первой команды:

```okkama
let completed = await PipelineCapture(
    commands: [decoder, validator],
    input: payload,
    limit: 16 * 1024 * 1024,
).completion();
```

`CompletedPipeline` содержит:

- `exits()` — один `Exit` на команду в исходном порядке;
- `output()` — stdout последней команды;
- `errors()` — один stderr-массив на каждую команду.

Завершение ждёт все процессы и не возвращает частичный результат. Пустой список
бросает `EmptyPipeline`. Лимит применяется к финальному stdout и каждому stderr;
`PipelineCaptureLimitExceeded` сообщает индекс команды и поток.

Shell-конструкцию

```text
producer | transformer | consumer
```

не надо собирать строкой: перечисление `Commanding` сохраняет точные границы
аргументов и не открывает shell injection.

## 10. `os/storage`

`Storage` привязывает наблюдение к пути:

```okkama
let storage = Storage(path: Path(origin: "data"));
let usage = storage.usage();
Stdout().writeln("capacity: ", usage.capacity().string());
Stdout().writeln("used: ", usage.used().string());
Stdout().writeln("available: ", usage.available().string());
```

Все значения — `BigInt`, потому что размер носителя не обязан помещаться в `int`.
`usage()` возвращает новый снимок: диск может измениться сразу после вызова. Не
кешируйте его как вечную истину. Ошибки провайдера превращаются в `StorageFailure`.

## 11. Практические рецепты

### Запустить команду и получить UTF-8 текст

```okkama
let command is Commanding = InheritedCommand(
    program: Path(origin: "git"),
    arguments: ["status", "--short"],
);
let completed = await CommandCapture(
    origin: command,
    limit: 4 * 1024 * 1024,
).completion();
confirm completed.exit().success()
    else throw CommandFailed(completed.exit());
let status = Utf8Decoded(origin: completed.output()).string();
```

### Изолированное окружение ребёнка

```okkama
let environment = Environment(values: [
    "PATH": Environment().variable("PATH"),
    "LANG": "C.UTF-8",
]);
let command = Command(
    program: Path(origin: "compiler"),
    arguments: ["source.ok"],
    environment: environment,
    directory: WorkingDirectory(),
);
```

### Ограничить параллелизм числом CPU

```okkama
let permits = Semaphore(permits: Host().cpu_count());
```

Это лишь разумная стартовая оценка; workload может быть I/O-bound или контейнер
может накладывать другие лимиты.

## 12. Частые ошибки

1. Не передавайте целую shell-команду как `program`.
2. Не ждите expansion переменных, glob и redirection — shell не участвует.
3. Не логируйте `Command.string()` без проверки секретов.
4. Не меняйте process-global cwd; задавайте `directory` команды.
5. Не изменяйте глобальное окружение; стройте новый `Environment` через `add/diff`.
6. Не ждите процесс до дренирования больших stdout/stderr; используйте capture.
7. Не декодируйте output неявно.
8. Не вызывайте `Exit.code()` для signal termination.
9. Не считайте `StorageUsage` постоянной характеристикой.

## 13. Карта выбора

| Нужно | Используйте |
|---|---|
| snapshot окружения | `Environment()` |
| модифицированное окружение | `add` / `diff` |
| домашний каталог | `HomeDirectory` |
| разобрать PATH | `SearchPath` |
| сведения об ОС | `Platform` |
| hostname / CPU | `Host` |
| пользователь / группа | `User` / `Group` |
| текущий каталог как capability | `WorkingDirectory` |
| описать воспроизводимую команду | `Command` |
| коротко унаследовать контекст | `InheritedCommand` |
| вручную управлять процессом | `Process` и stream views |
| безопасно захватить результат | `CommandCapture` |
| соединить команды | `PipelineCapture` |
| наблюдать место на носителе | `Storage().usage()` |

### Согласованность StorageUsage (v4)

`StorageUsage(capacity: BigInt(...), available: BigInt(...))` хранит два числа.
`used()` вычисляет точную разность без сужения до `int`; `fits(amount)` проверяет
доступную ёмкость. Отрицательные значения и available > capacity отвергаются.
Это снимок наблюдения, поэтому `fits()` не резервирует место и не обещает успех
будущей записи. Для нового снимка вызовите `Storage.usage()` снова.
