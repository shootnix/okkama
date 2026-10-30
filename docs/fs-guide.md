# Файловая система Okkama: подробное руководство

Модули `fs/*` покрывают пути, файлы, каталоги, метаданные, блокировки, временные
ресурсы, glob-поиск, ссылки и наблюдение за изменениями. API переносим: прикладной
код не выбирает разделитель текущей ОС и не работает с нативными дескрипторами.

## 1. Четыре разных понятия

Не смешивайте их:

| Объект | Что означает |
|---|---|
| `Path` | значение пути, ресурс может не существовать |
| `FileEntry` | файловая запись по этому пути |
| `File` | лениво открываемый поток с режимом, кодировкой и позицией |
| `FileStat` | снимок метаданных в конкретный момент |

Создание `Path("report.txt")` не создаёт файл. Создание `FileEntry` тоже ничего не
открывает. `File` открывает нативный handle лениво при первом ресурсном действии.

`File` реализует `Closing`; `binary()` возвращает `FileBytes` с ролями
`Reading, Writing`, а `text()` — `FileText` с ролями `ReadingText, WritingText`.
Представления сохраняют ресурс, делят позицию и состояние закрытия. Уход
представления из области видимости не закрывает владельца; закрытие владельца
делает все представления непригодными для дальнейшего I/O. Выбор представления
не меняет режим и кодировку файла. `text().text()` читает остаток с сохранением
переводов строк и возвращает пустую строку на EOF; `line()` бросает `EndOfInput`.

## 2. `fs/path`

### Обычный путь

```okkama
let config = Path(origin: "config/application.toml");
Stdout().writeln(config.string());
let parent = config.parent();
let cache = parent.child("cache");
```

`absolute()` сообщает, абсолютен ли путь. `Path` реализует `Comparing<Path>`, поэтому
пути можно стабильно сортировать. Сравнение и строка используют нормализованное
представление выбранного стиля, но не обращаются к файловой системе.

### Переносимое соединение частей

```okkama
let log = Path("var", "lib", "okkama", "events.log");
```

Вариадический конструктор `Path(parts is ...string)` сам получает стиль текущей
платформы через runtime provider. Пустой список частей и пустая часть отвергаются
через `InvalidPath`; более поздняя абсолютная часть заменяет всё, что было до неё.
Пользователь не выбирает `/` или `\`. Это основной API для пути, который должен
работать на разных ОС.

Явный `PathStyle.posix` или `PathStyle.windows` нужен только при обработке чужого
формата, например пути из конфигурации для удалённой машины:

```okkama
let remote = Path(origin: "C:\\build\\result", style: PathStyle.windows);
```

Не используйте явный стиль для обычного локального пути.

`Path.child(part)` принимает один компонент, а не произвольный путь с несколькими
разделителями. Недопустимые значения бросают `InvalidPath`.

## 3. `fs/file`: выбор режима

`FileMode`:

| Режим | Чтение | Запись | Начальное состояние |
|---|---:|---:|---|
| `reading` | да | нет | файл обязан существовать |
| `writing` | нет | да | создать или обнулить |
| `appending` | нет | да | создать, каждая запись в конец |
| `reading_and_writing` | да | да | файл обязан существовать, не обнулять |
| `reading_and_appending` | да | да | создать при отсутствии, запись только в конец |

Конструкторы явно задают режим и слой данных:

```okkama
File(mode: FileMode.reading, path: path)
File(mode: FileMode.reading, path: path, encoding: "utf-8")
File(mode: FileMode.reading, path: path, binary: true)
File(mode: FileMode.reading, path: path, encoding: "latin-1", binary: false)
```

По умолчанию файл текстовый, кодировка — строгий UTF-8. В binary-режиме кодировку
указывать нельзя.

Поддерживаемые точные имена: `utf-8`, `utf-16-le`, `utf-16-be`, `utf-32-le`,
`utf-32-be`, `ascii`, `latin-1`. Неизвестное имя бросает
`UnsupportedFileEncoding`. BOM не распознаётся и не записывается автоматически;
порядок байтов всегда явный.

## 4. Чтение текста по строкам

`file.text().line()` читает одну физическую строку и возвращает её без LF и
предшествующего CR.

Полный пример:

```okkama
import "fs/path" take Path;
import "fs/file" take File, FileMode;
import "io/stdin" take EndOfInput;
import "io/stdout" take Stdout;

let file = File(
    mode: FileMode.reading,
    path: Path(origin: "data/events.log"),
);
defer file.close();

try {
    for {
        let line = file.text().line();
        Stdout().writeln(line);
    }
}
catch error is EndOfInput {
    // Весь файл прочитан.
}
```

Последняя непустая строка без LF возвращается нормально. `EndOfInput` возникает
только при следующей попытке. Библиотечная память ограничена длиной самой длинной
физической строки, а не размером файла. Следовательно, файл в 20 ГБ можно читать
так же; но одна строка в 20 ГБ всё равно потребует память под эту строку.

## 5. Бинарное чтение порциями

`read(limit)` доступен только в binary-режиме и возвращает не больше `limit` байтов.
Пустой массив — устойчивый EOF.

```okkama
import "fs/path" take Path;
import "fs/file" take File, FileMode;

let file = File(
    mode: FileMode.reading,
    path: Path(origin: "images/archive.bin"),
    binary: true,
);
defer file.close();

for {
    let chunk = file.binary().bytes(1024 * 1024);
    last if chunk.length() == 0;
    process(chunk);
}
```

Такой цикл ограничивает библиотечную буферизацию запрошенным размером порции. Он
пригоден для многогигабайтных файлов. `limit <= 0` бросает `InvalidReadLimit`.

## 6. Запись текста и байтов

### UTF-8 текст

```okkama
let report = File(
    mode: FileMode.writing,
    path: Path(origin: "out/report.txt"),
);
defer report.close();

report.text().write("status: ", "ready");
report.text().writeln();
report.text().writeln("items: ", count.string());
report.text().flush();
```

`writeln` добавляет ровно один закодированный LF после всех аргументов. `close()`
идемпотентен и проталкивает ожидающий вывод.

### Бинарный файл

```okkama
import "io/decorators" take ByteSnapshot;

let file = File(
    mode: FileMode.writing,
    path: Path(origin: "out/image.dat"),
    binary: true,
);
defer file.close();

file.binary().write(ByteSnapshot(origin: bytes));
file.binary().flush();
```

Текстовая запись в binary-файл и байтовая запись в текстовый файл бросают
`InvalidFileOperation`. Так же отвергается чтение из write-only режима и запись в
read-only режим.

## 7. Update-режимы и позиция

В `reading_and_writing` чтение и запись используют одну логическую позицию.
Библиотека компенсирует скрытое read-ahead нативного буфера: запись происходит там,
где пользователь закончил логическое чтение. Перед чтением ожидающая запись
автоматически flush-ится.

`FilePosition` отделяет операции над байтовой позицией:

```okkama
let file = File(
    mode: FileMode.reading_and_writing,
    path: path,
    binary: true,
);
defer file.close();

let position = FilePosition(origin: file);
position.seek(offset: 0, from: SeekOrigin.end);
let end = position.offset();
position.truncate(end - 16);
```

`SeekOrigin` имеет `start`, `current`, `end`. Позиция, seek и truncate доступны
только в binary-режиме: в переменной кодировке байтовый offset не является позицией
Unicode-текста.

## 8. `FileEntry` и полное содержимое

```okkama
let entry = FileEntry(path: Path(origin: "data/state.db"));
if entry.existence() {
    let stat = entry.stat();
    Stdout().writeln(stat.size().string());
}
```

`existence()` возвращает `false` только для отсутствующего пути. Permission и I/O
ошибки не маскируются под отсутствие.

`remove()` удаляет запись, `move(destination)` перемещает её. `stat()` возвращает
снимок метаданных.

```okkama
let all = FileContent(origin: entry).bytes();
```

`FileContent.bytes()` материализует весь файл в памяти. Не используйте его для
неизвестного или большого размера; открывайте `File` и читайте порциями.

Равенство двух `FileEntry` означает равенство идентичности файловой записи, а не
побайтовое сравнение содержимого. Специального `fs/compare` нет: если нужно сравнить
содержимое, напишите инкрементальный алгоритм поверх двух binary `File`.

## 9. `fs/stat`

`FileStat` объединяет:

- `kind()` — `FileKind`;
- `size()` — размер в байтах;
- `mode()` — `PermissionMode`;
- `times()` — `FileTimes`;
- `identity()` — `FileIdentity`.

`FileKind` различает обычный файл, каталог, символическую ссылку и другие
платформенные виды. `FileIdentity(device, inode)` — переносимая библиотечная форма
идентичности в пределах поддерживаемой файловой системы. `same(other)` сравнивает
идентичности без публичных device/inode getters; не превращайте её в имя.

```okkama
let mode = entry.stat().mode();
if mode.owner().writable() {
    update(entry);
}
```

`PermissionMode.owner()`, `group()`, `others()` возвращают `Permissions` с
`readable()`, `writable()`, `executable()`. Это наблюдение битов режима, а не
гарантия, что текущий процесс реально сможет выполнить операцию.

`FileTimes` содержит access/modification/change и, когда платформа предоставляет,
creation. Отсутствующий creation time бросает `MissingCreationTime`.

## 10. `fs/lock`

Блокировка ставится на уже открытый `File`, а не повторно разрешает его путь:

```okkama
let file = File(
    mode: FileMode.reading_and_writing,
    path: Path(origin: "data/state.db"),
    binary: true,
);
defer file.close();

let lock = FileLock(origin: file, mode: FileLockMode.exclusive);
defer lock.close();

update_database(file);
```

`shared` совместим только с другими shared-блокировками. `exclusive` конфликтует
со всеми. По умолчанию ожидание блокирует выполнение до получения lock. Для
немедленной попытки:

```okkama
let lock = FileLock(
    origin: file,
    mode: FileLockMode.exclusive,
    waiting: FileLockWaiting.immediate,
);
```

Если lock занят, возникает `UnavailableFileLock`. Блокировки whole-file,
advisory и нормализованы runtime provider-ом на всех платформах. Второй живой lock
на том же состоянии `File` запрещён. Закрытие файла освобождает его lock; явный
`lock.close()` идемпотентен.

Advisory означает: корректно сотрудничающие процессы соблюдают lock, но он не
магически запрещает любому чужому коду обращаться к файлу.

## 11. `fs/directory`

```okkama
let directory = Directory(path: Path(origin: "out/cache"));
if !directory.existence() {
    directory.create();
}
```

`Directory` предоставляет `path`, `existence`, `create`, `remove`, `move`.
`remove()` принимает только пустой каталог; иначе бросает `DirectoryNotEmpty`.

Непосредственные записи:

```okkama
for DirectoryEntries(origin: directory) -> entry {
    Stdout().writeln(entry.path().string());
}
```

Рекурсивный обход:

```okkama
for Descendants(origin: directory) -> entry {
    inspect(entry.path(), entry.kind());
}
```

Оба обхода лексикографически отсортированы. Символические ссылки считаются листьями
и не используются для рекурсии.

`DirectoryTree(origin: directory).remove()` явно удаляет дерево и возвращает `void`.
Конструирование `DirectoryTree` не удаляет ничего; проекции с побочным удалением нет.

## 12. `fs/glob`

`MatchingPaths` ищет пути относительно корня:

```okkama
let source = Directory(path: Path(origin: "src"));
for MatchingPaths(root: source, pattern: "**/*.ok") -> path {
    Stdout().writeln(path.string());
}
```

Практические шаблоны:

```okkama
MatchingPaths(root: project, pattern: "*.md");
MatchingPaths(root: project, pattern: "tests/test_?.ok");
MatchingPaths(root: project, pattern: "assets/[a-z]*.png");
MatchingPaths(root: project, pattern: "**/*.json");
```

В шаблоне разделитель всегда `/`, даже на Windows. Поддерживаются `*`, `?`, классы
символов и отдельные рекурсивные сегменты `**`. Результат отсортирован. Поиск не
переходит через symlink. Синтаксическая ошибка шаблона — `InvalidGlobPattern`.

## 13. `fs/link`

### Символическая ссылка

```okkama
let link = SymbolicLink(path: Path(origin: "current"));
link.create(target: Path(origin: "releases/2026-08"));

if link.existence() {
    Stdout().writeln(link.target().string());
}
defer link.remove();
```

`existence()` проверяет сам link, поэтому сломанная ссылка тоже существует.
`target()` возвращает сырой target ровно в той форме, в которой он записан, а не
разрешённый абсолютный путь. Target не обязан существовать во время создания.
`remove()` удаляет только ссылку.

### Жёсткая ссылка

```okkama
let link = HardLink(
    source: FileEntry(path: Path(origin: "data/original.db")),
    destination: Path(origin: "data/alias.db"),
);
link.create();
```

Создание обоих видов никогда не заменяет существующее назначение. Каталог как
источник hard link переносимо запрещён.

## 14. `fs/temp`

### Временный файл

```okkama
let temporary = TemporaryFile(directory: cache, prefix: "upload-");
defer temporary.close();

temporary.write(ByteSnapshot(origin: payload));
temporary.flush();
let path = value TemporaryFilePath(origin: temporary);
```

Временный файл бинарный и реализует `Reading`, `Writing`, `Closing`. При закрытии он
удаляется. Чтобы сохранить его в новое постоянное назначение:

```okkama
let preservation = TemporaryFilePreservation(
    origin: temporary,
    destination: final_path,
);
preservation.preserve();
let entry = preservation.destination();
```

После успешного сохранения временное удаление больше не уничтожает постоянный файл.

### Временный каталог

```okkama
let scratch = TemporaryDirectory(parent: workspace, prefix: "build-");
defer scratch.close();
compile_into(scratch.path());
```

Закрытие рекурсивно очищает каталог, не переходя через symlink. `close()`
идемпотентен.

## 15. `fs/watch`

Наблюдение асинхронно и не маскируется под синхронный `Cursor`:

```okkama
let watch = FileWatch(path: Path(origin: "config"));
defer watch.close();

for {
    let event = await watch.event();
    match event.kind() {
        case FileEventKind.created { added(event.path()); }
        case FileEventKind.modified { reloaded(event.path()); }
        case FileEventKind.removed { forgotten(event.path()); }
        case FileEventKind.renamed {
            moved(event.previous_path(), event.path());
        }
    }
}
```

`previous_path()` имеет смысл только для rename; иначе бросает
`MissingPreviousPath`. События отражают модель runtime provider-а: потребитель
должен быть готов повторно проверить актуальное состояние, потому что файловая
система меняется между событием и обработкой.

## 16. Практический рецепт: публикация нового файла

```okkama
let target = Path(origin: "data/state.db");
let parent = Directory(path: target.parent());
let temporary = TemporaryFile(directory: parent, prefix: ".state-");
defer temporary.close();

temporary.write(ByteSnapshot(origin: encoded_state));
temporary.flush();

let preservation = TemporaryFilePreservation(
    origin: temporary,
    destination: target,
);
preservation.preserve();
let stored = preservation.destination();
```

Назначение должно отсутствовать: существующий файл не заменяется. В пределах
одной файловой системы используется атомарное перемещение; при переходе между
файловыми системами — копирование ограниченными порциями. Это не атомарная замена
существующего файла и не транзакция. `destination()` возвращает только локатор;
сохранение выполняет исключительно `preserve()`.

## 17. Частые ошибки

1. Не вставляйте нативный разделитель вручную — используйте `Path("a", "b", …)`.
2. Не вызывайте `FileContent.bytes()` для огромного файла.
3. Не смешивайте text и binary операции одного `File`.
4. Не используйте byte seek в текстовом режиме.
5. Не считайте `existence() == false` универсальным ответом на любую ошибку.
6. Не следуйте symlink при рекурсивном удалении.
7. Не сравнивайте `FileEntry`, ожидая сравнения содержимого.
8. Не полагайтесь на advisory lock против несотрудничающей программы.
9. `File`, `FileLock` и временные ресурсы закрываются через `destroy` владельца;
   `close()`/`defer` позволяют завершить эффект раньше. Для `FileWatch` сохраняется
   явное закрытие через `defer`. Не считайте native handle автоматическим scope-owner.

## 18. Карта выбора

| Нужно | Используйте |
|---|---|
| представить путь | `Path` |
| соединить части переносимо | `Path(parts...)` |
| читать/писать потоково | `File` |
| прочитать маленький файл целиком | `FileContent` |
| проверить/удалить/переместить файл | `FileEntry` |
| получить метаданные | `FileStat` |
| seek/truncate | `FilePosition` в binary-режиме |
| блокировать открытый файл | `FileLock` |
| перечислить каталог | `DirectoryEntries` |
| рекурсивно обойти | `Descendants` |
| найти по шаблону | `MatchingPaths` |
| создать ссылку | `SymbolicLink` / `HardLink` |
| безопасный временный ресурс | `TemporaryFile` / `TemporaryDirectory` |
| ждать изменений | `FileWatch` |
