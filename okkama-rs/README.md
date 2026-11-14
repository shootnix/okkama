# okkama-rs

Реализация языка Okkama на Rust: компилятор (лексер, парсер, анализ,
lowering в OIR-1), бэкенд Sparrow, VM Sparrow с JIT
(Cranelift) и сборщиком циклов, рантайм и natives std, LSP, форматтер и
линтер. Единственный движок Okkama — Sparrow.
Conformance (`../conformance`) проверяет поведение по общим ожиданиям.

## Сборка

Нужен Rust 1.97 или новее (edition 2024; столько требует Cranelift); зависимости — из crates.io.
Сборку и установку всего языка из корня репозитория делает `make install`.

```sh
cd okkama-rs
cargo build              # отладочная сборка: target/debug/okkama
cargo build --release    # оптимизированная: target/release/okkama
```

## Использование

```sh
# токены
cargo run -p okkama-cli -- tokens examples/hello.ok
cargo run -p okkama-cli -- tokens examples/hello.ok --format json

# дерево разбора
cargo run -p okkama-cli -- ast examples/hello.ok
cargo run -p okkama-cli -- ast examples/hello.ok --format json
cargo run -p okkama-cli -- ast examples/hello.ok --spans

# IR модуля, main() и её тела
cargo run -p okkama-cli -- ir examples/arithmetic.ok
cargo run -p okkama-cli -- ir examples/arithmetic.ok --format text

# сборка SPB-артефакта
cargo run -p okkama-cli -- build examples/arithmetic.ok --target sparrow -o out.spb
sparrow out.spb
# Код завершения процесса: 2 (результат main).

# исходный текст из stdin
cat examples/hello.ok | cargo run -p okkama-cli -- ast -
cat examples/arithmetic.ok | cargo run -p okkama-cli -- ir -

# production-разбор: тела test/fake не разбираются, а удаление
# тестовых объявлений — отдельный явный шаг
cargo run -p okkama-cli -- ast examples/tests-and-fakes.ok --mode production
cargo run -p okkama-cli -- ast examples/tests-and-fakes.ok --mode production --prune-tests

# форматирование (perlstyle): вывод, запись на месте, проверка
okkama fmt examples/hello.ok
okkama fmt -w src/*.ok
okkama fmt --check src/*.ok

# ошибки, предупреждения и линтер
okkama lint src/*.ok

# сервер языка для редакторов (LSP через stdio)
okkama lsp

cargo run -p okkama-cli -- --help
```

Результат печатается в stdout, диагностика — в stderr. При ошибке stdout пуст.

| Код выхода | Значение |
|---|---|
| 0 | успех |
| 1 | ошибка исходника, lowering или codegen, включая неверный UTF-8 |
| 2 | неверная командная строка |
| 3 | ошибка чтения входа или записи результата |

Запуск внутри Rust-процесса и Sparrow-байткод:

```sh
cargo run --offline -p okkama-cli -- run examples/arithmetic.ok
cargo run --offline -p okkama-cli -- run --engine sparrow examples/arithmetic.ok
cargo run --offline -p okkama-cli -- build examples/arithmetic.ok --target sparrow -o out.spb
cargo run --offline -p okkama-cli -- exec out.spb
```

Sparrow — регистровая VM: OIR компилируется в SPB 9, файл несёт программу
с импортированной std и исполняется без исходников и Java. `sparrow-core`
(версия Tomato) содержит значения, кучу, GC, планировщик, JIT и workers.
`okkama-runtime-core` сохраняет native-провайдеры и их значения, host context,
stdout, scheduler bridge, точную арифметику и вычисление констант компилятора.
Результат main в 0..255 становится кодом выхода; неперехваченный отказ —
диагностикой в stderr с кодом 1. Запуск исходника добавляет место отказа и
стек вызовов; `sparrow` и `okkama exec` — стек в терминах Sparrow.
`OKKAMA_TRACE=off` печатает только сам отказ.

IR-интерпретатор и зависевший от него `okkama repl` удалены (#584).
`--engine ir` и `OKKAMA_ENGINE=ir` возвращают понятную ошибку.
`--engine sparrow` и `OKKAMA_ENGINE=sparrow` явно выбирают Sparrow. OIR и его passes остаются частью компилятора.

[План полного рантайма, GC и JIT](docs/sparrow-runtime-roadmap.md).

Приоритет разработки — полнота `conformance`, затем `examples` и всей `std`.
[История независимых прогонов, исходные результаты и очередь реализации](docs/conformance-progress.md).

Sparrow также собирается отдельным бинарником, без зависимости от компилятора:

```sh
cargo build --offline --release -p sparrow-cli
./target/release/sparrow check out.spb
./target/release/sparrow out.spb [ARGS...]
./target/release/sparrow --disassemble out.spb -o out.spa
```


### IR модуля и функций

Команда `ir <FILE|-> [--format text]` принимает полный исходник Okkama.
Например:

```okkama
module app v1.0.0;
main() is int {
    1 + 1;
}
```

Результат:

```text
module app v1.0.0 {
  func main() -> int {
    %0 = int.const 1
    %1 = int.const 1
    %2 = int.add %0, %1
    ret %2
  }
}
```

IR сохраняет имя и версию модуля, импорты и саму функцию. Все выражения тела
переводятся в общую последовательность инструкций; нумерация значений локальна
для функции. `ret` завершает её блок возвратом значения последнего выражения
или явного `return`. Поле `LinearExpr.result` остаётся внутренним средством
построения выражений и не выводится вместо возврата функции.

На этом этапе поддерживаются синхронные функции с результатом `int`, целые
литералы, сложение, безусловные выражения и `return` со значением. Для других
конструкций команда выдаёт `lowering error` с именем файла, строкой и столбцом.
Исходник не сокращается до первого выражения, неподдерживаемый код не пропускается.
Только успешное преобразование всего модуля печатает IR в stdout.

### Выбор backend и сборка

```sh
cargo run -p okkama-cli -- build examples/arithmetic.ok --target sparrow -o out.spb
cargo run -p okkama-cli -- build examples/arithmetic.ok --target=sparrow --output=out.spb
cat examples/arithmetic.ok | cargo run -p okkama-cli -- build - --target sparrow -o out.spb
cargo run -p okkama-cli -- build --help
```

В `build` обязательны входной файл (или `-`), `-o`/`--output`; `--target sparrow` необязателен.
`--backend` остаётся алиасом `--target`. Разные цели в двух ключах дают ошибку.
Команда выполняет цепочку `source → AST → module IR → backend → файл`.
Backend получает готовый модуль IR и возвращает байты артефакта; файловый ввод
и вывод остаются в CLI. Неизвестное имя backend считается ошибкой аргументов.

`codegen::generate(module, backend, options)` передаёт IR выбранному backend.
В `Options.artifact_name` CLI передаёт имя выходного файла без каталога и
расширения. Sparrow генерирует проверенный SPB; запуск: `okkama exec out.spb`
или `sparrow out.spb`. При ошибке генерации выходной файл не перезаписывается.

### Токены

```text
$ okkama tokens examples/hello.ok
1:1-1:7      MODULE         "module"
1:8-1:11     IDENTIFIER     "app"
...
8:37-8:41    STRING         "endl"  string endl "\n"
```

В каждой строке: диапазон `строка:столбец-строка:столбец` (конец
исключающий), вид токена (имена совпадают с `Okkama::TokenType`), исходное
написание в кавычках, а для литералов — разобранное значение. С
`--format json` выводится массив объектов `kind`, `lexeme`, `span`, `value`.

### Дерево (`tree`)

```text
$ okkama ast examples/hello.ok
Program
  module: ModuleDecl
    path: [app]
    version: v1.0.0
  imports:
    - ImportDecl
      path: "io/stdout"
      alias: (none)
      take: [Stdout]
  items:
    - ActionDecl
      async: false
      visibility: public
      ...
```

Каждый узел печатает свой вид, а под ним, с отступом, подписанные поля.
Скаляры и списки скаляров стоят в строке поля. Вложенные узлы и списки узлов
идут ниже, и элементы списков начинаются с `- `. Отсутствующее необязательное
поле печатается как `(none)`. Строковые данные (значения литералов, пути
импорта, имена тестов) выводятся в кавычках с JSON-экранированием. Операторы
и имена печатаются без кавычек.

### JSON (`json`)

Каждый узел — объект, у которого первый ключ `"node"` (вид узла), затем,
если указан `--spans`, идёт `"span"`, затем поля в фиксированном порядке.
Список — массив, отсутствующее поле — `null`. Цифры целых литералов остаются
строкой, поэтому большие значения не округляются. С `--spans` имя
превращается в `{"name": "...", "span": {...}}`. Вывод детерминирован: ни
адресов, ни хешей, ни случайных идентификаторов в нём нет.

### Source spans и диагностика

Все позиции внутри компилятора — диапазоны байтов UTF-8 `start..end` (конец
исключающий). Строки и столбцы вычисляются из них только для вывода:

- строки разделяет только LF; CR относится к строке, которую он завершает,
  поэтому файлы с CRLF нумеруются так же, как с LF, а одиночный CR новую
  строку не начинает;
- строки и столбцы считаются с 1;
- столбец считается в Unicode scalar values, поэтому табуляция, CR, `é` и
  `🙂` занимают по одному столбцу.

`--spans` в `tree` добавляет к узлам и именам `@строка:столбец-строка:столбец`.
В `json` у узлов появляется объект `span` с полями `start`, `end`
(байты), `start_line`, `start_column`, `end_line`, `end_column`.

Диагностика выглядит так:

```text
main.ok:3:15: syntax error at "2": Expect ";" after let declaration.
  |
3 |     let x = 1 2;
  |               ^
```

Тексты синтаксических ошибок намеренно совпадают с Perl-парсером слово в
слово. Лексер сообщает все лексические ошибки файла. Если они есть, парсер не
запускается. Парсер останавливается на первой синтаксической ошибке.

## Режимы разбора

- `--mode full` (по умолчанию) разбирает всё, включая тела `test` и `fake`.
  Это грамматика `okkama -t`, и в этом режиме ничего не теряется.
- `--mode production` работает по спецификации §14: заголовки `test`/`fake`
  разбираются, тела проходят как сбалансированные скобки без разбора
  внутренней грамматики, а лексические ошибки в них по-прежнему учитываются.
  Узлы остаются в дереве с телом `SkippedBody`.
- `--prune-tests` — отдельный проход (`syntax::prune::strip_test_items`),
  который удаляет все `test` и role `fake`. Связка
  `--mode production --prune-tests` даёт то же дерево, что production-режим
  Perl-парсера.

## Форматтер и сервер языка

`okkama fmt` печатает код в одном стиле (perlstyle): отступ 4 пробела,
каждый оператор и член объекта на своей строке, `{` на строке конструкции,
`else`/`catch` на новой строке после `}`, пробелы вокруг бинарных операторов и
после запятых, пустая строка между объявлениями и между членами объекта.
Списки (аргументы, параметры, литералы) переносятся по элементу на строку,
если не помещаются в 100 столбцов, если автор их уже перенёс или поставил
висячую запятую; цепочки `&&`/`||` переносятся перед оператором. Пустые
строки автора сохраняются. Меняются только пробелы и переводы строк: после
форматирования токены и комментарии сверяются с исходными, при расхождении
файл не трогается (`formatter error`). Файл с синтаксической ошибкой не
форматируется. `--check` — код 1 и список неотформатированных файлов.

`okkama lint` — то же, что `okkama check`, плюс правила линтера
(`ide::lint`): `unused-import`, `unused-let` (кроме имён с `_` и объектов с
`destroy`), `unused-private`, `unreachable`, `snake-case`; код 1, если есть
находки.

`okkama lsp` — сервер языка (`syntax::format`, `ide`, `lowering::inspect`):
диагностики и линтер, наведение, переход к определению, ссылки и подсветка
вхождений, переименование локальных, автодополнение (члены по типу, области
видимости, `take`), подсказка сигнатуры, структура файла и форматирование.
Пока строка не дописана, типы берутся из последней успешной проверки, а
только что набранный получатель выводится по имени и цепочке вызовов.
Клиент — расширение `vscode-okkama`.

## Структура

```text
okkama-rs/
  Cargo.toml, Cargo.lock      workspace без внешних зависимостей
  crates/okkama-compiler/     библиотека
    src/source/               исходный текст, Span, строки и столбцы
    src/diagnostics/          структурированная диагностика и её рендеринг
    src/syntax/token.rs       виды токенов и значения литералов
    src/syntax/lexer/         лексер (+ модульные тесты)
    src/syntax/ast/           типизированное AST: decl/stmt/expr/types
    src/syntax/parser/        recursive descent: mod, decl, stmt, expr, types
    src/syntax/prune.rs       удаление test/fake для production
    src/syntax/print/         принтеры tree, json и токенов
    src/ir/                   модуль, функции, инструкции, терминаторы, принтер
    src/lowering/             AST → IR, разворачивание выражений в инструкции
    src/codegen/mod.rs        выбор backend и общий интерфейс генерации
    src/analysis, jit         границы будущих стадий (только документация)
    tests/                    интеграционные тесты, фикстуры, снапшоты, корпус
  crates/okkama-cli/          бинарник `okkama`: файлы, stdin, опции, коды выхода
  crates/blade/               библиотека менеджера проектов Blade (`blade new`)
  crates/blade-cli/           бинарник `blade`
  examples/                   небольшие корректные программы на Okkama
  docs/                       architecture.md, syntax-coverage.md, compatibility.md
```

Библиотека не читает аргументы командной строки, ничего не печатает и не
завершает процесс. Всё это делает CLI.

## Тесты и проверки

Для сборки и проверки достаточно Rust; криптография встроена в Sparrow.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`cargo test` проверяет следующее:

- модульные тесты лексера, источника и диагностики;
- парсер: объявления, операторы, выражения, приоритеты, ошибки;
- устойчивость: глубокая вложенность, все префиксы программы, удаление
  любого токена, случайный ввод;
- принтеры: каждый вид узла, снапшоты, корректность JSON, детерминизм;
- корпус: все `.ok` в `std/`, `examples/` и `okkama-rs/examples/` в обоих
  режимах, а для каждого случая `conformance/cases` — ожидаемый исход
  разбора из `case.json`;
- IR: выражения, функции, возвраты и source spans;
- CLI: команды, stdin, форматы, коды выхода, диагностика lowering.
- Sparrow: детерминизм SPB, проверка IR, исполнение программ и бюджетов.

Корпусные тесты требуют полного checkout репозитория. Пропустить их можно
только явно: `OKKAMA_SKIP_CORPUS=1`.

Снапшоты принтеров обновляются осознанно, после чего diff нужно просмотреть:

```sh
OKKAMA_UPDATE_SNAPSHOTS=1 cargo test -p okkama-compiler --test printer
```

## Документация

- [docs/architecture.md](docs/architecture.md) — устройство, решения, где
  добавлять токен, AST-узел и правило парсинга, как тестировать новую
  конструкцию, границы будущих стадий;
- [docs/syntax-coverage.md](docs/syntax-coverage.md) — таблица
  «конструкция → источник правила → реализация → тесты»;
- [docs/compatibility.md](docs/compatibility.md) — совместимость с
  документацией и эталонной реализацией, расхождения движков и их классификация.
