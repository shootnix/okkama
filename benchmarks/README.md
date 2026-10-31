# Бенчмарки движков Okkama

Программы по возрастанию сложности: от арифметики в цикле до приложения из
шести модулей. Каждая печатает контрольный результат; раннер требует, чтобы
вывод всех движков совпадал.

| Каталог | Что нагружает |
|---|---|
| `00-hello` | только запуск движка |
| `01-arith` | целые в цикле (LCG, хвостовая рекурсия → цикл, 3 млн шагов) |
| `02-calls` | рекурсивные вызовы: `fib(32)`, ~7 млн вызовов |
| `03-float` | `float`: Мандельброт 240×120, до 200 итераций |
| `04-bigint` | точные целые: `12000!` и 150000-е число Фибоначчи |
| `05-strings` | строки: сборка 30000 строк, `split`, `replace`, разбор чисел |
| `06-collections` | `map`/`filter`/`reduce`, `Sorted`, `Frequencies`, словари |
| `07-objects` | 1 млн successor-шагов неизменяемых объектов |
| `08-roles` | вызовы через роль над 300000 объектами четырёх видов |
| `09-closures` | замыкания, композиция, action-значения в коллекциях |
| `10-app` | приложение: 6 модулей, роли правил цены, ошибки домена, async, отчёт |

Конфигурации: `jvm` (`java -jar` собранного JAR), `sparrow` (VM с JIT),
`sparrow-interp` (`OKKAMA_JIT=off`), `ir` (`okkama run --engine ir`,
включая разбор и lowering).

Для сравнения рядом с `main.ok` лежат те же программы на Python, Perl и Ruby
(`main.py`, `main.pl`, `main.rb`; конфигурации `python`, `perl`, `ruby`). Порт
повторяет алгоритм `main.ok` и печатает тот же вывод: неизменяемые обновления
создают новый объект, хвостовая рекурсия Okkama — цикл, `map`/`reduce` —
идиоматические аналоги языка. В Perl и Ruby нет `async`, поэтому котировка в
`10-app` — обычный вызов. Perl считает точные целые `Math::BigInt` без GMP,
поэтому `04-bigint` у него медленный не из-за программы. Интерпретаторы берутся
из PATH; другие версии задают `--python`, `--perl`, `--ruby`.

```sh
cd okkama-rs && cargo build --release -p okkama-cli -p sparrow-cli
cd .. && python3 benchmarks/run.py --runs 5
python3 benchmarks/run.py --only 02-calls,10-app --engines sparrow,jvm
python3 benchmarks/run.py --engines sparrow,python,perl,ruby --python python3.13
```

Результат — `benchmarks/results/<время>.json` (`latest.json` — последний полный прогон; `--only`/`--engines` пишут `partial-<время>.json`):
медианы wall/user/sys, пиковая память (RSS и footprint), инструкции и циклы
CPU (`/usr/bin/time -l`, macOS), время и размер сборки `.spb` и `.jar`,
размеры бинарников. Первый прогон каждой конфигурации — прогрев, не
учитывается. Артефакты сборки — в `benchmarks/build/`.

`results/latest.json` также пишет размеры установок: бинарники `okkama` и
`sparrow`, нативную библиотеку JVM, всю установку Okkama (с исходниками std),
JDK и каталоги установки Python, Perl и Ruby (их `/usr/bin` — заглушки, поэтому
считается вся установка).

## Отчёт к релизу

`export.py` превращает результат прогона в наборы данных и отчёт:

```sh
python3 benchmarks/run.py --runs 10
python3 benchmarks/export.py benchmarks/results/latest.json \
  --before benchmarks/results/<прогон до изменений>.json \
  --out benchmarks/reports/<ГГГГ-ММ-ДД>-v<версия> --title "Бенчмарки Okkama <версия> — <дата>"
```

В каталоге отчёта: `samples.csv` (каждый замер), `stats.csv` (по паре «тест ×
движок»: n, среднее, медиана, квартили, минимум, максимум, σ, CV, медианы CPU,
RSS, инструкций и IPC), `summary.csv` (по движку: средние геометрические
отношений к Python, Perl, Ruby и JVM по тестам 01–10, запуск, память),
`builds.csv`, `installs.csv`, `before_after.csv` (с `--before`),
`environment.csv` и `REPORT.md` со всеми таблицами. Сырые `results.json` и
`before.json` кладутся рядом руками, вместе с `README.md` — выводами релиза.

| Релиз | Отчёт |
|---|---|
| 0.3.0 | [reports/2026-10-31-v0.3.0](reports/2026-10-31-v0.3.0/README.md) |
| 0.3.1 | [reports/2026-10-31-v0.3.1](reports/2026-10-31-v0.3.1/README.md) |
