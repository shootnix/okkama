# Исполняемый эталон OIR-2 ownership — #582, draft 3

Python 3.9+ и стандартная библиотека. Модель реализует конечные исполнения
ownership-протокола OIR-2 draft 3 от 14 ноября 2026, §§3–10, 14, 21.
Это независимый артефакт P0: небольшой текстовый IR, интерпретатор,
ограниченный исчерпывающий verifier, фикстуры и ручные переводы корпуса.
Целевой движок проекта — Sparrow; ни он, ни другие runtime здесь не запускались.

**Результат:** 155 уникальных `.oir`-фикстур; 101 принята на 103 конечных путях,
54 отвергнуты с ожидаемыми кодом и фрагментом диагностики. Все 21 unittest
проходят. Каждый из девяти ответов M1–M9 имеет положительные и отрицательные
свидетельства. Все восемь бывших #581-условных фикстур теперь нормативны.
Отдельный fake-storage протокол и его фикстура удалены по #583; состояние
тестов подчиняется Cell/Atomic. Условных фикстур нет.

Переведена **41 полная программа**: прежние 25 и 16 новых. Из 215 исходных
TAP-проверок совпали **214**; полностью совпадают **40 программ** (25 прежних
и 15 новых). Один checkpoint `cold-futures.ok` расходится с §10.1 draft 3;
это явно сохранено как различие источника и нормы, а не выдано за паритет.
[results.json](results.json) содержит SHA-256 исходников, оба ожидания,
расписания и покрытие девяти ответов. `result: PASS` означает прохождение
нормативных проверок модели; отдельное `corpus_parity: DIFFERENCES` сообщает
о неполном совпадении с корпусом. G0 и compiler gates G1–G5 не объявляются закрытыми.

## Запуск

Из корня worktree, только Python:

```text
python3 -m unittest discover -s tools/ownership/model -v
python3 tools/ownership/model/report.py
python3 tools/ownership/model/model.py verify tools/ownership/model/fixtures/valid/M7-borrowed-cached-cause.oir
python3 tools/ownership/model/model.py run tools/ownership/model/fixtures/valid/M9-explicit-service-schedule.oir --events
python3 tools/ownership/model/model.py format tools/ownership/model/fixtures/review/01-bound-take-aliases.oir
```

`run` сначала проверяет все конечные пути, затем исполняет выбранный путь.
JSON содержит `destroy`, `log`, `checkpoints` и счётчики; `--events` добавляет
serial, holder, epoch, roots, фазы cleanup и выбор service. `destroy` перечисляет
вызовы destroy; `log` отражает наблюдаемые эффекты, включая defer и начало/конец
async destroy. Verifier error завершает CLI с кодом 1 и `Bномер line N: …`.
Моделируемый typed throw/panic сам по себе не является ошибкой verifier.

`report.py` заново проверяет каталог и записывает только локальный
`results.json`, без времени запуска и других случайных полей. Проверки читают
`tests/ownership-corpus`; отдельно IR работает без Rust, toolchain и репозитория
документации. Ни одна команда модели не вызывает subprocess, runtime или сеть.

## Состояние и переходы draft 3

| Файл | Назначение |
| --- | --- |
| [ir.py](ir.py) | Парсер, арность и принтер с сохранением кавычек |
| [machine.py](machine.py) | Протокол, cleanup, holders и глобальные инварианты |
| [checker.py](checker.py) | Все конечные CFG-пути и normal/unwind edge-results |
| [test_model.py](test_model.py) | Каталог, независимые ожидания и чтение TAP-литералов |
| [report.py](report.py) | Воспроизводимый отчёт, расхождения и расписания |
| [fixtures/manifest.json](fixtures/manifest.json) | Ручные goldens, диагностика, правила и M1–M9 |

Ссылка — копируемый `Value` с retention selection, полными deps, полями,
позициями и зависимостями от кэшей. Identity имеет стабильный ключ, serial,
единственного holder, owner через запись holder, монотонный epoch, provenance
и состояние `live/destroying/dead`. Plain-контейнер и служебный holder не
получают фиктивного resource serial. После каждой инструкции проверяются
сохранение обязательств и согласованность holder; в конце проверяются все
области, пакеты, capabilities и pending exits, включая пустые состояния.

Девять прежних вопросов закрыты нормативно в приложении А.2 draft 3.
Здесь перечислены реализованные ответы, а не новые вопросы:

| Ответ | Исполняемое правило и свидетельство |
| --- | --- |
| M1 | Реестр конкретных observation-opcode (§6.5) сохраняет admission caller; объектный метод проходит call boundary. [Положительный](fixtures/valid/M1-observation-registry.oir), [отрицательный](fixtures/invalid/M1-object-method-is-not-observation.oir) |
| M2 | Шесть состояний provenance §3.3. Bind, staging, carrier/cache publication и call-entry завершают допуск; alias/move/rehome его не восстанавливают. Bound take сохраняет старую границу, temporary take допускает только собственный пакет. [Таблица переходов](fixtures/valid/M2-provenance-table.oir), [недопустимый повторный fresh transfer](fixtures/invalid/M2-bound-take-not-fresh.oir) |
| M3 | `scope.split/transfer` не извлекают cache, inputs/frame, channel, carrier или pending exit; нужны их протоколы. [Доступный домен](fixtures/valid/M3-holder-domain.oir), отрицательные `M3-*-split` и `B9-split-steals-shared-cache` |
| M4 | `selected = O ∩ selection`, `rest = O \ selection`, `external_deps = value.deps \ selected`. Полные deps остаются; owner-dependent view удерживает owner без прямого поля. [Удержание и partition](fixtures/valid/M4-retention-and-external-deps.oir), [пустой selected не plain](fixtures/invalid/M4-empty-selection-is-managed.oir) |
| M5 | Реальная смена holder/owner увеличивает epoch один раз. No-op и логическое SSA-переименование holder не увеличивают его. Alias использует stable key; snapshot проверяется отдельно, refresh проверяет live/lifetime. [Rename/refresh](fixtures/valid/M5-epoch-refresh-and-rename.oir), [rehome инвалидирует snapshot](fixtures/invalid/M5-rehome-stales-snapshot.oir) |
| M6 | `cleanup_origin` хранится в Failure отдельно от kind, точного Error и primary/secondary. Typed Cleanup catchable, panic — нет. [Typed](fixtures/valid/M6-typed-cleanup-origin.oir), [panic](fixtures/invalid/M6-panic-is-not-typed-cleanup.oir) |
| M7 | Carrier attach/transfer/rehome/release охватывают весь cause/secondary graph. Default owner — охватывающий handler; корень требует диагностический контекст. Cached cause остаётся borrow, повторный failure имеет один carrier и пакет. [Протокол](fixtures/valid/M7-carrier-protocol.oir), [cached cause](fixtures/valid/M7-borrowed-cached-cause.oir), [запрет release shared cache](fixtures/invalid/M7-cache-carrier-cannot-release.oir) |
| M8 | Abort имеет отдельную границу и сохраняет исполненный префикс. Отмена во время destroy A при return R даёт A R; отказ defer до destroy даёт R A. [Отмена](fixtures/valid/final-cleanup-cancel-aborts-result.oir), [defer](fixtures/valid/M8-defer-failure-returns-reserve.oir), [запрет commit](fixtures/invalid/M8-cancelled-return-cannot-commit.oir) |
| M9 | Фазы `open → deferring → service-settle → closing → closed`. Служебные holders отдают остаток в журнал до общего serial-плана. Явный schedule задаёт выбор задач; ошибки не пропускают остальные services. [Расписание](fixtures/valid/M9-explicit-service-schedule.oir), [неполное расписание](fixtures/invalid/M9-incomplete-service-schedule.oir) |

Тест двух допустимых расписаний получает `BYAXO` и `AXBYO`: ресурсы X/Y создаются
в локальном destroy задач, а O закрывается после service-settle. Повтор одного
расписания даёт одинаковые события. Порядок объявления оставшихся tasks —
детерминированная политика модели, а не норма языка. Неявное закрытие запрашивает
отмену работающих детей; cold Future не запускается. Собственные границы задач
сохраняются; их уже исполненные эффекты не сортируются заново в плане owner.

## Протокольный IR

Формы — поднабор модели, а не production-грамматика §15:

| Семейство | Основные формы |
| --- | --- |
| Значения | `plain`; `new/new.async value label [field=value …]`; `object/array`; `alias`; `use`; `drop`; `get/slice/concat/replace`; `deps.view`; `closure/cap/bound/role/any` |
| Области | `scope.begin S [async]`; `scope.end/unwind`; `bind/rebind`; `stage/staged`; `scope.transfer source target value` |
| Пакеты | `own.empty/move/join/admit`; `scope.split S value packet`; `own.split[.checked] packet value rest selected external_deps`; `fresh.transfer packet callee` |
| Dynamic | `own.dynamic packet target reason protocol expected-owner`; `dynamic.check value owner holder epoch`; `dynamic.refresh snapshot value`; `snapshot.check snapshot` |
| Вызовы | `summary name borrow-or-take direct-or-role-or-closure-or-intrinsic`; `call.enter/exit`; `observe opcode value` |
| Pending/cleanup | `exit.prepare/commit/abort`; `defer/defer.block`; `cleanup.begin/step/resume`; `cancel`; `service.schedule S futures…`; `service.settle S`; `destroy.direct`; `end packet boundary`; `gc` |
| Destroy | `on.destroy value 'instruction' …`; `destroy.log value begin-text end-text` задаёт наблюдаемые эффекты до suspend и после resume отдельно от имени identity |
| Failure | `failure f typed-or-panic-or-cancel exact-error`; `failure.cause/secondary/view`; `catch exact-error saved`; внутри тела `fail kind exact-error` |
| Carrier | `carrier.attach f packet [owner]`; `carrier.transfer f scope-or-cache:future`; `carrier.rehome/release f owner`; прежнее `carrier f packet owner` — синоним attach |
| Future | `future f new label bound-or-temporary [inputs…]`; режимы `borrow value`, `void -`, `fail Error`, `literal string`; `future.body f 'instruction' … suspend …` |
| Task/cache | `future.start/settle/cancel`; `future.request-cancel` только записывает запрос; `await`; `unique-cache`; `await.take`; `cache.rehome`; `waiter/waiter.cancel` |
| Collectors | `collect.all`; `completions/completion.next`; `race.winner result selected-future losers…`; `group/group.start/group.cancel/group.wait` |
| Mutable state | `storage name atomic-or-cell-or-channel`; `storage.set/get/remove/clear/close` |
| Независимая линейность | `succ/succ.take`; `builder.new/push/finish/abandon`; `once/worker/native/error.payload/descriptor.plain` |
| CFG | `label/jump/branch`; `invoke.split packet value ok-or-fail normal unwind`; `edge.bind`; `unwind.use` |
| Наблюдения | `emit/emit.label/reset.log/check`; `check.value`; `assert.owner/state/failure/items/provenance/cleanup-origin` |

Normal-результаты checked split — `$rest/$selected/$external_deps`, unwind —
`$packet/$failure`. Пакетные edge-results связываются в начале successor;
старый consumed input недоступен на обоих рёбрах. Для старого краткого
`dynamic.check value holder epoch` также проверяется live/holder/epoch;
четырёхоперандная форма дополнительно сравнивает owner.

`future.body` содержит конечный префикс до `suspend` и продолжение до исхода.
Префикс создаёт настоящую область задачи с локальными ресурсами и defers;
settle закрывает её до публикации cache. Имена локальных SSA-значений в fixture
уникальны. `literal` представляет свёрнутый plain-результат чистого вычисления,
например строку `RA`; ownership/deps этим не выдумываются. Прямая публикация
carrier в cache предназначена для изолированных failure-протоколов с уже
урегулированными входами, без тела задачи; обычные тела проходят task cleanup.

Пример rebind/defer:

```text
profile own
summary inspect borrow direct
scope.begin caller
new a A
bind r a
scope.begin child
new b B
rebind r b
scope.end child
defer caller emit.label &r
scope.end caller
check BBA
```

## Корпус и границы проверки

| Раздел | Файлов |
| --- | ---: |
| `corpus/` | 41 |
| `review/` | 24 (пункты 1–24; №19 проверяет Atomic/channel) |
| `valid/` | 39 |
| `invalid/` | 51 (ещё три ожидаемых отказа в `review/`) |

Прежние 25 программ: `scopes`, `branches`, `loops`, `rebinds`,
`temporary-receivers`, `temporary-arguments`, `call-chains`, `call-results`,
`fields`, `arrays`, `maps`, `collection-copies`, `successor-fields`,
`successor-staging`, `unwind`, `try-results`, `defer-order`, `destroy-failures`,
`partial-construction`, `confirm-and-errors`, `closures`, `bound-actions`,
`caps`, `roles-generics`, `atomic-resources`. Все 133 проверки совпали.

Новые 16 переводов:

| Программа | TAP-проверок | Сопоставление |
| --- | ---: | --- |
| `pending/581-future-completions` | 4 | 4/4; исторический путь pending сохранён только в источнике |
| `all-futures`, `580-all-futures` | 3 + 3 | 6/6 |
| `raced` | 4 | 4/4 |
| `cold-futures` | 10 | 9/10, расхождение ниже |
| `async-results` | 8 | 8/8 |
| `channels`, `channel-transfer` | 8 + 6 | 14/14 |
| `async-destroy` | 10 | 10/10 |
| `spawn-cancellation`, `future-group` | 5 + 5 | 10/10 |
| `panic-unwind`, `secondary-failures` | 2 + 2 | 4/4; намеренные failures, включая primary/secondary проверки |
| `gen-000`, `gen-001`, `gen-002` | 4 + 4 + 4 | 12/12; block/iteration, nested/fields, normal/throw |

Переводы ручные, конечные циклы развернуты. Сохранены создание identity,
существенные call boundaries, escape, cleanup, control reads и все TAP-проверки.
Events и gates задают точки расписания; чистая арифметика, TAP plumbing и
plain Atomic log не создают ресурсных обязательств и опущены. Async destroy
сохраняет отдельные наблюдения до и после suspend; отсутствие раннего destroy
проверяется промежуточными чтениями, owners, roots и состояниями.

Это bounded protocol model: CFG-циклы отвергаются, индукции для произвольных
циклов и alias-analysis frontend нет. Проверяются все ветви и оба исхода
checked split; остальные failure-точки задаются явным IR. `own/coro/low`
не заменяют настоящий lowering, codegen, native ABI, worker serialization или
GC stack maps. `gc` проверяет ресурсные корни. Нет ОС, таймеров, потоков и
перебора всех scheduler interleavings. Коллекторы используют явные точки
start/settle; модель не воспроизводит все очереди pending-запросов библиотеки.
Канал моделирует принятие/доставку пакета и borrowed lifetime; заблокированный
непринятый send сохраняет пакет у sender. Последний capability совпадает
с явно закрываемой областью канала; произвольный подсчёт capabilities вне
поднабора. `unique-cache` консервативно учитывает все зарегистрированные
aliases/views/waiters/collectors; `drop` не доказывает исчезновение alias.

## Новые результаты сверки draft 3

**Новых неоднозначностей в девяти принятых ответах M1–M9 не обнаружено.**
Обнаружена одна новая несогласованность нормы с актуальным корпусом:

**Cold Future с plain-исходом — §10.1 против `cold-futures.ok`, TAP №9.**
`Receiver("R").read(Res("A"))` возвращает вычисленную строку `"RA"`,
не удерживающую resource identities. §10.1 требует закрыть не удержанные
исходом свежие входы при завершении action. Поэтому после первого await
модель уже имеет лог `AR`; повторный await по-прежнему возвращает `RA`.
Исходник ожидает пустой лог после обоих await и `AR` лишь при выходе
cache-owner. Решение #581 назначает владельца **кэшированному исходу**;
само по себе оно не устанавливает lifetime всех не удержанных исходом входов.

Модель следует явному правилу §10.1, сохраняет исходное ожидание `""` рядом
с нормативным `"AR"` в manifest/results и не скрывает различие. Нужна
синхронизация `cold-futures.ok` с §10.1 либо отдельное уточнение нормы о том,
что plain-исход должен удерживать такие входы. Это новая граница сверки, а не
повторное открытие M2 или решения #581. Исходник и read-only спецификация
не изменялись. Иных новых spec findings в выполненном поднаборе нет.
