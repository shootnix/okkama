# Перестройка ядра Sparrow

Статус: план, согласован пользователем 2026-11-06. Основание —
[разведка ядра](core-spike.md): прототип `crates/sparrow-proto` под клеткой
быстрее текущего `sparrow-engine` в 23–40 раз и sparrow-origin в 9–15 раз,
на массовых спаунах — на порядки.

## Целевая модель

1. **Синхронные функции** компилируются целиком (Cranelift) и вызывают друг
   друга прямыми машинными вызовами на нативном стеке. Роли — таблица
   `[роль, слот][дескриптор]` и косвенный вызов; замыкания — косвенный вызов.
2. **Async-функции** — возобновляемые машины состояний. Кадр — объект кучи и
   одновременно Future. `await` готового — результат; холодного — запуск тела
   тут же; иначе — сохранить регистры, записаться ожидающим, уступить.
   `spawn` стартует немедленно. Планировщик и всё внешнее ожидание — на Tokio.
3. **Значения без тегов там, где тип статический.** Тег остаётся у `int`
   (малое / BigInt), `any`, `rat` и незаполненных полей.
4. **Объекты — указатели на `[заголовок, поля…]`**, поле читается по смещению
   в машинном коде, аллокация — инлайн.
5. **Точный GC.** Корни: stack maps Cranelift (проход по frame records), кадры
   async в куче, корни Rust-стороны через посетителей. GC запускается только
   в точках безопасности машинного кода и диспетчера, никогда внутри native.
6. **Вытеснения внутри синхронного кода нет** (уступка — только на `await`).
   На обратных переходах циклов — проверка флага прерывания: отмена задачи,
   таймаут теста и завершение процесса останавливают зависший цикл
   типизированной ошибкой, а не вытеснением.

## Что сохраняется

- Загрузчик, верификатор SPB, построение плана (`Op`), анализ живых значений,
  проверка ролей и сигнатур — вход нового JIT.
- Natives (~17 тыс. строк, включая SQLite) — через сохранённый API `Heap`
  (`text`, `elements`, `allocate_*`, `object_get`, handles). Меняется реализация
  API, не провайдеры.
- Async-хост на Tokio, `native_io` Request, семантика отмены, cleanup,
  `destroy`, ошибок и трасс.
- Тесты: все семантические проверки (программа → результат, вывод, ошибка).

## Что переписывается

- `execution.rs` (плоский диспетчер, continuations, saved), `jit.rs`
  (регионы, heap calls), `value.rs`/`arena.rs` (handles с поколениями, `Box`
  на объект), синтезированные cold-планы intrinsic (становятся обычными
  функциями ядра или прямыми вызовами Rust).
- Тесты внутренних свойств плоской модели (14 наборов используют
  `resume(1)`, `continuation_depth`, `saved_value_count`): переписать на свойства
  новой модели или удалить, если свойство исчезло.

## Решения (подтверждены пользователем 2026-11-06)

- **R1. GC без перемещения на первом этапе.** Mark-sweep с классами размеров,
  аллокация инлайн из свободных блоков. Natives держат значения в локальных
  переменных Rust между аллокациями, runtime-объекты (Atomic, Event, Lock,
  Channel…) сейчас ищутся в картах по адресу объекта — перемещение сломало бы
  и то и другое. Перемещающий nursery — отдельная оптимизация позже, когда
  идентичность вынесена в заголовок, а natives переведены на корни-области.
- **R2. Глубокая рекурсия — большой стек потока VM** (резерв 1 ГБ виртуальной
  памяти, физическая — по мере роста) вместо отката в интерпретатор. Исчерпание —
  типизированная ошибка переполнения стека, как сейчас ограничение памяти.
- **R3. Без cold-интерпретатора.** Один режим исполнения; функции
  компилируются лениво при первом вызове (заглушка вызова). `Mode::Cold`
  в тестах исчезает; дифференциальная проверка — против sparrow-origin
  и IR-интерпретатора на conformance.
- **R4. Новый crate `sparrow-core`**, а не правка `sparrow-engine` на месте:
  текущий движок работает до паритета, natives и тесты переносятся в новый
  crate по этапам, после паритета `sparrow-engine` удаляется. CLI `sparrow-next`
  переключается на новое ядро в конце этапа 4.

## Этапы

Каждый этап заканчивается зелёными тестами своего объёма и замером против
origin и текущего next на одинаковых программах. Async-, spawn- и стресс-проверки —
только под `tools/new-runtime/caged.py`; process workers — только Docker/VM.

### Этап 0. Контракты и разбор тестов — выполнен 2026-11-06

- Публичный API ядра: `Executable`, `Session`, `Outcome`, `run`, `run_tests`,
  `AsyncHost`, диагностика (`FailureReport`), `--stats`.
- Разметить 286 тестов `sparrow-engine`: семантические / внутренние / natives.
- Каркас `sparrow-core`, прогон `tools/check-sparrow-boundary.py`.

Готово, когда: таблица тестов с решением по каждому, crate собирается.

#### Результат этапа 0 (2026-11-06)

**Внешний контракт — CLI `sparrow-next`.** Библиотеку `sparrow-engine` не использует
ни один другой crate; скрипты `tools/new-runtime` вызывают бинарь. Новое ядро
сохраняет: `sparrow-next [--stats] [--test] [--worker] FILE.spb [ARGS…]`,
`--native-capabilities`, коды выхода (результат `main` 0..255, 1 — отказ, 2 —
ошибка аргументов), текст ошибок `sparrow-next: …`, протокол worker. Флаг
`--cold` по R3 становится синонимом обычного запуска (скрипты его передают).

**Библиотечный API для тестов:** `Executable::{decode, new, run, start,
with_arguments, with_worker_command, worker_response_with_command}`,
`Session::{complete_async, failure_report}`, `AsyncHost::{new, run, block_on}`,
`Outcome::{Returned, Waiting}`, `Integer`, `FailureReport` и узлы,
`run_tests_with_worker_command`, `native_capabilities`, `stats`. Уходят:
`Mode` (R3), пошаговый `Session::resume(fuel)` и `Outcome::Yielded` (синхронный
код не уступает), интроспекция плоской модели (`continuation_depth`,
`saved_value_count`, `live_registers`, `live_integer`, `elided_object_count`,
`recurrence_loop_count`), `stress_collection` заменяется параметром порога GC.

**Разметка тестов** (`tools/new-runtime/core_test_triage.py`, `--internal`
печатает список внутренних):

| Файл | semantic | heap | native | internal | worker |
|---|---:|---:|---:|---:|---:|
| `src/array.rs` |  | 1 |  |  |  |
| `src/async_host.rs` | 1 |  |  | 1 |  |
| `src/barrier.rs` | 2 |  |  |  |  |
| `src/channel.rs` | 3 |  |  |  |  |
| `src/collect.rs` | 1 |  |  |  |  |
| `src/diagnostic.rs` | 1 |  |  |  |  |
| `src/equality.rs` |  | 3 |  |  |  |
| `src/execution.rs` | 13 |  |  | 1 |  |
| `src/failure.rs` |  | 2 |  |  |  |
| `src/fake_storage.rs` | 1 |  |  |  |  |
| `src/future.rs` | 5 |  |  |  |  |
| `src/handle.rs` | 1 |  |  |  |  |
| `src/intrinsic.rs` | 6 |  |  |  |  |
| `src/liveness.rs` |  | 3 |  |  |  |
| `src/main.rs` | 3 |  |  |  |  |
| `src/map.rs` |  | 2 |  |  |  |
| `src/native.rs` |  |  | 5 |  |  |
| `src/native_binary.rs` |  |  | 1 |  |  |
| `src/native_buffer.rs` |  |  | 1 |  |  |
| `src/native_crypto.rs` |  |  | 1 |  |  |
| `src/native_data.rs` |  |  | 1 |  |  |
| `src/native_db.rs` |  |  | 3 |  |  |
| `src/native_dns.rs` |  |  | 5 |  |  |
| `src/native_encoding.rs` |  |  | 3 |  |  |
| `src/native_environment.rs` |  |  | 3 |  |  |
| `src/native_file.rs` |  |  | 3 |  |  |
| `src/native_file_state.rs` |  |  | 4 |  |  |
| `src/native_fs.rs` |  |  | 1 |  |  |
| `src/native_glob.rs` |  |  | 1 |  |  |
| `src/native_idna.rs` |  |  | 1 |  |  |
| `src/native_input.rs` |  |  | 2 |  |  |
| `src/native_io.rs` |  |  | 8 |  |  |
| `src/native_json/mod.rs` |  |  | 4 |  |  |
| `src/native_multicast.rs` |  |  | 1 |  |  |
| `src/native_net.rs` |  |  | 1 |  |  |
| `src/native_network.rs` |  |  | 4 |  |  |
| `src/native_numbers.rs` |  |  | 2 |  |  |
| `src/native_process.rs` |  |  | 2 |  |  |
| `src/native_regex.rs` |  |  | 3 |  |  |
| `src/native_root.rs` |  |  | 3 |  |  |
| `src/native_secret.rs` |  |  | 1 |  |  |
| `src/native_sqlite.rs` |  |  | 2 |  |  |
| `src/native_temporary.rs` |  |  | 1 |  |  |
| `src/native_text.rs` |  |  | 4 |  |  |
| `src/native_udp.rs` |  |  | 3 |  |  |
| `src/native_unix.rs` |  |  | 3 |  |  |
| `src/native_watch.rs` |  |  | 1 |  |  |
| `src/ordering.rs` |  | 1 |  |  |  |
| `src/process_host.rs` | 3 |  |  |  |  |
| `src/resource.rs` |  | 6 |  |  |  |
| `src/semaphore.rs` | 2 |  |  |  |  |
| `src/stream.rs` | 2 |  |  |  |  |
| `src/testing.rs` | 2 |  |  |  |  |
| `src/text.rs` |  | 1 |  |  |  |
| `src/validate.rs` |  | 1 |  |  |  |
| `src/value.rs` |  | 4 |  | 1 |  |
| `src/worker.rs` |  |  |  |  | 3 |
| `src/worker_process.rs` | 2 |  |  |  | 3 |
| `src/worker_wire.rs` | 3 |  |  |  |  |
| `tests/arithmetic.rs` | 3 |  |  | 1 |  |
| `tests/arrays.rs` | 7 |  |  |  |  |
| `tests/async_host.rs` | 4 |  |  |  |  |
| `tests/atomics.rs` | 1 |  |  |  |  |
| `tests/barriers.rs` | 1 |  |  |  |  |
| `tests/bootstrap.rs` | 6 |  |  | 1 |  |
| `tests/calls.rs` | 2 |  |  | 5 |  |
| `tests/channels.rs` | 3 |  |  |  |  |
| `tests/collect.rs` | 3 |  |  |  |  |
| `tests/completions.rs` | 1 |  |  |  |  |
| `tests/empty_literals.rs` | 1 |  |  |  |  |
| `tests/equality.rs` | 1 |  |  |  |  |
| `tests/events.rs` | 3 |  |  |  |  |
| `tests/failures.rs` | 8 |  |  | 3 |  |
| `tests/fake_storage.rs` | 1 |  |  |  |  |
| `tests/futures.rs` | 4 |  |  | 3 |  |
| `tests/groups.rs` | 3 |  |  |  |  |
| `tests/jit_regions.rs` | 1 |  |  | 1 |  |
| `tests/locks.rs` | 1 |  |  |  |  |
| `tests/maps.rs` | 4 |  |  |  |  |
| `tests/native_async.rs` | 2 |  |  | 1 |  |
| `tests/native_io.rs` | 4 |  |  |  |  |
| `tests/objects.rs` | 4 |  |  | 5 |  |
| `tests/ordering.rs` |  |  |  | 1 |  |
| `tests/processes.rs` | 2 |  |  |  |  |
| `tests/rational.rs` | 2 |  |  |  |  |
| `tests/recurrence.rs` |  |  |  | 4 |  |
| `tests/resources.rs` | 4 |  |  | 3 |  |
| `tests/scalars.rs` | 5 |  |  |  |  |
| `tests/scopes.rs` | 1 |  |  |  |  |
| `tests/semaphores.rs` | 1 |  |  |  |  |
| `tests/spawn.rs` | 4 |  |  |  |  |
| `tests/test_profile.rs` | 1 |  |  |  |  |
| `tests/text.rs` | 3 |  |  |  |  |
| `tests/timeouts.rs` | 1 |  |  |  |  |
| `tests/variants.rs` | 2 |  |  | 1 |  |
| `tests/workers.rs` |  |  |  |  | 1 |
| **всего 286** | **145** | **24** | **78** | **32** | **7** |

Внутренние (32) проверяют пошаговое исполнение, continuations, сохранённые
значения, перезапись плана (recurrence, object elision) и JIT-регионы. Для
каждого — свойство новой модели вместо старого: например, «глубокая рекурсия
не растит continuations» → «глубокая рекурсия до 1 ГБ стека, затем типизированная
ошибка»; «только живые значения пересекают приостановку» → «кадр async хранит
регистры по типам, GC видит их».

### Этап 1. Куча и значения

- Представление: заголовок объекта, строки (UTF-8 + длина + кэш индексации),
  массивы (типизированные для `int`/`float`/`byte`), map, BigInt/rational
  в куче, closures, Future-кадры, failure, region.
- Неперемещающий GC (R1); посетители корней Rust-стороны (`&mut Slot`);
  side-таблица native handles с финализацией (закрытие ресурсов при сборке).
- API `Heap` для natives — прежние сигнатуры.

Готово, когда: модульные тесты кучи (сборка, циклы, глубокие графы, BigInt,
строки, map, handles) зелёные; natives компилируются против нового API.

### Этап 2. Синхронное исполнение

- JIT целой функции по плану `Op` с типами регистров; ленивая компиляция;
  прямые вызовы, роли, замыкания, natives — прямым вызовом Rust.
- Ошибки: статус после вызова, таблицы обработчиков в коде, трассы (кадр
  записывается при раскрутке), `destroy` и cleanup-регионы.
- Stack maps, проход по кадрам, большой стек (R2), флаг прерывания.

Готово, когда: семантические синхронные тесты и бенчмарки 01–10 (синхронные
части) совпадают по выводу; заметно быстрее origin на 02, 07, 08, 10.

### Этап 3. Async

- Кадры async-функций, `await`, немедленный `spawn`, повторный await,
  несколько ожидающих, отмена (доставка в кадр, cleanup до завершения),
  группы, collect/race, каналы, события, блокировки, семафоры, таймеры,
  стримы — поверх Tokio-хоста и `native_io`.
- Ожидание внешнего I/O, диагностика простоя.

Готово, когда: async-тесты и выбранные L08-кейсы зелёные под клеткой;
`asyncs` и DB-бенчмарк не медленнее прототипа на порядок.

### Этап 4. Natives, std и CLI

- Перенос провайдеров (механически), SQLite, JSON, regex, сеть, файлы;
  `--test`, `--stats`, `--native-capabilities`.
- Conformance: SQLite-кейсы, L01–L13 по наборам (без workers).
- `sparrow-next` переключается на `sparrow-core`.

Готово, когда: все перенесённые тесты и conformance-наборы, проходившие
на `sparrow-engine`, проходят на новом ядре.

### Этап 5. Производительность и std/db

- Встраивание мелких функций, nursery (если R1 пересмотрен), профили.
- std/db: строки батчем (ответ fetch — Rust-данные, `Row` = батч + номер),
  колонка по номеру, лёгкий горячий путь `SqliteRows.row()`.
- Цель roadmap: SQL/DB не хуже 1.5× Python.

### Этап 6. Удаление старого ядра

- `sparrow-engine` удаляется, `sparrow-proto` — тоже; документация и
  roadmap обновляются.

## Риски

- **Объём переноса natives** — механический, но большой; API `Heap` держит его
  в рамках.
- **Семантика порядка в async** (немедленный старт, порядок пробуждений,
  отмена) — проверяется существующими async-тестами и L08.
- **Платформы:** frame records на aarch64 (x29) и x86_64 (rbp); Linux и macOS.
- **Отладка JIT** без интерпретатора: дамп CLIF по функции, `--stats`,
  дифференциальные прогоны против origin.
