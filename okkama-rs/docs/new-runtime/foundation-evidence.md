# Сверка основания: фазы 0–1

Статический аудит 2026-11-03. Порядок: [execution-order.md](execution-order.md).
Наличие реализации/теста ниже не означает его повторного успешного запуска.
Исторические результаты сохраняются отдельно от текущих проверок.

## Фаза 0

| Требование | Найденные артефакты | Чего недостаёт | Следующая проверка |
|---|---|---|---|
| Эталон, происхождение и воспроизводимая сборка | `baseline/README.md`, `baseline/build.json`, `../../../benchmarks/db/results/sparrow-origin.json` | Повторная сверка хешей; воспроизведение сборки сейчас не выполнялось | Чтение metadata и последовательный SHA-256; сборка отдельно с лимитами |
| Отделение ядра | `../../tools/check-sparrow-boundary.py`, `../../crates/sparrow-engine/Cargo.toml` | Свежий результат guard; guard проверяет зависимости, не всю семантику fallback | Offline cargo tree через guard, затем аудит loader/JIT ошибок |
| Полный перечень инструкций/типов/intrinsics/native/CLI/контрактов с тестами | `inventory.json`, `adapter-coverage.json`, `../../tools/new-runtime/coverage_check.py` | Инструкции/native имеют placeholder mappings, типы перечислены без тестов; CLI не покрыт отдельной матрицей. Совпадение ABI не доказывает реализацию | Сопоставлять каждый ID с кодом, конкретным тестом и результатом; CLI добавить явно |
| Полный классифицированный манифест программ | `inventory.json`, `baseline/source-checks.json` | `fixture_hints` — эвристика; успешная компиляция не классифицирует сценарий запуска. Не всем runtime entries назначены ожидания и среда | Объединить по пути с сохранением исходных хешей, отрицательных ожиданий, сценариев завершения и NOT_RUN |
| Полный эталон поведения, включая сервисы | `baseline/conformance-summary.json`, `baseline/stdlib.json`, `baseline/postgres.json`, `baseline/cookbook.json` | Исторические 835 OCS/239 stdlib и две PG-проверки не покрывают все программы. Cookbook неполон; MySQL/MariaDB остаётся обязательством реализации | Сначала список непокрытых сценариев; сервисные/worker запуски только в изоляции |

`inventory.py` ранее без проверки выставлял всем инструкциям/native/intrinsics
`not_implemented`. Это не результаты аудита реализации. Для будущих снимков нужен
`unknown`; исторический `inventory.json` не переписывать, поскольку на его хеш
ссылается отчёт adapter coverage. Числа в старом снимке не фиксируют размер будущей
приёмки: перед полной приёмкой перечень необходимо актуализировать.

## Фаза 1

| Требование | Реализация / существующая проверка | Чего недостаёт | Следующая проверка |
|---|---|---|---|
| Отдельное ядро и сборочная цель | `crates/sparrow-engine` (пути в этой таблице относительно `okkama-rs`), `Cargo.toml`, boundary guard | Повторная сверка production dependency graph | Offline guard без запуска VM |
| Контекст, ABI, владение, ошибки | `docs/new-runtime/abi.md`; `src/jit.rs`, `execution.rs`, `value.rs`, `native_io.rs`, `failure.rs` | ABI-документ содержит устаревшие общие утверждения перед поздними checkpoints; не все ABI состояния сведены с реализацией | Сверить статусы, RunState layout/lifetime, native publication, roots, cancellation, host teardown; исправлять документ только по коду |
| Loader/verifier, entry/return | `tests/bootstrap.rs::decoded_spb_has_matching_cold_and_native_results`, `malformed_and_unsupported_programs_do_not_fall_back`; `src/validate.rs` | Свежий ограниченный прогон и полнота отказов verifier | Читать выбранные тесты, затем запускать по точному имени; весь bootstrap не запускать автоматически |
| Нет старого runtime/fallback | Boundary guard, malformed/unsupported test, `src/lib.rs`, `src/jit.rs` | Dependency guard не проверяет скрытый запуск бинарника и все error branches | Статический аудит launch sites и ошибок генерации; небольшой отрицательный тест |
| Cold/JIT, платформенные ограничения и отказ компиляции | `tests/bootstrap.rs`, `tests/jit_regions.rs`, `src/lib.rs`, `src/jit.rs` | Отдельная сверка platform cfg, диагностики и compiler failure path; результаты на одной платформе не доказывают остальные | Сначала статическая матрица платформ/режимов; недоступные платформы оставить NOT_RUN |

## Следующие небольшие задачи

1. Выполнено: placeholder генератора изменён на `unknown`; исторические отчёты
   сохранены. `ast.parse` подтвердил синтаксис; сам генератор не запускался.
2. Выполнено: `python3 tools/check-sparrow-boundary.py` — exit 0,
   `sparrow-engine`, `sparrow-cli`, `okkama-runtime`: independent production dependency graph OK.
3. Разобрать структуру inventory/source-checks и подготовить матрицу оставшихся
   классификаций без запуска перечисленных программ.
4. Свести ABI-документ с кодом; это подготовка зависимости фазы 1, пока полный
   baseline фазы 0 ожидает изолированную среду.

Фазы 0 и 1 остаются открытыми. Эта матрица не заменяет поэлементное покрытие
122 инструкций, типов и контрактов; числа относятся к историческому inventory.

## Результат статического объединения

Команда: `python3 tools/new-runtime/foundation_audit.py > docs/new-runtime/foundation-audit.json`
из `okkama-rs`. [Отчёт](foundation-audit.json) содержит хеши входных отчётов,
результаты по каждому пути и ограничения интерпретации. Программы не запускались.

- Все 1922 исходника исторического inventory совпадают с текущими по SHA-256.
  Новые файлы вне снимка этим не проверяются.
- 1022 записи имеют совпадающее по хешу свидетельство компиляции; в source-checks
  их runtime имеет статус NOT_RUN. Это не отменяет отдельных stdlib/cookbook отчётов.
- 900 исходников связаны с conformance-кейсами и не входят в source-checks;
  их UNKNOWN требует сверки с отдельными OCS-результатами, не означает провал.
- 436 из 729 контрактов не имеют явной case-связи в inventory. Большая часть
  относится к stdlib; отсутствие связи не доказывает отсутствие тестов в модулях.
  Приоритет — языковые контракты без связей, затем stdlib по зависимостям.
- Повторяющиеся пути в обоих входах отклоняются; baseline-путей вне inventory нет.

### Уточнение статусов контрактов

Три языковых записи без case-связей (`CTRL.MATCH-EXHAUSTIVENESS`,
`LEX.DECIMAL-LITERALS`, `VAL.DECIMAL-SPELLING`) имеют официальный статус `deferred`
в `conformance/contracts`; решения также записаны в discrepancy ledger.
Связывать их с похожими choice/decimal тестами и объявлять обязательными нельзя.
Генератор inventory ранее терял status/aspects/discrepancies; теперь сохраняет
их вместе с путём и хешем определения. Исторический snapshot не перезаписывался.

Аудит читает текущие определения и сохраняет их хеши: 372 mandatory без case-связи,
293 со связями, требующими проверки аспектов, 64 deferred/informative.
`python3 -m unittest discover -s tools/new-runtime -p test_foundation_audit.py`:
4 PASS, без запуска VM, сети или subprocess. Проверены различие статусов,
сохранение аспектов и отклонение дубликатов.

### Исторические runtime-свидетельства

Сопоставление сохранённых результатов без запуска программ: 835 OCS-кейсов
совпадают с актуальными файлами кейсов (включая case.json, ожидания и support);
239 stdlib-записей совпадают по хешу модуля и имеют сохранённый exit code 0.
Аудитор явно называет их `historical_pass`, не текущим PASS нового ядра.
Хеш модуля не доказывает неизменность всех импортов и внешней среды.
Успех отрицательного OCS-кейса означает ожидаемый отказ, а не успешный запуск main.
Старые исходные отчёты сохранены. Пять unit-тестов аудитора прошли, включая
недопустимость переноса PASS при несовпадающем источнике и трактовки NOT_RUN как PASS.

### Сверка ABI и минимального каркаса

`abi.md` сверен с execution/jit/future/native_io/async_host/value: добавлен
пропущенный статус REGION_TRANSFER=11, уточнены уже реализованные async-корни,
cleanup, native failures, external waits и границы blocking admission.
Статически подтверждено: ошибка `Compiled::new` передаётся через `?` наружу,
а Cold выбирается режимом, не ошибкой JIT. Fault-injection отказа компилятора
и проверки других платформ этим не заменяются.

Три точечных запуска 2026-11-03 прошли (по одному тесту, без дочерних workers):

```sh
CARGO_BUILD_JOBS=1 cargo test --offline -p sparrow-engine --test bootstrap decoded_spb_has_matching_cold_and_native_results -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --offline -p sparrow-engine --test bootstrap malformed_and_unsupported_programs_do_not_fall_back -- --exact --test-threads=1
CARGO_BUILD_JOBS=1 cargo test --offline -p sparrow-engine --test jit_regions region_edges_preserve_parallel_moves_big_tags_and_exact_instruction_budgets -- --exact --test-threads=1
```

Время самих тестов: 0.03s, 0.00s, 0.07s. Весь bootstrap намеренно не запускался:
он также содержит subprocess-проверку. Это текущие результаты нового ядра,
в отличие от исторических OCS/stdlib-свидетельств выше.

### Отказ ленивой JIT-компиляции

Добавлена thread-local fault injection только под `cfg(test)`, со сбросом через
Drop. Тест `jit_compile_failure_is_terminal_without_fallback_and_releases_dead_values`
проверяет отказ первого и второго региона, точный PC, отсутствие скомпилированного
региона/fallback, сохранение ошибки при повторном resume, освобождение вычисленного
bigint после GC и успешную новую сессию с тем же Code. Ошибка ядра не превращается
в catchable language failure. Производственная сборка hook не содержит.

Первый вариант теста ошибочно ожидал, что активные регистры terminal-сессии остаются
живыми. По `Task::live_registers` и `Task::resume` это неверно: повторный запуск
невозможен, значит значения не должны удерживаться. Исправлено ожидание теста,
поведение ядра не менялось. Точечный запуск с `--lib`, offline, одним build job
и `--test-threads=1`: 1 PASS, 0.09s. Это проверка обработки ошибки, не fault injection
внутри allocator/Cranelift finalization и не доказательство всех OS-отказов памяти.
