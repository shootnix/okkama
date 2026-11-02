# Встроенные тесты и документация std

Требование распространяется на каждый Okkama-модуль корневой `std/`, включая
native declarations. Существующие внешние Perl-тесты его не заменяют.

Для каждого модуля при работе по очереди:

- [ ] Комментарии внутри `.ok`: сущность, пример, конструкторы и методы,
  ограничения, пустой вход, ошибки, повторные вызовы, владение и закрытие.
- [ ] Встроенные `test "..."` с независимыми ожидаемыми результатами,
  ветвями ошибок, граничными значениями и жизненным циклом.
- [ ] Запустить модуль как корневой test program и проверить каждое assertion.
- [ ] Запустить внешние целевые тесты изменённого API и связанных модулей.
- [ ] Обновить этот список. Наличие test-блоков не означает полное покрытие.

Команды из корня (Perl 5.42):

```sh
OKKAMA_STDLIB_DIR=std perl okkama-pl/bin/okkama -t std/csv.ok
cd okkama-pl
prove -l t/interpreter/266-std-v4-inline-tests.t
```

Вторая команда запускает каждый модуль с test-блоками отдельным корнем:
тесты зависимостей по правилам языка не запускаются через импорт. Полная
регрессия обязательна при изменении интерпретатора; для std-only изменений
используются встроенные, целевые внешние и связанные проверки.

Это явный долг существующей библиотеки: модули без встроенных тестов перечислены
ниже, их нельзя считать покрытыми этим механизмом. Retrofit выполняется вместе
с задачами соответствующих модулей, затем отдельными задачами по этому списку.
Подробность комментариев требует ручного ревью; количество комментариев её
не доказывает.

| Модуль | Блоков test | Состояние |
| --- | ---: | --- |
| [mail/smtp_reply.ok](../../std/mail/smtp_reply.ok) | 2 | PASS: split CRLF/remainder and multiline code mismatch |
| [mail/smtp.ok](../../std/mail/smtp.ok) | 2 | PASS: SIZE presence, unknown extensions and greeting isolation |
| [mail/smtp_data.ok](../../std/mail/smtp_data.ok) | 1 | PASS: pre-stuff content size and dot-only line transparency |
| [mail/smtp_reading.ok](../../std/mail/smtp_reading.ok) | 1 | PASS: completed reply snapshot, consumed bytes and exact remainder; async behavior covered by test 369 |
| [mail/imap_name.ok](../../std/mail/imap_name.ok) | 2 | PASS: ampersand/supplementary-scalar round trip and always-quoted mailbox arguments |
| [mail/imap.ok](../../std/mail/imap.ok) | 2 | PASS: unknown advertisements without inferred features and distinct ordered AUTH mechanisms |
| [mail/imap_response.ok](../../std/mail/imap_response.ok) | 2 | PASS: status braces/remainder and zero literal followed by final CRLF |
| [mail/imap_reading.ok](../../std/mail/imap_reading.ok) | 1 | PASS: completed response snapshot, consumed length and exact remainder; async behavior covered by test 373 |
| [mail/imap_search.ok](../../std/mail/imap_search.ok) | 1 | PASS: typed bounded criteria |
| [mail/imap_uid.ok](../../std/mail/imap_uid.ok) | 1 | PASS: stable UID deduplication |
| [mail/imap_data.ok](../../std/mail/imap_data.ok) | 1 | PASS: NIL/empty/list distinction |
| [mail/imap_command_reading.ok](../../std/mail/imap_command_reading.ok) | 1 | PASS: aligned tagged rejection |
| [mail/imap_command.ok](../../std/mail/imap_command.ok) | 1 | PASS: modified UTF-7 command quoting |
| [mail/imap_exchange.ok](../../std/mail/imap_exchange.ok) | 1 | PASS: remainder and tag serial state |
| [mail/imap_metadata.ok](../../std/mail/imap_metadata.ok) | 1 | PASS: ESEARCH count and extrema |
| [mail/imap_state.ok](../../std/mail/imap_state.ok) | 1 | PASS: absent versus zero observations |
| [mail/imap_message.ok](../../std/mail/imap_message.ok) | 1 | PASS: missing FETCH content |
| [mail/imap_session.ok](../../std/mail/imap_session.ok) | 1 | PASS: unselected initial state |
| [mail/imap_changes.ok](../../std/mail/imap_changes.ok) | 1 | PASS: unknown assigned UIDs |
| [mail/pop3_reply.ok](../../std/mail/pop3_reply.ok) | 2 | PASS: negative multiline termination/remainder and dot unstuffing |
| [mail/pop3_reading.ok](../../std/mail/pop3_reading.ok) | 1 | PASS: completed status/remainder snapshot; async contracts covered by test 388 |
| [mail/mbox_store.ok](../../std/mail/mbox_store.ok) | 1 | PASS: pure coordinate; filesystem effects and successor lifecycle covered by Perl fixtures |
| [mail/mbox.ok](../../std/mail/mbox.ok) | 2 | PASS: empty archive and nested From quoting round trip; expanded boundaries covered by Perl fixtures |
| [mail/mailbox.ok](../../std/mail/mailbox.ok) | 6 | PASS: flag variants/sets, canonical filename flags and stored message snapshot; filesystem behavior covered by Perl fixtures |
| [mail/message.ok](../../std/mail/message.ok) | 3 | PASS: Bcc view/source isolation, strict MessageId and unknown date offset |
| [mime/types.ok](../../std/mime/types.ok) | 2 | PASS: lexical standard lookup, compound override and invalid extension |
| [mime/part.ok](../../std/mime/part.ok) | 3 | PASS: default metadata, multipart round trip, depth bounds and missing closing delimiter |
| [url/percent.ok](../../std/url/percent.ok) | 2 | PASS: separator encoding and one-pass decoding; expanded boundary/error coverage in focused Perl tests |
| [url/value.ok](../../std/url/value.ok) | 3 | PASS: presence and reserved escapes; expanded boundary/error coverage in focused Perl tests |
| [url/query.ok](../../std/url/query.ok) | 1 | PASS: assigned flags and duplicate parameters; expanded boundary/error coverage in focused Perl tests |
| [url/form.ok](../../std/url/form.ok) | 1 | PASS: space versus literal plus; expanded boundary/error coverage in focused Perl tests |
| [message/header.ok](../../std/message/header.ok) | 1 | PASS: case-insensitive lookup and spelling; expanded boundary/error coverage in focused Perl tests |
| [mime/media_type.ok](../../std/mime/media_type.ok) | 1 | PASS: canonical type and extended value; expanded boundary/error coverage in focused Perl tests |
| [mime/disposition.ok](../../std/mime/disposition.ok) | 2 | PASS: decoded filename data; expanded boundary/error coverage in focused Perl tests |
| [mime/transfer.ok](../../std/mime/transfer.ok) | 2 | PASS: binary byte preservation; expanded boundary/error coverage in focused Perl tests |
| [mime/word.ok](../../std/mime/word.ok) | 2 | PASS: UTF-8 B round trip; expanded boundary/error coverage in focused Perl tests |
| [async/barrier.ok](../../std/async/barrier.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [async/cancellation.ok](../../std/async/cancellation.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [async/channel.ok](../../std/async/channel.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [async/collect.ok](../../std/async/collect.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [async/event.ok](../../std/async/event.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [async/lock.ok](../../std/async/lock.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [async/scope.ok](../../std/async/scope.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [async/semaphore.ok](../../std/async/semaphore.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [async/timeout.ok](../../std/async/timeout.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [calendar.ok](../../std/calendar.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [checksum/adler32.ok](../../std/checksum/adler32.ok) | 3 | GREEN: empty/Wikipedia/binary vectors, immutable branches, normalized and invalid seeds |
| [checksum/crc.ok](../../std/checksum/crc.ok) | 5 | GREEN: CRC-32/16 reference vectors and parameters, unsigned bytes, branches, empty chunks and invalid seeds |
| [checksum/state.ok](../../std/checksum/state.ok) | 3 | GREEN: nominal role, immutable fake branches, empty chunks and seed diagnostics |
| [collections/combinatorics.ok](../../std/collections/combinatorics.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [collections/combining.ok](../../std/collections/combining.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [collections/deque.ok](../../std/collections/deque.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [collections/enumeration.ok](../../std/collections/enumeration.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [collections/frequencies.ok](../../std/collections/frequencies.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [collections/heap.ok](../../std/collections/heap.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [collections/iteration.ok](../../std/collections/iteration.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [collections/list.ok](../../std/collections/list.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [collections/ordering.ok](../../std/collections/ordering.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [collections/repetition.ok](../../std/collections/repetition.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [collections/sequence.ok](../../std/collections/sequence.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [collections/set.ok](../../std/collections/set.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [collections/views.ok](../../std/collections/views.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [compare.ok](../../std/compare.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [config/ini.ok](../../std/config/ini.ok) | 6 | Встроенные tests ordered coordinates, duplicates, budgets и nominal emission; комментарии контракта |
| [csv.ok](../../std/csv.ok) | 7 | Есть встроенные тесты; проверить полноту контракта |
| [csv/input.ok](../../std/csv/input.ok) | 7 | Есть встроенные тесты и комментарии контракта |
| [compression/state.ok](../../std/compression/state.ok) | 5 | GREEN: finite owned output, cumulative budget, delayed output, fake lifecycle and typed errors |
| [archive/path.ok](../../std/archive/path.ok) | 3 | GREEN: exact spelling/components, traversal rejection and typed parent absence; 10 external consumer groups |
| [archive/metadata.ok](../../std/archive/metadata.ok) | 3 | GREEN: immutable snapshot, typed absence and exact epoch conversion; 11 external groups include presence variants and three TZ settings |
| [archive/entry.ok](../../std/archive/entry.ok) | 3 | GREEN: immutable file/role coordinates, distinct links and device invariants; 10 external groups include nominal visitor dispatch/errors and absent non-file data actions |
| [archive/limits.ok](../../std/archive/limits.ok) | 4 | GREEN: positive policy, path length, cumulative actual-byte accounting, phase/branch invariants and diagnostics; 14 external consumer groups |
| [archive/stream.ok](../../std/archive/stream.ok) | 3 | GREEN: local role EOF/finish/close contracts; 13 external groups verify concrete callbacks, errors, ownership and no cursors; root imported-action signature limitation documented |
| [archive/tar.ok](../../std/archive/tar.ok) | 8 | GREEN: writing fields/framing/time, input numeric/time/attributes, stream counters and local extension lifetime; 12 writing + 37 value-reading + 25 streaming groups, independent fixtures, short reads, admission, lifecycle and interoperation |
| [archive/zip.ok](../../std/archive/zip.ok) | 2 | GREEN: method/default/explicit level validation; 11 external choice groups + 5 member-binding groups cover levels, bounds, equality, effect-free composition and nominal variants/iteration; imported ArchiveEntry.fake is unavailable to root inline tests; no codec implementation |
| [compression/read.ok](../../std/compression/read.ok) | 3 | GREEN: delayed output, bounded buffering, stable EOF, validation and owned-buffer close; 13 external groups check public wrappers and failures |
| [digest/blocks.ok](../../std/digest/blocks.ok) | 3 | GREEN: padding boundaries, lazy block fake, empty append and typed failures |
| [digest/legacy.ok](../../std/digest/legacy.ok) | 3 | GREEN: MD5/SHA-1 raw empty/abc vectors, observation then append, immutable origins |
| [digest/state.ok](../../std/digest/state.ok) | 2 | GREEN: immutable byte-echo fake, branches, saved results and empty chunks |
| [encoding/base32.ok](../../std/encoding/base32.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [encoding/base64.ok](../../std/encoding/base64.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [encoding/base85.ok](../../std/encoding/base85.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [encoding/binary.ok](../../std/encoding/binary.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [encoding/hex.ok](../../std/encoding/hex.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [encoding/quoted_printable.ok](../../std/encoding/quoted_printable.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [encoding/single_byte.ok](../../std/encoding/single_byte.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [encoding/utf.ok](../../std/encoding/utf.ok) | 3 | Встроенные сценарии наблюдений, ошибок и границ; комментарии контракта |
| [error.ok](../../std/error.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [fs/directory.ok](../../std/fs/directory.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [fs/file.ok](../../std/fs/file.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [fs/glob.ok](../../std/fs/glob.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [fs/link.ok](../../std/fs/link.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [fs/lock.ok](../../std/fs/lock.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [fs/path.ok](../../std/fs/path.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [fs/stat.ok](../../std/fs/stat.ok) | 1 | GREEN: PermissionMode bit groups and bounds; external test covers all 4096 modes; other filesystem stat behavior remains inline-test debt |
| [fs/temp.ok](../../std/fs/temp.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [fs/watch.ok](../../std/fs/watch.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [html/entities.ok](../../std/html/entities.ok) | 6 | GREEN: escaping policies, Unicode, decoding/recovery, markup preservation, roundtrip/repeats; комментарии контракта |
| [html/events.ok](../../std/html/events.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [io/buffer.ok](../../std/io/buffer.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [io/decorators.ok](../../std/io/decorators.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [io/stdin.ok](../../std/io/stdin.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [io/stdout.ok](../../std/io/stdout.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [io/stream.ok](../../std/io/stream.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [io/text.ok](../../std/io/text.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [json.ok](../../std/json.ok) | 5 | GREEN: построение, запись, повторные имена, чтения с указателями, optional, круговой путь |
| [math/aggregate.ok](../../std/math/aggregate.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [math/constants.ok](../../std/math/constants.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [math/decimal.ok](../../std/math/decimal.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [math/rounding.ok](../../std/math/rounding.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [net/failure.ok](../../std/net/failure.ok) | 2 | GREEN: operation/kind/diagnostic identity and catchable values; 31 external groups verify portable mapping, applicable boundaries, bare rethrow, TimedOut/Cancelled and real unsupported core.net input |
| [net/endpoint.ok](../../std/net/endpoint.ok) | 4 | GREEN: typed port/zone diagnostics and scoped IPv6 formatting; 34 focused assertions; file CLI needs the inline import-scope runtime fix |
| [net/public_suffix.ok](../../std/net/public_suffix.ok) | 1 | Pinned private boundaries and wildcard exceptions; suites 409/native 61 cover full dataset integrity and indexed rule observation bounds |
| [net/interface.ok](../../std/net/interface.ok) | 1 | GREEN: immutable fixture snapshot; 7 focused assertions; system enumeration remains open |
| [net/ip.ok](../../std/net/ip.ok) | 5 | GREEN: canonical identity, rejected decorations and CIDR endpoints; 36 value groups, 76 property assertions, 18 CIDR groups including all 33/129 prefixes; 11 lazy traversal and 11 interface assertions |
| [net/dns.ok](../../std/net/dns.ok) | 2 async | GREEN: fixed requested-port lookup and unstarted system cancellation; native/std tests cover order/scopes, limits and worker lifecycle |
| [crypto/x509/value.ok](../../std/crypto/x509/value.ok) | 1 | GREEN: absent versus zero CA depth; 434 covers period order, inclusive equality, nominal clock domains and coherent constraint failures |
| [crypto/x509/name.ok](../../std/crypto/x509/name.ok) | 1 | GREEN: multivalued RDN membership; 435 covers repeated OIDs, Unicode, canonical roots/arcs, snapshot sampling and pre-item budgets |
| [crypto/x509/certificate.ok](../../std/crypto/x509/certificate.ok) | 3 | GREEN: empty DER/chain and exact unknown purpose OID; 441/native 39 add BC/KU/EKU schemas, canonicality, depth/OID limits and unknown critical payload lookup |
| [crypto/x509/trust.ok](../../std/crypto/x509/trust.ok) | 3 | GREEN: empty roots, unstarted system request and nominal identity kinds; 440/442/443 cover roots, acquisition and actual signed verification without depending on host trust |
| [encoding/pem.ok](../../std/encoding/pem.ok) | 1 | GREEN: independent snapshot cursor branches/order/EOF; 439/native 38 count one bulk parse and no full-document reparses per item; existing 187 compatibility PASS |
| [runtime/pem.ok](../../std/runtime/pem.ok) | 1 | GREEN: direct malformed bulk input retains portable invalid_data |
| [runtime/x509.ok](../../std/runtime/x509.ok) | 1 | GREEN: malformed native envelope retains portable invalid_data category |
| [crypto/key.ok](../../std/crypto/key.ok) | 0 | Declarative PublicKey role: real certificate-backed nominal use covered by 436; no standalone behavior/fake key inline test |
| [net/stream.ok](../../std/net/stream.ok) | 1 | GREEN: async roles and concrete IP halves; alias admission, queued shutdown and lock-grant cancellation verified |
| [net/tcp.ok](../../std/net/tcp.ok) | 3 | GREEN: numeric connect/listen/accept, actual endpoints, option application and unpublished-owner cleanup; hostname racing remains open |
| [http/connection.ok](../../std/http/connection.ok) | 1 | GREEN: incompatible ALPN rejects before acquisition; 453/454 cover actual routed HTTP/HTTPS, identity/SNI and unclaimed result disposal |
| [http/endpoint.ok](../../std/http/endpoint.ok) | 1 | GREEN: explicit/public/empty policy; 451 covers route bounds, source snapshots, checks and resolution cancellation |
| [http/tls_session.ok](../../std/http/tls_session.ok) | 1 | GREEN: exact/absent ALPN, h2/case rejection; 450/452 actual TLS/CLI and owner lifecycle |
| [net/tcp_racing.ok](../../std/net/tcp_racing.ok) | 1 | GREEN: singleton snapshot/empty rejection; 455..458 cover families, bounds, relative starts/failure, cancellation and ready losers |
| [net/tls.ok](../../std/net/tls.ok) | 2 | GREEN: modern defaults/empty trust and fixed phase diagnostics; 444..449/native42..51 cover policy, verified transport, lifecycle and actual HTTPS |
| [net/udp.ok](../../std/net/udp.ok) | 5 | GREEN: packet observations, explicit truncation, independent options, effect-free bind requests and size failures; loopback acquisition/lifecycle verified externally; live packet I/O remains open |
| [net/ip_registry.ok](../../std/net/ip_registry.ok) | 2 | GREEN: specific IANA exceptions and reserved-space overrides; offline generation from pinned CSV |
| [numbers/bigint.ok](../../std/numbers/bigint.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [numbers/bigrational.ok](../../std/numbers/bigrational.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [os/account.ok](../../std/os/account.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [os/command.ok](../../std/os/command.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [os/command_capture.ok](../../std/os/command_capture.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [os/directory.ok](../../std/os/directory.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [os/environment.ok](../../std/os/environment.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [os/pipeline_capture.ok](../../std/os/pipeline_capture.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [os/platform.ok](../../std/os/platform.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [os/process.ok](../../std/os/process.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [os/storage.ok](../../std/os/storage.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [parallel/map.ok](../../std/parallel/map.ok) | 1 | GREEN: typed count diagnostic; 15 external groups cover process dispatch, ordering, bounds, transfer and cleanup |
| [random/distribution.ok](../../std/random/distribution.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [random/source.ok](../../std/random/source.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [regex/escape.ok](../../std/regex/escape.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [regex/match.ok](../../std/regex/match.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [regex/pattern.ok](../../std/regex/pattern.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [regex/range.ok](../../std/regex/range.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [regex/replace.ok](../../std/regex/replace.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [regex/search.ok](../../std/regex/search.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [regex/split.ok](../../std/regex/split.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/account.ok](../../std/runtime/account.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/base32.ok](../../std/runtime/base32.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/base64.ok](../../std/runtime/base64.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/base85.ok](../../std/runtime/base85.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/bigint.ok](../../std/runtime/bigint.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/binary.ok](../../std/runtime/binary.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/buffer.ok](../../std/runtime/buffer.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/decimal.ok](../../std/runtime/decimal.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/environment.ok](../../std/runtime/environment.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/fs.ok](../../std/runtime/fs.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/hex.ok](../../std/runtime/hex.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/html.ok](../../std/runtime/html.ok) | 2 | Встроенные сценарии наблюдений, ошибок и границ; комментарии контракта |
| [runtime/input_state.ok](../../std/runtime/input_state.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/io_decorators.ok](../../std/runtime/io_decorators.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/ini.ok](../../std/runtime/ini.ok) | 2 | Parse coordinates/order, sticky output/budgets; detailed native contract comments |
| [runtime/json.ok](../../std/runtime/json.ok) | 3 | Встроенные сценарии наблюдений, ошибок и границ; комментарии контракта |
| [runtime/layout.ok](../../std/runtime/layout.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/math_aggregate.ok](../../std/runtime/math_aggregate.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/net.ok](../../std/runtime/net.ok) | 1 | GREEN: real numeric provider parse/format round trip |
| [runtime/network.ok](../../std/runtime/network.ok) | 1 | GREEN: native capability closure is shared and idempotent; UDP provider loopback/flags/options/cleanup tested externally |
| [runtime/tls_transport.ok](../../std/runtime/tls_transport.ok) | 0 | Private consumed TLS ABI; native42..51/public445..449 exercise actual ownership/verification/I/O rather than fabricated handles |
| [runtime/ordering.ok](../../std/runtime/ordering.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/process.ok](../../std/runtime/process.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/quoted_printable.ok](../../std/runtime/quoted_printable.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/random.ok](../../std/runtime/random.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/random_distribution.ok](../../std/runtime/random_distribution.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/regex.ok](../../std/runtime/regex.ok) | 4 | Встроенные сценарии наблюдений, ошибок и границ; комментарии контракта |
| [runtime/single_byte.ok](../../std/runtime/single_byte.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/statistics.ok](../../std/runtime/statistics.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/stdio.ok](../../std/runtime/stdio.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/system.ok](../../std/runtime/system.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/toml.ok](../../std/runtime/toml.ok) | 2 | Встроенные tests opaque state, diagnostics и exact integer; комментарии ABI |
| [runtime/template.ok](../../std/runtime/template.ok) | 3 | Complete program/order, diagnostics, byte preflight; native ownership and bounds comments |
| [runtime/time.ok](../../std/runtime/time.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/time_format.ok](../../std/runtime/time_format.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/time_zone.ok](../../std/runtime/time_zone.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/unicode_character.ok](../../std/runtime/unicode_character.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/unicode_normalization.ok](../../std/runtime/unicode_normalization.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [runtime/utf.ok](../../std/runtime/utf.ok) | 2 | Встроенные сценарии наблюдений, ошибок и границ; комментарии контракта |
| [runtime/uuid.ok](../../std/runtime/uuid.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [size.ok](../../std/size.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [statistics.ok](../../std/statistics.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [strings/format.ok](../../std/strings/format.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [strings/join.ok](../../std/strings/join.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [strings/parse.ok](../../std/strings/parse.ok) | 10 | Есть встроенные тесты; проверить полноту контракта |
| [template/compile.ok](../../std/template/compile.ok) | 6 | Okkama flat parser, syntax, budgets, coordinates and escapes; parity suite 289 |
| [template/tree.ok](../../std/template/tree.ok) | 8 | Structured grammar, standalone lines, matching, depth/token bounds; consumer suite 290 |
| [template/html_library.ok](../../std/template/html_library.ok) | 3 | Empty/missing/limit and diagnostic contracts; consumer suite 296 covers typed snapshots and source bounds |
| [plist/write.ok](../../std/plist/write.ok) | 4 | Diagnostics, inline lengths, exact duplicate names and binary envelope budget; 33 consumer groups cover wire fixtures, borrowing, protocol and format limits |
| [plist/parse.ok](../../std/plist/parse.ok) | 3 | Empty/truncated input, configuration bounds and both wire formats; 72 consumer groups cover grammar, payloads, coordinates, binary corruption and cycles |
| [plist/value.ok](../../std/plist/value.ok) | 3 | Scalar kinds, ordered containers, missing/duplicate/limit errors; consumer suite 298 covers all payloads, nominal printing, source reads and lifetime |
| [locale/tag.ok](../../std/locale/tag.ok) | 3 | Case-normal coordinates, parsing, typed absence/duplicates and unsupported syntax; consumer suite 297 covers bounds, comparison and snapshot lifetime |
| [locale/match.ok](../../std/locale/match.ok) | 2 | Basic grammar, hyphen/case matching and descending prefixes; suite 406 adds size/subtag bounds, offsets and HTTP consumer selection |
| [template/html.ok](../../std/template/html.ok) | 3 | Text vs quoted context, restricted raw/event contexts, ASCII names and unfinished fragments; consumer suite 295 |
| [template/sections.ok](../../std/template/sections.ok) | 7 | Iteration ledger, zero/negative budgets and diagnostics; Eight-byte output meter boundaries; consumer suites 292/293/294 cover rendering, partials, streaming and sources |
| [template/model.ok](../../std/template/model.ok) | 2 | Exact bindings, fake nominal role, typed lookup/duplicates/budgets; comments and lifetime |
| [template/library.ok](../../std/template/library.ok) | 3 | GREEN: empty/limits/errors; 17 consumer groups cover nonempty snapshots, lookup, lifetime and failures |
| [template/json.ok](../../std/template/json.ok) | 1 | GREEN: diagnostics; 24 scalar and 11 context consumer groups cover current item, nominal traversal, limits, failures and lifetime |
| [template/path.ok](../../std/template/path.ok) | 2 | Nominal comparison, exact Unicode, syntax and budgets; detailed snapshot/lifetime comments |
| [template/render.ok](../../std/template/render.ok) | 2 | Repeat command, borrowed lifetime and empty output; detailed partial-effect/chunk contract |
| [text/layout.ok](../../std/text/layout.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [time/clock.ok](../../std/time/clock.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [http/conditional.ok](../../std/http/conditional.ok) | 3 | Validator strength/wildcard and exact/coalesced byte ranges; suites 399/400 cover grammar, bounds, budgets, nominal snapshots and independent normalization oracle |
| [http/negotiation.ok](../../std/http/negotiation.ok) | 6 | Exact quality/token identity, four field zero-before-wildcard policies and specificity; suites 401/402/404/405/406 cover qvalue grid, presence, budgets and ranking oracles |
| [http/media_type.ok](../../std/http/media_type.ok) | 1 | Canonical type/charset/parameter identity; suite 403 covers HTTP grammar, obs-text, source budgets and binary lookup cost |
| [http/http1_head.ok](../../std/http/http1_head.ok) | 1 | Extension status/duplicates and missing Host; suite 419 covers strict lines/fields/framing, credentials, bounds and canonical logical-vs-wire space cost |
| [http/http1_reading.ok](../../std/http/http1_reading.ok) | 1 | Delimiter boundary; suite 420 verifies fragmentation, remainder, source bounds, memoization and structured cancellation |
| [http/http1_framing.ok](../../std/http/http1_framing.ok) | 1 | Explicit zero-length fixed framing; suite 421 covers ambiguity, BigInt bounds, method/status precedence and eligibility |
| [http/http1_chunk.ok](../../std/http/http1_chunk.ok) | 1 | Zero chunk/empty trailers; suite 422 covers extension grammar, quoted pairs, numeric/wire limits and forbidden names |
| [http/http1_body.ok](../../std/http/http1_body.ok) | 1 | Explicit owner EOF; suite 423 covers payload/tail, chunk metadata, abort/cancel, limits and one-shot body role |
| [http/http1_writing.ok](../../std/http/http1_writing.ok) | 1 | Canonical extension status; suite 424 covers lazy/memoized writes, framing/wire budgets, failures and drained cancellation |
| [http/http1_body_writing.ok](../../std/http/http1_body_writing.ok) | 1 | Exact lowercase chunk size; suite 425 covers streaming, length contradictions, bounds, trailers, source/sink failure and cancellation |
| [http/http1_message_writing.ok](../../std/http/http1_message_writing.ok) | 1 | Empty request round-trip; suite 426 covers pre-head body validation, suppressed sources, memoization and started/unstarted cancellation |
| [http/http1_session.ok](../../std/http/http1_session.ok) | 1 async | GREEN: unstarted cancellation; 431/432 cover EOF leases, close/error/cancel, old-reader isolation, informational bounds and startup release; real TCP reuse example PASS |
| [http/http1_body_check.ok](../../std/http/http1_body_check.ok) | 1 | GREEN: shared pre-head contradiction validation; existing message/body writer and continue suites preserve source/length/limit rules |
| [http/http1_continue.ok](../../std/http/http1_continue.ok) | 1 | GREEN: ordinary request state; 433 covers deferred opening, early final, duplicate 100, 103, absolute timeout and parent cancellation; real TCP handshake PASS |
| [http/message.ok](../../std/http/message.ok) | 1 | Extension status, defaults and empty body; suite 418 proves no constructor body observations and target rules |
| [http/version.ok](../../std/http/version.ok) | 1 | Exact HTTP1 token and HTTP2 rejection; suite 418 covers unsupported/malformed tokens without source diagnostics |
| [http/target.ok](../../std/http/target.ok) | 1 | Double-slash origin path, query presence and method forms; suite 417 covers all forms, wrong coordinates, bounds and malicious nominal roles |
| [http/cookie.ok](../../std/http/cookie.ok) | 2 | Session absence, prefix/wire/redaction; suites 407/412 cover expiry precision/lifetime, IDNA, flags, grammar, prefixes and default diagnostics |
| [http/cookie_date.ok](../../std/http/cookie_date.ok) | 1 | Weekday ignored and invalid calendar; suite 413 covers token layouts, centuries, first field wins, octet/source limits and diagnostics |
| [http/cookie_policy.ok](../../std/http/cookie_policy.ok) | 1 | Default path and age precedence; suite 414 covers PSL, origin/context, duplicate attrs, quoted wire, bounds, prefix derivation and diagnostics |
| [http/cookie_jar.ok](../../std/http/cookie_jar.ok) | 1 | Empty effect/close semantics; suite 415 covers snapshots, replacement, expiry, SameSite/partitions, eviction and clock reentry |
| [time/date.ok](../../std/time/date.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [time/day.ok](../../std/time/day.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [time/duration.ok](../../std/time/duration.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [time/format.ok](../../std/time/format.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [time/iso.ok](../../std/time/iso.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [time/local_datetime.ok](../../std/time/local_datetime.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [time/rfc3339.ok](../../std/time/rfc3339.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [time/zone.ok](../../std/time/zone.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [unicode/character.ok](../../std/unicode/character.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [unicode/normalization.ok](../../std/unicode/normalization.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [uuid/generation.ok](../../std/uuid/generation.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [uuid/value.ok](../../std/uuid/value.ok) | 0 | TODO: встроенные тесты и ревью комментариев |
| [toml/parse.ok](../../std/toml/parse.ok) | 6 | Встроенные tests всех категорий, ошибок, бюджетов и повторного наблюдения; контракт в комментариях |
| [toml/value.ok](../../std/toml/value.ok) | 9 | Есть встроенные тесты и комментарии контракта |

HTTP client: client_policy, client_request and client inline tests PASS. Actual transport/pool/deadline/HTTPS fixtures are interpreter/462..464; inline does not substitute for those tests.

http/redirect: policy bounds/default downgrade inline PASS; actual follow/ownership/policy tests remain focused 465..467.

http/cookie_client: first-party context and empty cookie serialization inline PASS. Actual jar/redirect/body-error contracts are covered in 468..470.

SecretBytes export/shared closure and http/auth canonical scope inline PASS. Native provider full gate: 672 files / 12185 assertions PASS. ConstantTimeEqual/Digest remain open.

http/http2_settings: explicit zero capacities and push-disabled client settings inline PASS; internal frame/reader contracts are focused 474..476.

HTTP2 peer SETTINGS: inline zero-concurrency/unlimited distinction PASS; 477..479 focused and real TCP example PASS.

HPACK primitives: inline/example PASS; native-change full gate 680 files / 12,266 assertions PASS.

HPACK table/block slice: inline sensitivity roundtrip and executable context example PASS; root compatibility 35 assertions PASS.

HTTP2 header-block/field modules: inline padding separation and pseudo-status separation PASS; actual TCP 485 PASS; focused 480..485 forty-three assertions PASS.

HTTP2 flow accounting: inline dual-window charge and executable padded DATA example PASS; compatibility 21 assertions PASS. Live scheduling remains open.

HTTP2 control values: inline GOAWAY stop and executable reset/cutoff example PASS; frame/control compatibility twelve assertions PASS.

HTTP wire bytes: inline independent snapshot after private buffer close PASS; 488 exact 16 KiB payload vectors and affected H2/HPACK/TCP consumers verified.

HTTP2 stream state: inline sibling independence and executable lifecycle/IDs example PASS; controls/life compatibility ten assertions PASS.


runtime/collections: bounded string-map snapshot empty/exact-boundary inline PASS; focused3files/37checks PASS. Required frozen native-change full gate779files/13,120checks PASS; see evidence.


http/routing: encoded-slash/segment-count inline PASS; focused3files/10checks including actual H1/H2 HEAD PASS. Dynamic map provider full gate is separate. [Contract](http-routing.md).


http/form: [bounded streaming multipart, URL form and explicit spool](http-form.md) verified by focused, inline and executable example gates. No interpreter/provider or syntax change; no full regression repeated.


http/sse: [owned bounded event streams](http-sse.md), focused4files/12checks and inline/example PASS. Std-only; PERF-32 records remaining malformed UTF8 dispatch cost.


http/robots: [bounded immutable rules and explicit owned fetch](http-robots.md), focused/inline/example PASS; PERF-33 records dispatch cost. No runtime or syntax change.


http/xmlrpc: [bounded capability codecs and HTTP adapters](http-xmlrpc.md), five focused files/16checks including real H1, inline/example PASS. No runtime or syntax change; PERF-34 records residual XML scanning costs.
