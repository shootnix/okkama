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

```sh
cd okkama-rs && cargo build --release -p okkama-cli -p sparrow-cli
cd .. && python3 benchmarks/run.py --runs 5
python3 benchmarks/run.py --only 02-calls,10-app --engines sparrow,jvm
```

Результат — `benchmarks/results/<время>.json` (`latest.json` — последний полный прогон; `--only`/`--engines` пишут `partial-<время>.json`):
медианы wall/user/sys, пиковая память (RSS и footprint), инструкции и циклы
CPU (`/usr/bin/time -l`, macOS), время и размер сборки `.spb` и `.jar`,
размеры бинарников. Первый прогон каждой конфигурации — прогрев, не
учитывается. Артефакты сборки — в `benchmarks/build/`.
