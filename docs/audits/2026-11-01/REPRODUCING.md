# Как читать и воспроизвести проверки

Все пути команд ниже отсчитываются от корня репозитория. Проверенная версия: `61f388bcca150554051331097c5edef89e82309d`. Тестовые файлы следует создавать в отдельном временном каталоге. Исходники реализации и пользовательские benchmark results в ходе аудита не менялись.

## Обычные проверки

```sh
cd okkama-rs
cargo test --release --workspace --locked --offline
cargo test --workspace --locked --offline
cargo clippy --workspace --all-targets --locked --offline
```

Debug-прогон на проверенной версии аварийно завершается на освобождении DAG. Обычный clippy завершается на `approx_constant`. Для просмотра остальных lint в аудите отдельно выполнялся:

```sh
cargo clippy --workspace --all-targets --locked --offline -- -A clippy::approx_constant
```

Полная матрица проекта:

```sh
bash tools/gate/gate.sh
```

Дополнительный полный interpreter-only режим, из `conformance/`:

```sh
./ocs run --adapter adapters/rust/adapter.json --profile full-release \
  --stdlib-root ../std --adapter-env OCS_RUST_ENGINE=sparrow \
  --adapter-env OKKAMA_JIT=off -j 6
```

В песочнице локальная сеть может быть недоступна. Три сетевых кейса были повторены с разрешённым loopback отдельно для IR, JVM и Sparrow off/default/eager/threshold 2/10. Фильтр — **`--case L11.tcp` без завершающей точки**. В отфильтрованном запуске 790 остальных кейсов имеют NOT_RUN, поэтому общий exit code не показывает результат полного набора. Сохранены исходные stdout в `loopback-results-verified.json`.

## JIT: ошибочная живость на обработчике

```sh
okkama-rs/target/release/sparrow --assemble \
  docs/audits/2026-11-01/evidence/handler-liveness.spa \
  -o /tmp/okkama-audit-handler.spb
okkama-rs/target/release/sparrow check /tmp/okkama-audit-handler.spb
OKKAMA_JIT=off okkama-rs/target/release/sparrow /tmp/okkama-audit-handler.spb
OKKAMA_JIT=eager OKKAMA_JIT_STATS=1 \
  okkama-rs/target/release/sparrow /tmp/okkama-audit-handler.spb
```

На проверенном коммите off возвращает **5**, eager — **1** и внутреннюю ошибку. Код 5 здесь результат программы, а не отказ проверки. Проверять exit status нужно отдельно после каждого запуска, не объединять через `&&`.

## Rust runtime probes

`evidence/runtime-probe.rs` содержит два режима: `registry N` и `cert DIR N`. `evidence/drop-probe.rs` отдельно локализует падение освобождения. Эти небольшие standalone-программы используют уже собранные библиотеки workspace; они не добавляют crates или tests в проект.

Для release probe использовались параметры rustc:

```text
--edition=2024 -Copt-level=3 -Clto=fat
--extern okkama_runtime_core=<актуальный libokkama_runtime_core-*.rlib>
-L dependency=okkama-rs/target/release/deps
```

Путь rlib нужно брать из соответствующей текущему toolchain сборки, а не из произвольного старого hash. Для drop-probe — библиотека из `target/debug/deps`, без оптимизации probe. Его функция исполняется в потоке со стеком 2 МиБ. На текущей версии он abort; при повторении отключайте core dump для дочернего процесса (`RLIMIT_CORE=0`).

Проверка registry запускалась для N = 1000, 4000, 16000, 32000, 64000. Важны не только секунды, но `retained_after_drop=true` и `retained_after_reset=false`.

Проверка сертификатов использовала **только локальный native call**, без подключения к внешнему серверу. В `evidence/certs/` сохранены публичные DER тестовых сертификатов; приватные ключи не включены. Число промежуточных сертификатов: 0/4/6/8/9/10 и отдельно 11 с внешним тайм-аутом 8 секунд.

Сертификаты действовали 1–3 ноября 2026. `runtime-probe.rs` берёт время проверки из `SystemTime::now()`: после истечения срока для воспроизведения исторического теста нужно **в копии тестового harness** передать timestamp внутри интервала действия либо создать новый эквивалентный набор. Это изменение только тестовых данных; production clock/проверки не отключались. Ограничение времени обязательно: завершения на больших N ждать не нужно.

`host-probe.rs` — архив исходной программы с временным путём к `hello.spb`; перед запуском копии заменить этот путь на локальную копию fixture. Она проверяет сохранение виртуального environment через два execute и `size_of::<Value>()`. Это не проверка межпользовательской OS-изоляции.

## Масштабирование верификатора и импортов

Для верификатора генерировалась функция SPB assembly со следующей структурой:

```text
sparrow 9
module "audit" "v1.0.0"
type t0 int
function f0 "main" {
 registers [t0, ... N элементов ...]
 params []
 result t0
 0: const r0, int 0
 ... по одной записи для каждого регистра ...
 N: ret r(N-1)
}
entry f0
```

Использованы N = 1000/4000/16000/32000. После assemble выполнялся только `sparrow check`, три раза на размер. Python `time.perf_counter()` измерял wall time, `os.wait4(child_pid, 0)` давал peak RSS. На macOS `ru_maxrss` измеряется в **байтах**, на Linux — в КиБ, это нужно учитывать при переносе. Сохранены все повторы в `dataflow-scaling-verified.json`. Не следует увеличивать N без лимита памяти дочернего процесса.

Для проверки импортов создавались `m0.ok`…`m15999.ok`. Каждый файл:

```text
module mI v1.0.0;
import "mNEXT";
```

У последнего нет import; в `m0.ok` дополнительно `main() is int { 0; }`. Выполнялся release `okkama check m0.ok`, внешний timeout 25 секунд, `RLIMIT_CORE=0`. SIGABRT воспроизведён на 16 000; меньшие проверенные глубины перечислены в JSON. Никакой код Okkama при `check` не исполняется.

## Blade: контроль выхода за назначение

Архив результата — `blade-path-result.json`. Временная структура содержала три соседних каталога: `project`, `package`, `victim`. В `victim/proof.txt` была контрольная строка, а в пакете — одноимённый файл с другой строкой и manifest с некорректным именем. `.ok`-файлов в пакете не было. Выполнен `blade install <локальный package>` из `project`. Проверялось только содержимое контрольного файла внутри временного дерева. Не используйте для этого реальный домашний каталог или существующий проект.

После исправления тест должен отвергать некорректное имя до любых изменений вне staging. Исходный JSON содержит использованный manifest и результат операции.

## Fuzz smoke и арифметический differential

`bytecode_checks.py` сохранён как **архив фактически выполненного скрипта**, с исходными абсолютными путями машины и `/tmp/okkama-audit-20261101`. Перед переносом изменить ROOT/OUT в его копии и положить `hello.spb` в OUT. Не запускать его в каталоге с ценными одноимёнными файлами: скрипт пишет `mutant.spb`, `generated.spa` и результаты.

- PRNG seed: 11012026.
- 1500 мутаций: случайные bytes, truncation, изменения bytes, вставки, хвосты; timeout subprocess 3 секунды.
- 200 valid arithmetic programs: семь регистров, add/sub/mul, большие значения, нормализованный результат mod 251.
- Независимый oracle: целые Python; сравнивались off/eager.
- Это smoke-кампания без обратной связи по покрытию. Отсутствие найденного сбоя не означает отсутствие дефектов.

## Зависимости и provenance

`dependency-versions.json` — компактный снимок Cargo metadata. `advisories.rs` — архив сканера базы RustSec: пропускает withdrawn, сопоставляет patched/unaffected через Rust semver. Путь к БД и metadata в архиве фиксирован на временный каталог. Для постоянного CI нужен поддерживаемый `cargo audit`/эквивалент, с проверкой prerelease-правил и policy исключений.

Использована официальная RustSec БД revision `2295466c4e9df26647fd67d8fa1c705ac2fc7f81`. Три advisory дополнительно сверены с официальными страницами; ссылки есть в основном отчёте. Самостоятельный scanner не анализирует feature reachability и не доказывает эксплуатацию конкретного API.

## Бенчмарки

`existing-benchmark-report.md` — неизменённая копия уже существовавшего отчёта пользователя для того же SHA. `focused-benchmarks-serial.json` — новый последовательный прогон аудита: заранее собранный SPB, один прогрев и пять отдельных запусков на workload/mode. В таблице отчёта использован именно этот повтор, выполненный после завершения остальных сборок/тестов аудита. Первый диагностический повтор, пересекавшийся со сборкой probe, в итоговые числа не включён.

Для получения счётчиков helpers отдельно выполнялись:

```sh
OKKAMA_JIT_STATS=1 okkama-rs/target/release/okkama run --engine sparrow \
  benchmarks/10-app/main.ok
```

Аналогично для bigint, collections и closures. Статистика меняет накладные расходы; latency с включёнными счётчиками не использовалась как benchmark.

## Целостность материалов

`evidence/SHA256SUMS` содержит хеши файлов evidence, кроме самого манифеста. Он фиксирует состав отчёта, но не является цифровой подписью. Логи содержат исходные локальные пути и служебные идентификаторы процессов; сетевых секретов и приватных ключей в набор не добавлялось.

## Повторная проверка оптимизаций (раздел 11)

Новые материалы находятся в `evidence/optimization-recheck/`. `summary.json` содержит медиану/min/max каждого измерения; raw JSON сохраняет повторы. `provenance.json` фиксирует SHA исходной Rust-реализации и хеши использованных CLI/SPB. Во время проверки пользователь сохранил прежний отчёт коммитом `2f669ba7`; Rust-дерево между ним и `61f388bc` не отличается.

Скрипты сохранены с фактическими путями `/tmp/okkama-opt-recheck-20261101` и `/tmp/okkama-audit-20261101`. Для переноса поменять их **в копиях** и использовать пустой временный каталог. Это лабораторные сценарии, не установленные инструменты проекта.

1. Собрать `alloc_probe.rs` и `micro_probe.rs` через rustc с `--edition=2024 -Copt-level=3 -Clto=fat`, соответствующими release rlib из `okkama-rs/target/release/deps` и `-L dependency=...`. Первому нужны `--extern okkama_sparrow=...` и `--extern sparrow_vm=...`, второму — `--extern okkama_runtime_core=...`. Использовать тот же rustc, которым собран workspace.
2. Собрать исходные workloads `04-bigint`, `06-collections`, `09-closures`, `10-app` через `okkama build ... --target sparrow`. Сам runtime и benchmark sources не менять.
3. Для O11 собрать сохранённые `closures-original.spa` и пять `closures-extra-*.spa` в соответствующие SPB. Варианты отличаются только невызываемыми дополнительными функциями; `closures-extra-513-not-target` — отрицательный контроль. Не подменять исходный benchmark этими файлами.
4. `run_checks.py` выполняет O11, проверяет равенство stdout (включая off-controls), снимает JIT_STATS отдельно, затем базовые замеры, allocation counts, строки, byte RSS и import graph. Тесты идут последовательно. Для первой калибровки импортов был запуск, пересекавшийся со сборкой harness; его результаты заменены последовательным повтором в конце script. Итоговый `import-reparse.json` относится к этому повтору.
5. `source_differential.py` создаёт 60 корректных программ и сравнивает результаты с Python oracle в пяти режимах. Полный corpus восстанавливается по seed, один пример включён в evidence. Первый невалидный генератор был отклонён и исправлен до этой кампании.
6. Для sample использована `closures-profile.spa`: главный цикл увеличен в 20 раз только для достаточной длительности. Запускается отдельный дочерний Sparrow; `/usr/bin/sample <его PID> 2 1 -file <отчёт>` собирает две секунды, после чего завершается только этот дочерний процесс. Окончательного результата такой прерванной программы не требуется: это CPU sample, не проверка семантики и не замер полного runtime. Два запуска — default и `OKKAMA_JIT=off`. В песочнице первый sample не разрешён; повтор с разрешённым доступом к своему процессу успешен, оба статуса сохранены.

Allocator-counter делегирует **System**, тогда как CLI делегирует **mimalloc**. Число и сумма запросов характеризуют Rust allocation traffic на instrumented run; это не профиль внутренних malloc arenas production CLI. `requested_bytes` включает полные новые размеры realloc, а `peak_extra_live_bytes` измеряет изменение живых Rust allocations относительно состояния после decode. Нельзя трактовать сумму 3,6 ГиБ на closures off как одновременно занятую память или leak. Instrumentation-time не используется в performance-таблице.

Byte RSS относится к отдельной микропробе на System. Сравнение repeated slice/iterator показывает разницу алгоритмов обхода; iterator не является drop-in реализацией произвольного random access. Профиль sample содержит несимволизированные адреса JIT; доли вычислены по **top-of-stack**, без суммирования inclusive counts.
