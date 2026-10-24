# Завершение stdlib v4: рабочий план

Этот документ — очередь **нереализованной** функциональности корневой `std/`.
Он дополняет [архитектуру v4](stdlib-roadmap-v4.md),
[правила проектирования](stdlib-design-principles.md) и
[аудит исходного API](stdlib-api-audit-v4.md). Legacy `stdlib/` не считается
реализацией новой библиотеки. Проверки выполняются Perl-интерпретатором.

## Последние завершённые срезы

- Web: `web/*` поверх http/routing и http/server — контроллеры как bound actions,
  вложенные Scope, отдельные ExceptionScope, Exception (RFC 9457), bounded тела запросов;
  `json/read.JsonView` для типизированного чтения; JSON-числа печатаются позиционно.
  Runtime: bounded trampoline завершений (PERF-25 закрыт) и `throw` для successor-значений.
  Статус и проверки — в разделах `json/read` и `web/*` ниже.

- POP3: bounded transaction session поверх переданного авторизованного транспорта;
  STAT/LIST/UIDL, RETR/TOP, DELE/RSET/QUIT и cancellation. TLS/auth BLOCKED;
  текущий результат проверок и регрессии — в разделе `mail/pop3` ниже.

- MIME: media types, disposition, transfer encoding, encoded words, bounded
  recursive parts/multipart и определение типов по расширениям.
- Mail: содержимое сообщения, явное представление без Bcc/Resent-Bcc,
  MessageId, числовые даты с сохранением неизвестного offset и значения флагов.
  Далее проверены MailFlags/StoredMail, Maildir read/add/remove/flags с блокировкой
  и кодек mboxrd; добавлена локальная MboxSession с successor-изменениями.
  SMTP: проверены инкрементальный ASCII reply decoder, EHLO capabilities и
  bounded DATA preparation; добавлено асинхронное чтение одного ответа с явным
  remainder и композицией таймаута. Envelope, адреса и transport остаются отдельными задачами.
  IMAP: ограниченный modified UTF-7, quoted mailbox names, CAPABILITY snapshots
  и строгий разбор отдельной untagged CAPABILITY line; безопасное подключение и переключение UTF-8 ещё не реализованы.
  Добавлен authenticated rev1 protocol layer: typed commands, scoped UID actions, mailbox state и IDLE; см. текущий статус ниже.
  Добавлен bounded ASCII response framer с literal/literal8, независимыми
  successor-ветками и точным remainder; полная семантика ответов остаётся открытой.
  Добавлено асинхронное чтение одного ответа из borrowed AsyncReading: общий Future,
  ограниченные чтения, явный remainder и cleanup при отмене внутри literal.
- URL query/fragment, query/form parameters и общие message headers.
- UDP receive/send/connection, multicast и Unix stream/pair/listener;
  ограничения платформы и результаты регрессии сохранены в evidence.
- CSV: строгий разбор, лимиты входа/полей/строк/столбцов, выделение поля из
  исходного текста один раз, ограниченные snapshot и печать и настраиваемые диалекты;
  bounded resource document input; incremental row reading остаётся отдельной задачей.
- strings/join: ограниченный неизменяемый Joined для nominal Iterating<string>.
- numbers/bigrational: канонические точные дроби и четыре арифметических
  представления; итоговый статус проверки указан в соответствующем блоке ниже.

## Как пользоваться очередью

Активная задача проходит **DESIGN → CONTRACT → IMPLEMENT → VERIFIED**.
**DEFERRED** означает согласованный перенос: пропускаем в текущей очереди,
checkbox остаётся незакрытым; это не выполненная функциональность.
Статус каждого блока указан в его заголовке; остаток исторического
объёма в приложении имеет статус DESIGN. Наличие старой сигнатуры в приложении
не разрешает её реализовать без пересмотра. Удалённые v4 API не восстанавливать.

## Криптография отложена — решение 2026-09-25

По решению пользователя оставшиеся `digest/sha2`, `digest/sha3`, `digest/shake`,
`digest/blake2`, `digest/read` и все `crypto/*` пропускаются в текущем проходе.
После `digest/legacy` очередь продолжена с `compression/state` и `compression/read`;
оба раздела проверены. Алгоритмы сжатия также отложены; очередь продолжается
с `archive/path`, `archive/metadata`, `archive/entry` и `archive/limits`
(VERIFIED). `archive/file` блокируется отсутствующей no-follow file capability;
`archive/stream` реализован в части контрактов/адаптеров; в `archive/tar` готовы
TarArchive/TarWriting/ParsedTar/TarReading и ZIP writing choices. Оставшиеся
архивные задачи отложены по решению пользователя. `net/failure` проверен;
`parallel/map` проверен после runtime-исправлений cleanup и generic result type;
`net/ip` также проверен; `net/udp`, включая connected UDP, проверен.
`net/multicast` также проверен; `net/unix` реализован до stream/pair/listener
и явных unnamed endpoints. Numeric `net/tcp` и fixed/system `net/dns` проверены;
HostName/IDNA, PSL и полный URL также готовы. Для порученного HTTP-прохода
`net/tls`: X.509/trust/path verification и verified owned client transport
реализованы; полный transport gate 647/11950 PASS. Продолжаем HTTP
protocol selection/routing/pool; server/client identity ждёт PrivateKey.
IO::Socket::SSL 2.099 уже установлен в текущем Perlbrew;
его наличие не означает готовый v4 TLS-контракт. `runtime/message` не
расширяем без consuming object. Проверен `url/percent`; далее реализованы query/fragment из `url/value`,
`url/query`, `url/form` и `message/header`. Полный URL API и HostName/IDNA проверены;
`url/file` и `url/data` остаются отдельными задачами, их прежняя Url-зависимость
снята. Реализованы `mime/media_type`, `mime/disposition` и `mime/transfer`;
`mime/word` реализован для unfolded unstructured fields. `mime/part` и `mime/types`
проверены. Для `mail/address` HostName/IDNA уже готовы; `mail/auth` ещё требует
SecretBytes/TLS. `mail/message` проверен в части содержимого, MessageId и дат;
envelope/generation ждут Mailbox. `mail/mailbox`: реализованы MailFlags/StoredMail
и Maildir read/add/remove/flags с блокировкой; `mail/mbox` — ограниченный кодек
mboxrd; `mail/mbox_store` — локальные Mbox/MboxSession с атомарной заменой и постоянной блокировкой.
HTTP1 head/body read/write, owned sessions и 100-continue проверены; Internet
routing/TLS, shared pool, HTTP2 и остальная HTTP-очередь продолжаются до БД.
Последняя полная регрессия после native TLS transport: 647 файлов / 11 950
проверок, PASS, через 449/native51.
Новые std-only срезы проверяются focused/inline/examples отдельно; полная
регрессия запускается при следующем изменении кода интерпретатора/provider.
Исторические красные baseline и их исправления сохранены в отчётах аудита.

При возврате: объектные контракты и композиция в std поверх проверенной реализации
хоста/криптобиблиотеки. Не продолжать перенос криптографических примитивов на
чистый Okkama. Хостовое размещение само по себе не доказывает constant-time,
защиту ключей или корректность; эти свойства и ограничения provider проверяются
отдельно. Существующий `Core/Crypto.pm` использует `Digest::SHA` для SHA-256/384/512
и HMAC, но это ещё не полный v4 API и не утверждение о готовности всех свойств.

Реализованные `digest/state`, `digest/blocks`, `digest/legacy` остаются в истории
выполненных задач; новые алгоритмы к ним сейчас не добавляются. Возможный перевод
MD5/SHA-1 на provider — отдельная задача с сохранением контракта и эталонов.
`checksum/*` не входит в перенос: Adler-32/CRC нужны форматам сжатия и архивам.
Необходимые X.509/key модули теперь используют crypto-зависимости в рамках
HTTP/TLS-поручения; UUID использует свой существующий host provider. Для остальных
TLS/auth/подписей продолжать явно учитывать зависимости, не подменяя криптографию
заглушкой или самописным примитивом.

Текущее поручение продолжить весь HTTP-проход включает необходимые provider
зависимости; изменения runtime проходят полный regression gate. Остальные
не связанные с HTTP криптографические примитивы остаются отложенными.

## Работа над активной задачей

Для каждого следующего объекта:

- [ ] Записать, какую сущность он представляет, её неизменяемые координаты,
  роли, все публичные перегрузки, ограничения и типизированные ошибки.
- [ ] Указать момент наблюдения: конструкция, запрос, явная команда или snapshot.
- [ ] Для каждого метода назвать результат, эффект, поведение при пустом входе,
  ошибке, повторном вызове и отмене; определить границы потребления памяти.
- [ ] Указать владельца ресурса, заимствования, escape, закрытие, `destroy`,
  частичную инициализацию и передачу владения при успехе/ошибке.
- [ ] Добавить тесты контракта **до** реализации; сохранить ожидаемое падение.
- [ ] В каждом `.ok` модуле: встроенные test-блоки и подробные комментарии;
  [полный список долга и команды](stdlib-inline-tests-todo-v4.md).
- [ ] Реализовать минимальный вертикальный срез: substrate → std → consumer.
- [ ] Прогнать focused tests, соответствующую группу std, полную регрессию.
- [ ] Добавить исполняемый пример; обновить v4, текущий API и эту очередь.

Один checked checkbox означает проверенное действие, а не наличие файла или enum.
Отдельные `native` объявления без рабочего provider и тестов не являются готовностью.

## Обнаруженные при TOML native-дефекты — VERIFIED; audit других UTF-8 границ открыт

[Контракт](native-scalar-boundary-guide.md), evidence.

- [x] JSON/HTML bounds и unready regex substitution используют существующие portable codes.
- [x] Нет совпадения regex -> not_found; invalid query и compile limit имеют стабильные ошибки.
- [x] Все 66 Unicode noncharacters сохраняются UTF-8/16/32, legacy UTF-8 и stream adaptation.
- [x] Surrogate/overlong/out-of-range scalars остаются ошибками; независимые byte fixtures.
- [x] Пять существующих std-модулей получили собственные встроенные тесты и комментарии.
- [x] Полный прогон commit 96e29d2 из изолированного checkout: **403 файла / 5352 теста, PASS**.
- [ ] Отдельно проверить UTF-8 границы filesystem/environment/process с конкретными fixtures;
  там ещё используются Encode strict interchange codecs. Не менять resource-specific
  поведение без тестов его контракта.

## Приоритет 0: точные числа и вывод

### BigInt: арифметические представления — VERIFIED

Реализованы все восемь арифметических представлений, включая прежний BigDifference.
Контракт и бюджет степени описаны в [BigInt guide](bigint-guide.md).
Проверено: 24 новых группы; связанные suites — 7 файлов / 512 тестов;
полная регрессия — 371 файл / 3834 теста, PASS.
Логи red/green и регрессии.
Объекты содержат только свои операнды и явную конфигурацию;
`#projected value() is BigInt` вычисляет результат. Вызов не меняет операнды.

| TODO | Конструктор | Контракт `value()` | Обязательные проверки |
| --- | --- | --- | --- |
| [x] BigSum | `left is BigInt, right is BigInt` | точная сумма | ноль, разные знаки, перенос через сотни цифр, неизменность обоих аргументов |
| [x] BigProduct | `left is BigInt, right is BigInt` | точное произведение | 0/±1, знаки, большие числа, повторный запрос |
| [x] BigQuotient | `left is BigInt, right is BigInt` | частное с усечением к нулю; нулевой делитель — typed error | ±7/±3; неделимость; очень большие операнды |
| [x] BigRemainder | `left is BigInt, right is BigInt` | остаток: a = q*b+r, знак r как у a либо ноль | все сочетания знаков; abs(r)<abs(b); нулевой делитель |
| [x] BigPower | `base is BigInt, exponent is int` | exponent ≥ 0; степень 0 даёт 1, включая 0^0 | отрицательная степень; 0, 1, большой показатель и контролируемые ресурсы |
| [x] BigGcd | `left is BigInt, right is BigInt` | неотрицательный НОД; gcd(0,0)=0 | симметрия, нули, знаки, взаимно простые |
| [x] BigLcm | `left is BigInt, right is BigInt` | неотрицательный НОК; ноль, если один операнд ноль | произведение/НОД, знаки, большие значения без narrowing |

Зависимость: расширять `runtime/bigint` только операциями, используемыми этими
объектами; нормализовать decimal strings. Provider Perl не должен возвращать
Math::BigInt как пользовательское значение. Для Rust нужны те же fixtures.
Ошибки зафиксированы до реализации: BigDivisionByZero, InvalidBigExponent,
InvalidBigPowerLimit, BigPowerLimitExceeded. Нативные коды остаются в закрытом ABI.

### PrettyJson — VERIFIED

- [x] `PrettyJson(origin is JsonValue, indentation is int)`; неизменяемые зависимости.
- [x] `string() is string`: только протокол `JsonWriting`, без полей/кастов.
- [x] Неотрицательный indentation; отрицательный — отдельная typed error.
- [x] Пустые массив/объект печатаются `[]`/`{}`; непустые — по одному элементу
  на строку, LF, отступ level*indentation, один пробел после `:`, без конечного LF.
- [x] При indentation=0 сохраняются структурные LF и пробел после `:`.
- [x] Использовать те же правила escaping, exact Decimal и порядка членов,
  что `CompactJson`; не копировать декодирование arbitrary objects.
- [x] Golden tests: все шесть JSON-вариантов, вложенные пустые контейнеры,
  Unicode, все U+0000..001F, кавычки/слеши, большие exact numbers.
- [x] Круговой тест ParsedJson → PrettyJson → ParsedJson через nominal protocol;
  пользовательская реализация JsonValue должна печататься без type inspection.
- [x] До широкого применения определить лимит глубины/вывода для пользовательских
  JsonValue: парсер ограничен, произвольный печатаемый объект — ещё нет.

Проверено: 6 файлов / 501 тест; полная регрессия — 372 файла / 3851 тест, PASS.
[Контракт и ограничение Perl runtime](json-printing-guide.md).

### Структурный текст: CSV частично реализован; конфигурации, шаблоны — DESIGN

Порядок: CSV → TOML/INI по отдельным грамматикам → plain templates → HTML.
Не переносить CsvDialect как общедоступную запись из четырёх getters автоматически:
проверить, достаточно ли immutable dialect object со специализированным поведением
проверки/чтения/печати. Parsing и printing — разные представления одного формата.
Шаблон связывает текст и явные строковые bindings; arbitrary object reflection запрещена.
Тесты: пустые/последние поля, многострочные quotes, BOM policy, offsets, строгий
конец документа, невалидный Unicode, повторные запросы. HTML escaping и trusted
HTML должны различаться номинально. XSS-safe claim требует отдельного контракта.

## Приоритет 1: сеть как ресурсы, а не socket-records

`net/ip` проверен: числовой IpAddress с pinned IANA properties, CIDR, ленивый
обход и IpInterface. Числовой endpoint готов; текущая работа — `net/udp`.
`runtime/net` — substrate, а не завершённый сетевой стек.

### Адреса и DNS — DESIGN → CONTRACT

- [ ] IP address: parsing/formatting/order/membership CIDR; immutable bytes внутри,
  без host-specific structs. Разделить IP value и DNS host name.
- [ ] CIDR: `membership(address)` и пересечение/подсеть как поведение, проверить
  границы 0/32/128, host bits policy, IPv4-mapped IPv6, scope identifiers.
- [ ] Endpoint: coherent address+port с invariant 0..65535; отображение через
  явный protocol/representation, не универсальный `AddressData`.
- [ ] Resolver: injected capability; async query, явная политика timeout/cache.
  Fakes возвращают контролируемые последовательности IPv4/IPv6 и ошибки.
- [ ] Native results: portable addresses/errors; никакого errno/addrinfo наружу.

### TCP/UDP/Unix — DESIGN → CONTRACT

- [ ] TCP connection: отдельные input/output capabilities и owner.close;
  listener.acceptance/connection не владеет уже принятой независимой connection.
- [ ] Connect: перебор адресов и задержки fallback через injected Timing;
  победитель единственный, все проигравшие попытки закрываются при любом исходе.
- [ ] TCP tests: loopback, partial writes, EOF, half-close policy, reset,
  bind-to-port-zero, invalid port, отмена connect/accept/read/write, двойной close,
  escaped input после выхода connection variable и явный close с висящим await.
- [ ] UDP: datagram как bounded packet с контекстом отправителя; политика
  truncation и пустой datagram обязательна. Не возвращать self из async send.
  Для команды без результата использовать `async ... is void`; если число
  отправленных байтов или receipt имеет доменный смысл, сохранить этот результат.
- [ ] Unix sockets: отдельно stream/datagram, переносимые unavailable errors,
  cleanup только принадлежащего пути, не удалять чужой файл при bind.
- [ ] Вместо getter-only `*Options` выбрать immutable policy/capability,
  которая применяет только свои настройки на нужной фазе открытия.

### TLS, HTTP и WebSocket — DESIGN

Зависимости: готовые bounded streams, cancellation, deadlines, DNS, entropy.
TLS требует сертификатной проверки имени и цепочки, injected trust/clock where
applicable, handshake failures, short I/O, close_notify и отсутствия downgrade.
HTTP: ограничить headers/body, chunking, framing ambiguity, keepalive lifecycle,
redirect policy и ownership response body. WebSocket: bounded frames, маскирование,
UTF-8 fragmentation и close handshake. Полноценные protocol fixtures обязательны;
legacy реализации — материал для тестов, не готовые std modules.

## Приоритет 2: filesystem, процессы и async completion

- [ ] Копирование/извлечение/права: объект обозначает source+destination или
  конкретную запись; явная void-команда, никакой effectful `value()` projection.
- [ ] Atomic replacement — отдельный контракт, не TemporaryFilePreservation:
  последний не перезаписывает существующее назначение. Тесты same/cross filesystem,
  failure rollback, symlink, permissions, durability отдельно от atomicity.
- [x] File text drain: реализован и проверен; сохранять distinction между bounded
  binary reads и whole-rest `text()`; общий курсор, mode/encoding gate, close alias.
- [ ] Process pipelines: ограничить stdout/stderr buffering, draining при ошибках,
  закрытие stdin, kill/wait/reap, orphan/escaped capability, cancellation ordering.
- [ ] Signals: сигнал как событие capability, не произвольный async callback из
  host signal handler. Тесты subscription release, pending signal, shutdown.
- [ ] Async queues/streams: состояние в runtime capability, не mutable Okkama
  fields; waiter races, backpressure, cancellation после/до handoff и `destroy`.
- [ ] Для каждой добавляемой async операции явно назвать owner future и поведение
  при отмене родителя; не строить циклы polling в пользовательских cursor objects.

Эта группа включает будущие срезы из приложения; уже работающие API не нужно
переписывать только ради этих контрольных требований.

## Приоритет 3: archives, crypto, DB, observability

- [ ] Archives/compression: explicit Reading/Writing compositions; bounded chunks;
  zip-slip, decompression bomb budgets, truncated/checksum errors, unsafe links,
  cleanup partial output. Extraction только явной командой на destination.
- [ ] Crypto: конкретный алгоритм + явная entropy/key capability; provider-backed
  well-tested primitives. Known-answer vectors, invalid key/nonce/tag, zero-length,
  authenticated-decryption no-plaintext-before-verification policy. Не писать
  собственную криптографию на уровне std ради заполнения списка.
- [ ] DB/SQL: запрос содержит immutable bindings; connections/transactions владеют
  ресурсами; commit/rollback — команды. Row — явный schema protocol, не всеобщий
  map<any>; NULL только SQL literal. Cursor/connection lifetime и rollback on error.
- [ ] Logging: запись направляется в injected output capability; formatter —
  конкретное представление события, без глобального logger manager.
- [ ] Metrics/tracing: counter/span capabilities отражают внешний collector;
  immutable event identity, context propagation явно; bounded queue/drop policy;
  sink failures, no recursion, shutdown flush и cancellation.
- [ ] Profiling/coverage: portable observation protocol и feature availability;
  не экспортировать Perl stack frames или внутренние object fields.

## Сквозной технический долг

- [x] Diagnostic source positions: DuplicateJsonMember now exposes `located()`;
  unknown-coordinate readers throw typed `MissingJsonLocation`, never `-1`.
  Constructor-created/parser-created errors and confirm ranges are covered.
  Contract and verification.

- [ ] Остальные registry-backed capabilities (`FileWatch`, Process/stream views,
  buffers и async handles): для каждой явно выбрать scope owner или обязательный
  close/defer contract. Native registry teardown не заменяет `destroy`. Не добавлять
  destructor владельцу, не обеспечив удержание owner всеми escaping views/futures.
  Для fs/file, fs/temp и fs/lock этот срез уже выполнен и проверен.

- [ ] У Okkama нет module-private declarations: implementation collaborators
  importable. Не маскировать это «private API» обещанием, которого язык не enforce-ит.
- [x] IMP-27: field grants удалены; File выполняет acquisition и возвращает
  типизированный lock lease без доступа к FileState. См. `docs/allow-removal.md`.
- [ ] Eager snapshot constructors и native memo allocation — описанные компромиссы.
  Переводить в lazy только с тестами сохранения snapshot identity и destroy.
- [x] `JsonObject` duplicate check больше не сравнивает все пары: сортируются
  отдельные индексы имён, порядок members и первый исходный duplicate сохранены.
  PERF-59, коммит `3fd180a4`; structural RED/GREEN, целевые 5/55 и полный
  944/19 800 PASS. Отчёт.
- [ ] JSON printing сейчас по scalar writes в локальный buffer; измерить большие
  строки и глубину до добавления batching. Не ослаблять escaping ради скорости.
- [ ] Numeric/time/metadata snapshots с selectors требуют перечисленного
  domain rationale; новые поля не добавлять механически как getters.
- [ ] Для backend Rust переносить контрактные fixtures, не детали Perl host API.
- [ ] Тесты этой поставки не доказывают отсутствие всех дефектов. Новые property,
  fuzz и масштабные тесты должны проверять контракты, а не повторять код.

## Полный реестр оставшегося объёма

Ниже reconciled список из всех 320 секций v3 с учётом реально объявленного
API после v4. В каждой записи перечислены недостающие объекты/методы либо
неисполненные требования. Это рабочие задачи DESIGN, а **не** текущие публичные
сигнатуры. Старые `handle`, `execute`, `Options`/DTO и effectful projections
преднамеренно видны как предмет замены; переносить их буквально запрещено.
CONTRACT-задачи выше имеют приоритет над этой исторической нотацией.

Для каждого модуля сначала завершить его family gate, затем утвердить финальные
сигнатуры, добавить красные тесты и лишь после этого реализацию. Если функциональность
не нужна, закрыть задачу как исключённую с обоснованием, а не создавать stub.
### `collections/set` — DESIGN

**Зависимости и gate:** immutable origin, preserved aliases, empty/exhausted cursors, concrete generic use, lazy/materialized timing и scale.

Модуль частично существует; перечислены отсутствующие части. Наличие файла не закрывает эти задачи.

- [ ] **HashSet<T>** — определить сущность и контракт до реализации.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `strings/join` — VERIFIED

- [x] **Joined**: finite snapshot of nominal `Iterating<string>` and a literal
  separator; projected `string()` matches builtin @string.join.
- [x] Observe source once; preserve errors; borrow resources without closing.
- [x] Default and explicit item/output-scalar budgets; invalid configuration
  before observation, infinite-source cutoff and exact separator accounting.
- [x] Empty/single/empty-item/Unicode/control fixtures, repeated observation,
  generic List example and nominal custom sources.
- [x] Full-project regression: 386 files / 4177 tests, PASS;
  RED/GREEN evidence.

[Contract and example](strings-join-guide.md). V3's unbounded breadth is not a
reason to omit resource limits on newly implemented materializing representations.

### `diff` — bounded native LCS + std representations VERIFIED focused

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **UnifiedDiff** — bounded immutable value; [contract](diff.md).

  Кандидаты из v3 для пересмотра: `UnifiedDiff(left is Sequencing<string>, right is Sequencing<string>)`; `#projected string() is string`.

- [x] **ContextDiff** — bounded immutable value; [contract](diff.md).

  Кандидаты из v3 для пересмотра: `ContextDiff(left is Sequencing<string>, right is Sequencing<string>)`; `#projected string() is string`.

- [x] **MatchingBlocks** — bounded immutable value; [contract](diff.md).
- [x] **MatchingRatio** — bounded immutable value; [contract](diff.md).

  Кандидаты из v3 для пересмотра: `#projected value() is float`.

- [x] Remove vague algorithm-free `Similarity`.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `math/decimal` — DESIGN

**Зависимости и gate:** числовые инварианты, точность/округление, non-finite policy, typed domain errors; native только при необходимости.

Модуль частично существует; перечислены отсутствующие части. Наличие файла не закрывает эти задачи.

- [ ] **DecimalSum** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `DecimalSum(left is Decimal, right is Decimal)`; `#projected value() is Decimal`.

- [ ] **DecimalDifference** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `DecimalDifference(left is Decimal, right is Decimal)`; `#projected value() is Decimal`.

- [ ] **DecimalProduct** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `DecimalProduct(left is Decimal, right is Decimal)`; `#projected value() is Decimal`.

- [ ] **DecimalDivision** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `DecimalDivision(left is Decimal, right is Decimal, scale is int, rounding is Rounding)`; `#projected value() is Decimal`.

- [ ] **Quantized** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Quantized(origin is Decimal, exponent is int, rounding is Rounding)`; `#projected value() is Decimal`.


**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `numbers/bigint` — VERIFIED

Семь недостающих арифметических объектов реализованы и проверены.
[Контракт, ограничения и пример](bigint-guide.md). Полная регрессия:
371 файл / 3834 теста, PASS; связанные suites: 7 файлов / 512 тестов, PASS.


### `numbers/bigrational` — VERIFIED

- [x] **BigRational**: canonical reduced BigInt numerator/positive denominator;
  zero is 0/1, zero denominator is a typed constructor failure.
- [x] Exact Comparing<BigRational>, canonical n/d text and structural equality;
  no narrowing to builtin numeric types.
- [x] BigRationalSum, BigRationalDifference, BigRationalProduct,
  BigRationalQuotient: immutable expression operands and projected value().
- [x] Division by zero is checked on quotient projection, not construction.
- [x] Sign/zero/canonical-coordinate fixtures, repeated results and unchanged
  operands, independent small-arithmetic tables, 80/100-digit exact fixtures.
- [x] Full regression: 387 files / 4210 tests, PASS;
  RED/GREEN evidence.

[Contract and example](bigrational-guide.md). Existing BigInt substrate only;
no new native API. Result/intermediate size budgets remain a separate numeric
resource-policy task, as for BigInt sum/product.

### `math/complex` — DESIGN

**Зависимости и gate:** числовые инварианты, точность/округление, non-finite policy, typed domain errors; native только при необходимости.

- [ ] **Complex** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Complex(real is float, imaginary is float)`; `real() is float`; `imaginary() is float`; `magnitude() is float`; `phase() is float`; `conjugate() is Complex`.

- [ ] **ComplexSum** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ComplexSum(left is Complex, right is Complex)`; `#projected value() is Complex`.

- [ ] **ComplexDifference** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ComplexDifference(left is Complex, right is Complex)`; `#projected value() is Complex`.

- [ ] **ComplexProduct** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ComplexProduct(left is Complex, right is Complex)`; `#projected value() is Complex`.

- [ ] **ComplexQuotient** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ComplexQuotient(left is Complex, right is Complex)`; `#projected value() is Complex`.

- [ ] **ComplexPower** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ComplexPower(base is Complex, exponent is Complex)`; `#projected value() is Complex`.

- [ ] **ComplexString** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ComplexString(origin is Complex)`; `#projected string() is string`.

- [ ] Specify complex exponential results.
- [ ] Specify complex logarithmic results.
- [ ] Specify complex trigonometric results.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `uuid/generation` — DESIGN

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

Модуль частично существует; перечислены отсутствующие части. Наличие файла не закрывает эти задачи.

- [ ] **Uuid7s** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Uuid7s(clock is Timing, random is SecureRandom)`; `sample() is Uuid`.

- [ ] Add a monotonic per-millisecond counter to `Uuid7s`.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `fs/path` — DESIGN

**Зависимости и gate:** locator или owner или command; symlink/permissions/missing target, bounded I/O, alias lifetime и explicit close.

Модуль частично существует; перечислены отсутствующие части. Наличие файла не закрывает эти задачи.

- [ ] **PathName** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `PathName(origin is Path)`; `name() is string`; `stem() is string`; `suffix() is string`; `suffixes() is @string`.

- [ ] **PathParts** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `PathParts(origin is Path)`; `drive() is string`; `root() is string`; `segments() is @string`.

- [ ] **NormalizedPath** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `NormalizedPath(origin is Path)`; `#projected value() is Path`.

- [ ] **AbsolutePath** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AbsolutePath(origin is Path, base is Path)`; `#projected value() is Path`.

- [ ] **RelativePath** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `RelativePath(origin is Path, base is Path)`; `#projected value() is Path`.

- [ ] **ResolvedPath** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ResolvedPath(origin is Path)`; `#projected value() is Path`.

- [ ] Let only `ResolvedPath` consult the filesystem.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `fs/stat` — DESIGN

**Зависимости и gate:** locator или owner или command; symlink/permissions/missing target, bounded I/O, alias lifetime и explicit close.

Модуль частично существует; перечислены отсутствующие части. Наличие файла не закрывает эти задачи.

- [ ] **ChangedMode** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ChangedMode(origin is FileEntry, mode is PermissionMode)`; `#projected value() is FileEntry`.

- [ ] **TouchedFile** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TouchedFile(origin is FileEntry, at is Instant)`; `#projected value() is FileEntry`.


**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `fs/copy` — DESIGN

**Зависимости и gate:** locator или owner или command; symlink/permissions/missing target, bounded I/O, alias lifetime и explicit close.

- [ ] **CopiedFile** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `CopiedFile(origin is FileEntry, destination is Path, copying is Copying)`; `#projected value() is FileEntry`.

- [ ] **CopiedTree** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `CopiedTree(origin is Directory, destination is Path, copying is Copying)`; `#projected value() is Directory`.

- [ ] Document exactly which metadata is portable.
- [ ] Use native fast-copy paths where available.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `fs/mmap` — DESIGN

**Зависимости и gate:** locator или owner или command; symlink/permissions/missing target, bounded I/O, alias lifetime и explicit close.

- [ ] **MemoryMap** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `MemoryMap(file is FileEntry, offset is int, size is int)`; `bytes(limit is int) is @byte`; `write(source is ByteSource) is void`; `flush() is void`; `closed() is bool`; `close() is void`.

- [ ] **MappedPosition** — определить сущность и контракт до реализации.
- [ ] Provide an explicit emulation on platforms without mmap.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `json/write` — VERIFIED

PrettyJson реализован через JsonWriting; лимиты и ошибки описаны в
[контракте печати](json-printing-guide.md). Полная регрессия: 372 файла / 3851 тест, PASS.

- [x] Числа печатаются позиционно, пока это коротко (пороги JS Number#toString), точно и без float;
  дальше — каноническая форма. Найдено на web/exception (`404` печаталось как `4.04e2`).
  RED/GREEN и обновлённые фикстуры: evidence.

### `json/read` — VERIFIED

[Контракт](json-reading.md). Тесты: 660 (8 проверок) и inline.

- [x] JsonView через протокол print(JsonWriting), без приведения типов; один уровень за обращение.
- [x] JSON Pointer (RFC 6901) в каждой ошибке: WrongJsonKind, MissingJsonValue, InvalidJsonNumber.
- [x] Точный BigInt, int в пределах 64 бит, ограниченное раскрытие экспоненты (4096 бит).
- [x] Нарушение протокола пользовательским JsonValue — InvalidJsonProtocol, без подмены значения.

- [x] Устранить host-предупреждения Deep recursion в Perl runtime при допустимом
  вложенном обходе. Воспроизведение: 252-std-v4-pretty-json.t, recursive custom value.
  Типизированный лимит глубины работает; нельзя маскировать произвольные warnings
  или менять ожидаемую ошибку ради зелёного теста. Проверить также обычную рекурсию
  языка и обработку исключений при раскрутке стека. Проверено в\n  24-interpreter-recursion.t: 120 рекурсивных вызовов, 121 defer при исключении,\n  исходный JsonPrintDepthExceeded. Evidence.


### `csv` — printing, parsing and dialects VERIFIED; bounded document input VERIFIED

Implemented first slice: CsvString(rows), CsvRecord(fields), CsvField(origin),
EmptyCsvRecord. [Printing contract](csv-guide.md). Fixed comma/double quote/CRLF;
the CSV module as a whole is still incomplete. Initial printing checks: 4 files / 494 tests,
PASS. No full-suite claim is made for this slice.

Fixed-format parsing follow-up: 29 new test groups; full regression —
383 files / 4103 checks, PASS. RED/GREEN evidence.
CsvRows and InvalidCsv are VERIFIED; budgets, dialects and bounded resource document input
are completed below; incremental row reading remains open.


**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **CsvDialect** — immutable grammar with scalar classification, field
  encoding/decoding, encoded size and bounded record printing; see csv-guide.md.
- [x] **CustomCsvDialect** candidate resolved by CsvDialect(delimiter, quote, line_ending).
  No separate getter-only wrapper or doubled_quotes switch; doubling is mandatory.
- [x] **ExcelCsvDialect / UnixCsvDialect / TabCsvDialect** candidates retired as
  redundant preset types. Use `CsvDialect(",", "\"", "\r\n")`,
  `CsvDialect(",", "\"", "\n")`, or `CsvDialect("\t", "\"", "\n")`.
  These are grammar choices, not claims of full spreadsheet interoperability.

- [x] **CsvRows** — fixed-format `CsvRows(origin is string)` with lazy
  construction, full validation on cursor(), and independent immutable cursors.
  Configurable two/six-argument dialect overloads are implemented.

- [x] **CsvString** - fixed-format snapshot representation implemented.

  `CsvString(rows is Iterating<@string>)`; `#projected string() is string`.
- [x] Configurable dialect behavior — VERIFIED: 33 new groups, focused 8 files /
  624 tests; full 388 files / 4246 tests, PASS. Preserved default overloads; tests 265.
  Evidence.

- [x] **InvalidCsv** — `InvalidCsv(offset is int)`; `offset() is int` and
  `text() is string`. Offset counts Unicode scalars from zero, including EOF;
  redundant stored line/column coordinates are not part of the contract.

- [x] Reject equal delimiter and quote characters.
- [x] Reject an empty line ending.
- [x] Preserve empty fields and empty final fields.
- [x] Preserve embedded line endings inside quoted fields.
- [x] Quote fields only when their contents require it.
- [x] Make malformed quoting fail instead of guessing.
- [x] Keep blocking CSV input out of `Cursor` traversal.
- [x] Read budgets — VERIFIED (384 files / 4127 tests, PASS): default and explicit scalar input/field/row/column limits;
  typed InvalidCsvLimits/CsvLimitExceeded, zero budgets, quoted field accounting.
- [x] Replace repeated field concatenation with source offsets and one final decode.
- [x] Printing budgets — VERIFIED (385 files / 4152 tests, PASS): bound snapshot traversal and encoded output before allocation;
  repeated observation, infinite-source cutoff and quoted scalar accounting tested.
- [x] Первый ресурсный срез: `csv/input.CsvInput.read()` читает bounded UTF-8
  документ из borrowed Reading и возвращает проверенные строки. Это явная
  операция, без I/O внутри Cursor. [Контракт](csv-input-guide.md), встроенные
  тесты в модуле и внешние 267. Ошибки не возвращают частичных строк и не
  откатывают источник; повторный read потребляет оставшиеся байты.
  Проверка: 8 файлов / 628 тестов, PASS; evidence.
- [ ] Отдельный следующий ресурсный срез DESIGN: incremental row-at-a-time
  reader, terminal failure state и owned-source adapters. Не блокирует переход
  к TOML после готового bounded document input. План:
  - [ ] Выбрать существующую capability чтения и отдельный row-reading протокол;
    определить EOF отдельно от пустой строки и пустого поля, синхронность/отмену.
  - [ ] Зафиксировать borrowed/owned варианты, escaped reader, явный close,
    повторный close и поведение после закрытия источника; не закрывать borrowed origin.
  - [ ] До кода: fake источник с разрывами внутри UTF-8, CRLF и удвоенной кавычки;
    ошибки чтения, EOF в quoted field, пустой источник и повторный EOF.
  - [ ] До кода: бюджеты документа/поля/строки/колонок, ограниченное чтение вперёд,
    отсутствие частично выданной строки при ошибке, точные scalar offsets.
  - [ ] Реализовать один формат через текущий CsvDialect, затем все поддержанные
    диалекты на тех же fixtures; не нормализовать исходные newlines до разбора.
  - [ ] Интеграционный пример с ресурсом, focused/full regression, контракт API,
    evidence и отдельный commit. Streaming output остаётся отдельным решением.


**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `toml/value` — value model and temporal coordinates VERIFIED

[Контракт](toml-values-guide.md). Не переносить getter-only DTO из v3.
Проверено: 32 внешних сценария, 5 встроенных групп; связанные проверки —
5 файлов / 560 тестов, PASS. Evidence.

- [x] **TomlValue / TomlVisitor** — номинальный typed callback protocol `accept`;
  наблюдатель выбирает обход, без kind/cast/JSON и скрытой рекурсии.
- [x] **TomlString / TomlInteger / TomlFloat / TomlBoolean** — точные typed
  координаты и однократная передача соответствующему методу observer.
- [x] **TomlArray** — bounded snapshot Sequencing<TomlValue>, смешанные типы,
  порядок и повторное наблюдение, `values()` без повторного чтения source.
- [x] **TomlMember / TomlTable** — ordered snapshot, literal Unicode names,
  membership/value/members, typed duplicate/missing errors; без map inheritance.
- [x] **MissingTomlMember / DuplicateTomlMember** — имя проблемного binding и text.
- [x] **InvalidTomlLimit / InvalidTomlSize / TomlLimitExceeded** — ошибки bound и
  неправомерного отрицательного source.size; отказ до чтения элементов.
- [x] Модель независима от JsonValue; таблицы сохраняют исходный порядок.
- [x] Встроенные тесты и документация находятся в `std/toml/value.ok`.

Следующие микрозадачи перед `toml/parse`:

- [x] **TomlOffsetDateTime / TomlLocalDateTime / TomlLocalDate / TomlLocalTime**:
  собственные валидируемые координаты; год 0000, nanosecond, запись секунды 60,
  отсутствие неявного преобразования в системный instant/time zone.
- [x] **TomlOffset** сохраняет known/unknown zero: Z и -00:00 различимы;
  диапазон ±23:59, unknown допускается только при minutes=0.
- [x] 40 внешних fixtures и 3 встроенные группы написаны до реализации;
  расширен visitor четырьмя nominal callbacks.
- [x] Контракт: проверка фактического leap event не входит в модель координат;
  будущий parser обрезает fractional digits до наносекунд, никогда не округляет.
  Temporal checks: 11 файлов / 645 тестов; полный прогон после исправления
  анализатора — 393 файла / 4371 тест, PASS. Evidence.
- [x] Parser обрезает fractional digits до наносекунд; тесты проверяют отсутствие округления.
- [x] Проверить binary64, ±0, ±inf и nan перед реализацией float parser:
  TomlFloat сохраняет конечный runtime float, включая знаковый ноль и subnormal;
  TomlInfinity и TomlNaN передают специальные категории через typed visitor.
  Исправлены потеря знакового нуля и unchecked overflow runtime Float;
  RED/GREEN evidence.
  Конформность будущего текстового parser проверяется отдельно.
- [ ] При необходимости заменить O(n²) duplicate check таблиц на bounded index
  с теми же Unicode/equality fixtures; текущий default maximum_members=1024.
- [x] ParsedToml реализован ниже; независимые fixtures проверяют TOML 1.0.

### `toml/parse` — VERIFIED

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **ParsedToml** — [контракт и последовательность реализации](toml-parse-guide.md) записаны до кода.

  Принято: ParsedToml(origin) и overload с тремя бюджетами; явный value() is TomlTable.

- [x] **InvalidToml** — координаты и повторяемая ошибка наблюдения заданы до кода; InvalidTomlBudget проверяет конфигурацию.

  Принято: `InvalidToml(offset is int, line is int, column is int, message is string)`; `offset() is int`; `line() is int`; `column() is int`; `text() is string`.

- [x] Implement TOML 1.0 grammar.
- [x] Reject duplicate keys.
- [x] Reject conflicting dotted keys and tables.
- [x] Reject redefining a table.
- [x] Preserve arrays of tables as arrays containing `TomlTable` values.
- [x] Preserve TOML integer precision within the runtime `int` range.
- [x] Reject integers outside the runtime `int` range.
- [x] Parse offset date-times with an explicit fixed offset.
- [x] Do not add TOML writing in the first implementation.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

Тесты до реализации: 127 consumer scenarios и встроенные suites двух новых модулей.
Независимый corpus toml-test v2.2.0: 679 fixtures TOML 1.0, включая 205 expected trees.
Дополнительные проверки: границы глубины/узлов/контейнеров, escape, opaque handle reuse,
ошибки native ABI и teardown. Полная регрессия: **400 файлов / 5259 тестов, PASS**.
Evidence.

### `config/ini` — VERIFIED (model, parser and canonical renderer)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **IniDocument / IniContent / IniWriting** — bounded snapshot и nominal print protocol; [контракт](ini-guide.md).

  Кандидаты из v3 для пересмотра: `IniDocument(sections is Sequencing<IniSection>)`; `sections() is Sequencing<IniSection>`; `section(name is string) is IniSection`.

- [x] **IniSection / IniEntry** — ordered nominal bindings вместо map-shaped DTO; bounded snapshot и lookup.

  Кандидаты из v3 для пересмотра: `IniSection(name is string, entries is Sequencing<IniEntry>)`; `name() is string`; `membership(name is string) is bool`; `value(name is string) is string`; `entries() is Sequencing<IniEntry>`.

- [x] **ParsedIni** — lazy bounded parser по ini-guide, точные координаты, nominal values.

  Кандидаты из v3 для пересмотра: `ParsedIni(origin is string)`; `#projected value() is IniDocument`.

- [x] **IniString** — bounded canonical nominal printing; sticky errors, no partial text.

  Кандидаты из v3 для пересмотра: `IniString(origin is IniContent)`; `string() is string`.

- [x] **MissingIniSection** — typed coordinates and text реализованы; также добавлены duplicate/budget errors.

  Кандидаты из v3 для пересмотра: `MissingIniSection(name is string)`; `name() is string`; `text() is string`.

- [x] **MissingIniEntry** — typed coordinates and text реализованы; также добавлены duplicate/budget errors.

  Кандидаты из v3 для пересмотра: `MissingIniEntry(section is string, name is string)`; `section() is string`; `name() is string`; `text() is string`.

- [x] **InvalidIni** — one-based original scalar line/column and diagnostic.

  Кандидаты из v3 для пересмотра: `InvalidIni(line is int, column is int, message is string)`; `line() is int`; `column() is int`; `text() is string`.

- [x] Preserve section and entry insertion order in the model.
- [x] Treat section and entry names as case-sensitive.
- [x] Reject duplicate decoded sections and entries in model, parser and custom emission.
- [x] Model stores exact strings without interpolation.
- [x] No ambient DEFAULT section; explicit section of that name is ordinary.
- [x] Document the accepted INI grammar as Okkama's dialect before parser code.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `xml/name` — immutable namespace coordinates VERIFIED focused

**Зависимости и gate:** explicit grammar events/tree protocol, entities/DTD policy и limits, no arbitrary object reflection.

- [x] **XmlName** — accepted contract in [xml-values.md](xml-values.md).

  Кандидаты из v3 для пересмотра: `XmlName(namespace is string, local is string, prefix is string)`; `namespace() is string`; `local_name() is string`; `prefix() is string`.

- [x] Reject an empty local name.
- [x] Treat prefix as serialization metadata, not namespace identity.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `xml/node` — bounded immutable represented tree VERIFIED focused

**Зависимости и gate:** explicit grammar events/tree protocol, entities/DTD policy и limits, no arbitrary object reflection.

- [x] **XmlNode** — accepted contract in [xml-values.md](xml-values.md).

  Кандидаты из v3 для пересмотра: `xml() is string`.

- [x] **XmlText** — accepted contract in [xml-values.md](xml-values.md).

  Кандидаты из v3 для пересмотра: `XmlText(value is string)`; `value() is string`; `xml() is string`.

- [x] **XmlComment** — accepted contract in [xml-values.md](xml-values.md).

  Кандидаты из v3 для пересмотра: `XmlComment(value is string)`; `value() is string`; `xml() is string`.

- [x] **XmlProcessingInstruction** — accepted contract in [xml-values.md](xml-values.md).

  Кандидаты из v3 для пересмотра: `XmlProcessingInstruction(target is string, data is string)`; `target() is string`; `data() is string`; `xml() is string`.

- [x] **XmlElement** — accepted contract in [xml-values.md](xml-values.md).

  Кандидаты из v3 для пересмотра: `XmlElement(name is XmlName, attributes is %XmlName:string, children is Sequencing<XmlNode>)`; `name() is XmlName`; `attributes() is %XmlName:string`; `children() is Sequencing<XmlNode>`; `xml() is string`.

- [x] **XmlDocument** — accepted contract in [xml-values.md](xml-values.md).

  Кандидаты из v3 для пересмотра: `XmlDocument(root is XmlElement)`; `root() is XmlElement`; `xml() is string`.

- [x] Preserve text as child nodes in mixed content.
- [x] Preserve comment and processing-instruction positions.
- [x] Keep namespace URI separate from lexical prefix.
- [x] Reject duplicate expanded attribute names.
- [x] Do not add lossy `inner_text()`.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `xml/parse` — DESIGN

**Зависимости и gate:** explicit grammar events/tree protocol, entities/DTD policy и limits, no arbitrary object reflection.

- [ ] **ParsedXml** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ParsedXml(origin is string)`; `#projected value() is XmlDocument`.

- [ ] **InvalidXml** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `InvalidXml(offset is int, line is int, column is int, message is string)`; `offset() is int`; `line() is int`; `column() is int`; `text() is string`.

- [ ] Parse XML 1.0 documents.
- [ ] Reject multiple root elements.
- [ ] Reject undeclared namespace prefixes.
- [ ] Reject malformed entity references.
- [ ] Reject external entities.
- [ ] Reject external DTD loading.
- [ ] Never read files or network resources while parsing.
- [ ] Add entity-expansion limits for internal entities.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `xml/write` — deterministic XML 1.0 value serialization VERIFIED focused

**Зависимости и gate:** explicit grammar events/tree protocol, entities/DTD policy и limits, no arbitrary object reflection.

- [x] Accepted bounded XML writing contract in [xml-values.md](xml-values.md).
- [x] Serialize every `XmlNode` through `xml()`.
- [x] Escape text content independently from attribute content.
- [x] Reject `--` inside XML comments.
- [x] Reject invalid processing-instruction targets and data.
- [x] Emit namespace declarations deterministically.
- [x] Preserve child order exactly.
- [x] Do not add automatic pretty-printing that changes mixed content.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `html/entities` — escaping and text-context decoding VERIFIED

Контракты: [EscapedHtml / UnescapedHtml](html-entities-guide.md).
Экранирование — Pure Okkama; decoding использует существующий runtime/html
без изменений provider. Текстовый контекст, один проход, сохранение literal markup.
Проверено: 2 файла / 35 top-level tests, 6 embedded blocks и исполняемые примеры.
Escaping evidence,
decoding RED/GREEN.

**Зависимости и gate:** explicit event/printing protocol, malformed markup, escaping/trust boundary, no automatic sanitizer claim.

- [x] **EscapedHtml** — неизменяемый текст и политика кавычек; контракт и тесты выполнены.

  Кандидаты из v3 для пересмотра: `EscapedHtml(origin is string)`; `EscapedHtml(origin is string, quotes is bool)`; `string() is string`.

- [x] **UnescapedHtml** — text-context decoding, immutable source, lazy string observation.

  Кандидаты из v3 для пересмотра: `UnescapedHtml(origin is string)`; `string() is string`.

- [x] Escape ampersand, less-than and greater-than characters.
- [x] Escape quotation marks only when requested.
- [x] Decode the standard HTML named-character reference set (all 2231 pinned entries tested).
- [x] Follow HTML text-context recovery rules for malformed character references.
- [ ] Move event-based HTML parsing to Stage 14.
- [ ] Do not model HTML as lenient XML.
- [ ] Do not add a browser-style HTML DOM.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `plist/value` — VERIFIED

[Контракт](plist-value-guide.md), [реализация](../../std/plist/value.ok),
RED/GREEN evidence,
[пример](../../examples/std-v4/plist-value.ok).
13 consumer-групп и 3 встроенные группы PASS. Только std-код; без изменений
интерпретатора/provider и без полного регрессионного прогона.

- [x] **PlistValue** — самостоятельная nominal role: `kind() is PlistKind` и
  `print(to is PlistWriting)`; девять типов событий без reflection/casts.
- [x] **PlistBoolean** — `PlistBoolean(value is bool)`; `value() is bool`.
- [x] **PlistInteger** — `PlistInteger(value is BigInt)`; точный signed BigInt.
- [x] **PlistReal** — `PlistReal(value is float)`; finite float языка без narrowing.
- [x] **PlistString** — `PlistString(value is string)`; точная строка без нормализации.
- [x] **PlistData** — `PlistData(value is @byte)`; неизменяемые байты.
- [x] **PlistDate** — `PlistDate(value is Instant)`; точный момент без timezone conversion.
- [x] **PlistArray** — `PlistArray(values is Sequencing<PlistValue>)`;
  optional `maximum_items`; bounded snapshot и `values()`.
- [x] **PlistDictionary** — `PlistDictionary(members is Sequencing<PlistMember>)`;
  optional `maximum_items`; `membership(name)`, `value(name)`, `members()`.
  Кандидат native map заменён упорядоченными nominal entries.
- [x] **PlistUid** — `PlistUid(value is BigInt)`; неотрицательный непрозрачный UID,
  typed failure при отрицательном значении, без разрешения ссылок.
- [x] Keep the plist model independent from `JsonValue` (nominal rejection tested).
- [x] Preserve dictionary insertion order; exact names and typed duplicate/missing errors.
- [x] Контейнеры: size один раз, item по порядку один раз, ошибки/лимиты до чтения,
  borrowed sources без close, повторные queries и escape используют snapshot.
- [x] Комментарии, встроенные тесты, nominal print protocol, пример и API.

Форматные ограничения, XML/binary encoding и traversal budgets относятся к
следующим задачам parse/write. Наличие value model не означает готовность кодеков.

### `plist/parse` — VERIFIED (strict UTF-8 XML / bplist00)

[Контракт](plist-parse-guide.md), [реализация](../../std/plist/parse.ok),
RED/GREEN evidence,
[пример](../../examples/std-v4/plist-parse.ok).
Проверено: 72 parser-группы, вместе с value model — 2 файла / 85 групп PASS;
3 встроенные группы и пример PASS. Без изменений интерпретатора/provider.

- [x] **ParsedPlist** — immutable bytes, lazy repeated `#projected value() is PlistValue`;
  overload с лимитами входа, посещённых узлов и глубины.
- [x] **InvalidPlist** — исходный byte `offset()` и `text()`; отдельный
  InvalidPlistLimits для отрицательной конфигурации.
- [x] Detect XML and binary plist formats from the input: UTF-8/BOM и bplist00;
  неподдерживаемые версии/кодировки отклоняются явно.
- [x] Parse XML plist without enabling external XML entities: строгий plist-диалект,
  стандартный DOCTYPE без загрузки, запрет внутренних DTD и пользовательских сущностей.
- [x] Validate binary plist offsets before reading data: trailer, table, every
  object offset, payload span и reference bounds до чтения/материализации.
- [x] Reject cyclic or self-referential object graphs: active-path cycle check;
  shared acyclic refs разворачиваются под общим visited-node budget.
- [x] Implement plist after the other Stage 13 formats: модель и parser добавлены
  после JSON/CSV/TOML/INI/HTML entities, на существующих scalar capabilities.
- [x] Точные типы/порядок, duplicate keys, malformed input, lifetime/repeat,
  finite-real и nanosecond-date ограничения проверены consumer-тестами.
- [x] Комментарии, встроенные тесты, пример, API и evidence.

Ограничения диалекта и binary widths указаны в контракте. Unreachable binary
payloads не декодируются (их offsets проверяются); это не general XML processor.
Полная регрессия не запускалась по принятой методике. Запись реализована ниже.

### `plist/write` — VERIFIED (XML / bplist00)

[Контракт](plist-write-guide.md), [реализация](../../std/plist/write.ok),
RED/GREEN evidence,
[пример](../../examples/std-v4/plist-write.ok).
33 writer-группы; вместе с value model — 2 файла / 46 групп PASS;
4 встроенные группы и независимое чтение примера через Python plistlib PASS.

- [x] **EncodedPlist** — lazy `(origin is PlistValue, format is PlistFormat)`;
  `bytes() is @byte`, explicit byte/node/depth limits and fresh query state.
- [x] Write deterministic dictionary ordering: preserve ordered nominal entries;
  fixed postorder binary layout, no hidden sorting/deduplication.
- [x] Normalize plist dates to UTC instants: UTC RFC3339 XML; Apple-epoch binary64
  dates reject lost nanoseconds under the existing Instant conversion contract.
- [x] Reject values unsupported by the selected plist format: XML UID/invalid
  characters, binary integer/UID widths and date range/precision fail explicitly.
- [x] Nominal PlistWriting traversal: exactly one event, custom implementations,
  no kind()/reflection; callback/source errors propagate.
- [x] Borrowed sequences: size once, node/byte preflight before items, ascending
  reads once, no close; complete envelope/table/trailer budgets and scoped buffers.
- [x] Independent XML/binary wire fixtures, repeated queries, source/protocol
  errors, recursion, limits and escaping byte snapshots.
- [x] Комментарии, встроенные тесты, пример, API и evidence.

Ограничения форматов (включая даты 0001..9999) описаны в контракте. Только std-код,
без изменений интерпретатора/provider, новых зависимостей и полной регрессии.

### `regex/search` — VERIFIED

Повторный аудит 2026-09-25: обе ошибки уже реализованы в `regex/pattern` и
описаны в API v4. Search использует эти номинальные типы; дублировать их не нужно.
Контракт и evidence.

- [x] **RegexRangeOutsideInput** — конструктор `(range is RegexRange, length is int)`;
  `range()` сохраняет выбранные полуоткрытые границы, `length()` — длину входа
  в Unicode scalar values; `text()` — `regex range lies outside its input`.
  Выход за вход проверяется при наблюдении каждого режима поиска.
- [x] **NoRegexMatch** — конструктор без аргументов, `text()` — `no regex match`.
  Ошибка обязательного first/prefix/entire match; обычное отсутствие в коллекции
  остаётся `found() == false`.

Проверка: `230-std-v3-regex-match-search.t`, 9 групп PASS, включая поля ошибок,
все режимы поиска и Unicode. Это сверка существующей реализации: RED не заявлен.

### `checksum/state` — VERIFIED

[Контракт](checksum-guide.md#checksumstate-contract) ·
evidence.

- [x] **Checksumming** — `state(origin is @byte) is ^Checksumming`; `value() is int`.
  Возврат уточнён до successor по текущей семантике Okkama; fake считает байты.
- [x] Immutable states; append оставляет исходное состояние и массив без изменений.
- [x] Результат — неотрицательный `int`; алгоритмы проверяются отдельно.
- [x] Контрольные суммы явно отделены от криптографических digest.
- [x] Общая типизированная ошибка `InvalidChecksumInitial(initial, maximum)`.
- [x] Комментарии и исполняемые встроенные примеры: 3 группы; внешний контракт: 3 группы PASS.

### `checksum/adler32` — VERIFIED

[Контракт](checksum-guide.md#checksumadler32-contract) ·
evidence ·
[пример](../../examples/std-v4/checksum-adler32.ok).

- [x] **Adler32** — три конструктора, lazy `value()`, `state(bytes) is ^Adler32`.
- [x] Начальное значение по умолчанию 1; explicit seed только unsigned 32-bit.
- [x] Adler-32 по RFC 1950, включая zlib-нормализацию seed на пустом входе.
- [x] Эталоны пустого входа и `Wikipedia`, все значения байта, Unicode UTF-8,
  wraparound и вход 5553 байта проверены независимо через Python zlib.
- [x] Все разбиения `Wikipedia`, многократные successors, неизменность и ветвление.
- [x] Комментарии, API, пример; 8 внешних и 3 встроенных группы PASS.

### `checksum/crc` — VERIFIED

[Контракт](checksum-guide.md#checksumcrc-contract) ·
evidence ·
[пример](../../examples/std-v4/checksum-crc.ok).

- [x] **Crc32** — три конструктора, lazy `value()`, `state(bytes) is ^Crc32`.
- [x] **Crc16Ccitt** — два конструктора с обязательным seed, `value()`,
  `state(bytes) is ^Crc16Ccitt`.
- [x] CRC-32/ISO-HDLC: default public seed 0; explicit seed unsigned 32-bit;
  pre/post XOR `0xFFFFFFFF`, reflected polynomial `0xEDB88320`.
- [x] CRC-16: polynomial `0x1021`, MSB first, без reflection/final XOR;
  explicit seed unsigned 16-bit. Ошибки seed типизированы в обоих конструкторах.
- [x] Эталоны `123456789` для CRC-32 и CRC-16 с seed 0/65535; бинарные входы,
  все 256 байтов, независимые zlib/binascii fixtures и явный UTF-8.
- [x] Все разбиения check vector с разными seed, пустые chunks, ветвление,
  повторные наблюдения, неизменность origin и длинная последовательность successors.
- [x] Не добавлять неоднозначный `Crc64`.
- [x] Комментарии, API, пример; 11 внешних и 5 встроенных групп PASS.

### `digest/state` — VERIFIED (role contract)

[Контракт](digest-guide.md#digeststate-contract) ·
evidence.

- [x] **Digesting** — `state(origin is @byte) is ^Digesting`; `bytes() is @byte`.
- [x] Immutable successors; append сохраняет receiver, вход и ранее выданные bytes.
- [x] Контракт пустого конструктора: связать пустой вход, вычислять при `bytes()`;
  конкретные реализации и эталоны проверяются в разделах алгоритмов ниже.
- [x] Результат — raw bytes; hexadecimal через `HexEncoded`.
- [x] Unicode требует явного кодирования в bytes.
- [x] Комментарии, fake и исполняемые примеры; 3 внешние и 2 встроенные группы PASS.

### `digest/legacy` — VERIFIED

[Контракт](digest-guide.md#digestlegacy-contract) ·
evidence ·
[пример](../../examples/std-v4/digest-legacy.ok).

- [x] **Md5** — пустой/byte-array конструктор, lazy raw `bytes()` длиной 16;
  `state(origin is @byte) is ^Md5` сохраняет исходный digest.
- [x] **Sha1** — пустой/byte-array конструктор, lazy raw `bytes()` длиной 20;
  `state(origin is @byte) is ^Sha1` сохраняет исходный digest.
- [x] Официальные empty/abc vectors MD5 и SHA-1; SHA-1 empty сверен с NIST CAVS.
- [x] MD5 и SHA-1 документированы только для legacy interoperability;
  не использовать для новых security decisions.
- [x] Общая обработка `digest/blocks` на Okkama: unfinalized chaining state,
  хвост до 63 байтов, lazy новый chunk, временный padding при наблюдении.
- [x] Границы padding и блоков, high-bit length trailer, бинарные/UTF-8 эталоны,
  разбиения, ветвление, append после bytes(), ошибки и сохранённые результаты.
- [x] Комментарии, API и пример; 26 алгоритмических + 6 block + 3 role групп PASS;
  встроенные группы: legacy 3, blocks 3, state 2. Без изменений интерпретатора.

### `digest/sha2` — DEFERRED (host crypto provider)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **Sha224** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Sha224()`; `Sha224(origin is @byte)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] **Sha256** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Sha256()`; `Sha256(origin is @byte)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] **Sha384** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Sha384()`; `Sha384(origin is @byte)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] **Sha512** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Sha512()`; `Sha512(origin is @byte)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] **Sha512_224** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Sha512_224()`; `Sha512_224(origin is @byte)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] **Sha512_256** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Sha512_256()`; `Sha512_256(origin is @byte)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] Implement SHA-512/224 as its standardized variant.
- [ ] Implement SHA-512/256 as its standardized variant.
- [ ] Do not implement SHA-512/224 by truncating a SHA-512 result.
- [ ] Do not implement SHA-512/256 by truncating a SHA-512 result.
- [ ] Add official empty-input vectors for every SHA-2 variant.
- [ ] Add official `abc` vectors for every SHA-2 variant.
- [ ] Verify chunked and one-shot results for every SHA-2 variant.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `digest/sha3` — DEFERRED (host crypto provider)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **Sha3_224** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Sha3_224()`; `Sha3_224(origin is @byte)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] **Sha3_256** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Sha3_256()`; `Sha3_256(origin is @byte)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] **Sha3_384** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Sha3_384()`; `Sha3_384(origin is @byte)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] **Sha3_512** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Sha3_512()`; `Sha3_512(origin is @byte)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] Use official SHA-3 names with an underscore before output size.
- [ ] Add official empty-input vectors for every SHA-3 variant.
- [ ] Add official `abc` vectors for every SHA-3 variant.
- [ ] Verify chunked and one-shot results for every SHA-3 variant.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `digest/shake` — DEFERRED (host crypto provider)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **Shake128** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Shake128(size is int)`; `Shake128(origin is @byte, size is int)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] **Shake256** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Shake256(size is int)`; `Shake256(origin is @byte, size is int)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] Interpret size as output bytes rather than bits.
- [ ] Reject a non-positive output size.
- [ ] Preserve output size through `state()`.
- [ ] Add official vectors at multiple output sizes.
- [ ] Verify chunked and one-shot results.
- [ ] Do not allow SHAKE as an HMAC algorithm.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `digest/blake2` — DEFERRED (host crypto provider)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **Blake2b** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Blake2b()`; `Blake2b(origin is @byte)`; `Blake2b(origin is @byte, size is int)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] **Blake2s** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Blake2s()`; `Blake2s(origin is @byte)`; `Blake2s(origin is @byte, size is int)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] **KeyedBlake2b** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `KeyedBlake2b(key is @byte, size is int)`; `KeyedBlake2b(origin is @byte, key is @byte, size is int)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] **KeyedBlake2s** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `KeyedBlake2s(key is @byte, size is int)`; `KeyedBlake2s(origin is @byte, key is @byte, size is int)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] Default `Blake2b` output to 64 bytes.
- [ ] Default `Blake2s` output to 32 bytes.
- [ ] Accept `Blake2b` output sizes from 1 through 64 bytes.
- [ ] Accept `Blake2s` output sizes from 1 through 32 bytes.
- [ ] Accept `KeyedBlake2b` keys from 1 through 64 bytes.
- [ ] Accept `KeyedBlake2s` keys from 1 through 32 bytes.
- [ ] Preserve output size and key state through `state()`.
- [ ] Add official keyed and unkeyed vectors.
- [ ] Verify chunked and one-shot results.
- [ ] Keep BLAKE2 tree mode, salt and personalization out of the first API.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `digest/read` — DEFERRED (host crypto provider)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **ReadingDigest** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ReadingDigest(origin is Reading, digest is Digesting)`; `ReadingDigest(origin is Reading, digest is Digesting, chunk_size is int)`; `result() is @byte`.

- [ ] Choose and document the default chunk size.
- [ ] Reject a non-positive explicit chunk size.
- [ ] Read until `Reading.bytes()` returns an empty array.
- [ ] Continue the supplied digest state instead of replacing it.
- [ ] Propagate reading errors unchanged.
- [ ] Leave the source open after success.
- [ ] Leave the source lifecycle under caller control after failure.
- [ ] Test with reads shorter than the requested chunk size.
- [ ] Test empty input.
- [ ] Test a source larger than several chunks.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/constant-time` — DEFERRED (host crypto provider)

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **ConstantTimeEqual** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ConstantTimeEqual(left is @byte, right is @byte)`; `#projected value() is bool`.

- [ ] Compare every byte when input lengths are equal.
- [ ] Do not return at the first differing byte.
- [ ] Document that input length is not hidden.
- [ ] Accept only raw bytes.
- [ ] Add equal, unequal and unequal-length tests.
- [ ] Add a regression test preventing replacement with ordinary array equality.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/hmac` — DEFERRED (host crypto provider)

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **Hmac** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Hmac(key is @byte, algorithm is HmacAlgorithm)`; `Hmac(origin is @byte, key is @byte, algorithm is HmacAlgorithm)`; `state(origin is @byte) is Digesting`; `bytes() is @byte`.

- [ ] Accept keys of any byte length according to HMAC key normalization.
- [ ] Preserve key and algorithm through `state()`.
- [ ] Add RFC 4231 vectors for HMAC-SHA224, HMAC-SHA256, HMAC-SHA384 and HMAC-SHA512.
- [ ] Add NIST vectors for every HMAC-SHA3 variant.
- [ ] Add RFC 2202 interoperability vectors for HMAC-MD5 and HMAC-SHA1.
- [ ] Document HMAC-MD5 and HMAC-SHA1 as legacy interoperability only.
- [ ] Verify HMAC tags through `ConstantTimeEqual`.
- [ ] Do not add a separate `ValidHmac` object.
- [ ] Do not allow SHAKE as an HMAC algorithm.
- [ ] Do not wrap keyed BLAKE2 in HMAC.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `compression/state` — VERIFIED (contracts and shared primitives)

[Контракт](compression-guide.md#compressionstate) ·
evidence ·
[пример](../../examples/std-v4/compression-state.ok).

- [x] **Compressing / Decompressing** — changing-resource roles: write/output/finish/
  finished и явные close/closed; у распаковки — remainder после завершения.
- [x] `output()` передаёт независимую конечную порцию Reading: её EOF постоянен,
  пустая порция до finish не означает EOF кодека. Общий owner `CompressionOutput`.
- [x] Отложенный вывод, finish-flush, запрет write/repeated finish после завершения,
  закрытие и независимость переданных порций проверены reference fakes.
- [x] **CompressionLimitExceeded(maximum)** и **InvalidCompressionLimit(maximum)**:
  стабильные сообщения и сохранённый максимум, без host diagnostic text.
- [x] **DecompressionBudget** — положительный предел, cumulative immutable spending,
  drain не восстанавливает бюджет; проверки до вычитания, без переполнения.
- [x] Комментарии, встроенные тесты и пример: 13 внешних + 5 встроенных групп PASS.

Это контракты, budget и owner вывода, не реализация формата сжатия. Fakes только
копируют байты. Wire codecs, malformed/truncated input и roundtrip остаются
задачами конкретных форматов ниже; provider пока отсутствует.

### `compression/read` — VERIFIED (codec-independent adapters)

[Контракт](compression-guide.md#compressionread) ·
evidence ·
[исполняемый test-mode пример](../../examples/std-v4/compression-read.ok).

- [x] **CompressedReading / DecompressedReading** — consuming Reading + Closing
  поверх заимствованных origin и nominal codec; необязательный положительный
  chunk_size, по умолчанию 8192; конструкторы не потребляют зависимости.
- [x] Never return more than the requested limit.
- [x] Reject a non-positive read limit before consumption.
- [x] Buffer codec output that does not fit the requested limit.
- [x] Continue reading when a codec emits an empty intermediate chunk.
- [x] Finalize the codec after the origin reaches EOF; reject incomplete finish.
- [x] Return stable EOF only after final codec output is consumed.
- [x] Propagate origin and codec errors unchanged; failed reads are not retried.
- [x] Leave the origin and codec open after success.
- [x] Leave origin and codec lifecycle under caller control after failure.
- [x] Do not make compressed readings iterable cursors.
- [x] Idempotent close/destroy освобождают только собственные буферы;
  раннее завершение кодека не вызывает чтение источника или повторный finish.
- [x] Комментарии, 13 внешних + 3 встроенных группы PASS и исполняемый пример.

Проверены независимые от формата адаптеры с reference codecs и fault sources.
Expansion budget обеспечивает сам decompressor; его ошибка передаётся без замены.
Wire roundtrip и повреждённые/обрезанные форматы остаются в секциях кодеков ниже.

## Алгоритмы сжатия отложены — решение 2026-09-25

По решению пользователя `compression/gzip`, `compression/zlib`,
`compression/deflate`, `compression/bzip2`, `compression/xz`, `compression/lzma`
и `compression/zstd` пропускаются до появления проверенных хостовых библиотек.
В текущем std/provider нет DEFLATE и остальных кодеков; писать их заново на Okkama
в этом проходе не требуется. Запрет на изменения интерпретатора сохраняется.
Не подменять provider запуском внешних утилит или неработающими native-декларациями.
Checkbox ниже остаются незакрытыми; это перенос, а не реализация.

Готовые `compression/state` и `compression/read` сохраняются как контракты,
общие примитивы и адаптеры. Возврат к форматам потребует совместимости с эталонными
библиотеками, malformed/truncated input, cumulative expansion limits, exact EOF/
remainder и проверки владения ресурсами. Их fakes не доказывают готовность кодеков.

Разделы `archive/path`, `archive/metadata`, `archive/entry` и `archive/limits`
проверены. `archive/file` имеет технический блокер ниже. После TAR value/stream
API и ZIP writing choices оставшиеся архивные задачи также отложены пользователем;
Worker cleanup для `parallel/map` исправлен отдельным runtime-срезом;
`net/failure` проверен. При возврате к архивам сжатые ZIP
entries и HTTP content-coding должны явно
учитывать отложенную зависимость и не считаться выполненными без реальных кодеков.

### `compression/gzip` — DEFERRED (host compression provider)

**Зависимости и gate:** Reading/Writing compositions, truncated/invalid streams, expansion limits, exact roundtrip и borrowed close.

- [ ] **GzipCompressed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `GzipCompressed(origin is @byte)`; `GzipCompressed(origin is @byte, level is int)`; `bytes() is @byte`.

- [ ] **GzipDecompressed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `GzipDecompressed(origin is @byte, maximum_size is int)`; `bytes() is @byte`.

- [ ] **GzipCompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `GzipCompression()`; `GzipCompression(level is int)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`.

- [ ] **GzipDecompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `GzipDecompression(maximum_size is int)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`; `remainder() is @byte`.

- [ ] **InvalidGzip** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Implement the RFC 1952 wrapper.
- [ ] Use compression level `6` by default.
- [ ] Accept compression levels from `0` through `9`.
- [ ] Set default modification time to the Unix epoch.
- [ ] Omit filename and comment from the default header.
- [ ] Emit a deterministic OS marker.
- [ ] Validate the CRC-32 trailer.
- [ ] Validate the uncompressed-size trailer.
- [ ] Decode every concatenated gzip member.
- [ ] Apply maximum size across all concatenated members.
- [ ] Add empty-input and multi-member vectors.
- [ ] Test damaged header, payload and trailer separately.
- [ ] Verify one-shot and streaming interoperability.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `compression/zlib` — DEFERRED (host compression provider)

**Зависимости и gate:** Reading/Writing compositions, truncated/invalid streams, expansion limits, exact roundtrip и borrowed close.

- [ ] **ZlibCompressed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ZlibCompressed(origin is @byte)`; `ZlibCompressed(origin is @byte, level is int)`; `ZlibCompressed(origin is @byte, level is int, dictionary is @byte)`; `bytes() is @byte`.

- [ ] **ZlibDecompressed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ZlibDecompressed(origin is @byte, maximum_size is int)`; `ZlibDecompressed(origin is @byte, maximum_size is int, dictionary is @byte)`; `bytes() is @byte`.

- [ ] **ZlibCompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ZlibCompression()`; `ZlibCompression(level is int)`; `ZlibCompression(level is int, dictionary is @byte)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`.

- [ ] **ZlibDecompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ZlibDecompression(maximum_size is int)`; `ZlibDecompression(maximum_size is int, dictionary is @byte)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`; `remainder() is @byte`.

- [ ] **InvalidZlib** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] **MissingZlibDictionary** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `identifier() is int`; `text() is string`.

- [ ] Implement the RFC 1950 wrapper around DEFLATE.
- [ ] Use compression level `6` by default.
- [ ] Accept compression levels from `0` through `9`.
- [ ] Validate the Adler-32 trailer.
- [ ] Validate a supplied preset dictionary identifier.
- [ ] Throw `MissingZlibDictionary` when required dictionary data is absent.
- [ ] Reject trailing bytes in one-shot decompression.
- [ ] Add preset-dictionary interoperability vectors.
- [ ] Test damaged header, payload and trailer separately.
- [ ] Verify one-shot and streaming interoperability.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `compression/deflate` — DEFERRED (host compression provider)

**Зависимости и gate:** Reading/Writing compositions, truncated/invalid streams, expansion limits, exact roundtrip и borrowed close.

- [ ] **RawDeflated** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `RawDeflated(origin is @byte)`; `RawDeflated(origin is @byte, level is int)`; `RawDeflated(origin is @byte, level is int, dictionary is @byte)`; `bytes() is @byte`.

- [ ] **RawInflated** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `RawInflated(origin is @byte, maximum_size is int)`; `RawInflated(origin is @byte, maximum_size is int, dictionary is @byte)`; `bytes() is @byte`.

- [ ] **RawDeflateCompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `RawDeflateCompression()`; `RawDeflateCompression(level is int)`; `RawDeflateCompression(level is int, dictionary is @byte)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`.

- [ ] **RawDeflateDecompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `RawDeflateDecompression(maximum_size is int)`; `RawDeflateDecompression(maximum_size is int, dictionary is @byte)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`; `remainder() is @byte`.

- [ ] **InvalidDeflate** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Implement raw RFC 1951 DEFLATE without gzip or zlib wrappers.
- [ ] Use compression level `6` by default.
- [ ] Accept compression levels from `0` through `9`.
- [ ] Reject trailing bytes in one-shot decompression.
- [ ] Add preset-dictionary interoperability vectors.
- [ ] Test truncated and malformed block streams.
- [ ] Verify one-shot and streaming interoperability.
- [ ] Do not add a boolean raw-mode flag to zlib objects.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `compression/bzip2` — DEFERRED (host compression provider)

**Зависимости и gate:** Reading/Writing compositions, truncated/invalid streams, expansion limits, exact roundtrip и borrowed close.

- [ ] **Bzip2Compressed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Bzip2Compressed(origin is @byte)`; `Bzip2Compressed(origin is @byte, level is int)`; `bytes() is @byte`.

- [ ] **Bzip2Decompressed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Bzip2Decompressed(origin is @byte, maximum_size is int)`; `bytes() is @byte`.

- [ ] **Bzip2Compression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Bzip2Compression()`; `Bzip2Compression(level is int)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`.

- [ ] **Bzip2Decompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Bzip2Decompression(maximum_size is int)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`; `remainder() is @byte`.

- [ ] **InvalidBzip2** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Use compression level `9` by default.
- [ ] Accept compression levels from `1` through `9`.
- [ ] Decode every concatenated bzip2 stream in one-shot mode.
- [ ] Apply maximum size across all concatenated streams.
- [ ] Test damaged and truncated streams.
- [ ] Verify one-shot and streaming interoperability.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `compression/xz` — DEFERRED (host compression provider)

**Зависимости и gate:** Reading/Writing compositions, truncated/invalid streams, expansion limits, exact roundtrip и borrowed close.

- [ ] **XzCompressed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `XzCompressed(origin is @byte)`; `XzCompressed(origin is @byte, level is int)`; `bytes() is @byte`.

- [ ] **XzDecompressed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `XzDecompressed(origin is @byte, maximum_size is int)`; `bytes() is @byte`.

- [ ] **XzCompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `XzCompression()`; `XzCompression(level is int)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`.

- [ ] **XzDecompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `XzDecompression(maximum_size is int)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`; `remainder() is @byte`.

- [ ] **InvalidXz** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Use compression level `6` by default.
- [ ] Accept compression levels from `0` through `9`.
- [ ] Validate XZ integrity checks.
- [ ] Decode concatenated XZ streams and permitted stream padding.
- [ ] Apply maximum size across all concatenated streams.
- [ ] Test damaged index, footer and integrity checks.
- [ ] Verify one-shot and streaming interoperability.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `compression/lzma` — DEFERRED (host compression provider)

**Зависимости и gate:** Reading/Writing compositions, truncated/invalid streams, expansion limits, exact roundtrip и borrowed close.

- [ ] **LzmaCompressed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `LzmaCompressed(origin is @byte)`; `LzmaCompressed(origin is @byte, level is int)`; `bytes() is @byte`.

- [ ] **LzmaDecompressed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `LzmaDecompressed(origin is @byte, maximum_size is int)`; `bytes() is @byte`.

- [ ] **LzmaCompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `LzmaCompression()`; `LzmaCompression(level is int)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`.

- [ ] **LzmaDecompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `LzmaDecompression(maximum_size is int)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`; `remainder() is @byte`.

- [ ] **InvalidLzma** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Implement the legacy LZMA-Alone format.
- [ ] Use compression level `6` by default.
- [ ] Accept compression levels from `0` through `9`.
- [ ] Reject trailing bytes in one-shot decompression.
- [ ] Test damaged and truncated streams.
- [ ] Verify one-shot and streaming interoperability.
- [ ] Do not add raw LZMA filter chains without a concrete consumer.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `compression/zstd` — DEFERRED (host compression provider)

**Зависимости и gate:** Reading/Writing compositions, truncated/invalid streams, expansion limits, exact roundtrip и borrowed close.

- [ ] **ZstdOptions** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ZstdOptions(level is int, checksum is bool, content_size is bool)`; `level() is int`; `checksum() is bool`; `content_size() is bool`.

- [ ] **ZstdDictionary** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ZstdDictionary(data is @byte)`; `data() is @byte`; `identifier() is int`.

- [ ] **TrainedZstdDictionary** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TrainedZstdDictionary(samples is Iterating<@byte>, size is int)`; `result() is ZstdDictionary`.

- [ ] **ZstdCompressed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ZstdCompressed(origin is @byte)`; `ZstdCompressed(origin is @byte, options is ZstdOptions)`; `ZstdCompressed(origin is @byte, options is ZstdOptions, dictionary is ZstdDictionary)`; `bytes() is @byte`.

- [ ] **ZstdDecompressed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ZstdDecompressed(origin is @byte, maximum_size is int)`; `ZstdDecompressed(origin is @byte, maximum_size is int, dictionary is ZstdDictionary)`; `bytes() is @byte`.

- [ ] **ZstdCompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ZstdCompression()`; `ZstdCompression(options is ZstdOptions)`; `ZstdCompression(options is ZstdOptions, dictionary is ZstdDictionary)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`.

- [ ] **ZstdDecompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ZstdDecompression(maximum_size is int)`; `ZstdDecompression(maximum_size is int, dictionary is ZstdDictionary)`; `write(origin is @byte) is void`; `output() is Reading`; `finish() is void`; `finished() is bool`; `remainder() is @byte`.

- [ ] **InvalidZstd** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] **MissingZstdDictionary** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `MissingZstdDictionary(identifier is int)`; `identifier() is int`; `text() is string`.

- [ ] Use compression level `3` by default.
- [ ] Define stable supported compression-level bounds.
- [ ] Bind dictionary bytes during construction and validate them lazily on first use or `identifier()`.
- [ ] Reject an empty dictionary-training sample set.
- [ ] Reject a non-positive dictionary size.
- [ ] Perform dictionary training only through `result()`.
- [ ] Decode every concatenated zstd frame in one-shot mode.
- [ ] Apply maximum size across all concatenated frames.
- [ ] Validate frame checksums when present.
- [ ] Throw `MissingZstdDictionary` with the required dictionary identifier.
- [ ] Add official dictionary and multi-frame vectors.
- [ ] Test damaged headers, blocks and checksums.
- [ ] Verify one-shot and streaming interoperability.
- [ ] Keep advanced native compression parameters out of the first API.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

## Оставшиеся архивные задачи отложены — решение 2026-09-25

По решению пользователя всю дальнейшую работу над архивами пропускаем в текущем
проходе: `archive/file`, ZIP wire APIs, `archive/extract`, незакрытые format gates
и композицию со сжатием. Реализованные модули и закрытые задачи сохраняют VERIFIED;
оставшиеся checkbox не закрываем. Технический блокер no-follow и перенос host
кодеков сохраняются, но перенос архивов распространяется и на независимые задачи.
После уже отложенного блока `crypto/*` следует `parallel/map` (runtime cleanup исправлен ниже),
затем проверенный `net/failure` и активные `net/ip`, `net/endpoint`, `net/dns`.

### `archive/path` — VERIFIED

[Контракт](archive-guide.md#archivepath) ·
evidence ·
[пример](../../examples/std-v4/archive-path.ok).

- [x] **ArchivePath** — immutable relative member name, независимый от fs/Path.

  Проверенный контракт: `ArchivePath(origin is string)`; `string() is string`; `segments() is Sequencing<string>`; `parent() is ArchivePath`; `name() is string`.

- [x] **UnsafeArchivePath** — исходный путь и стабильная причина отказа.

  Проверенный контракт: `UnsafeArchivePath(origin is string, message is string)`; `path() is string`; `text() is string`.

- [x] **MissingArchiveParent** — отсутствие родителя с сохранённым ArchivePath.

  Проверенный контракт: `MissingArchiveParent(path is ArchivePath)`; `path() is ArchivePath`; `text() is string`.

- [x] Reject an empty archive path.
- [x] Reject absolute paths.
- [x] Reject `.` path segments.
- [x] Reject `..` path segments.
- [x] Reject empty path segments.
- [x] Reject backslash as a path separator.
- [x] Reject NUL characters.
- [x] Reject Windows drive prefixes.
- [x] Reject UNC prefixes.
- [x] Use `/` as the only archive path separator.
- [x] Preserve valid Unicode path characters exactly.
- [x] Make `parent()` throw `MissingArchiveParent` for a one-segment path.
- [x] Keep archive paths independent from filesystem `Path`.

**Проверка:** 10 внешних + 3 встроенных группы PASS, исполняемый пример.
Это лексическая проверка имени; filesystem containment, ссылки и ограничения
имён конкретной платформы остаются задачами extraction capability.

### `archive/metadata` — VERIFIED

[Контракт](archive-guide.md#archivemetadata) ·
evidence ·
[пример](../../examples/std-v4/archive-metadata.ok).

- [x] **ArchiveOwnership** — immutable format-boundary ownership; exact names и nonnegative BigInt IDs.

  Проверенный контракт: `ArchiveOwnership(user is string, group is string, user_identity is BigInt, group_identity is BigInt)`; `user() is string`; `group() is string`; `user_identity() is BigInt`; `group_identity() is BigInt`.

- [x] **ArchiveMetadata** — immutable snapshot; календарь/comment и необязательные mode/ownership.

  Проверенный контракт: `ArchiveMetadata(modified is LocalDateTime, comment is string)`; `ArchiveMetadata(mode is PermissionMode, modified is LocalDateTime, comment is string)`; `ArchiveMetadata(mode is PermissionMode, modified is LocalDateTime, ownership is ArchiveOwnership, comment is string)`; `mode() is PermissionMode`; `modification() is LocalDateTime`; `ownership() is ArchiveOwnership`; `comment() is string`.

- [x] **MissingArchiveMode** — типизированное отсутствие mode.

  Проверенный контракт: `text() is string`.

- [x] **MissingArchiveOwnership** — типизированное отсутствие ownership.

  Проверенный контракт: `text() is string`.

- [x] Make `mode()` throw `MissingArchiveMode` when mode is absent.
- [x] Make `ownership()` throw `MissingArchiveOwnership` when ownership is absent.
- [x] Preserve timezone-naive ZIP calendar fields as `LocalDateTime`.
- [x] Represent TAR epoch timestamps as UTC `LocalDateTime` values.
- [x] Never interpret ZIP timestamps through the process timezone.
- [x] Do not use magic identities such as `-1` for missing ownership.

- [x] **ArchiveModificationFromEpoch(seconds is rat)** — явное преобразование в UTC
  LocalDateTime, целые наносекунды, годы 1..9999; нет зависимости от process TZ.
- [x] **InvalidArchiveOwnership** — отрицательные UID/GID отклоняются при создании.

**Проверка:** 11 внешних + 3 встроенных группы PASS, пример; проверки календаря
повторены при TZ=UTC, America/New_York и Asia/Tokyo. Это модель metadata,
не парсер TAR/ZIP. Три конструктора имеют arity 2/3/4; ownership без mode
пока не представляется, подставлять фиктивный mode нельзя.

### `archive/entry` — VERIFIED

[Контракт](archive-guide.md#archiveentry) ·
evidence ·
[пример](../../examples/std-v4/archive-entry.ok).

- [x] **ArchiveEntry** — nominal role для неизменяемых записей.

  Проверенный контракт: `path() is ArchivePath`; `kind() is ArchiveEntryKind`; `metadata() is ArchiveMetadata`; `accept(to is ArchiveEntryVisitor) is void`.

- [x] **ArchiveFile** — immutable member value; контракт проверен.

  Проверенный контракт: `ArchiveFile(path is ArchivePath, metadata is ArchiveMetadata, data is @byte)`; `path() is ArchivePath`; `kind() is ArchiveEntryKind`; `metadata() is ArchiveMetadata`; `data() is @byte`.

- [x] **ArchiveDirectory** — immutable member value; контракт проверен.

  Проверенный контракт: `ArchiveDirectory(path is ArchivePath, metadata is ArchiveMetadata)`; `path() is ArchivePath`; `kind() is ArchiveEntryKind`; `metadata() is ArchiveMetadata`.

- [x] **ArchiveSymbolicLink** — immutable member value; контракт проверен.

  Проверенный контракт: `ArchiveSymbolicLink(path is ArchivePath, metadata is ArchiveMetadata, target is ArchivePath)`; `path() is ArchivePath`; `kind() is ArchiveEntryKind`; `metadata() is ArchiveMetadata`; `target() is ArchivePath`.

- [x] **ArchiveHardLink** — immutable member value; контракт проверен.

  Проверенный контракт: `ArchiveHardLink(path is ArchivePath, metadata is ArchiveMetadata, target is ArchivePath)`; `path() is ArchivePath`; `kind() is ArchiveEntryKind`; `metadata() is ArchiveMetadata`; `target() is ArchivePath`.

- [x] **ArchiveFifo** — immutable member value; контракт проверен.

  Проверенный контракт: `ArchiveFifo(path is ArchivePath, metadata is ArchiveMetadata)`; `path() is ArchivePath`; `kind() is ArchiveEntryKind`; `metadata() is ArchiveMetadata`.

- [x] **ArchiveDevice** — immutable member value; контракт проверен.

  Проверенный контракт: `ArchiveDevice(path is ArchivePath, kind is ArchiveEntryKind, metadata is ArchiveMetadata, major is int, minor is int)`; `path() is ArchivePath`; `kind() is ArchiveEntryKind`; `metadata() is ArchiveMetadata`; `major() is int`; `minor() is int`.

- [x] Reject a device kind other than `character_device` or `block_device`.
- [x] Reject negative device numbers.
- [x] Keep every archive entry immutable.
- [x] Keep file data unavailable on non-file entries.
- [x] Keep symbolic-link and hard-link targets distinct.
- [x] Keep the noun role name because an entry is a domain value.

- [x] **ArchiveEntryKind / ArchiveEntryVisitor** — семь kind, шесть concrete callbacks;
  accept передаёт ровно одну запись, callback errors сохраняют тип, повторение допустимо.
- [x] **InvalidArchiveDevice** — стабильная ошибка kind/номеров.

**Проверка:** 10 внешних + 3 встроенных группы PASS, исполняемый visitor-пример.
Значения не выполняют I/O или extraction. Цели ссылок ограничены ArchivePath;
absolute/parent targets не представляются, resolution/containment остаются позже.

### `archive/limits` — VERIFIED (policy and incremental accounting)

[Контракт](archive-guide.md#archivelimits) ·
evidence ·
[пример](../../examples/std-v4/archive-limits.ok).

- [x] **ArchiveLimits** — положительные независимые пределы entries/entry_size/
  total_size/path_length; path проверяет длину до ArchivePath, budget выдаёт
  независимый неизменяемый бюджет. Размеры — expanded bytes, путь — code points.
- [x] **ArchiveLimitExceeded / InvalidArchiveLimit** — стабильные labels, исходный
  настроенный maximum и сообщения; неверные пределы отклоняются по порядку.
- [x] Reject every non-positive limit.
- [x] **ArchiveBudget.entered** проверяет entry count до admission и расходует
  слот даже для пустой/non-file записи; declared size проверяется до payload.
- [x] **ArchiveBudget.expanded** проверяет фактическую порцию до её сохранения;
  entry_size и total_size расходуются по фактическим байтам, не по заголовку.
- [x] Общий размер накопительный; completed не возвращает slots/total bytes,
  следующая запись получает только новый per-entry allowance.
- [x] Path length проверяется до конструирования ArchivePath; Unicode не меняется.
- [x] Проверки до вычитания, включая INT64_MAX; отрицательные sizes, неверная фаза,
  независимые ветки и неизменность старого бюджета после ошибки.
- [x] Комментарии, 14 внешних + 4 встроенных группы PASS, исполняемый пример.

Это примитивы политики и учёта, не готовый TAR/ZIP reader. Consumer-тест проверяет
проверку до append в ByteBuffer. Реальные парсеры обязаны удерживать successors,
проверять admission/allocation boundaries и actual expanded bytes; соответствие
header/body, raw input/metadata/temporary codec bounds проверяются в их разделах.

### `archive/file` — DEFERRED (user decision; no-follow capability still missing)

**Технический блокер:** read-only audit.
Текущий provider не предоставляет no-follow open и metadata открытого descriptor.
`lstat` до обычного открытия не гарантирует отсутствие подмены пути; эту проверку
нельзя считать выполнением требования ниже. При действующем запрете на правки
интерпретатора пункт остаётся открытым. Контракты `archive/stream` реализованы;
оставшиеся архивные задачи отложены пользователем. `net/failure` проверен;
`parallel/map` и `net/ip` проверены; текущая работа — `net/udp`.
HostName/HostEndpoint and pinned PSL are verified; full URL/HTTP integration follows.
Итог текущего прохода: 520 файлов / 8799 тестов; все новые тесты PASS, точное совпадение семи исходных failing files. Общий suite не green.

**Зависимости и gate:** bounded streams, explicit extraction command, path/link traversal protection, partial-output cleanup и resource limits.

- [ ] **ArchivedFile** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ArchivedFile(origin is FileEntry, path is ArchivePath, maximum_size is int)`; `result() is ArchiveFile`.

- [ ] Reject a non-positive maximum size.
- [ ] Read filesystem data only through `result()`.
- [ ] Reject a filesystem entry that is not a regular file.
- [ ] Reject a file larger than maximum size before reading when size is known.
- [ ] Enforce maximum size while reading when size changes concurrently.
- [ ] Read the file without following a symbolic link.
- [ ] Capture available mode and modification metadata.
- [ ] Do not hide filesystem I/O behind `ArchiveFile.data()`.
- [ ] Remove the legacy archive-local `FileEntry` and `DataEntry` objects.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `archive/stream` — VERIFIED (contracts and lifecycle adapters; remaining format gate DEFERRED)

[Контракт](archive-guide.md#archivestream) ·
evidence ·
[пример](../../examples/std-v4/archive-stream.ok).

- [x] **ReadingArchive** — consuming resource: entry/finished/close/closed;
  EndOfArchive после наблюдённого нормального EOF, finished не читает источник.
- [x] **WritingArchive** — write/finish/finished/close/closed; явные команды,
  успешная финализация предшествует finished=true; пустой архив допустим.
- [x] **EndOfArchive** — стабильная typed error: `text() == "end of archive"`.
- [x] **ArchiveReading / ArchiveWriting** — общая логика состояния поверх явно
  переданных синхронных operations, без вызовов при конструировании.
- [x] Make archive readings and writings changing resources.
- [x] Make `entry()` throw `EndOfArchive` after normal completion; repeat EOF is cached.
- [x] Make successful `finish()` set `finished() == true`.
- [x] Throw `InvalidState` when writing a finished archive.
- [x] Throw `InvalidState` when finishing an archive twice.
- [x] Один вызов next-entry operation на активный entry(), без prefetch и удержания
  результата адаптером; ошибки сохраняют тип и запрещают повторную обработку.
- [ ] Materialize at most one bounded entry at a time while reading — обязательный
  gate TarReading/ZipReading: ArchiveLimits должны проверяться до payload allocation.
  Общий callback-адаптер не может ограничить уже построенный результат задним числом.
  Для TarReading проверено; ZipReading остаётся открытым.
- [x] Keep blocking archive work out of cursor traversal.
- [x] Общие адаптеры оставляют lifecycle источника/приёмника вызывающей стороне
  при success/error/close; destroy освобождает только собственный lifecycle buffer.
- [x] Close идемпотентен, не финализирует архив; после close только close/closed.
- [x] Комментарии, 13 внешних + 3 встроенных группы PASS, исполняемый пример.

Встроенные тесты проверяют локальные роли: корневой runner не разрешает identity
импортированного ArchiveEntry в callback-сигнатуре. Concrete adapters проверены
обычными импортами в consumer suite; ограничение runner документировано в evidence.
Fakes/пример не являются wire codecs. Truncated formats, allocation bounds и
wire-level borrowed I/O проверяются также в конкретных форматах ниже.

### `archive/tar` — VERIFIED value and streaming APIs

[Контракты](archive-tar.md), [реализация](../../std/archive/tar.ok),
запись: RED/GREEN,
value-чтение: evidence,
потоковое чтение: RED/GREEN,
[потоковый пример](../../examples/std-v4/archive-tar-stream.ok).

**Зависимости и gate:** bounded streams, explicit extraction command, path/link traversal protection, partial-output cleanup и resource limits.

- [x] **TarArchive** — ленивое связывание Iterating<ArchiveEntry>, повторяемый
  bytes(), PAX по умолчанию, явный TarFormat.ustar; полный output в памяти.

  Кандидаты из v3 для пересмотра: `TarArchive(entries is Iterating<ArchiveEntry>)`; `TarArchive(entries is Iterating<ArchiveEntry>, format is TarFormat)`; `bytes() is @byte`.

- [x] **ParsedTar** — immutable bytes → независимый Sequencing<ArchiveEntry>; повторяемый разбор,
  ArchiveLimits до payload slice и отдельные бюджеты metadata/extensions.

  Кандидаты из v3 для пересмотра: `ParsedTar(origin is @byte, limits is ArchiveLimits)`; `entries() is Sequencing<ArchiveEntry>`.

- [x] **TarReading** — borrowed Reading, одна запись за вызов без prefetch;
  лимиты до body I/O, cumulative metadata/count/total, passive finished,
  cached normal EOF, poisoning при ошибке, close/destroy только своего состояния.

  Кандидаты из v3 для пересмотра: `TarReading(origin is Reading, limits is ArchiveLimits)`; `entry() is ArchiveEntry`; `finished() is bool`.

- [x] **TarWriting** — borrowed Writing; write одного member + padding,
  finish пишет два EOF-блока и flush ровно один раз; close/destroy только своё
  состояние; ошибки сохраняются и запрещают повтор; finished без I/O.

  Кандидаты из v3 для пересмотра: `TarWriting(origin is Writing)`; `TarWriting(origin is Writing, format is TarFormat)`; `write(entry is ArchiveEntry) is void`; `finish() is void`; `finished() is bool`.

- [x] **InvalidTar** — Error с диагностикой представимости полей, checksum,
  malformed header/extension, truncation, padding и EOF.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] **DuplicateArchivePath** — сохраняет ArchivePath; запись отклоняет точный
  дубль до первого байта повторной записи независимо от вида entry; ParsedTar
  и TarReading проверяют effective path после расширений до payload slice/read.

  Кандидаты из v3 для пересмотра: `DuplicateArchivePath(path is ArchivePath)`; `path() is ArchivePath`; `text() is string`.

- [x] Use PAX as the default writing format.
- [x] Validate every TAR header checksum, including PAX/GNU extension headers.
- [x] Parse POSIX.1-2001 PAX local/global headers, override/delete and byte framing.
- [x] Parse GNU long-name/long-link extensions and base-256 numeric fields.
- [x] Do not write GNU format by default.
- [x] Reject duplicate archive paths in writing and both readers.
- [x] Reject truncated entry data, missing/nonzero padding and incomplete EOF in both readers.
- [x] Preserve declared entry order in value and streaming APIs.
- [x] Support regular files and directories as written/parsed values.
- [x] Support symbolic and hard links as written/parsed values.
- [x] Support FIFOs and character/block devices as written/parsed values.
- [x] Test USTAR writing field-size boundaries (100/155/32 bytes, octal IDs/time).
- [x] Test PAX writing long paths and large numeric fields with an independent reader.
- [x] Verify value and streaming APIs interoperate.
- [x] Do not add compressed TAR variants.
- [ ] Compose compressed TAR through Stage 16 codecs.

Проверено: 12 внешних групп записи, 37 групп value-чтения, 25 групп потокового
чтения, 8 inline-групп, примеры записи, PAX round trip и consuming reader.
Input fixtures независимы от writer std.
Попутно исправлены integer bit projections PermissionMode в std/fs/stat.ok:
все 4096 комбинаций + inline проверены. Изменений interpreter/provider нет.
TarReading не удерживает предыдущие payload и не закрывает borrowed source;
возвращённые записи переживают close. ParsedTar получает
уже выделенный @byte; он не заявляет bounds на исходную аллокацию или overhead VM.
Payload admission — до копирования, metadata независимо ограничена по bytes/count.
Sparse/V7/не-UTF-8 names отвергаются; извлечения в filesystem нет. Compressed TAR
composition отложена вместе с алгоритмами Stage 16 по решению пользователя.

**Готовность TarReading:** CONTRACT → RED → std → GREEN/inline → пример выполнены.
Оставшиеся архивные задачи отложены; `net/failure` проверен, worker cleanup
для возврата к `parallel/map` исправлен отдельно.

### `archive/zip` — ZipCompression/ZipArchiveEntry VERIFIED; wire APIs DEFERRED

[Контракт выбора сжатия](archive-zip.md), [реализация](../../std/archive/zip.ok),
RED/GREEN,
[пример](../../examples/std-v4/archive-zip-compression.ok).
ZipArchiveEntry: evidence,
[пример](../../examples/std-v4/archive-zip-entry.ok).
Пока реализованы только immutable параметры записи, без codec/I/O. Алгоритмы
сжатия остаются DEFERRED; их наличие не следует из значения ZipMethod.

**Зависимости и gate:** bounded streams, explicit extraction command, path/link traversal protection, partial-output cleanup и resource limits.

- [x] **ZipMethod / ZipCompression** — выбор метода и проверенный effective level;
  no I/O/provider discovery; InvalidArgument при нарушении диапазона.

  Кандидаты из v3 для пересмотра: `ZipCompression(method is ZipMethod)`; `ZipCompression(method is ZipMethod, level is int)`; `method() is ZipMethod`; `level() is int`.

- [x] **ZipArchiveEntry** — immutable запись + явный ZipCompression;
  сохраняет зависимости без вызова их методов, I/O, visitor dispatch и lifecycle.
  5 внешних групп проверяют effect-free binding, visitor payload, независимые
  choices, все виды ArchiveEntry и nominal iteration.

  Кандидаты из v3 для пересмотра: `ZipArchiveEntry(entry is ArchiveEntry, compression is ZipCompression)`; `entry() is ArchiveEntry`; `compression() is ZipCompression`.

- [ ] **ZipArchive** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ZipArchive(entries is Iterating<ZipArchiveEntry>)`; `bytes() is @byte`.

- [ ] **ParsedZip** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ParsedZip(origin is @byte, limits is ArchiveLimits)`; `entries() is Sequencing<ZipArchiveEntry>`.

- [ ] **ZipReading** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ZipReading(origin is Reading, limits is ArchiveLimits)`; `entry() is ArchiveEntry`; `finished() is bool`.

- [ ] **ZipWriting** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ZipWriting(origin is Writing)`; `ZipWriting(origin is Writing, compression is ZipCompression)`; `write(entry is ArchiveEntry) is void`; `write(entry is ZipArchiveEntry) is void`; `finish() is void`; `finished() is bool`.

- [ ] **InvalidZip** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] **UnsupportedZipCompression** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `UnsupportedZipCompression(identifier is int)`; `identifier() is int`; `text() is string`.

- [ ] **EncryptedZipEntry** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `EncryptedZipEntry(path is ArchivePath)`; `path() is ArchivePath`; `text() is string`.

- [ ] Use DEFLATE as the default writing method.
- [x] Resolve a default level for every compression method: 0/6/9/0/3.
- [x] Reject explicit levels for stored and LZMA entries, including explicit zero.
- [x] Return level `0` for stored and LZMA entries (no configurable ZIP level).
- [x] Validate levels according to the selected compression method: deflated 0..9,
  bzip2 1..9, zstandard -131072..22; zstandard zero resolves to 3.
- [ ] Enable ZIP64 automatically when required.
- [ ] Write entry names as UTF-8.
- [ ] Honor the UTF-8 filename flag while reading.
- [ ] Decode unflagged legacy names as CP437.
- [ ] Never decode names through the process locale.
- [ ] Validate local and central-directory consistency.
- [ ] Validate every entry CRC-32.
- [ ] Reject duplicate archive paths.
- [ ] Reject encrypted entries in the first API.
- [ ] Reject multipart ZIP archives.
- [ ] Read Zstandard method identifiers `20` and `93`.
- [ ] Write only Zstandard method identifier `93`.
- [ ] Preserve declared entry order.
- [ ] Test stored, DEFLATE, bzip2, LZMA and Zstandard entries.
- [ ] Test mixed compression methods in one archive.
- [ ] Test data descriptors and non-seekable writing.
- [ ] Test ZIP64 size and entry-count boundaries.
- [ ] Verify value and streaming APIs interoperate.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `archive/extract` — DEFERRED (user decision)

**Зависимости и gate:** bounded streams, explicit extraction command, path/link traversal protection, partial-output cleanup и resource limits.

- [ ] **ExtractedArchiveEntry** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ExtractedArchiveEntry(origin is ArchiveEntry, destination is Directory)`; `result() is Path`.

- [ ] **UnsupportedArchiveEntry** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `UnsupportedArchiveEntry(kind is ArchiveEntryKind)`; `kind() is ArchiveEntryKind`; `text() is string`.

- [ ] **ExistingArchiveDestination** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ExistingArchiveDestination(path is Path)`; `path() is Path`; `text() is string`.

- [ ] Extract regular files.
- [ ] Extract directories.
- [ ] Reject symbolic links.
- [ ] Reject hard links.
- [ ] Reject FIFOs.
- [ ] Reject character devices.
- [ ] Reject block devices.
- [ ] Never overwrite an existing destination.
- [ ] Resolve every entry relative to the supplied directory.
- [ ] Use directory-relative no-follow filesystem operations.
- [ ] Reject a symlink in any existing destination path component.
- [ ] Reject platform-specific device and reserved names.
- [ ] Reject case-folding path collisions on case-insensitive filesystems.
- [ ] Create each file with a safe initial mode.
- [ ] Do not restore archive ownership.
- [ ] Do not restore special permission bits.
- [ ] Remove a partially created file after a failed write.
- [ ] Limit failure effects to the current entry.
- [ ] Leave the destination directory lifecycle under caller control.
- [ ] Do not add batch extraction.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/secret` — opaque ownership/export VERIFIED; comparison/crypto APIs open

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [x] **SecretBytes** — [secret](secret-bytes.md)/[scoped auth](http-auth.md), focused/actual HTTPS plus full 672/12185 PASS.

  Кандидаты из v3 для пересмотра: `SecretBytes(origin is @byte)`; `size() is int`.

- [x] **ExportedSecret** — [secret](secret-bytes.md)/[scoped auth](http-auth.md), focused/actual HTTPS plus full 672/12185 PASS.

  Кандидаты из v3 для пересмотра: `ExportedSecret(origin is SecretBytes)`; `bytes() is @byte`.

- [x] Keep secret contents unavailable through ordinary observations.
- [x] Do not add `bytes()`, `string()` or `text()` to `SecretBytes`.
- [x] Do not make `SecretBytes` a `ByteSource`.
- [x] Do not stream secrets through `<-`.
- [x] Do not add ordinary equality for secrets.
- [ ] Require `ConstantTimeEqual` for exported-secret comparison.
- [x] Make export explicit through `ExportedSecret`.
- [x] Document that a garbage-collected runtime cannot guarantee complete zeroization.
- [ ] Do not duplicate `SecureRandom` from Stage 09.
- [ ] Require the concrete `SecureRandom` object for cryptographic generation.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/kdf` — DEFERRED (host crypto provider)

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **Hkdf** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Hkdf(secret is SecretBytes, salt is @byte, info is @byte, size is int, algorithm is KdfAlgorithm)`; `secret() is SecretBytes`.

- [ ] **Pbkdf2** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Pbkdf2(password is SecretBytes, salt is @byte, iterations is int, size is int, algorithm is KdfAlgorithm)`; `secret() is SecretBytes`.

- [ ] **ScryptParameters** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ScryptParameters(cost is int, block_size is int, parallelism is int, size is int)`; `cost() is int`; `block_size() is int`; `parallelism() is int`; `size() is int`.

- [ ] **Scrypt** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Scrypt(password is SecretBytes, salt is @byte, parameters is ScryptParameters)`; `secret() is SecretBytes`.

- [ ] **Argon2idParameters** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Argon2idParameters(memory is int, iterations is int, parallelism is int, size is int)`; `memory() is int`; `iterations() is int`; `parallelism() is int`; `size() is int`.

- [ ] **RecommendedArgon2id** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `#projected value() is Argon2idParameters`.

- [ ] **Argon2id** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Argon2id(password is SecretBytes, salt is @byte, parameters is Argon2idParameters)`; `secret() is SecretBytes`.

- [ ] **InvalidKdfParameters** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Reject MD5 and SHA-1 as KDF algorithms.
- [ ] Reject a non-positive derived size.
- [ ] Enforce the RFC 5869 HKDF output-size bound.
- [ ] Permit an empty HKDF salt and info value.
- [ ] Reject a non-positive PBKDF2 iteration count.
- [ ] Document PBKDF2 as legacy and protocol interoperability.
- [ ] Require scrypt cost greater than one and a power of two.
- [ ] Reject non-positive scrypt block size and parallelism.
- [ ] Reject overflowing scrypt memory calculations before allocation.
- [ ] Reject non-positive Argon2id memory, iterations and parallelism.
- [ ] Require an Argon2id memory value compatible with parallelism.
- [ ] Define `RecommendedArgon2id` as 64 MiB, three iterations and four lanes.
- [ ] Use a 32-byte output in `RecommendedArgon2id`.
- [ ] Add RFC vectors for HKDF, PBKDF2, scrypt and Argon2id.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/password` — DEFERRED (host crypto provider)

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **Argon2idPassword** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Argon2idPassword(password is SecretBytes, random is SecureRandom)`; `Argon2idPassword(password is SecretBytes, random is SecureRandom, parameters is Argon2idParameters)`; `string() is string`.

- [ ] **VerifiedPassword** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `VerifiedPassword(password is SecretBytes, encoded is string)`; `#projected value() is bool`.

- [ ] **InvalidPasswordHash** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] **PasswordHashLimitExceeded** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Encode Argon2id password hashes in PHC string format.
- [ ] Generate a fresh 16-byte salt through `SecureRandom`.
- [ ] Use `RecommendedArgon2id` in the constructor without explicit parameters.
- [ ] Store algorithm version and all parameters in the encoded value.
- [ ] Parse and bound memory before allocating it during verification.
- [ ] Parse and bound iterations and parallelism before computing.
- [ ] Compare the computed tag in constant time.
- [ ] Return false for an incorrect password.
- [ ] Throw `InvalidPasswordHash` for malformed encoding.
- [ ] Throw `PasswordHashLimitExceeded` for valid but unacceptable resource parameters.
- [ ] Do not expose whether salt or tag caused verification failure.
- [ ] Do not make PBKDF2 the default password-storage format.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/nonce` — DEFERRED (host crypto provider)

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **AeadNonce** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AeadNonce(origin is @byte)`; `bytes() is @byte`.

- [ ] **NonceSequence** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `NonceSequence(prefix is @byte, initial is BigInt)`; `nonce() is AeadNonce`; `exhausted() is bool`.

- [ ] **NonceSequenceExhausted** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Require every AEAD nonce to contain exactly 12 bytes.
- [ ] Require a four-byte sequence prefix.
- [ ] Require the initial counter to fit unsigned 64-bit range.
- [ ] Construct nonce bytes from prefix followed by a big-endian counter.
- [ ] Advance the sequence exactly once per successful `nonce()`.
- [ ] Throw `NonceSequenceExhausted` after the maximum counter.
- [ ] Never wrap a nonce counter.
- [ ] Document that a prefix must be unique for every key and sequence.
- [ ] Do not provide a random-nonce default that pretends to guarantee uniqueness.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/aead` — DEFERRED (host crypto provider)

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **AuthenticationFailed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Use the same authentication failure for wrong key, nonce, AAD, ciphertext and tag.
- [ ] Do not reveal which authenticated input was incorrect.
- [ ] Authenticate before exposing plaintext.
- [ ] Keep AEAD operations bounded to in-memory byte arrays.
- [ ] Do not add unauthenticated streaming plaintext output.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/aes-gcm` — DEFERRED (host crypto provider)

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **AesGcmKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AesGcmKey(origin is SecretBytes)`.

- [ ] **GeneratedAesGcmKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `GeneratedAesGcmKey(random is SecureRandom, size is int)`; `result() is AesGcmKey`.

- [ ] **AesGcmSealed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AesGcmSealed(nonce is AeadNonce, ciphertext is @byte, tag is @byte)`; `nonce() is AeadNonce`; `ciphertext() is @byte`; `tag() is @byte`.

- [ ] **AesGcmEncryption** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AesGcmEncryption(key is AesGcmKey, nonce is AeadNonce, plaintext is @byte, associated is @byte)`; `result() is AesGcmSealed`.

- [ ] **AesGcmDecryption** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AesGcmDecryption(key is AesGcmKey, sealed is AesGcmSealed, associated is @byte)`; `result() is @byte`.

- [ ] Accept AES-GCM keys of 16, 24 or 32 bytes only.
- [ ] Generate AES-GCM keys of 16, 24 or 32 bytes only.
- [ ] Require a full 16-byte authentication tag.
- [ ] Reject truncated GCM tags.
- [ ] Permit empty plaintext and associated data.
- [ ] Add NIST AES-GCM vectors for every supported key size.
- [ ] Test ciphertext, nonce, AAD and tag corruption independently.
- [ ] Document catastrophic nonce reuse under the same key.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/chacha20-poly1305` — DEFERRED (host crypto provider)

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **ChaCha20Poly1305Key** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ChaCha20Poly1305Key(origin is SecretBytes)`.

- [ ] **GeneratedChaCha20Poly1305Key** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `GeneratedChaCha20Poly1305Key(random is SecureRandom)`; `result() is ChaCha20Poly1305Key`.

- [ ] **ChaCha20Poly1305Sealed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ChaCha20Poly1305Sealed(nonce is AeadNonce, ciphertext is @byte, tag is @byte)`; `nonce() is AeadNonce`; `ciphertext() is @byte`; `tag() is @byte`.

- [ ] **ChaCha20Poly1305Encryption** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ChaCha20Poly1305Encryption(key is ChaCha20Poly1305Key, nonce is AeadNonce, plaintext is @byte, associated is @byte)`; `result() is ChaCha20Poly1305Sealed`.

- [ ] **ChaCha20Poly1305Decryption** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ChaCha20Poly1305Decryption(key is ChaCha20Poly1305Key, sealed is ChaCha20Poly1305Sealed, associated is @byte)`; `result() is @byte`.

- [ ] Require exactly 32 key bytes.
- [ ] Require a full 16-byte authentication tag.
- [ ] Permit empty plaintext and associated data.
- [ ] Implement the RFC 8439 construction.
- [ ] Add every RFC 8439 AEAD vector.
- [ ] Test ciphertext, nonce, AAD and tag corruption independently.
- [ ] Document catastrophic nonce reuse under the same key.
- [ ] Do not expose unauthenticated raw ChaCha20.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/key` — PublicKey SPKI role VERIFIED; PrivateKey DEFERRED

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **PrivateKey** — определить сущность и контракт до реализации.
- [x] **PublicKey** — noun role exposing complete DER SubjectPublicKeyInfo bytes;
  real certificate-backed use is covered by 436. No private-key capability.
- [ ] Keep private key material unavailable through ordinary observations.
- [x] Permit public keys to expose standardized public encodings (SPKI).
- [x] Keep noun role names because these are domain categories.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/ed25519` — DEFERRED (host crypto provider)

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **Ed25519PrivateKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Ed25519PrivateKey(seed is SecretBytes)`; `public_key() is Ed25519PublicKey`.

- [ ] **Ed25519PublicKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Ed25519PublicKey(origin is @byte)`; `bytes() is @byte`.

- [ ] **GeneratedEd25519Key** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `GeneratedEd25519Key(random is SecureRandom)`; `result() is Ed25519PrivateKey`.

- [ ] **Ed25519Signature** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Ed25519Signature(origin is @byte)`; `bytes() is @byte`.

- [ ] **Ed25519Signed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Ed25519Signed(key is Ed25519PrivateKey, message is @byte)`; `result() is Ed25519Signature`.

- [ ] **VerifiedEd25519** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `VerifiedEd25519(key is Ed25519PublicKey, message is @byte, signature is Ed25519Signature)`; `#projected value() is bool`.

- [ ] **InvalidEd25519Key** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] **InvalidEd25519Signature** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Require a 32-byte Ed25519 seed.
- [ ] Require a 32-byte encoded public key.
- [ ] Require a 64-byte encoded signature.
- [ ] Validate imported public points.
- [ ] Reject non-canonical signatures.
- [ ] Add RFC 8032 generation, signing and verification vectors.
- [ ] Return false for a well-formed but incorrect signature.
- [ ] Keep signature parsing distinct from signing.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/x25519` — DEFERRED (host crypto provider)

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **X25519PrivateKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `X25519PrivateKey(secret is SecretBytes)`; `public_key() is X25519PublicKey`.

- [ ] **X25519PublicKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `X25519PublicKey(origin is @byte)`; `bytes() is @byte`.

- [ ] **GeneratedX25519Key** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `GeneratedX25519Key(random is SecureRandom)`; `result() is X25519PrivateKey`.

- [ ] **X25519SharedSecret** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `X25519SharedSecret(private is X25519PrivateKey, public is X25519PublicKey)`; `secret() is SecretBytes`.

- [ ] **InvalidX25519Key** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] **InvalidX25519SharedSecret** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Require 32-byte private and public inputs.
- [ ] Validate imported public keys.
- [ ] Reject an all-zero shared secret.
- [ ] Add RFC 7748 key agreement vectors.
- [ ] Require HKDF before using a shared secret as an application key.
- [ ] Do not expose a generic unspecified ECDH key.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/ecdsa` — DEFERRED (host crypto provider)

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **EcdsaPrivateKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `EcdsaPrivateKey(secret is SecretBytes, curve is EcdsaCurve)`; `curve() is EcdsaCurve`; `public_key() is EcdsaPublicKey`.

- [ ] **EcdsaPublicKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `EcdsaPublicKey(origin is @byte, curve is EcdsaCurve)`; `curve() is EcdsaCurve`; `bytes() is @byte`.

- [ ] **GeneratedEcdsaKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `GeneratedEcdsaKey(random is SecureRandom, curve is EcdsaCurve)`; `result() is EcdsaPrivateKey`.

- [ ] **EcdsaSignature** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `EcdsaSignature(r is BigInt, s is BigInt)`; `r() is BigInt`; `s() is BigInt`.

- [ ] **EcdsaSigned** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `EcdsaSigned(key is EcdsaPrivateKey, message is @byte)`; `result() is EcdsaSignature`.

- [ ] **VerifiedEcdsa** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `VerifiedEcdsa(key is EcdsaPublicKey, message is @byte, signature is EcdsaSignature)`; `#projected value() is bool`.

- [ ] **DerEcdsaSignature** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `DerEcdsaSignature(origin is EcdsaSignature)`; `bytes() is @byte`.

- [ ] **EcdsaSignatureFromDer** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `EcdsaSignatureFromDer(origin is @byte)`; `#projected value() is EcdsaSignature`.

- [ ] **InvalidEcdsaKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] **InvalidEcdsaSignature** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Map P-256 to SHA-256.
- [ ] Map P-384 to SHA-384.
- [ ] Map P-521 to SHA-512.
- [ ] Sign messages rather than caller-supplied digests.
- [ ] Generate signing nonces deterministically according to RFC 6979.
- [ ] Normalize generated signatures to low-S form.
- [ ] Reject out-of-range `r` and `s` values.
- [ ] Validate imported curve points.
- [ ] Require canonical DER integers and lengths.
- [ ] Add NIST and RFC 6979 vectors for every curve.
- [ ] Return false for a well-formed but incorrect signature.
- [ ] Do not add custom curves in the first API.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/rsa` — DEFERRED (host crypto provider)

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **RsaPrivateKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `public_key() is RsaPublicKey`; `bits() is int`.

- [ ] **RsaPublicKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `RsaPublicKey(modulus is BigInt, exponent is int)`; `modulus() is BigInt`; `exponent() is int`; `bits() is int`.

- [ ] **GeneratedRsaKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `GeneratedRsaKey(random is SecureRandom)`; `GeneratedRsaKey(random is SecureRandom, bits is int)`; `result() is RsaPrivateKey`.

- [ ] **RsaPssSignature** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `RsaPssSignature(origin is @byte)`; `bytes() is @byte`.

- [ ] **RsaPssSigned** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `RsaPssSigned(key is RsaPrivateKey, message is @byte)`; `RsaPssSigned(key is RsaPrivateKey, message is @byte, hash is RsaHash)`; `result() is RsaPssSignature`.

- [ ] **VerifiedRsaPss** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `VerifiedRsaPss(key is RsaPublicKey, message is @byte, signature is RsaPssSignature)`; `VerifiedRsaPss(key is RsaPublicKey, message is @byte, signature is RsaPssSignature, hash is RsaHash)`; `#projected value() is bool`.

- [ ] **RsaOaepCiphertext** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `RsaOaepCiphertext(origin is @byte)`; `bytes() is @byte`.

- [ ] **RsaOaepEncrypted** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `RsaOaepEncrypted(key is RsaPublicKey, plaintext is SecretBytes, label is @byte, random is SecureRandom)`; `RsaOaepEncrypted(key is RsaPublicKey, plaintext is SecretBytes, label is @byte, random is SecureRandom, hash is RsaHash)`; `result() is RsaOaepCiphertext`.

- [ ] **RsaOaepDecrypted** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `RsaOaepDecrypted(key is RsaPrivateKey, ciphertext is RsaOaepCiphertext, label is @byte)`; `RsaOaepDecrypted(key is RsaPrivateKey, ciphertext is RsaOaepCiphertext, label is @byte, hash is RsaHash)`; `secret() is SecretBytes`.

- [ ] **InvalidRsaKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] **InvalidRsaSignature** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] **RsaDecryptionFailed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Generate 3072-bit RSA keys by default.
- [ ] Reject RSA keys smaller than 2048 bits.
- [ ] Require public exponent 65537 for generated keys.
- [ ] Validate imported modulus and exponent values.
- [ ] Use SHA-256 by default for PSS and OAEP.
- [ ] Use MGF1 with the selected hash.
- [ ] Use PSS salt length equal to digest length.
- [ ] Enforce the OAEP maximum plaintext size before encryption.
- [ ] Return one indistinguishable error for OAEP decryption failures.
- [ ] Document OAEP as small key-material transport only.
- [ ] Add RFC 8017 interoperability vectors.
- [ ] Do not expose raw RSA operations.
- [ ] Do not expose PKCS#1 v1.5 encryption.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/key-format` — DEFERRED (host crypto provider)

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [ ] **UnencryptedPkcs8** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `UnencryptedPkcs8(key is PrivateKey)`; `bytes() is @byte`.

- [ ] **EncryptedPkcs8** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `EncryptedPkcs8(key is PrivateKey, password is SecretBytes, random is SecureRandom)`; `result() is @byte`.

- [ ] **ParsedPrivateKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ParsedPrivateKey(origin is @byte)`; `#projected value() is PrivateKey`.

- [ ] **DecryptedPrivateKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `DecryptedPrivateKey(origin is @byte, password is SecretBytes)`; `#projected value() is PrivateKey`.

- [ ] **SubjectPublicKeyInfo** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SubjectPublicKeyInfo(key is PublicKey)`; `bytes() is @byte`.

- [ ] **ParsedPublicKey** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ParsedPublicKey(origin is @byte)`; `#projected value() is PublicKey`.

- [ ] **InvalidKeyEncoding** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Export private keys only through PKCS#8.
- [ ] Make unencrypted private-key export explicit in the object name.
- [ ] Encrypt PKCS#8 through PBES2 with PBKDF2-HMAC-SHA256 and AES-256-GCM.
- [ ] Generate fresh encryption salt and nonce through `SecureRandom`.
- [ ] Reject unsupported key algorithms.
- [ ] Reject trailing DER data.
- [ ] Require canonical DER encoding.
- [ ] Compose PEM through `PemEncoded` and `PemBlocks`.
- [ ] Keep the DER codec internal and schema-specific.
- [ ] Do not expose a generic public ASN.1 value model.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/x509/name` — grouped metadata VERIFIED; host DER integration NEXT

[Grouped value contract](x509-name.md): 18 focused name groups / 25 with
validity/constraint compatibility, inline/example PASS. Explicit RDN groups
replace the withdrawn flat v3 candidate; no lossy hash/display reconstruction.

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [x] **NameAttribute** — canonical dotted OID and exact bounded decoded text.

  Кандидаты из v3 для пересмотра: `NameAttribute(identifier is string, value is string)`; `identifier() is string`; `value() is string`.

- [x] **DistinguishedName** — ordered snapshots of RelativeDistinguishedName groups.

  Кандидаты из v3 для пересмотра: `DistinguishedName(attributes is Sequencing<NameAttribute>)`; `attributes() is Sequencing<NameAttribute>`.

- [x] Preserve repeated name attributes.
- [x] Preserve relative-distinguished-name ordering.
- [x] Represent attribute identifiers as dotted object identifiers.
- [x] Do not flatten names into a lossy display string.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/x509/certificate` — metadata/DER/SPKI/extensions implemented; verification NEXT for TLS

[Pure values](x509-values.md), implemented in crypto/x509/value: 7 focused groups,
inline/example PASS. Validity is wall-clock metadata; path depth distinguishes
absence and zero. [DER certificate parsing](x509-certificate.md) now retains original
RDN groups, exact epochs and host-decoded SPKI. Focused 434/435/436/native 36:
4 files / 74 assertions PASS; inline/example PASS. Actual default CA bundle:
191/191 parses, without applying issuance-profile serial/time-choice restrictions
to metadata reading. Full runtime gate: 619 files / 11 691 assertions PASS,
through 436/native 36; later isolated prototypes are excluded.
[SAN/supplied chain and explicit roots](certificate-observers.md) now have 109
focused compatibility assertions plus inline/example PASS; full native gate:
625 files / 11 725 assertions PASS through 440/native 38.
[BC/KU/EKU/unknown extension metadata](certificate-extensions.md) now has 90
focused compatibility assertions plus inline/example PASS; full native gate:
627 files / 11 753 assertions PASS through 441/native 39. Explicit system acquisition
and explicit path verification are implemented below;
metadata grants no critical-extension acceptance.

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [x] **CertificateValidity** — exact inclusive wall-clock interval, ordered endpoints.

  Кандидаты из v3 для пересмотра: `CertificateValidity(starts is Instant, ends is Instant)`; `starts() is Instant`; `ends() is Instant`.

- [x] **Certificate** — immutable exact DER/deterministic facts; subject/issuer,
  validity and PublicKey observers. No current-time/hostname/trust assertion.

  Кандидаты из v3 для пересмотра: `der() is @byte`; `subject() is DistinguishedName`; `issuer() is DistinguishedName`; `validity() is CertificateValidity`; `public_key() is PublicKey`.

- [x] **ParsedCertificate** — bounded DER constructor projection with typed failure.

  Кандидаты из v3 для пересмотра: `ParsedCertificate(origin is @byte)`; `#projected value() is Certificate`.

- [x] **CertificateNames** — bounded SAN-only DNS strings and nominal IPs; no
  CN fallback. v4 ips() is Sequencing<IpAddress>, replacing the string candidate.

  Кандидаты из v3 для пересмотра: `CertificateNames(origin is Certificate)`; `dns() is Sequencing<string>`; `ips() is Sequencing<string>`.

- [x] **CertificateConstraints** — CA flag and explicit absent/present nonnegative depth.

  Кандидаты из v3 для пересмотра: `CertificateConstraints(authority is bool, limited is bool, maximum_depth is int)`; `authority() is bool`; `limited() is bool`; `maximum_depth() is int`.

- [x] **MissingCertificateDepth** — typed absent-depth observation.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] **CertificateExtensions** — bounded BC/KU/EKU metadata, critical OIDs and
  exact unknown payload entities/lookup; no verification claim.
- [x] **CertificateExtension** — immutable identifier/critical/exact payload metadata.
- [x] **CertificatePurpose** — canonical OID identity, preserving unknown EKU values.
- [x] **CertificateKeyUsage** — nine standard named bits with canonical wire interpretation.

  Кандидаты из v3 для пересмотра: `CertificateExtensions(origin is Certificate)`; `constraints() is CertificateConstraints`; `key_usages() is Set<CertificateKeyUsage>`; `purposes() is Set<CertificatePurpose>`; `critical() is Sequencing<string>`.

- [x] **CertificateChain** — immutable supplied leaf-first untrusted snapshot;
  1..32 certificates/8 MiB DER, source size/items sampled once, repetitions retained.

  Кандидаты из v3 для пересмотра: `CertificateChain(certificates is Sequencing<Certificate>)`; `certificates() is Sequencing<Certificate>`.

- [x] **InvalidCertificate** — reason-bearing metadata validation failure; parser will reuse it.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] Make `maximum_depth()` throw `MissingCertificateDepth` when `limited() == false`.
- [x] Reject an empty certificate chain.
- [x] Reject trailing DER data.
- [ ] Require canonical DER encoding.
- [x] Preserve unknown extensions by identifier, including critical flags/raw bytes.
- [x] Reject duplicate extensions forbidden by X.509.
- [x] Keep DNS names separate from IP address names.
- [x] Do not fall back to common name (even when SAN is absent).
- [x] Do not add `Certificate.valid()` or `Certificate.matches()`.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `crypto/x509/trust` — explicit/system roots and TLS-purpose verification implemented

[Explicit snapshots](certificate-observers.md): TrustStore, TrustedCertificates
and strict CERTIFICATE-only PemTrustStore; empty roots never enable defaults.
Focused 440 and inline/example PASS; full runtime gate with SAN/bulk PEM:
625 files / 11 725 assertions PASS, through 440/native 38.
Remaining startup cost is open.
[System acquisition](system-trust-store.md): effect-free request, explicit
result() reads bounded actual OpenSSL sources; 47 focused compatibility assertions
and inline/actual-host example PASS (191 roots). Full native gate:
629 files / 11,774 assertions PASS through 442/native 40.
Native OS stores/auxiliary trust remain outside this profile.
[Explicit verification](certificate-verification.md): immutable request, nominal
DNS/IP identity, exact inclusive Instant and serverAuth/clientAuth profile.
Final focused compatibility: 97 assertions PASS plus inline/example; full gate:
631 files / 11,826 assertions PASS through 443/native 41. Partial anchors, revocation, arbitrary policy
processing and configurable algorithm levels remain outside this first profile.

**Зависимости и gate:** проверенный provider, key/entropy capability, known-answer vectors, rejected malformed inputs, no accidental secrets output.

- [x] **TrustStore** — noun role exposing explicit immutable certificate roots.
- [x] **TrustedCertificates** — bounded snapshots, 0..1024/16 MiB DER; empty means no anchors.

  Кандидаты из v3 для пересмотра: `certificates() is Sequencing<Certificate>`.

- [x] **SystemTrustStore** — explicit result() acquires bounded immutable OpenSSL
  root snapshots; typed SystemTrustUnavailable/TrustStoreLimitExceeded failures.

  Кандидаты из v3 для пересмотра: `result() is TrustStore`.

- [x] **PemTrustStore** — strict public CERTIFICATE-only source; no filesystem effect,
  <=8 MiB input, explicit empty selection and typed failures. v4 named argument source.

  Кандидаты из v3 для пересмотра: `PemTrustStore(origin is string)`; `certificates() is Sequencing<Certificate>`.

- [x] **VerifiedCertificateChain** — explicit bounded request; result() returns
  verified leaf and path() returns actual selected leaf-to-anchor sequence.

  v4: host is CertificateIdentity; DnsCertificateIdentity(HostName) and
  IpCertificateIdentity(IpAddress) preserve nominal selection. The v3 raw-string
  host signature is withdrawn; constructor dispatch is unchanged.

- [x] **CertificateVerificationFailed** — closed CertificateFailure reason enum
  and fixed diagnostic text; UnsupportedCertificatePurpose rejects unknown requests.

  Кандидаты из v3 для пересмотра: `CertificateVerificationFailed(reason is CertificateFailure)`; `reason() is CertificateFailure`; `text() is string`.

- [x] Read system roots only through `SystemTrustStore.result()`.
- [x] Snapshot system roots into an immutable trust store.
- [x] Build and verify a chain to a trusted root (complete self-signed-anchor profile).
- [x] Verify every selected path certificate signature, including self-signed anchor.
- [x] Verify validity at the explicit `Instant`, exact/inclusive, with stated anchor policy.
- [x] Enforce basic constraints and maximum path depth.
- [x] Enforce key usage and extended key usage.
- [x] Reject unsupported critical extensions.
- [x] Verify DNS names according to IDNA-aware hostname rules and pinned suffix policy.
- [x] Verify IP addresses only against IP subjectAltName entries.
- [x] Keep hostname verification inside chain verification.
- [x] Document that revocation checking is outside the first API.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `parallel/map` — VERIFIED

[Контракт](parallel-map.md), [модуль](../../std/parallel/map.ok),
TDD и реальные process-пробы,
[пример](../../examples/std-v4/parallel-map.ok).
Ограниченные пакеты synchronous typed bound workers; FutureScope + AllFutures
ожидают cleanup и сохраняют порядок. Исправления предпосылок runtime документированы
отдельно: cleanup,
generic result type.
Произвольные closures/объекты не переносятся; ошибки worker сохраняют текущую
проверенную границу spawn diagnostics, а не восстанавливаются как child Error.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **ParallelMapped<T, R>** — ограниченное упорядоченное отображение; workers() и async results().

  Кандидаты из v3 для пересмотра: `ParallelMapped(origins is Sequencing<T>, action is (T) is R)`; `ParallelMapped(origins is Sequencing<T>, action is (T) is R, workers is int)`; `async results() is Sequencing<R>`.

- [x] **InvalidWorkerCount** — сохраняет workers(), стабильный text(), отклоняет <= 0.

  Кандидаты из v3 для пересмотра: `InvalidWorkerCount(workers is int)`; `workers() is int`; `text() is string`.

- [x] Execute actions through the existing isolated process-worker path.
- [x] Preserve input order in results.
- [x] Bound active workers.
- [x] Stop assigning new inputs after the first failure.
- [x] Terminate or cancel active workers after failure or parent cancellation.
- [x] Await worker cleanup before returning or failing.
- [x] Validate every argument, result and failure crossing the process boundary.
- [x] Reject non-positive worker counts.
- [x] Choose a runtime-informed default worker count.
- [x] Do not add shared-memory threads in the first API.
- [x] Do not call process work `async`; it is parallel work awaited asynchronously.

**Готовность:** CONTRACT → RED → std → 15 внешних групп + 1 inline PASS → process-пример.
Полная регрессия отдельного runtime-исправления — в его отчёте. Следующий пункт — net/ip.

### `net/failure` — VERIFIED

**Зависимости и gate:** адреса → DNS → bounded streams → cancellation/ownership; loopback/fake resolver/clock и переносимые failures.

[Контракт](network-failure.md), [реализация](../../std/net/failure.ok),
RED/GREEN,
[пример](../../examples/std-v4/network-failure.ok).

- [x] **NetworkFailure** — добавлены operation()/kind(); прежний конструктор,
  enum members и точный text() сохранены.

  Кандидаты из v3 для пересмотра: `operation() is NetworkOperation`; `kind() is NetworkFailureKind`.

- [x] **NetworkError** — явный adapter NativeFailure + NetworkOperation;
  applicable() проверяет terminal core.net scope; value() возвращает Error.
- [x] Translate stable native failure categories into `NetworkFailureKind`.
- [x] Preserve the attempted operation separately from the failure kind.
- [x] Keep human-readable native detail in `text()`.
- [x] Do not expose platform `errno`, Winsock codes or TLS-library codes.
- [x] Let Stage 19 cancellation remain `Cancelled`, not `NetworkFailure`.
- [x] Let Stage 19 timeout remain `TimedOut`, not a socket-specific timeout object.

Проверено: 31 внешняя группа + 2 inline, пример на реальном unsupported core.net
исходе без открытия сокета. Не распознаём причины по тексту; объединённые provider
категории не разделяем догадками. would_block/interrupted и чужие domains остаются
для bare throw в исходном catch; value() при applicable=false даёт InvalidArgument.
TCP/DNS/poll остаются отдельными задачами. Interpreter/provider не менялись.

**Готовность:** CONTRACT → RED → std → GREEN/inline → пример выполнены.

### `net/ip` — VERIFIED

**Зависимости и gate:** адреса → DNS → bounded streams → cancellation/ownership; loopback/fake resolver/clock и переносимые failures.

[Числовой адрес](ip-address.md) и [классификация IANA](ip-properties.md) реализованы.
36 value groups и
76 property assertions PASS;
Inline coverage: net/ip — 5 groups, private registry — 2, numeric provider — 1.
[CIDR-сеть](ip-network.md) также проверена: 18 групп, все 33/129 допустимых
префикса, exact BigInt size и inclusive membership.
Evidence.
[Ленивый обход](ip-addresses.md): 11 focused assertions и 4 inline groups PASS;
evidence.
[IpInterface](ip-interface.md): 11 focused assertions и 5 inline groups PASS;
evidence. Следующий раздел — net/endpoint.

- [x] **IpAddress** — строгий numeric value, canonical text/bytes/version/order и pinned properties().

  Кандидаты из v3 для пересмотра: `IpAddress(origin is string)`; `version() is IpVersion`; `bytes() is @byte`; `properties() is Set<IpProperty>`; `string() is string`; `order(other is IpAddress) is Order`.

- [x] **IpNetwork** — нормализованный inclusive CIDR interval и точный BigInt size().

  Кандидаты из v3 для пересмотра: `IpNetwork(address is IpAddress, prefix is int)`; `network() is IpAddress`; `broadcast() is IpAddress`; `prefix() is int`; `membership(address is IpAddress) is bool`; `size() is BigInt`.

- [x] **IpAddresses** — ленивый inclusive Iterating с независимыми immutable cursors.

  Кандидаты из v3 для пересмотра: `IpAddresses(origin is IpNetwork)`; `cursor() is Cursor<IpAddress>`.

- [x] **IpInterface** — immutable host/prefix, сохраняет host bits и наблюдает IpNetwork.

  Кандидаты из v3 для пересмотра: `IpInterface(address is IpAddress, prefix is int)`; `address() is IpAddress`; `network() is IpNetwork`; `string() is string`.

- [x] **InvalidIpAddress** — сохраняет исходную строку в typed text() diagnostics.

  Кандидаты из v3 для пересмотра: `InvalidIpAddress(origin is string)`; `text() is string`.

- [x] **InvalidIpPrefix** — исходный prefix() и family-specific text().

  Кандидаты из v3 для пересмотра: `InvalidIpPrefix(prefix is int, version is IpVersion)`; `prefix() is int`; `text() is string`.

- [x] Parse only numeric addresses; never perform DNS from `IpAddress`.
- [x] Emit canonical compressed lowercase IPv6 text.
- [x] Compare addresses first by version and then by unsigned packed bytes.
- [x] Validate prefixes in `0..32` for IPv4 and `0..128` for IPv6.
- [x] Mask host bits when constructing `IpNetwork`.
- [x] Define `broadcast()` as the final address for both IP versions.
- [x] Iterate network addresses lazily.
- [x] Keep IPv6 cardinality in `BigInt`.
- [x] Classify special-use ranges from versioned IANA data.
- [x] Do not attach a port or IPv6 zone to `IpAddress`.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `net/endpoint` — numeric/IDNA host endpoints VERIFIED

[Numeric endpoint contract](ip-endpoint.md): 34 focused assertions, 3 inline
groups and runnable example PASS; evidence.
HostName/HostEndpoint теперь реализованы через проверенный strict nontransitional
UTS46 backend. PublicSuffixList закрепляет 10336 ICANN/PRIVATE rules offline.
[Контракт](host-policy.md), evidence.
Focused: 4 files / 177 groups PASS; prior numeric endpoint suite 34 groups PASS.
Runtime regression with real loopback: 588 files / 10921 groups PASS.
Four endpoint inline tests, one PSL inline test and example PASS.
Port zero representable; запрет в connect/send остаётся открытым до этих операций.

**Зависимости и gate:** адреса → DNS → bounded streams → cancellation/ownership; loopback/fake resolver/clock и переносимые failures.

- [x] **HostName** — bounded canonical ASCII/Unicode spelling, strict UTS46 and Comparing<HostName>.

  Кандидаты из v3 для пересмотра: `HostName(origin is string)`; `ascii() is string`; `unicode() is string`.

- [x] **HostEndpoint** — immutable HostName/integer port, no DNS/socket effects.

  Кандидаты из v3 для пересмотра: `HostEndpoint(host is HostName, port is int)`; `host() is HostName`; `port() is int`; `string() is string`.

- [x] **IpEndpoint** — immutable numeric address/port/zone, canonical display and no I/O.

  Кандидаты из v3 для пересмотра: `IpEndpoint(address is IpAddress, port is int)`; `IpEndpoint(address is IpAddress, port is int, zone is string)`; `address() is IpAddress`; `port() is int`; `zone() is string`; `string() is string`.

- [x] **InvalidHostName** — typed original origin()/text(); capability failures remain native unsupported.

  Кандидаты из v3 для пересмотра: `InvalidHostName(origin is string)`; `text() is string`.

- [x] **InvalidPort** — typed original port() and range diagnostics.

  Кандидаты из v3 для пересмотра: `InvalidPort(port is int)`; `port() is int`; `text() is string`.

- [x] **InvalidIpv6Zone** — typed invalid spelling or IPv4-zone diagnostics.

  Кандидаты из v3 для пересмотра: `InvalidIpv6Zone(zone is string)`; `text() is string`.

- [x] Validate ports in `0..65535`.
- [ ] Permit port zero only where the consuming operation allows OS assignment.
- [x] Normalize host names through IDNA before resolution.
- [x] Reject empty labels, embedded NUL and overlong DNS names.
- [x] Permit a zone only for IPv6 endpoints.
- [x] Bracket IPv6 addresses when formatting an endpoint with a port.
- [x] Keep service-name lookup outside the first API; ports remain integers.
- [x] Do not create a one-method `Port` wrapper.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `net/dns` — fixed and system resolution VERIFIED

[Fixed configuration](fixed-resolution.md), [system worker contract](system-resolution.md).
Explicit typed configuration records replace the withdrawn v3 object-keyed maps.
First occurrence/order, requested ports, IDNA/root qualification, bounded answers,
typed misses/transient failures and started/unstarted cancellation are tested.
System workers can finish a libc call after caller cancellation; no stronger
interruption guarantee is claimed. Recursive DNS clients remain deferred.

**Зависимости и gate:** адреса → DNS → bounded streams → cancellation/ownership; loopback/fake resolver/clock и переносимые failures.

- [x] **Resolving** — nominal forward/reverse async contract.

  Кандидаты из v3 для пересмотра: `async addresses(origin is HostEndpoint) is Sequencing<IpEndpoint>`; `async name(origin is IpAddress) is HostName`.

- [x] **SystemResolution** — worker-backed configured host resolver.

  Кандидаты из v3 для пересмотра: `SystemResolution()`; `async addresses(origin is HostEndpoint) is Sequencing<IpEndpoint>`; `async name(origin is IpAddress) is HostName`.

- [x] **FixedResolution** — immutable HostAddresses/AddressName configuration.

  Кандидаты из v3 для пересмотра: `FixedResolution(addresses is %HostName:Sequencing<IpAddress>, names is %IpAddress:HostName)`; `FixedResolution(addresses is %HostName:Sequencing<IpAddress>)`; `async addresses(origin is HostEndpoint) is Sequencing<IpEndpoint>`; `async name(origin is IpAddress) is HostName`.

- [x] **UnknownHost** — exact requested HostName and bounded diagnostic.

  Кандидаты из v3 для пересмотра: `UnknownHost(origin is HostName)`; `host() is HostName`; `text() is string`.

- [x] **UnknownAddress** — exact requested IpAddress and bounded diagnostic.

  Кандидаты из v3 для пересмотра: `UnknownAddress(origin is IpAddress)`; `address() is IpAddress`; `text() is string`.

- [x] **DnsFailure** — system/transient failure without false unknown-host classification.

  Кандидаты из v3 для пересмотра: `DnsFailure(message is string)`; `text() is string`.

- [x] Preserve resolver result order while removing exact duplicates.
- [x] Attach the requested port to every forward result.
- [x] Return both IPv6 and IPv4 results when available.
- [x] Make resolution cancellable without blocking unrelated tasks.
- [x] Compose DNS deadlines through `Within`, not resolver-specific timeout settings (installed worker safety timeout documented).
- [x] Keep `FixedResolution` deterministic and free of native DNS/socket/system-name calls; pure numeric/IDNA/ordering primitives remain used.
- [x] Do not expose raw `getaddrinfo` flags or DNS wire records.
- [x] Defer configurable recursive DNS clients to a separate future stage.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `net/interface` — observation VERIFIED; system enumeration BLOCKED

[Snapshot contract](network-interface.md): 7 focused assertions, 1 inline group
and explicit fixture example PASS; evidence.
The current provider has no interface enumeration with assigned prefixes;
SystemInterfaces remains open, and no OS snapshot is claimed.

**Зависимости и gate:** адреса → DNS → bounded streams → cancellation/ownership; loopback/fake resolver/clock и переносимые failures.

- [x] **NetworkInterface** — immutable observation from explicit name/index/address/flag data.

  Кандидаты из v3 для пересмотра: `name() is string`; `index() is int`; `addresses() is Sequencing<IpInterface>`; `up() is bool`; `loopback() is bool`.

- [ ] **SystemInterfaces** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SystemInterfaces()`; `interfaces() is Sequencing<NetworkInterface>`; `interface(name is string) is NetworkInterface`; `interface(index is int) is NetworkInterface`.

- [ ] **MissingNetworkInterface** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Snapshot interface observations for each `SystemInterfaces` call.
- [ ] Preserve stable system interface indexes.
- [ ] Include every assigned IPv4 and IPv6 prefix available portably.
- [ ] Do not expose mutable interface configuration in the portable stdlib.
- [ ] Keep routes, firewall state and platform flags in Stage 34.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `net/stream` — async roles/endpoints and IP halves VERIFIED

[Contracts](async-stream-contracts.md): 9 focused assertions, 1 inline group and
memory example PASS; evidence.
[Concrete IP stream contract](tcp-stream.md): alias read admission, FIFO writes,
directional shutdown, queued close and lock-grant cancellation are verified.
Native/std/Unix compatibility: 4 files / 118 assertions PASS; full regression is
recorded with the TCP foundation evidence.

**Зависимости и gate:** адреса → DNS → bounded streams → cancellation/ownership; loopback/fake resolver/clock и переносимые failures.

- [x] **AsyncReading** — nominal positive bounded async read/EOF contract.

  Кандидаты из v3 для пересмотра: `async bytes(limit is int) is @byte`.

- [x] **AsyncWriting** — nominal async all-or-failure ByteSource write contract.

  Кандидаты из v3 для пересмотра: `async write(source is ByteSource) is AsyncWriting`.

- [x] **StreamReading** — bounded owned-operation reads and input shutdown.

  Кандидаты из v3 для пересмотра: `async bytes(limit is int) is @byte`; `closed() is bool`; `close() is void`.

- [x] **StreamWriting** — all-or-failure FIFO writes and draining output shutdown.

  Кандидаты из v3 для пересмотра: `async write(source is ByteSource) is StreamWriting`; `closed() is bool`; `close() is void`.

- [x] **StreamConnection** — shared Closing owner and actual IP endpoint snapshots.

  Кандидаты из v3 для пересмотра: `input() is StreamReading`; `output() is StreamWriting`; `closed() is bool`; `close() is void`.

- [ ] **StreamAcceptance** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `async connection() is StreamConnection`; `local_endpoint() is IpEndpoint`; `closed() is bool`; `close() is void`.

- [x] **ConnectionEndpoints<T>** — immutable directional pair of exact typed endpoints.

  Кандидаты из v3 для пересмотра: `ConnectionEndpoints(local is T, remote is T)`; `local_endpoint() is T`; `remote_endpoint() is T`.

- [x] Make empty `bytes()` output mean orderly end of stream.
- [x] Reject non-positive read limits.
- [x] Make `write()` all-or-failure even when native writes are partial.
- [ ] Preserve bytes already written when reporting a later failure only in internal diagnostics.
- [x] Make closing a reading half perform input shutdown.
- [x] Make closing a writing half perform output shutdown after queued writes finish.
- [x] Make connection close close both halves idempotently.
- [x] Serialize overlapping writes on one writing half.
- [x] Reject overlapping reads on one reading half with `InvalidState`.
- [x] Let cancellation stop waiting without corrupting the byte stream (an in-flight write closes its owner if a prefix may have escaped).
- [ ] Do not add `flush()` to unbuffered network streams.
- [ ] Do not make async streams satisfy synchronous Stage 07 `Reading` or `Writing`.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `net/tcp` — numeric socket capabilities VERIFIED; hostname racing DESIGN

[Option contract](tcp-options.md): 17 focused assertions, 2 inline groups and
example PASS; evidence.
The [numeric provider](tcp-stream.md) now applies the requests, acquires connections
and listeners lazily and owns cancellation cleanup. Host resolution and Happy
Eyeballs remain separate follow-up work.

**Зависимости и gate:** адреса → DNS → bounded streams → cancellation/ownership; loopback/fake resolver/clock и переносимые failures.

- [x] **TcpOptions** — immutable keepalive/no-delay requests with explicit defaults.

  Кандидаты из v3 для пересмотра: `TcpOptions()`; `TcpOptions(keep_alive is bool, no_delay is bool)`; `keep_alive() is bool`; `no_delay() is bool`.

- [x] **TcpListeningOptions** — immutable reuse/dual-stack/backlog requests.

  Кандидаты из v3 для пересмотра: `TcpListeningOptions()`; `TcpListeningOptions(address_reuse is bool, dual_stack is bool, backlog is int)`; `address_reuse() is bool`; `dual_stack() is bool`; `backlog() is int`.

- [x] **TcpConnecting** — numeric constructor and owned async connection; hostname overload remains open.

  Кандидаты из v3 для пересмотра: `TcpConnecting(remote is HostEndpoint, resolver is Resolving, options is TcpOptions, clock is Timing)`; `TcpConnecting(remote is IpEndpoint, options is TcpOptions)`; `async connection() is TcpConnection`.

- [x] **TcpListening** — lazy bind/listen acquisition with accepted-connection options.

  Кандидаты из v3 для пересмотра: `TcpListening(local is IpEndpoint, options is TcpListeningOptions)`; `async acceptance() is TcpAcceptance`.

- [x] **TcpAcceptance** — independent accepted owners and actual local endpoint.

  Кандидаты из v3 для пересмотра: `async connection() is TcpConnection`; `local_endpoint() is IpEndpoint`; `closed() is bool`; `close() is void`.

- [x] **TcpConnection** — Closing owner, concrete halves and actual endpoint pair.

  Кандидаты из v3 для пересмотра: `input() is StreamReading`; `output() is StreamWriting`; `endpoints() is ConnectionEndpoints<IpEndpoint>`; `closed() is bool`; `close() is void`.

- [ ] Resolve and attempt multiple addresses through a Happy Eyeballs strategy.
- [x] Prefer neither IPv4 nor IPv6 unconditionally.
- [x] Delay fallback attempts through injected monotonic `Timing`.
- [x] Close every losing connection attempt.
- [x] Make parent cancellation close every active attempt.
- [x] Apply keepalive and packet-delay settings after connection establishment.
- [x] Apply listener options before bind.
- [x] Reject negative backlog and choose a conservative default.
- [x] Report the OS-selected address after binding port zero.
- [x] Keep accepted connections independent from listener lifecycle.
- [ ] Compose connection, read and write deadlines through `Within`.
- [ ] Do not preserve the old empty `Socket` role containing only `local()`.
- [ ] Do not expose raw `connect()`, `bind()` or `listen()` mutation on a socket object.
- [ ] Do not let one `TcpConnection` accumulate stream, endpoint and shutdown actions directly.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

[TcpRacing](tcp-racing.md) snapshots supplied numeric candidates, stably interleaves
families, staggers relative starts and accelerates known failure. Pending/ready
losers are disposed before publication, using ordinals rather than native handle
equality. 455..458 and HTTP 453/454: 6 files / 36 assertions, inline/example PASS.
This is a connection-attempt primitive; hostname overload and strict RFC8305
resolver/minimum-spacing profile remain open.

### `net/udp` — VERIFIED

[Datagram contract](udp-datagram.md): 10 focused assertions, 2 inline groups
and runnable fixture PASS; evidence.
This value preserves captured sender/payload/truncation; live receive is described below.
[UDP option contract](udp-options.md): 11 additional assertions, combined module
run 21/21 PASS and 3 inline groups PASS; evidence.
[UDP binding contract](udp-binding.md): real nonblocking IPv4/IPv6 bind, actual
OS endpoint, pre-bind options and deterministic resource closure. Focused tests
also cover lazy/cached Futures and cancellation before startup;
evidence and runtime regression.
[Payload-size failure](udp-size-failure.md): DatagramTooLarge preserves a validated
byte count and stable diagnostic; 8 focused assertions and inline test PASS.
The send step below now uses this value for IP ceilings and host size rejection;
evidence.

[Receive contract](udp-receive.md): positive limits, one packet per lazy Future,
actual sender, bounded prefix and explicit truncation, cancellation and close.
Focused native/std tests: 35 assertions PASS; inline groups and example PASS.
Full regression: 490 files / 7812 tests; seven failing files reproduced on clean
current HEAD on this host, with identical failed-test numbers and exit statuses.
No new failing file; full suite is not green.
Evidence.

[Send contract](udp-send.md): one packet per lazy Future, one ByteSource snapshot,
accepted-byte result, IPv4/IPv6/mapped limits and typed host size rejection,
writable readiness, cancellation, independent read/write waits and close.
Focused send/receive/native compatibility: 6 files / 108 tests PASS.
Two further real-host size checks pass in the final 45-assertion native send suite.
Full regression: 492 files / 7873 tests; the same seven failing files, test numbers
and exit statuses as the clean-HEAD baseline. No new failures; full suite is not
green. Inline tests and loopback example PASS.
Evidence.

[Connected contract](udp-connection.md): fresh owned acquisition, fixed-peer
packet APIs, actual endpoints, filtering across IPv4/mapped IPv6 representations,
connect/readiness errors and cancellation cleanup. Focused UDP suite:
7 files / 153 assertions PASS, including 54 connected assertions. Eight net/udp
and three runtime/network inline groups and the loopback example PASS.
Full regression: 494 files / 7929 tests; the same seven failing files, failed-test
numbers, exit statuses and TAP diagnostics as the clean-HEAD baseline.
No new failures; the full suite is not green.
Evidence.

**Зависимости и gate:** адреса → DNS → bounded streams → cancellation/ownership; loopback/fake resolver/clock и переносимые failures.

- [x] **Datagram** — immutable captured sender/payload/truncation and finite ByteSource.

  Кандидаты из v3 для пересмотра: `Datagram(source is IpEndpoint, bytes is @byte, truncated is bool)`; `source() is IpEndpoint`; `bytes() is @byte`; `truncated() is bool`.

- [x] **UdpOptions** — immutable independent reuse/broadcast/dual-stack requests, defaults false.

  Кандидаты из v3 для пересмотра: `UdpOptions()`; `UdpOptions(address_reuse is bool, broadcast is bool, dual_stack is bool)`; `address_reuse() is bool`; `broadcast() is bool`; `dual_stack() is bool`.

- [x] **UdpBinding** — lazy numeric acquisition with pre-bind options and owned socket lifetime.

  Кандидаты из v3 для пересмотра: `UdpBinding(local is IpEndpoint, options is UdpOptions)`; `async socket() is UdpSocket`.

- [x] **UdpSocket** — lifecycle, async receive and send verified with bounded packets and cancellation.

  Утверждённый контракт: `async send(target is IpEndpoint, source is ByteSource) is int`;
  `async datagram(limit is int) is Datagram`; `local_endpoint() is IpEndpoint`;
  `closed() is bool`; `close() is void`. Send возвращает число принятых ОС байтов,
  без передачи владения сокетом; прежний v3 self-result не восстановлен.

- [x] **UdpConnecting** — immutable lazy acquisition of a fresh owned fixed-peer socket.

  Контракт: `UdpConnecting(local is IpEndpoint, remote is IpEndpoint, options is UdpOptions)`;
  `async connection() is UdpConnection`. Existing public sockets are not consumed.

- [x] **UdpConnection** — fixed-peer datagrams, actual endpoint snapshots and owned lifecycle.

  Контракт: `async send(source is ByteSource) is int`; `async datagram(limit is int) is Datagram`;
  `endpoints() is ConnectionEndpoints<IpEndpoint>`; `closed() is bool`; `close() is void`.
  The v3 self-result and truncation-losing bytes() candidate are withdrawn.

- [x] **DatagramTooLarge** — immutable non-negative attempted payload size, typed Error and stable diagnostic.

  Кандидаты из v3 для пересмотра: `DatagramTooLarge(size is int)`; `size() is int`; `text() is string`.

- [x] Preserve one datagram boundary per send and receive, including empty packets.
- [x] Report the actual sender endpoint, including IPv6 scope.
- [x] Report payload truncation explicitly.
- [x] Reject non-positive receive limits.
- [x] Reject IP-version oversized payloads before host submission; translate atomic host
  size rejection to DatagramTooLarge. No portable host/path limit pre-query is claimed.
- [x] Receive cancellation removes readiness without consuming a later datagram;
  send cancellation removes readiness without a later send attempt.
- [x] Apply reuse, broadcast and dual-stack options before bind.
- [x] Make connected UDP accept packets only from its fixed peer, including pre-connect queues.
- [x] Surface asynchronous ICMP failures when the platform reports them; no delivery guarantee.
- [x] Keep unconnected and connected UDP APIs separate.
- [x] Do not model UDP as a byte stream.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `net/multicast` — VERIFIED

[Option contract](multicast-options.md): default one hop and loopback enabled;
independent immutable settings, inclusive `0..255`, typed InvalidArgument outside
the range, no I/O. Focused options regression: 3 files / 45 assertions PASS
(17 multicast assertions), three inline groups and executable example PASS.
Evidence.

[Membership contract](multicast-membership.md): IPv4/IPv6 any-source join,
retained UDP owner, idempotent leave and parent closure, actual host interface
validation. First success pins socket-wide interface/hop/loopback policy;
conflicts fail without mutation. Failed acquisition rolls back settings; failed
rollback or leave closes the socket. Source-specific multicast is excluded.
Final membership coverage: 75 native and 16 public assertions PASS, four inline
groups and executable loopback example PASS. Real IPv4/IPv6 packet exchanges,
rollback/leave errors, parent ownership and cancellation are exercised.
Full regression: 497 files / 8044 tests; the same seven failing files and exact
failure/exit/TAP diagnostics as the clean-HEAD baseline. No new failures;
the full suite is not green. Tested on Darwin; Linux provider path is not
executed on this host and other IPv4 providers report unsupported.
Evidence.

**Зависимости и gate:** адреса → DNS → bounded streams → cancellation/ownership; loopback/fake resolver/clock и переносимые failures.

- [x] **MulticastOptions** — immutable hop/loopback requests with strict range validation.

  Контракт: `MulticastOptions()`; `MulticastOptions(hop_limit is int, loopback is bool)`;
  `hop_limit() is int`; `loopback() is bool`. Defaults `1,true`; no host-default sentinel.

- [x] **MulticastJoining** — pure validated request, synchronous membership acquisition.

  Контракт: `MulticastJoining(socket is UdpSocket, group is IpAddress, interface is NetworkInterface, options is MulticastOptions)`; `membership() is MulticastMembership`.

- [x] **MulticastMembership** — owned any-source membership retaining its UDP socket.

  Контракт: `group() is IpAddress`; `interface() is NetworkInterface`; `closed() is bool`; `close() is void`.

- [x] **InvalidMulticastGroup** — typed Error retaining the canonical address diagnostic.

  Контракт: `InvalidMulticastGroup(origin is IpAddress)`; `text() is string`.

- [x] Accept only addresses classified as multicast.
- [x] Validate hop limits in `0..255`.
- [x] Support IPv4 and IPv6 memberships through one public shape.
- [x] Leave the group exactly once when membership closes.
- [x] Let closing the socket implicitly end all of its memberships.
- [x] Keep source-specific multicast outside the first API.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `net/unix` — VERIFIED (filesystem streams/pairs; platform extensions deferred)

[Address contract](unix-address.md): exact lexical Path, no filesystem/socket I/O,
NUL rejection and UTF-8 byte-limit validation with a reserved terminator. Host
layout is inferred from pack/unpack probes; missing/unrecognized representation
raises UnixSocketsUnsupported. Native preflight is reused at stream
acquisition boundaries. Focused address/Path/legacy compatibility: 4 files /
54 assertions PASS, including 25 native and 19 public address assertions.
An independent real host bind at the exact byte limit passes on Darwin.
Full regression: 499 files / 8093 tests; the same seven failing files, failed-test
numbers, exit statuses and TAP diagnostics as the clean-HEAD baseline. No new
failures; full suite is not green. Inline diagnostic and executable example PASS.
Evidence.

[Stream contract](unix-streams.md): UnixEndpoint distinguishes named and unnamed
ends; UnixConnection owns retained AsyncReading/AsyncWriting halves. Actual
AF_UNIX pair/connect/listen/accept, partial I/O, cancellation and independent
accepted ownership are implemented. Focused: 2 files / 84 assertions PASS.
Listener cleanup is guarded by device/inode/type; callers protect the parent
directory against concurrent replacement during acquisition/close. This is not
an atomic conditional unlink or an adversarial-directory security guarantee.
Full regression: 502 files / 8219 tests; exact same seven failing-file summaries
as clean HEAD, including failed assertions, exit codes and TAP diagnostics.
No new failures; full suite is not green. Inline test and real executable example
PASS. Stream evidence.

**Зависимости и gate:** адреса → DNS → bounded streams → cancellation/ownership; loopback/fake resolver/clock и переносимые failures.

- [x] **UnixAddress** — named filesystem address with exact Path and host byte-limit preflight.

  Контракт: `UnixAddress(origin is Path)`; `path() is Path`; `string() is string`.

- [x] **UnixEndpoint** — named or unnamed immutable endpoint observation.

  Контракт: `UnixEndpoint()`; `UnixEndpoint(address is UnixAddress)`;
  `named() is bool`; `address() is UnixAddress` (unnamed throws InvalidArgument).

- [x] **UnixConnecting** — lazy, cancellable fresh acquisition.

  Контракт: `UnixConnecting(remote is UnixAddress)`; `async connection() is UnixConnection`.

- [x] **UnixListening** — lazy bind/listen without replacing existing entries.

  Контракт: `UnixListening(local is UnixAddress, backlog is int)`; `async acceptance() is UnixAcceptance`.

- [x] **UnixAcceptance** — owned listener, cancellable accept and stable absolute address.

  Контракт: `async connection() is UnixConnection`; `local_address() is UnixAddress`; `closed() is bool`; `close() is void`.

- [x] **UnixConnection** — owned stream and retained async halves.

  Контракт: `input() is AsyncReading`; `output() is AsyncWriting`; `endpoints() is ConnectionEndpoints<UnixEndpoint>`; `closed() is bool`; `close() is void`.

- [x] **UnixPaired** — lazy atomic acquisition of two connected owners.

  Контракт: `UnixPaired()`; `async result() is UnixPair`.

- [x] **UnixPair** — retains two independently closable connections.

  Контракт: `first() is UnixConnection`; `second() is UnixConnection`.

- [x] **UnixSocketsUnsupported** — explicit provider capability Error, used by address preflight.

  Контракт: `UnixSocketsUnsupported()`; `text() is string`.

- [x] Reject embedded NUL in filesystem socket paths.
- [x] Reject paths longer than the platform limit before bind or connect, including after resolving relative paths.
- [x] Guard listener entry removal by captured identity, under the protected-directory precondition above.
- [x] Keep peer credentials outside this stream API; a separate platform capability is deferred.
- [x] Put Linux abstract addresses and descriptor passing in Stage 34 (not implemented here).
- [x] Actual AF_UNIX streams are implemented independently of the unchanged legacy unsupported wrapper.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `net/tls` — owned verified client VERIFIED; server/identity open

[Client policy](tls-client-policy.md) retains explicit roots, TLS 1.2/1.3 and
opaque ordered ALPN. [Owned client transport](tls-client-transport.md) transfers
managed TCP at started handshake, verifies the actual peer and publishes owned
async halves/metadata only afterwards. Focused compatibility: 21 files / 187
assertions PASS; final cancellation/HTTPS CLI: 2 files / 6 PASS; two inline blocks
PASS. Full runtime regression: 647 files / 11950 assertions PASS. Server TLS, TlsIdentity and mutual TLS
remain open because PrivateKey lifetime/ownership is not yet defined.

**Зависимости и gate:** адреса → DNS → bounded streams → cancellation/ownership; loopback/fake resolver/clock и переносимые failures.

- [ ] **TlsIdentity** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TlsIdentity(certificates is CertificateChain, key is PrivateKey)`; `certificates() is CertificateChain`; `key() is PrivateKey`.

- [x] **TlsClientPolicy** — reusable root/version/ALPN snapshots, closed TlsVersion
  enum and fixed InvalidTlsPolicy errors; no ambient root acquisition or server identity.

  Кандидаты из v3 для пересмотра: `TlsClientPolicy(trust is TrustStore, versions is Set<TlsVersion>, protocols is Sequencing<string>)`; `trust() is TrustStore`; `versions() is Set<TlsVersion>`; `protocols() is Sequencing<string>`.

- [ ] **TlsServerPolicy** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TlsServerPolicy(identity is TlsIdentity, versions is Set<TlsVersion>, protocols is Sequencing<string>)`; `identity() is TlsIdentity`; `versions() is Set<TlsVersion>`; `protocols() is Sequencing<string>`.

- [x] **TlsClientHandshake** — lazy TCP-consuming request with nominal CertificateIdentity and optional Timing; async connection() returns only an actual verified peer.

  Кандидаты из v3 для пересмотра: `TlsClientHandshake(origin is TcpConnection, server is CertificateIdentity, policy is TlsClientPolicy[, clock is Timing])`; `async connection() is TlsConnection`. Client identity remains open.

- [ ] **TlsServerHandshake** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TlsServerHandshake(origin is TcpConnection, policy is TlsServerPolicy)`; `async connection() is TlsConnection`.

- [ ] **TlsAcceptance** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TlsAcceptance(origin is TcpAcceptance, policy is TlsServerPolicy)`; `async connection() is TlsConnection`; `local_endpoint() is IpEndpoint`; `closed() is bool`; `close() is void`.

- [x] **TlsConnection** — owns TlsReading/TlsWriting and immutable details; close() aborts, async shutdown() performs close-notify and disposal.

  Кандидаты из v3 для пересмотра: `input() is TlsReading`; `output() is TlsWriting`; `details() is TlsDetails`; `closed() is bool`; `close() is void`.

- [x] **TlsDetails** — actual endpoint/peer-chain/version/cipher/ALPN snapshots retained after closure.

  Кандидаты из v3 для пересмотра: `endpoints() is ConnectionEndpoints<IpEndpoint>`; `peer() is CertificateChain`; `version() is TlsVersion`; `cipher() is string`; `protocol() is string`.

- [x] **TlsFailure** — portable reason and TlsPhase; certificate-policy rejection retains CertificateVerificationFailed.

  Кандидаты из v3 для пересмотра: `TlsFailure(phase is TlsPhase, reason is string)`; `reason() is string`; `phase() is TlsPhase`; `text() is string`.

- [x] Require peer verification and hostname verification for every client handshake.
- [x] Verify the `HostName` supplied to each client handshake independently from reusable policy.
- [x] Do not provide a `verification: disabled` production policy.
- [x] Permit explicit private trust stores for tests and private deployments.
- [ ] Support optional client identity without weakening server verification.
- [x] Support only TLS 1.2 and TLS 1.3.
- [x] Reject an empty enabled-version set in client policy.
- [ ] Validate certificate and private-key correspondence before listening.
- [x] Advertise ALPN in caller preference order and retain the actual server-selected protocol; the server need not choose the first preference.
- [x] Return an empty negotiated protocol when neither side advertises ALPN.
- [x] Verify certificates through Stage 18 trust and X.509 policy.
- [x] Reuse Stage 18 Certificate/CertificateChain/TrustStore; PrivateKey reuse remains part of the open identity slice.
- [x] Make handshake, read, write and orderly shutdown cancellable.
- [x] Close the consumed TCP connection after any failed handshake.
- [ ] Keep accepted TLS sessions independent from listener lifecycle.
- [x] Add deterministic loopback tests with generated test certificates.
- [x] Add interoperability tests against an external TLS implementation.
- [ ] Add server-name identity selection after the single-identity API is stable.
- [ ] Add required and optional mutual-TLS client verification after the single-trust API is stable.
- [x] Do not duplicate Stage 18 certificate value objects.
- [x] Do not expose TLS engine handles, cipher numeric identifiers or raw alert codes.
- [x] Do not place TLS back into `crypto/tls`; it is a network transport using cryptography.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `runtime/message` — DEFERRED substrate

No consuming contract currently needs a native URL/MIME parser. URL percent
grammar is implemented in ordinary std over the existing UTF/hex/buffer APIs.
Mail transport still depends on async TCP/TLS. No speculative native API added.

**Зависимости и gate:** сначала consuming std object + контракт, затем ABI/provider; opaque handles, portable wire errors, fake provider + real integration.

- [ ] Утвердить недостающий объектный API этой области; готовые сигнатуры не выдумывать.
- [ ] Do not add native URL, MIME or mail value objects.
- [ ] Implement URL grammar and resolution in ordinary stdlib code.
- [ ] Implement MIME and Internet Message Format grammar in ordinary stdlib code.
- [ ] Build mail transports on Stage 20 async streams and TLS.
- [ ] Reuse Stage 06 Base64 and quoted-printable codecs.
- [ ] Reuse Stage 09 secure randomness for MIME boundaries and message identifiers.
- [ ] Reuse Stage 10 time values for message dates.
- [ ] Add a native bulk path only after profiling proves a specific parser bottleneck.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `url/percent` — VERIFIED

[Contract](url-percent.md): explicit component-safe sets, UTF-8 encoding,
one-pass decoding with source scalar errors and independent binary output.
Focused: 35 assertions PASS; inline tests and executable example PASS.
Shared full regression: 502 files / 8219 tests, exact same seven baseline failing
summaries, no new failures; full suite is not green.
Evidence.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **PercentEncoded** — decoded text plus UrlComponent choice; conversion on string().

  Контракт: `PercentEncoded(origin is string, component is UrlComponent)`; `string() is string`.

- [x] **PercentDecoded** — strict triplets, literal plus/Unicode, bytes before UTF-8.

  Контракт: `PercentDecoded(origin is string)`; `bytes() is @byte`; `string() is string`.

- [x] **InvalidPercentEncoding** — source scalar offset of first malformed percent.

  Контракт: `InvalidPercentEncoding(offset is int)`; `offset() is int`; `text() is string`.

- [x] Encode the UTF-8 bytes of non-ASCII characters.
- [x] Preserve only the unreserved and component-safe ASCII set.
- [x] Emit uppercase hexadecimal digits.
- [x] Decode percent triplets into bytes before decoding UTF-8.
- [x] Make `PercentDecoded.string()` throw `InvalidUtf8` for non-text bytes.
- [x] Preserve source offsets for malformed triplets.
- [x] Do not treat `+` as space outside form encoding.
- [x] Do not add primitive string actions for percent conversion.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `url/value` — full bounded URL/authority/reference API VERIFIED (focused)

Historical candidate signatures below are superseded by the authoritative
[contract](url-value.md), including UrlUserInfo/UrlPort presence coordinates.

[Component contract](url-parameters.md): eager URI/IRI component validation,
canonical ASCII, explicit presence, source scalar diagnostics. 21 focused
assertions PASS; inline test/example PASS. Full Url/UrlReference/UrlParts and authority/typed host projections are now
implemented with strict IDNA dependencies, ordinary URI/IRI parsing and all 42
RFC 3986 reference-resolution examples. [Contract](url-value.md).
Focused compatibility: 5 files / 99 groups PASS; bulk final: 3 files / 59 groups
PASS, including the independent 442-path RFC oracle. Three inline tests and
example PASS. Evidence.
Measured dot-stack VM overhead was investigated and removed through a private
bounded text operation; general ownership/import work is in PERF-12.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **Url** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `Url(origin is UrlParts)`; `parts() is UrlParts`; `url(reference is UrlReference) is Url`; `string() is string`.

- [x] **UrlReference** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `UrlReference(origin is UrlParts)`; `absolute() is bool`; `parts() is UrlParts`; `string() is string`.

- [x] **ParsedUrl** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `ParsedUrl(origin is string)`; `#projected value() is Url`.

- [x] **ParsedUrlReference** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `ParsedUrlReference(origin is string)`; `#projected value() is UrlReference`.

- [x] **UrlParts** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `UrlParts(scheme is string, authority is UrlAuthority, path is string, query is UrlQuery, fragment is UrlFragment)`; `scheme() is string`; `authority() is UrlAuthority`; `path() is string`; `query() is UrlQuery`; `fragment() is UrlFragment`.

- [x] **UrlAuthority** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `UrlAuthority()`; `UrlAuthority(host is UrlHost)`; `UrlAuthority(host is UrlHost, port is int)`; `UrlAuthority(userinfo is string, host is UrlHost, port is int)`; `present() is bool`; `userinfo() is string`; `host() is UrlHost`; `port() is int`; `string() is string`.

- [x] **UrlHost** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `kind() is UrlHostKind`; `name() is HostName`; `address() is IpAddress`; `string() is string`.

- [x] **NamedUrlHost** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `NamedUrlHost(origin is HostName)`; `kind() is UrlHostKind`; `name() is HostName`; `address() is IpAddress`; `string() is string`.

- [x] **AddressUrlHost** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `AddressUrlHost(origin is IpAddress)`; `kind() is UrlHostKind`; `name() is HostName`; `address() is IpAddress`; `string() is string`.

- [x] **FutureUrlHost** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `FutureUrlHost(origin is string)`; `kind() is UrlHostKind`; `name() is HostName`; `address() is IpAddress`; `string() is string`.

- [x] **UrlQuery** — контракт и реализация в url-parameters.md.

  Кандидаты из v3 для пересмотра: `UrlQuery()`; `UrlQuery(origin is string)`; `present() is bool`; `string() is string`.

- [x] **UrlFragment** — контракт и реализация в url-parameters.md.

  Кандидаты из v3 для пересмотра: `UrlFragment()`; `UrlFragment(origin is string)`; `present() is bool`; `string() is string`.

- [x] **InvalidUrl** — контракт и реализация в url-parameters.md.

  Кандидаты из v3 для пересмотра: `InvalidUrl(offset is int, reason is string)`; `offset() is int`; `text() is string`.

- [x] **MissingUrlAuthority** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] **MissingUrlPort** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] **UrlHostIsNotName** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] **UrlHostIsNotAddress** — implemented; authoritative contract in url-value.md.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] Require a scheme for `Url` and permit its absence in `UrlReference`.
- [x] Support hierarchical and opaque absolute URLs.
- [x] Permit authority only where introduced by `//`.
- [x] Make absent and explicitly empty query values distinguishable.
- [x] Make absent and explicitly empty fragments distinguishable.
- [x] Throw `MissingUrlAuthority` from authority-dependent observations when absent.
- [x] Throw `MissingUrlPort` instead of inventing a scheme-default port.
- [x] Parse bracketed IPv6 and IPvFuture without confusing their colons with a port.
- [x] Convert registered names through Stage 20 `HostName` and IDNA rules.
- [x] Accept Unicode IRI characters and serialize them as canonical ASCII URL text.
- [x] Lowercase schemes and registered host names.
- [x] Canonicalize numeric IP address text through `IpAddress`.
- [x] Preserve escaped reserved delimiters as escaped data.
- [x] Resolve references according to RFC 3986 including dot-segment removal.
- [x] Preserve a trailing slash during reference resolution.
- [x] Define equality from canonical serialized URL components.
- [x] Make zero-argument authority, query and fragment constructors represent absence.
- [x] Validate composed parts by the same rules as parsed parts.
- [x] Return an empty scheme only for a relative `UrlReference` without one.
- [x] Do not remove default ports without scheme-specific policy.
- [x] Do not sort query parameters during generic URL serialization.
- [x] Do not expose internal parser-part objects.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `url/query` — VERIFIED

[Contract](url-parameters.md): bounded immutable snapshot, ordered duplicate
parameters and assignment, split-before-decode, exact encoded input offsets.
17 focused assertions PASS, including source observation and limit ordering.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **QueryParameter** — реализован по контракту url-parameters.md.

  Контракт: `QueryParameter(name is string, value is string)`; `QueryParameter(name is string)`; `name() is string`; `value() is string`; `assigned() is bool`.

- [x] **QueryParameters** — реализован по контракту url-parameters.md.

  Контракт: `QueryParameters(entries is Sequencing<QueryParameter>)`; `entries() is Sequencing<QueryParameter>`; `membership(name is string) is bool`; `first(name is string) is string`; `values(name is string) is Sequencing<string>`; `string() is string`.

- [x] **ParsedQueryParameters** — реализован по контракту url-parameters.md.

  Контракт: `ParsedQueryParameters(origin is UrlQuery)`; `#projected value() is QueryParameters`.

- [x] **MissingQueryParameter** — реализован по контракту url-parameters.md.

  Контракт: `MissingQueryParameter(name is string)`; `name() is string`; `text() is string`.

- [x] Preserve parameter order and duplicate names.
- [x] Preserve the distinction between `flag` and `flag=`.
- [x] Split only on `&` in the first API.
- [x] Decode names and values independently after structural splitting.
- [x] Treat `+` as a literal plus.
- [x] Percent-encode spaces as `%20`.
- [x] Make `values()` return an empty sequence for a missing name; no all() alias.
- [x] Make `first()` throw `MissingQueryParameter` for a missing name.
- [x] Do not assume that every `UrlQuery` is a parameter list.

Additional approved errors: InvalidQueryEncoding(offset,reason) and
QueryLimitExceeded(kind); explicit limit constructor overloads are documented
in the API catalog.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `url/form` — VERIFIED

[Contract](url-parameters.md): separate strict UTF-8 form codec, plus semantics,
original byte error offsets, bounded cardinality/input. 19 focused assertions
PASS, including complete ASCII serialization and control-byte round-trip.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **FormUrlEncoded** — реализован по контракту url-parameters.md.

  Контракт: `FormUrlEncoded(origin is QueryParameters)`; `bytes() is @byte`; `string() is string`.

- [x] **ParsedForm** — реализован по контракту url-parameters.md.

  Контракт: `ParsedForm(origin is @byte)`; `parameters() is QueryParameters`.

- [x] **InvalidFormEncoding** — реализован по контракту url-parameters.md.

  Контракт: `InvalidFormEncoding(offset is int)`; `offset() is int`; `text() is string`.

- [x] Encode form data as UTF-8.
- [x] Encode spaces as `+` and literal plus as `%2B`.
- [x] Preserve ordering, duplicates and empty values.
- [x] Require `name=value` when serializing form parameters.
- [x] Decode percent triplets before UTF-8.
- [x] Reject malformed UTF-8 with an input offset.
- [x] Keep form encoding separate from generic URL query encoding.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `url/file` — BLOCKED

Depends on the unimplemented complete Url/authority model; query and fragment
values alone are insufficient for file conversion. No filesystem probing or
partial URL representation substituted.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **FileUrl** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `FileUrl(origin is Path)`; `url() is Url`.

- [ ] **UrlFile** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `UrlFile(origin is Url, style is PathStyle)`; `path() is Path`.

- [ ] **InvalidFileUrl** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Require the `file` scheme when projecting a path.
- [ ] Encode path segments without encoding path separators.
- [ ] Handle POSIX roots and Windows drive letters explicitly.
- [ ] Accept an empty authority and canonical `localhost` as local.
- [ ] Reject non-local authorities in the portable projection.
- [ ] Keep UNC and device paths behind explicit Windows behavior.
- [ ] Do not consult the filesystem while converting paths and URLs.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `message/header` — VERIFIED

[Contract](message-headers.md): ASCII field values, exact unfolding, ordered
bounded snapshots, case-insensitive lookup and CRLF rendering with hard/soft
line limits. 26 focused assertions PASS. Full header-section parsing and
field-specific grammar remain with their consuming message capabilities.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **MessageHeaderName** — реализован по контракту message-headers.md.

  Контракт: `MessageHeaderName(origin is string)`; `string() is string`.

- [x] **MessageHeader** — реализован по контракту message-headers.md.

  Контракт: `MessageHeader(name is MessageHeaderName, value is string)`; `name() is MessageHeaderName`; `value() is string`.

- [x] **MessageHeaders** — реализован по контракту message-headers.md.

  Контракт: `MessageHeaders(entries is Sequencing<MessageHeader>)`; `entries() is Sequencing<MessageHeader>`; `membership(name is MessageHeaderName) is bool`; `first(name is MessageHeaderName) is string`; `values(name is MessageHeaderName) is Sequencing<string>`; `addition(entry is MessageHeader) is MessageHeaders`.

- [x] **RenderedMessageHeaders** — реализован по контракту message-headers.md.

  Контракт: `RenderedMessageHeaders(origin is MessageHeaders)`; `bytes() is @byte`.

- [x] **InvalidMessageHeader** — реализован по контракту message-headers.md.

  Контракт: `InvalidMessageHeader(offset is int, reason is string)`; `offset() is int`; `text() is string`.

- [x] **MissingMessageHeader** — реализован по контракту message-headers.md.

  Контракт: `MissingMessageHeader(name is MessageHeaderName)`; `name() is MessageHeaderName`; `text() is string`.

- [x] Compare header names case-insensitively while preserving original spelling.
- [x] Preserve field order and duplicate fields.
- [x] Reject colon, controls, CR and LF in a header name.
- [x] Reject bare CR or LF in a logical field value.
- [x] Fold only before whitespace runs, conservatively excluding backslash-escaped boundaries; field-specific valid grammar is a caller precondition.
- [x] Use CRLF for physical lines.
- [x] Enforce the hard 998-octet line limit.
- [x] Target a soft 78-character line length.
- [x] Unfold continuation lines before exposing values.
- [x] Make `first()` throw and `values()` return empty on absence; no all() alias.
- [x] Do not reuse HTTP header values or parsing rules.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `mime/media-type` (executable `mime/media_type`) — VERIFIED (documented scope)

[Контракт](mime-media-type.md) · Evidence.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **MimeParameter** — контракт и реализация проверены focused tests.

  Публичный контракт: `MimeParameter(name is string, value is string)`; `name() is string`; `value() is string`.

- [x] **MimeParameters** — контракт и реализация проверены focused tests.

  Публичный контракт: `MimeParameters(entries is Sequencing<MimeParameter>)`; `entries() is Sequencing<MimeParameter>`; `membership(name is string) is bool`; `first(name is string) is string`; `values(name is string) is Sequencing<string>`; `string() is string`.

- [x] **MediaType** — контракт и реализация проверены focused tests.

  Публичный контракт: `MediaType(origin is string)`; `MediaType(type is string, subtype is string, parameters is MimeParameters)`; `type() is string`; `subtype() is string`; `parameters() is MimeParameters`; `string() is string`.

- [x] **InvalidMediaType** — контракт и реализация проверены focused tests.

  Публичный контракт: `InvalidMediaType(offset is int, reason is string)`; `offset() is int`; `text() is string`.

- [x] **MissingMimeParameter** — контракт и реализация проверены focused tests.

  Публичный контракт: `MissingMimeParameter(name is string)`; `name() is string`; `text() is string`.

- [x] Lowercase type, subtype and parameter names.
- [x] Preserve parameter ordering and unknown parameters.
- [x] Parse quoted strings and backslash escapes strictly.
- [x] Support extended UTF-8 parameters and continuations from RFC 2231.
- [x] Prefer the extended value when plain and extended forms coexist validly.
- [x] Make `first()` throw and `values()` return empty on absence.
- [x] Reject duplicate continuation indexes and missing continuation gaps.
- [x] Serialize non-ASCII parameter values through the extended form.
- [x] Do not maintain a closed enum of media types.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `url/data` — BLOCKED on full Url/authority API

MediaType is now available; DataUrl/ParsedDataUrl still require the deferred full `Url` value. No string-only substitute API is introduced.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **DataUrl** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `DataUrl(type is MediaType, data is @byte)`; `url() is Url`.

- [ ] **ParsedDataUrl** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ParsedDataUrl(origin is Url)`; `ParsedDataUrl(origin is Url, maximum is int)`; `type() is MediaType`; `data() is @byte`.

- [ ] **InvalidDataUrl** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Use percent encoding for compact safe text and Base64 otherwise.
- [ ] Recognize the `;base64` marker case-insensitively.
- [ ] Default an omitted media type to `text/plain;charset=US-ASCII`.
- [ ] Enforce the decoded-size limit while decoding.
- [ ] Reject fragments on data URLs.
- [ ] Never fetch external data while parsing a data URL.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `mime/disposition` — VERIFIED (documented scope)

[Контракт](mime-disposition.md) · Evidence.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **ContentDisposition** — контракт и реализация проверены focused tests.

  Публичный контракт: `ContentDisposition(origin is string)`; `ContentDisposition(kind is string, parameters is MimeParameters)`; `kind() is string`; `parameters() is MimeParameters`; `string() is string`.

- [x] **InvalidContentDisposition** — контракт и реализация проверены focused tests.

  Публичный контракт: `InvalidContentDisposition(offset is int, reason is string)`; `offset() is int`; `text() is string`.

- [x] **MissingContentDisposition** — контракт и реализация проверены focused tests.

  Публичный контракт: `text() is string`.

- [x] Lowercase disposition kinds and parameter names.
- [x] Preserve extension disposition kinds.
- [x] Parse filenames through RFC 2231 extended parameters.
- [x] Keep a received filename as an untrusted name, never a path.
- [x] Do not join a received filename to a filesystem directory implicitly.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `mime/transfer` — VERIFIED (documented scope)

[Контракт](mime-transfer.md) · Evidence.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **TransferEncodedBody** — контракт и реализация проверены focused tests.

  Публичный контракт: `TransferEncodedBody(origin is ByteSource, encoding is TransferEncoding)`; `bytes() is @byte`.

- [x] **TransferDecodedBody** — контракт и реализация проверены focused tests.

  Публичный контракт: `TransferDecodedBody(origin is ByteSource, encoding is TransferEncoding)`; `bytes() is @byte`.

- [x] **InvalidTransferEncoding** — контракт и реализация проверены focused tests.

  Публичный контракт: `text() is string`.

- [x] Enforce CRLF-safe line wrapping for MIME Base64.
- [x] Enforce MIME quoted-printable line limits and trailing-whitespace rules.
- [x] Reject bytes outside the declared seven-bit profile.
- [x] Preserve bytes exactly for binary transfer.
- [x] Reject unknown transfer-encoding tokens in MimePart/ParsedMime; transfer adapters take a closed enum.
- [x] Reuse Stage 06 codecs rather than duplicate their alphabets.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `mime/word` — VERIFIED (documented scope)

[Контракт](mime-word.md) · Evidence.

Unstructured/unfolded fields and explicit forbidden context are implemented; phrase/comment recognition remains with future structured parsers.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **EncodedWord** — контракт и реализация проверены focused tests.

  Публичный контракт: `EncodedWord(origin is string, charset is string)`; `string() is string`.

- [x] **DecodedWords** — контракт и реализация проверены focused tests.

  Публичный контракт: `DecodedWords(origin is string)`; `string() is string`.

- [x] **InvalidEncodedWord** — контракт и реализация проверены focused tests.

  Публичный контракт: `InvalidEncodedWord(offset is int, reason is string)`; `offset() is int`; `text() is string`.

- [x] Support Base64 and Q encoded-word forms.
- [x] Prefer UTF-8 for newly encoded words.
- [x] Split long encoded words without splitting a Unicode scalar value.
- [x] Decode adjacent encoded words according to RFC 2047 whitespace rules.
- [x] Never decode encoded-word syntax in forbidden structured positions.
- [x] Reject unsupported declared character sets explicitly.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `mime/part` — VERIFIED within documented profile

[Контракт](mime-part.md) · Evidence.

Strict materializing parser; message/rfc822 payloads stay opaque. Public constructor/role refinements and resource budgets are in the contract/API.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **MimeBody** — контракт и реализация; проверки в evidence.

  Публичный контракт: `bytes() is @byte`; `validate(type is MediaType) is void`.

- [x] **BinaryBody** — контракт и реализация; проверки в evidence.

  Публичный контракт: `BinaryBody(origin is @byte)`; `bytes() is @byte`.

- [x] **TextBody** — контракт и реализация; проверки в evidence.

  Публичный контракт: `TextBody(origin is string, charset is string)`; `string() is string`; `charset() is string`; `bytes() is @byte`.

- [x] **UnsupportedMimeCharset** — контракт и реализация; проверки в evidence.

  Публичный контракт: `UnsupportedMimeCharset(charset is string)`; `charset() is string`; `text() is string`.

- [x] **MultipartBody** — контракт и реализация; проверки в evidence.

  Публичный контракт: `MultipartBody(subtype is string, boundary is string, parts is Sequencing<MimePart>)`; `subtype() is string`; `boundary() is string`; `cursor() is Cursor<MimePart>`; `bytes() is @byte`.

- [x] **GeneratedBoundary** — контракт и реализация; проверки в evidence.

  Публичный контракт: `GeneratedBoundary(random is Randomizing)`; `string() is string`.

- [x] **MimePart** — контракт и реализация; проверки в evidence.

  Публичный контракт: `MimePart(headers is MessageHeaders, body is MimeBody)`; `headers() is MessageHeaders`; `body() is MimeBody`; `bytes() is @byte`.

- [x] **MimeContent** — контракт и реализация; проверки в evidence.

  Публичный контракт: `MimeContent(origin is MimePart)`; `type() is MediaType`; `disposition() is ContentDisposition`; `encoding() is TransferEncoding`.

- [x] **MimeLimits** — контракт и реализация; проверки в evidence.

  Публичный контракт: `MimeLimits()`; `MimeLimits(header_bytes is int, line_bytes is int, parts is int, depth is int, body_bytes is int)`; `header_bytes() is int`; `line_bytes() is int`; `parts() is int`; `depth() is int`; `body_bytes() is int`.

- [x] **ParsedMime** — контракт и реализация; проверки в evidence.

  Публичный контракт: `ParsedMime(origin is @byte)`; `ParsedMime(origin is @byte, limits is MimeLimits)`; `part() is MimePart`; `result() is MimeParsedNode` with immutable ordered `children()` navigation.

- [x] **InvalidMime** — контракт и реализация; проверки в evidence.

  Публичный контракт: `InvalidMime(offset is int, reason is string)`; `offset() is int`; `text() is string`.

- [x] Match multipart boundaries only at valid line boundaries.
- [x] Exclude delimiter CRLF from child body bytes.
- [x] Preserve preamble and epilogue internally without treating them as parts.
- [x] Require a closing boundary in strict parsing.
- [x] Validate that multipart media type and body boundary agree.
- [x] Apply content-transfer decoding only after boundary separation.
- [x] Enforce header, line, part, nesting and decoded-body limits incrementally.
- [x] Generate boundaries absent from every serialized child body.
- [x] Use CRLF in rendered MIME entities.
- [x] Preserve unknown MIME headers.
- [x] Make an absent content type default to `text/plain;charset=US-ASCII`.
- [x] Make an absent transfer encoding default to `seven_bit`.
- [x] Make an absent disposition throw `MissingContentDisposition`.
- [x] Decode only character sets supported by Stage 06 and report the rest explicitly.
- [x] Do not expose multipart traversal as a blocking cursor.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `mime/types` — VERIFIED

[Контракт](mime-types.md) · Evidence.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **MediaIdentifying** — контракт и реализация; проверки в evidence.

  Публичный контракт: `type(path is Path) is MediaType`; `extensions(type is MediaType) is Sequencing<string>`.

- [x] **StandardMediaTypes** — контракт и реализация; проверки в evidence.

  Публичный контракт: `StandardMediaTypes()`; `type(path is Path) is MediaType`; `extensions(type is MediaType) is Sequencing<string>`.

- [x] **SystemMediaTypes** — контракт и реализация; проверки в evidence.

  Публичный контракт: `SystemMediaTypes(fallback is MediaIdentifying)`; `type(path is Path) is MediaType`; `extensions(type is MediaType) is Sequencing<string>`.

- [x] **UnknownMediaType** — контракт и реализация; проверки в evidence.

  Публичный контракт: `UnknownMediaType(path is Path)`; `path() is Path`; `text() is string`.

- [x] Ship a deterministic IANA-derived common type table.
- [x] Match extensions case-insensitively.
- [x] Prefer the longest compound extension.
- [x] Snapshot optional system media-type files.
- [x] Keep caller overrides outside the immutable standard table.
- [x] Do not inspect file contents in extension-based identification.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `mail/address` — BLOCKED on HostName/IDNA

HostName/IDNA dependency is now verified; mailbox parsing/composition remains DESIGN. Do not substitute raw strings for its domain contract.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **Mailbox** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Mailbox(local is string, domain is HostName)`; `Mailbox(name is string, local is string, domain is HostName)`; `name() is string`; `local_part() is string`; `domain() is HostName`; `string() is string`.

- [ ] **AddressGroup** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AddressGroup(name is string, members is Sequencing<Mailbox>)`; `name() is string`; `members() is Sequencing<Mailbox>`; `string() is string`.

- [ ] **MailAddress** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `kind() is MailAddressKind`; `mailbox() is Mailbox`; `group() is AddressGroup`; `string() is string`.

- [ ] **MailboxAddress** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `MailboxAddress(origin is Mailbox)`; `kind() is MailAddressKind`; `mailbox() is Mailbox`; `group() is AddressGroup`; `string() is string`.

- [ ] **GroupAddress** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `GroupAddress(origin is AddressGroup)`; `kind() is MailAddressKind`; `mailbox() is Mailbox`; `group() is AddressGroup`; `string() is string`.

- [ ] **MailAddresses** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `MailAddresses(entries is Sequencing<MailAddress>)`; `entries() is Sequencing<MailAddress>`; `mailboxes() is Sequencing<Mailbox>`; `string() is string`.

- [ ] **ParsedMailAddresses** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ParsedMailAddresses(origin is string)`; `#projected value() is MailAddresses`.

- [ ] **InvalidMailAddress** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `InvalidMailAddress(offset is int, reason is string)`; `offset() is int`; `text() is string`.

- [ ] **MailAddressIsNotMailbox** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] **MailAddressIsNotGroup** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Parse quoted display names, comments, groups and angle addresses.
- [ ] Preserve display names as Unicode logical text.
- [ ] Serialize non-ASCII display names through encoded words.
- [ ] Normalize domains through `HostName`.
- [ ] Preserve local-part case.
- [ ] Throw a typed variant error from the wrong `MailAddress` projection.
- [ ] Support UTF-8 local parts only when SMTPUTF8 is explicitly available.
- [ ] Bind address parts without validation and reject obsolete route-address syntax lazily on the first projection.
- [ ] Do not expose a misleading undivided `email()` string action.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `mail/auth` — BLOCKED on deferred SecretBytes and verified TLS

SecretBytes/redacted credential ownership and TlsClientPolicy are deferred Stage 18/20 capabilities. Do not expose credentials in ordinary strings or add a plaintext authentication fallback.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **MailAuthentication** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `kind() is MailAuthenticationKind`; `identity() is string`.

- [ ] **MailCredentials** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `MailCredentials(username is string, password is SecretBytes)`; `kind() is MailAuthenticationKind`; `identity() is string`.

- [ ] **BearerCredentials** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `BearerCredentials(identity is string, token is SecretBytes)`; `kind() is MailAuthenticationKind`; `identity() is string`.

- [ ] Reject embedded NUL, CR and LF in authentication identities.
- [ ] Never expose password or bearer-token bytes through public actions.
- [ ] Redact all credential objects in diagnostics.
- [ ] Do not authenticate over plaintext transport.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `mail/message` — VERIFIED content/date profile; envelope BLOCKED

**Контракт:** [mail-message.md](mail-message.md), [API](stdlib-api-v4.md#mailmessage). Контейнер, рекурсивный разбор, явный DeliveredMail, современный MessageId и ограниченный профиль дат реализованы. Проверка адресных полей и обязательных From/Date остаётся будущим контрактом addressed message. Envelope/ReversePath/OutgoingMail и генерация MessageId BLOCKED на Mailbox/HostName; сырые строки их не заменяют.

- [x] **MailMessage** — реализован; точные перегрузки и ограничения в контракте.

  Кандидаты из v3 для пересмотра: `MailMessage(headers is MessageHeaders, body is MimeBody)`; `headers() is MessageHeaders`; `body() is MimeBody`; `bytes() is @byte`.

- [x] **ParsedMail** — реализован; точные перегрузки и ограничения в контракте.

  Кандидаты из v3 для пересмотра: `ParsedMail(origin is @byte)`; `ParsedMail(origin is @byte, limits is MimeLimits)`; `message() is MailMessage`.

- [ ] **ReversePath** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ReversePath()`; `ReversePath(origin is Mailbox)`; `present() is bool`; `mailbox() is Mailbox`.

- [ ] **MissingReversePath** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] **MailEnvelope** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `MailEnvelope(reverse_path is ReversePath, recipients is Sequencing<Mailbox>)`; `reverse_path() is ReversePath`; `recipients() is Sequencing<Mailbox>`.

- [ ] **OutgoingMail** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `OutgoingMail(envelope is MailEnvelope, message is MailMessage)`; `envelope() is MailEnvelope`; `message() is MailMessage`.

- [x] **MessageId** — реализован; точные перегрузки и ограничения в контракте.

  Кандидаты из v3 для пересмотра: `MessageId(origin is string)`; `string() is string`.

- [ ] **GeneratedMessageId** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `GeneratedMessageId(domain is HostName, clock is Timing, random is SecureRandom)`; `result() is MessageId`.

- [x] **MailDate** — реализован; точные перегрузки и ограничения в контракте.

  Кандидаты из v3 для пересмотра: `MailDate(origin is ZonedDateTime)`; `string() is string`.

- [x] **ParsedMailDate** — реализован; точные перегрузки и ограничения в контракте.

  Кандидаты из v3 для пересмотра: `ParsedMailDate(origin is string)`; `value() is ZonedDateTime`.

- [x] **InvalidMail** — реализован; точные перегрузки и ограничения в контракте.

  Кандидаты из v3 для пересмотра: `InvalidMail(offset is int, reason is string)`; `offset() is int`; `text() is string`.

- [ ] Reject an envelope without recipients.
- [ ] Keep transport envelope sender and recipients separate from message headers.
- [ ] Permit a null envelope sender for delivery-status messages.
- [ ] Make `EnvelopeSender.mailbox()` throw `MissingEnvelopeSender` for the null sender.
- [x] Preserve unknown headers and their order.
- [x] Parse MIME recursively through `ParsedMime` and shared limits.
- [x] Render one blank CRLF line between header block and body.
- [ ] Generate globally unpredictable message identifiers without embedding private host data.
- [x] Parse obsolete date zones only where RFC compatibility requires them.
- [x] Render new dates with a numeric offset.
- [x] Do not infer an SMTP envelope from `From`, `Sender`, `To`, `Cc` or `Bcc`.
- [x] Remove `Bcc` fields from a delivered wire message without changing the envelope.

**Проверка:** focused/inline/example и итоговая общая регрессия: evidence. Незакрытые пункты относятся к envelope и генерации ID, а не к готовности всего mail/message.

### `mail/mailbox` — VERIFIED local Maildir/mboxrd profiles; optional adapters DESIGN

**Контракт флагов:** [mail-flags.md](mail-flags.md), evidence. SystemFlag/KeywordFlag сохраняют wire spelling, сравнивают ASCII без регистра и дают типизированную ошибку неверной проекции. [StoredMail/MailFlags](stored-mail.md) и [Maildir](maildir.md) реализованы. Maildir публикует готовое письмо через atomic no-replace hard link, без copy fallback; это пересмотр первоначального предложения rename. ChangedMaildirFlags реализован с постоянной cooperative lock-координатой для всех операций Maildir. [Кодек mboxrd](mbox.md) реализован отдельно; [Mbox/MboxSession](mbox-session.md) реализованы в `mail/mbox_store`: постоянная sidecar-блокировка, session-local IDs, eager снимки и successor-команды с атомарной заменой. Старые void-сигнатуры пересмотрены под неизменяемую модель языка; add принимает явный MboxRecord.

- [x] **MailFlag** — реализован по ограниченному контракту флагов.

  Кандидаты из v3 для пересмотра: `kind() is MailFlagKind`; `string() is string`.

- [x] **SystemFlag** — реализован по ограниченному контракту флагов.

  Кандидаты из v3 для пересмотра: `SystemFlag(origin is SystemMailFlag)`; `kind() is MailFlagKind`; `system() is SystemMailFlag`; `string() is string`.

- [x] **KeywordFlag** — реализован по ограниченному контракту флагов.

  Кандидаты из v3 для пересмотра: `KeywordFlag(origin is string)`; `kind() is MailFlagKind`; `keyword() is string`; `string() is string`.

- [x] **StoredMail** — реализован по контракту ограниченных снимков и trusted-local Maildir.

  Кандидаты из v3 для пересмотра: `identifier() is string`; `message() is MailMessage`; `flags() is Set<MailFlag>`.

- [x] **Maildir** — реализован по контракту ограниченных снимков и trusted-local Maildir.

  Кандидаты из v3 для пересмотра: `Maildir(path is Path)`; `path() is Path`; `messages() is Sequencing<StoredMail>`; `add(message is MailMessage) is void`; `remove(identifier is string) is void`.

- [x] **ChangedMaildirFlags** — снимок флагов, проверка MIME до commit, canonical DFRST rename под общей exclusive-блокировкой.

  Кандидаты из v3 для пересмотра: `ChangedMaildirFlags(origin is Maildir, identifier is string, flags is Set<MailFlag>)`; `result() is StoredMail`.

- [x] **Mbox** — чистая координата существующего локального mboxrd-файла; session() открывает ограниченный снимок под блокировкой.

  v4: `Mbox(path is Path[,limits is MboxLimits])`; `path()/limits()`; `session() is MboxSession` (`mail/mbox_store`).

- [x] **MboxSession** — общая lease для successor-версий; стабильные в lineage ID, проверка исходных байтов, закрытие и атомарная запись.

  v4: `messages() is Sequencing<StoredMail>`; `records() is Sequencing<MboxRecord>`; `add(record is MboxRecord) is ^MboxSession`; `remove(identifier is string) is ^MboxSession`; `closed()/close()`. ID не сохраняются после reopen; flags не выводятся из недоверенных заголовков.

- [x] **InvalidMailbox** — реализован по контракту ограниченных снимков и trusted-local Maildir.

  Кандидаты из v3 для пересмотра: `InvalidMailbox(offset is int, reason is string)`; `offset() is int`; `text() is string`.

- [x] Commit Maildir messages through same-filesystem atomic no-replace publication (hard link; revised contract).
- [x] Generate collision-resistant Maildir identifiers (192 random bits; bounded collision retries).
- [x] Preserve Maildir flags in filenames without trusting arbitrary path text (DFRST profile; unsupported keyword/Recent flags fail explicitly).
- [x] Preserve unknown IMAP keyword flags.
- [x] Throw a typed variant error from the wrong `MailFlag` projection.
- [x] Lock mbox before reading or changing it (permanent exclusive sidecar; immediate contention).
- [x] Escape and unescape mbox `From ` separator lines reversibly (`mail/mbox`, explicit mboxrd profile).
- [x] Rewrite mbox changes atomically before releasing the lock (same-directory temporary + rename; no copy fallback).
- [x] Enforce `MimeLimits` while loading stored messages, plus aggregate wire/message limits.
- [x] Do not hold an mbox lock after `MboxSession.close()` (all successor versions share the closed lease).
- [ ] Add MMDF, MH and Babyl only as low-priority format adapters.

**Проверки:** StoredMail evidence, Maildir evidence. Maildir flags evidence, mboxrd evidence. MboxSession evidence. Оставшийся checkbox — дополнительные форматы MMDF/MH/Babyl; durable UIDs, чужие lock-протоколы и filesystem metadata вне текущего локального профиля.

### `mail/smtp` — wire primitives VERIFIED; sessions DESIGN

**Реализовано:** [контракт](smtp-wire.md), evidence. `mail/smtp_reply` даёт строгие reply values и bounded successor decoder с remainder; `mail/smtp` — EHLO capabilities без догадок по greeting; `mail/smtp_data` — eager DATA с Bcc filtering, точным SIZE, проверкой 8BITMIME и dot transparency. Сетевых эффектов, авторизации и отправки писем нет.

- [x] Bounded SMTP reply values and incremental CRLF framing, preserving coalesced next-reply bytes.
- [x] Explicit SIZE absence/zero/BigInt states and retained unknown EHLO extensions.
- [x] Eager DATA byte snapshot with one borrowed body.bytes() observation.
- [x] `mail/smtp_reading`: lazy one-shot `SmtpReplyReading` over borrowed AsyncReading, cached Future, explicit remainder, bounded requests and cancellation cleanup. [Contract](smtp-reading.md), evidence.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **SmtpServer** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SmtpServer(endpoint is HostEndpoint, security is SmtpSecurity)`; `SmtpServer(endpoint is HostEndpoint, security is SmtpSecurity, protocol is SmtpProtocol)`; `endpoint() is HostEndpoint`; `security() is SmtpSecurity`; `protocol() is SmtpProtocol`.

- [x] **SmtpCapabilities** — eager snapshot успешного EHLO, явный SIZE, SASL names, unknown extensions и отказ от duplicate keywords.

  v4: `SmtpCapabilities(reply is SmtpReply)`; `extensions()/features()/mechanisms()`; `has_maximum_size()`; `maximum_size() is BigInt` (MissingSmtpSize без числа).

- [ ] **SmtpConnecting** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SmtpConnecting(server is SmtpServer, resolver is Resolving, clock is Timing)`; `SmtpConnecting(server is SmtpServer, resolver is Resolving, tls is TlsClientPolicy, clock is Timing)`; `async session() is SmtpSession`.

- [ ] **LmtpUnixConnecting** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `LmtpUnixConnecting(endpoint is UnixAddress, clock is Timing)`; `async session() is SmtpSession`.

- [ ] **SmtpSession** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `capabilities() is SmtpCapabilities`; `closed() is bool`; `close() is void`.

- [ ] **SmtpAuthenticated** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SmtpAuthenticated(origin is SmtpSession, authentication is MailAuthentication)`; `async session() is SmtpSession`.

- [ ] **SmtpDelivery** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SmtpDelivery(origin is SmtpSession, mail is OutgoingMail)`; `async receipt() is SmtpReceipt`.

- [ ] **RecipientDelivery** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `recipient() is Mailbox`; `accepted() is bool`; `code() is int`; `text() is string`.

- [ ] **SmtpReceipt** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `recipients() is Sequencing<RecipientDelivery>`; `accepted() is bool`; `code() is int`; `text() is string`.

- [ ] **SmtpFailure** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SmtpFailure(code is int, message is string)`; `code() is int`; `text() is string`.

- [x] Read multiline replies with strict line and aggregate limits (`SmtpReplyDecoder` + `SmtpReplyReading`; Within/Before compose around the one-shot result).
- [ ] Send EHLO and retain unknown extension tokens internally.
- [ ] Require a successful second EHLO after STARTTLS.
- [ ] Verify implicit TLS and STARTTLS with `TlsClientPolicy`.
- [ ] Permit the constructor without `TlsClientPolicy` only for explicitly plain unauthenticated transport.
- [ ] Make `SmtpServer` default to the SMTP protocol.
- [ ] Support local LMTP over a Unix-domain connection without pretending it is SMTP over TCP.
- [ ] Refuse credentials before verified TLS is active.
- [ ] Support PLAIN and bearer authentication only over verified TLS.
- [x] Dot-stuff message lines and terminate DATA with CRLF dot CRLF (PreparedSmtpData; literal content requires terminal CRLF).
- [ ] Negotiate SMTPUTF8 before sending UTF-8 envelope local parts.
- [ ] Negotiate 8BITMIME or choose a seven-bit transfer encoding.
- [x] Enforce the advertised message-size limit before DATA (bounded preparation; no implicit network command).
- [ ] Preserve per-recipient partial acceptance in `SmtpReceipt`.
- [ ] Preserve LMTP's final response for every accepted recipient.
- [ ] Reuse an open session for sequential deliveries.
- [ ] Serialize commands within one session.
- [ ] Make every network wait cancellable.
- [ ] Compose connection and command deadlines through `Within`.
- [ ] Send QUIT during orderly close but close immediately after cancellation or protocol failure.
- [ ] Do not expose a raw SMTP command method.
- [ ] Do not retry delivery implicitly.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `mail/imap` — authenticated rev1 protocol VERIFIED; secure connection BLOCKED

**Реализовано:** `mail/imap_name`: bounded eager `ImapUtf7Encoded`/`ImapUtf7Decoded`
и `ImapMailboxName` с безопасным quoting. [Контракт](imap-names.md),
evidence. Codecs сохраняют Unicode;
mailbox profile отклоняет ASCII controls. UTF8=ACCEPT/rev2 negotiation,
полная семантика расширений и безопасное подключение остаются отдельными задачами; HostName/TLS/auth ещё отсутствуют.
Добавлен [authenticated protocol layer](imap-protocol.md): typed commands, UID scopes, selection/FETCH/изменения и IDLE.
222 focused assertions, включая lifecycle/UIDPLUS и реальную Unix socket pair, 11 inline-групп и пример PASS.
Полная регрессия: 546 файлов / 9544 проверки; прежние семь failure signatures без изменений. Evidence.
Это не полное завершение IMAP: TLS/auth, FromMail и UTF-8 negotiation остаются открыты.

- [x] Canonical modified UTF-7 codec with scalar/wire bounds and strict malformed-input handling.
- [x] Quoted IMAP4rev1 mailbox-name values without automatic case/Unicode normalization.
- [x] Bounded CAPABILITY snapshots and exact untagged line parsing: unknown tokens, source order/spelling, explicit revision and distinct AUTH observations. [Contract](imap-capabilities.md), evidence.
- [x] Bounded ASCII response framing for documented tagged/untagged/continuation forms, literal/literal8 and exact remainder. [Contract](imap-responses.md), evidence.
- [x] `mail/imap_reading`: lazy one-shot response reading over borrowed AsyncReading, cached Future, explicit remainder, bounded requests and cancellation cleanup, including mid-literal. [Contract](imap-reading.md), evidence.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **MailMatching** — `key() is ImapSearchKey`: bounded typed criteria, no raw query strings.

- [x] **AllMail** — `AllMail().key()` — ALL.

- [x] **UnseenMail** — `UnseenMail().key()` — UNSEEN.

- [ ] **FromMail** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `FromMail(origin is Mailbox)`; `criterion() is string`.

- [x] **SinceMail** — `SinceMail(Date).key()` — inclusive INTERNALDATE date criterion.

- [x] **AndMail** — `AndMail(left,right[,limits]).key()` — bounded conjunction.

- [x] **OrMail** — `OrMail(left,right[,limits]).key()` — bounded disjunction.

- [x] **NotMail** — `NotMail(origin[,limits]).key()` — bounded negation.

- [ ] **ImapServer** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ImapServer(endpoint is HostEndpoint, security is ImapSecurity)`; `endpoint() is HostEndpoint`; `security() is ImapSecurity`.

- [ ] **ImapConnecting** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ImapConnecting(server is ImapServer, resolver is Resolving, tls is TlsClientPolicy, authentication is MailAuthentication, clock is Timing)`; `async session() is ImapSession`.

- [x] **ImapCapabilities** — eager bounded snapshot объявленных возможностей; требует rev1/rev2, не означает включение расширений или TLS.

  v4: `ImapCapabilities(entries is Sequencing<string>[,limits is ImapCapabilityLimits])`; `tokens()/names()/features()/authentication()/bytes()`; `ParsedImapCapabilities(@byte[,limits]).capabilities()` для одной untagged строки.

- [x] **ImapMailbox** — basic LIST/LSUB observation: typed name, optional delimiter, string attributes.

- [x] **SelectedMailbox** — generation-scoped selection; async state snapshot; close aborts owner without expunging.

- [x] **ImapMailboxState** — immutable observations; optional RECENT/UIDNEXT/first-unseen sequence; UIDVALIDITY invalidation.

- [x] **ImapSession** — authenticated protocol infrastructure only: exclusive owned channel, advertised rev1 mode, account scope, serialization.

- [x] **ImapMessage** — UID-bearing FETCH observation; optional content/flags remain explicit.

- [x] **SearchedMail** — cached UID SEARCH → account/name/UIDVALIDITY-scoped MailboxUids.

- [x] **FetchedMail** — cached UID FETCH with BODY.PEEK and mandatory UID; typed requested data.

- [x] **ChangedMailFlags** — cached UID STORE with read-only and UID epoch checks.

- [x] **ExpungedMail** — targeted UID EXPUNGE only; returns observed sequence updates, including possible unsolicited updates.

- [x] **AppendedMail** — one synchronizing APPEND literal; optional UID assignment from validated APPENDUID.

- [x] **CopiedMail** — UID COPY; optional one-to-one COPYUID mapping, never invented target UIDs.

- [x] **MovedMail** — advertised UID MOVE; no implicit non-atomic fallback.

- [x] **CreatedImapMailbox** — cached acknowledged mailbox name; no fabricated LIST attributes.

- [x] **ImapMailboxDeletion** — cached DELETE; successful deletion expires selection conservatively.

- [x] **RenamedImapMailbox** — cached RENAME with quoted source/destination and selection expiry.

- [x] **ImapSubscription** — cached SUBSCRIBE/UNSUBSCRIBE → acknowledged name.

- [x] **MailboxEvent** — exact response and resulting state; sequence/UID/flags only when present.

- [x] **MailboxEvents** — lazy advertised IDLE, bounded event queue, async DONE-and-completion close.

- [x] **Typed IMAP failures** — command rejection, invalid data/identifiers, bounds, stale scope, missing data and closed channel; generic v3 ImapFailure is withdrawn.

- [ ] Require verified TLS before authentication.
- [ ] Support SASL PLAIN and bearer authentication over verified TLS.
- [x] Generate unique command tags internally.
- [ ] Parse tagged, untagged and continuation responses incrementally (known ASCII framing profile VERIFIED; complete semantic parsing, extension grammar and UTF-8 mode remain).
- [x] Bound response lines, literals and aggregate command output, including IDLE response/event budgets.
- [ ] Quote or literal-encode every mailbox name and search value (implemented commands use typed quoting; FromMail waits for Mailbox).
- [ ] Support modified UTF-7 until UTF8=ACCEPT is active (codec VERIFIED; session mode negotiation remains).
- [x] Keep sequence numbers and stable UIDs distinct.
- [x] Use UID SEARCH, UID FETCH and UID STORE by default.
- [x] Use UID COPY, UID MOVE and UID EXPUNGE when the server supports them.
- [ ] Equivalent MOVE fallback is intentionally not implemented; unsupported MOVE fails before I/O.
- [x] Return assigned UIDs only when UIDPLUS supplies them.
- [x] Invalidate cached UIDs when UIDVALIDITY changes.
- [x] Surface unsolicited mailbox state changes through the next operation.
- [x] Serialize commands within one session.
- [x] Enter IDLE lazily on the first `MailboxEvents.next()` call.
- [x] Leave IDLE before executing another command on the session.
- [x] Make closing `MailboxEvents` send DONE and await tagged completion.
- [x] Compose all deadlines through `Within`.
- [x] Do not expose raw IMAP commands or unescaped criteria strings.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `mail/pop3` — reply framing/reading и transaction session VERIFIED; connection/auth BLOCKED

[Reply framing](pop3-responses.md) дополнено [контрактом транзакционной сессии](pop3-session.md).
Реализованы typed CAPA/STAT/LIST/UIDL snapshots, RETR/TOP, DELE/RSET и явный QUIT.
Сессия принимает исключительно принадлежащий ей **уже авторизованный** transport;
она не подтверждает TLS и не отправляет credentials. Полная регрессия: 564 файла /
10285 проверок; новые POP3 тесты PASS, семь прежних падений подтверждены на baseline.

- [ ] **PopServer / PopConnecting** — BLOCKED на HostName/HostEndpoint/IDNA,
  TcpConnection, verified TLS и SecretBytes/MailCredentials. Не добавлены фиктивные
  constructors, строки пароля или небезопасный fallback.
- [x] **PopMessage** — immutable typed PopNumber, exact BigInt size и optional
  PopIdentifier; отсутствие UIDL наблюдаемо и не заменяется номером сообщения.
- [x] **PopMailbox** — live capability: async state/count+size snapshot,
  bounded messages, bytes/parsed message; async void delete marks through DELE.
- [x] **PopSession** — exclusive authenticated transport, lock serialization,
  Atomic unread prefix, explicit abort/QUIT ownership, typed failures.
- [x] **PreviewedPopMail** — advertised TOP, validated line count, cached lazy result.
- [x] **PopDeleteReset** — explicit async void RSET.
- [x] **Typed failures** — InvalidPopData, PopListingLimitExceeded,
  MissingPopIdentifier, PopCommandRejected, PopSessionClosed; generic PopFailure
  из v3 не восстановлен.
- [ ] Require verified TLS before authentication — BLOCKED вместе с connector.
- [ ] Support USER/PASS only over verified TLS — BLOCKED, credential API отсутствует.
- [x] Incremental dot-stuffed multiline framing и cached borrowed reading.
- [x] Независимые reply line/body/wire/line-count и listing entry/decimal limits.
- [x] Session message numbers distinct from persistent maildrop-scoped UIDL values.
- [x] UIDL capability absence explicit; LIST/UIDL number sets validated under one lock.
- [x] TOP only when advertised; negative line counts rejected before I/O.
- [x] Успешный orderly QUIT подтверждает UPDATE; close/destroy не посылают QUIT.
  Неуспешный/потерянный ответ QUIT не обещает rollback: сервер мог удалить часть
  marked messages (уточнение прежней формулировки по RFC 1939).
- [x] Explicit RSET resets pending deletes.
- [x] Cancellable waits, queued cancellation without I/O, started cancellation
  drains nested cleanup and aborts transport.
- [x] Deadlines through existing Within; async void не подменяется receiver result.
- [x] No raw commands or unescaped credential API; typed command infrastructure only.

**Проверки:** tests-first absent-module failure; deterministic protocol cases,
реальный Unix socket pair с двухбайтовыми reads, inline tests и
[пример](../../examples/std-v4/pop3-session.ok). Финальный focused: **4 файла / 73 проверки, PASS**,
оба inline test-блока и пример PASS; evidence.
Полная регрессия: **564 файла / 10285 проверок**; общий результат FAIL,
ровно семь сигнатур совпадают с чистым `ddb3a7b`. Новых падений не обнаружено.
Connection/auth остаются BLOCKED, не VERIFIED.

### `runtime/http` — CONTRACT; network foundation BLOCKED; native acceleration DEFERRED

**Зависимости и gate:** сначала consuming std object + контракт, затем ABI/provider; opaque handles, portable wire errors, fake provider + real integration.

- [ ] Утвердить недостающий объектный API этой области; готовые сигнатуры не выдумывать.
- [ ] Build HTTP/1.1 and HTTP/2 on Stage 20 async byte streams.
  [Current gate](http-runtime-gate.md): pure values need no native ABI. TCP
  connections, verified TLS/ALPN, complete URL authority and deferred codecs
  prevent claiming a complete v4 HTTP stack. Independent pure contracts proceed.
- [ ] Drive protocol timeouts through Stage 19 cancellation and `Within`.
- [ ] Reuse Stage 15 digests for WebSocket handshake validation.
- [ ] Reuse Stage 16 compression for HTTP content codings.
- [ ] Reuse Stage 18 certificate verification and Stage 20 TLS.
- [ ] Reuse Stage 21 URL, form, MIME media-type and cookie-adjacent grammar.
- [x] Native Huffman bulk path after measured PERF-22 bottleneck; [primitives](hpack-primitives.md), provider/std focused PASS; full native-change gate PASS (680 files / 12,266 assertions). Other bulk paths require their own measurements.
- [x] No second event loop, socket layer, TLS abstraction or speculative native ABI added.

**Audit:** existing `std/net/stream` and `std/net/tcp` inspected; no runtime
integration is claimed until a concrete consumer and tested provider exist.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `http/method` — VERIFIED (focused)

[Contract](http-method.md): pure immutable values, no TLS/URL/runtime dependency.

- [x] Method(origin is string): nonempty ASCII HTTP token, exact spelling.
- [x] MethodSemantics(origin is Method): safe/idempotent standard observations.
- [x] InvalidMethod: typed rejection at construction.
- [x] Preserve case-sensitive identities, including extension methods. The old
  uppercasing candidate was incorrect under RFC 9110 section 9.1.
- [x] GET/HEAD/OPTIONS/TRACE safe; these plus PUT/DELETE idempotent.
- [x] Unknown extensions remain valid with conservative false observations.
- [x] No closed enum, automatic retry or GET/DELETE body prohibition.

**Verification:** 5 focused groups (all 128 ASCII octets, standard semantics,
injection/Unicode/empty rejection, case and long tokens), inline test and
[example](../../examples/std-v4/http-method.ok) PASS.
Evidence; shared repository regression
is recorded with the preceding POP3 slice and does not include this new test file.

### `http/status` — VERIFIED (focused)

[Contract](http-status.md): numeric immutable response identity, independent of transport.

- [x] Status(code): exact 100..599, including extension codes.
- [x] StatusClass from the hundred digit; all 500 accepted codes tested.
- [x] InvalidStatus preserves rejected code; invalid boundaries tested.
- [x] Optional display phrase from an explicit IANA snapshot; unknown/unused empty.
- [x] Phrases have no protocol semantics; no HTTP/2 phrase serialization added.

**Verification:** 2 files / 7 groups PASS, including unchanged legacy status tests;
inline test and [example](../../examples/std-v4/http-status.ok) PASS.
Evidence. Shared baseline regression
is recorded with POP3; this new module has separate focused/inline coverage.

### `http/version` — HTTP1 identities/TLS selection VERIFIED; HTTP2 open

[Message contract](http-message.md): HttpVersion identity is now consumed by
actual Request/Response metadata; exact HTTP1 token conversion/parsing is tested.
HTTP2 identity cannot be serialized as a textual HTTP1 version. This prerequisite
does not alone select a transport. [Http1TlsSession](http1-tls-session.md) now
consumes actual verified TLS metadata, accepting exact http/1.1 or absent ALPN
and rejecting other selections before HTTP I/O. HTTP2/prior-knowledge remain open.

- [x] **UnsupportedHttpVersion** — payload-free error for unsupported exact wire conversion/parsing.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] Negotiate HTTP/2 over TLS through ALPN `h2`.
- [x] Consume HTTP/1.1 through actual ALPN `http/1.1` or absent-ALPN fallback; other selected protocols never fall back.
- [x] Support cleartext HTTP/1.1 directly through the existing owned stream session.
- [ ] Support prior-knowledge cleartext HTTP/2 only through explicit policy.
- [ ] Do not silently attempt the ambiguous h2c upgrade path in the first API.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `http/header` — values VERIFIED (focused); HTTP/2 wire encoding remains protocol work

[Contract](http-header.md): independent HTTP token/value grammar and bounded snapshots.

- [x] HeaderName preserves spelling; ASCII lowercase/order define HTTP matching.
- [x] Header rejects injection/control/folding; Latin-1 octet values retain obs-text.
- [x] Only edge SP/HTAB trimmed; inner whitespace and non-breaking space preserved.
- [x] Headers snapshots each source item once; independent entry/byte limits.
- [x] Order/duplicates preserved; no implicit comma-join, including Set-Cookie.
- [x] Missing first throws typed MissingHeader; missing values returns empty.
- [x] Addition, HeadersWithout and ReplacedHeader are persistent and retain limits.
  Removal names use a bounded sequence, not structural Set equality.
- [x] Http2Headers produces lowercase logical fields, rejects connection-specific
  fields and permits TE trailers only for requests.
- [ ] HTTP/2 actual wire encoding/decoding: remains the protocol/HPACK section.
- [x] No mail/MIME header parser reuse; HTTP token grammar is shared with Method.

**Verification:** final 20 header groups PASS; related run 4 files / 33 groups
PASS before the final snapshot test; inline test and
[example](../../examples/std-v4/http-header.ok) PASS.
Evidence. Empty-name generic inference
uses an explicitly typed local; no interpreter change. Existing-suite regression
is shared with POP3; the new modules have their own focused/inline coverage.

### `http/body` — byte/owned-stream bodies VERIFIED (focused); FileBody and wire lifecycle BLOCKED

[Contract](http-body.md): explicit BodyOpening and BodyReading roles, owned reading.

- [x] EmptyBody / ByteBody: exact BigInt length, independent replayable readers.
- [x] OneShotBody: atomic single reading claim, unknown length throws explicitly;
  unopened destruction closes the adopted reader.
- [x] StreamBodyReading adopts already-delimited AsyncReading + Closing owner.
- [x] Positive bounded reads; empty output is permanent clean EOF.
- [x] Trailers only after clean EOF; early close never publishes them.
- [x] Close cancels active reads; cancelled await drains suspended nested cleanup.
- [x] Unstarted/queued cancellation preserves source; started failures abort it.
- [x] One FutureGroup per read preserves secondary cleanup failures without
  accumulating completed reads. Typed source failures and panic remain distinct.
- [x] No implicit full-body materialization, retry, drain or connection reuse.
- [ ] FileBody — BLOCKED on cancellable file reading / pinned replay semantics.
  Existing synchronous FileBytes cannot establish those guarantees by wrapping
  it in an async action. No substitute full-file buffer is added.
- [ ] HTTP/1 bounded drain/reuse and HTTP/2 stream reset — actual protocol work.

**Verification:** 2 files / 36 groups PASS (16 body + 20 header), including
suspended cleanup and structured typed secondary failure assertions. Async inline
test and [example](../../examples/std-v4/http-body.ok) PASS.
Evidence. No compiler/runtime changes.

### `http/content-coding` — names VERIFIED (focused); body codecs BLOCKED by deferred Stage 16

**Зависимости и gate:** готовые streams/TLS/URI; framing и limits, immutable headers, explicit body ownership, malformed peers и cancellation.

- [x] **ContentCoding** — canonical lowercase HTTP token, explicit identity and
  open extensions; wildcard belongs to negotiation. InvalidContentCoding is typed.
  [Contract](http-content-coding.md), evidence.

  Кандидаты из v3 для пересмотра: `ContentCoding(origin is string)`; `string() is string`.

- [ ] **ContentEncodedBody** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ContentEncodedBody(origin is BodyOpening, coding is ContentCoding)`; `length() is BigInt`; `replayable() is bool`; `reading() is BodyReading`.

- [ ] **ContentDecodedBody** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ContentDecodedBody(origin is BodyOpening, coding is ContentCoding, maximum is BigInt)`; `length() is BigInt`; `replayable() is bool`; `reading() is BodyReading`.

- [ ] **UnsupportedContentCoding** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `UnsupportedContentCoding(origin is ContentCoding)`; `coding() is ContentCoding`; `text() is string`.

- [ ] Support identity, gzip, deflate and zstd through Stage 16.
- [ ] Apply stacked content codings in header order and decode in reverse order.
- [ ] Remove `Content-Length` when encoding changes the unknown wire length.
- [ ] Enforce decoded-size and expansion-ratio limits incrementally.
- [ ] Do not advertise Brotli until Stage 16 contains a Brotli codec.
- [ ] Do not silently decode a coding the client did not request.

**Verification:** 2 files / 9 groups PASS, inline test and
[example](../../examples/std-v4/http-content-coding.ok) PASS. Body codecs, stacking,
Content-Length rewrite and decompression budgets remain BLOCKED: current
compression roles/fakes are not gzip/deflate/zstd algorithms. No unavailable
codec is advertised, no silent decoding or fake identity-only pipeline added.

### `http/message` — targets/Request/Response metadata VERIFIED (focused); exchange NEXT

[Target contract](http-target.md): http/target implements actual origin/absolute/
authority/asterisk coordinates and method rules, using verified full URL values.
28 target groups / 70 with URL compatibility, inline/example PASS.
Evidence. Interpreter unchanged.
Origin-form owns path/query: //path cannot be represented faithfully by a generic
URI-reference. The withdrawn v3 reference() candidate is replaced by these explicit
coordinates. Nominal target snapshots validate wire against coordinates.

Request/Response and wire identities now pass 15 consumer groups / 59 with
target/body compatibility, inline/examples PASS. Evidence.
Construction never observes/opens the body role. Actual TLS/ALPN and connection
capabilities remain necessary for exchange metadata/negotiation, not message values.
No raw-string URL substitution or fake exchange provider is introduced.

- [x] **RequestTarget** — implemented in http/target; verified coordinate contract above.

  Кандидаты из v3 для пересмотра: `kind() is RequestTargetKind`; `url() is Url`; `reference() is UrlReference`; `authority() is UrlAuthority`; `string() is string`.

- [x] **OriginTarget** — implemented in http/target; verified coordinate contract above.

  Кандидаты из v3 для пересмотра: `OriginTarget(origin is UrlReference)`; `kind() is RequestTargetKind`; `url() is Url`; `reference() is UrlReference`; `authority() is UrlAuthority`; `string() is string`.

- [x] **AbsoluteTarget** — implemented in http/target; verified coordinate contract above.

  Кандидаты из v3 для пересмотра: `AbsoluteTarget(origin is Url)`; `kind() is RequestTargetKind`; `url() is Url`; `reference() is UrlReference`; `authority() is UrlAuthority`; `string() is string`.

- [x] **AuthorityTarget** — implemented in http/target; verified coordinate contract above.

  Кандидаты из v3 для пересмотра: `AuthorityTarget(origin is UrlAuthority)`; `kind() is RequestTargetKind`; `url() is Url`; `reference() is UrlReference`; `authority() is UrlAuthority`; `string() is string`.

- [x] **AsteriskTarget** — implemented in http/target; verified coordinate contract above.

  Кандидаты из v3 для пересмотра: `AsteriskTarget()`; `kind() is RequestTargetKind`; `url() is Url`; `reference() is UrlReference`; `authority() is UrlAuthority`; `string() is string`.

- [x] **Request** — immutable metadata with declared BodyOpening capability; implemented in http/message.

  Кандидаты из v3 для пересмотра: `Request(method is Method, target is RequestTarget, headers is Headers, body is BodyOpening, version is HttpVersion)`; `method() is Method`; `target() is RequestTarget`; `headers() is Headers`; `body() is BodyOpening`; `version() is HttpVersion`.

- [x] **Response** — immutable metadata with declared BodyOpening capability; implemented in http/message.

  Кандидаты из v3 для пересмотра: `Response(status is Status, headers is Headers, body is BodyOpening, version is HttpVersion)`; `status() is Status`; `headers() is Headers`; `body() is BodyOpening`; `version() is HttpVersion`.

- [x] **ExchangeDetails** — direct HTTP1 [client contract](http-client.md); metadata/owned pool/lifecycle/HTTPS focused PASS. Explicit H2 shared pool, redirects/cookies/scoped Basic/Bearer are verified; mixed negotiation, decoding, retry and other decorators remain open.

  Кандидаты из v3 для пересмотра: `endpoints() is ConnectionEndpoints<IpEndpoint>`; `secure() is bool`; `tls() is TlsDetails`; `version() is HttpVersion`.

- [x] **ExchangeHop** — direct HTTP1 [client contract](http-client.md); metadata/owned pool/lifecycle/HTTPS focused PASS. Explicit H2 shared pool, redirects/cookies/scoped Basic/Bearer are verified; mixed negotiation, decoding, retry and other decorators remain open.

  Кандидаты из v3 для пересмотра: `request() is Request`; `response() is Response`.

- [x] **ClientResponse** — direct HTTP1 [client contract](http-client.md); metadata/owned pool/lifecycle/HTTPS focused PASS. Explicit H2 shared pool, redirects/cookies/scoped Basic/Bearer are verified; mixed negotiation, decoding, retry and other decorators remain open.

  Кандидаты из v3 для пересмотра: `request() is Request`; `response() is Response`; `history() is Sequencing<ExchangeHop>`; `details() is ExchangeDetails`.

- [ ] **RequestContext** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `endpoints() is ConnectionEndpoints<IpEndpoint>`; `secure() is bool`; `tls() is TlsDetails`; `cancelling() is Cancelling`; `parameters() is %string:string`.

- [x] **WrongRequestTargetKind** — implemented in http/target; verified coordinate contract above.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] **PlainHttpHasNoTls** — direct HTTP1 [client contract](http-client.md); metadata/owned pool/lifecycle/HTTPS focused PASS. Explicit H2 shared pool, redirects/cookies/scoped Basic/Bearer are verified; mixed negotiation, decoding, retry and other decorators remain open.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] Throw `WrongRequestTargetKind` from an invalid target projection.
- [x] Throw `PlainHttpHasNoTls` from TLS observations on plaintext exchanges. (Direct HTTP1 profile; see http-client.md.)
- [x] Generate origin-form for normal direct client requests; origin servers must also parse absolute-form per RFC 9112. (Direct HTTP1 profile; see http-client.md.)
- [x] Require absolute-form for forward-proxy requests.
- [x] Require authority-form for CONNECT.
- [x] Require asterisk-form only where the method permits it.
- [x] Keep message objects immutable while body resources remain stateful.
- [x] Do not expose raw serialized request or status lines as public values (metadata APIs expose only coordinates).

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `http/date` — VERIFIED (focused)

[Contract](http-date.md): immutable whole-second UTC identity and strict bounded parsing.

- [x] HttpDate(Instant): floor fractional seconds, including before epoch;
  value() and Comparing<HttpDate> observe the normalized instant.
- [x] Emit IMF-fixdate only, literal GMT, fixed English names and spacing.
- [x] Parse IMF-fixdate, asctime and RFC850; weekday/calendar consistency checked.
- [x] RFC850 uses explicit reference Instant; absent reference is a typed error.
  Exact 50-year boundary and one second beyond tested without ambient clock I/O.
- [x] Reject unsupported leap seconds, invalid layouts/dates/times and years outside 1..9999.
- [x] Bound grammar input to 33 scalars; reuse Stage 10 calendar/time invariants.

**Verification:** 4 files / 106 groups PASS (HTTP, RFC3339, format, mail date);
inline test and [example](../../examples/std-v4/http-date.ok) PASS.
Evidence; no existing production module changed.

### `http/conditional` — validator/range values VERIFIED (focused); protocol consumers BLOCKED

[Contract](http-conditional.md). Pure validator values do not require TLS/URL or
framing. Protocol evaluation, date precedence and multipart serving remain open.

- [x] EntityTag: validated opaque octets and strength; exact representation;
  separate RFC strong/weak comparisons without reflection or unescaping.
- [x] EntityTags: wildcard/empty/list identity, ordered bounded snapshot from
  nominal Sequencing, duplicate preservation and member comparison.
- [x] ParsedEntityTags: deferred bounded recipient grammar, exact W/ and OWS,
  embedded commas/backslashes, ignored empty slots charged to the entry budget.
- [x] ConditionalLimits and typed offset/budget failures; input byte preflight
  and source size checked before item observation.

**Verification:** 3 files / 67 groups PASS (26 validator, 20 header, 21 date);
2 inline tests and [example](../../examples/std-v4/http-entity-tags.ok) PASS.
Evidence.
Interpreter unchanged; full regression omitted by the user's 2026-10-11 policy.

- [x] ByteRange: validated inclusive nonnegative BigInt coordinates, derived
  exact size, canonical identity and explicit representation.
- [x] ByteRanges/InvalidRange: bounded recipient bytes grammar, canonical unit
  and decimals, open/suffix/bounded forms and typed malformed input.
- [x] Normalize concrete intervals against a known nonnegative BigInt length,
  clipping endpoints and preserving RFC suffix satisfiability for size zero.
- [x] Sort and coalesce overlapping/adjacent intervals without narrowing offsets.
- [x] Apply caller-supplied ConditionalLimits to wire/canonical bytes and all raw
  list slots before coalescing; ignored empty slots still consume budget.

**Range verification:** 4 files / 106 groups PASS (41 ranges, 26 validators,
24 BigInt, 15 ordering). [Example](../../examples/std-v4/http-byte-ranges.ok) and three module inline tests
PASS. Evidence includes the RED
missing-declaration result, focused dependency checks and a 24-case independent
bitmap oracle. Interpreter unchanged; no full regression for this std-only slice.

**Protocol gate:** evaluation/304/412 precedence needs the actual request/selected
representation contracts, not raw-string substitutes for blocked `http/message`.
Serving multipart byte ranges needs actual representation/body length and framing
consumers. The value helpers do not claim a complete conditional HTTP handler.

- [ ] Use strong comparison for `If-Match` and weak comparison for `If-None-Match`.
- [ ] Give entity-tag conditions precedence over date conditions.
- [x] Normalize satisfiable byte ranges against the selected representation length (pure value plan).
- [x] Coalesce overlapping ranges in the normalized value plan; serving remains protocol work.
- [x] Reject excessive wire list slots through explicit caller-supplied limits before coalescing.
- [ ] Render multiple ranges through `multipart/byteranges`.
- [ ] Do not apply ranges to an unknown-length representation.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `http/negotiation` — bounded preference profiles VERIFIED (focused); response effects remain protocol work

[Contract](http-negotiation.md). Pure preference values need no streams/TLS/URI;
actual response selection and representation metadata remain protocol work.

- [x] Quality: exact canonical thousandths in 0..1, no rounding, acceptability,
  comparison and qvalue rendering; ParsedQuality uses the bounded ASCII grammar.
- [x] WeightedValue: validated HTTP token with Quality, default preference 1,
  preserved spelling and explicit weighted representation.
- [x] InvalidQuality/InvalidWeightedValue: typed grammar, precision and token errors.
- [x] All 1001 valid quality values round-trip exactly; illegal signs, spacing,
  exponents, Unicode digits, range and excessive precision are rejected.

**Verification:** 3 files / 52 groups PASS (42 new quality, 5 rat, 5 method);
2 inline tests and [example](../../examples/std-v4/http-quality.ok) PASS.
Evidence. Interpreter unchanged;
no full regression under the user's 2026-10-11 policy.

- [x] AcceptedContentCodings: explicit absent/present-empty identity, bounded
  Accept-Encoding grammar, exact/wildcard/identity rules and immutable entries.
- [x] Highest positive quality then client/server order; selection returns only
  a supplied ContentCoding. Independent candidate-table oracle covers 24 cases.
- [x] NoAcceptableValue, MissingNegotiationField, InvalidNegotiation and budget
  errors; raw slots and available source size preflight.
- [x] Immutable Sorted/LowerBound index preserves client positions and rejects
  duplicate canonical names without repeated full preference scans.
- [x] HTTP media-value prerequisite: HttpMediaType/parameters, strict HTTP
  octet/quoted-string grammar, canonical ordered identity, bounded source snapshots
  and typed failures. Separate from MIME RFC2231 wire interpretation.

**Media-value verification:** 3 files / 101 groups PASS (37 HTTP media, 20 header,
44 existing MIME); inline and [example](../../examples/std-v4/http-media-type.ok) PASS.
Parameter lookup RED: 16/128 name observations; GREEN: 5/8 via LowerBound.
[Contract](http-media-type.md), evidence.
Interpreter unchanged; no full regression under the user's 2026-10-11 policy.

- [x] Media type negotiation: bounded ranges, parameter specificity, exact-zero
  precedence and supplied-only selection; q recognized in any parameter order.
- [x] Charset negotiation: explicit canonical names, strict weighted tokens,
  exact/wildcard matching, supplied-only selection and no implicit legacy charset.
- [x] Language negotiation: explicit basic-filtering policy in locale/match,
  longest-range quality, exact zero, specificity/client/server ties and supplied-only Locale selection.

The unqualified v3 AcceptedValues candidate is withdrawn: each field has its own
nominal contract and wildcard/absence semantics. Content coding is ready without
advertising unavailable Stage 16 codecs. All four bounded preference profiles are ready; response effects remain protocol work.

**Content-coding verification:** 5 files / 114 groups PASS (33 selection,
42 quality, 4 coding, 20 header, 15 ordering); three inline tests and
[example](../../examples/std-v4/http-accepted-codings.ok) PASS.
Evidence. Interpreter unchanged;
no full regression under the user's 2026-10-11 policy.

**Media-profile verification:** 4 files / 138 groups PASS (26 media selection,
37 HTTP media values, 33 coding, 42 quality); four inline tests and
[example](../../examples/std-v4/http-accepted-media.ok) PASS.
Evidence. Includes the RFC
quality table, 24 independent ranking cases, quoted separators, literal extension
parameters, strict limits and source preflight. Interpreter unchanged; no full
regression under the user's 2026-10-11 policy.

**Charset-profile verification:** 4 files / 123 groups PASS (22 charset,
33 coding, 26 media selection, 42 quality); five inline tests and
[example](../../examples/std-v4/http-accepted-charsets.ok) PASS.
Evidence. Shared binary
lookup refactored with coding behavior preserved; 24 independent ranking cases.
Interpreter unchanged; no full regression under the user's 2026-10-11 policy.

**Language-profile verification:** 5 files / 126 groups PASS (31 language,
22 charset, 33 coding, 26 media selection, 14 existing locale); six HTTP inline
tests, two locale-match inline tests and [example](../../examples/std-v4/http-accepted-languages.ok) PASS.
Evidence. Includes grammar
bounds/offsets, hyphen matching, source preflight and 24 independent ranking cases.
After local refactoring: 31 language groups and inline/example checks PASS.
Interpreter unchanged; no full regression under the user's 2026-10-11 policy.

- [x] Validate quality values in `0..1` with at most three decimal places, exactly without float rounding.
- [x] Preserve client order between equal quality values in the content-coding profile; remaining profiles must retain this rule.
- [x] Apply specificity before original order for media ranges; equal specificity
  retains the first client occurrence, regardless of its quality.
- [x] Support bounded media type, content coding, charset and language preference profiles.
- [x] Treat explicit quality zero as unacceptable in all four preference profiles.
- [x] Keep basic language matching in Stage 26 locale/match; HTTP weight/tie policy stays explicit.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `http/cookie` — values/date/policy/memory/persistence VERIFIED (focused)

[Expiry contract](http-cookie.md): CookieExpiry is explicit session absence or a
supplied absolute Instant, retained at its existing precision. No clock query,
HTTP-date rounding or epoch sentinel. MissingCookieExpiry is the typed absence.
Verification: 3 files / 17 groups PASS (4 expiry, 8 duration, 5 clock), one inline
test and [example](../../examples/std-v4/http-cookie-expiry.ok) PASS.
Evidence. Interpreter unchanged;
no full regression under the user's 2026-10-11 policy.

Cookie and CookieAttributes: bounded pair/attribute grammar, canonical IDNA
Domain, snapshot flag roles, Secure/SameSite/Partitioned and case-insensitive
prefix checks. Default diagnostics redact values. 3 focused files / 48 groups,
two inline tests and executable example PASS; interpreter unchanged.
Values evidence.
Actual Url and pinned PSL prerequisites are ready. Parsing/date/context and jar
are the next slices, with explicit origin, reference time and caller context.
Cookie-date prerequisite is now verified: ParsedCookieDate in http/cookie_date,
25 consumer groups plus 21 HTTP-date compatibility groups; inline/example PASS.
Date evidence. Set-Cookie
interpretation now passes: ParsedSetCookie/CookieContext in http/cookie_policy.
3 files / 75 groups, inline/example PASS. Evidence.
MemoryCookieJar now passes 10 consumer groups / 37 with Set-Cookie compatibility;
inline/example PASS. Evidence.
Persistence now supports exact immutable snapshots, private same-filesystem staging,
schema/policy/identity validation, session exclusion and tested failure cleanup.
Evidence.
PERF-13 investigates remaining VM overhead.

- [x] **CookieExpiry** — explicit session/absolute value; verified contract above.

  Кандидаты из v3 для пересмотра: `CookieExpiry()`; `CookieExpiry(origin is Instant)`; `persistent() is bool`; `instant() is Instant`.

- [x] **MissingCookieExpiry** — typed session absence, stable text().

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] **CookieAttributes** — implemented; see bounded values contract.

  Кандидаты из v3 для пересмотра: `CookieAttributes(domain is string, path is string, expiry is CookieExpiry, same_site is SameSite, flags is Set<CookieFlag>)`; `domain() is string`; `path() is string`; `expiry() is CookieExpiry`; `same_site() is SameSite`; `flags() is Set<CookieFlag>`.

- [x] **Cookie** — implemented; see bounded values contract.

  Кандидаты из v3 для пересмотра: `Cookie(name is string, value is string, attributes is CookieAttributes)`; `name() is string`; `value() is string`; `attributes() is CookieAttributes`; `string() is string`.

- [x] **CookieContext** — explicit schemeful site/navigation/method; implemented in http/cookie_policy.

  Кандидаты из v3 для пересмотра: `CookieContext(site is Url, top_level is bool, method is Method)`; `site() is Url`; `top_level() is bool`; `method() is Method`.

- [x] **CookieStoring** — implemented in http/cookie_jar with explicit Timing and snapshot/publication contract.

  Кандидаты из v3 для пересмотра: `cookies(origin is Url, context is CookieContext) is Sequencing<Cookie>`; `store(origin is Url, headers is Headers, context is CookieContext) is void`; `clear() is void`.

- [x] **MemoryCookieJar** — implemented in http/cookie_jar with explicit Timing and snapshot/publication contract.

  Кандидаты из v3 для пересмотра: `MemoryCookieJar(clock is Timing)`; `cookies(origin is Url, context is CookieContext) is Sequencing<Cookie>`; `store(origin is Url, headers is Headers, context is CookieContext) is void`; `clear() is void`.

- [x] **SavedCookieJar** — explicit bounded domain protocol and filesystem command; implemented in http/cookie_jar.

  Кандидаты из v3 для пересмотра: `SavedCookieJar(origin is MemoryCookieJar, destination is FileEntry)`; `result() is FileEntry`.

- [x] **LoadedCookieJar** — explicit bounded domain protocol and filesystem command; implemented in http/cookie_jar.

  Кандидаты из v3 для пересмотра: `LoadedCookieJar(origin is FileEntry, clock is Timing)`; `result() is MemoryCookieJar`.

- [x] **InvalidCookie** — implemented; see bounded values contract.

  Кандидаты из v3 для пересмотра: `InvalidCookie(reason is string)`; `text() is string`.

- [x] Implement the documented bounded strict 6265bis draft-22 domain/path/expiry/prefix consumer profile.
- [x] Make zero-argument `CookieExpiry` represent a session cookie.
- [x] Make `CookieExpiry.instant()` throw `MissingCookieExpiry` for a session cookie.
- [x] Give Max-Age precedence over Expires and store one absolute expiry instant.
- [x] Use a pinned public-suffix list for domain-cookie rejection.
- [x] Require Secure for SameSite=None, Partitioned and `__Secure-` names.
- [x] Enforce the `__Host-` prefix rules.
- [x] Reject control characters and separator injection.
- [x] Apply per-domain and total cookie limits with deterministic eviction.
- [x] Order outbound cookies by descending path length then creation time.
- [x] Never merge multiple Set-Cookie fields.
- [x] Redact cookie values from default diagnostics.
- [x] Persist `SavedCookieJar` atomically through an explicit effect.
- [x] Never persist session cookies.
- [x] Do not let a cookie jar decide browser-site context implicitly.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `http/protocol` — HTTP1 stream sessions/continue VERIFIED; H2 NEXT

Bounded message/body framing now compose into [owned stream sessions](http1-session.md).
Session tests: 431 (18), owner/error 432 (3), message writer 426 (11): 3 files /
32 groups PASS; inline and two real exchanges on one TCP connection PASS.
Verified EOF releases admission; early close/error/cancel discards transport.
Continue coordination now validates before head emission, defers opening until
100, preserves early-final sources and bounds waiting across 103 with an injected
absolute deadline. [Contract](http1-continue.md); real TCP handshake PASS.
Dedicated upgrades, Internet routing/TLS and pooling remain
follow-up work; no fabricated ExchangeDetails or HTTP2 negotiation is claimed.

[HTTP1 head contract](http1-framing.md): exact request/status lines, CRLF,
logical fields/duplicates, Host authority and wire/line/count budgets verified.
30 consumer groups / 65 with message/header compatibility, inline/example PASS.
Evidence. Async head reading now
passes 48 consumer groups, inline/example, preserving exact binary remainder and
structured cancellation cleanup. Reader evidence.
[Body boundary decisions](http1-body-framing.md) now verified: 45 consumer groups,
91 with compatibility, inline/example PASS. Chunked stream consumption and HTTP2
follow; no Internet connection provider is claimed.
[Chunk/trailer metadata](http1-body-framing.md#chunk-lines-and-trailer-sections):
59 consumer / 134 compatibility groups, inline/example PASS.
[Owned body consumption](http1-body-reading.md): final 22 focused groups PASS,
85 head/body compatibility groups, inline/Unix example PASS.
[Canonical head writing](http1-framing.md#canonical-borrowed-head-writing):
8 consumer / 38 compatibility groups, inline/round-trip example PASS.
[Body writing](http1-body-writing.md): lazy one-shot source, exact fixed bytes,
chunked/trailers, cumulative payload/metadata budgets and bounded current groups
verified: final 15 focused / 43 compatibility groups, inline/round-trip PASS.
[Complete-message output](http1-message-writing.md) checks known body metadata
before any head emission: 11 focused / 26 compatibility groups, inline/round-trip
PASS. Owned stream exchanges/leases are now verified above; Internet routing,
TLS, pooling and HTTP2 remain follow-up work.

- [x] **Http2Settings** — bounded local advertisements and internal wire/reader foundation; [contract](http2-wire-foundation.md). Full H2 engine remains open.

  Кандидаты из v3 для пересмотра: `Http2Settings(header_table is int, initial_window is int, frame_size is int, streams is int)`; `header_table() is int`; `initial_window() is int`; `frame_size() is int`; `streams() is int`.

- [x] Implement strict HTTP/1.1 receiving: request-line, status-line, fields and bounded chunk/trailer framing; encoding follows.
- [x] Require exactly one valid Host field on HTTP/1.1 requests.
- [x] Require HTTP/2 authority and Host to agree after canonical host/default-port normalization on input/output; [fields](http2-headers.md).
- [x] Treat close-delimited response body plans as non-reusable; actual lease enforcement follows.
- [x] Reject conflicting Content-Length values unless byte-for-byte identical.
- [x] Reject Transfer-Encoding plus Content-Length on requests (strict ordinary responses too).
- [x] Require chunked as the final supported transfer coding; initial payload profile supports chunked alone.
- [x] Parse chunk extensions within limits and ignore unknown safe extensions.
- [x] Validate trailer fields against forbidden framing/routing/auth/control names and Connection nominations.
- [x] Internal client preface: exact magic/SETTINGS, server first SETTINGS application and ACK; [contract](http2-preface.md), focused 477..479 verified.
- [ ] Owning HTTP2 connection SETTINGS ACK tracking/update effects and deadlines; full handshake/session integration.
- [x] Internal frame header/payload structural rules and incremental reading, including error scope/alignment.
- [x] Bounded HEADERS/CONTINUATION assembly/output and typed field mapping; [contract](http2-headers.md), 483..485 verified.
- [x] Internal consuming connection reader, initial ACK tracking and queued replies; [input](http2-input.md), 490 verified.
- [ ] Live consuming DATA, SETTINGS, WINDOW_UPDATE, RST_STREAM, PING and GOAWAY connection/stream behavior, including header-block scheduler integration.
- [ ] Reject PUSH_PROMISE; server push is outside the first API.
- [x] Implement bounded HPACK static/dynamic decoding and conservative encoder; [contract](hpack-blocks.md), 481/482 verified.
- [x] HPACK encoder marks auth/cookies and explicitly sensitive fields never-indexed.
- [x] HPACK decoded field-list/count budgets include every indexed repeat and per-field overhead; overflow aborts connection.
- [x] Pure signed flow windows, padded DATA charges, scoped overflow and dual send reservation; [accounting](http2-flow.md), 486 verified.
- [x] Internal incoming body buffer, padding/consumption credit and one-shot lifetime; [body](http2-body.md), 494 verified.
- [x] Internal aggregate incoming ledger plus actual TCP consumption WINDOW_UPDATE; [receiving](http2-receiving.md), 495/496 verified.
- [ ] Enforce live connection/stream flow with async credit notifications, bounded backpressure and no deadlock.
- [x] Internal bounded connection writer and cancellation-isolated header/control tickets; [output](http2-output.md), 491 verified.
- [x] Internal outbound DATA scheduler offers one frame per ready producer FIFO; [sending](http2-sending.md), 493 and actual TCP 492 verified. Owning session integration follows.
- [ ] Distinguish connection errors from stream errors.
- [x] Pure GOAWAY cutoff/admission metadata, exact RST/PING values and bounded diagnostic emission; [controls](http2-controls.md), 487 verified.
- [ ] Stop creating live streams after GOAWAY and retry only eligible unprocessed requests.
- [x] Pure stream half-close/reset/minimal late-frame state and monotone client IDs; [lifecycle](http2-stream-life.md), 489 verified.
- [ ] Send actual RST_STREAM on live stream cancellation without cancelling sibling streams.
- [x] Frame/HPACK/head collaborators stay internal; no stable public raw frame/HPACK API.
- [x] Typed HTTP2 request/response mappings keep pseudo-fields outside ordinary Headers; body remains unobserved.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `http/client` — direct HTTP1 client/pool VERIFIED; H2/decorators next

[Checked endpoint contract](http-endpoints.md): 12 focused assertions, inline
and deterministic example PASS. This filter acquires no connection.
[Http1Connecting](http1-connecting.md) opens actual HTTP/HTTPS sessions using exact
checked endpoints and URL-derived identity: 453/454 15 assertions and inline PASS.
TcpRacing now resolves the checked-route sequential PERF-18 case. Strict resolver
profile/DNS streaming remain open. Direct HTTP1 owned pools are now verified;
H2 multiplexing, stale retry and decorators remain open.

**Зависимости и gate:** готовые streams/TLS/URI; framing и limits, immutable headers, explicit body ownership, malformed peers и cancellation.

- [x] **Exchanging** — direct HTTP1 [client contract](http-client.md); metadata/owned pool/lifecycle/HTTPS focused PASS. Explicit H2 shared pool, redirects/cookies/scoped Basic/Bearer are verified; mixed negotiation, decoding, retry and other decorators remain open.

  Кандидаты из v3 для пересмотра: `async response(request is Request) is ClientResponse`.

- [x] **EndpointAllowing** — explicit URL plus exact numeric destination decision; no lookup/socket capability.

  Кандидаты из v3 для пересмотра: `allowed(url is Url, endpoint is IpEndpoint) is bool`.

- [x] **AllEndpoints** — explicit unrestricted policy.

  Кандидаты из v3 для пересмотра: `AllEndpoints()`; `allowed(url is Url, endpoint is IpEndpoint) is bool`.

- [x] **PublicEndpoints** — affirmative pinned global unicast, excluding all special/scoped categories.

  Кандидаты из v3 для пересмотра: `PublicEndpoints()`; `allowed(url is Url, endpoint is IpEndpoint) is bool`.

- [x] **ListedNetworks** — bounded immutable CIDR snapshot; empty denies all.

  Кандидаты из v3 для пересмотра: `ListedNetworks(origins is Sequencing<IpNetwork>)`; `allowed(url is Url, endpoint is IpEndpoint) is bool`.

- [x] **HttpLimits** — bounded explicit inputs; [contract](http-client-policy.md), interpreter/459 + inline PASS. Consumed by the verified direct HTTP1 client.

  Кандидаты из v3 для пересмотра: `HttpLimits()`; `HttpLimits(header_bytes is int, line_bytes is int, body_bytes is BigInt)`; `header_bytes() is int`; `line_bytes() is int`; `body_bytes() is BigInt`.

- [x] **ClientTimeouts** — bounded explicit inputs; [contract](http-client-policy.md), interpreter/459 + inline PASS. Consumed by the verified direct HTTP1 client.

  Кандидаты из v3 для пересмотра: `ClientTimeouts(connection is Duration, headers is Duration, body_idle is Duration)`; `connection_timeout() is Duration`; `headers_timeout() is Duration`; `body_idle_timeout() is Duration`.

- [x] **ClientPolicy** — bounded explicit inputs; [contract](http-client-policy.md), interpreter/459 + inline PASS. Consumed by the verified direct HTTP1 client.

  Кандидаты из v3 для пересмотра: `ClientPolicy(protocols is Set<HttpVersion>, limits is HttpLimits, timeouts is ClientTimeouts, connections is int, access is EndpointAllowing)`; `protocols() is Set<HttpVersion>`; `limits() is HttpLimits`; `timeouts() is ClientTimeouts`; `connections() is int`; `access() is EndpointAllowing`.

- [x] **HttpClient** — direct HTTP1 [client contract](http-client.md); metadata/owned pool/lifecycle/HTTPS focused PASS. Explicit H2 shared pool, redirects/cookies/scoped Basic/Bearer are verified; mixed negotiation, decoding, retry and other decorators remain open.

  Кандидаты из v3 для пересмотра: `HttpClient(resolver is Resolving, tls is TlsClientPolicy, policy is ClientPolicy, clock is Timing)`; `async response(request is Request) is ClientResponse`; `closed() is bool`; `close() is void`.

- [x] **Retrying / HttpStaleConnection** — [explicit contract](http-retry.md), real TCP/mixed H1/verified TLS and compatibility PASS.

- [ ] **Decoding** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Decoding(origin is Exchanging, codings is Sequencing<ContentCoding>, maximum is BigInt)`; `async response(request is Request) is ClientResponse`.

- [ ] **HttpFailure** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `HttpFailure(message is string)`; `text() is string`.

- [x] Reject negative client timeouts and make zero explicitly disable that deadline. (Direct HTTP1 profile; see http-client.md.)
- [x] Resolve only HTTP and HTTPS absolute targets in the network client. (Direct HTTP1 profile; see http-client.md.)
- [x] Reject an empty enabled-protocol set and non-positive limits or connection counts. (Direct HTTP1 profile; see http-client.md.)
- [x] Validate that HTTP protocol preference and TLS ALPN policy do not contradict each other. (Direct HTTP1 profile; see http-client.md.)
- [x] Check every resolved candidate through EndpointAllowing in HttpEndpointSelection; the connector follows.
- [x] Connect only to checked numeric addresses through Http1Connecting, without a second lookup.
- [x] Make PublicEndpoints reject loopback/private/link-local/multicast/unspecified/reserved/documentation/non-global/scoped ranges.
- [x] Apply endpoint policy to every redirect URL, including the actual peer of a reused connection.
- [x] Apply endpoint policy after every proxy decision.
- [x] Derive the selected endpoint port only from positive explicit port or HTTP/HTTPS defaults, rejecting resolver substitution.
- [x] Session opening verifies HTTPS through its URL DNS/IP host. Redirect handling must create a fresh route; that consumer remains open.
- [x] Pool connections by scheme, host, port, proxy and TLS policy. (Direct HTTP1 profile; see http-client.md.)
- [x] Reuse HTTP/1.1 connections only after clean body completion or bounded drain. (Direct HTTP1 profile; see http-client.md.)
- [x] Multiplex compatible HTTP/2 exchanges on one connection. Explicit H2 profile: [shared owner contract](http2-client.md), actual TCP/TLS and cancellation/deadline/GOAWAY tests PASS. Mixed negotiation remains separate.
- [x] Bound idle and total connections per pool key. (Direct HTTP1 profile; see http-client.md.)
- [x] Evict idle connections deterministically. (Direct HTTP1 profile; see http-client.md.)
- [x] Retry a stale pooled connection only for replayable idempotent requests. [Optional H1 contract](http-retry.md); H2 unprocessed replay stays separate.
- [x] Never retry after observing response bytes. Includes partial and informational heads; evidence.
- [x] Handle `100 Continue` without sending a large body prematurely. (Direct HTTP1 profile; see http-client.md.)
- [x] Ignore unsupported informational responses except `101` protocol switching. (Direct HTTP1 profile; see http-client.md.)
- [x] Expose an empty body for HEAD, 1xx, 204 and 304 responses regardless of framing fields. (Direct HTTP1 profile; see http-client.md.)
- [x] Treat successful CONNECT as a tunnel transition only inside typed proxy handling.
- [x] Apply header and body limits incrementally. (Direct HTTP1 profile; see http-client.md.)
- [x] Apply connect, response-header and body-idle deadlines independently. (Direct HTTP1 profile; see http-client.md.)
- [x] Return once response headers arrive; stream the body afterward. (Direct HTTP1 profile; see http-client.md.)
- [x] Make closing the client cancel pending exchanges and close pooled connections. (Direct HTTP1 profile; see http-client.md.)
- [x] Do not enable redirects, cookies, authentication or cache implicitly. (Direct HTTP1 profile; see http-client.md.)
- [ ] Make `Decoding` advertise only its configured supported codings.
- [ ] Make `Decoding` update Content-Encoding and Content-Length after transformation.
- [x] Do not add a default User-Agent field; callers and higher-level clients choose one explicitly. (Direct HTTP1 profile; see http-client.md.)

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `http/redirect` — VERIFIED (focused and actual loopback)

**Зависимости и gate:** готовые streams/TLS/URI; framing и limits, immutable headers, explicit body ownership, malformed peers и cancellation.

- [x] **RedirectPolicy** — [explicit redirect contract](http-redirect.md), 465..467 focused/loopback verified.

  Кандидаты из v3 для пересмотра: `RedirectPolicy(maximum is int, https_downgrade is bool)`; `maximum() is int`; `https_downgrade() is bool`.

- [x] **Redirecting** — [explicit redirect contract](http-redirect.md), 465..467 focused/loopback verified.

  Кандидаты из v3 для пересмотра: `Redirecting(origin is Exchanging, policy is RedirectPolicy)`; `async response(request is Request) is ClientResponse`.

- [x] **TooManyRedirects** — [explicit redirect contract](http-redirect.md), 465..467 focused/loopback verified.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] **RedirectLoop** — [explicit redirect contract](http-redirect.md), 465..467 focused/loopback verified.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] Resolve relative Location values against the effective request URL.
- [x] Detect loops from canonical effective URLs.
- [x] Implement 301, 302 and 303 method rewriting explicitly.
- [x] Preserve method and body for 307 and 308.
- [x] Reject replay of a non-replayable body.
- [x] Strip authorization and cookie fields across origins.
- [x] Forbid HTTPS-to-HTTP downgrade by default.
- [x] Preserve every intermediate response in `ClientResponse.history()`.
- [x] Do not interpret a missing Location as a redirect.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `http/auth` / `http/digest` — Basic/Bearer and SHA-256 Digest VERIFIED; additional profiles open

**Зависимости и gate:** готовые streams/TLS/URI; framing и limits, immutable headers, explicit body ownership, malformed peers и cancellation.

- [x] **BasicCredentials** — [secret](secret-bytes.md)/[scoped auth](http-auth.md), focused/actual HTTPS plus full 672/12185 PASS.

  Кандидаты из v3 для пересмотра: `BasicCredentials(username is string, password is SecretBytes)`; `username() is string`.

- [x] **DigestCredentials** — import from `http/digest`; [SHA-256 contract](http-digest.md), focused/independent HTTPS PASS. Additional profiles remain open.

  Кандидаты из v3 для пересмотра: `DigestCredentials(username is string, password is SecretBytes)`; `username() is string`.

- [x] **BearerAuthorization** — [secret](secret-bytes.md)/[scoped auth](http-auth.md), focused/actual HTTPS plus full 672/12185 PASS.

  Кандидаты из v3 для пересмотра: `BearerAuthorization(token is SecretBytes)`.

- [x] **BasicExchanging** — [secret](secret-bytes.md)/[scoped auth](http-auth.md), focused/actual HTTPS plus full 672/12185 PASS.

  Кандидаты из v3 для пересмотра: `BasicExchanging(origin is Exchanging, scope is Url, credentials is BasicCredentials)`; `async response(request is Request) is ClientResponse`.

- [x] **DigestExchanging** — import from `http/digest`; [SHA-256 contract](http-digest.md), focused/independent HTTPS PASS. Additional profiles remain open.

  Кандидаты из v3 для пересмотра: `DigestExchanging(origin is Exchanging, scope is Url, credentials is DigestCredentials, random is SecureRandom)`; `async response(request is Request) is ClientResponse`.

- [x] **BearerExchanging** — [secret](secret-bytes.md)/[scoped auth](http-auth.md), focused/actual HTTPS plus full 672/12185 PASS.

  Кандидаты из v3 для пересмотра: `BearerExchanging(origin is Exchanging, scope is Url, authorization is BearerAuthorization)`; `async response(request is Request) is ClientResponse`.

- [x] Limit preemptive credentials to the configured origin and path scope.
- [x] Strip credentials on every cross-origin redirect.
- [x] Support Basic only over verified TLS by default.
- [x] Support Digest SHA-256 and SHA-512-256 with fresh client nonces. Both session variants and explicit UTF-8/NFC are verified; evidence.
- [ ] Do not add new MD5 Digest credentials; parse legacy challenges only under explicit compatibility policy.
- [x] Prevent bearer and password values from entering diagnostics.
- [ ] Do not implement NTLM or negotiate platform credentials in the portable stdlib.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `http/cookie-client` — VERIFIED (focused and actual redirect wire)

**Зависимости и gate:** готовые streams/TLS/URI; framing и limits, immutable headers, explicit body ownership, malformed peers и cancellation.

- [x] **CookieContextualizing** — [explicit cookie consumer](http-cookie-client.md), 468..470 focused/actual wire verified.

  Кандидаты из v3 для пересмотра: `context(request is Request) is CookieContext`.

- [x] **FirstPartyCookies** — [explicit cookie consumer](http-cookie-client.md), 468..470 focused/actual wire verified.

  Кандидаты из v3 для пересмотра: `FirstPartyCookies()`; `context(request is Request) is CookieContext`.

- [x] **CookieExchanging** — [explicit cookie consumer](http-cookie-client.md), 468..470 focused/actual wire verified.

  Кандидаты из v3 для пересмотра: `CookieExchanging(origin is Exchanging, jar is CookieStoring, contexts is CookieContextualizing)`; `async response(request is Request) is ClientResponse`.

- [x] Make `FirstPartyCookies` explicitly treat every request as a top-level first-party request.
- [x] Select matching cookies immediately before each exchange.
- [x] Store every Set-Cookie field from the effective response URL.
- [x] Re-evaluate cookies for every hop with Redirecting(CookieExchanging(...)); wrapper ordering is explicit in the contract.
- [x] Preserve an explicit caller Cookie field only under an explicit conflict rule.
- [x] Serialize concurrent cookie-jar changes safely.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `http/proxy` — VERIFIED

**Зависимости и gate:** готовые streams/TLS/URI; framing и limits, immutable headers, explicit body ownership, malformed peers и cancellation.

- [x] **ProxyPolicy** — контракт реализован и проверен.

  Реализованный контракт: `ProxyPolicy(origin is Url)`; `ProxyPolicy(origin is Url, credentials is BasicCredentials)`; `url() is Url`; `authenticated() is bool`; `credentials() is BasicCredentials`.

- [x] **MissingProxyCredentials** — контракт реализован и проверен.

  Реализованный контракт: `text() is string`.

- [x] **Proxied** — контракт реализован и проверен.

  Реализованный контракт: `Proxied(proxy is ProxyPolicy, resolver is Resolving, tls is TlsClientPolicy, policy is ClientPolicy, clock is Timing)`; `async response(request is Request) is ClientResponse`; `closed() is bool`; `close() is void`.

- [x] **EnvironmentProxy** — контракт реализован и проверен.

  Реализованный контракт: `EnvironmentProxy(environment is Environment)`; `proxy(origin is Url) is ProxyPolicy`.

- [x] **MissingProxy** — контракт реализован и проверен.

  Реализованный контракт: `text() is string`.

- [x] Send absolute-form targets to a plaintext HTTP proxy.
- [x] Establish HTTPS through CONNECT before the origin TLS handshake.
- [x] Verify the origin certificate after tunneling.
- [x] Keep proxy authorization separate from origin authorization.
- [x] Make `ProxyPolicy.credentials()` throw `MissingProxyCredentials` when absent.
- [x] Match no-proxy host suffixes, exact IP addresses and CIDR networks explicitly.
- [x] Apply `ClientPolicy.access()` to the actual proxy endpoint.
- [x] Document that a remotely resolving proxy owns origin-address filtering after CONNECT.
- [x] Parse environment proxy variables only through `EnvironmentProxy`.
- [x] Never trust ambient proxy variables unless the caller constructs `EnvironmentProxy`.
- [x] Do not support SOCKS until a separate typed transport module exists.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

[Проверенный контракт и ограничения](http-proxy.md): 37 файлов /339 проверок
PASS, inline PASS. TLS к самому proxy и SOCKS отдельно; PERF-26 открыт.

### `http/cache` — VERIFIED private GET200 profile

[Contract](http-cache.md), [metadata](http-cache-metadata.md),
final evidence.

- [x] ResponseCaching/MemoryHttpCache/Caching and typed local misses: completed
  entries replace the withdrawn synchronous store(Request,live ClientResponse).
- [x] RFC9111 explicit freshness/age, Date/Age/Expires/Cache-Control and Vary.
- [x] No-store refusal before sanitization; explicit authenticated policy and partitions.
- [x] Publish only clean complete bounded bodies; preserve trailers, refuse partials.
- [x] Conditional ETag/Last-Modified validation and identified 304 metadata merge.
- [x] Successful unsafe target invalidation and late-publication revision barriers.
- [x] Concurrent current validation collapse, including existing no-cache waiters.
- [x] Explicit max-stale constrained by must-revalidate and valid metadata.
- [x] Private behavior; no shared-cache or implicit network-error stale fallback.

Conservative scope: complete GET200, caller preconditions/ranges delegated,
only-if-cached returns typed MissingCachedResponse without fabricated endpoints.
Heuristic freshness, disk/shared caching and additional cacheable statuses are
separate extensions. Eleven files /59 final cache assertions PASS, actual TCP;
HeaderName/client compatibility independently 58/523 PASS. Native unchanged/no full.

### `http/websocket` — PARTIAL: HTTP1 Upgrade transport verified

Current object profile and tests: [WebSocket](websocket.md),
13 files /70 assertions evidence.
Values, framing, controls/fragments, direct checked TCP/verified TLS, bounded
queues/close and explicit server Origin Upgrade are implemented. Withdrawn v3
signature list below is historical design input, not the current API. Full HTTP
server integration, explicit compression and RFC8441 remain open. PERF-29 tracks
measured mask cost.

**Зависимости и gate:** готовые streams/TLS/URI; framing и limits, immutable headers, explicit body ownership, malformed peers и cancellation.

- [ ] **WebSocketMessage** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `kind() is WebSocketMessageKind`; `text() is string`; `bytes() is @byte`.

- [ ] **TextWebSocketMessage** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TextWebSocketMessage(origin is string)`; `kind() is WebSocketMessageKind`; `text() is string`; `bytes() is @byte`.

- [ ] **BinaryWebSocketMessage** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `BinaryWebSocketMessage(origin is @byte)`; `kind() is WebSocketMessageKind`; `text() is string`; `bytes() is @byte`.

- [ ] **WebSocketLimits** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `WebSocketLimits(message_bytes is BigInt, frame_bytes is int, queue is int)`; `message_bytes() is BigInt`; `frame_bytes() is int`; `queue() is int`.

- [ ] **WebSocketConnection** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `async message() is WebSocketMessage`; `async send(origin is WebSocketMessage) is WebSocketConnection`; `async ping(origin is @byte) is WebSocketConnection`; `async close(code is int, reason is string) is WebSocketConnection`; `closed() is bool`.

- [ ] **WebSocketMessageIsNotText** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] **WebSocketMessageIsNotBinary** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `text() is string`.

- [ ] **WebSocketHandling** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `async handle(connection is WebSocketConnection, request is Request, context is RequestContext) is WebSocketHandling`.

- [ ] **WebSocketAllowing** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `allowed(request is Request) is bool`.

- [ ] **SameOriginWebSockets** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SameOriginWebSockets()`; `allowed(request is Request) is bool`.

- [ ] **WebSocketUpgrade** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `WebSocketUpgrade(protocols is Sequencing<string>, handling is WebSocketHandling, limits is WebSocketLimits, access is WebSocketAllowing)`; `protocols() is Sequencing<string>`; `handling() is WebSocketHandling`; `limits() is WebSocketLimits`; `access() is WebSocketAllowing`.

- [ ] **WebSocketConnecting** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `WebSocketConnecting(origin is Exchanging, url is Url, protocols is Sequencing<string>, headers is Headers, random is SecureRandom, limits is WebSocketLimits)`; `async connection() is WebSocketConnection`.

- [ ] **WebSocketClosed** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `code() is int`; `reason() is string`; `text() is string`.

- [ ] **WebSocketFailure** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `WebSocketFailure(reason is string)`; `text() is string`.

- [ ] Validate HTTP Upgrade, Connection, version, nonce and accept fields.
- [ ] Check server-side Origin policy before sending the switching response.
- [ ] Make `SameOriginWebSockets` reject absent, malformed and cross-origin browser origins.
- [ ] Generate every client nonce from `SecureRandom`.
- [ ] Mask every client-to-server frame and reject masked server frames.
- [ ] Require minimal payload-length encoding.
- [ ] Reassemble fragmented data messages within limits.
- [ ] Validate text messages and close reasons as UTF-8.
- [ ] Throw a typed variant error from the wrong `WebSocketMessage` projection.
- [ ] Handle ping, pong and close frames internally while fragments are open.
- [ ] Enforce control-frame finality and 125-byte payload limit.
- [ ] Validate registered close codes and reason length.
- [ ] Accept at most one server-selected subprotocol and require that the client offered it.
- [ ] Complete the close handshake once and then close the transport.
- [ ] Apply bounded queues and backpressure to outgoing messages.
- [ ] Cancel pending send and receive operations when closing.
- [ ] Negotiate per-message deflate only with explicit policy and expansion limits.
- [ ] Support HTTP/1.1 Upgrade first and RFC 8441 extended CONNECT after HTTP/2 is stable.
- [ ] Exempt an accepted WebSocket handler from ordinary HTTP handler deadlines while retaining server cancellation.
- [ ] Do not expose raw frames in the first public API.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `http/server` — PARTIAL: owned plaintext HTTP1.1 and H2 verified

Current [server API](http-server.md) replaces the withdrawn v3 signatures.
Evidence: H1 7 files /26 checks,
H2 foundation 4/35,
H2 session 7/39,
H2 listener plus H1 compatibility 3/14.
These gates overlap; do not add their counts as distinct tests.

- [x] HttpRequestContext endpoint/TLS observations, restricted cancellation and bounded successor route parameters.
- [x] Handling/HandlingResult, ResponseHandling, WebSocketHandlingResult with separate application, WrongHandlingKind.
- [x] ServerTimeouts/ServerPolicy validate protocol sets, nonnegative deadlines and bounded resources; zero disables only each configured deadline.
- [x] HttpServer owns plaintext listener and accepted tasks; explicit H1.1 or H2-only selection.
- [x] H1 framing before handler, smuggling/obs-fold rejection, incremental limits, independent deadlines, Expect100 willingness and ordered pipelines.
- [x] Independent owned H2 stream handlers, checked body/trailers, shared flow/output and RST/EOF cancellation.
- [x] Minimal500 before output; close/reset after mid-response failure; no implicit Date/Server.
- [x] Immediate close cancels/joins active work; ServerDrain stops acceptance, finishes admitted work and cancels/joins at its explicit deadline.
- [x] Typed H1 WebSocket upgrade with configured trusted Origin; no raw hijacking.
- [ ] TLS server keys/identity and negotiated ALPN/mixed protocol selection.
- [x] H2 connection-idle/header-block deadlines, independent physical DATA budget and bounded protocol-failure GOAWAY; 11 files /47 checks.
- [x] H2 Expect100 willingness/417 before effects; 3 files /8 checks.
- [ ] Negotiated extended CONNECT/RFC8441.
- [ ] General H1 peer-disconnect cancellation while handler awaits unrelated work.

Std-only gates above do not rerun full regression. The prior frozen runtime gate is separate.

### `http/routing` — VERIFIED (focused/inline/actual H1 and H2)

Current [route API](http-routing.md): RoutePattern/RouteMatch, Route, Routes,
InvalidRoute. Native dynamic-map foundation has its own required full gate;
route std-only evidence is separate and does not imply that gate completed.

- [x] Raw split before one percent/UTF8 decode; encoded slash stays in one segment.
- [x] Literal, unique named and final wildcard segments, including zero suffix; exact Unicode/trailing slashes.
- [x] Bounded immutable configuration, same-method intersection rejection before declaration-order dispatch.
- [x]405 Allow with deduplicated methods and implicit HEAD; explicit HEAD wins over GET fallback.
- [x] Successor context parameters preserve endpoint/cancellation facts; only selected route constructs captures.
- [x] Origin and reconstructed absolute targets; query ignored for routing; malformed/control paths400.
- [x] Scoped handler cancellation/cleanup; explicit fallback and no mutable route registration.
- [x] Host selection stays an explicit outer Handling composition and confers no TLS/Origin trust.
- [x] Actual H1/H2 HEAD suppresses body; explicit representation metadata stays with framing.

### `web/*` — VERIFIED (focused/inline/actual TCP, live example)

[Контракт](web.md), аудит, evidence.
Тесты 661–663 (25 проверок, включая настоящий TCP keep-alive); пример [server-ddd](../../examples/web/server-ddd.ok).

- [x] Endpoint — bound action контроллера `async (WebRequest) is HandlingResult`; сервисы с методами, не объект на use case.
- [x] Scope-префиксы и маршруты компилируются в один std Routes: overlap, 405/Allow и HEAD остаются std.
- [x] ExceptionScope — отдельный объект с обычным try/catch; цепочка снаружи внутрь, неизвестные ошибки идут дальше.
- [x] Exception — Error и ответ RFC 9457; JSON-ошибки чтения — 422; прочие ошибки — std 500, отмена сохраняет смысл.
- [x] WebRequest: параметры, query как в браузере (`+` — пробел), поля, bounded text/json/form, 400/413/415.
- [x] Ответы владеют framing: Content-Type/Length; 204 без framing-полей; redirect с длиной 0.
- [x] PERF-25 под скоупами устранён trampoline-ом; `throw` принимает successor (Exception.header — successor).
- [x] 400/405 от std Routes — через новую точку расширения `RouteRejecting`; web отвечает problem-телом и сохраняет Allow.
- [x] 422 только для тела запроса: `json()` возвращает `RequestJson`; ошибки серверных JSON-данных через `JsonView` не переводятся (500).

Graph validation's measured interpreted construction cost is PERF-31 OPEN;
problem/cause/solutions. Further
specialized host/trie decorators are separate optimization/API work.

### `http/static` — VERIFIED (focused, inline, actual H1 pipeline)

- [x] StaticPolicy and borrowed-root StaticFiles Handling contract.
- [x] Descriptor-relative no-follow paths, exact one-time decoding and regular files.
- [x] Explicit index/listing policy; escaped bounded directory snapshot.
- [x] Weak ETag, Date/Last-Modified, exact length and ordered preconditions before reads.
- [x] HEAD/304 no pread; single/multipart ranges with precise bytes and lifetime.
- [x] Explicit precompressed artifacts, timestamp eligibility and Accept-Encoding negotiation.
- [x] Positive partial reads, truncation failure and one-shot body ownership.
- [x] Runnable example and real H1 HEAD/range pipeline.

[Contract](http-static.md) replaces the v3 Directory constructor with FileRoot.
Evidence:8files/30checks PASS, inline/example PASS.
Native root substrate has separate804-file corrected coverage. Static composition
is std-only; no repeated full regression. PERF-35 OPEN; no filesystem snapshot or
content-derived strong validator is claimed.

### `http/form` — VERIFIED (focused, inline, real H1 pipeline)

- [x] FormLimits, FormPart/MultipartParts capability roles, ParsedMultipartForm and typed errors.
- [x] One active logical operation; read the part before advancing; close cancels and settled joins.
- [x] Field/file/count/aggregate quotas during streaming; exact delimiter framing and binary data.
- [x] UTF-8 metadata in HTTP Headers, filename retained only as untrusted display data.
- [x] Explicit FormText supported charset projection, without overriding declarations.
- [x] Explicit TemporaryFile spool with caller-selected directory/prefix and failed-acquisition deletion.
- [x] Bounded URL-encoded HTTP adapter reusing url/form and ordered immutable parameters.
- [x] Runnable example, inline and real H1 pipeline gate.

[Contract](http-form.md) supersedes the v3 ASCII-only MessageHeaders candidate.
Evidence: 9files/38checks PASS; inline/example PASS.
Std-only; no repeated full regression. Remaining HTTP work continues below.

### `http/sse` — VERIFIED (focused, inline, actual H1 wire)

- [x] Immutable ServerEvent, explicit wire/headers/comment and one-shot EventBody.
- [x] Owned ServerEvents, explicit close/settled and concurrent-read rejection.
- [x] UTF-8 maximal-subpart replacement, one BOM, CR/LF/CRLF and multiline data.
- [x] Comments/unknown fields ignored, persistent id/retry and incomplete EOF discard.
- [x] MissingEventRetry, exact milliseconds and explicit resource/metadata bounds.
- [x] Caller-owned keepalive cadence and explicit no-buffering response headers.
- [x] Automatic reconnect stays separate client policy.

[Contract](http-sse.md) and evidence:
4files/12checks, inline and executable example PASS. PERF-32 retains malformed-text
CPU investigation. Std-only; no full regression repeated.

### `http/robots` — VERIFIED (focused/inline/example)

- [x] Immutable RobotsPolicy, bounded groups/wildcards/percent normalization.
- [x] Longest explicit product-token matching, equal groups combine, allow tie.
- [x] Typed absent Crawl-delay and explicit decimal extension, HTTP sitemap URLs.
- [x] Lazy owned FetchedRobots with limits, cancellation and explicit exchange policy.
- [x] Focused policy/fetch/pattern, inline and runnable example gates.

[Contract](http-robots.md), evidence/performance.
PERF-33 tracks interpreted dispatch cost. Std-only; no full regression repeated.

### `http/xmlrpc` — VERIFIED bounded snapshot/HTTP profile

- [x] Immutable values and single typed print event, ordered duplicate-checked members.
- [x] Call/fault/result variants, exact decimal double wire and local whole-second dates.
- [x] Explicit limits/nil negotiation, XML1/Base64 grammar and DTD/external-entity rejection.
- [x] Bounded call/result codecs; explicit Exchanging client and POST Handling adapter.
- [x] Cancellation cleanup, malformed/limit tests, actual H1 POST, inline/example.

[Contract](http-xmlrpc.md) supersedes the v3 map/payload-only-role candidates.
The HTTP/XML input is explicitly bounded and materialized before token parsing;
this is not a streaming/general XML implementation. Five focused files/16checks
PASS. Evidence/performance, PERF-34 OPEN.
Std-only; no full regression repeated.

### `runtime/database` — VERIFIED (native full + current focused)

- [x] Optional SQLite worker provider with canonical signatures and generation-checked opaque handles.
- [x] Bounded pollable messages, private protected acknowledgment lifecycle and separate interruption control.
- [x] Exact-child resource closing/reaping; slow-query timer, stale reply and cancellation tests.

[SQLite contract](db-sqlite.md), [pool contract](db-pool.md),
verification, and
performance investigation.
Native endpoint matrix: SQLite3.41.0/3.54.0, system integration3.51.0.
Full frozen regression:823files/13395checks PASS;4513inputs unchanged,
148root native files match the snapshot. Current std follow-ups:100focused checks PASS.

### `db/value` — VERIFIED (portable focused)

[Contract](db-values.md) defines this layer's role printing, immutable snapshots,
checked projections and explicit budgets. Evidence:
2files/12checks PASS across value/statement/row/error, plus inline/example.
Provider parsing, asynchronous readers and SQLite integration remain below.
This slice is std-only; no full regression repeated. PERF-36 OPEN.


### `db/statement` — VERIFIED (portable focused)

[Contract](db-values.md) defines this layer's role printing, immutable snapshots,
checked projections and explicit budgets. Evidence:
2files/12checks PASS across value/statement/row/error, plus inline/example.
Provider parsing, asynchronous readers and SQLite integration remain below.
This slice is std-only; no full regression repeated. PERF-36 OPEN.


### `db/row` — VERIFIED (portable focused)

[Contract](db-values.md) defines this layer's role printing, immutable snapshots,
checked projections and explicit budgets. Evidence:
2files/12checks PASS across value/statement/row/error, plus inline/example.
Provider parsing, asynchronous readers and SQLite integration remain below.
This slice is std-only; no full regression repeated. PERF-36 OPEN.


### `db/error` — VERIFIED (portable focused)

[Contract](db-values.md) defines this layer's role printing, immutable snapshots,
checked projections and explicit budgets. Evidence:
2files/12checks PASS across value/statement/row/error, plus inline/example.
Provider parsing, asynchronous readers and SQLite integration remain below.
This slice is std-only; no full regression repeated. PERF-36 OPEN.


### `db/connection` — VERIFIED (native full + current focused)

- [x] Portable DbExecuting/DbConnection/DbConnecting roles; explicit six-action cleanup capability.
- [x] Serialized operations, one active reader/BLOB and ownership/bypass rejection.
- [x] Safe storage-class copies, prepare-tail, native bindings, known/unknown affected counts.

[SQLite contract](db-sqlite.md), [pool contract](db-pool.md),
verification, and
performance investigation.
Native endpoint matrix: SQLite3.41.0/3.54.0, system integration3.51.0.
Full frozen regression:823files/13395checks PASS;4513inputs unchanged,
148root native files match the snapshot. Current std follow-ups:100focused checks PASS.

### `db/transaction` — VERIFIED (native full + current focused)

- [x] Default/serializable isolation; explicit SQLite deferred/immediate/exclusive modes.
- [x] Confirmed terminal idempotence, lexical rollback and rejection of nested/connection bypass.
- [x] Cancelled/unacknowledged commit records IndeterminateCommit and closes physical connection.

[SQLite contract](db-sqlite.md), [pool contract](db-pool.md),
verification, and
performance investigation.
Native endpoint matrix: SQLite3.41.0/3.54.0, system integration3.51.0.
Full frozen regression:823files/13395checks PASS;4513inputs unchanged,
148root native files match the snapshot. Current std follow-ups:100focused checks PASS.

### `db/prepared` — VERIFIED (native full + current focused)

- [x] Separate DbPreparing capability and Prepared/PreparedStatement facade.
- [x] Native prepare-tail, reset/clear after success/failure, reuse exclusion and close revocation.
- [x] Automatic schema metadata drift rejects before publishing incomplete/misnamed rows.

[SQLite contract](db-sqlite.md), [pool contract](db-pool.md),
verification, and
performance investigation.
Native endpoint matrix: SQLite3.41.0/3.54.0, system integration3.51.0.
Full frozen regression:823files/13395checks PASS;4513inputs unchanged,
148root native files match the snapshot. Current std follow-ups:100focused checks PASS.

### `db/acquisition` — VERIFIED (std composition)

- [x] Explicit DbAcquisition facets preserve granted preparation/transaction capabilities.
- [x] PortableDbAcquiring retains role confinement and reports absent preparation as unsupported.
- [x] SqliteAcquiring/DbPreparedPool/PooledTransactions compose native prepared queries with leases.
- [x] Transaction-bound preparation/reader revocation and pooled preparation tests.

[Contract](db-pool.md#explicit-preparation-facets). Std-only follow-up to the frozen native gate.

### `db/pool` — VERIFIED (native full + current focused)

- [x] FIFO bounded leases, shared acquisition deadlines, cancellation/shutdown cleanup.
- [x] Lazy minimum warmup, idle/lifetime expiry, explicit optional health checks.
- [x] Reader/transaction revocation, reset before slot release and disposal of failed physical connections.

[SQLite contract](db-sqlite.md), [pool contract](db-pool.md),
verification, and
performance investigation.
Native endpoint matrix: SQLite3.41.0/3.54.0, system integration3.51.0.
Full frozen regression:823files/13395checks PASS;4513inputs unchanged,
148root native files match the snapshot. Current std follow-ups:100focused checks PASS.

### `db/sqlite` — VERIFIED (native full + current focused)

- [x] SQLite 3.41+/optional FFI 2.12 isolated POSIX implementation; unsupported Windows.
- [x] Literal file paths, verified FK/journal flags, exact signed64/NUL/storage-class semantics.
- [x] Explicit typed configuration; raw PRAGMA/ATTACH/control and loadable extensions cannot bypass it.

[SQLite contract](db-sqlite.md), [pool contract](db-pool.md),
verification, and
performance investigation.
Native endpoint matrix: SQLite3.41.0/3.54.0, system integration3.51.0.
Full frozen regression:823files/13395checks PASS;4513inputs unchanged,
148root native files match the snapshot. Current std follow-ups:100focused checks PASS.

### `db/sqlite/blob` — VERIFIED (native full + current focused)

- [x] Identifier-taking C API, fixed size, exact BigInt bounds and64 KiB copies.
- [x] Writable capability checks, commit busy until close, rollback/connection invalidation.

[SQLite contract](db-sqlite.md), [pool contract](db-pool.md),
verification, and
performance investigation.
Native endpoint matrix: SQLite3.41.0/3.54.0, system integration3.51.0.
Full frozen regression:823files/13395checks PASS;4513inputs unchanged,
148root native files match the snapshot. Current std follow-ups:100focused checks PASS.

### `db/sqlite/backup` — VERIFIED (native full + current focused)

- [x] Owned destination file handle in source worker; existing connection pointers never cross workers.
- [x] 1..256 page batches with explicit monotonic deadline and bounded busy retries.
- [x] Cancelled incomplete target transaction rolls back; independent destination query checks.

[SQLite contract](db-sqlite.md), [pool contract](db-pool.md),
verification, and
performance investigation.
Native endpoint matrix: SQLite3.41.0/3.54.0, system integration3.51.0.
Full frozen regression:823files/13395checks PASS;4513inputs unchanged,
148root native files match the snapshot. Current std follow-ups:100focused checks PASS.

### `db/sqlite/script` — VERIFIED (native full + current focused)

- [x] SQLite prepare-tail for quotes/comments/trigger bodies;1 MiB/256 statement limits.
- [x] Reject row-producing/control SQL, stop at first failure, preserve caller transaction effects.

[SQLite contract](db-sqlite.md), [pool contract](db-pool.md),
verification, and
performance investigation.
Native endpoint matrix: SQLite3.41.0/3.54.0, system integration3.51.0.
Full frozen regression:823files/13395checks PASS;4513inputs unchanged,
148root native files match the snapshot. Current std follow-ups:100focused checks PASS.

### `runtime/application` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** сначала consuming std object + контракт, затем ABI/provider; opaque handles, portable wire errors, fake provider + real integration.

- [x] Контракт принят: [application/CLI](application-cli.md), confined mode/screen/resize capabilities и owned session; сигнатуры v3 заменены исполняемыми v4 объектами.
- [x] Add native standard-stream TTY detection.
- [x] Add native controlling-terminal discovery and opening.
- [x] Add terminal width and height queries.
- [x] Add guarded cooked, cbreak and raw terminal modes.
- [x] Add a pollable terminal-resize source.
- [x] Add scheduler-aware standard-input readiness without a blocking host read.
- [x] Add Windows console Unicode input and output support.
- [x] Add guarded Windows virtual-terminal processing.
- [x] Restore terminal modes and console flags after normal return, error, cancellation and panic.
- [x] Keep config grammars, CLI policy and ANSI rendering out of the runtime.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `cli/arguments` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** explicit arguments/environment/output, parsing failures, stable help, no ambient subprocess/shell magic.

- [x] **ProcessArguments** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `cursor() is Cursor<@byte>`; `size() is int`; `item(index is int) is @byte`; `first() is @byte`; `last() is @byte`.

- [x] **Utf8Arguments** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `Utf8Arguments(origin is ProcessArguments)`; `cursor() is Cursor<string>`; `size() is int`; `item(index is int) is string`; `first() is string`; `last() is string`.

- [x] Decode each argument independently with Stage 06 strict UTF-8.
- [x] Preserve argument boundaries exactly.
- [x] Report the failing argument index and byte offset on invalid UTF-8.
- [x] Do not reinterpret quotes, escapes, globs or environment references after the host has split argv.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `config/key` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **SettingKey** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `SettingKey(segments is Sequencing<string>)`; `segments() is Sequencing<string>`; `order(other is SettingKey) is Order`; `string() is string`.

- [x] **SettingKeyFromString** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `SettingKeyFromString(origin is string, separator is string)`; `#projected value() is SettingKey`.

- [x] **InvalidSettingKey** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `InvalidSettingKey(reason is string)`; `text() is string`.

- [x] Reject an empty key, empty segment or empty separator.
- [x] Keep segments as the identity; make `string()` diagnostic rather than round-trippable.
- [x] Do not assign universal meaning to dots, underscores or case folding.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `config/source` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **SettingSource** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `SettingSource(kind is SettingSourceKind, name is string)`; `kind() is SettingSourceKind`; `name() is string`.

- [x] **SettingEntry** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `SettingEntry(key is SettingKey, values is Sequencing<string>, source is SettingSource)`; `key() is SettingKey`; `values() is Sequencing<string>`; `source() is SettingSource`.

- [x] **SettingReading** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `membership(key is SettingKey) is bool`; `entry(key is SettingKey) is SettingEntry`; `keys() is Sequencing<SettingKey>`.

- [x] **Settings** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `Settings(entries is Sequencing<SettingEntry>)`; `membership(key is SettingKey) is bool`; `entry(key is SettingKey) is SettingEntry`; `keys() is Sequencing<SettingKey>`.

- [x] **LayeredSettings** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `LayeredSettings(layers is Sequencing<SettingReading>)`; `membership(key is SettingKey) is bool`; `entry(key is SettingKey) is SettingEntry`; `keys() is Sequencing<SettingKey>`.

- [x] **MissingSetting** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `MissingSetting(key is SettingKey)`; `key() is SettingKey`; `text() is string`.

- [x] Reject duplicate keys inside one `Settings` object.
- [x] Let a later layer replace the complete entry from an earlier layer.
- [x] Never append repeated values across precedence layers implicitly.
- [x] Preserve the winning entry's source for diagnostics.
- [x] Preserve deterministic key order by first appearance, replacing values in place.
- [x] Keep `Settings` immutable.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `config/value` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **Setting** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `Setting(origin is SettingReading, key is SettingKey)`; `values() is Sequencing<string>`; `source() is SettingSource`.

- [x] **ScalarSetting** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `ScalarSetting(origin is SettingReading, key is SettingKey)`; `value() is string`; `source() is SettingSource`.

- [x] **ScalarSettingOr** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `ScalarSettingOr(origin is SettingReading, key is SettingKey, fallback is string)`; `value() is string`; `source() is SettingSource`.

- [x] **NonScalarSetting** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `NonScalarSetting(key is SettingKey, count is int)`; `key() is SettingKey`; `count() is int`; `text() is string`.

- [x] Make `Setting` throw `MissingSetting` when the key is absent.
- [x] Make `ScalarSetting` require exactly one value.
- [x] Make `ScalarSettingOr` use its fallback only when the key is absent, not when the value is empty.
- [x] Give a fallback the synthetic source `SettingSourceKind.defaults`.
- [x] Parse scalar text explicitly through Stage 05 `strings/parse` objects.
- [x] Do not create a parallel typed configuration-value hierarchy.
- [x] Do not infer secrets from key names; callers decide which values must be redacted.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `config/environment` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **EnvironmentSettings** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `EnvironmentSettings(origin is Environment, names is %string:SettingKey, source is SettingSource)`; `membership(key is SettingKey) is bool`; `entry(key is SettingKey) is SettingEntry`; `keys() is Sequencing<SettingKey>`.

- [x] Decode mapped names and values as strict UTF-8.
- [x] Keep missing variables absent and empty variables present with one empty value.
- [x] Reject two environment names mapped to the same setting key.
- [x] Require an explicit environment-name mapping.
- [x] Do not invent implicit prefixes, underscore splitting or case normalization.
- [x] Do not mutate the process environment while reading configuration.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `config/document` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **TomlSettings** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `TomlSettings(origin is TomlTable, source is SettingSource)`; `membership(key is SettingKey) is bool`; `entry(key is SettingKey) is SettingEntry`; `keys() is Sequencing<SettingKey>`.

- [x] **JsonSettings** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `JsonSettings(origin is JsonObject, source is SettingSource)`; `membership(key is SettingKey) is bool`; `entry(key is SettingKey) is SettingEntry`; `keys() is Sequencing<SettingKey>`.

- [x] **IniSettings** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `IniSettings(origin is IniDocument, source is SettingSource)`; `membership(key is SettingKey) is bool`; `entry(key is SettingKey) is SettingEntry`; `keys() is Sequencing<SettingKey>`.

- [x] Flatten nested TOML tables and JSON objects into key segments without joining segment text.
- [x] Convert scalar values to their canonical source-format text.
- [x] Convert an array of scalars into repeated values from one source.
- [x] Reject nested arrays, arrays containing objects and TOML arrays of tables.
- [x] Map INI sections and entry names to exactly two key segments.
- [x] Preserve document insertion order.
- [x] Keep file opening, charset decoding and size limits explicit through Stage 07 and Stage 11 objects.
- [x] Do not select a configuration grammar from a filename extension implicitly.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `cli/spec` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** explicit arguments/environment/output, parsing failures, stable help, no ambient subprocess/shell magic.

- [x] **OptionValue** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `OptionValue(name is string, repeated is bool)`; `name() is string`; `repeated() is bool`.

- [x] **Option** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `Option(key is SettingKey, names is Sequencing<string>, value is OptionValue, help is string)`; `key() is SettingKey`; `names() is Sequencing<string>`; `value() is OptionValue`; `help() is string`.

- [x] **Flag** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `Flag(key is SettingKey, names is Sequencing<string>, present is string, help is string)`; `key() is SettingKey`; `names() is Sequencing<string>`; `present() is string`; `help() is string`.

- [x] **Operand** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `Operand(key is SettingKey, name is string, cardinality is OperandCardinality, help is string)`; `key() is SettingKey`; `name() is string`; `cardinality() is OperandCardinality`; `help() is string`.

- [x] **CommandSyntax** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `CommandSyntax(options is Sequencing<Option>, flags is Sequencing<Flag>, operands is Sequencing<Operand>)`; `options() is Sequencing<Option>`; `flags() is Sequencing<Flag>`; `operands() is Sequencing<Operand>`.

- [x] **CliCommand** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `CliCommand(name is string, summary is string, syntax is CommandSyntax, children is Sequencing<CliCommand>)`; `name() is string`; `summary() is string`; `syntax() is CommandSyntax`; `children() is Sequencing<CliCommand>`.

- [x] Validate the complete command tree before parsing arguments.
- [x] Require long names to match `--[a-z0-9][a-z0-9-]*`.
- [x] Require short names to contain one visible ASCII character after `-`.
- [x] Reject duplicate option names and child command names in one command.
- [x] Permit multiple flags for one setting key only to express explicit alternatives such as `true` and `false`.
- [x] Reject a setting key shared by value options, operands or incompatible declarations.
- [x] Require optional operands to follow required operands.
- [x] Require repeated operands to be last.
- [x] Keep help and version as ordinary explicitly declared flags.
- [x] Make every `Option` consume exactly one value per occurrence.
- [x] Check whether a setting is required only after all configuration layers are composed.
- [x] Do not attach callbacks or process exits to command specifications.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `cli/parse` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** explicit arguments/environment/output, parsing failures, stable help, no ambient subprocess/shell magic.

- [x] **ParsedCommand** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `ParsedCommand(origin is CliCommand, arguments is Sequencing<string>)`; `#projected value() is Invocation`.

- [x] **Invocation** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `command() is CliCommand`; `path() is Sequencing<string>`; `settings() is Settings`.

- [x] **CliFailure** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `CliFailure(index is int, token is string, message is string)`; `index() is int`; `token() is string`; `text() is string`.

- [x] Accept exact long options as `--name value` and `--name=value`.
- [x] Accept short flags in clusters such as `-abc`.
- [x] Let only the final short name in a cluster consume a value.
- [x] Make `--` terminate option recognition.
- [x] Do not abbreviate long names or correct misspellings automatically.
- [x] Reject a repeated scalar option unless its `OptionValue.repeated()` is true.
- [x] Store a present flag as its declared `Flag.present()` value.
- [x] Reject a flag value such as `--verbose=false`; model negation as a separate explicit flag.
- [x] Select a child command only at the command position defined by the validated syntax.
- [x] Preserve repeated option and operand order.
- [x] Treat a negative numeric token as an operand when no exact option name matches it.
- [x] Reject unknown options, missing values, extra operands and incomplete command paths.
- [x] Return argument-derived `Settings` without reading files or environment variables.
- [x] Never print help, write diagnostics or terminate the process from the parser.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `cli/help` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** explicit arguments/environment/output, parsing failures, stable help, no ambient subprocess/shell magic.

- [x] **Usage** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `Usage(origin is Command, program is string, width is int)`; `string() is string`.

- [x] **ShellCompletion** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `ShellCompletion(origin is Command, program is string, shell is CompletionShell)`; `string() is string`.

- [x] Render deterministic plain text with no ANSI sequences.
- [x] Preserve declaration order for options, operands and child commands.
- [x] Display every accepted name, value placeholder and repetition marker.
- [x] Wrap descriptions on Unicode display width rather than byte or code-point count.
- [x] Reject unusably narrow and non-positive widths.
- [x] Quote shell completion literals for the selected shell.
- [x] Generate static completion only; keep dynamic completion callbacks outside the first API.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `terminal/capability` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **TerminalSize** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `TerminalSize(columns is int, rows is int)`; `columns() is int`; `rows() is int`.

- [x] **TerminalCapabilities** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `input_interactive() is bool`; `output_interactive() is bool`; `colors() is TerminalColorDepth`; `size() is TerminalSize`.

- [x] Respect `ColorPolicy.never` unconditionally.
- [x] Respect `ColorPolicy.always` even for redirected output, but only for styling sequences.
- [x] In automatic mode require a TTY, honor `NO_COLOR`, reject `TERM=dumb` and inspect platform capabilities.
- [x] Use `COLUMNS` and `LINES` only as validated fallbacks when a native size query is unavailable.
- [x] Keep terminal capability detection immutable for one opened session except for size changes.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `terminal/width` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **DisplayWidth** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `DisplayWidth(origin is string)`; `#projected value() is int`.

- [x] **TruncatedDisplay** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `TruncatedDisplay(origin is string, width is int, ellipsis is string)`; `string() is string`.

- [x] **InvalidDisplayText** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `InvalidDisplayText(index is int, reason is string)`; `index() is int`; `text() is string`.

- [x] Use the Stage 06 pinned Unicode character database.
- [x] Give combining marks, variation selectors and zero-width joiners zero standalone width.
- [x] Account for East Asian wide characters and joined emoji sequences.
- [x] Treat control characters as `InvalidDisplayText` rather than assigning them a printable width.
- [x] Truncate only at extended grapheme-cluster boundaries.
- [x] Reject negative widths and an ellipsis wider than the target width.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `terminal/style` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **TerminalColor** — реализован по [v4 contract](application-cli.md).
- [x] **DefaultColor** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `DefaultColor()`.

- [x] **NamedColor** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `NamedColor(value is BasicColor)`; `value() is BasicColor`.

- [x] **IndexedColor** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `IndexedColor(value is int)`; `value() is int`.

- [x] **RgbColor** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `RgbColor(red is byte, green is byte, blue is byte)`; `red() is byte`; `green() is byte`; `blue() is byte`.

- [x] **TextStyle** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `TextStyle(foreground is TerminalColor, background is TerminalColor, attributes is Set<TextAttribute>)`; `foreground() is TerminalColor`; `background() is TerminalColor`; `attributes() is Set<TextAttribute>`.

- [x] **StyledText** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `StyledText(origin is string, style is TextStyle)`; `text() is string`; `style() is TextStyle`.

- [x] Reject indexed colors outside 0..255.
- [x] Downgrade colors deterministically to the detected depth.
- [x] Emit a complete style reset after each `StyledText` value.
- [x] Never count ANSI control bytes toward display width.
- [x] Keep styling separate from semantic log levels.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `terminal/session` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **OpenedStandardTerminal** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `OpenedStandardTerminal(output is TerminalOutputKind, policy is ColorPolicy)`; `session() is TerminalSession`.

- [x] **OpenedControllingTerminal** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `OpenedControllingTerminal(policy is ColorPolicy)`; `session() is TerminalSession`.

- [x] **TerminalSession** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `input() is TerminalInput`; `output() is TerminalOutput`; `capabilities() is TerminalCapabilities`; `closed() is bool`; `close() is void`.

- [x] **TerminalInput** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `bytes(limit is int) is @byte`; `line() is string`; `text() is string`; `capabilities() is TerminalCapabilities`.

- [x] **TerminalOutput** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `write(source is ByteSource) is void`; `write(values is ...string) is void`; `write(value is StyledText) is void`; `writeln(values is ...string) is void`; `writeln(value is StyledText) is void`; `flush() is void`; `capabilities() is TerminalCapabilities`.

- [x] Make standard-terminal interactivity depend on the selected standard output and standard input independently.
- [x] Open the controlling terminal independently of redirected stdin, stdout and stderr.
- [x] Make a missing controlling terminal a typed `TerminalUnavailable` failure.
- [x] Make session closing restore every mode and screen change made through that session.
- [x] Render `StyledText` as plain text when styling is disabled.
- [x] Preserve the Stage 07 `write`, `writeln`, `flush` and `<-` behavior.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `terminal/mode` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **AppliedTerminalMode** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `AppliedTerminalMode(origin is TerminalSession, mode is TerminalMode)`; `lease() is TerminalModeLease`.

- [x] **TerminalModeLease** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `mode() is TerminalMode`; `closed() is bool`; `close() is void`.

- [x] Apply mode changes only to an interactive terminal input.
- [x] Keep mode leases properly nested.
- [x] Make closing a lease restore the immediately previous mode.
- [x] Restore all outstanding leases when their session closes.
- [x] Do not expose host `termios` bit masks or Windows console flags.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `terminal/screen` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **TerminalScreen** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `TerminalScreen(origin is TerminalSession)`; `clear(region is ClearRegion) is void`; `move(row is int, column is int) is void`; `display_cursor(visible is bool) is void`; `use_alternate_screen(enabled is bool) is void`.

- [x] **TerminalResizes** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `TerminalResizes(origin is TerminalSession)`; `async size() is TerminalSize`; `closed() is bool`; `close() is void`.

- [x] Reject screen control on non-interactive output even under `ColorPolicy.always`.
- [x] Treat row and column as zero-based logical coordinates.
- [x] Coalesce resize notifications and return the newest observed size.
- [x] Restore a visible cursor and leave the alternate screen when the session closes.
- [x] Do not expose arbitrary escape-sequence construction.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `terminal/prompt` — VERIFIED (native full + focused; Windows live separate)

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **PromptedLine** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `PromptedLine(origin is TerminalSession, prompt is string, limit is int)`; `string() is string`.

- [x] **PromptedSecret** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `PromptedSecret(origin is TerminalSession, prompt is string, limit is int)`; `string() is string`.

- [x] **TerminalUnavailable** — реализован по [v4 contract](application-cli.md).

  Исторические кандидаты v3 (актуальные сигнатуры — в [контракте](application-cli.md)): `TerminalUnavailable(reason is string)`; `text() is string`.

- [x] Flush the prompt before waiting for input.
- [x] Strip one trailing line ending without trimming other whitespace.
- [x] Enforce a positive byte limit incrementally.
- [x] Require a controlling terminal for secret input.
- [x] Disable echo only for the secret read and restore it on every exit path.
- [x] Do not return partial secret input after interruption or overflow.
- [x] Do not add confirmation, menu or form widgets to the primitive prompt layer.

**Проверки:** RED→GREEN; focused17files/103checks PASS; полный native gate840files/13550checks PASS, все5273 frozen inputs неизменны. [Пример](../../examples/std-v4/application-cli.ok), отчёт/evidence. POSIX: real PTY PASS; Windows:16 fake API checks PASS, live test требует Windows runner.

### `runtime/logging` — VERIFIED

**Зависимости и gate:** сначала consuming std object + контракт, затем ABI/provider; opaque handles, portable wire errors, fake provider + real integration.

- [x] Проверена композиция существующих clocks/process/I/O; отдельный runtime/logging не нужен.
- [x] Task identity в Stage25 не требуется; решение остаётся за потребностями Stage28.
- [x] Do not add ambient task-local logging context as a runtime intrinsic.
- [x] Do not add a native logging provider; compose logging from existing clocks and I/O capabilities.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/level` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **LogLevelName** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **LogLevelFromName** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] Order levels exactly as declared from least to most severe.
- [x] Parse canonical lowercase names case-insensitively.
- [x] Render canonical uppercase names for text output.
- [x] Keep tracing spans separate instead of adding an ambiguous `trace` log level.
- [x] Do not add custom numeric levels to the portable API.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/name` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **LogName** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **ChildLogName** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **InvalidLogName** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] Reject empty names and empty segments.
- [x] Preserve segments as identity and use a dot only for diagnostic rendering.
- [x] Compare names lexicographically by Unicode scalar value.
- [x] Do not make name ancestry imply filtering or sink inheritance automatically.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/field` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **LogFields** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **EmptyLogFields** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **CombinedLogFields** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **MissingLogField** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] Reject empty field names.
- [x] Preserve field insertion order.
- [x] Let an explicitly added field replace a base field without moving its position.
- [x] Reuse Stage 13 `JsonValue` for structured scalar, array and object fields.
- [x] Encode bytes, timestamps and application domain objects explicitly before logging them.
- [x] Do not stringify arbitrary `any` values through the runtime debug dumper.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/record` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **LogContext** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **LogRecord** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **MissingLogCause** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] Keep records and contexts immutable.
- [x] Capture the wall-clock timestamp when the logging action executes.
- [x] Make `cause()` throw `MissingLogCause` when no cause was supplied.
- [x] Preserve a cause as an `Error`; format only its public `text()` in Stage 25.
- [x] Add stack and source diagnostics only through Stage 28 capabilities.
- [x] Do not infer fields by reflecting over the message or cause.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/write` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **LogWriting** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **DiscardingLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **CollectedLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **BroadcastLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **FallbackLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **BestEffortLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **LogDeliveryFailure** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] Keep one `write()` call equal to one complete immutable record.
- [x] Make `CollectedLogs.records()` return a snapshot.
- [x] Attempt broadcast destinations in declaration order and report every failure together.
- [x] Invoke fallback only when primary delivery fails.
- [x] Document that fallback delivery can duplicate a record after an indeterminate partial primary write.
- [x] Let ordinary sink failures propagate unless wrapped in `FallbackLogs` or `BestEffortLogs`.
- [x] Never log a logging failure through the failing destination recursively.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/named` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **Logging** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **NamedLogging** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] Merge logger fields with call fields, giving call fields precedence.
- [x] Capture one timestamp and process identifier per delivered record.
- [x] Construct the record before invoking its destination exactly once.
- [x] Keep logging actions synchronous and deterministic.
- [x] Do not hide sink failures by default.
- [x] Do not install a process-global default logger.
- [x] Do not derive logger names from source packages or object names implicitly.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/enrichment` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **FieldLogging** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **ChildLogging** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] Merge contextual fields before call fields so the nearest explicit value wins.
- [x] Preserve the destination, clock and base fields in child loggers.
- [x] Keep contextual logging as an explicit object passed to collaborators.
- [x] Do not use thread-local, task-local or dynamically scoped field maps.
- [x] Reserve trace and span correlation-field construction for Stage 28.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/filter` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **LogAllowing** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **LevelThresholds** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **AllowedLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **EnabledLog** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] Match the longest exact name-prefix override by `LogName` segments.
- [x] Let a threshold admit its own level and every more severe level.
- [x] Filter before formatting and I/O.
- [x] Use `EnabledLog` only when constructing a record would be materially expensive.
- [x] Keep field- and message-dependent policy in explicit application `LogWriting` decorators.
- [x] Do not make filtering mutate records.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/redact` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **RedactedLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **RedactedCauseLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] Replace matching top-level field values without removing their names or positions.
- [x] Apply redaction before broadcasting, queuing, formatting or persistence.
- [x] Treat field-name matching as exact and case-sensitive.
- [x] Replace a cause with public replacement text without inspecting private state.
- [x] Do not pretend key-based redaction can discover secrets embedded in arbitrary messages.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/line` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **TextLogLine** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **JsonLogLine** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] Render text timestamps as RFC 3339 with an explicit offset.
- [x] Render level, logger name, process, message, fields and cause in a stable order.
- [x] Escape newlines and control characters in text records so one record occupies one physical line.
- [x] Encode JSON records through Stage 13 without hand-built quoting.
- [x] Put application fields under a dedicated `fields` JSON member to prevent reserved-key collisions.
- [x] Represent an absent cause by omitting the JSON member.
- [x] Return no trailing line ending from a log-line object.
- [x] Do not support an unrestricted interpolation template language for log formats.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/stream` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **TextStreamLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **JsonStreamLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **TextTerminalLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **JsonTerminalLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] Encode formatted records as UTF-8 and append exactly one newline.
- [x] Submit the complete encoded record through one all-or-error `Writing.write()` action.
- [x] Make terminal log sinks style the complete rendered record according to its level.
- [x] Let terminal capability and `ColorPolicy` decide whether style sequences are emitted.
- [x] Keep flushing under the caller's control through the underlying `Writing` capability.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/file` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **FileLogOptions** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **LogFile** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] Open the file lazily in append mode with close-on-exec.
- [x] Create a missing file with the requested permissions without changing an existing file's permissions.
- [x] Encode and append each complete record under the sink's serialization boundary.
- [x] Make `flush_each` flush userspace buffers and `sync_each` request durable storage.
- [x] Make `reopen()` support external rotation without losing a record between handles.
- [x] Keep multi-process record atomicity outside the portable guarantee.
- [x] Surface disk-full, permission and short-write failures.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/rotation` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **Rotating** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **SizeRotating** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **IntervalRotating** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] **RotatingLogFile** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); v3-кандидат заменён проверенным API.

- [x] Reject non-positive size and time limits and negative backup counts.
- [x] Check rotation before appending the next complete record.
- [x] Rotate through atomic renames when the filesystem supports them.
- [x] Keep at most the configured number of completed files.
- [x] Never split one record across files.
- [x] Keep rotation serialized within one sink.
- [x] Document that several processes must not rotate the same path independently.
- [x] Compose Stage 16 compression explicitly after a file is no longer active.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/queue` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **LogDraining** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); прежний кандидат заменён проверенным API.

- [x] **QueuedLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); прежний кандидат заменён проверенным API.

- [x] Reject non-positive capacity.
- [x] Keep accepted immutable record references in a private bounded deque.
- [x] Mutate the deque and overflow counter only inside one non-suspending object action.
- [x] Wake the worker through a Stage 19 `Event` when the queue changes.
- [x] Never block a logging caller waiting for queue capacity.
- [x] Apply the configured overflow decision atomically and increment `dropped_records()`.
- [x] Preserve FIFO order among records that remain queued.
- [x] Register one lazily started delivery worker in the supplied structured-concurrency scope.
- [x] Let that one worker deliver records serially to the destination.
- [x] Make `drain()` wait for pre-call records to be delivered, explicitly overflow-discarded, or failed.
- [x] Propagate the first downstream failure from `drain()` and reject further records.
- [x] Make `close()` reject new records and cancel undrained work; call `drain()` first for graceful shutdown.
- [x] Do not spawn hidden unmanaged tasks.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `logging/syslog` — VERIFIED

**Зависимости и gate:** injected output, coherent event representation, explicit formatting, sink failure и no global manager.

- [x] **SyslogLogs** — контракт и реализация проверены.

  Фактический API: [контракт logging](logging.md); прежний кандидат заменён проверенным API.

- [x] Format RFC 5424 version 1 messages with valid PRI and severity mapping.
- [x] Use `-` for unavailable hostname, application, process, message ID and structured data.
- [x] Encode message text as UTF-8 with the BOM required by RFC 5424 for unambiguous Unicode.
- [x] Reject a formatted datagram above the configured UDP payload limit; never truncate it silently.
- [x] Reuse queued overflow, drain and failure semantics.
- [x] Keep TCP, TLS and local-system journal transports out of the first portable syslog sink.

**Проверки:** current focused15files/87checks PASS; полный frozen853-file набор восстановлен после одной fixture-only ошибки (855files/13670 уникальных проверок с двумя поздними тестами). Все152 runtime/runner inputs совпадают. [Контракт](logging.md), отчёт/evidence.
### `runtime/locale` — native Unicode preference snapshot VERIFIED full + focused; Windows live separate

**Зависимости и gate:** сначала consuming std object + контракт, затем ABI/provider; opaque handles, portable wire errors, fake provider + real integration.

- [x] Consuming SystemLanguagePreferences / PreferredLocales API defined in locale-i18n.md.
- [x] Add a native immutable snapshot of platform-preferred language tags.
- [x] Return Windows and Apple preferred languages through Unicode APIs rather than legacy code pages.
- [x] Keep POSIX `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` and `LANG` interpretation in stdlib policy.
- [x] Do not call host `setlocale()` for formatting, parsing or collation.
- [x] Do not delegate portable output to host-dependent `strftime()`, `strcoll()` or locale databases.
- [x] Keep templates, CLDR data, message catalogs and plural rules out of the runtime.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `template/path` — VERIFIED

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **TemplatePath** — реализован bounded immutable контракт; см. [guide](template-guide.md).

  Кандидаты из v3 для пересмотра: `TemplatePath(segments is Sequencing<string>)`; `segments() is Sequencing<string>`; `order(other is TemplatePath) is Order`; `string() is string`.

- [x] **TemplatePathFromString** — реализован bounded immutable контракт; см. [guide](template-guide.md).

  Кандидаты из v3 для пересмотра: `TemplatePathFromString(origin is string)`; `#projected value() is TemplatePath`.

- [x] **InvalidTemplatePath** — реализован bounded immutable контракт; см. [guide](template-guide.md).

  Кандидаты из v3 для пересмотра: `InvalidTemplatePath(reason is string)`; `text() is string`.

- [x] Reject empty paths and empty segments.
- [x] Split source paths on literal dots without trimming or case folding.
- [x] Keep path segments as identity and make `string()` diagnostic.
- [x] Reserve the single dot path for the current section item.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

Проверены nominal identity, Unicode order, lazy parsing, exact indexed snapshot,
borrowed source failures, limits and scope escape. Evidence.

### `template/model` — explicit string bindings and libraries VERIFIED; sections DESIGN

- [x] TemplateStrings / TemplateBinding / TemplateBindings: bounded nominal string
  source, exact path identity, no arbitrary object reflection or implicit JSON.
- [x] MissingTemplateValue / DuplicateTemplateValue and typed source/budget errors.
  [Contract](template-guide.md), evidence.
  Other candidates below remain unimplemented until their own contract/tests.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **TemplateLimits** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TemplateLimits(depth is int, partial_depth is int, iterations is BigInt, output_bytes is BigInt)`; `depth() is int`; `partial_depth() is int`; `iterations() is BigInt`; `output_bytes() is BigInt`.

- [ ] **TemplateRendering** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `string(data is JsonValue, partials is TemplateLibrary, limits is TemplateLimits) is string`.

- [x] **TemplateLibrary** — bounded ordered snapshot in `template/library`;
  [contract](template-library-guide.md), RED/GREEN.

  Кандидаты из v3 для пересмотра: `TemplateLibrary(values is %string:TemplateRendering)`; `membership(name is string) is bool`; `value(name is string) is TemplateRendering`; `names() is Sequencing<string>`.

- [x] **EmptyTemplateLibrary** — v3 candidate replaced explicitly by `TemplateLibrary()`;
  no redundant projection object is added.

  Кандидаты из v3 для пересмотра: `#projected value() is TemplateLibrary`.

- [x] **MissingTemplate** — exact missing name and diagnostic, in `template/library`.

  Кандидаты из v3 для пересмотра: `MissingTemplate(name is string)`; `name() is string`; `text() is string`.

- [x] Reject negative budgets and empty partial names; zero intentionally permits
  empty work/output (the v3 positive-only candidate is superseded).
- [x] Keep compiled templates and libraries immutable (library lookup never compiles).
- [x] Preserve partial declaration order (NamedTemplate snapshot and names()).
- [x] Resolve partials only through the explicitly supplied library.
- [x] Do not let a template open files, import modules or access the environment.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `template/compile` and `template/tree` — plain and structured compilation VERIFIED

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **TextTemplate** — lazy bounded plain interpolation over TemplateStrings; [contract](template-guide.md).

  Принято: TextTemplate(origin is string); optional maximum_input/maximum_nodes/maximum_output; source() is string; string(data is TemplateStrings) is string.

- [x] **TemplateFailure** — original scalar offset/line/column; syntax and compile-budget errors.

  Кандидаты из v3 для пересмотра: `TemplateFailure(offset is int, line is int, column is int, message is string)`; `offset() is int`; `line() is int`; `column() is int`; `text() is string`.

- [x] Parse variables as `{{ path }}`.
- [x] Parse sections as `{{# path }}...{{/ path }}`.
- [x] Parse inverted sections as `{{^ path }}...{{/ path }}`.
- [x] Parse static partials as `{{> name }}`.
- [x] Parse comments as `{{! text }}`.
- [x] Escape a literal opening delimiter as `{{{{` and closing delimiter as `}}}}`.
- [x] Reject mismatched, crossing and unclosed sections with source positions.
- [x] Reject dynamic partial names and delimiter-changing directives.
- [x] Preserve ordinary whitespace exactly.
- [x] Apply standalone-line removal only to section, partial and comment tags occupying the whole logical line.
- [x] Compile to a bounded immutable instruction tree before rendering.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

TextTemplate retains flat interpolation. TemplateTree compiles sections, inverted
sections and static partials with exact matching, standalone removal and bounded
depth entirely in Okkama. Compilation precedes data observation; structured
rendering remains below. [Tree contract and evidence](template-tree-guide.md),
[parser migration](template-parser-migration.md).

### `template/render` and `template/sections` — plain streaming, section strings and partials VERIFIED

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **TemplateOutput** — bounded chunks, borrowed sink and explicit partial effects; [contract](template-guide.md).

  Принято: TemplateOutput(origin is TextTemplate, data is TemplateStrings, destination is WritingText); write() is void. Limits belong to TextTemplate.

- [x] **MissingTemplateValue** — реализован в template/model, retains exact TemplatePath and text.

  Кандидаты из v3 для пересмотра: `MissingTemplateValue(path is TemplatePath)`; `path() is TemplatePath`; `text() is string`.

- [x] **NonScalarTemplateValue** — exact path and JSON kind, `template/json`.
  [Explicit adapter contract](template-json-guide.md),
  RED/GREEN evidence.

  Кандидаты из v3 для пересмотра: `NonScalarTemplateValue(path is TemplatePath, kind is JsonKind)`; `path() is TemplatePath`; `kind() is JsonKind`; `text() is string`.

- [x] Resolve a path only through `JsonObject` members and the current section item.
  JsonTemplateStrings binds an explicit item: string(TemplatePath) keeps exact
  member identity; string(string) accepts dotted paths and current-item `.`.
  No parent fallback, array indexing or implicit conversion in TextTemplate.
  Context evidence.
- [x] Render JSON strings as their unquoted text and numbers in canonical decimal form.
- [x] Render booleans as canonical `true` or `false` and null as an empty string.
- [x] Reject array and object interpolation outside a section.
- [x] Iterate array sections in order and render object sections once with that object as context.
- [x] Render scalar sections once when truthy and inverted sections once when false or absent.
- [x] Define false, null, an empty string, an empty array and absence as false; define zero as true.
- [x] Make ordinary missing interpolation a typed failure rather than silently losing text.
- [x] SectionTemplate string rendering: tree nesting, shared iteration and aggregate
  UTF-8 byte limits. [Contract](template-sections-guide.md),
  RED/GREEN evidence.
- [x] Enforce partial-expansion depth incrementally and share work/output limits;
  [contract and evidence](template-partials-guide.md).
- [x] Add section/partial streaming with shared work/output limits, bounded
  chunks and string parity; [contract and evidence](template-section-output-guide.md).
- [x] Detect direct and indirect partial recursion through the partial-depth limit.
- [x] Make `TemplateOutput` stream bounded chunks without constructing the complete output string.
- [x] Make string rendering and streamed rendering byte-for-byte identical.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `template/html` — flat contextual interpolation VERIFIED; composition pending

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **HtmlTemplate** — explicit JsonValue string/stream rendering with a strict
  contextual HTML grammar; [contract](template-html-guide.md).

  Кандидаты из v3 для пересмотра: `HtmlTemplate(origin is string)`; `source() is string`; `string(data is JsonValue, partials is TemplateLibrary, limits is TemplateLimits) is string`.

- [x] Bind template source during construction and validate it lazily on the first `string()` builder.
- [x] Permit interpolation only in ordinary HTML text and fully quoted ordinary attribute values.
  Initial whitelist: id/class/title/alt/role/lang/dir/aria-*/data-*; other
  attributes are literal-only. Foreign markup and ambiguous recovery are rejected.
- [x] Escape text through Stage 13 `EscapedHtml` without quote escaping.
- [x] Escape quoted attribute values through `EscapedHtml` with quote escaping.
- [x] Reject interpolation in tag names, attribute names, comments and declarations.
- [x] Reject interpolation inside `script`, `style`, raw-text and escapable-raw-text elements.
- [x] Reject interpolation in event-handler, `srcdoc`, URL-bearing and CSS-bearing attributes.
- [x] HtmlTemplateLibrary / NamedHtmlTemplate: bounded ordered snapshots accepting
  only nominal HtmlTemplate values; [contract](template-html-library-guide.md).
- [ ] Require every partial used by an `HtmlTemplate` to be an `HtmlTemplate`.
- [ ] Validate the composed contextual state at each partial boundary.
- [x] Do not provide an unescaped or trusted-markup interpolation form.
- [x] Do not claim browser sanitization for literal markup authored in the template itself.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `template/syntax exclusions` — DESIGN

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] Утвердить недостающий объектный API этой области; готовые сигнатуры не выдумывать.
- [ ] Do not add arithmetic, boolean expressions or comparisons.
- [ ] Do not add function calls, filters, pipes, macros or user-defined directives.
- [ ] Do not add assignment, mutation, includes by path or template inheritance.
- [ ] Do not expose application objects, reflection or arbitrary `any` values.
- [ ] Do not add shell, SQL, JavaScript, CSS or JSON source templating helpers.
- [ ] Build SQL through Stage 23 parameter binding and commands through Stage 12 argument sequences.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `locale/tag` — complete bounded syntax and pinned IANA preferences VERIFIED focused

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **Locale** — immutable canonical tag coordinates and complete extensions; bounded snapshots and pinned IANA preferred values. [Contract/evidence](locale-tag-guide.md).

  Кандидаты из v3 для пересмотра: `Locale(language is string)`; `Locale(language is string, region is string)`; `Locale(language is string, script is string, region is string, variants is Sequencing<string>)`; `language() is string`; `script() is string`; `region() is string`; `string() is string`; `order(other is Locale) is Order`.

- [x] **LocaleFromLanguageTag** — lazy complete bounded parser, including extension/extlang/grandfathered/private-use forms; [contract](locale-tag-guide.md).

  Кандидаты из v3 для пересмотра: `LocaleFromLanguageTag(origin is string)`; `#projected value() is Locale`.

- [x] **LocaleExtensions** — immutable attribute/keyword/transformation/private-use sequences; [accepted contract](locale-tag-guide.md).

  Кандидаты из v3 для пересмотра: `LocaleExtensions(origin is Locale)`; `unicode() is %string:Sequencing<string>`; `transformations() is Sequencing<string>`; `private_use() is Sequencing<string>`.

- [x] **LocaleVariants** — represented directly by `Locale.variants() is Sequencing<string>`; no redundant wrapper object.

  Кандидаты из v3 для пересмотра: `LocaleVariants(origin is Locale)`; `values() is Sequencing<string>`.

- [x] **MissingLocaleSubtag** — typed missing script/region with `kind()` and `text()`.

  Кандидаты из v3 для пересмотра: `MissingLocaleSubtag(kind is string)`; `text() is string`.

- [x] **InvalidLocale** — original scalar `offset()` and `text()`; direct-coordinate failures use offset zero.

  Кандидаты из v3 для пересмотра: `InvalidLocale(offset is int, reason is string)`; `offset() is int`; `text() is string`.

- [x] Implement current BCP 47 syntax and IANA language-subtag aliases.
- [x] Canonicalize language to lowercase, script to title case and region to uppercase; pinned aliases are applied.
- [x] Preserve registered variants and Unicode, transformed and private-use extensions.
- [x] Reject duplicate variants without case distinction.
- [x] Reject duplicate extension singletons and Unicode extension keys.
- [x] Make absent `script()` and `region()` projections typed failures.
- [x] Compare complete case-normal represented tags, not localized display names.
- [x] Extend comparison to registry-canonical tags including extensions and aliases.
- [x] Do not accept POSIX locale suffixes such as `.UTF-8` or `@euro` as BCP 47 syntax.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `locale/data` — pinned plural/parent/likely core VERIFIED; complete consumer datasets VERIFIED

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **LocaleDataVersion** — accepted contract in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `cldr() is string`; `unicode() is string`; `currencies() is string`.

- [x] **LocaleFallbacks** — accepted contract in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `LocaleFallbacks(origin is Locale)`; `cursor() is Cursor<Locale>`; `size() is int`; `item(index is int) is Locale`; `first() is Locale`; `last() is Locale`.

- [x] Bundle one reviewed and pinned CLDR release.
- [x] Generate compact immutable locale tables at build time.
- [x] Bundle parent/likely/number/plural/Gregorian/currency CLDR 46.1 tables; pin the required ICU 76.1 / CLDR 46.0 collation archive and reproducible build.
- [x] Expose exact CLDR/Unicode/IANA/ISO snapshot and separate ICU/UCA/collation-CLDR coordinates.
- [x] Apply CLDR parent-locale rules before structural subtag removal.
- [x] End every data fallback chain at the CLDR root locale.
- [x] Keep language-tag negotiation distinct from CLDR data fallback.
- [x] Never depend on the host locale database for deterministic output.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `locale/system` — explicit POSIX/native preferences VERIFIED focused

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **PreferredLocales** — accepted profile in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `PreferredLocales(origin is Environment)`; `cursor() is Cursor<Locale>`; `size() is int`; `item(index is int) is Locale`; `first() is Locale`; `last() is Locale`.

- [x] Read POSIX preference in `LANGUAGE`, `LC_ALL`, `LC_MESSAGES`, then `LANG` order.
- [x] Treat `C` and `POSIX` as the invariant root locale.
- [x] Strip only validated POSIX charset and modifier suffixes before BCP 47 conversion.
- [x] Prefer the native platform language snapshot when environment convention is inapplicable.
- [x] Deduplicate canonical locales while preserving preference order.
- [x] Return the invariant root locale when no valid preference exists.
- [x] Do not mutate environment variables or process locale state.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `locale/match` — basic ranges, lookup/filter and explicit CLDR expansion VERIFIED

[Contract](locale-match-guide.md). BasicLanguageRange implements bounded RFC 4647
basic-range grammar and explicit matches(Locale); LanguagePrefixes is the
most-specific-first immutable prefix snapshot. HTTP weights remain in its field
profile. Suite 406 and existing locale suite 297 verify the pure boundary.
LookedUpLocale and FilteredLocales are implemented with bounded snapshots;
LikelyLocale is the explicit CLDR expansion object in locale/likely.


**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **LookedUpLocale** — bounded contract in locale-match-guide.md.

  Кандидаты из v3 для пересмотра: `LookedUpLocale(requested is Sequencing<Locale>, available is Sequencing<Locale>, fallback is Locale)`; `#projected value() is Locale`.

- [x] **FilteredLocales** — bounded contract in locale-match-guide.md.

  Кандидаты из v3 для пересмотра: `FilteredLocales(requested is Sequencing<Locale>, available is Sequencing<Locale>)`; `cursor() is Cursor<Locale>`; `size() is int`; `item(index is int) is Locale`; `first() is Locale`; `last() is Locale`.

- [x] **LikelyLocale** — accepted contract in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `LikelyLocale(origin is Locale)`; `#projected value() is Locale`.

- [x] Implement RFC 4647 lookup and basic filtering over canonical language ranges.
- [x] Preserve caller preference order and available-locale order deterministically.
- [x] Return only locales from the supplied available set.
- [x] Use CLDR likely subtags only through the explicit `LikelyLocale` object.
- [x] Do not use likely-subtag expansion implicitly during RFC 4647 matching.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `locale/number` — VERIFIED focused

[Numeric contract](locale-number.md): exact decimal rounding, pinned CLDR patterns,
77 numeric systems, scientific/engineering/compact output and strict decimal/BigInt
parsing. Float uses [ShortestFloat](shortest-float.md); nonfinite symbols are explicit.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **NumberPrecision** — implemented with the bounded immutable contract.

  Кандидаты из v3 для пересмотра: `NumberPrecision(minimum_integer is int, minimum_fraction is int, maximum_fraction is int, rounding is Rounding)`; `minimum_integer() is int`; `minimum_fraction() is int`; `maximum_fraction() is int`; `rounding() is Rounding`.

- [x] **NumberFormat** — implemented with the bounded immutable contract.

  Кандидаты из v3 для пересмотра: `NumberFormat(notation is NumberNotation, grouping is NumberGrouping, sign is NumberSign, precision is NumberPrecision)`; `notation() is NumberNotation`; `grouping() is NumberGrouping`; `sign() is NumberSign`; `precision() is NumberPrecision`.

- [x] **LocalizedInteger** — implemented with the bounded immutable contract.

  Кандидаты из v3 для пересмотра: `LocalizedInteger(origin is int, locale is Locale, format is NumberFormat)`; `string() is string`.

- [x] **LocalizedBigInteger** — implemented with the bounded immutable contract.

  Кандидаты из v3 для пересмотра: `LocalizedBigInteger(origin is BigInt, locale is Locale, format is NumberFormat)`; `string() is string`.

- [x] **LocalizedDecimal** — implemented with the bounded immutable contract.

  Кандидаты из v3 для пересмотра: `LocalizedDecimal(origin is Decimal, locale is Locale, format is NumberFormat)`; `string() is string`.

- [x] **LocalizedFloat** — implemented with the bounded immutable contract.

  Кандидаты из v3 для пересмотра: `LocalizedFloat(origin is float, locale is Locale, format is NumberFormat)`; `string() is string`.

- [x] **LocalizedDecimalFromString** — implemented with the bounded immutable contract.

  Кандидаты из v3 для пересмотра: `LocalizedDecimalFromString(origin is string, locale is Locale, format is NumberFormat)`; `#projected value() is Decimal`.

- [x] **LocalizedIntegerFromString** — implemented with the bounded immutable contract.

  Кандидаты из v3 для пересмотра: `LocalizedIntegerFromString(origin is string, locale is Locale, grouping is NumberGrouping)`; `#projected value() is BigInt`.

- [x] **LocalizedPercent** — implemented with the bounded immutable contract.

  Кандидаты из v3 для пересмотра: `LocalizedPercent(origin is Decimal, locale is Locale, format is NumberFormat, scale is PercentScale)`; `string() is string`.

- [x] Validate precision ranges and reject `minimum_fraction > maximum_fraction`.
- [x] Use locale numbering-system extensions only when bundled data supports them.
- [x] Apply decimal rounding before digit substitution and grouping.
- [x] Localize finite float values from their exact shortest decimal representation.
- [x] Render localized NaN and infinity symbols explicitly.
- [x] Parse strictly with the requested locale's digits, signs, decimal and grouping rules.
- [x] Reject mixed digit systems and misplaced grouping separators.
- [x] Scale percent and per-mille values before rounding and append the locale pattern's sign.
- [x] Do not parse compact notation because its inverse is ambiguous.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `locale/currency` — VERIFIED focused

[Contract](locale-currency.md): current ISO 4217 identity, explicit undefined minor
units, CLDR standard/accounting/symbol/name formats and all numeric notations.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **Currency** — immutable pinned identity/policy/representation implemented.

  Кандидаты из v3 для пересмотра: `Currency(code is string)`; `code() is string`; `digits() is int`; `order(other is Currency) is Order`.

- [x] **CurrencyFormat** — immutable pinned identity/policy/representation implemented.

  Кандидаты из v3 для пересмотра: `CurrencyFormat(numbers is NumberFormat, display is CurrencyDisplay, accounting is bool, currency_digits is bool)`; `numbers() is NumberFormat`; `display() is CurrencyDisplay`; `accounting() is bool`; `currency_digits() is bool`.

- [x] **LocalizedCurrency** — immutable pinned identity/policy/representation implemented.

  Кандидаты из v3 для пересмотра: `LocalizedCurrency(amount is Decimal, currency is Currency, locale is Locale, format is CurrencyFormat)`; `string() is string`.

- [x] Validate uppercase ISO 4217 alphabetic codes against the pinned currency dataset.
- [x] Override number precision with pinned currency fraction digits only when `currency_digits()` is true.
- [x] Select localized currency names through CLDR plural rules.
- [x] Keep currency formatting separate from exchange rates and monetary arithmetic.
- [x] Do not infer a currency from locale or region.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `locale/date-time` — VERIFIED focused

[Contract](locale-date-time.md), module `locale/date_time`: pinned Gregorian
styles, numeric extensions, flexible day periods and explicit timezone names.
Wall long/full use zone-free medium time; instant uses supplied TimeZone.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **DateTimeStyle** — immutable Gregorian representation implemented.

  Кандидаты из v3 для пересмотра: `DateTimeStyle(date is DateStyle, time is TimeStyle)`; `date() is DateStyle`; `time() is TimeStyle`.

- [x] **LocalizedDate** — immutable Gregorian representation implemented.

  Кандидаты из v3 для пересмотра: `LocalizedDate(origin is Date, locale is Locale, style is DateStyle)`; `string() is string`.

- [x] **LocalizedTime** — immutable Gregorian representation implemented.

  Кандидаты из v3 для пересмотра: `LocalizedTime(origin is TimeOfDay, locale is Locale, style is TimeStyle)`; `string() is string`.

- [x] **LocalizedLocalDateTime** — immutable Gregorian representation implemented.

  Кандидаты из v3 для пересмотра: `LocalizedLocalDateTime(origin is LocalDateTime, locale is Locale, style is DateTimeStyle)`; `string() is string`.

- [x] **LocalizedInstant** — immutable Gregorian representation implemented.

  Кандидаты из v3 для пересмотра: `LocalizedInstant(origin is Instant, zone is TimeZone, locale is Locale, style is DateTimeStyle)`; `string() is string`.

- [x] Apply locale calendar and numbering extensions only when explicitly supported.
- [x] Default to the Gregorian calendar without changing the instant or time zone.
- [x] Use CLDR month, weekday, day-period, era and timezone-name data.
- [x] Keep timezone selection explicit for every absolute instant.
- [x] Preserve `LocalDateTime` as timezone-free.
- [x] Do not add locale-dependent date parsing without an explicit unambiguous pattern.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `locale/list` — pinned CLDR patterns VERIFIED focused

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **LocalizedList** — bounded immutable CLDR lists; [accepted contract](locale-list.md).

  Кандидаты из v3 для пересмотра: `LocalizedList(values is Sequencing<string>, locale is Locale, kind is ListKind, style is ListStyle)`; `string() is string`.

- [x] Use distinct zero-, one-, two- and many-item CLDR patterns.
- [x] Preserve item text exactly and never parse it as markup.
- [x] Do not infer conjunction style from surrounding prose.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `locale/collation` — VERIFIED focused + full

[Contract and pinned build](locale-collation.md). Native 75 and language 690 PASS; full gate 889 files / 17,515 checks PASS.

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **CollationOptions** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `CollationOptions(strength is CollationStrength, case_first is CollationCaseFirst, numeric is bool, normalization is bool)`; `strength() is CollationStrength`; `case_first() is CollationCaseFirst`; `numeric() is bool`; `normalization() is bool`.

- [x] **CollatedString** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `CollatedString(origin is string, locale is Locale, options is CollationOptions)`; `string() is string`; `key() is @byte`; `order(other is CollatedString) is Order`.

- [x] Implement the Unicode Collation Algorithm with pinned CLDR tailoring.
- [x] Make `order()` and byte-key ordering identical.
- [x] Treat collation keys as opaque and valid only for the same data version, locale and options.
- [x] Do not persist collation keys or use them as text identities.
- [x] Reject comparison between values with different locale, options or pinned data versions.
- [x] Sort localized text by decorating each value with `CollatedString` rather than passing a comparator service.
- [x] Keep builtin string equality and ordering locale-independent.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `i18n/plural` — plain-decimal cardinal/ordinal profile VERIFIED

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **PluralCategoryOf** — accepted contract in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `PluralCategoryOf(origin is Decimal, locale is Locale, kind is PluralKind)`; `#projected value() is PluralCategory`.

- [x] Evaluate pinned CLDR plural operands without converting through float.
- [x] Distinguish visible fractional digits as required by CLDR rules.
- [x] Require every plural message set to contain `other`.
- [x] Do not assume `one` means the numeric value one in every locale.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `i18n/catalog` — bounded immutable catalogs VERIFIED

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **MessageKey** — accepted contract in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `MessageKey(context is string, identity is string)`; `context() is string`; `identity() is string`; `order(other is MessageKey) is Order`.

- [x] **MessageEntry** — accepted contract in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `key() is MessageKey`; `plural() is bool`.

- [x] **SingularMessage** — accepted contract in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `SingularMessage(key is MessageKey, text is string)`; `key() is MessageKey`; `plural() is bool`; `text() is string`.

- [x] **PluralMessage** — accepted contract in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `PluralMessage(key is MessageKey, forms is %PluralCategory:string)`; `key() is MessageKey`; `plural() is bool`; `forms() is %PluralCategory:string`.

- [x] **Translating** — accepted contract in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `membership(key is MessageKey) is bool`; `text(key is MessageKey) is string`; `plural(key is MessageKey, count is Decimal) is string`.

- [x] **MessageCatalog** — accepted contract in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `MessageCatalog(locale is Locale, entries is Sequencing<MessageEntry>)`; `membership(key is MessageKey) is bool`; `text(key is MessageKey) is string`; `plural(key is MessageKey, count is Decimal) is string`.

- [x] **FallbackTranslation** — accepted contract in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `FallbackTranslation(primary is Translating, fallback is Translating)`; `membership(key is MessageKey) is bool`; `text(key is MessageKey) is string`; `plural(key is MessageKey, count is Decimal) is string`.

- [x] **MissingMessage** — accepted contract in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `MissingMessage(key is MessageKey)`; `key() is MessageKey`; `text() is string`.

- [x] **WrongMessageEntry** — accepted contract in [locale-i18n.md](locale-i18n.md).

  Кандидаты из v3 для пересмотра: `WrongMessageEntry(expected_plural is bool, actual_plural is bool)`; `expected_plural() is bool`; `actual_plural() is bool`; `text() is string`.

- [x] Reject duplicate keys, empty identities and plural sets without `other`.
- [x] Make the wrong singular or plural projection a typed error.
- [x] Select plural forms through the catalog locale's CLDR rule.
- [x] Keep fallback order explicit and deterministic.
- [x] Do not return source identifiers silently when a translation is missing.
- [x] Format translated text through `TextTemplate` only when the caller explicitly composes them.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `i18n/gettext` — active MO/PO profile VERIFIED

[Contract, exact supported formats and bounds](gettext.md).

- [x] GettextCatalog immutable MO bytes + explicit Locale; exact singular/plural/context lookup.
- [x] PoComments ordered nominal records and grouped projections; retain interleaved comment order.
- [x] PoEntry immutable coordinates, indexed translations and explicit context presence.
- [x] PoDocument ordered entries, nominal metadata and trailing active comments.
- [x] ParsedPo eager bounded Unicode parsing with an optional explicit escape charset.
- [x] PoString derived canonical Unicode text cached once.
- [x] InvalidCatalog typed byte/scalar/whole-catalog diagnostics.
- [x] Parse GNU MO little/big endian revision 0.0; reject unsupported revisions rather than hide system-dependent strings.
- [x] Bound counts, offsets, terminators, hash words, strings and aggregate decoded text.
- [x] Decode declared UTF-8, ASCII or Latin-1; reject unsupported charsets and malformed text.
- [x] Support absent, empty and nonempty context and plural translations.
- [x] Parse Plural-Forms into a bounded AST without host eval; lazy C precedence and portable unsigned64 arithmetic.
- [x] Require nonnegative integral Decimal counts within unsigned64.
- [x] Preserve active comments, references, unknown flags, previous comments, contexts, plurals and multiline strings semantically.
- [x] Keep obsolete source outside the active model; #~ is explicitly rejected pending a separate tooling layer.
- [x] Preserve entry/comment order and trailing comments; recognize GNU #= and emit canonical #,.
- [x] Avoid global translation functions, text domains and process locale state.
- [x] GettextTranslation implements Translating and composes with explicit fallback without suppressing errors.

GREEN: focused contracts/boundaries/GNU source oracle and catalog compatibility;
GNU recompilation of all six reconstructed PO documents preserves complete catalogs.
No interpreter/native change for gettext, so full regression is not repeated.

### `runtime/testing` — context, owners and process runner VERIFIED focused; full gate 911/19,132 PASS

**Зависимости и gate:** сначала consuming std object + контракт, затем ABI/provider; opaque handles, portable wire errors, fake provider + real integration.

- [x] Accepted contextual API and [nominal runner/report bridge](testing.md).
- [x] Support `async test "name" { ... }` and await its complete future before reporting.
- [x] Make cancellation and timeout run async cleanup exactly once.
- [x] Add `test.skip(reason is string)` as the fifth contextual test action.
- [x] Attach assertion call-site module, line and column to every failure.
- [x] Preserve a stdlib assertion's external test call site instead of reporting its implementation line.
- [x] Record test start, duration, captured output and terminal status structurally.
- [x] Bound captured stdout and stderr per test and report truncation.
- [x] Add deterministic name filtering, shuffling seed and timeout inputs to the runner.
- [x] Add process-isolated parallel execution without sharing interpreter or native-provider state.
- [x] Mint unique test-only manual clock domains; guarded Atomic/Event owners retain virtual state.
- [x] Add test-only recorder and stub state owners without exposing them in production mode.
- [x] Adopt split fake.reading/fake.writing; retain deprecated fake.storage as a compatibility adapter.
- [x] Expose contextual `fake.reading.get(key is K) is V`.
- [x] Expose contextual `fake.reading.has(key is K) is bool`.
- [x] Expose contextual `fake.reading.size() is int`.
- [x] Expose contextual `fake.writing.set(key is K, value is V)`.
- [x] Expose contextual `fake.writing.remove(key is K) is bool`.
- [x] Expose contextual `fake.writing.clear()`.
- [x] Keep both fake-state capabilities unavailable as first-class values.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `testing/check` — bounded scalar/byte/generic/unified diagnostics VERIFIED focused

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **Checking** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `check() is void`.

- [x] **AsyncChecking** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `async check() is AsyncChecking`.

- [x] **CloseTo** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `CloseTo(actual is float, expected is float, relative is float, absolute is float)`; `check() is void`.

- [x] **MatchingRegex** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `MatchingRegex(origin is string, pattern is Regex)`; `check() is void`.

- [x] **TextEquals** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `TextEquals(actual is string, expected is string, context is int)`; `check() is void`.

- [x] **BytesEqual** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `BytesEqual(actual is @byte, expected is @byte, context is int)`; `check() is void`.

- [x] **SequenceEquals<T>** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `SequenceEquals(actual is Sequencing<T>, expected is Sequencing<T>, context is int)`; `check() is void`.

- [x] **Eventually** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `Eventually(condition is () is bool, clock is Timing, timeout is Duration, interval is Duration)`; `async check() is Eventually`.

- [x] **AsyncEventually** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `AsyncEventually(condition is async () is bool, clock is Timing, timeout is Duration, interval is Duration)`; `async check() is AsyncEventually`.

- [x] Make every checker report through the current contextual `test.fail()`.
- [x] Reject checker execution outside a running test with a typed `MissingTestContext` error.
- [x] Reject negative tolerances and non-positive diff context.
- [x] Non-finite input is rejected by builtin float before CloseTo; NaN comparison is inapplicable to this domain.
- [x] Infinity is likewise outside the finite float contract; scaled extreme finite comparison is covered.
- [x] Use `max(absolute, relative * max(abs(actual), abs(expected)))` for finite closeness.
- [x] Report the first byte or item mismatch and a bounded surrounding diff.
- [x] Use Stage 05 unified text diffs without reading files.
- [x] Make `Eventually` evaluate immediately, then wait through the injected clock.
- [x] Make `AsyncEventually` await each condition evaluation serially.
- [x] Stop `Eventually` at the first successful exact `bool` result.
- [x] Propagate a condition failure instead of retrying it as false.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `testing/error` — typed matchers, sync/async assertions and structured causes VERIFIED focused

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **ErrorMatching** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `match(origin is Error) is bool`; `description() is string`.

- [x] **AnyErrorMatching** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `AnyErrorMatching()`; `match(origin is Error) is bool`; `description() is string`.

- [x] **ErrorTextMatching** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `ErrorTextMatching(expected is string)`; `match(origin is Error) is bool`; `description() is string`.

- [x] **ErrorRegexMatching** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `ErrorRegexMatching(pattern is Regex)`; `match(origin is Error) is bool`; `description() is string`.

- [x] **Throwing<R>** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `Throwing(action is () is R, matching is ErrorMatching)`; `check() is void`.

- [x] **AsyncThrowing<R>** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `AsyncThrowing(action is async () is R, matching is ErrorMatching)`; `async check() is AsyncThrowing<R>`.

- [x] **MissingTestContext** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] Fail when the action returns normally or throws a nonmatching typed `Error`.
- [x] Preserve the original error as diagnostic cause when matching fails.
- [x] Let panic and programming errors remain outside typed-error matching.
- [x] Do not add runtime type reflection solely for exception assertions.
- [x] Let applications implement precise domain matchers through `ErrorMatching`.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `testing/time` — guarded ManualClock + native domain identity VERIFIED focused

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **ManualClock** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `ManualClock(start is Instant)`; `instant() is Instant`; `monotonic_instant() is MonotonicInstant`; `async wait(duration is Duration) is Timing`; `advance(duration is Duration) is void`; `set_wall(value is Instant) is void`.

- [x] Reject negative advances and waits.
- [x] Advance wall and monotonic time together through `advance()`.
- [x] Let `set_wall()` jump wall time without changing monotonic time.
- [x] Complete every due waiter in deadline and registration order.
- [x] Remove cancelled waiters promptly.
- [x] Never sleep or read a host clock.
- [x] Use ordinary dependency injection; do not replace `SystemClock` globally.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `testing/generation` — scalar/domain/filter/sequence generators VERIFIED focused

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **Generation<T>** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `sample(random is Randomizing, size is int) is Generated<T>`.

- [x] **Generated<T>** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `value() is T`; `reductions() is Iterating<T>`; `string() is string`.

- [x] **GeneratedElement<T>** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `GeneratedElement(value is T, string is string)`; `value() is T`; `string() is string`.

- [x] **BooleanGeneration** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `BooleanGeneration()`; `sample(random is Randomizing, size is int) is Generated<bool>`.

- [x] **IntegerGeneration** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `IntegerGeneration(minimum is int, maximum is int)`; `sample(random is Randomizing, size is int) is Generated<int>`.

- [x] **FiniteFloatGeneration** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `FiniteFloatGeneration(minimum is float, maximum is float)`; `sample(random is Randomizing, size is int) is Generated<float>`.

- [x] **ElementGeneration<T>** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `ElementGeneration(values is Sequencing<GeneratedElement<T>>)`; `sample(random is Randomizing, size is int) is Generated<T>`.

- [x] **SequenceGeneration<T>** — inclusive bounded lengths and lazy candidate plans; accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `SequenceGeneration(origin is Generation<T>, minimum is int, maximum is int)`; `sample(random is Randomizing, size is int) is Generated<Sequencing<T>>`.

- [x] **FilteredGeneration<T>** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `FilteredGeneration(origin is Generation<T>, condition is (T) is bool, attempts is int)`; `sample(random is Randomizing, size is int) is Generated<T>`.

- [x] **GenerationExhausted** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `GenerationExhausted(attempts is int)`; `attempts() is int`; `text() is string`.

- [x] Reject empty element sources, inverted ranges and invalid size bounds.
- [x] Let size control complexity without changing configured hard bounds.
- [x] Generate finite floats from documented bit patterns without NaN or infinity.
- [x] Shrink toward simple canonical values and preserve generator invariants.
- [x] Core reductions are finite, deterministic and unique; DistinctReductions explicitly enforces reproduction-key uniqueness for custom sources and filters without forcing lazy property frontiers.
- [x] Filter shrink candidates through the same predicate.
- [x] Fail after the explicit rejection-attempt bound.
- [x] Carry stable reproduction text inside every `Generated` object instead of invoking a static representation service.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `testing/property` — seeded sync/async cases and bounded shrinking VERIFIED focused

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **Property<T>** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `Property(origin is Generation<T>, seed is int, cases is int, maximum_size is int, shrink_limit is int, condition is (T) is bool)`; `check() is void`.

- [x] **AsyncProperty<T>** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `AsyncProperty(origin is Generation<T>, seed is int, cases is int, maximum_size is int, shrink_limit is int, condition is async (T) is bool)`; `async check() is AsyncProperty<T>`.

- [x] **PropertyReproduction** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `PropertyReproduction(seed is int, case_index is int, size is int, value is string)`; `seed() is int`; `case_index() is int`; `size() is int`; `value() is string`.

- [x] Reject non-positive case counts and negative size or shrink limits.
- [x] Create a private Stage 09 `PcgRandom` from the explicit seed.
- [x] Increase size deterministically from zero through the configured maximum.
- [x] Treat false, typed errors and cancellation as distinct property failures.
- [x] Shrink only the first failing generated case.
- [x] Explore shrink candidates deterministically within the explicit limit.
- [x] Report seed, case index, size and smallest observed counterexample.
- [x] Make the async property await one case at a time in generation order.
- [x] Do not parallelize cases because that would change reproducibility and shrinking order.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `testing/snapshot` — explicit bounded POSIX descriptor effects VERIFIED focused

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **SnapshotFile** — accepted bounded contract in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `SnapshotFile(root is Directory, name is string)`; `path() is Path`.

- [x] **TextSnapshot** — accepted bounded contract in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `TextSnapshot(origin is string)`; `bytes() is @byte`.

- [x] **JsonSnapshot** — accepted bounded contract in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `JsonSnapshot(origin is JsonValue)`; `bytes() is @byte`.

- [x] **RawSnapshot** — accepted bounded contract in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `RawSnapshot(origin is @byte)`; `bytes() is @byte`.

- [x] **Snapshot** — accepted bounded contract in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `Snapshot(origin is ByteSource, file is SnapshotFile, limit is BigInt)`; `check() is void`.

- [x] **SnapshotUpdate** — accepted bounded contract in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `SnapshotUpdate(origin is ByteSource, file is SnapshotFile, limit is BigInt)`; `update() is void`.

- [x] Reject absolute names, parent traversal, separators and non-positive limits.
- [x] Resolve every snapshot beneath its explicit root without following an escaping symlink.
- [x] Normalize text snapshots to UTF-8 and LF only through `TextSnapshot`.
- [x] Render JSON snapshots deterministically through Stage 13.
- [x] Compare snapshot bytes exactly and report a bounded text or byte diff.
- [x] Make a missing snapshot a failure during checking.
- [x] Update snapshots only through the explicitly constructed `SnapshotUpdate` object.
- [x] Write updates atomically through a sibling temporary file.
- [x] Never let a normal test run rewrite expected data.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `testing/double` — guarded native recorder/stub cursor owners VERIFIED focused

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **Recorded<T>** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `index() is BigInt`; `value() is T`.

- [x] **RecordedCalls<T>** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `RecordedCalls()`; `record(value is T) is void`; `records() is Sequencing<Recorded<T>>`; `size() is int`.

- [x] **Stub<T>** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `Stub(values is Sequencing<T>)`; `value() is T`; `size() is int`.

- [x] **StubExhausted** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `text() is string`.

- [x] Make recorder and stub state test-mode-only runtime state owners.
- [x] Preserve call and stub order exactly.
- [x] Make `records()` return an immutable snapshot.
- [x] Make `Stub.next()` throw after the configured sequence is exhausted.
- [x] Construct fresh `RecordedCalls` and `Stub` objects instead of resetting test state.
- [x] Compose recorders and stubs explicitly inside role fake actions.
- [x] Do not generate mocks through reflection or string action names.
- [x] Do not add ordered-expectation DSLs, implicit verification or call-count magic.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `testing/result` — bounded explicit value model VERIFIED focused; process runner bridge VERIFIED focused

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **TestPosition** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `TestPosition(module is string, line is int, column is int)`; `module() is string`; `line() is int`; `column() is int`.

- [x] **TestFailure** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `TestFailure(message is string)`; `TestFailure(message is string, position is TestPosition)`; `message() is string`; `located() is bool`; `position() is TestPosition`.

- [x] **TestOutput** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `TestOutput(output is @byte, errors is @byte, truncated is bool)`; `output() is @byte`; `errors() is @byte`; `truncated() is bool`.

- [x] **TestResult** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `name() is string`; `status() is TestStatus`; `duration() is Duration`; `failures() is Sequencing<TestFailure>`; `output() is TestOutput`.

- [x] **TestCounts** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `passes() is int`; `failures() is int`; `skips() is int`; `total() is int`.

- [x] **TestReport** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `start() is Instant`; `duration() is Duration`; `seed() is int`; `counts() is TestCounts`; `results() is Sequencing<TestResult>`.

- [x] Preserve discovery order in reports even when process workers finish out of order.
- [x] Make a missing `TestFailure.position()` a typed projection error.
- [x] Keep captured output as bytes until an explicit report value decodes it.
- [x] Count timeout and crash results as failed in the process exit status.
- [x] Reserve `crashed` for runner or worker termination; ordinary assertions, typed errors and panics are `failed`.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `testing/report` — human/TAP13/JSON VERIFIED focused; JUnit XML and process bridge VERIFIED focused

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [x] **HumanTestReport** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `HumanTestReport(origin is TestReport, width is int)`; `string() is string`.

- [x] **TapTestReport** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `TapTestReport(origin is TestReport)`; `string() is string`.

- [x] **JUnitTestReport** — exact-duration XML value profile; [contract](testing.md).

  Кандидаты из v3 для пересмотра: `JUnitTestReport(origin is TestReport, suite is string)`; `string() is string`.

- [x] **JsonTestReport** — accepted profile in [testing.md](testing.md).

  Кандидаты из v3 для пересмотра: `JsonTestReport(origin is TestReport)`; `string() is string`.

- [x] Keep human output deterministic and free of ANSI styling.
- [x] Emit valid TAP version 13 with escaped diagnostics.
- [x] Emit JUnit XML through Stage 13 XML objects rather than string concatenation.
- [x] Emit JSON through Stage 13 with stable member order.
- [x] Include reproduction seeds and captured output without assuming UTF-8 validity.
- [x] Escape invalid output bytes in human and TAP reports and Base64-encode them in XML and JSON reports.
- [x] Apply Stage 24 terminal styling only through an explicit output decorator.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `diagnostics/source` — DESIGN

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **SourcePosition** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SourcePosition(module is string, line is int, column is int, offset is int)`; `module() is string`; `line() is int`; `column() is int`; `offset() is int`.

- [ ] **SourceRange** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `known() is bool`; `start() is SourcePosition`; `end() is SourcePosition`.

- [ ] **KnownSourceRange** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `KnownSourceRange(start is SourcePosition, end is SourcePosition)`; `known() is bool`; `start() is SourcePosition`; `end() is SourcePosition`.

- [ ] **UnknownSourceRange** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `UnknownSourceRange()`; `known() is bool`; `start() is SourcePosition`; `end() is SourcePosition`.

- [ ] **SourceReading** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `string(module is string) is string`.

- [ ] **SourceExcerpt** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SourceExcerpt(range is SourceRange, sources is SourceReading, context is int)`; `#projected value() is string`.

- [ ] Number lines and columns from one and UTF-8 byte offsets from zero.
- [ ] Make ranges half-open and require both endpoints to belong to the same module.
- [ ] Make `start()` and `end()` fail explicitly for an unknown range.
- [ ] Keep module identifiers logical and stable; do not require a filesystem path.
- [ ] Make source access explicit so formatting a diagnostic cannot read files implicitly.
- [ ] Bound excerpt context, line length and total rendered size.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `diagnostics/model` — DESIGN

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **DiagnosticCode** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `DiagnosticCode(domain is string, value is string)`; `domain() is string`; `value() is string`; `string() is string`.

- [ ] **CallFrame** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `CallFrame(name is string, kind is CallKind, range is SourceRange)`; `name() is string`; `kind() is CallKind`; `range() is SourceRange`.

- [ ] **DiagnosticNote** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `DiagnosticNote(message is string, range is SourceRange)`; `message() is string`; `range() is SourceRange`.

- [ ] **DiagnosticContext** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `DiagnosticContext(range is SourceRange, frames is @CallFrame, notes is @DiagnosticNote, causes is @Diagnostic, suppressed is @Diagnostic)`; `range() is SourceRange`; `frames() is @CallFrame`; `notes() is @DiagnosticNote`; `causes() is @Diagnostic`; `suppressed_diagnostics() is @Diagnostic`.

- [ ] **Diagnostic** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Diagnostic(severity is DiagnosticSeverity, code is DiagnosticCode, message is string, context is DiagnosticContext)`; `severity() is DiagnosticSeverity`; `code() is DiagnosticCode`; `message() is string`; `context() is DiagnosticContext`.

- [ ] **DiagnosticReport** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `DiagnosticReport(diagnostics is @Diagnostic)`; `diagnostics() is @Diagnostic`; `failed() is bool`.

- [ ] **ErrorDiagnostic** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ErrorDiagnostic(origin is Error, code is DiagnosticCode)`; `#projected value() is Diagnostic`.

- [ ] Validate diagnostic domains and codes as portable lower-case identifiers.
- [ ] Use an unknown `SourceRange` and empty sequences instead of nullable members.
- [ ] Keep causal failures distinct from failures suppressed during cleanup.
- [ ] Detect cycles and enforce depth, frame, note and cause limits while building a bounded report.
- [ ] Convert an arbitrary `Error` to an unlocated diagnostic without inventing a stack.
- [ ] Let the runtime construct the rich diagnostic only from the actual failure carrier.
- [ ] Keep `Error` small; do not add diagnostic, source or stack actions to it.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `diagnostics/format` — DESIGN

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **HumanDiagnosticReport** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `HumanDiagnosticReport(origin is DiagnosticReport, sources is SourceReading)`; `string() is string`.

- [ ] **JsonDiagnosticReport** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `JsonDiagnosticReport(origin is DiagnosticReport)`; `string() is string`.

- [ ] Render causes and suppressed failures as an explicit tree with a configured depth limit.
- [ ] Render frames from newest to oldest and collapse only consecutive identical frames.
- [ ] Keep human output deterministic and free of ANSI control sequences.
- [ ] Apply Stage 24 styling only in a separate terminal-output decorator.
- [ ] Encode machine output through Stage 13 JSON with stable field names and ordering.
- [ ] Preserve unknown locations as explicit JSON nulls while keeping public objects non-null.
- [ ] Escape control characters and bound messages, excerpts and complete reports.
- [ ] Do not expose host stack traces or native exception text in public diagnostics.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `observability/attributes` — DESIGN

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **AttributeValue** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AttributeValue(value is JsonValue)`; `#projected value() is JsonValue`.

- [ ] **Attributes** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Attributes()`; `Attributes(values is %string:AttributeValue)`; `Attributes(values is %string:AttributeValue, limits is TelemetryLimits)`; `membership(name is string) is bool`; `value(name is string) is AttributeValue`; `values() is %string:AttributeValue`.

- [ ] **CombinedAttributes** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `CombinedAttributes(base is Attributes, overriding is Attributes)`; `#projected value() is Attributes`.

- [ ] **TelemetryLimits** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TelemetryLimits(attributes is int, events is int, batch_items is int, batch_bytes is int, queue_items is int)`; `attributes() is int`; `events() is int`; `batch_items() is int`; `batch_bytes() is int`; `queue_items() is int`.

- [ ] Accept only JSON booleans, numbers, strings and homogeneous arrays of those scalar kinds.
- [ ] Reject nested objects, nulls, nested arrays and heterogeneous arrays as telemetry attributes.
- [ ] Validate names and enforce configured count, name-length and value-length limits.
- [ ] Give the overriding set precedence without mutating either input.
- [ ] Document that secrets and unbounded identifiers do not belong in attributes.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `tracing/model` — DESIGN

**Зависимости и gate:** explicit context/span lifecycle, fake clock/exporter, bounded buffers, failure/no recursion и shutdown.

- [ ] **TraceId** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TraceId(bytes is @byte)`; `bytes() is @byte`; `string() is string`.

- [ ] **TraceIdFromString** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TraceIdFromString(origin is string)`; `#projected value() is TraceId`.

- [ ] **SpanId** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SpanId(bytes is @byte)`; `bytes() is @byte`; `string() is string`.

- [ ] **SpanIdFromString** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SpanIdFromString(origin is string)`; `#projected value() is SpanId`.

- [ ] **TraceFlags** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TraceFlags(sampled is bool)`; `sampled() is bool`.

- [ ] **TraceContext** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TraceContext(trace is TraceId, span is SpanId, flags is TraceFlags, state is string)`; `trace() is TraceId`; `span() is SpanId`; `flags() is TraceFlags`; `state() is string`.

- [ ] **SpanEvent** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SpanEvent(name is string, occurred is Instant, attributes is Attributes)`; `name() is string`; `time() is Instant`; `attributes() is Attributes`.

- [ ] **SpanLink** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SpanLink(context is TraceContext, attributes is Attributes)`; `context() is TraceContext`; `attributes() is Attributes`.

- [ ] **SpanParent** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `root() is bool`; `context() is TraceContext`.

- [ ] **RootSpanParent** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `RootSpanParent()`; `root() is bool`; `context() is TraceContext`.

- [ ] **ChildSpanParent** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ChildSpanParent(context is TraceContext)`; `root() is bool`; `context() is TraceContext`.

- [ ] **SpanStart** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SpanStart(name is string, kind is SpanKind, attributes is Attributes, parent is SpanParent, links is @SpanLink)`; `name() is string`; `kind() is SpanKind`; `attributes() is Attributes`; `parent() is SpanParent`; `links() is @SpanLink`.

- [ ] Make trace IDs exactly 16 bytes, span IDs exactly 8 bytes and reject all-zero IDs.
- [ ] Render IDs as fixed-width lower-case hexadecimal.
- [ ] Keep trace state opaque but validate its grammar and size at the boundary.
- [ ] Make asking a root `SpanParent` for `context()` fail explicitly.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `tracing/span` — DESIGN

**Зависимости и gate:** explicit context/span lifecycle, fake clock/exporter, bounded buffers, failure/no recursion и shutdown.

- [ ] **SpanRecording** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `context() is TraceContext`; `annotate(name is string, value is AttributeValue) is void`; `record(origin is SpanEvent) is void`; `mark(value is SpanStatus, message is string) is void`; `end() is void`.

- [ ] **Tracing** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `span(origin is SpanStart) is SpanRecording`.

- [ ] **ConfiguredTracing** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ConfiguredTracing(ids is TraceIdentifying, sampling is TraceSamplingPolicy, clock is Timing, destination is SpanWriting)`; `span(origin is SpanStart) is SpanRecording`.

- [ ] **NoTracing** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `NoTracing()`; `span(origin is SpanStart) is SpanRecording`.

- [ ] **SpanOperation** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SpanOperation(name is string, kind is SpanKind, status is SpanStatus, message is string)`; `name() is string`; `kind() is SpanKind`; `status() is SpanStatus`; `message() is string`.

- [ ] **SpanTiming** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SpanTiming(started is Instant, ended is Instant, duration is Duration)`; `start() is Instant`; `end() is Instant`; `duration() is Duration`.

- [ ] **FinishedSpan** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `FinishedSpan(context is TraceContext, parent is SpanParent, operation is SpanOperation, timing is SpanTiming, data is SpanData)`; `context() is TraceContext`; `parent() is SpanParent`; `operation() is SpanOperation`; `timing() is SpanTiming`; `data() is SpanData`.

- [ ] **SpanData** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SpanData(attributes is Attributes, events is @SpanEvent, links is @SpanLink)`; `attributes() is Attributes`; `events() is @SpanEvent`; `links() is @SpanLink`.

- [ ] Capture both wall start time and monotonic elapsed time.
- [ ] Make `end()` idempotent and export exactly one immutable finished span.
- [ ] Ignore recording after `end()` and count it as an internal dropped operation.
- [ ] Require callers to set error status explicitly; do not infer it from message text.
- [ ] Return a real no-op span from `NoTracing` so callers need no branches.
- [ ] Do not install an ambient current span or a process-global tracer.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `tracing/sampling` — DESIGN

**Зависимости и gate:** explicit context/span lifecycle, fake clock/exporter, bounded buffers, failure/no recursion и shutdown.

- [ ] **TraceIdentifying** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `trace() is TraceId`; `span() is SpanId`.

- [ ] **SecureTraceIds** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `SecureTraceIds(random is SecureRandom)`; `trace() is TraceId`; `span() is SpanId`.

- [ ] **TraceSamplingPolicy** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `sampled(origin is SpanStart, trace is TraceId) is bool`.

- [ ] **AlwaysSampling** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AlwaysSampling()`; `sampled(origin is SpanStart, trace is TraceId) is bool`.

- [ ] **NeverSampling** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `NeverSampling()`; `sampled(origin is SpanStart, trace is TraceId) is bool`.

- [ ] **ProbabilitySampling** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ProbabilitySampling(probability is float)`; `sampled(origin is SpanStart, trace is TraceId) is bool`.

- [ ] **ParentSampling** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ParentSampling(root is TraceSamplingPolicy)`; `sampled(origin is SpanStart, trace is TraceId) is bool`.

- [ ] Generate IDs from Stage 15 secure randomness, never from the ordinary seeded generator.
- [ ] Make probability decisions deterministic from the trace ID so all services agree.
- [ ] Preserve a valid parent's sampling decision in `ParentSampling`.
- [ ] Record no events or attributes for unsampled spans while preserving propagation context.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `tracing/propagation` — DESIGN

**Зависимости и gate:** explicit context/span lifecycle, fake clock/exporter, bounded buffers, failure/no recursion и shutdown.

- [ ] **PropagationLimits** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `PropagationLimits(fields is int, bytes is int, member_bytes is int)`; `fields() is int`; `bytes() is int`; `member_bytes() is int`.

- [ ] **W3cTraceContext** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `W3cTraceContext(headers is Headers, limits is PropagationLimits)`; `#projected value() is TraceContext`.

- [ ] **W3cTraceHeaders** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `W3cTraceHeaders(origin is TraceContext)`; `#projected value() is Headers`.

- [ ] **Baggage** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Baggage(values is %string:BaggageMember)`; `membership(name is string) is bool`; `member(name is string) is BaggageMember`; `members() is %string:BaggageMember`.

- [ ] **BaggageMember** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `BaggageMember(value is string, properties is %string:string)`; `value() is string`; `properties() is %string:string`.

- [ ] **W3cBaggage** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `W3cBaggage(header is string, limits is PropagationLimits)`; `#projected value() is Baggage`.

- [ ] **BaggageHeader** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `BaggageHeader(origin is Baggage)`; `#projected value() is string`.

- [ ] Follow W3C `traceparent`, `tracestate` and `baggage` grammar exactly.
- [ ] Apply version-specific parsing and reject unsupported extensions conservatively.
- [ ] Return a documented invalid-context error instead of silently creating a new parent.
- [ ] Keep baggage separate from span attributes and propagate it only when explicitly requested.
- [ ] Apply strict member, byte and value limits before decoding baggage.
- [ ] Never place authorization, cookies or other request headers into baggage automatically.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `tracing/export` — DESIGN

**Зависимости и gate:** explicit context/span lifecycle, fake clock/exporter, bounded buffers, failure/no recursion и shutdown.

- [ ] **SpanWriting** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `write(origin is FinishedSpan) is void`.

- [ ] **SpanExporting** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `async export(origin is @FinishedSpan) is SpanExporting`.

- [ ] **DiscardingSpans** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `DiscardingSpans()`; `write(origin is FinishedSpan) is void`.

- [ ] **CollectingSpans** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `CollectingSpans(limit is int)`; `write(origin is FinishedSpan) is void`; `spans() is @FinishedSpan`; `dropped_spans() is int`.

- [ ] **OtlpJsonSpans** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `OtlpJsonSpans(origin is @FinishedSpan, resource is Attributes)`; `#projected value() is JsonValue`.

- [ ] **OtlpHttpSpans** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `OtlpHttpSpans(endpoint is Uri, headers is Headers, client is Exchanging, resource is Attributes, limits is TelemetryLimits)`; `async export(origin is @FinishedSpan) is OtlpHttpSpans`.

- [ ] **QueuedSpans** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `QueuedSpans(destination is SpanExporting, limits is TelemetryLimits)`; `write(origin is FinishedSpan) is void`; `async run() is QueuedSpans`; `dropped_spans() is int`; `closed() is bool`; `close() is void`.

- [ ] Batch by explicit item and byte limits before serialization.
- [ ] Use OTLP/HTTP JSON so Stage 28 does not require a new protobuf subsystem.
- [ ] Apply Stage 22 timeout, TLS and retry policy explicitly at construction.
- [ ] Retry only retryable transport failures and honor cancellation and server backoff.
- [ ] Make `SpanWriting.write()` synchronous, bounded and free of network I/O.
- [ ] Run `QueuedSpans.run()` inside a Stage 19 structured-concurrency scope.
- [ ] Bound queues and expose dropped-span counts instead of blocking application work forever.
- [ ] Make `close()` stop acceptance and wake the worker; let `run()` drain accepted batches before returning.
- [ ] Put the surrounding scope's explicit deadline around waiting for `run()`.
- [ ] Never recursively instrument the exporter HTTP client.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `tracing/instrument` — DESIGN

**Зависимости и gate:** explicit context/span lifecycle, fake clock/exporter, bounded buffers, failure/no recursion и shutdown.

- [ ] **TraceLogging** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TraceLogging(origin is Logging, context is TraceContext)`; `log(level is LogLevel, message is string) is void`; `log(level is LogLevel, message is string, fields is LogFields) is void`; `log(level is LogLevel, message is string, fields is LogFields, cause is Error) is void`.

- [ ] **TracedExchanging** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TracedExchanging(origin is Exchanging, tracing is Tracing)`; `async response(request is Request) is ClientResponse`.

- [ ] **TracedHandling** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TracedHandling(origin is Handling, tracing is Tracing)`; `async handling(request is Request, context is RequestContext) is HandlingResult`.

- [ ] **TracedDbConnecting** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `TracedDbConnecting(origin is DbConnecting, tracing is Tracing)`; `async connection() is DbConnection`.

- [ ] Add only trace and span IDs to logs; never copy all baggage or span attributes.
- [ ] Inject outbound context and extract inbound context only at the HTTP decorators.
- [ ] Record HTTP method, route template and status without raw query strings or credentials.
- [ ] Record database system and normalized operation without statement values.
- [ ] End spans on success, typed error, panic and cancellation.
- [ ] Keep instrumentation decorators out of exporter transports to prevent recursion.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `metrics/model` — DESIGN

**Зависимости и gate:** collector capability и bounded publication, immutable metric identity, monotonicity/overflow, shutdown.

- [ ] **MetricDescriptor** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `MetricDescriptor(name is string, kind is MetricKind, unit is string, description is string)`; `name() is string`; `kind() is MetricKind`; `unit() is string`; `description() is string`.

- [ ] **MetricCounting** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `add(value is float, attributes is Attributes) is void`.

- [ ] **MetricGauging** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `set(value is float, attributes is Attributes) is void`.

- [ ] **MetricRecording** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `record(value is float, attributes is Attributes) is void`.

- [ ] **HistogramBoundaries** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `HistogramBoundaries(values is @float)`; `values() is @float`.

- [ ] **HistogramBucket** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `HistogramBucket(upper is float, count is int)`; `upper_bound() is float`; `count() is int`.

- [ ] **HistogramValue** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `HistogramValue(count is int, sum is float, buckets is @HistogramBucket)`; `count() is int`; `sum() is float`; `buckets() is @HistogramBucket`.

- [ ] **MetricValue** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `kind() is MetricKind`.

- [ ] **ScalarMetricValue** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ScalarMetricValue(kind is MetricKind, value is float)`; `kind() is MetricKind`; `float() is float`.

- [ ] **HistogramMetricValue** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `HistogramMetricValue(value is HistogramValue)`; `kind() is MetricKind`; `histogram() is HistogramValue`.

- [ ] **MetricSample** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `MetricSample(descriptor is MetricDescriptor, attributes is Attributes, occurred is Instant, value is MetricValue)`; `descriptor() is MetricDescriptor`; `attributes() is Attributes`; `time() is Instant`; `value() is MetricValue`.

- [ ] **MetricSnapshot** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `MetricSnapshot(collected is Instant, samples is @MetricSample)`; `time() is Instant`; `samples() is @MetricSample`.

- [ ] Require counters to receive finite non-negative deltas.
- [ ] Let up/down counters accept finite signed deltas and gauges accept finite values.
- [ ] Require strictly increasing finite histogram boundaries.
- [ ] Keep histogram counts cumulative and include the positive-infinity bucket.
- [ ] Do not implement client-side quantile summaries because they cannot be merged correctly.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `metrics/state` — DESIGN

**Зависимости и gate:** collector capability и bounded publication, immutable metric identity, monotonicity/overflow, shutdown.

- [ ] **MetricCollecting** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `snapshot() is MetricSnapshot`.

- [ ] **Metrics** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Metrics(clock is Timing, limits is MetricLimits)`; `snapshot() is MetricSnapshot`.

- [ ] **Counter** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Counter(metrics is Metrics, descriptor is MetricDescriptor)`; `add(value is float, attributes is Attributes) is void`.

- [ ] **UpDownCounter** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `UpDownCounter(metrics is Metrics, descriptor is MetricDescriptor)`; `add(value is float, attributes is Attributes) is void`.

- [ ] **Gauge** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Gauge(metrics is Metrics, descriptor is MetricDescriptor)`; `set(value is float, attributes is Attributes) is void`.

- [ ] **Histogram** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Histogram(metrics is Metrics, descriptor is MetricDescriptor, boundaries is HistogramBoundaries)`; `record(value is float, attributes is Attributes) is void`.

- [ ] **MetricLimits** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `MetricLimits(instruments is int, series is int, attributes is int)`; `instruments() is int`; `series() is int`; `attributes() is int`.

- [ ] Reject duplicate instrument names with incompatible kind, unit or boundaries.
- [ ] Own all instrument state inside one explicitly passed `Metrics` object.
- [ ] Register an instrument lazily on its first manipulator call, never during construction.
- [ ] Bound instruments and distinct attribute series and count rejected updates.
- [ ] Never coalesce excess labels into a misleading synthetic series.
- [ ] Make snapshot collection deterministic and safe between cooperative task turns.
- [ ] Provide no factory methods, process-global metrics, default labels or callback-based observable gauges.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `metrics/export` — DESIGN

**Зависимости и gate:** collector capability и bounded publication, immutable metric identity, monotonicity/overflow, shutdown.

- [ ] **MetricExporting** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `async export(origin is MetricSnapshot) is MetricExporting`.

- [ ] **PrometheusMetrics** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `PrometheusMetrics(origin is MetricSnapshot)`; `string() is string`.

- [ ] **OtlpJsonMetrics** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `OtlpJsonMetrics(origin is MetricSnapshot, resource is Attributes)`; `#projected value() is JsonValue`.

- [ ] **OtlpHttpMetrics** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `OtlpHttpMetrics(endpoint is Uri, headers is Headers, client is Exchanging, resource is Attributes, limits is TelemetryLimits)`; `async export(origin is MetricSnapshot) is OtlpHttpMetrics`.

- [ ] **PeriodicMetrics** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `PeriodicMetrics(source is MetricCollecting, destination is MetricExporting, interval is Duration, clock is Timing)`; `async run() is PeriodicMetrics`; `closed() is bool`; `close() is void`.

- [ ] Render Prometheus names and labels deterministically and escape them exactly once.
- [ ] Reject descriptors that cannot be represented without a naming collision.
- [ ] Export cumulative counters and histograms and current gauges.
- [ ] Bound export payloads and split only between complete metric series.
- [ ] Make `close()` request a cooperative stop and let `run()` export the last complete snapshot before returning.
- [ ] Never recursively measure the exporter HTTP client.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `metrics/instrument` — DESIGN

**Зависимости и gate:** collector capability и bounded publication, immutable metric identity, monotonicity/overflow, shutdown.

- [ ] **MeasuredExchanging** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `MeasuredExchanging(origin is Exchanging, metrics is Metrics)`; `async response(request is Request) is ClientResponse`.

- [ ] **MeasuredHandling** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `MeasuredHandling(origin is Handling, metrics is Metrics)`; `async handling(request is Request, context is RequestContext) is HandlingResult`.

- [ ] **MeasuredDbConnecting** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `MeasuredDbConnecting(origin is DbConnecting, metrics is Metrics)`; `async connection() is DbConnection`.

- [ ] Record request counts, active requests and elapsed histograms with bounded attributes.
- [ ] Use route templates rather than raw paths for server metric labels.
- [ ] Keep status classes bounded and do not label by error message or database statement.
- [ ] Expose runtime and process metrics only through separately constructed collectors.
- [ ] Do not make metric instrumentation a side effect of tracing.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `profiling/cpu` — DESIGN

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **ProfileFrame** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ProfileFrame(name is string, range is SourceRange)`; `name() is string`; `range() is SourceRange`.

- [ ] **ProfileSample** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ProfileSample(occurred is MonotonicInstant, frames is @ProfileFrame)`; `time() is MonotonicInstant`; `frames() is @ProfileFrame`.

- [ ] **CpuProfile** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `CpuProfile(started is Instant, duration is Duration, samples is @ProfileSample, dropped is int)`; `start() is Instant`; `duration() is Duration`; `samples() is @ProfileSample`; `dropped_samples() is int`.

- [ ] **CpuProfiling<R>** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `CpuProfiling(action is () is R, interval is Duration, limit is int, clock is Timing)`; `result() is Profiled<R>`.

- [ ] **AsyncCpuProfiling<R>** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AsyncCpuProfiling(action is async () is R, interval is Duration, limit is int, clock is Timing)`; `async result() is Profiled<R>`.

- [ ] **Profiled<R>** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Profiled(value is R, profile is CpuProfile)`; `value() is R`; `profile() is CpuProfile`.

- [ ] **FoldedProfile** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `FoldedProfile(origin is CpuProfile)`; `string() is string`.

- [ ] Sample only at runtime safe points and document that native blocking time is attributed to its native frame.
- [ ] Bound samples and stack depth and count dropped samples.
- [ ] Preserve the action result or failure exactly and always stop the sampler during cleanup.
- [ ] Keep profiling explicit; do not add a global signal-driven profiler.
- [ ] Render folded stacks deterministically for flame-graph tools.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `profiling/allocation` — DESIGN

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **AllocationSite** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AllocationSite(type is string, range is SourceRange)`; `type() is string`; `range() is SourceRange`.

- [ ] **AllocationStat** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AllocationStat(site is AllocationSite, count is int, bytes is int)`; `site() is AllocationSite`; `count() is int`; `bytes() is int`.

- [ ] **AllocationProfile** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AllocationProfile(stats is @AllocationStat, dropped is int)`; `stats() is @AllocationStat`; `dropped_sites() is int`.

- [ ] **AllocationProfiling<R>** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AllocationProfiling(action is () is R, limit is int)`; `result() is AllocationProfiled<R>`.

- [ ] **AllocationProfiled<R>** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AllocationProfiled(value is R, profile is AllocationProfile)`; `value() is R`; `profile() is AllocationProfile`.

- [ ] Count language-visible allocation size rather than host allocator overhead.
- [ ] Aggregate by language type and source site with a strict site limit.
- [ ] Disable allocation hooks completely outside an explicit profiling scope.
- [ ] Preserve the action result or failure and drain cleanup before finalizing the profile.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.

### `coverage` — DESIGN

**Зависимости и gate:** immutable represented entity + explicit parsing/printing/effect contract; boundary inputs, deterministic fakes, typed failures, no utility namespace.

- [ ] **CoveragePoint** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `CoveragePoint(kind is CoverageKind, range is SourceRange, hits is int)`; `kind() is CoverageKind`; `range() is SourceRange`; `hits() is int`.

- [ ] **ModuleCoverage** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `ModuleCoverage(module is string, points is @CoveragePoint)`; `module() is string`; `points() is @CoveragePoint`; `covered_points() is int`; `total() is int`.

- [ ] **CoverageReport** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `CoverageReport(modules is @ModuleCoverage)`; `modules() is @ModuleCoverage`; `covered_points() is int`; `total() is int`; `ratio() is float`.

- [ ] **CoverageCollecting<R>** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `CoverageCollecting(action is () is R, modules is @string)`; `result() is Covered<R>`.

- [ ] **AsyncCoverageCollecting<R>** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `AsyncCoverageCollecting(action is async () is R, modules is @string)`; `async result() is Covered<R>`.

- [ ] **Covered<R>** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `Covered(value is R, report is CoverageReport)`; `value() is R`; `report() is CoverageReport`.

- [ ] **LcovCoverage** — определить сущность и контракт до реализации.

  Кандидаты из v3 для пересмотра: `LcovCoverage(origin is CoverageReport)`; `string() is string`.

- [ ] Count executable statements and every reachable decision branch separately.
- [ ] Keep source identities stable and sort modules and positions deterministically.
- [ ] Include uncovered points known from the compiled module, not only observed points.
- [ ] Restrict collection to an explicit module allow-list.
- [ ] Preserve the action result or failure and finalize coverage after cleanup.
- [ ] Integrate Stage 27 tests by wrapping the runner explicitly, never through hidden test mode.

**Готовность:** CONTRACT в v4 → failing tests → std/provider → focused/полная регрессия → исполняемый пример.


## Контроль покрытия

Проверены все 320 исходных module headings. Изначально в очередь вошли 224 области, 1025 отсутствующих candidate types и 3043 candidate constructor/action signatures. После завершения BigInt и PrettyJson остаются 222 области, 1017 candidate types и 3027 candidate signatures. Задача предупреждений Perl runtime учитывается отдельно. Это остаток для проектирования, не обещание реализовать все классы v3.

Языковые области: `builtin/conversion`, `builtin/array`, `builtin/map`, `builtin/range`, `builtin/string`, `builtin/integer-math`, `builtin/float-math`, `builtin/rat-math`, `builtin/async` — проверяются семантикой языка, не дублируются в std. Уже существующие runtime modules перечислены в API-аудите; их расширение должно быть привязано к конкретной CONTRACT-задаче.

CSV printing follow-up: one further candidate type (CsvString) and its two
constructor/action signatures are implemented. Historical remaining scope after printing alone:
222 areas, 1016 candidate types, 3025 candidate signatures. The parsing follow-up
below supersedes the parsing status; dialects and the separately tracked Perl
recursion diagnostic remain open.

CSV parsing follow-up: CsvRows and InvalidCsv are implemented with the fixed-format
contract in csv-guide.md. The CSV area remains open. Two further candidate types are resolved; their
legacy signatures are superseded by the smaller fixed-format contract, so the
historical signature tally above is not a count of the approved remaining API.
Internal CsvPrefix/CsvMode are implementation collaborators, not completed v3
candidates. Parsing and printing budgets are implemented. Next CSV tasks:
explicit resource row-reading; configurable dialect behavior is implemented; no blocking
I/O in Cursor.

The strings/join and numbers/bigrational follow-ups resolve two further scope
areas; the original v3 candidate-count figures above are historical, not a
current promise to implement every superseded signature.


### HTTP2 owned-session progress — 2026-10-12

[Owned session](http2-session.md) is implemented and verified, including typed multiplexing on real TCP, reverse-order bodies, stream cancellation/reset, GOAWAY, flow credit and completed-body retention after close. Final focused nine files / 79 assertions PASS; interpreter cancellation fixes required the full gate: 699 files / 12455 assertions PASS. Public-client shared H2 pool admission, mixed ALPN negotiation, retry and other HTTP modules remain unchecked until their own focused tests pass.


### HTTP2 verified HTTPS and shared client pool

[Opening](http2-connecting.md) verifies URL-derived certificate identity/SNI and actual h2 ALPN before preface. An explicit H2 policy in the normal [HttpClient](http2-client.md) enables shared physical owners, multiplexed leases, stream-local abandonment/body deadlines, checked-peer reuse and GOAWAY replacement. Final root H2 ten files /83 assertions PASS; unchanged H1 client/decorator compatibility eleven files /89 PASS. Interpreter ownership correction full gate: 700/12465 PASS; std-only client changes use focused gates. Mixed ALPN, replay-safe retry and remaining HTTP modules stay open. PERF-25 records visible continuation-depth warnings and an open investigation.


### Mixed HTTP1/H2 ALPN and shared conversation admission

[Negotiation contract](http-alpn.md): HttpClientProtocols validates exact enabled/advertised agreement; HttpSelectedConnecting adopts one checked verified transport according to actual ALPN. HttpClient supports a mixed H1.1/H2 policy; absent ALPN selects enabled H1, ordinary cleartext uses H1, H2-only remains explicit prior knowledge. The common physical pool serializes actual H1 body leases and multiplexes H2. Unsent queued requests are re-admitted after conversation retirement; emitted requests are not replayed. Whole admission tasks are supervised so client close cancels queues before releasing references. Final H1/H2/decorator/mixed compatibility: 28 files /240 assertions PASS. Std-only; the last interpreter full gate stays 700/12465 PASS. Replay-safe stale retry and remaining HTTP modules follow.


### Replay-safe stale HTTP1 retry

[Optional Retrying](http-retry.md) performs at most one replacement exchange for
an idempotent request with a replayable body after zero-response-byte failure on
a reused H1 conversation. Partial/1xx bytes, fresh errors, POST, one-shot bodies,
timeout/cancellation and broad TLS failures do not qualify. Direct session errors
remain compatible. Real TCP, mixed H1 and verified close_notify TLS are covered;
affected H1/H2/decorator compatibility: 33 files /295 assertions PASS. Std-only,
no repeated full regression. H2 unprocessed replay and other HTTP modules remain
open.


### Scoped Digest SHA-256

[Digest profile](http-digest.md) supports SHA-256/SHA-256-sess, qop=auth, userhash,
fresh secure cnonce, atomic bounded nonce counts, direct HTTPS scope, one bounded
stale refresh and optional header/trailer rspauth verification. Generic received
challenge parsing handles combined schemes, token68, quoted commas and bounded
empty list members. Final client/decorator/secret/Digest compatibility: 18 files
/144 assertions PASS; inline and independent actual HTTPS CLI PASS. No interpreter
or provider code changed, so no full regression repeated. SHA-512/256, NFC
credentials, auth-int and preemptive nonce caching remain open.


### Digest SHA-512/256 and UTF-8/NFC

[Modern Digest profile](http-digest.md) adds SHA-512-256/-sess, explicit UTF-8
credential NFC normalization, userhash/username* and exact UTF-8 realm wire
handling. Existing ASCII profiles remain covered. Correct algorithm vectors
agree with independent OpenSSL; RFC7616's truncated-SHA-512 example has a
reported erratum and is documented. Final focused 3/35 PASS; required native
full gate 727 files /12773 assertions PASS, unchanged 2079-file inventory.
Auth-int and preemptive nonce caching remain open. Proxy work is separate.

### HTTP cache metadata foundation

[Bounded cache metadata](http-cache-metadata.md) implements Cache-Control and
private-cache freshness/age over explicit wall receipt and monotonic elapsed
coordinates. Five files /90 assertions and inline PASS. Std-only; no full rerun.
Body storage, Vary, revalidation and concurrent collapse remain open.

### Completed private HTTP cache storage

[Memory and streaming consumer](http-cache.md) implement bounded completed GET200
storage, Vary/auth/cookie partitioning, newest-Date selection, independent bodies
and unsafe invalidation with a late-GET barrier. Six files /40 assertions PASS,
including actual TCP. Std-only; no full rerun. Revalidation/collapse remain open.

### HTTP cache conditional validation

[Private cache](http-cache.md) now validates ETag/Last-Modified, merges identified
304 metadata over complete bytes, constrains request freshness and handles
only-if-cached without network effects. Original no-store and hop-field rules
remain effective through sanitization. Seven distinct files /43 assertions PASS;
std-only/no full. Concurrent collapse and stale permissions remain open.

### Cache admission and immutable header names

[Admission](http-cache.md) collapses ordinary concurrent validation and retains
full-body publication ownership; explicit waiter cancellation retires its key.
Fresh hits bypass hashing and immutable HeaderName canonicalization is memoized.
58 targeted HTTP files /523 assertions PASS; 2112 source hashes unchanged. No
interpreter/native change/full rerun. PERF-27/28 report remaining performance work.
Explicit stale permission and concurrent no-cache sharing are the next slice.

### WebSocket value foundation

[Typed values](websocket.md) implement text/binary projection errors, explicit
message/frame/queue limits and UTF-8-sized wire close values with pinned IANA codes.
Four assertions and inline PASS; std-only/no full rerun. Framing/transport follow.


Latest required native-change gate: bounded dynamic string-map provider, 779files/13,120assertions PASS, frozen2172inputs verified unchanged. Later std-only routing/forms gates are separate and do not repeat full regression.
