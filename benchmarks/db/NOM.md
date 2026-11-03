# NOM: реализация объектов и вызовов, без изменения языка

Ветка `nom`. Сохраняются наблюдаемая семантика объектов, идентичность, destroy/cleanup, исключения, async и отмена. Предыдущие измерения и оптимизация рабочих буферов: [SQLite layers](SQLITE-LAYERS.md).

## Шаг 1: неизменяемые метаданные intrinsic-вызовов

В Sparrow `call_now` (путь из JIT) и `call` (интерпретатор байткода) при каждом intrinsic-вызове создавали Vec идентификаторов объектов и вспомогательных функций. Содержимое зависит только от функции в неизменяемой программе. Пустые Vec и раньше не требовали выделения буфера; устраняется повторное построение непустых массивов.

Добавлен ленивый `OnceCell<IntrinsicBindings>` на функцию внутри Engine. Массивы подготавливаются при первом intrinsic-вызове и далее передаются срезами. Метаданные не разделяются между Engine, не содержат пользовательских Value и не образуют Rc-цикл. Цена — таблица ячеек на Engine и удержание подготовленных массивов до конца запуска. Входные аргументы, Host, Future и регистровые кадры этим изменением не оптимизируются.

Это первый небольшой шаг в механизме вызова, а не готовый аналог vectorcall. Сокращение массивов аргументов потребует отдельно учитывать сохранение значений async-функциями, замыканиями и intrinsic-реализациями.

### Скорость

Baseline уже содержит переиспользование буферов collect. Пять последовательных раундов с ротацией порядка before-worker, after-worker, before-inline, after-inline, Python. Четыре прогрева фаз; 5000 строк, 500 lookup/insert. Сборки и тесты не выполнялись параллельно timing-серии. Контрольные суммы проверены. Медианы, мс:

| Фаза | Worker до | Worker после | Inline до | Inline после |
|---|---:|---:|---:|---:|
| projection_only | 4.758 | 4.707 | 4.699 | 4.648 |
| scan_raw | 19.780 | 19.011 | 18.383 | 17.951 |
| scan_stream | 27.938 | 26.940 | 26.154 | 25.754 |
| scan_all | 28.603 | 27.417 | 26.528 | 26.154 |
| lookup_prepared | 36.554 | 34.910 | 24.089 | 23.919 |
| lookup_statement | 36.861 | 36.488 | 25.254 | 25.088 |
| insert_transaction | 10.777 | 10.568 | 5.250 | 5.233 |

Prepared worker: −4.5% времени в этой серии, inline: −0.7%. Малые изменения не доказывают статистически устойчивого ускорения. Python prepared: 0.767 мс; целевой разрыв 1.2–1.5× ещё не достигнут. [Все раунды и хеши бинарников](results/intrinsic-bindings-timing.json).

### Аллокации

Два повтора 500/1000/1500 prepared lookup с одинаковым стартом и четырьмя прогревами по 500. На всех четырёх разностях удалены ровно 74 alloc-запроса и 592 запрошенных байта на дополнительный lookup. Общий счётчик alloc уменьшился с 925.978–926.002 до 851.978–852.002 на запрос (~8%). Realloc сохранился: 50.270–50.272. Изменение целиком относится к rest: региональные категории не изменились. Это запросы Rust к MiMalloc, не RSS и не физический размер блоков аллокатора. Контрольные суммы всех шести процессов проверены, сумма категорий равна общему счётчику.

[Сырые счётчики](results/intrinsic-bindings-allocations.json). Диагностический executable `/tmp/okkama-bindings-counts`, timing executable без счётчиков `/tmp/okkama-bindings-inline`. Baseline: `/tmp/okkama-scratch-inline`. Для повторения счётчиков собрать CLI с `okkama-cli/allocation-stats,okkama-runtime-core/sqlite-inline-bench,okkama-runtime-core/allocation-scopes` и запустить `count-allocations.py --require-scopes`; для timing — только с sqlite-inline-bench, через `compare-scratch.py --before ... --after ... --out ...`.

### Проверки корректности

`cargo test -p sparrow-vm`: 2 unit + 4 integration теста прошли. Новый unit-тест проверяет повторное использование, разные функции и разные Engine. По 92 выбранных conformance-сценария на Sparrow с обычной настройкой JIT и с `OKKAMA_JIT=off` — 184 PASS. Выборка: L07, L08, L09.db, L11.sqlite, L12.future. Непроверенные случаи полного профиля остаются NOT_RUN, поэтому runner возвращает 1. Отдельный запуск с `OKKAMA_JIT=0` не считается проверкой без JIT: реализация распознаёт именно `off`.

Обычный executable сохраняется отдельно от диагностических сборок; установка в систему, коммит и слияние в этом шаге не выполнялись.

[Сводки проверок и хеш исходника](results/intrinsic-bindings-validation.json).

## Шаг 2: кэш регистровых кадров — эксперимент не принят

В интерпретаторе Sparrow обычные вызовы уже имеют быстрый путь с заполнением регистров без промежуточного Vec и локальным переиспользованием кадров. Проверен другой участок: `invoke` при входе в JIT создаёт новый регистровый Vec. Эксперимент добавлял кэш Engine на 16 пустых буферов с capacity не более 4096 Value каждый. Живой/приостановленный кадр оставался эксклюзивно занят, возвращался только после завершения; ссылки очищались до заимствования кэша. Массивы аргументов и семантика языка не менялись.

По двум сериям счётчиков 500/1000/1500 lookup кэш устранил 19 alloc requests и 6840 запрашиваемых байтов на дополнительный запрос: около 852 → 833 выделений, 59573 → 52733 байта в первой разности. Realloc остался около 50.27. Это сокращение запросов Rust к MiMalloc, не RSS и не объём копирования.

Пять парных раундов по прежней методике (4 прогрева, 5000 строк, 500 lookup/insert); все результаты проверены. Медианы, мс:

| Фаза | Worker до | Worker с кэшем | Inline до | Inline с кэшем |
|---|---:|---:|---:|---:|
| projection_only | 4.741 | 4.690 | 4.658 | 4.676 |
| scan_raw | 18.990 | 18.964 | 17.992 | 18.211 |
| scan_stream | 26.990 | 26.782 | 25.889 | 25.805 |
| scan_all | 27.367 | 27.153 | 26.110 | 26.281 |
| lookup_prepared | 35.176 | 34.825 | 23.870 | 24.180 |
| lookup_statement | 36.559 | 36.325 | 25.216 | 25.151 |
| insert_transaction | 10.411 | 10.352 | 5.279 | 5.274 |

Worker prepared −1.0%, inline +1.3% времени. Убедительного ускорения этим набором не показано; сравнительно небольшое сокращение аллокаций не оправдало дополнительный кэш и удержание пустых буферов для текущей SQL-задачи. Вывод ограничен измеренным workload: полезность для других программ не исключена.

Проверки кандидата: 4 unit + 4 integration теста VM; 92 conformance с обычной настройкой JIT и 92 с `OKKAMA_JIT=eager` — 184 PASS. Новые тесты проверяли отсутствие переиспользования живого кадра, освобождение Rc при unwind, Undef в повторно взятых регистрах, повторное использование малого буфера и ограничения кэша.

**Решение:** исходник machine.rs восстановлен точно до предыдущего проверенного SHA-256; кэш intrinsic-метаданных шага 1 сохранён. Обычный release executable восстановлен. Кандидат и его тесты сохранены отдельным [патчем эксперимента](register-pool-experiment.patch), без применения в текущем исходнике. Следующая область — временные аргументы вызовов; этот эксперимент их не устранял.

[Тайминги](results/register-pool-timing.json), [счётчики](results/register-pool-allocations.json), [проверки и хеши](results/register-pool-validation.json).

## Шаг 3: короткие intrinsic-аргументы на стеке

Добавлен `intrinsics::call_borrowed` со срезом аргументов; прежний `call(Vec<Value>)` сохранён как обёртка для существующих клиентов. Аргументы, которыми пользуется intrinsic, по-прежнему клонируются функцией arg; возвращаемые холодные Future удерживают собственные значения.

В JIT call_generic прямой Call intrinsic с не более чем 8 аргументами теперь собирает владеющий массив Value на стеке вместо Vec. Копии освобождаются до записи результата в регистр, как в прежнем пути. Длинные списки, вызовы через closure/role/cap и остальные пути сохраняют прежнюю обработку. Нового unsafe-кода не добавлено. Кэш кадров предыдущего эксперимента отсутствует.

Парное сравнение с состоянием после шага 1: пять раундов, четыре прогрева, 5000 строк и 500 lookup/insert. Последовательные процессы без одновременных сборок/тестов, контрольные суммы проверены. Медианы, мс:

| Фаза | Worker до | Worker после | Inline до | Inline после |
|---|---:|---:|---:|---:|
| projection_only | 4.703 | 4.721 | 4.714 | 4.651 |
| scan_raw | 19.242 | 18.161 | 18.220 | 17.258 |
| scan_stream | 27.393 | 25.948 | 26.143 | 25.004 |
| scan_all | 28.056 | 27.235 | 26.316 | 25.066 |
| lookup_prepared | 35.100 | 34.298 | 24.785 | 23.923 |
| lookup_statement | 36.921 | 36.552 | 26.132 | 25.255 |
| insert_transaction | 10.527 | 10.595 | 5.328 | 5.147 |

Worker streaming −5.3%, prepared −2.3%; inline streaming −4.4%, prepared −3.5%. Projection worker +0.4%, insert worker +0.6% времени: сохранены все фазы, без отбора улучшений. Малые различия не доказывают устойчивого ускорения. Это локальное упрощение реализации вызовов, не устранение разрыва с Python.

Проверки: 56 runtime-core + 2 VM unit + 4 VM integration = 62 теста; 93 conformance на Sparrow с `OKKAMA_JIT=eager` и 93 на IR — 186 PASS; validator: 834 cases, validation OK. Новый L08.intrinsic.retained-arguments создаёт холодную отправку и Atomic со строковыми значениями, выполняет другие вызовы, затем проверяет сохранность исходных значений и отложенную ошибку после закрытия канала. Финальная timing-сборка повторно собрана после уточнения порядка освобождения аргументов.

[Тайминги](results/intrinsic-stackargs-timing.json), [новый conformance-сценарий](../../conformance/cases/L08-async-and-concurrency/intrinsic.retained-arguments/main.ok).

Две серии счётчиков 500/1000/1500 lookup: на всех четырёх разностях устранены 100 alloc requests и 9600 запрашиваемых байтов на дополнительный запрос. Всего ~852 → ~752 выделения (~11.7%); realloc остаётся ~50.27. Изменение целиком вне collect/Region, их разности не изменились. Это запросы Rust к MiMalloc, не физические байты блоков, RSS или объём копирования. Обычный executable восстановлен; timing executable `/tmp/okkama-stackargs-inline`, инструментированный `/tmp/okkama-stackargs-counts`. [Счётчики](results/intrinsic-stackargs-allocations.json), [проверки и хеши исходников](results/intrinsic-stackargs-validation.json).

## Что именно дорого: SPB, графы и destroy

SPB — регистровый байткод, но его регистры содержат managed Value, включая ссылки на объекты, массивы, замыкания и Future. Граф данных живёт в runtime. Его нельзя отождествлять с AST или графом управления компилятора. Инструкции region.adopt/transfer/leave реализуют операции над этим графом; JIT может вызвать runtime-помощник, а не заменить весь алгоритм одной машинной инструкцией.

73 измеренных collect на lookup — не 73 вызова destroy: 59 относятся к register, 1 к transfer, 13 к корням begin_close. Последнее не означает 13 разных закрытых регионов. Цена самих пользовательских тел destroy отдельно не измерялась. Поэтому вывод относится к динамическому обнаружению и сопровождению владения, а не к неизбежной цене детерминированного уничтожения.

Java backend также реализует Region.collect/register/transfer с ArrayList и IdentityHashMap. GC JVM не заменяет автоматически правила уничтожения Okkama. Сравнение JVM и Sparrow поэтому проверяет разные реализации всего пути, а не изолированную скорость одного GC или объектного заголовка.

## Контроль: JVM / Sparrow / Python

Одинаковый основной workload, текущая сборка после шага 3, SQLite worker включён. Первая серия: пять раундов, четыре прогрева каждой фазы. Вторая: три раунда, двадцать прогревов, чтобы проверить чувствительность результата JVM к более длинному прогреву. Порядок движков вращается, процессы последовательные. Запуск процесса/компиляция вне фазовых таймеров, однако JIT-компиляция и GC внутри исполнения могут влиять на фазы; двадцать прогревов не доказывают достижения steady state.

Java HotSpot 26+35-2893. Фактический SQL `SELECT sqlite_version()` дал JVM 3.51.0 и Sparrow 3.50.2; Python sqlite3 также 3.51.0. Java backend использует собственный FFM SQLite provider и планировщик с переиспользуемыми platform-thread carriers; это не rusqlite. Значит, таблица не изолирует вклад одного только исполнения объектов или одного только GC.

Медианы, мс; все контрольные суммы совпали:

| Фаза | Python, 4 прогрева | Sparrow, 4 | JVM, 4 | Python, 20 | Sparrow, 20 | JVM, 20 |
|---|---:|---:|---:|---:|---:|---:|
| projection_only | 0.960 | 4.677 | 1.861 | 0.953 | 4.751 | 1.453 |
| scan_raw | 1.981 | 17.945 | 75.275 | 1.988 | 17.672 | 77.380 |
| scan_stream | 2.950 | 25.593 | 75.133 | 2.932 | 29.259 | 75.333 |
| scan_all | 3.104 | 26.236 | 81.052 | 3.146 | 26.188 | 78.201 |
| lookup_prepared | 0.762 | 36.041 | 105.437 | 0.755 | 33.415 | 99.697 |
| lookup_statement | 0.857 | 36.711 | 115.350 | 0.844 | 36.136 | 112.440 |
| insert_transaction | 0.325 | 10.578 | 32.184 | 0.321 | 10.312 | 31.571 |

Вторая серия: JVM быстрее Sparrow примерно в 3.3× на projection_only, но медленнее примерно в 3× на prepared lookup. Повышение прогрева не устранило SQL-разрыв. Не следует выводить отсюда, что JVM в целом медленная, или приписывать разность конкретно FFM/планировщику без их профилирования. Наличие графовых операций Okkama подтверждено в Region.java, но их CPU-доля на JVM здесь не измерена.

[Серия 4 прогрева](results/nom-jvm-python.json), [контроль 20 прогревов](results/nom-jvm-python-warm20.json). Воспроизведение: `DB_BENCH_WARMUPS=4 OKKAMA_SQLITE_INLINE_BENCH=0 python3 benchmarks/db/run.py --compiler /tmp/okkama-stackargs-inline --engines python,sparrow,jvm --runs 5 --out /tmp/jvm-comparison.json`; для второго контроля warmups=20, runs=3.


## Шаг 4: диагностическое отключение регионов

Feature `okkama-runtime-core/no-regions-bench`, переключатель `OKKAMA_NO_REGIONS_BENCH=1`. Один release-бинарник, пять последовательных раундов с вращением порядка, четыре прогрева, 5000 строк / 500 запросов. Отключены register, transfer и begin_close: автоматические destroy не вызываются. Rc, сборщик циклов, явные defer/close сохранены. Объекты регионов и инструкции байткода ещё существуют. Это намеренное изменение семантики для измерения, НЕ реализация опционального destroy.

Медианы, мс:

| Фаза | Регионы, worker | Без регионов, worker | Регионы, inline | Без регионов, inline | Python |
|---|---:|---:|---:|---:|---:|
| projection_only | 4.677 | 4.757 | 4.653 | 4.667 | 0.955 |
| scan_raw | 17.874 | 17.276 | 16.894 | 16.511 | 1.963 |
| scan_stream | 25.609 | 25.029 | 24.404 | 23.740 | 2.901 |
| scan_all | 26.030 | 25.681 | 24.959 | 24.885 | 3.175 |
| lookup_prepared | 34.308 | 24.228 | 23.600 | 15.500 | 0.764 |
| lookup_statement | 36.197 | 23.965 | 25.031 | 15.223 | 0.900 |
| insert_transaction | 10.483 | 9.851 | 5.183 | 4.605 | 0.330 |

Prepared worker: −29.4%; inline: −34.3%. Однако даже inline без регионов остаётся примерно в 20.3 раза медленнее Python. Потоковое чтение изменилось гораздо меньше. Это эффект всего механизма: исчезают также удерживающие Rc-ссылки и меняются условия обновления объектов на месте; нельзя приписывать разность только обходу графа или телам destroy.

Контрольная программа проверяет destroy вложенного Child внутри Wrapper без собственного destroy и сохранение явного defer: четыре проверки (IR/Sparrow, режим включён/выключен) успешны. Проверены оба режима SQLite worker/inline. Все контрольные суммы во всех 25 запусках совпали. Основной SQL workload явно закрывает connection, prepared statements и rows, завершает транзакции. Это не проверка корректности отключённого режима при всех ошибках/отменах.

56 runtime-core unit tests прошли с диагностическим feature и выключенным переключателем. Обычная конфигурация Sparrow: 93 выбранных conformance PASS, 741 NOT_RUN. Полный conformance с отключёнными регионами не заявляется: старые контракты destroy намеренно нарушены. Обычный release executable восстановлен, SHA-256 совпадает с сохранённым до эксперимента.

[Сырые замеры, контрольные проверки и хеши](results/no-regions-timing.json). Воспроизведение: собрать CLI с features `okkama-runtime-core/sqlite-inline-bench,okkama-runtime-core/no-regions-bench`, затем `python3 benchmarks/db/compare-no-regions.py --compiler /path/to/experimental/okkama --out /tmp/no-regions.json`.

### Граница самостоятельного Sparrow runtime

В репозитории уже есть `sparrow-cli` (SPB/SPA, assembler/disassembler) и `sparrow-vm`; VM не зависит от компилятора, но разделяет с IR `okkama-runtime-core`: Value, heap/Region, intrinsics и scheduler. Следующий архитектурный кандидат — отдельный crate для runtime Sparrow внутри текущего workspace, с собственным представлением значений и управлением памятью. Отдельный git-репозиторий пока не нужен: ABI/байткод и генератор придётся согласованно менять. Нативные библиотеки можно разделять через границу без общего VM Value; существующий native Context пока связан с текущими значениями и не является готовой независимой границей. Это предложение, перенос ещё не выполнен.


## Step 5: independent Sparrow runtime (2026-11-03)

Added `sparrow-runtime`: an independent copy of 53 runtime source files, without re-exports or dependency on the IR runtime. Implementation files match the original core at the split; only the lib.rs introductory comment differs. Rc, cycle GC, regions and destroy semantics are preserved. No performance improvement is claimed or measured in this structural step.

The VM now owns its Value/IntValue, failures, native Context, scheduler and worker state. Removed `okkama_runtime::sparrow`; CLI and OCS call the VM directly and format results at the external boundary. Source traces translate only function/pc coordinates. Worker commands and ICU environment defaults are configured for the selected engine. Cross-engine tests compare exact results and diagnostics instead of Rust type identity.

Production dependencies are separate in both directions, checked by `python3 okkama-rs/tools/check-sparrow-boundary.py`. IR cross-engine tests retain a VM dev-dependency. The combined `okkama` binary includes both runtimes; standalone `sparrow` includes only its own. Native ABI, independent libraries and build-time JVM data tables remain shared. The tables still reside under the compiler source directory. Implementation duplication means shared semantic fixes must be ported explicitly.

Validation:

- 279 unit/integration tests passed across runtimes, VM and CLIs.
- 93 CLI/JVM tests passed again after the per-engine ICU environment adjustment.
- Sparrow full conformance: 831 PASS, 3 TCP cases blocked by sandbox loopback restrictions. Separate authorized loopback run: 3 PASS. All 834 mandatory cases passed across the two runs, no failures.
- IR: 93 selected conformance cases passed; 741 not run.
- Release sparrow-cli built; standalone SPB, okkama exec, both okkama run engines and SPB/ASM/SPB smoke checks passed. Bytecode roundtrip identical.
- Dependency guard and git diff --check passed. No installation or commit.

[Architecture and commands](../../okkama-rs/crates/sparrow-runtime/README.md), [validation and source hashes](results/runtime-split-validation.json). Changing Sparrow memory management is the next separate step; this crate split does not implement it.


## Step 6: standalone baseline before a larger runtime redesign

Measured commit `1c289f8b` using standalone Sparrow and a precompiled, identical SPB file. No compiler, LSP or IR interpreter executes in the benchmark process. Five rotated rounds, four warmups, 5000 rows / 500 queries. All checksums and both diagnostic mode probes passed. Release binary restored afterwards (SHA-256 matches the saved normal build). No production runtime change was made in this step; the proposed per-intrinsic VmHost allocation optimization was deferred in favour of evaluating a larger redesign.

| Phase (ms) | Regions, worker | No regions, worker | Regions, inline | No regions, inline | Python |
|---|---:|---:|---:|---:|---:|
| projection_only | 4.644 | 4.804 | 4.685 | 4.755 | 0.951 |
| scan_raw | 18.010 | 17.469 | 16.844 | 16.212 | 1.960 |
| scan_stream | 25.703 | 25.114 | 24.643 | 24.184 | 2.908 |
| scan_all | 26.147 | 25.405 | 25.623 | 24.951 | 3.083 |
| lookup_prepared | 34.100 | 24.224 | 23.448 | 15.543 | 0.762 |
| lookup_statement | 35.975 | 23.761 | 24.752 | 15.161 | 0.857 |
| insert_transaction | 10.400 | 9.774 | 5.151 | 4.637 | 0.322 |

Region removal reduces prepared worker time by about 29%, but inline/no-regions remains about 20.4x Python. These diagnostic modes change destruction and scheduling semantics; they are not a production speedup. Small phase differences are not statistical proof of regression or improvement. Native library version differences remain as documented above.

Measured Rust layouts on this host: Value 24 bytes, IntValue 16, Object 64, Region 96, FutureCell 184. These are size_of results, excluding Rc allocation headers, field buffers, retained values, closures and coroutine stacks. Sizes alone do not establish CPU cost. JIT h_obj_new builds Vec<Value> fields and invokes ObjectHeap.allocate; machine Frame stores Vec<Value>. The execution representation remains shared between VM and JIT even though Sparrow is now independent of IR.

### Proposed next POC boundary (not implemented)

Replace the value/heap/call-frame subsystem as one vertical slice, instead of tuning each allocation separately. Candidate representation: compact tagged slots with immediate scalar values and heap references for objects, strings, large integers and other composites. Preserve exact integer semantics. Allocate objects and their fields together; avoid a separate Vec allocation for each object. Use reusable contiguous frame storage, with explicit roots at allocation/suspension/native-call boundaries. Start with a non-moving tracing design to keep the first root/FFI contract tractable; generational or moving collection requires separate evidence and barriers.

Automatic resource cleanup must be specified separately from memory reclamation. Existing defer/close paths remain explicit. Merely skipping objects without their own destroy is incorrect when they contain a resource-bearing child. The existing no-regions flag remains a diagnostic ablation, not the optional-destroy implementation.

Before integrating SQL, the POC must exercise the real object allocation/field access/call path, aliases and successors, cyclic and deep structures, and collection while values are live. Every JIT spill, suspended coroutine, Future capture and native-held value needs an explicit rooting/tracing contract. Keeping everything alive until process exit is not an acceptable memory-management result. Then test sustained SQL workloads and peak memory as well as latency. The SQL drivers and protocol libraries are not scheduled for replacement in this first slice.

[Standalone samples and hashes](results/standalone-sparrow-regions.json), [runner](compare-sparrow-regions.py). Build diagnostic VM with `cargo build --offline --release --manifest-path okkama-rs/Cargo.toml -p sparrow-cli -p sparrow-runtime --features sparrow-runtime/no-regions-bench,sparrow-runtime/sqlite-inline-bench`; compile main.ok and both probes to SPB using the ordinary compiler, then pass them to the runner's --bytecode, --region-probe and --inline-probe arguments. No acceleration is claimed from this investigation.


## Шаг 7: scalar replacement для локальных объектов

Реализован первый узкий путь в JIT Sparrow: конструктор с прямым заполнением int-полей и простые getters могут исполняться без объекта. Поля захватываются как SSA-значения в момент создания; перезапись исходных регистров не меняет снимок. В ARM64 dump тестового Work.compute значения загружаются в x21/x19. Для больших int выполняется исходный путь; переполнение арифметики остаётся точным. Ни язык, ни IR-runtime не менялись.

Консервативные ограничения: линейные функции до 256 инструкций, до восьми полей, без async/handlers/destroy. Возврат или передача объекта неизвестному коду, алиасы, перезапись его регистра и участие в регионах исключают кандидат. Разрешены пустые переходы к следующей инструкции; в выбранной функции нет обратных переходов, вход JIT только с начала. Объектный header/fields не выделяются, но универсальные Value-слоты, вызовный ABI и fallback пока остаются. Это не обещание «все значения в CPU-регистрах».

Финальные измерения: 200000 вычислений Point(a,b).first()+second(), 3000 прогревочных вызовов, девять чередующихся раундов, standalone SPB, таймер внутри программы:

| Сравнение | До / off, мс | После / on, мс | Ускорение |
|---|---:|---:|---:|
| Сохранённый sparrow-origin против финального release | 19.976 | 9.460 | 2.11× |
| Один финальный бинарник, OKKAMA_JIT_SCALAR off/on | 20.109 | 9.478 | 2.12× |

Диагностический счётчик MiMalloc, две серии по 5000/10000/15000 вызовов после одинакового прогрева: на каждой разности 2 → 0 alloc/call, устранены 128 запрошенных байт и два освобождения на вызов. Это наклон счётчиков, исключающий фиксированную загрузку и JIT; весь процесс не является безаллокаторным. Время инструментированной debug-сборки не использовано для оценки скорости.

SQL/DB измерялся отдельно, worker включён, 5000 строк / 500 запросов, четыре прогрева и пять чередующихся раундов. Prepared lookup: origin 34.158 → new 33.840 мс; same-binary off 34.186 → on 34.582 мс. Остальные фазы в same-binary контроле изменились от +0.4% до +4.2%, тогда как origin-серия дала разнонаправленные небольшие изменения. Устойчивое ускорение SQL НЕ установлено; улучшение Point нельзя переносить на весь runtime. Цель 1.5× Python (~1.14 мс для этих 500 lookup при Python ~0.762 мс) пока не достигнута.

Проверки: 8 VM unit/integration PASS; 194 выбранных conformance с eager JIT PASS (641 NOT_RUN); новый кейс на IR PASS; после ограничения размера анализа новый SPB-кейс повторно прошёл JIT off/eager и VM tests. Проверяются точные большие целые, переполнение, снимок полей при перезаписи источника, исключение escape/destroy/control-flow и слишком больших функций. Release собран, dependency guard и diff check прошли.

Исходный release сохранён в корне как `sparrow-origin` (Mach-O arm64), хеш 0961567b86755d430b58512d38e9c51ca2f19198b397d184e835d7b8b8886260. Это сохранённый бинарник до POC, а не извлечённый из Git artifact. [Происхождение](results/sparrow-origin.json). Бинарник и изменения не закоммичены.

[Origin comparison](results/scalar-origin-timing.json), [same-binary comparison](results/scalar-timing-final.json), [allocation slopes](results/scalar-allocations-final.json), [CLIF и ARM64](results/scalar-work-code.txt), [проверки и хеши](results/scalar-validation.json).

### Следующий участок: реальная SQL-цепочка

После POC снят новый профиль standalone Sparrow на 300000 prepared lookup после четырёх прогревов. Чистый повтор без параллельных бенчмарков, восемь секунд sample с интервалом 1 мс. Контрольная сумма 753623580 подтверждена. Время процесса под профилировщиком не является benchmark timing.

В 5799 наблюдениях основного потока: Region inclusive 19.57%; collect inclusive 10.36%; allocator inclusive 7.92%; Value clone self 3.17%; Value drop self 7.38%; ожидание inclusive 13.88%; JIT call helpers inclusive 12.90%. Категории inclusive пересекаются, складывать их нельзя. Scheduler inclusive охватывает почти всю программу и НЕ означает соответствующую стоимость планировщика. SQLite работает на другом потоке; отсутствие sqlite3 на основном потоке не доказывает нулевую стоимость SQL. Неизвестные JIT-адреса и инлайнинг ограничивают атрибуцию.

Найден конкретный кандидат на удаление лишней цепочки: PreparedQueryOpening создаёт DbValueSnapshot, который уже формирует native DbCellState и возвращает DbNativeValue. SqliteBindings затем снова создаёт ObservedDbValue для каждого такого значения. Его print восстанавливает типизированные значения (для integer — BigInt из native строки), а DbValueCapture снова формирует native ячейку. Нужен путь повторного использования готовых native-ячеек с сохранением проверки лимитов и поведения пользовательских DbValue. В этом шаге этот путь ещё НЕ изменён, его эффект не измерен. Существующие проверки cardinality, cancellation, закрытия курсора и ошибок должны сохраниться.

[Профиль](results/scalar-sql-sample.txt.gz), [агрегация](results/scalar-sql-sample-summary.json), [метаданные](results/scalar-sql-profile-run.json). Воспроизводимые инструменты: profile-sparrow.py и summarize-runtime-sample.py.
