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

## JSON

`json/` сравнивает разбор, обход и запись JSON на шести видах документов:
`records` (много мелких объектов, 17 МБ), `wide` (несколько очень широких
объектов), `deep` (глубокая вложенность), `strings` (длинные строки с
экранированием), `numbers` (точные числа), `orders` (заказы с вложенными
строками). Реализации: Okkama на Sparrow и на JVM (`walk.ok`), Python 3
(`walk.py`, модуль `json` на C), Perl (`walk.pl`, чистый `JSON::PP`). Каждая
программа сама замеряет три фазы и печатает контрольную сумму; раннер требует
её совпадения у всех.

```sh
python3 benchmarks/json/generate.py /tmp/jsonbench    # документы, детерминированно
python3 benchmarks/json/run.py --data /tmp/jsonbench --out /tmp/jsonbench/result.md
```

Медианы, мс (Apple M2 Max, обычная release-сборка без PGO, 2026-10-31):

| вид | размер | фаза | okkama | okkama-jvm | python | perl |
|---|---:|---|---:|---:|---:|---:|
| records | 17.4 MB | parse | 111.6 | 193.8 | 147.5 | 6553.8 |
|  |  | walk | 867.3 | 152.9 | 298.7 | 447.7 |
|  |  | write | 45.6 | 152.7 | 142.7 | 1375.8 |
| wide | 2.1 MB | parse | 15.2 | 60.2 | 12.1 | 840.1 |
|  |  | walk | 85.2 | 52.5 | 28.1 | 44.2 |
|  |  | write | 7.6 | 41.8 | 15.6 | 118.0 |
| deep | 2.2 MB | parse | 33.2 | 100.8 | 35.8 | 864.3 |
|  |  | walk | 301.0 | 77.0 | 94.1 | 127.9 |
|  |  | write | 21.7 | 100.1 | 41.9 | 355.9 |
| strings | 4.8 MB | parse | 16.4 | 68.1 | 18.7 | 16290.0 |
|  |  | walk | 35.0 | 14.7 | 4.9 | 9.4 |
|  |  | write | 12.4 | 77.9 | 18.1 | 100.1 |
| numbers | 3.0 MB | parse | 27.5 | 120.2 | 16.1 | 942.6 |
|  |  | walk | 161.3 | 58.2 | 69.4 | 98.0 |
|  |  | write | 34.3 | 153.5 | 27.1 | 250.8 |
| orders | 2.8 MB | parse | 20.7 | 71.4 | 18.8 | 1042.5 |
|  |  | walk | 189.2 | 55.5 | 55.6 | 82.0 |
|  |  | write | 9.9 | 60.4 | 27.4 | 248.1 |

Разбор и запись идут в runtime: разбор 0.7–1.7× от Python, запись 0.3–1.3×.
Обход — код на Okkama, один объект на значение: на Sparrow 2–7× от Python, на
JVM 0.5–3×.

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
