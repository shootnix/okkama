# Исполняемый эталон OIR-2 ownership — #582

Python 3.9+ и стандартная библиотека. Модель реализует конечные исполнения
протокола draft 2 от 14 ноября 2026, §§3–10, 14, 21. Это артефакт P0:
интерпретатор, парсер/принтер небольшого текстового IR, проверка протокола,
положительные и отрицательные примеры, ручные переводы корпуса.

**Результат:** 110 уникальных `.oir`-фикстур; 73 приняты, 37 отвергнуты с
ожидаемыми кодом и фрагментом сообщения. Проверены 75 положительных путей.
Все **133 TAP-проверки из 25 полных программ** корпуса воспроизведены.
Одиннадцать unittest-тестов дополнительно проверяют каталог, round-trip, другой
исход checked split, невыполненную ветвь, повреждение holder и повторяемость
событий. Машиночитаемый отчёт: [results.json](results.json).

Из 73 принимаемых примеров 8 имеют статус `proposed-581`, один —
`unresolved-fake`. Успешное исполнение этих **условных** примеров не означает
утверждения предложений. Остальные 64 — нормативный поднабор и corpus goldens.
Runtime Okkama не запускался; сравнение выполнено непосредственно с ожидаемыми
литералами `t.is(log.value(), "…", …)` в исходниках корпуса. Наблюдения старых
движков, нормативные ожидания и предложения не смешиваются.

## Запуск

Из корня worktree:

```text
python3 -m unittest discover -s tools/ownership/model -v
python3 tools/ownership/model/report.py
python3 tools/ownership/model/model.py verify tools/ownership/model/fixtures/review/02-overlap-split.oir
python3 tools/ownership/model/model.py run tools/ownership/model/fixtures/valid/pending-abort-after-destroy-prefix.oir --events
python3 tools/ownership/model/model.py format tools/ownership/model/fixtures/review/01-bound-take-aliases.oir
```

`run` сначала проверяет все пути, затем исполняет выбранный путь и печатает
JSON с `destroy`, `log`, `checkpoints`, счётчиками. `destroy` содержит только
вызовы destroy; `log` также содержит наблюдаемые эффекты тела/defer и может
сбрасываться, как Atomic log в корпусе. `--events` добавляет события с ID,
serial, holder, epoch, roots и фазами cleanup. Ошибка verifier даёт код выхода
1 и `Bномер line N: …`; успешно выполненный typed throw остаётся моделируемым
исходом программы и сам по себе не является ошибкой verifier.

`report.py` заново проверяет фикстуры и записывает только локальный
`results.json`, включая SHA-256 каждого сравниваемого исходника корпуса.
Ни одна команда не вызывает компилятор, runtime, subprocess или сеть.
Для сравнения с корпусом нужны исходники `tests/ownership-corpus`; отдельный
интерпретатор и его fixtures работают без Rust и без репозитория документации.

## Устройство и границы

| Файл | Назначение |
| --- | --- |
| [ir.py](ir.py) | Парсер, проверка арности, принтер с сохранением кавычек |
| [machine.py](machine.py) | Состояние, переходы, cleanup и глобальные инварианты |
| [checker.py](checker.py) | Исчерпывающий обход конечного CFG и edge-results |
| [model.py](model.py) | CLI |
| [test_model.py](test_model.py) | Независимые ожидания, проверки и чтение TAP goldens |
| [fixtures/manifest.json](fixtures/manifest.json) | Ожидаемые destroy/log/checkpoints либо ошибка; статус и связь с review/правилами |

Ссылка — неизменяемый `Value` с selection, deps, полями/позициями и cache views.
Идентичность имеет монотонный serial, одного holder, epoch, признак свежести
и состояние `live/destroying/dead`. Обязательства хранятся в множествах-пакетах;
имена ссылок не являются владельцами. Plain-контейнер не получает фиктивного
serial, но его содержимое участвует в managed-анализе.

Область хранит журнал, staging, defers, pending exit, failure и состояние
приостановленного cleanup. Пакеты admission, dynamic, cache, inputs, frame,
carrier и storage имеют отдельные holder ID и явную область-владельца.
После каждой инструкции проверяются сохранение обязательств, совпадение
holder с пакетом и отсутствие живых обязательств у закрытого владельца.
Проверки использования deps, линейности токенов, протокольных передач,
лексических выходов и очередного destroy выполняются на соответствующем шаге.
В конце пути проверяются все закрытия, даже пустые пакеты, unique capabilities
и неподтверждённые pending exits.

Call-entry принимает дедуплицированную свежую часть receiver/аргументов;
связанные входы остаются borrows. Call-exit закрывает callee до продолжения
caller. Для ошибок вычисления позднего аргумента callee ещё не создан.
Незавершённый constructor представлен корневой областью со staged-полями;
`new` для self появляется только на успешном завершении.

Cleanup исполняет все достигнутые defers в LIFO, затем единый serial-order
для собственного остатка. После отказа он продолжается. При abort уже
исполненный префикс сохраняется, резерв результата объединяется с оставшимся
планом. Есть специальный `self`-borrow, собственная область каждого destroy,
ручные close-step/suspend/resume, shield, отложенная отмена и resource roots.

Future имеет отдельные inputs/frame/cache и единственный исход. Поддержаны
холодный escape, never-started/cancelled, явный запуск/settle, повторные await,
фактический пакет `await.take`, unique-cache и rehome, несколько waiters,
дубликаты позиций AllFutures/Completions и повторный failure. Race представлен
явными переходами winner/loser и commit после урегулирования проигравшего.
Для внутренних carrier учитываются cause и secondaries; обычный Error plain.
Atomic заимствует; channel передаёт принятый свежий пакет при доставке;
fake storage сохраняет удалённые/вытесненные свежие входы в журнале.

Это намеренно **конечный протокольный IR**, а не каноническая грамматика всего
OIR-2 §15 или интерпретатор Okkama:

- `scope` обозначает стабильную логическую область; её состояние обновляет
  автомат. Не воспроизводятся все SSA-регистры `%q0/%q1` production IR.
  Пакетные SSA-токены линейны; значения копируемы.
- Условные переходы проверяются по обоим рёбрам, `invoke.split` — по обоим
  исходам независимо от выбранного интерпретатором. Циклический CFG отвергается;
  конечные итерации примеров развернуты вручную. Это bounded model checker,
  без доказательства индукции для произвольного цикла и без alias-analysis
  произвольного исходного языка.
- Тела вызовов развёрнуты между `call.enter/exit`. Summary проверяет
  borrow/take и наличие ABI-адаптера; роль/intrinsic не исполняют скрытый native
  код. Future body — одна из четырёх описанных ниже детерминированных схем.
  Это позволяет проверять владельцев без второго интерпретатора языка.
- Управление отказами явно в IR. `failure`, `scope.unwind`, `call.exit … f`
  задают failure-путь; код после них не перескакивает неявно в catch.
  Checked split имеет настоящий CFG normal/unwind; precommit-failure builder
  задаётся операндом `fail`. Это тестируемые точки инъекции, не исчерпывающий
  перебор отказа каждой аллокации реального runtime.
- `own/coro/low` проверяют представленные здесь переходы. `high` не принимается;
  full type/authority/descriptor dictionary/role-target анализ frontend,
  произвольные native ABI, сериализация worker, настоящий GC и codegen вне
  модели. В11 проверяет конкретные managed-графы, границы и frame-состояния
  данного поднабора. `gc` проверяет resource roots, а не машинные stack maps.
- Асинхронность кооперативная и детерминированная: расписание задаёт программа.
  Не моделируются ОС, таймеры, физические потоки, SIGKILL, произвольные тела
  async-action и все расписания scheduler. Канал представлен публикацией и
  однократной доставкой, без реализации очередей заблокированных send/receive.
- `unique-cache` консервативен: учитывает все зарегистрированные aliases,
  views и waiters; не доказывает исчезновение alias по последнему использованию.
  `drop` означает discarded demand, не уничтожение и не доказательство unique.
  Ссылочные дескрипторы остаются доступны удерживающим их агрегатам.

Счётчики `transfers`, `dynamic:*`, `visited_nodes/edges`, `max_journal` относятся
к операциям этой модели. Они полезны для различения DAG/журнала/обязательств,
но не заменяют измерений Champ/Sparrow из §21.4.

## Текстовый IR

Одна инструкция на строку; токены разбирает `shlex`, `#` начинает комментарий.
Строки с пробелами и пустые строки заключаются в кавычки. Имя `&r` читает
последнюю ссылку binding `r`, в том числе из defer. Имена значений SSA;
имена локальных значений тела destroy принадлежат конкретному вызову.

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

Checkpoint — `BBA`: defer читает B, затем B и A уничтожаются. Более сложный
пример: `fixtures/review/07-rebind-staging-pending.oir`.

| Семейство | Форма |
| --- | --- |
| Значения | `plain v`; `new v label [field=value …]`; `new.async …`; `object v field=value …`; `array v [item …]`; `alias dst src`; `use v`; `drop v` |
| Проекции | `get dst aggregate field-or-index`; `slice dst array start end`; `concat dst left right`; `replace dst array index value`; `deps.view dst source` |
| Удержание | `closure/cap/bound/role/any dst values…`; cap содержит полный owner graph |
| Области | `scope.begin S [async]`; `scope.end S`; `scope.unwind S failure`; `scope.transfer source target value` |
| Привязки | `bind name value`; `rebind name value`; `stage action field value`; `staged dst action field` |
| Пакеты | `own.empty p`; `scope.split S value p`; `own.move old new`; `own.split[.checked] p value rest selected deps`; `own.join result p…`; `own.admit p S` |
| Dynamic | `own.dynamic p target reason protocol expected-owner`; `dynamic.check value holder epoch` |
| Вызовы | `summary name borrow-or-take direct-or-role-or-closure-or-intrinsic`; `call.enter S summary args…`; `call.exit S result-or-- [failure]` |
| Pending | `exit.prepare S value`; `exit.commit S target`; `exit.abort S` |
| Defer/destroy | `defer S opcode args…`; `defer.block S 'instruction' …`; `on.destroy value 'instruction' …`; внутри тела `fail kind name` прекращает это тело |
| Отказы | `failure f typed-or-panic-or-cancel name`; `failure.cause f cause`; `failure.secondary f secondary`; `carrier f packet owner`; `failure.view value f`; `catch ErrorName saved-carrier` |
| Cleanup | `cleanup.begin S [failure]`; `cleanup.step S`; `cleanup.resume S`; `cancel S`; `gc values…`; `destroy.direct S value`; `end p boundary` |
| Независимая линейность | `succ v`; `succ.take v`; `builder.new b`; `builder.push old new value ok-or-fail`; `builder.finish b result`; `builder.abandon b` |
| Future | `future f new label bound-or-temporary [inputs…]`; вместо `new label`: `borrow value`, `void -`, `fail ErrorName` |
| Task/cache | `future.start/settle/cancel f`; `await result f`; `unique-cache capability f`; `await.take result f capability`; `cache.rehome f target` |
| Waiters/collectors | `waiter name f`; `waiter.cancel name f`; `collect.all result f…`; `completions stream f…`; `completion.next result stream` |
| Frame | `frame.save S frame`; `frame.resume S frame` в профиле `coro` |
| Mutable storage | `storage name atomic-or-channel-or-fake`; `storage.set name key value`; `storage.get result name key`; `storage.remove name key`; `storage.clear/close name` |
| Ограничения | `once/worker/native/error.payload/descriptor.plain value` |
| CFG | `label name`; `jump name`; `branch true-or-false left right`; `invoke.split packet value ok-or-fail normal unwind`; `edge.bind local $result`; `unwind.use value` |
| Наблюдения | `emit string`; `emit.label value`; `reset.log`; `check expected`; `assert.owner value S`; `assert.state S phase`; `assert.failure primary [secondaries…]`; `assert.items aggregate names…` |

В `invoke.split` normal выдаёт `$rest/$selected/$deps`, unwind —
`$packet/$failure`. Линейные edge-results нужно связать в начале соответствующего
successor; старый input недоступен на обоих рёбрах. Встроенные графовые
наблюдения (`use` агрегата) не являются callee и сохраняют admission в caller.
`end p boundary` допустим только для всей текущей явно завершаемой области;
`end p asap` запрещён. Лексические границы в fixtures — исходная аннотация,
не результат восстановления областей из машинного кода.

## Покрытие

| Раздел каталога | Файлов | Назначение |
| --- | ---: | --- |
| `corpus/` | 25 | Полные ручные переводы, все промежуточные TAP log checks |
| `review/` | 25 | Все пункты 1–24; №19 отдельно для storage и открытого fake lifetime |
| `valid/` | 26 | Дополнительные протокольные переходы, CFG, cleanup, отмена |
| `invalid/` | 34 | Отрицательные verifier-примеры; ещё 3 намеренных отказа в `review/` |

Связь с правилами записана также у каждого примера в manifest:

| Правило §14 | Основные проверки |
| --- | --- |
| В1 | SSA value/token разделены; копирование aliases; deps у managed графа |
| В2 | Сохранение creation events, disjoint packets, оба split-результата, consumed tokens, взаимоисключающие CFG edges |
| В3 | Stable deps, dead borrow, достаточный lifetime escape, полный cap/capture graph |
| В4 | Стек и однократное закрытие, rebind/staging journals, dynamic-only scope, запрет ASAP |
| В5 | Проверяемые summaries, fresh/bound inputs, role/closure/intrinsic entry, удержание только с take |
| В6 | Normal/unwind checked split, видимость edge-results, block-local unwind, все CFG successors |
| В7 | Reason/target/protocol, owner/epoch, различие borrowed storage и принятого пакета |
| В8 | Must-use отдельно от ownership, plain-successor, актуальная версия builder, precommit failure |
| В9 | Inputs/frame/cache holders, ordinary await view, unique take, граница cleanup, rehome, позиции collector |
| В10 | Общий serial-order, destroying, roots/shield, вторичные отказы, pending commit/abort, продолжение после suspend |
| В11 | Managed once/Error/native/worker/descriptor ограничения, sync/async создание, coro/low проверки поднабора |

Негативные примеры проходят напрямую через текстовый IR, не через frontend.
Для каждой ошибки сравниваются код и значимый фрагмент сообщения; случайное
падение Python или иной verifier error не считается успешным отказом.

Все 25 переводов исходников из `tests/ownership-corpus`:

| Программы | Число TAP-проверок |
| --- | ---: |
| `scopes`, `branches`, `loops`, `rebinds` | 6 + 8 + 6 + 5 |
| `temporary-receivers`, `temporary-arguments`, `call-chains`, `call-results` | 6 + 5 + 5 + 6 |
| `fields`, `arrays`, `maps`, `collection-copies` | 6 + 7 + 5 + 5 |
| `successor-fields`, `successor-staging`, `unwind`, `try-results` | 4 + 4 + 5 + 6 |
| `defer-order`, `destroy-failures`, `partial-construction`, `confirm-and-errors` | 5 + 4 + 4 + 5 |
| `closures`, `bound-actions`, `caps`, `roles-generics`, `atomic-resources` | 6 + 5 + 4 + 5 + 6 |

Переводы написаны вручную по исходникам. Конечные циклы развернуты по посещённым
итерациям; constant branches сохраняют выбранные тела и их границы. Tap/note,
чистая арифметика, строковое форматирование и закрытие plain Atomic log не
порождают ownership-событий и опущены. Порядок создания, вызовы callee,
связывание/escape, эффекты defer/catch и все log checkpoints сохранены.
`inspect`/`label` при borrow смоделированы вызовом или контрольным `use`;
каркас map с plain keys представлен именованными полями.

## Выявленные вопросы к draft 2 и предлагаемые исправления

Ниже отделены решения модели от утверждённой семантики. Эти места стали
существенными именно при записи исполняемых переходов. Исходная спецификация
не изменялась. Точки входа: OIR-2 §§4–10, 14–15, 21; нумерация review сохранена.

1. **Наблюдения коллекций и общий call-entry — §§4.1, 6.3.**
   Таблица наблюдений оставляет свежий вход в admission caller, а формулировка
   про fresh receiver/args у call-entry без исключения принимает его в callee.
   `temporary-receivers.ok` требует, чтобы `[Res("C")].size()` оставлял C в
   блоке, а `Res("A").label()` закрывал A при возврате. Решение модели:
   builtin observation — отдельная инструкция, объектный метод — call.
   **Правка:** перечислить такие intrinsics в реестре и задать свойство
   `observation/admission-stays-caller`; явно ограничить §6.3 настоящими
   call boundaries. [Пример](fixtures/corpus/temporary-receivers.oir).

2. **Свежесть пакета результата требует точного перехода — §§3.3, 6.1, 10.2–10.3.**
   Недостаточно понимать «свежая часть» как недавно созданную identity или любой
   явно возвращённый пакет. Публикация в shared cache должна закончить свежесть
   для call-entry. Иначе `label(await f)` заберёт результат из кэша. Даже
   `await.take` связанного уникального Future в той же области не должен
   автоматически делать результат временным входом следующего callee.
   **Правка:** добавить таблицу admission provenance: unbound result, bound,
   staged/carrier-owned, cache-owned, extracted-at-existing-boundary,
   temporary-await result; отдельно
   указать право fresh call transfer. В модели shared/bound extraction остаётся
   несвежим; временный take возвращает свежую часть.
   [Shared](fixtures/review/11-shared-outcome-and-borrowed-take.oir),
   [bound take](fixtures/valid/bound-unique-take-keeps-original-boundary.oir).

3. **Владелец области не равен непосредственному holder — §§3.2, 4.2, 5.2, 15.**
   Буквальное `scope.split` всего множества S способно извлечь cached outcome
   по ordinary await view, обойдя `unique-cache`; аналогичен обход протокола
   task inputs или принятого channel packet. **Правка:** определить transferable
   domain `scope.split`: только непосредственно доступные пакеты/journal S;
   cache/frame/channel/carrier — дочерние протокольные holders. Их пакеты входят
   в cleanup S после settle, но извлекаются только своим протоколом. Модель
   отвергает такой split кодом В9/В7.
   [Отрицательный пример](fixtures/invalid/B9-split-steals-shared-cache.oir).

4. **Два значения слова deps в split — §§3.2, 4.2, 15.**
   `deps = R у других владельцев` описывает внешнюю часть partition; это не
   полный `deps(value)`. После выбора owned A внешний deps может быть пустым,
   но alias A всё равно должен зависеть от стабильного `obl(A)`.
   **Правка:** назвать результат split `external_deps`, явно сохранить полный
   `value.deps`, определить retention selection для view, зависящего от owner
   без прямого поля. Нельзя сертифицировать такой view как plain только потому,
   что `selected` пуст. Модель хранит эти состояния раздельно.
   [Split](fixtures/review/02-overlap-split.oir),
   [escape](fixtures/invalid/B3-dangling-escape.oir).

5. **Правило обновления owner epoch не определено — §§3.2, 7.1, 17.**
   Обычный alias должен переживать move, а динамический snapshot owner/epoch
   после move обязан обнаруживать устаревание. При прямом хранении старого epoch
   в каждом alias эти требования конфликтуют. **Правка:** явно разделить
   стабильный obligation key/indirection и проверяемый owner snapshot; описать
   refresh/check после передачи. В модели deps обращается по стабильному ID,
   epoch меняется при реальном перемещении holder, `dynamic.check` сравнивает
   snapshot. No-op передачи epoch не меняют.
   [Живой alias](fixtures/valid/stable-deps-after-token-move.oir),
   [устаревший snapshot](fixtures/invalid/B7-stale-epoch.oir).

6. **«Нормализация» отказа destroy не задаёт catchability — §9.3, пункт 4.**
   Можно прочитать её как превращение typed Cleanup в неcatchable runtime
   failure. Корпус `destroy-failures.ok` ловит именно Cleanup. **Правка:**
   нормативно определить carrier cleanup-origin, сохраняющий исходный kind и
   точное Error для catch; panic остаётся panic. Primary/secondary priority
   ортогонален этому признаку. Модель сохраняет typed Error, добавляя событию
   `cleanup: true`. [Пример](fixtures/corpus/destroy-failures.oir).

7. **Граница внутреннего resource-bearing carrier остаётся неназванной — §9.4.**
   Требование «явная граница владельца» не определяет, кто этот владелец при
   throw/rethrow/catch, и когда относительно catch можно закрыть cause/secondaries.
   Простой admission carrier в покидаемую try-область уничтожил бы payload до
   наблюдения catch. **Правка:** добавить `carrier.attach(packet, owner)`,
   transfer/rehome и release; указать общий owner графа cause/secondaries и
   протокол cache для повторного failure. В модели `carrier f p owner` требует
   явно назначенную охватывающую область. Это соглашение для внутреннего IR,
   не разрешение пользовательских ресурсных Error.
   [Пример с чтением после catch](fixtures/valid/carrier-cause-secondary-roots.oir).

8. **Момент abort результата при отложенной отмене — §§9.1, 9.3, 10.4.**
   Текст требует доставить cancellation после shielded cleanup и до commit,
   но стоит явно закрепить порядок резервного результата относительно уже
   закрытого остатка. Для A(serial 1), return R(serial 2), отмены во время
   async destroy A модель даёт `A R`, а не ретроспективное `R A`.
   **Правка:** добавить эту трассу и правило отдельной abort boundary при
   deferred cancellation; отдельно указать отличие от отказа defer до начала
   destroy, который возвращает резерв в исходный общий план.
   [Пример](fixtures/valid/final-cleanup-cancel-aborts-result.oir).

9. **Порядок settle служебных владельцев при закрытии области — §§3.2, 9.3, 10.4–10.5.**
   Глобальный serial-order ресурсов не задаёт порядок урегулирования нескольких
   ещё работающих frames/groups. Их cleanup может создавать новые identities,
   следовательно итоговая трасса зависит от выбранного расписания ещё до общего
   плана S. **Правка:** выделить service-settle phase и её входное расписание;
   указать, какие порядки нормативны, а какие являются наблюдениями конкретного
   schedule. Эталон должен сравнивать трассу при одинаковом schedule, не
   объявлять порядок обхода таблицы Futures нормой. Модель позволяет явно
   задавать start/settle/cancel; при неявном закрытии оставшихся tasks использует
   порядок их объявления как детерминированную политику модели.
   [Waiters](fixtures/valid/shared-waiter-cancellation.oir),
   [race](fixtures/valid/race-loser-before-winner-publication.oir).

Уже открытые в самом draft 2 решения не считаются вновь найденными ошибками:

- **#581:** принять/отклонить lifetime cache-owner, rehome и views. Восемь
  связанных фикстур выполняются по предложению и отдельно помечены.
- **§7.4:** утвердить владельца свежих ресурсов общего fake storage. Одна
  фикстура проверяет предложенную область исходного fake и retired journal;
  она остаётся `unresolved-fake`.

Эти два решения и согласование формулировок выше нужны владельцу спецификации
для G0. Реализация модели не отмечает утверждение G0 или compiler gates G1–G5.
P1, backend parity и измерения реального runtime не входят в выполненную #582.
