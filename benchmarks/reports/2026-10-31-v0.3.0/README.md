# Бенчмарки Okkama 0.3.0 — 31 октября 2026

Замер перед релизом 0.3.0: три движка Okkama (Sparrow с JIT и без него, IR,
JVM) против тех же программ на Python, Perl и Ruby. Все таблицы — в
[REPORT.md](REPORT.md), сырые замеры — в CSV и JSON рядом.

## Итог

Среднее геометрическое отношений медианного wall-time по тестам 01–10
(больше 1 — Okkama быстрее):

| | Python 3.9 | Perl 5.42 | Ruby 2.6 | Okkama JVM |
|---|---:|---:|---:|---:|
| **Sparrow с JIT** | **2.31×** | **2.34×** | **1.72×** | **1.81×** |
| Sparrow без JIT | 0.76× | 0.68× | 0.56× | 0.59× |
| Okkama IR | 0.50× | 0.43× | 0.37× | 0.39× |

- Sparrow с JIT быстрее всех на арифметике, плавающей точке, вызовах,
  объектах, ролях, точных целых и приложении из шести модулей.
- Запуск: 5.7 мс у Sparrow, 6 мс у Perl, 20 мс у Python, 40 мс у Ruby,
  45 мс у JVM.
- Память: медиана пикового RSS — 12 МиБ у Sparrow, 11 у Python, 9 у Perl,
  31 у Ruby, 246 у JVM.
- Установка целиком: Okkama 34 МиБ (`okkama`, `sparrow`, нативная
  библиотека JVM и std), Python 63, Perl 67, Ruby 49, JDK 371.

### Где мы проигрываем

| Тест | Sparrow с JIT | Лучший | Отставание |
|---|---:|---|---:|
| 05-strings | 0.054 с | Perl 0.029 с | 1.9× |
| 06-collections | 0.092 с | Perl 0.050 с | 1.8× |
| 09-closures | 0.300 с | Okkama JVM 0.215 с | 1.4× |

Это следующие цели: строковые операции и сборка массивов в машинном коде,
вызов замыкания без копирования захватов.

Интерпретатор Sparrow (`OKKAMA_JIT=off`) быстрее Python на `01-arith` и
`04-bigint` (на `04-bigint` — быстрее всех трёх языков); в среднем он
медленнее Python в 1.3 раза.

## Что дала подготовка к релизу

Сравнение с утренним прогоном того же дня ([before.json](before.json),
5 замеров) — после него сделаны оптимизации из раздела Performance
[CHANGELOG](../../../CHANGELOG.md):

| | Sparrow с JIT | Sparrow без JIT |
|---|---:|---:|
| 01-arith | 1.19× | 3.86× |
| 02-calls | 1.01× | 2.33× |
| 05-strings | 2.37× | 4.34× |
| 06-collections | 4.01× | 6.61× |
| 07-objects | 1.06× | 1.81× |
| 10-app | 1.12× | 2.01× |

Запуск `00-hello` вырос с 5.1 до 5.7 мс (вероятно, LTO). `09-closures` с JIT не
изменился.

## Как измерено

- Apple M2 Max, 32 GiB, macOS (Darwin 26.5.2); release-сборка с fat LTO.
- 10 замеров каждой пары «тест × движок» после одного прогревочного;
  `benchmarks/run.py --runs 10`. Wall-time — часы Python, user/sys, RSS,
  footprint, инструкции и циклы — `/usr/bin/time -l`.
- Вывод всех семи конфигураций совпадает во всех 11 тестах (раннер это
  проверяет).
- Порты на Python, Perl и Ruby повторяют алгоритм `main.ok`: неизменяемые
  обновления создают новый объект, хвостовая рекурсия — цикл.

Оговорки:

- Python 3.9 и Ruby 2.6 — системные версии macOS; Python 3.11+ и Ruby 3.x с
  YJIT заметно быстрее, поэтому сравнение с ними льстит Okkama.
- У Perl `Math::BigInt` без GMP: `04-bigint` идёт 23.8 с и исключён из
  среднего для Perl.
- В Perl и Ruby нет `async`: котировка в `10-app` — обычный вызов.
- Дерево было с незакоммиченными изменениями поверх `02d9f009` — это и есть
  оптимизации из CHANGELOG.

## Файлы

| Файл | Что в нём |
|---|---|
| [REPORT.md](REPORT.md) | все таблицы: медианы, среднее/мин/макс, CV, CPU, RSS, инструкции, размеры, сборка, до/после |
| [results.json](results.json) | сырой результат `run.py` (все замеры, окружение, размеры) |
| [before.json](before.json) | утренний прогон до оптимизаций |
| [samples.csv](samples.csv) | каждый замер: тест, движок, номер, wall, user, sys, CPU, RSS, footprint, инструкции, циклы |
| [stats.csv](stats.csv) | по паре: n, среднее, медиана, квартили, мин, макс, σ, CV, медианы CPU, RSS, инструкций, IPC |
| [summary.csv](summary.csv) | по движку: отношения к Python/Perl/Ruby/JVM, запуск, суммы CPU, память, IPC |
| [builds.csv](builds.csv) | время сборки и размер `.spb`/`.jar` |
| [installs.csv](installs.csv) | размеры бинарников и установок |
| [before_after.csv](before_after.csv) | медианы Sparrow до и после |
| [environment.csv](environment.csv) | машина и версии |

Пересобрать отчёт из сырых данных:

```sh
python3 benchmarks/export.py benchmarks/reports/2026-10-31-v0.3.0/results.json \
  --before benchmarks/reports/2026-10-31-v0.3.0/before.json \
  --out benchmarks/reports/2026-10-31-v0.3.0 --title "Бенчмарки Okkama 0.3.0 — 31.10.2026"
```
