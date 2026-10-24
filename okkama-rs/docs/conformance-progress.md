# Полнота Rust-интерпретатора и Sparrow

Цель: `conformance` → `examples` → вся `std`. Ожидаемые результаты тестов не
подгоняются под возможности реализации.

## Итог — 24 октября 2026

`full-release`, 772 обязательных случая: **JVM 772/772, IR 772/772,
Sparrow 772/772** (все режимы, включая `test`). Паритет `std` (237 файлов
в тестовом режиме) и `examples` (422 программы) с JVM — в
[sparrow-runtime-roadmap.md](sparrow-runtime-roadmap.md#состояние--24-октября-2026-паритет-с-jvm).
Разделы ниже — история очереди реализации.

## Как воспроизвести

Из `conformance`:

```sh
./ocs run --adapter adapters/rust/adapter.json --profile full-release \
  --adapter-env OCS_RUST_ENGINE=ir -j 8
./ocs run --adapter adapters/rust/adapter.json --profile full-release \
  --adapter-env OCS_RUST_ENGINE=sparrow -j 8
./ocs run --adapter adapters/rust/adapter.json --profile full-release \
  --adapter-env OCS_RUST_ENGINE=jvm -j 8
```

По умолчанию выбран JVM — как у `okkama run` (с 24.10.2026, при слиянии с
`feature/rs`); IR и Sparrow задаются `OCS_RUST_ENGINE`. До 22 октября 2026 адаптер исполнял только JVM;
старые результаты Rust-адаптера нельзя считать результатами IR или Sparrow.
Выбор engine записывается в отчёт и учитывается в кэше capabilities.
Режим `test` объявляют все три движка. Sparrow-прогон проходит через
генерацию, encode/decode SPB и ту же VM, которую использует бинарник `sparrow`.
Самостоятельный CLI дополнительно проверяется интеграционными Rust-тестами.

## Исходная точка — 22 октября 2026

Профиль `full-release`, 772 обязательных случая:

| Исполнитель | PASS | UNSUPPORTED | BLOCKED | FAIL / CRASH / HARNESS_ERROR |
|---|---:|---:|---:|---:|
| IR | 404 | 365 | 3 | 0 |
| Sparrow | 400 | 369 | 3 | 0 |

PASS включает парсинг, анализ и ожидаемые отказы. Это не число успешно
исполненных положительных программ: с `outcome: completed` прошли только
19 случаев IR и 15 Sparrow; по 372 PASS относятся к ожидаемому отклонению
исходника. BLOCKED здесь — три TCP-теста: среда
запрещает bind на 127.0.0.1. Эти случаи не считаются пройденными.
`full-release` — профиль; его запуск не заменяет `ocs gate` и проверку
полноты самой спецификации OCS.

Первичные отчёты этого запуска: `/tmp/okkama-ocs-ir-baseline/` и
`/tmp/okkama-ocs-sparrow-baseline/`. Каталоги временные; команды выше создают
новые отчёты с запросами, ответами, хешами и потоками каждого случая.

## Очередь по зависимостям

- [x] Независимые OCS-прогоны IR/Sparrow/JVM без скрытого fallback.
- [x] Целочисленный `%` в общем runtime, IR и SPB/VM: знак делимого,
  произвольная разрядность, ошибка при нулевом делителе, opcode 17.
- [x] Строки в Sparrow: тип, константы, concat/equality/length/slice,
  trim/chomp/replace/replace_first, сигнатуры функций,
  формат SPB и проверка операндов. Общая семантика с IR.
- [x] Обычные объекты и массивы в Sparrow, таблицы типов/полей и общий heap;
  поддержка связанных модулей с объявлениями типов.
  Обычные объекты, defaults, методы, generics и successor реализованы в SPB v3;
  IR и Sparrow используют общие ObjectHeap/ArrayHeap. Массивы, size/get/slice,
  ленивый lookup и split/join реализованы в SPB v4. ObjInit и lifecycle впереди.
- [ ] Scalar conversions и byte/char: базовые преобразования, сравнения и
  побитовые byte-операции работают; остальные скаляры ещё в очереди.
- [x] Native ABI/registry и UTF-8/16/32: общий provider, проверка сигнатур,
  NativeCall в SPB, структурированные NativeFailure без catch/unwind.
- [x] Native stdout/stderr и stdin read/read-line. Реальные текстовые
  Stdout/Stderr работают в IR без обходов в адаптере; прямой runtime/stdio
  работает также в автономном Sparrow. Role-based stream-write остаётся ниже.
- [x] Roles, nominal narrowing, generic-роли и dispatch в обеих машинах.
- [ ] Capabilities и оставшиеся зависимости `L00.stdout.stream-write`.
- [ ] Остальные скаляры и операторы, коллекции/builders, closures.
- [ ] Regions, destroy, typed errors и unwind; ошибки native-вызовов.
- [ ] Async/futures/workers и scheduler, включая `L00.exit.async-main`.
- [ ] Test/fake, аргументы процесса и все режимы conformance.
- [ ] Остальные native-модули std и реальные сценарии examples.
- [ ] Сборка импортов в автономный SPB: проверять запуск после удаления
  исходников и std, без загрузки `.ok` во время исполнения.

В исходном IR-прогоне первые ограничения включали `utf8_encode` (65 случаев),
async main (42), unwind (28), RegionEnter (22), а также Cvt, BuilderNew,
RoleWrap и closures. Это первые обнаруженные ограничения, не полный список
препятствий внутри каждой программы. В Sparrow 360 случаев упирались в общий
запрет объявлений типов; снятие этого запрета требует реализации их семантики.

## Прогресс строк и объектов

Sparrow после добавления строк и обычных объектов: **403 PASS, 366 UNSUPPORTED,
3 BLOCKED**, новых FAIL/CRASH/HARNESS_ERROR нет. В PASS перешли
`L01.gram.test.skipped-in-production`, `L01.lex.identifiers.underscore-digits`,
`L01.lex.with.unrelated-names-allowed`. Отчёт: `/tmp/okkama-ocs-sparrow-objects/`.
Прямые сквозные тесты дополнительно проверяют Unicode-срезы/замену, строки в
вызовах и generic-полях, рекурсию методов, вложенные объекты и неизменность
старых алиасов после successor. Далее добавлены массивы в SPB/VM, включая
передачу через generic-поля и проверку payload только на found-ребре.
Далее реализованы byte/char и часть scalar conversions; следующий участок —
оставшиеся операции скаляров/builders, затем native UTF-8 и IO.

После массивов, slice и split/join результат Sparrow остаётся **403 PASS,
366 UNSUPPORTED, 3 BLOCKED** (`/tmp/okkama-ocs-sparrow-arrays/`). Большинство
положительных OCS-сценариев печатает через Stdout, поэтому снятие ограничений
внутри программы пока не меняет её итоговый статус. Полнота этих операций
проверяется сквозными тестами исходник → IR → encode/decode SPB → VM,
включая ошибки индексов и ленивое выполнение fallback.

## Скаляры и внутренние builders

IR и Sparrow исполняют byte/char-константы и сравнения, int/bool/byte → string,
byte → int и строго проверяемое int → byte. Проверяется контекст ошибочного
аргумента `@byte(...)`, включая отсутствие вычислений в невыбранной ветви.
Добавлена IR-операция UTF-8; native-модуль UTF-8 ещё требует отдельного этапа.

Внутренние builders поддерживают push, append, finish и точные целочисленные
диапазоны. Сквозные тесты покрывают `@byte(nums...)`, `@string(...)`, map/filter,
неизменность входных массивов, пустые/включённые диапазоны и границы вне i64.
Узлы builder неизменяемы; finish и освобождение арены не используют рекурсию.
Загрузчик SPB проверяет тип элемента и канонический флаг inclusive, запрещает
хранение builder в полях объектов и элементах массивов.

После добавления диапазонов вся Rust-регрессия: **367 тестов прошли**;
workspace Clippy с `-D warnings` проходит.
Автономный SPB с `@byte(nums...)` вернул 42 после удаления исходника и с пустым
PATH; IR для того же исходника также вернул 42.

На завершённом коде full-release: IR **404 PASS / 365 UNSUPPORTED / 3 BLOCKED**,
Sparrow **403 PASS / 366 UNSUPPORTED / 3 BLOCKED**, без FAIL/CRASH/HARNESS_ERROR.
Отчёты: `/tmp/okkama-ocs-ir-ranges/` и `/tmp/okkama-ocs-sparrow-ranges/`.
Промежуточные отчёты `*-scalars/` непригодны для сравнения: часть adapter
build попала на незавершённое изменение инструкции, дав 66 переходов из PASS
в UNSUPPORTED. В повторных прогонах ошибок сборки нет. Capabilities-кэш
теперь хранит отдельную запись для каждого engine, чтобы параллельные прогоны
не вытесняли друг друга; изменение исходников по-прежнему инвалидирует запись.

## Побитовые byte-операции

Добавлены `&`, `|`, `^`, `~`, `<<`, `>>`, `>>>` в обе машины и SPB.
Сдвиги проверяют точный счётчик 0..7 и сохраняют восемь бит; правые сдвиги
заполняют нулями. Общий helper проверен для всех 256 значений и 8 счётчиков
в обоих направлениях. Сквозные тесты проверяют неверные счётчики и ленивую
ветку, codec/verifier — roundtrip и несовпадающие типы операндов.

Полная Rust-регрессия после этого шага: **370 тестов прошли**, Clippy
`--workspace --all-targets -- -D warnings` проходит. Отдельный `.spb` с
побитовыми операциями вернул 42 после удаления исходника и с пустым PATH.
Полный OCS на этом шаге не повторялся; последний подтверждённый результат
остаётся в отчётах `*-ranges/`. Следующая основная зависимость — native
registry: в том IR-прогоне 158 сценариев останавливаются на `utf8_encode`.

## Native ABI и UTF

Новый независимый crate `okkama-runtime-abi` хранит стабильные provider ID,
канонические символы, сигнатуры и поля NativeFailure. Оба исполнителя
используют один UTF provider в runtime-core. Девять native-функций runtime/utf
покрывают UTF-8/16/32 encode/decode/valid_prefix с точным порядком байтов,
без замены повреждённых данных и без удаления BOM/noncharacters.

Sparrow opcode 56 NativeCall вызывает provider через проверяемый ABI.
Генератор создаёт native stub-функции с обычным call/return. Неизвестный ID,
неверная арность, тип массива или операнда отклоняются до выполнения.
NativeFailure сохраняет domain/code/operation/message; OCS классифицирует
необработанный отказ как runtime failure. Перехват catch ждёт unwind.

Полная Rust-регрессия: **376 тестов прошли**. Workspace Clippy с
`--all-targets -- -D warnings`, rustfmt и diff whitespace checks проходят.

Реальный импорт std/runtime/utf проверен через CLI и автономный SPB:
после удаления исходника, с пустым PATH и вне каталога проекта программа
вернула 42. В зависимостях отдельного sparrow нет компилятора.

Full-release на завершённом коде: IR **405 PASS / 364 UNSUPPORTED / 3 BLOCKED**,
Sparrow **403 PASS / 366 UNSUPPORTED / 3 BLOCKED**, без FAIL/CRASH/HARNESS_ERROR.
Отчёты `/tmp/okkama-ocs-ir-native-utf/`, `/tmp/okkama-ocs-sparrow-native-utf/`.
В PASS перешёл `L02.shift.reject-byte-count-8` (операции предыдущего шага).
После UTF 153 IR-сценария упираются в `runtime/stdio@1::stdout_write/1`;
следующие зависимости — native stdio, роли/dispatch, typed errors/unwind.

## Stdio и исправления семантики

Provider runtime/stdio добавлен в общий ABI (ID 10–13). Передаются сырые
байты, запись сообщает принятый префикс; ошибки — NativeFailure core.stdio.
Контекст одного запуска хранит буфер stdin и стабильный EOF для read/read-line.
Проверены бинарные байты 0/255, короткая запись, Interrupted, broken_pipe,
отрицательные/огромные лимиты и смешанное чтение. В CLI-тестах обычные
Stdout/Stderr исполняются через настоящий std. Автономный SPB читает stdin и
пишет точные stdout/stderr после удаления исходника, с пустым PATH.

Первый прогон после IO обнаружил два ранее скрытых FAIL. Исправления следуют
решениям владельца языка, без изменения ожиданий OCS:

- D-016: результаты +, -, *, унарного минуса оборачиваются в signed-64.
  Хранение литералов, сравнения и проверки границ остаются точными; внутренние
  exact helpers отделены от операторов wrapping. Старые внутренние тесты
  произвольной точности арифметических результатов исправлены по этому контракту.
- D-033: chomp удаляет один LF/CRLF и сохраняет одиночный CR; trim уже был верен.
- JVM приведён к тем же правилам и D-031 для %, пересобран встроенный Rt.class.
  Три соответствующих OCS-сценария JVM проходят.

Повторный full-release: IR **535 PASS / 234 UNSUPPORTED / 3 BLOCKED**,
Sparrow **403 PASS / 366 UNSUPPORTED / 3 BLOCKED**, без FAIL/CRASH/HARNESS_ERROR.
Отчёты `/tmp/okkama-ocs-ir-stdio-fixed/`, `/tmp/okkama-ocs-sparrow-stdio-fixed/`;
JVM targeted: `/tmp/okkama-ocs-jvm-arithmetic-fixed/` (3 PASS, остальные не запускались).
Это +130 PASS для IR относительно UTF-этапа. Следующие зависимости — роли,
typed errors/unwind, регионы, остальные скаляры и async.

Полная Rust-регрессия после исправлений: **383 теста прошли**, Clippy
`--workspace --all-targets -- -D warnings`, rustfmt и diff checks проходят.

## Семантика остатка

`L02.int.modulo-sign`, решение владельца языка D-031: `-7 % 3 == -1`,
`7 % -3 == 1`, `-7 % -3 == -1`. IR, Sparrow и JVM следуют этому правилу.
JVM `Rt.mod` исправлен вместе с D-016/D-033; пересобран встроенный `Rt.class`.
Арифметика покрыта сквозными Rust-тестами; native stdout позволяет также
проверять реальный вывод OCS-сценария через IR.

## Роли и SPB v5

RoleWrap, RoleNarrow и CallRole реализованы в обеих машинах. Таблицы ролей и
witnesses связываются с функциями при сборке; VM сохраняет идентичность объекта.
Verifier проверяет типы, арность, сигнатуры и уникальность witnesses. Тесты
покрывают generics, массивы/поля, рекурсию, void, successor, неверное сужение,
повреждённые таблицы и все усечённые префиксы файла.

Полная Rust-регрессия: **389 тестов прошли**. После оптимизации проверки
witnesses повторены шесть focused role-тестов и workspace Clippy — успешно.
Автономный v5 SPB вызвал метод роли и напечатал 42 через импортированные
UTF/stdio после удаления исходника, вне проекта и с пустым PATH.

Full-release: IR **551 PASS / 218 UNSUPPORTED / 3 BLOCKED**, Sparrow
**403 PASS / 366 UNSUPPORTED / 3 BLOCKED**, без FAIL/CRASH/HARNESS_ERROR.
Отчёты: `/tmp/okkama-ocs-ir-roles/`, `/tmp/okkama-ocs-sparrow-roles/`.
IR получил ещё 16 PASS. Sparrow по-прежнему останавливается на оставшихся
неподдержанных объявлениях в 357 случаях; поддержка ролей сама по себе
не снимает ограничения enum/handle/capability и typed errors.

Уточнение диагностик и повторный прогон
`/tmp/okkama-ocs-sparrow-role-diagnostics/` подтвердили: все эти 357 случаев
сначала останавливаются на enum. В частности, `io/stdout` импортирует
`encoding/utf`, где объявлен ByteOrder. Следующий этап — enum-значения,
сравнения и их таблица в SPB, затем оставшиеся инструкции отказов.
После изменения диагностик 46 unit-тестов компилятора прошли; результат OCS
не изменился, новых FAIL/CRASH/HARNESS_ERROR нет.

## Enum: общий Value и IR

IR исполняет EnumCase и EnumCompare. Значения сохраняют номинальный тип и
вариант при передаче через generic-поля, массивы и методы ролей. Общий helper
отклоняет сравнение разных enum и Undef. Match проверен для каждой ветви.
Sparrow пока отклоняет объявления enum: следующий шаг — таблица типов,
кодировка, verifier, codegen и VM для этих же значений.

80 тестов runtime/runtime-core прошли, включая три новых enum-теста; Clippy
этих crates с all-targets и -D warnings, fmt и diff checks проходят.
Целевой OCS IR-прогон `/tmp/okkama-ocs-ir-enums/`: **6 PASS**, остальные
766 случаев не запускались. Это пять `L04.enums.*` и
`L03.match.enum-and-strings`; проверяется реальный stdout через std.

## Enum в Sparrow — SPB v6

Таблица enum хранит число вариантов для каждого номинального типа.
EnumCase/EnumCompare проходят генератор, codec, verifier и отдельную VM;
общий Value сохраняет идентичность типа. Проверяются границы вариантов,
типы сравнения, только Eq/Ne, усечённые файлы и повреждённые counts.
Сквозные тесты IR расширены на encode/decode SPB и VM, включая enum в
generic-поле, массиве и результате вызова роли. Старые версии v1–v5 читаются.
Автономный SPB v6 вернул 42 после удаления исходника и с пустым PATH.

Full-release: IR **556 PASS / 213 UNSUPPORTED / 3 BLOCKED**, Sparrow
**405 PASS / 364 UNSUPPORTED / 3 BLOCKED**, без FAIL/CRASH/HARNESS_ERROR.
Отчёты: `/tmp/okkama-ocs-ir-enums-full/`,
`/tmp/okkama-ocs-sparrow-enums-full/`.
Следующие ограничения Sparrow: 153 случая на неподдержанном терминаторе,
40 на unwind, 33 на handle, 22 на async entry и 21 на типах сигнатур.
Следующая зависимость — типизированные отказы и их распространение.

Полная Rust-регрессия после enum: **394 теста прошли**; workspace Clippy
с all-targets и -D warnings, rustfmt и diff checks проходят.

## Основа распространения отказов: Panic

Обе машины различают Panic и программный Trap. Терминатор panic принимает
вычисляемую строку; Sparrow opcode 62 проверяет её тип и завершает поток.
OCS возвращает категорию panic. Проверены Unicode-сообщение через вызов,
ленивая ветвь и отказ verifier на нестроковом операнде. Автономный SPB после
удаления исходника завершился кодом 1 с ожидаемым сообщением Panic.

Два новых runtime-теста, OCS CLI-тест для обеих машин и 32 теста модели
Sparrow прошли. Workspace check/clippy all-targets, fmt и diff checks проходят.
Полный OCS на этом шаге не повторялся. Unwind/defer/catch остаются следующей
задачей; такие блоки всё ещё явно отклоняются, cleanup не пропускается.

## IR unwind для Panic и программных ошибок

Общий runtime хранит неизменяемую запись отказа с вторичными отказами cleanup.
IR направляет ошибку инструкции или терминатора в unwind текущего блока;
при отсутствии обработчика снимает кадры до unwind вызывающей функции.
Raise продолжает распространение, FailSecondary сохраняет исходный отказ.
Catch Error не перехватывает Panic или программную ошибку.

Проверены обратный порядок defer, отказ в cleanup, ошибка арифметики и
неперехватываемость обоих видов отказа. 84 теста runtime/runtime-core прошли;
workspace check/clippy all-targets, fmt и diff checks проходят.
Full-release IR: **563 PASS / 206 UNSUPPORTED / 3 BLOCKED**, без
FAIL/CRASH/HARNESS_ERROR (`/tmp/okkama-ocs-ir-unwind/`). Новые семь PASS:
confirm.bare-uncatchable, confirm.else-panic, control.not-caught,
defer.on-return-and-loops, defer.unvisited-and-nested, panic.not-catchable,
panic.terminates. Типизированные/native-отказы и Sparrow unwind впереди.

## Типизированные отказы в IR

Throw хранит исходный объект; Catch проверяет номинальный объект/роль и
соблюдает порядок обработчиков. Raise сохраняет отказ. FailCaused добавляет
причину для нового типизированного отказа внутри catch без циклических ссылок.
Текст неперехваченного Error вызывается через его реальный text() в той же
куче после unwind; пойманные объекты не запускают text() автоматически.
OCS возвращает имя типа, сообщение и категорию uncaught-error.

87 тестов runtime/runtime-core прошли, включая выбор catch, ролевую ссылку,
повторный throw, вызовы, причину и типизированный вторичный отказ cleanup.
Workspace check/clippy all-targets проходят. Первый полный OCS выявил только
неверную категорию диагностики; после её исправления семь focused failure-
тестов повторены успешно. NativeFailure и Sparrow unwind ещё в очереди;
отказ внутри Error.text пока отклоняется явно.

Повторный full-release IR: **582 PASS / 187 UNSUPPORTED / 3 BLOCKED**,
без FAIL/CRASH/HARNESS_ERROR (`/tmp/okkama-ocs-ir-typed-fixed/`). Это +19 PASS
относительно этапа untyped unwind. Промежуточный отчёт `*-typed/` содержит
исправленную ошибку категории и не является итоговым.

## NativeFailure в IR unwind

NativeFailure теперь проходит unwind как типизированный отказ. Catch принимает
его как конкретный NativeFailure или Error; поддержаны getters четырёх полей,
Error.text, ограниченная ролевая ссылка, throw/rethrow, причины и вторичные
отказы cleanup. Значение создаётся провайдером и неизменяемо. Для catch Error
не требуется искусственно добавлять объявление NativeFailure в исходный IR.

89 тестов runtime/runtime-core прошли; workspace check/clippy all-targets,
fmt и diff checks проходят. Full-release IR остаётся **582 PASS / 187
UNSUPPORTED / 3 BLOCKED**, без FAIL/CRASH/HARNESS_ERROR
(`/tmp/okkama-ocs-ir-native-unwind/`). Новые native-пути покрыты прямыми
сквозными тестами UTF-провайдера; ограничения других native API остаются.
Следующий этап — таблицы обработчиков, инструкции отказов, verifier и VM
Sparrow с теми же записями Failure и NativeFailure.

## Sparrow unwind — SPB v7

Добавлены таблица защищённых диапазонов, внутренний тип Failure и инструкции
Raise/FailSecondary/FailCaused. Verifier учитывает исключительные рёбра со
состоянием ДО инструкции: результат неуспешной операции нельзя читать в
обработчике. Проверяются диапазоны, границы функций, параллельные аргументы,
регистр payload, повреждённые counts и усечённые файлы. Обход использует
существующие лимиты работы и состояний; поиск обработчиков — по отсортированной
таблице без полного перебора для каждой инструкции/функции.

Codegen связывает диапазоны и цели с итоговыми адресами функций. VM поднимает
Panic/Trap/NativeFailure по кадрам, выполняет defer и сохраняет вторичные
отказы. Сквозные тесты сравнивают IR и загруженный SPB, включая вложенные
вызовы, арифметическую ошибку, несколько cleanup и native-отказ в cleanup.
Автономный SPB v7 проверен после удаления исходника и с пустым PATH.

Full-release Sparrow: **405 PASS / 364 UNSUPPORTED / 3 BLOCKED**, без
FAIL/CRASH/HARNESS_ERROR (`/tmp/okkama-ocs-sparrow-unwind/`). Типизированные
Throw/Catch и диагностические сведения о типах Error ещё требуют переноса.
Workspace check/clippy all-targets проходят; 17 focused тестов прошли.

Полная Rust-регрессия после SPB v7: **408 тестов прошли**. Rustfmt и diff
checks также проходят.

## Типизированные отказы в Sparrow — SPB v8

Файл содержит роль Error, имена типов и идентификатор системного NativeFailure.
Throw/Catch проверяются по nominal witness; catch payload доступен только на
совпавшем ребре. NativeFailure нельзя создать ObjNew/ObjNext. VM сохраняет
объект при throw/rethrow, выбирает обработчик по номинальному типу/роли,
сохраняет причины и вторичные отказы, вызывает настоящий Error.text только
для неперехваченного отказа. Runtime-owned NativeFailure поддерживает getters
и вызов text через Error, даже когда его конкретный тип не объявлен в IR.

Сквозные тесты IR расширены на encode/decode SPB и VM. Дополнительные тесты
проверяют metadata, запрещённый throw, создание NativeFailure, чтение payload
на неверном ребре, повреждённые строки/counts и все усечённые префиксы файла.
Автономный SPB v8 с настоящим Stdout, catch и defer после удаления исходника
и с пустым PATH напечатал точно `missing 42\ncleanup\n`.

Full-release Sparrow: **576 PASS / 193 UNSUPPORTED / 3 BLOCKED**, без
FAIL/CRASH/HARNESS_ERROR (`/tmp/okkama-ocs-sparrow-typed/`). Это +171 PASS.
IR остаётся на подтверждённых 582 PASS. Шесть оставшихся различий связаны
с RangeNew, ValCompare и типами сигнатур в целиком компилируемых функциях.
Workspace check/clippy all-targets, fmt и diff checks проходят.

Полная Rust-регрессия после SPB v8: **412 тестов прошли**. Первый прогон
обнаружил смещение диагностики возврата Undef из main на инструкцию 0;
проверка перенесена к самой инструкции Return. После исправления полный
workspace-прогон и Clippy повторены успешно.

## Структурное равенство

ValCompare и Sparrow opcode 68 ValueCompare используют общий алгоритм:
массивы сравниваются по длине/порядку, обычные объекты — по конкретному типу
и всем полям, включая private; ролевые ссылки сохраняют это правило.
Сравнение не вызывает пользовательские методы. Обход итеративный, с учётом
пар уже посещённых handles; проверена глубина 20 000 и повторяющиеся ветви.
NativeFailure сравнивается по своим неизменяемым полям. Внутренние значения
не сравниваются, смешение скалярных категорий — программная ошибка.

Сквозные тесты покрывают массивы, enum, generic/private поля, ролевые ссылки,
разные конкретные типы и неизменность прежнего значения после successor.
Codec/verifier проверяют операторы Eq/Ne, одинаковые типы, индексы таблиц,
запрет Failure/Builder и усечённые файлы.

Full-release: IR **590 PASS / 179 UNSUPPORTED / 3 BLOCKED**, Sparrow
**587 PASS / 182 UNSUPPORTED / 3 BLOCKED**, без FAIL/CRASH/HARNESS_ERROR.
Отчёты `/tmp/okkama-ocs-ir-equality/`, `/tmp/okkama-ocs-sparrow-equality/`.
Это +8 PASS для IR и +11 для Sparrow. Три оставшихся различия: RangeNew
и два случая с неподдерживаемыми типами сигнатур. Workspace check/clippy
all-targets, fmt и diff checks проходят.

Регрессия затронутых runtime/runtime-core/model/VM crates: **138 тестов
прошли**. Автономный SPB с generic-объектом и вложенными массивами вернул
42 после удаления исходника и с пустым PATH. Полный workspace-прогон на
этом шаге не повторялся; последний полный результат — 412 тестов на SPB v8.

### Диапазоны как значения и точные счётчики циклов

IR и Sparrow используют общий неизменяемый Range: начало, конец и признак
включения конца. Значение переносится через generic-поля и методы, сравнивается
структурно и сериализуется инструкциями RangeNew/RangePart (69/70).
Убывающие границы дают программную ошибку.

Внутренний шаг counted_loop теперь получает точное следующее целое через
исключительную границу одноэлементного диапазона. Это устраняет зацикливание
на границе signed-64, сохраняя переполнение пользовательского int.add.
Обновлены два ожидания печатного IR для for/map. Сквозные тесты проверяют
хранение, generic-вызовы, равенство с учётом inclusive и проход через MAX_I64;
codec проверяет усечение, типы операндов и недопустимые флаги.

Full-release: IR **590 PASS / 179 UNSUPPORTED / 3 BLOCKED**, Sparrow
**588 PASS / 181 UNSUPPORTED / 3 BLOCKED**, без FAIL/CRASH/HARNESS_ERROR.
Отчёты: `/tmp/okkama-ocs-ir-ranges/`, `/tmp/okkama-ocs-sparrow-ranges/`.
Автономный Sparrow выполнил цикл через MAX_I64 из SPB после удаления исходника
и с пустым PATH; код выхода 42. Workspace check и Clippy all-targets проходят.

Полная регрессия workspace: **422 теста прошли**, включая JVM, IR, codec,
verifier и VM; `/tmp/okkama-ranges-workspace-final.log`. Форматирование и
`git diff --check` чистые. Следующий этап — битовые операции int и методы
работы с битами, уже представленные в IR/JVM.

### Битовые операторы int

Общее runtime-core теперь выполняет `&`, `|`, `^`, `~`, `<<`, `>>`, `>>>`
над signed-64; IR и VM используют эти же операции. Sparrow кодирует их
инструкциями 71–77, verifier требует int во всех регистрах. Число позиций
проверяется как точное целое до преобразования операнда: только `0..63`.
Логический сдвиг на ноль сохраняет битовый рисунок и знаковое представление.

Сквозные тесты проверяют знаковый бит, арифметический и логический сдвиг,
большие операнды, отрицательные/слишком большие счётчики и сериализацию.
Codec проверяет каждый новый opcode, усечённые файлы и неверный тип регистра.
Тест ошибки в callee теперь использует недопустимый сдвиг вместо ранее
неподдержанного `&`, сохраняя проверку независимости последующих запусков.

Full-release: IR **600 PASS / 169 UNSUPPORTED / 3 BLOCKED**, Sparrow
**598 PASS / 171 UNSUPPORTED / 3 BLOCKED**, без FAIL/CRASH/HARNESS_ERROR.
Это +10 PASS на каждом движке. Отчёты: `/tmp/okkama-ocs-ir-intbits/`,
`/tmp/okkama-ocs-sparrow-intbits/`. Методы bit_count/rotate и остальные
битовые методы — следующий этап.

Регрессия runtime/runtime-core/model/VM: **144 теста прошли** с учётом
повторного запуска двух обновлённых тестовых наборов execute и sparrow.
Логи: `/tmp/okkama-intbits-regression-fixed.log`,
`/tmp/okkama-intbits-execute.log`, `/tmp/okkama-intbits-sparrow.log`.
Workspace Clippy all-targets, fmt и diff checks проходят. Полный workspace
не повторялся: последний полный результат — 422 теста на этапе диапазонов.

## Слияние с `feature/rs` — 24 октября 2026

Ветка слита в `feature/rs`. Решения при слиянии:

- **R01 вместо D-016.** `int` точный: общий `okkama-runtime-core` даёт
  `add/subtract/multiply/negate` без переполнения signed-64 для IR и Sparrow;
  JVM-кодогенератор остаётся на `BigInteger.add/subtract/multiply/negate`
  (helpers `Rt.intAdd/...` не используются). Битовые операции и сдвиги — над
  словом signed-64, больший операнд — `Bitwise operand is outside signed-64
  range` во всех трёх движках. Тесты, закреплявшие переполнение, переписаны.
- **`okkama run` по умолчанию — JVM**, пока IR и Sparrow не покрывают язык и
  std; `--engine ir|sparrow` или `OKKAMA_ENGINE` выбирает Rust-процесс.
  Адаптер OCS по умолчанию тоже JVM.

Full-release после слияния: JVM **772/772 PASS**; IR **601 PASS / 171
UNSUPPORTED**; Sparrow **599 PASS / 173 UNSUPPORTED**; FAIL/CRASH нет
(`L02.int.overflow-wraps` перешёл в PASS у IR и Sparrow). Std на JVM —
230/230 файлов тестов. Rust-тесты — 467/467.
