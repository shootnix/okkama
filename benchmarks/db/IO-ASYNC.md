# SQL/DB: исследование I/O и async runtime

2026-11-02. Исследуется текущая ветка после переноса DB-ячеек в native runtime.
Это отдельный этап: нативное хранение значений уже присутствует в обеих версиях.

## Найдено и исправлено

1. `sqlite::request` и `maintenance` отправляли работу постоянному SQLite worker,
   затем вызывали `sched::offload` для блокирующего `recv` ответа. Offload создавал
   отдельный OS-поток на каждый такой вызов, хотя он лишь пересылал результат.
2. `SqliteTaskCleanup.close` вызывает `sqlite_settled` после ответа. Для открытого
   соединения settled ставил барьер в очередь worker и создавал ещё один поток,
   ожидающий этот барьер. Сам барьер сохранён; удалён поток-посредник.
3. `sched::register` проверял идентичность Future линейным проходом по всем ранее
   зарегистрированным Future. FutureGroup вызывает его для каждого дочернего
   Future: N уникальных регистраций требовали O(N²) сравнений. Добавлен HashSet
   идентичностей с ожидаемым O(1) поиском. Упорядоченный Vec сохранён для прежнего
   порядка shutdown и ошибок. Индекс очищается на всех прежних границах очистки;
   Vec удерживает Future, поэтому адреса в индексе не переиспользуются.

Новый `sched::external` регистрирует callback event loop и возвращает одноразовый
`Completion<R>`, который можно передать существующему worker. Callback исполняется
только в потоке программы; токен не содержит Rc объектов программы. `deliver`
передаёт результат, а Drop неиспользованного токена доставляет None (потеря worker),
чтобы await не зависал бесконечно. Отмена может отсоединить ожидание, но поздний
ответ всё равно обрабатывается для освобождения курсора и других ресурсов.

Ожидание полного выхода закрытого SQLite worker остаётся отдельной offload-
операцией. Общий offload для socket/TLS/fs в этом изменении не переписывался.

## Изолированная стоимость scheduler

Rust release example `async_delivery`, без SQL и без объектов stdlib: 500 операций,
один прогрев, пять образцов с ротацией порядка. Медиана микросекунд на операцию:

| Путь | мкс/операцию |
|---|---:|
| Уже завершённый Future | 0.047 |
| Cold Future с немедленным результатом | 0.147 |
| Новый offload-поток | 16.475 |
| Постоянный worker + новый поток-посредник | 17.718 |
| Постоянный worker → event loop напрямую | 3.459 |

Последние два пути различаются примерно в 5,1 раза. Это стоимость конкретного
механизма доставки, а не обещание ускорения любого SQL-запроса в 5 раз.
[Исходные измерения](results/async-delivery.txt).

## Методика полных DB-замеров

Пять пар before/after на IR и Sparrow, с чередованием порядка, без параллельных
сборок/тестов. Before — сохранённый release-бинарник перед изменением scheduler;
after — новая release-сборка. Stdlib в паре идентична. 5 000 строк, 500 запросов,
семь прежних фаз, все контрольные суммы проверены. Python измерен отдельно пять
раз после пар. Это SQLite в памяти; PostgreSQL и диск не измерялись.

Основной набор сохраняет прежний один внутренний прогрев. При 500 вызовах это
не гарантирует steady state Sparrow: его стандартный порог JIT равен 1 000.
Поэтому дополнительно сделан послойный тест с четырьмя прогревами каждой фазы.
Результаты разных наборов нельзя вычитать как точную стоимость отдельной функции.

## Что осталось исследовать

- **Сетевой I/O.** `network::start_read`/write вызывают offload для настоящего
  ожидания/чтения/записи сокета. Новая системная нить создаётся при запуске такого
  фонового задания; чтение уже буферизованных данных отдельной нити не требует.
  Shared.wait использует poll одного сокета. Следующий эксперимент — loopback
  read/write, число потоков и RSS при растущем числе соединений, затем общий
  механизм ожидания готовности. Замена на маленький общий blocking pool без
  анализа может блокировать прогресс при занятых длительными чтениями workers.
- **Удержание Future.** Реестр всё ещё хранит сильные ссылки на зарегистрированные
  Future до завершения запуска. Изменение HashSet убирает поиск O(N), но не меняет
  время жизни результатов. Нужен RSS-тест длинного потока/сервера и отдельный
  дизайн удаления завершённых задач с сохранением ненаблюдённых ошибок.
- **Обвязка драйвера.** FutureGroup, lifecycle, блокировки, карты reply metadata,
  преобразование параметров и построение объектов остаются над native-вызовами.
  Послойный тест ниже определяет масштаб этой разницы, но не заменяет CPU profile.
- **JVM требует отдельного исследования.** Там уже используются cached I/O pool
  и переиспользуемые carriers задач; перенос вывода «новый поток на каждый await»
  на JVM был бы неверен. Эти изменения касаются общего Rust runtime IR/Sparrow.

Timeout 50 мс в socket poll не означает обязательную задержку 50 мс на операцию:
готовый дескриптор возвращается сразу. Его уменьшение не следует выдавать за
исправление наблюдаемого отставания.

## Проверки

48 unit-тестов runtime-core прошли (RUST_MIN_STACK=16777216). 44 выбранных
async/SQLite conformance-кейса прошли на каждом из IR и Sparrow: 88 PASS.
Дополнительно кейс с реальной отменой открытия SQLite-курсора и повторным
использованием соединения прошёл на всех трёх движках. Проверены доставка
в поток программы, потерянный токен, поздний ответ после отмены, идентичность,
порядок и очистка регистрации Future. Release собирается.

См. [инструкции воспроизведения](README.md).

## Итоговые измерения после трёх исправлений

Медианы, миллисекунды. Before уже содержит нативное ядро DB; after дополнительно
содержит прямую доставку ответов/барьеров и индекс регистрации Future.

| Фаза | Python | IR before → after | Sparrow before → after | Sparrow / Python |
|---|---:|---:|---:|---:|
| projection_only | 0.976 | 47.048 → 46.553 | 9.047 → 9.047 | 9.3× |
| scan_raw | 2.033 | 75.865 → 59.563 | 63.842 → 46.693 | 23.0× |
| scan_stream | 2.956 | 138.717 → 119.124 | 63.459 → 47.364 | 16.0× |
| scan_all | 3.156 | 145.994 → 125.573 | 64.249 → 46.417 | 14.7× |
| lookup_prepared | 0.781 | 126.073 → 93.791 | 125.583 → 91.594 | 117.2× |
| lookup_statement | 0.860 | 123.311 → 93.591 | 101.294 → 67.188 | 78.2× |
| insert_transaction | 0.334 | 51.284 → 36.158 | 41.813 → 26.766 | 80.1× |

Поток/массив на Sparrow ускорились в 1,34–1,38 раза, отдельные Statement SELECT —
в 1,51 раза, вставки — в 1,56 раза. Projection-only почти не изменился. Это эффект
совокупности трёх исправлений. Между этапами сохранялись промежуточные результаты,
но они не являются рандомизированным факторным экспериментом по каждой правке.

## Послойный тест: одинаковый SELECT без параметров

500 раз `SELECT 7 AS n`, четыре прогрева каждой фазы, пять процессов на движок.
Медианы мс для всей серии:

| Уровень | IR | Sparrow |
|---|---:|---:|
| sync | 0.257 | 0.055 |
| async | 0.347 | 0.164 |
| native_one | 9.586 | 8.056 |
| driver_one | 49.908 | 33.308 |
| query_one | 61.745 | 44.474 |

`sync`/`async` — отдельный контроль без SQL: простой метод возвращает 1, результат
накапливается одинаковым способом. Он показывает небольшой overhead одиночного
await, но не измеряет все вложенные вызовы SQL API.

Native SELECT уже содержит реальную работу SQLite, worker round trips, чтение
reply metadata, проверку exactly-one и release курсора. Driver добавляет публичный
контракт, schema/Row, lifecycle и проверки; Query — универсальную обвязку. Разница
не равна стоимости одной конкретной функции. Дальнейший профиль стоит направить
на driver/session, а не приписывать весь остаток самому оператору await.

[Итоговые пары и хеши бинарников](results/io-async-paired.json),
[Python reference](results/io-async-python.json),
[послойные samples](results/io-async-layers.json), [workload](layers.ok).

История промежуточных измерений:
[только ответы/maintenance](results/async-delivery-paired.json),
[ответы + settled](results/async-final-paired.json).

## Обход полей объектов без промежуточного массива

Следующий эксперимент, 2026-11-02: в `heap::collect` заменён
`object.values().into_iter().rev()` на обратный проход по индексам через
`object.field(index)`. Поля сразу добавляются в стек обхода `pending`.
Устранён промежуточный Vec полей; клонирование самих Value, поиск посещённых
объектов и сортировка регионов сохранены. Поздно связанные поля по-прежнему
читаются через `field`, Undef пропускаются, порядок обхода сохранён.

Проверки: 48 unit-тестов runtime-core PASS; 90 выбранных conformance-кейсов
на каждом из IR и Sparrow (180 PASS): L07, L08, L11.sqlite,
L12.return-defer-destroy-order. Остальные 741 кейс каждого запуска не запускались;
это не полный conformance-прогон. Release успешно собран.

Сохранён бинарник до правки, stdlib одинакова. Выполнены пять чередующихся пар
before/after для каждого движка в каждом из двух наборов; сборки и тесты во время
измерений не выполнялись. Все контрольные суммы совпали. Послойный набор имеет
четыре прогрева; основной DB-набор сохраняет один прогрев и прежнее ограничение
по возможной JIT-компиляции внутри измеряемой фазы.

Послойные медианы, мс на 500 операций:

| Фаза | IR before → after | Sparrow before → after |
|---|---:|---:|
| native_one | 9.634 → 9.661 | 8.165 → 8.376 |
| driver_one | 45.816 → 45.545 | 33.674 → 32.113 |
| query_one | 61.384 → 62.157 | 46.206 → 44.605 |

Основной DB-набор, Sparrow, медианы мс:

| Фаза | Before | After | Изменение времени |
|---|---:|---:|---:|
| projection_only | 9.046 | 9.027 | −0.2% |
| scan_raw | 47.082 | 47.650 | +1.2% |
| scan_stream | 47.564 | 48.387 | +1.7% |
| scan_all | 48.798 | 48.640 | −0.3% |
| lookup_prepared | 93.691 | 92.171 | −1.6% |
| lookup_statement | 68.045 | 67.378 | −1.0% |
| insert_transaction | 26.649 | 26.632 | −0.1% |

Результат смешанный: в послойном тесте время driver/query на Sparrow снизилось
на 4.6%/3.5%, но native_one выросло на 2.6%; в основном наборе изменения малы
и разнонаправленны. На IR projection_only выросло на 2.5%, lookup_statement
снизилось на 3.5%. Эти данные не доказывают общее ускорение DB или отсутствие
небольших регрессий. Правка оставлена как устранение промежуточного массива;
существенного приближения к целевым 1.2–1.5× от Python этим не достигнуто.
Python в этом эксперименте заново не измерялся.

Исходные samples, контрольные суммы и хеши:
[послойные пары](results/collect-fields-layers.json),
[основной DB-набор](results/collect-fields-paired.json).

## SQL request throughput: adaptive batches and acknowledged replies

2026-11-02: three stdlib changes, with the same Rust compiler before/after:

- A stream starts with eight rows, then requests up to 64; the independent 8 MiB
  batch limit and one pending row remain enforced by the native driver.
- `SqliteTaskCleanup` skips the extra worker barrier only after an actual reply
  has been received. Cancellation/incomplete requests retain that barrier.
- Prepared bindings validate and capture parameters in one pass instead of
  constructing a dummy `Statement("SELECT 1", values)` and recapturing its values.

Five paired samples per engine plus five Python samples; 5000 rows, 500 lookups,
four warmups per phase, default JIT settings, sequential processes. This warmup
count differs from earlier tables and is not proof that every helper reached
steady state. All checksums match. Medians below are milliseconds:

| Phase | Sparrow before | Sparrow after | Speedup | Python | After / Python |
|---|---:|---:|---:|---:|---:|
| projection_only | 9.120 | 9.296 | 0.98× | 0.971 | 9.6× |
| scan_raw | 33.494 | 19.128 | 1.75× | 1.991 | 9.6× |
| scan_stream | 48.653 | 31.723 | 1.53× | 2.954 | 10.7× |
| scan_all | 48.421 | 32.690 | 1.48× | 3.122 | 10.5× |
| lookup_prepared | 55.427 | 46.089 | 1.20× | 0.761 | 60.5× |
| lookup_statement | 56.265 | 50.663 | 1.11× | 0.849 | 59.6× |
| insert_transaction | 16.701 | 12.084 | 1.38× | 0.324 | 37.3× |

IR also improves in all six DB phases; projection-only changes from 45.975 to
46.567 ms. Raw scan: 59.664 → 42.412 ms; prepared lookup: 90.188 → 78.049 ms;
transaction inserts: 35.859 → 26.130 ms. The 1.2–1.5× Python target is not met.

47 selected conformance cases passed on each of IR, Sparrow and JVM (141 PASS).
The new `L11.sqlite.throughput-lifetime` case adds 25 assertions covering batch
boundaries through 137 rows, large rows crossing the byte limit, early close,
fetch cancellation, late SQL failure, parameter snapshots, invalid printers,
and aggregate prepared parameter count/byte limits. Suite validation: 832 cases.

[Paired measurements and current Python reference](results/sql-throughput-paired.json).
[Reproduction runner](compare-throughput.py) accepts an explicit before-stdlib
snapshot; see the recorded roots and hashes. Preliminary
[driver ablations](results/driver-ablation.json) and
[parameter ablations](results/parameter-ablation.json) include deliberately
incomplete cleanup/validation paths and are diagnostic only, not shippable results.

### Next architecture: a thin Okkama facade over a Rust driver

The agreed direction is to use a maintained Rust database crate for database work,
with typed parameters and owned result handles at the language boundary. Candidate
driver APIs are [rusqlite](https://docs.rs/rusqlite/latest/rusqlite/struct.Connection.html)
for SQLite and [tokio-postgres](https://docs.rs/tokio-postgres/latest/tokio_postgres/struct.Client.html)
for PostgreSQL. This migration is not implemented by the throughput changes above.

The current SQLite engine already executes in Rust, but binds integer/float
parameters through strings and exports reply metadata as a string map that stdlib
parses again. Session/cursor state and substantial validation remain in Okkama.
Those boundaries must become typed native commands and results. User-supplied
DbValue printers still run in Okkama once; their captured native values should
then be passed through without reprinting. Row wrappers must own/share data whose
lifetime does not depend on an open cursor. Cancellation, transactions, bounded
streaming and errors remain part of the contract, not optional overhead.

The shared Rust provider serves IR and Sparrow. JVM currently has a separate Java
provider; using the same Rust implementation there would require a native bridge.
