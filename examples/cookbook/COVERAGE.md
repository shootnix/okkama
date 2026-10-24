# Карта использования stdlib

506 из 506 модулей связаны с 170 точками запуска.

Карта описывает прямые импорты и транзитивные зависимости реальных исходников. Цепочка включает вспомогательные модули приложения. Для internal и runtime это путь через использующий их API. Результаты выполнения сценариев хранятся отдельно в [checks.json](checks.json).

| Модуль | Приложение или лаборатория | Цепочка импортов |
|---|---|---|
| [`archive/entry`](../../std/archive/entry.ok) | [labs/example-archive-tar-reading/main.ok](labs/example-archive-tar-reading/main.ok) | `labs/example-archive-tar-reading/main` → `std/archive/entry` |
| [`archive/limits`](../../std/archive/limits.ok) | [labs/example-archive-limits/main.ok](labs/example-archive-limits/main.ok) | `labs/example-archive-limits/main` → `std/archive/limits` |
| [`archive/metadata`](../../std/archive/metadata.ok) | [labs/example-archive-metadata/main.ok](labs/example-archive-metadata/main.ok) | `labs/example-archive-metadata/main` → `std/archive/metadata` |
| [`archive/path`](../../std/archive/path.ok) | [labs/example-archive-path/main.ok](labs/example-archive-path/main.ok) | `labs/example-archive-path/main` → `std/archive/path` |
| [`archive/stream`](../../std/archive/stream.ok) | [labs/example-archive-tar-stream/main.ok](labs/example-archive-tar-stream/main.ok) | `labs/example-archive-tar-stream/main` → `std/archive/stream` |
| [`archive/tar`](../../std/archive/tar.ok) | [labs/example-archive-tar-reading/main.ok](labs/example-archive-tar-reading/main.ok) | `labs/example-archive-tar-reading/main` → `std/archive/tar` |
| [`archive/zip`](../../std/archive/zip.ok) | [labs/example-archive-zip-compression/main.ok](labs/example-archive-zip-compression/main.ok) | `labs/example-archive-zip-compression/main` → `std/archive/zip` |
| [`async/atomic`](../../std/async/atomic.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/async/atomic` |
| [`async/barrier`](../../std/async/barrier.ok) | [labs/recipe-async-phases/main.ok](labs/recipe-async-phases/main.ok) | `labs/recipe-async-phases/main` → `std/async/barrier` |
| [`async/cancellation`](../../std/async/cancellation.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/request_context` → `std/async/cancellation` |
| [`async/channel`](../../std/async/channel.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/async/channel` |
| [`async/collect`](../../std/async/collect.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/net/tcp_racing` → `std/async/collect` |
| [`async/event`](../../std/async/event.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/async/event` |
| [`async/lock`](../../std/async/lock.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/tls` → `std/async/lock` |
| [`async/scope`](../../std/async/scope.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/async/scope` |
| [`async/semaphore`](../../std/async/semaphore.ok) | [labs/recipe-async-phases/main.ok](labs/recipe-async-phases/main.ok) | `labs/recipe-async-phases/main` → `std/async/semaphore` |
| [`async/timeout`](../../std/async/timeout.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/async/timeout` |
| [`calendar`](../../std/calendar.ok) | [labs/recipe-calendar/main.ok](labs/recipe-calendar/main.ok) | `labs/recipe-calendar/main` → `std/calendar` |
| [`checksum/adler32`](../../std/checksum/adler32.ok) | [labs/example-checksum-adler32/main.ok](labs/example-checksum-adler32/main.ok) | `labs/example-checksum-adler32/main` → `std/checksum/adler32` |
| [`checksum/crc`](../../std/checksum/crc.ok) | [labs/example-checksum-crc/main.ok](labs/example-checksum-crc/main.ok) | `labs/example-checksum-crc/main` → `std/checksum/crc` |
| [`checksum/state`](../../std/checksum/state.ok) | [labs/example-checksum-adler32/main.ok](labs/example-checksum-adler32/main.ok) | `labs/example-checksum-adler32/main` → `std/checksum/adler32` → `std/checksum/state` |
| [`cli/arguments`](../../std/cli/arguments.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `support/arguments` → `std/cli/arguments` |
| [`cli/help`](../../std/cli/help.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/cli/help` |
| [`cli/parse`](../../std/cli/parse.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/cli/parse` |
| [`cli/spec`](../../std/cli/spec.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/cli/spec` |
| [`collections/combinatorics`](../../std/collections/combinatorics.ok) | [labs/recipe-views/main.ok](labs/recipe-views/main.ok) | `labs/recipe-views/main` → `std/collections/combinatorics` |
| [`collections/combining`](../../std/collections/combining.ok) | [apps/payroll.ok](apps/payroll.ok) | `apps/payroll` → `std/collections/set` → `std/collections/views` → `std/collections/combining` |
| [`collections/deque`](../../std/collections/deque.ok) | [labs/recipe-lists/main.ok](labs/recipe-lists/main.ok) | `labs/recipe-lists/main` → `std/collections/deque` |
| [`collections/enumeration`](../../std/collections/enumeration.ok) | [apps/payroll.ok](apps/payroll.ok) | `apps/payroll` → `std/collections/set` → `std/collections/views` → `std/collections/enumeration` |
| [`collections/frequencies`](../../std/collections/frequencies.ok) | [labs/recipe-lists/main.ok](labs/recipe-lists/main.ok) | `labs/recipe-lists/main` → `std/collections/frequencies` |
| [`collections/heap`](../../std/collections/heap.ok) | [labs/recipe-lists/main.ok](labs/recipe-lists/main.ok) | `labs/recipe-lists/main` → `std/collections/heap` |
| [`collections/iteration`](../../std/collections/iteration.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/ip` → `std/collections/iteration` |
| [`collections/list`](../../std/collections/list.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/csv` → `std/collections/list` |
| [`collections/ordering`](../../std/collections/ordering.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/dns` → `std/collections/ordering` |
| [`collections/repetition`](../../std/collections/repetition.ok) | [labs/recipe-views/main.ok](labs/recipe-views/main.ok) | `labs/recipe-views/main` → `std/collections/repetition` |
| [`collections/sequence`](../../std/collections/sequence.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/json/write` → `std/collections/sequence` |
| [`collections/set`](../../std/collections/set.ok) | [apps/payroll.ok](apps/payroll.ok) | `apps/payroll` → `std/collections/set` |
| [`collections/views`](../../std/collections/views.ok) | [apps/payroll.ok](apps/payroll.ok) | `apps/payroll` → `std/collections/set` → `std/collections/views` |
| [`compare`](../../std/compare.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/compare` |
| [`compression/read`](../../std/compression/read.ok) | [labs/example-compression-read/main.ok](labs/example-compression-read/main.ok) | `labs/example-compression-read/main` → `std/compression/read` |
| [`compression/state`](../../std/compression/state.ok) | [labs/example-compression-read/main.ok](labs/example-compression-read/main.ok) | `labs/example-compression-read/main` → `std/compression/state` |
| [`config/document`](../../std/config/document.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/config/document` |
| [`config/environment`](../../std/config/environment.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/config/environment` |
| [`config/ini`](../../std/config/ini.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/config/document` → `std/config/ini` |
| [`config/key`](../../std/config/key.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/config/key` |
| [`config/source`](../../std/config/source.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/config/source` |
| [`config/value`](../../std/config/value.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/config/value` |
| [`crypto/key`](../../std/crypto/key.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/tls` → `std/crypto/x509/certificate` → `std/crypto/key` |
| [`crypto/secret`](../../std/crypto/secret.ok) | [labs/example-http-digest-https/main.ok](labs/example-http-digest-https/main.ok) | `labs/example-http-digest-https/main` → `std/crypto/secret` |
| [`crypto/x509/certificate`](../../std/crypto/x509/certificate.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/tls` → `std/crypto/x509/certificate` |
| [`crypto/x509/name`](../../std/crypto/x509/name.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/tls` → `std/crypto/x509/certificate` → `std/crypto/x509/name` |
| [`crypto/x509/trust`](../../std/crypto/x509/trust.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/crypto/x509/trust` |
| [`crypto/x509/value`](../../std/crypto/x509/value.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/crypto/x509/trust` → `std/crypto/x509/value` |
| [`csv`](../../std/csv.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/csv` |
| [`csv/input`](../../std/csv/input.ok) | [labs/example-csv-input/main.ok](labs/example-csv-input/main.ok) | `labs/example-csv-input/main` → `std/csv/input` |
| [`csv/stream`](../../std/csv/stream.ok) | [labs/recipe-csv-stream/main.ok](labs/recipe-csv-stream/main.ok) | `labs/recipe-csv-stream/main` → `std/csv/stream` |
| [`db/acquisition`](../../std/db/acquisition.ok) | [apps/taxi.ok](apps/taxi.ok) | `apps/taxi` → `std/db/sqlite` → `std/db/acquisition` |
| [`db/connection`](../../std/db/connection.ok) | [apps/taxi.ok](apps/taxi.ok) | `apps/taxi` → `std/db/connection` |
| [`db/error`](../../std/db/error.ok) | [apps/taxi.ok](apps/taxi.ok) | `apps/taxi` → `std/db/sqlite` → `std/db/error` |
| [`db/pool`](../../std/db/pool.ok) | [labs/example-database-stack/main.ok](labs/example-database-stack/main.ok) | `labs/example-database-stack/main` → `std/db/pool` |
| [`db/prepared`](../../std/db/prepared.ok) | [apps/taxi.ok](apps/taxi.ok) | `apps/taxi` → `std/db/sqlite` → `std/db/prepared` |
| [`db/read`](../../std/db/read.ok) | [apps/bookstore.ok](apps/bookstore.ok) | `apps/bookstore` → `web/bookstore/model` → `std/db/read` |
| [`db/row`](../../std/db/row.ok) | [apps/bookstore.ok](apps/bookstore.ok) | `apps/bookstore` → `web/bookstore/model` → `std/db/row` |
| [`db/sqlite`](../../std/db/sqlite.ok) | [apps/taxi.ok](apps/taxi.ok) | `apps/taxi` → `std/db/sqlite` |
| [`db/sqlite/backup`](../../std/db/sqlite/backup.ok) | [apps/taxi.ok](apps/taxi.ok) | `apps/taxi` → `std/db/sqlite` → `std/db/sqlite/backup` |
| [`db/sqlite/blob`](../../std/db/sqlite/blob.ok) | [apps/taxi.ok](apps/taxi.ok) | `apps/taxi` → `std/db/sqlite` → `std/db/sqlite/blob` |
| [`db/sqlite/script`](../../std/db/sqlite/script.ok) | [apps/taxi.ok](apps/taxi.ok) | `apps/taxi` → `std/db/sqlite/script` |
| [`db/statement`](../../std/db/statement.ok) | [apps/taxi.ok](apps/taxi.ok) | `apps/taxi` → `std/db/statement` |
| [`db/transaction`](../../std/db/transaction.ok) | [apps/bookstore.ok](apps/bookstore.ok) | `apps/bookstore` → `web/bookstore/model` → `std/db/transaction` |
| [`db/value`](../../std/db/value.ok) | [apps/taxi.ok](apps/taxi.ok) | `apps/taxi` → `std/db/value` |
| [`diff`](../../std/diff.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/check` → `std/diff` |
| [`digest/blocks`](../../std/digest/blocks.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/websocket_handshake` → `std/digest/legacy` → `std/digest/blocks` |
| [`digest/legacy`](../../std/digest/legacy.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/websocket_handshake` → `std/digest/legacy` |
| [`digest/state`](../../std/digest/state.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/websocket_handshake` → `std/digest/legacy` → `std/digest/state` |
| [`encoding/base32`](../../std/encoding/base32.ok) | [labs/recipe-encodings/main.ok](labs/recipe-encodings/main.ok) | `labs/recipe-encodings/main` → `std/encoding/base32` |
| [`encoding/base64`](../../std/encoding/base64.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mime/word` → `std/encoding/base64` |
| [`encoding/base85`](../../std/encoding/base85.ok) | [labs/recipe-encodings/main.ok](labs/recipe-encodings/main.ok) | `labs/recipe-encodings/main` → `std/encoding/base85` |
| [`encoding/binary`](../../std/encoding/binary.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/url/value` → `std/encoding/binary` |
| [`encoding/hex`](../../std/encoding/hex.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mime/part` → `std/encoding/hex` |
| [`encoding/pem`](../../std/encoding/pem.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/crypto/x509/trust` → `std/encoding/pem` |
| [`encoding/quoted_printable`](../../std/encoding/quoted_printable.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mime/part` → `std/mime/transfer` → `std/encoding/quoted_printable` |
| [`encoding/single_byte`](../../std/encoding/single_byte.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/header` → `std/encoding/single_byte` |
| [`encoding/utf`](../../std/encoding/utf.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/encoding/utf` |
| [`error`](../../std/error.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/error` |
| [`fs/directory`](../../std/fs/directory.ok) | [labs/example-database-stack/main.ok](labs/example-database-stack/main.ok) | `labs/example-database-stack/main` → `std/fs/directory` |
| [`fs/file`](../../std/fs/file.ok) | [apps/docs.ok](apps/docs.ok) | `apps/docs` → `std/fs/file` |
| [`fs/glob`](../../std/fs/glob.ok) | [labs/recipe-file-discovery/main.ok](labs/recipe-file-discovery/main.ok) | `labs/recipe-file-discovery/main` → `std/fs/glob` |
| [`fs/link`](../../std/fs/link.ok) | [labs/example-mail-flags/main.ok](labs/example-mail-flags/main.ok) | `labs/example-mail-flags/main` → `std/mail/mailbox` → `std/fs/link` |
| [`fs/lock`](../../std/fs/lock.ok) | [labs/example-mail-flags/main.ok](labs/example-mail-flags/main.ok) | `labs/example-mail-flags/main` → `std/mail/mailbox` → `std/fs/lock` |
| [`fs/path`](../../std/fs/path.ok) | [apps/docs.ok](apps/docs.ok) | `apps/docs` → `std/fs/path` |
| [`fs/root`](../../std/fs/root.ok) | [labs/example-http-static/main.ok](labs/example-http-static/main.ok) | `labs/example-http-static/main` → `std/fs/root` |
| [`fs/stat`](../../std/fs/stat.ok) | [apps/docs.ok](apps/docs.ok) | `apps/docs` → `std/fs/file` → `std/fs/stat` |
| [`fs/temp`](../../std/fs/temp.ok) | [labs/example-database-stack/main.ok](labs/example-database-stack/main.ok) | `labs/example-database-stack/main` → `std/fs/temp` |
| [`fs/watch`](../../std/fs/watch.ok) | [labs/recipe-file-discovery/main.ok](labs/recipe-file-discovery/main.ok) | `labs/recipe-file-discovery/main` → `std/fs/watch` |
| [`html/entities`](../../std/html/entities.ok) | [apps/docs.ok](apps/docs.ok) | `apps/docs` → `std/html/entities` |
| [`html/events`](../../std/html/events.ok) | [labs/recipe-html-events/main.ok](labs/recipe-html-events/main.ok) | `labs/recipe-html-events/main` → `std/html/events` |
| [`http/auth`](../../std/http/auth.ok) | [labs/example-http-digest-https/main.ok](labs/example-http-digest-https/main.ok) | `labs/example-http-digest-https/main` → `std/http/auth` |
| [`http/auth_challenge`](../../std/http/auth_challenge.ok) | [labs/example-http-digest-https/main.ok](labs/example-http-digest-https/main.ok) | `labs/example-http-digest-https/main` → `std/http/digest` → `std/http/auth_challenge` |
| [`http/body`](../../std/http/body.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/body` |
| [`http/cache`](../../std/http/cache.ok) | [labs/example-http-cache/main.ok](labs/example-http-cache/main.ok) | `labs/example-http-cache/main` → `std/http/cache` |
| [`http/cache_admission`](../../std/http/cache_admission.ok) | [labs/example-http-cache/main.ok](labs/example-http-cache/main.ok) | `labs/example-http-cache/main` → `std/http/cache` → `std/http/cache_admission` |
| [`http/cache_control`](../../std/http/cache_control.ok) | [labs/example-http-cache/main.ok](labs/example-http-cache/main.ok) | `labs/example-http-cache/main` → `std/http/cache` → `std/http/cache_control` |
| [`http/cache_freshness`](../../std/http/cache_freshness.ok) | [labs/example-http-cache/main.ok](labs/example-http-cache/main.ok) | `labs/example-http-cache/main` → `std/http/cache` → `std/http/cache_freshness` |
| [`http/cache_memory`](../../std/http/cache_memory.ok) | [labs/example-http-cache/main.ok](labs/example-http-cache/main.ok) | `labs/example-http-cache/main` → `std/http/cache_memory` |
| [`http/cache_validation`](../../std/http/cache_validation.ok) | [labs/example-http-cache/main.ok](labs/example-http-cache/main.ok) | `labs/example-http-cache/main` → `std/http/cache` → `std/http/cache_validation` |
| [`http/client`](../../std/http/client.ok) | [labs/example-http-cache/main.ok](labs/example-http-cache/main.ok) | `labs/example-http-cache/main` → `std/http/client` |
| [`http/client_policy`](../../std/http/client_policy.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server_policy` → `std/http/client_policy` |
| [`http/client_protocols`](../../std/http/client_protocols.ok) | [labs/example-http-cache/main.ok](labs/example-http-cache/main.ok) | `labs/example-http-cache/main` → `std/http/client` → `std/http/client_protocols` |
| [`http/client_request`](../../std/http/client_request.ok) | [labs/example-http-cache/main.ok](labs/example-http-cache/main.ok) | `labs/example-http-cache/main` → `std/http/client` → `std/http/client_request` |
| [`http/conditional`](../../std/http/conditional.ok) | [labs/example-http-byte-ranges/main.ok](labs/example-http-byte-ranges/main.ok) | `labs/example-http-byte-ranges/main` → `std/http/conditional` |
| [`http/connection`](../../std/http/connection.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/connection` |
| [`http/content_coding`](../../std/http/content_coding.ok) | [labs/example-http-content-coding/main.ok](labs/example-http-content-coding/main.ok) | `labs/example-http-content-coding/main` → `std/http/content_coding` |
| [`http/conversation`](../../std/http/conversation.ok) | [labs/example-http-cache/main.ok](labs/example-http-cache/main.ok) | `labs/example-http-cache/main` → `std/http/client` → `std/http/conversation` |
| [`http/cookie`](../../std/http/cookie.ok) | [labs/example-http-cookie-values/main.ok](labs/example-http-cookie-values/main.ok) | `labs/example-http-cookie-values/main` → `std/http/cookie` |
| [`http/cookie_client`](../../std/http/cookie_client.ok) | [labs/example-http-cookie-client/main.ok](labs/example-http-cookie-client/main.ok) | `labs/example-http-cookie-client/main` → `std/http/cookie_client` |
| [`http/cookie_date`](../../std/http/cookie_date.ok) | [labs/example-http-cookie-date/main.ok](labs/example-http-cookie-date/main.ok) | `labs/example-http-cookie-date/main` → `std/http/cookie_date` |
| [`http/cookie_jar`](../../std/http/cookie_jar.ok) | [labs/example-http-cookie-client/main.ok](labs/example-http-cookie-client/main.ok) | `labs/example-http-cookie-client/main` → `std/http/cookie_jar` |
| [`http/cookie_policy`](../../std/http/cookie_policy.ok) | [labs/example-http-cookie-jar/main.ok](labs/example-http-cookie-jar/main.ok) | `labs/example-http-cookie-jar/main` → `std/http/cookie_policy` |
| [`http/date`](../../std/http/date.ok) | [labs/example-http-cookie-date/main.ok](labs/example-http-cookie-date/main.ok) | `labs/example-http-cookie-date/main` → `std/http/date` |
| [`http/digest`](../../std/http/digest.ok) | [labs/example-http-digest-https/main.ok](labs/example-http-digest-https/main.ok) | `labs/example-http-digest-https/main` → `std/http/digest` |
| [`http/endpoint`](../../std/http/endpoint.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/endpoint` |
| [`http/environment_proxy`](../../std/http/environment_proxy.ok) | [labs/recipe-proxy-environment/main.ok](labs/recipe-proxy-environment/main.ok) | `labs/recipe-proxy-environment/main` → `std/http/environment_proxy` |
| [`http/exchange`](../../std/http/exchange.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/request_context` → `std/http/exchange` |
| [`http/form`](../../std/http/form.ok) | [labs/example-http-form/main.ok](labs/example-http-form/main.ok) | `labs/example-http-form/main` → `std/http/form` |
| [`http/handler`](../../std/http/handler.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/handler` |
| [`http/header`](../../std/http/header.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/header` |
| [`http/http1_body`](../../std/http/http1_body.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http1_body` |
| [`http/http1_body_check`](../../std/http/http1_body_check.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/http1_message_writing` → `std/http/http1_body_check` |
| [`http/http1_body_writing`](../../std/http/http1_body_writing.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/http1_message_writing` → `std/http/http1_body_writing` |
| [`http/http1_chunk`](../../std/http/http1_chunk.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http1_body` → `std/http/http1_chunk` |
| [`http/http1_continue`](../../std/http/http1_continue.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/connection` → `std/http/http1_session` → `std/http/http1_continue` |
| [`http/http1_framing`](../../std/http/http1_framing.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http1_framing` |
| [`http/http1_head`](../../std/http/http1_head.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/http1_head` |
| [`http/http1_message_writing`](../../std/http/http1_message_writing.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/http1_message_writing` |
| [`http/http1_reading`](../../std/http/http1_reading.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/http1_reading` |
| [`http/http1_session`](../../std/http/http1_session.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/connection` → `std/http/http1_session` |
| [`http/http1_writing`](../../std/http/http1_writing.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http1_writing` |
| [`http/http2_connection`](../../std/http/http2_connection.ok) | [labs/example-http2-connecting/main.ok](labs/example-http2-connecting/main.ok) | `labs/example-http2-connecting/main` → `std/http/http2_connection` |
| [`http/http2_server`](../../std/http/http2_server.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` |
| [`http/http2_session`](../../std/http/http2_session.ok) | [labs/example-http2-session/main.ok](labs/example-http2-session/main.ok) | `labs/example-http2-session/main` → `std/http/http2_session` |
| [`http/http2_settings`](../../std/http/http2_settings.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/http2_settings` |
| [`http/http2_tls`](../../std/http/http2_tls.ok) | [labs/example-http2-connecting/main.ok](labs/example-http2-connecting/main.ok) | `labs/example-http2-connecting/main` → `std/http/http2_connection` → `std/http/http2_tls` |
| [`http/internal/hpack`](../../std/http/internal/hpack.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/hpack` |
| [`http/internal/hpack_primitives`](../../std/http/internal/hpack_primitives.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/hpack` → `std/http/internal/hpack_primitives` |
| [`http/internal/hpack_static`](../../std/http/internal/hpack_static.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/hpack_tables` → `std/http/internal/hpack_static` |
| [`http/internal/hpack_tables`](../../std/http/internal/hpack_tables.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/hpack_tables` |
| [`http/internal/http2_body`](../../std/http/internal/http2_body.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_body` |
| [`http/internal/http2_controls`](../../std/http/internal/http2_controls.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_controls` |
| [`http/internal/http2_fields`](../../std/http/internal/http2_fields.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_fields` |
| [`http/internal/http2_flow`](../../std/http/internal/http2_flow.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_flow` |
| [`http/internal/http2_frame`](../../std/http/internal/http2_frame.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_frame` |
| [`http/internal/http2_headers`](../../std/http/internal/http2_headers.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_headers` |
| [`http/internal/http2_input`](../../std/http/internal/http2_input.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_input` |
| [`http/internal/http2_life`](../../std/http/internal/http2_life.ok) | [labs/example-http2-session/main.ok](labs/example-http2-session/main.ok) | `labs/example-http2-session/main` → `std/http/http2_session` → `std/http/internal/http2_life` |
| [`http/internal/http2_output`](../../std/http/internal/http2_output.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_output` |
| [`http/internal/http2_preface`](../../std/http/internal/http2_preface.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_preface` |
| [`http/internal/http2_reading`](../../std/http/internal/http2_reading.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_preface` → `std/http/internal/http2_reading` |
| [`http/internal/http2_receiving`](../../std/http/internal/http2_receiving.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_receiving` |
| [`http/internal/http2_sending`](../../std/http/internal/http2_sending.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_sending` |
| [`http/internal/http2_settings`](../../std/http/internal/http2_settings.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_settings` |
| [`http/internal/trailers`](../../std/http/internal/trailers.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/http2_body` → `std/http/internal/trailers` |
| [`http/internal/websocket_frame`](../../std/http/internal/websocket_frame.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/websocket_session` → `std/http/internal/websocket_frame` |
| [`http/internal/websocket_messages`](../../std/http/internal/websocket_messages.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/websocket_session` → `std/http/internal/websocket_messages` |
| [`http/internal/websocket_output`](../../std/http/internal/websocket_output.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/internal/websocket_output` |
| [`http/internal/websocket_reading`](../../std/http/internal/websocket_reading.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/websocket_session` → `std/http/internal/websocket_reading` |
| [`http/internal/websocket_upgrade`](../../std/http/internal/websocket_upgrade.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/internal/websocket_upgrade` |
| [`http/internal/wire_bytes`](../../std/http/internal/wire_bytes.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/websocket_session` → `std/http/internal/wire_bytes` |
| [`http/media_type`](../../std/http/media_type.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/web/request` → `std/http/media_type` |
| [`http/message`](../../std/http/message.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/message` |
| [`http/method`](../../std/http/method.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/web/routing` → `std/http/method` |
| [`http/negotiation`](../../std/http/negotiation.ok) | [labs/example-http-accepted-charsets/main.ok](labs/example-http-accepted-charsets/main.ok) | `labs/example-http-accepted-charsets/main` → `std/http/negotiation` |
| [`http/proxy`](../../std/http/proxy.ok) | [labs/example-http-proxy/main.ok](labs/example-http-proxy/main.ok) | `labs/example-http-proxy/main` → `std/http/proxy` |
| [`http/proxy_connection`](../../std/http/proxy_connection.ok) | [labs/example-http-proxy/main.ok](labs/example-http-proxy/main.ok) | `labs/example-http-proxy/main` → `std/http/proxy` → `std/http/proxy_connection` |
| [`http/proxy_policy`](../../std/http/proxy_policy.ok) | [labs/example-http-proxy/main.ok](labs/example-http-proxy/main.ok) | `labs/example-http-proxy/main` → `std/http/proxy_policy` |
| [`http/proxy_request`](../../std/http/proxy_request.ok) | [labs/example-http-cache/main.ok](labs/example-http-cache/main.ok) | `labs/example-http-cache/main` → `std/http/client` → `std/http/conversation` → `std/http/proxy_request` |
| [`http/proxy_tunnel`](../../std/http/proxy_tunnel.ok) | [labs/example-http-proxy/main.ok](labs/example-http-proxy/main.ok) | `labs/example-http-proxy/main` → `std/http/proxy` → `std/http/proxy_connection` → `std/http/proxy_tunnel` |
| [`http/redirect`](../../std/http/redirect.ok) | [labs/example-http-cookie-client/main.ok](labs/example-http-cookie-client/main.ok) | `labs/example-http-cookie-client/main` → `std/http/redirect` |
| [`http/request_context`](../../std/http/request_context.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/request_context` |
| [`http/retry`](../../std/http/retry.ok) | [labs/example-http-retry/main.ok](labs/example-http-retry/main.ok) | `labs/example-http-retry/main` → `std/http/retry` |
| [`http/robots`](../../std/http/robots.ok) | [labs/example-http-robots/main.ok](labs/example-http-robots/main.ok) | `labs/example-http-robots/main` → `std/http/robots` |
| [`http/routing`](../../std/http/routing.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/web/routing` → `std/http/routing` |
| [`http/selected_connection`](../../std/http/selected_connection.ok) | [labs/example-http-selected-connecting/main.ok](labs/example-http-selected-connecting/main.ok) | `labs/example-http-selected-connecting/main` → `std/http/selected_connection` |
| [`http/server`](../../std/http/server.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` |
| [`http/server_policy`](../../std/http/server_policy.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server_policy` |
| [`http/sse`](../../std/http/sse.ok) | [labs/example-http-sse/main.ok](labs/example-http-sse/main.ok) | `labs/example-http-sse/main` → `std/http/sse` |
| [`http/static`](../../std/http/static.ok) | [labs/example-http-static/main.ok](labs/example-http-static/main.ok) | `labs/example-http-static/main` → `std/http/static` |
| [`http/status`](../../std/http/status.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/web/exception` → `std/http/status` |
| [`http/target`](../../std/http/target.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/web/request` → `std/http/target` |
| [`http/tls_session`](../../std/http/tls_session.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/connection` → `std/http/tls_session` |
| [`http/transport_observation`](../../std/http/transport_observation.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/connection` → `std/http/transport_observation` |
| [`http/version`](../../std/http/version.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/message` → `std/http/version` |
| [`http/websocket`](../../std/http/websocket.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket` |
| [`http/websocket_client`](../../std/http/websocket_client.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` |
| [`http/websocket_handshake`](../../std/http/websocket_handshake.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/websocket_handshake` |
| [`http/websocket_origin`](../../std/http/websocket_origin.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/websocket_origin` |
| [`http/websocket_server`](../../std/http/websocket_server.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/websocket_server` |
| [`http/websocket_session`](../../std/http/websocket_session.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/websocket_session` |
| [`http/xmlrpc`](../../std/http/xmlrpc.ok) | [labs/example-http-xmlrpc/main.ok](labs/example-http-xmlrpc/main.ok) | `labs/example-http-xmlrpc/main` → `std/http/xmlrpc` |
| [`i18n/catalog`](../../std/i18n/catalog.ok) | [labs/example-locale-i18n/main.ok](labs/example-locale-i18n/main.ok) | `labs/example-locale-i18n/main` → `std/i18n/catalog` |
| [`i18n/gettext`](../../std/i18n/gettext.ok) | [labs/example-gettext-po/main.ok](labs/example-gettext-po/main.ok) | `labs/example-gettext-po/main` → `std/i18n/gettext` |
| [`i18n/operands`](../../std/i18n/operands.ok) | [labs/example-locale-i18n/main.ok](labs/example-locale-i18n/main.ok) | `labs/example-locale-i18n/main` → `std/i18n/plural` → `std/i18n/operands` |
| [`i18n/plural`](../../std/i18n/plural.ok) | [labs/example-locale-i18n/main.ok](labs/example-locale-i18n/main.ok) | `labs/example-locale-i18n/main` → `std/i18n/plural` |
| [`io/buffer`](../../std/io/buffer.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/json/write` → `std/io/buffer` |
| [`io/decorators`](../../std/io/decorators.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/url/value` → `std/io/decorators` |
| [`io/stdin`](../../std/io/stdin.ok) | [apps/docs.ok](apps/docs.ok) | `apps/docs` → `std/fs/file` → `std/io/stdin` |
| [`io/stdout`](../../std/io/stdout.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/io/stdout` |
| [`io/stream`](../../std/io/stream.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/json/write` → `std/io/stream` |
| [`io/text`](../../std/io/text.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/json/write` → `std/io/text` |
| [`json/parse`](../../std/json/parse.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/json/parse` |
| [`json/read`](../../std/json/read.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/json/read` |
| [`json/value`](../../std/json/value.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/json/value` |
| [`json/write`](../../std/json/write.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/json/write` |
| [`locale/collation`](../../std/locale/collation.ok) | [labs/example-localized-collation/main.ok](labs/example-localized-collation/main.ok) | `labs/example-localized-collation/main` → `std/locale/collation` |
| [`locale/currency`](../../std/locale/currency.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` |
| [`locale/data`](../../std/locale/data.ok) | [labs/example-locale-i18n/main.ok](labs/example-locale-i18n/main.ok) | `labs/example-locale-i18n/main` → `std/locale/data` |
| [`locale/date_time`](../../std/locale/date_time.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` |
| [`locale/internal/currencies`](../../std/locale/internal/currencies.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` |
| [`locale/internal/currency_names_0`](../../std/locale/internal/currency_names_0.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_0` |
| [`locale/internal/currency_names_1`](../../std/locale/internal/currency_names_1.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_1` |
| [`locale/internal/currency_names_10`](../../std/locale/internal/currency_names_10.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_10` |
| [`locale/internal/currency_names_11`](../../std/locale/internal/currency_names_11.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_11` |
| [`locale/internal/currency_names_12`](../../std/locale/internal/currency_names_12.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_12` |
| [`locale/internal/currency_names_13`](../../std/locale/internal/currency_names_13.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_13` |
| [`locale/internal/currency_names_14`](../../std/locale/internal/currency_names_14.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_14` |
| [`locale/internal/currency_names_15`](../../std/locale/internal/currency_names_15.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_15` |
| [`locale/internal/currency_names_16`](../../std/locale/internal/currency_names_16.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_16` |
| [`locale/internal/currency_names_17`](../../std/locale/internal/currency_names_17.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_17` |
| [`locale/internal/currency_names_18`](../../std/locale/internal/currency_names_18.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_18` |
| [`locale/internal/currency_names_19`](../../std/locale/internal/currency_names_19.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_19` |
| [`locale/internal/currency_names_2`](../../std/locale/internal/currency_names_2.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_2` |
| [`locale/internal/currency_names_20`](../../std/locale/internal/currency_names_20.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_20` |
| [`locale/internal/currency_names_21`](../../std/locale/internal/currency_names_21.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_21` |
| [`locale/internal/currency_names_22`](../../std/locale/internal/currency_names_22.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_22` |
| [`locale/internal/currency_names_23`](../../std/locale/internal/currency_names_23.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_23` |
| [`locale/internal/currency_names_24`](../../std/locale/internal/currency_names_24.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_24` |
| [`locale/internal/currency_names_25`](../../std/locale/internal/currency_names_25.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_25` |
| [`locale/internal/currency_names_26`](../../std/locale/internal/currency_names_26.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_26` |
| [`locale/internal/currency_names_27`](../../std/locale/internal/currency_names_27.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_27` |
| [`locale/internal/currency_names_3`](../../std/locale/internal/currency_names_3.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_3` |
| [`locale/internal/currency_names_4`](../../std/locale/internal/currency_names_4.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_4` |
| [`locale/internal/currency_names_5`](../../std/locale/internal/currency_names_5.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_5` |
| [`locale/internal/currency_names_6`](../../std/locale/internal/currency_names_6.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_6` |
| [`locale/internal/currency_names_7`](../../std/locale/internal/currency_names_7.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_7` |
| [`locale/internal/currency_names_8`](../../std/locale/internal/currency_names_8.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_8` |
| [`locale/internal/currency_names_9`](../../std/locale/internal/currency_names_9.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_9` |
| [`locale/internal/currency_rows`](../../std/locale/internal/currency_rows.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/currency` → `std/locale/internal/currencies` → `std/locale/internal/currency_names_0` → `std/locale/internal/currency_rows` |
| [`locale/internal/day_periods`](../../std/locale/internal/day_periods.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/day_periods` |
| [`locale/internal/gregorian`](../../std/locale/internal/gregorian.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/gregorian` |
| [`locale/internal/iana`](../../std/locale/internal/iana.ok) | [labs/example-locale-i18n/main.ok](labs/example-locale-i18n/main.ok) | `labs/example-locale-i18n/main` → `std/locale/tag` → `std/locale/internal/iana` |
| [`locale/internal/likely`](../../std/locale/internal/likely.ok) | [labs/example-locale-i18n/main.ok](labs/example-locale-i18n/main.ok) | `labs/example-locale-i18n/main` → `std/locale/likely` → `std/locale/internal/likely` |
| [`locale/internal/lists`](../../std/locale/internal/lists.ok) | [labs/recipe-locale-list/main.ok](labs/recipe-locale-list/main.ok) | `labs/recipe-locale-list/main` → `std/locale/list` → `std/locale/internal/lists` |
| [`locale/internal/numbers`](../../std/locale/internal/numbers.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/number` → `std/locale/internal/numbers` |
| [`locale/internal/parents`](../../std/locale/internal/parents.ok) | [labs/example-locale-i18n/main.ok](labs/example-locale-i18n/main.ok) | `labs/example-locale-i18n/main` → `std/locale/data` → `std/locale/internal/parents` |
| [`locale/internal/plurals`](../../std/locale/internal/plurals.ok) | [labs/example-locale-i18n/main.ok](labs/example-locale-i18n/main.ok) | `labs/example-locale-i18n/main` → `std/i18n/plural` → `std/locale/internal/plurals` |
| [`locale/internal/zone_meta`](../../std/locale/internal/zone_meta.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_meta` |
| [`locale/internal/zone_name_rows`](../../std/locale/internal/zone_name_rows.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_0` → `std/locale/internal/zone_name_rows` |
| [`locale/internal/zone_names`](../../std/locale/internal/zone_names.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` |
| [`locale/internal/zone_names_0`](../../std/locale/internal/zone_names_0.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_0` |
| [`locale/internal/zone_names_1`](../../std/locale/internal/zone_names_1.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_1` |
| [`locale/internal/zone_names_10`](../../std/locale/internal/zone_names_10.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_10` |
| [`locale/internal/zone_names_11`](../../std/locale/internal/zone_names_11.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_11` |
| [`locale/internal/zone_names_12`](../../std/locale/internal/zone_names_12.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_12` |
| [`locale/internal/zone_names_13`](../../std/locale/internal/zone_names_13.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_13` |
| [`locale/internal/zone_names_14`](../../std/locale/internal/zone_names_14.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_14` |
| [`locale/internal/zone_names_15`](../../std/locale/internal/zone_names_15.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_15` |
| [`locale/internal/zone_names_16`](../../std/locale/internal/zone_names_16.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_16` |
| [`locale/internal/zone_names_17`](../../std/locale/internal/zone_names_17.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_17` |
| [`locale/internal/zone_names_18`](../../std/locale/internal/zone_names_18.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_18` |
| [`locale/internal/zone_names_19`](../../std/locale/internal/zone_names_19.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_19` |
| [`locale/internal/zone_names_2`](../../std/locale/internal/zone_names_2.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_2` |
| [`locale/internal/zone_names_20`](../../std/locale/internal/zone_names_20.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_20` |
| [`locale/internal/zone_names_21`](../../std/locale/internal/zone_names_21.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_21` |
| [`locale/internal/zone_names_22`](../../std/locale/internal/zone_names_22.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_22` |
| [`locale/internal/zone_names_23`](../../std/locale/internal/zone_names_23.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_23` |
| [`locale/internal/zone_names_24`](../../std/locale/internal/zone_names_24.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_24` |
| [`locale/internal/zone_names_25`](../../std/locale/internal/zone_names_25.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_25` |
| [`locale/internal/zone_names_26`](../../std/locale/internal/zone_names_26.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_26` |
| [`locale/internal/zone_names_27`](../../std/locale/internal/zone_names_27.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_27` |
| [`locale/internal/zone_names_3`](../../std/locale/internal/zone_names_3.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_3` |
| [`locale/internal/zone_names_4`](../../std/locale/internal/zone_names_4.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_4` |
| [`locale/internal/zone_names_5`](../../std/locale/internal/zone_names_5.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_5` |
| [`locale/internal/zone_names_6`](../../std/locale/internal/zone_names_6.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_6` |
| [`locale/internal/zone_names_7`](../../std/locale/internal/zone_names_7.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_7` |
| [`locale/internal/zone_names_8`](../../std/locale/internal/zone_names_8.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_8` |
| [`locale/internal/zone_names_9`](../../std/locale/internal/zone_names_9.ok) | [labs/example-localized-date-time/main.ok](labs/example-localized-date-time/main.ok) | `labs/example-localized-date-time/main` → `std/locale/date_time` → `std/locale/internal/zone_names` → `std/locale/internal/zone_names_9` |
| [`locale/likely`](../../std/locale/likely.ok) | [labs/example-locale-i18n/main.ok](labs/example-locale-i18n/main.ok) | `labs/example-locale-i18n/main` → `std/locale/likely` |
| [`locale/list`](../../std/locale/list.ok) | [labs/recipe-locale-list/main.ok](labs/recipe-locale-list/main.ok) | `labs/recipe-locale-list/main` → `std/locale/list` |
| [`locale/match`](../../std/locale/match.ok) | [labs/example-locale-i18n/main.ok](labs/example-locale-i18n/main.ok) | `labs/example-locale-i18n/main` → `std/locale/match` |
| [`locale/number`](../../std/locale/number.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/number` |
| [`locale/system`](../../std/locale/system.ok) | [labs/recipe-locale-list/main.ok](labs/recipe-locale-list/main.ok) | `labs/recipe-locale-list/main` → `std/locale/system` |
| [`locale/tag`](../../std/locale/tag.ok) | [labs/example-locale-i18n/main.ok](labs/example-locale-i18n/main.ok) | `labs/example-locale-i18n/main` → `std/locale/tag` |
| [`logging/enrichment`](../../std/logging/enrichment.ok) | [labs/recipe-logging-policy/main.ok](labs/recipe-logging-policy/main.ok) | `labs/recipe-logging-policy/main` → `std/logging/enrichment` |
| [`logging/field`](../../std/logging/field.ok) | [labs/example-logging-stack/main.ok](labs/example-logging-stack/main.ok) | `labs/example-logging-stack/main` → `std/logging/field` |
| [`logging/file`](../../std/logging/file.ok) | [labs/recipe-logging-file/main.ok](labs/recipe-logging-file/main.ok) | `labs/recipe-logging-file/main` → `std/logging/file` |
| [`logging/filter`](../../std/logging/filter.ok) | [labs/recipe-logging-policy/main.ok](labs/recipe-logging-policy/main.ok) | `labs/recipe-logging-policy/main` → `std/logging/filter` |
| [`logging/level`](../../std/logging/level.ok) | [labs/example-logging-stack/main.ok](labs/example-logging-stack/main.ok) | `labs/example-logging-stack/main` → `std/logging/level` |
| [`logging/line`](../../std/logging/line.ok) | [labs/example-logging-stack/main.ok](labs/example-logging-stack/main.ok) | `labs/example-logging-stack/main` → `std/logging/stream` → `std/logging/line` |
| [`logging/name`](../../std/logging/name.ok) | [labs/example-logging-stack/main.ok](labs/example-logging-stack/main.ok) | `labs/example-logging-stack/main` → `std/logging/name` |
| [`logging/named`](../../std/logging/named.ok) | [labs/example-logging-stack/main.ok](labs/example-logging-stack/main.ok) | `labs/example-logging-stack/main` → `std/logging/named` |
| [`logging/queue`](../../std/logging/queue.ok) | [labs/recipe-logging-file/main.ok](labs/recipe-logging-file/main.ok) | `labs/recipe-logging-file/main` → `std/logging/queue` |
| [`logging/record`](../../std/logging/record.ok) | [labs/example-logging-stack/main.ok](labs/example-logging-stack/main.ok) | `labs/example-logging-stack/main` → `std/logging/named` → `std/logging/record` |
| [`logging/redact`](../../std/logging/redact.ok) | [labs/example-logging-stack/main.ok](labs/example-logging-stack/main.ok) | `labs/example-logging-stack/main` → `std/logging/redact` |
| [`logging/rotation`](../../std/logging/rotation.ok) | [labs/recipe-logging-file/main.ok](labs/recipe-logging-file/main.ok) | `labs/recipe-logging-file/main` → `std/logging/rotation` |
| [`logging/stream`](../../std/logging/stream.ok) | [labs/example-logging-stack/main.ok](labs/example-logging-stack/main.ok) | `labs/example-logging-stack/main` → `std/logging/stream` |
| [`logging/syslog`](../../std/logging/syslog.ok) | [labs/recipe-logging-policy/main.ok](labs/recipe-logging-policy/main.ok) | `labs/recipe-logging-policy/main` → `std/logging/syslog` |
| [`logging/write`](../../std/logging/write.ok) | [labs/recipe-logging-policy/main.ok](labs/recipe-logging-policy/main.ok) | `labs/recipe-logging-policy/main` → `std/logging/write` |
| [`mail/imap`](../../std/mail/imap.ok) | [labs/example-imap-capabilities/main.ok](labs/example-imap-capabilities/main.ok) | `labs/example-imap-capabilities/main` → `std/mail/imap` |
| [`mail/imap_changes`](../../std/mail/imap_changes.ok) | [labs/recipe-imap-assignment/main.ok](labs/recipe-imap-assignment/main.ok) | `labs/recipe-imap-assignment/main` → `std/mail/imap_changes` |
| [`mail/imap_command`](../../std/mail/imap_command.ok) | [labs/example-imap-session/main.ok](labs/example-imap-session/main.ok) | `labs/example-imap-session/main` → `std/mail/imap_command` |
| [`mail/imap_command_reading`](../../std/mail/imap_command_reading.ok) | [labs/example-imap-session/main.ok](labs/example-imap-session/main.ok) | `labs/example-imap-session/main` → `std/mail/imap_command_reading` |
| [`mail/imap_data`](../../std/mail/imap_data.ok) | [labs/example-imap-session/main.ok](labs/example-imap-session/main.ok) | `labs/example-imap-session/main` → `std/mail/imap_session` → `std/mail/imap_data` |
| [`mail/imap_exchange`](../../std/mail/imap_exchange.ok) | [labs/example-imap-session/main.ok](labs/example-imap-session/main.ok) | `labs/example-imap-session/main` → `std/mail/imap_exchange` |
| [`mail/imap_message`](../../std/mail/imap_message.ok) | [labs/example-imap-session/main.ok](labs/example-imap-session/main.ok) | `labs/example-imap-session/main` → `std/mail/imap_session` → `std/mail/imap_message` |
| [`mail/imap_metadata`](../../std/mail/imap_metadata.ok) | [labs/example-imap-session/main.ok](labs/example-imap-session/main.ok) | `labs/example-imap-session/main` → `std/mail/imap_session` → `std/mail/imap_metadata` |
| [`mail/imap_name`](../../std/mail/imap_name.ok) | [labs/example-imap-names/main.ok](labs/example-imap-names/main.ok) | `labs/example-imap-names/main` → `std/mail/imap_name` |
| [`mail/imap_reading`](../../std/mail/imap_reading.ok) | [labs/example-imap-reading/main.ok](labs/example-imap-reading/main.ok) | `labs/example-imap-reading/main` → `std/mail/imap_reading` |
| [`mail/imap_response`](../../std/mail/imap_response.ok) | [labs/example-imap-responses/main.ok](labs/example-imap-responses/main.ok) | `labs/example-imap-responses/main` → `std/mail/imap_response` |
| [`mail/imap_search`](../../std/mail/imap_search.ok) | [labs/example-imap-session/main.ok](labs/example-imap-session/main.ok) | `labs/example-imap-session/main` → `std/mail/imap_search` |
| [`mail/imap_session`](../../std/mail/imap_session.ok) | [labs/example-imap-session/main.ok](labs/example-imap-session/main.ok) | `labs/example-imap-session/main` → `std/mail/imap_session` |
| [`mail/imap_state`](../../std/mail/imap_state.ok) | [labs/example-imap-session/main.ok](labs/example-imap-session/main.ok) | `labs/example-imap-session/main` → `std/mail/imap_session` → `std/mail/imap_state` |
| [`mail/imap_uid`](../../std/mail/imap_uid.ok) | [labs/example-imap-session/main.ok](labs/example-imap-session/main.ok) | `labs/example-imap-session/main` → `std/mail/imap_session` → `std/mail/imap_uid` |
| [`mail/mailbox`](../../std/mail/mailbox.ok) | [labs/example-mail-flags/main.ok](labs/example-mail-flags/main.ok) | `labs/example-mail-flags/main` → `std/mail/mailbox` |
| [`mail/mbox`](../../std/mail/mbox.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mail/mbox` |
| [`mail/mbox_store`](../../std/mail/mbox_store.ok) | [labs/example-mbox-session/main.ok](labs/example-mbox-session/main.ok) | `labs/example-mbox-session/main` → `std/mail/mbox_store` |
| [`mail/message`](../../std/mail/message.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mail/message` |
| [`mail/pop3`](../../std/mail/pop3.ok) | [labs/example-pop3-session/main.ok](labs/example-pop3-session/main.ok) | `labs/example-pop3-session/main` → `std/mail/pop3` |
| [`mail/pop3_reading`](../../std/mail/pop3_reading.ok) | [labs/example-pop3-replies/main.ok](labs/example-pop3-replies/main.ok) | `labs/example-pop3-replies/main` → `std/mail/pop3_reading` |
| [`mail/pop3_reply`](../../std/mail/pop3_reply.ok) | [labs/example-pop3-replies/main.ok](labs/example-pop3-replies/main.ok) | `labs/example-pop3-replies/main` → `std/mail/pop3_reply` |
| [`mail/pop3_session`](../../std/mail/pop3_session.ok) | [labs/example-pop3-session/main.ok](labs/example-pop3-session/main.ok) | `labs/example-pop3-session/main` → `std/mail/pop3_session` |
| [`mail/smtp`](../../std/mail/smtp.ok) | [labs/example-smtp-wire/main.ok](labs/example-smtp-wire/main.ok) | `labs/example-smtp-wire/main` → `std/mail/smtp` |
| [`mail/smtp_data`](../../std/mail/smtp_data.ok) | [labs/example-smtp-wire/main.ok](labs/example-smtp-wire/main.ok) | `labs/example-smtp-wire/main` → `std/mail/smtp_data` |
| [`mail/smtp_reading`](../../std/mail/smtp_reading.ok) | [labs/example-smtp-reading/main.ok](labs/example-smtp-reading/main.ok) | `labs/example-smtp-reading/main` → `std/mail/smtp_reading` |
| [`mail/smtp_reply`](../../std/mail/smtp_reply.ok) | [labs/example-smtp-wire/main.ok](labs/example-smtp-wire/main.ok) | `labs/example-smtp-wire/main` → `std/mail/smtp_reply` |
| [`math/aggregate`](../../std/math/aggregate.ok) | [labs/recipe-measurements/main.ok](labs/recipe-measurements/main.ok) | `labs/recipe-measurements/main` → `std/math/aggregate` |
| [`math/constants`](../../std/math/constants.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `support/chat_room` → `std/uuid/generation` → `std/random/distribution` → `std/math/constants` |
| [`math/decimal`](../../std/math/decimal.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/math/decimal` |
| [`math/rounding`](../../std/math/rounding.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/math/rounding` |
| [`message/header`](../../std/message/header.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/message/header` |
| [`mime/disposition`](../../std/mime/disposition.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mime/part` → `std/mime/disposition` |
| [`mime/media_type`](../../std/mime/media_type.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mail/message` → `std/mime/media_type` |
| [`mime/part`](../../std/mime/part.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mime/part` |
| [`mime/transfer`](../../std/mime/transfer.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mime/part` → `std/mime/transfer` |
| [`mime/types`](../../std/mime/types.ok) | [labs/example-http-static/main.ok](labs/example-http-static/main.ok) | `labs/example-http-static/main` → `std/mime/types` |
| [`mime/word`](../../std/mime/word.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mime/word` |
| [`net/dns`](../../std/net/dns.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/dns` |
| [`net/endpoint`](../../std/net/endpoint.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/net/endpoint` |
| [`net/failure`](../../std/net/failure.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/net/failure` |
| [`net/interface`](../../std/net/interface.ok) | [labs/example-network-interface/main.ok](labs/example-network-interface/main.ok) | `labs/example-network-interface/main` → `std/net/interface` |
| [`net/ip`](../../std/net/ip.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/ip` |
| [`net/ip_registry`](../../std/net/ip_registry.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/ip` → `std/net/ip_registry` |
| [`net/multicast`](../../std/net/multicast.ok) | [labs/example-multicast-options/main.ok](labs/example-multicast-options/main.ok) | `labs/example-multicast-options/main` → `std/net/multicast` |
| [`net/public_suffix`](../../std/net/public_suffix.ok) | [labs/example-host-policy/main.ok](labs/example-host-policy/main.ok) | `labs/example-host-policy/main` → `std/net/public_suffix` |
| [`net/stream`](../../std/net/stream.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/net/stream` |
| [`net/tcp`](../../std/net/tcp.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/net/tcp` |
| [`net/tcp_racing`](../../std/net/tcp_racing.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/net/tcp_racing` |
| [`net/tls`](../../std/net/tls.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/tls` |
| [`net/udp`](../../std/net/udp.ok) | [labs/example-udp-options/main.ok](labs/example-udp-options/main.ok) | `labs/example-udp-options/main` → `std/net/udp` |
| [`net/unix`](../../std/net/unix.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/http1_message_writing` → `std/net/unix` |
| [`numbers/bigint`](../../std/numbers/bigint.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/numbers/bigint` |
| [`numbers/bigrational`](../../std/numbers/bigrational.ok) | [labs/example-bigrational/main.ok](labs/example-bigrational/main.ok) | `labs/example-bigrational/main` → `std/numbers/bigrational` |
| [`numbers/float`](../../std/numbers/float.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/number` → `std/numbers/float` |
| [`os/account`](../../std/os/account.ok) | [labs/recipe-system/main.ok](labs/recipe-system/main.ok) | `labs/recipe-system/main` → `std/os/account` |
| [`os/command`](../../std/os/command.ok) | [labs/recipe-commands/main.ok](labs/recipe-commands/main.ok) | `labs/recipe-commands/main` → `std/os/command` |
| [`os/command_capture`](../../std/os/command_capture.ok) | [labs/recipe-commands/main.ok](labs/recipe-commands/main.ok) | `labs/recipe-commands/main` → `std/os/command_capture` |
| [`os/directory`](../../std/os/directory.ok) | [labs/recipe-system/main.ok](labs/recipe-system/main.ok) | `labs/recipe-system/main` → `std/os/directory` |
| [`os/environment`](../../std/os/environment.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/os/environment` |
| [`os/pipeline_capture`](../../std/os/pipeline_capture.ok) | [labs/recipe-pipeline/main.ok](labs/recipe-pipeline/main.ok) | `labs/recipe-pipeline/main` → `std/os/pipeline_capture` |
| [`os/platform`](../../std/os/platform.ok) | [labs/recipe-system/main.ok](labs/recipe-system/main.ok) | `labs/recipe-system/main` → `std/os/platform` |
| [`os/process`](../../std/os/process.ok) | [labs/recipe-commands/main.ok](labs/recipe-commands/main.ok) | `labs/recipe-commands/main` → `std/os/command_capture` → `std/os/process` |
| [`os/storage`](../../std/os/storage.ok) | [labs/recipe-system/main.ok](labs/recipe-system/main.ok) | `labs/recipe-system/main` → `std/os/storage` |
| [`parallel/map`](../../std/parallel/map.ok) | [labs/example-parallel-map/main.ok](labs/example-parallel-map/main.ok) | `labs/example-parallel-map/main` → `std/parallel/map` |
| [`plist/parse`](../../std/plist/parse.ok) | [labs/example-plist-parse/main.ok](labs/example-plist-parse/main.ok) | `labs/example-plist-parse/main` → `std/plist/parse` |
| [`plist/value`](../../std/plist/value.ok) | [labs/example-plist-parse/main.ok](labs/example-plist-parse/main.ok) | `labs/example-plist-parse/main` → `std/plist/value` |
| [`plist/write`](../../std/plist/write.ok) | [labs/example-plist-write/main.ok](labs/example-plist-write/main.ok) | `labs/example-plist-write/main` → `std/plist/write` |
| [`random/distribution`](../../std/random/distribution.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `support/chat_room` → `std/uuid/generation` → `std/random/distribution` |
| [`random/source`](../../std/random/source.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/random/source` |
| [`regex/escape`](../../std/regex/escape.ok) | [labs/recipe-regex/main.ok](labs/recipe-regex/main.ok) | `labs/recipe-regex/main` → `std/regex/escape` |
| [`regex/match`](../../std/regex/match.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/json/write` → `std/regex/pattern` → `std/regex/match` |
| [`regex/pattern`](../../std/regex/pattern.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/json/write` → `std/regex/pattern` |
| [`regex/range`](../../std/regex/range.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/json/write` → `std/regex/pattern` → `std/regex/range` |
| [`regex/replace`](../../std/regex/replace.ok) | [labs/recipe-regex/main.ok](labs/recipe-regex/main.ok) | `labs/recipe-regex/main` → `std/regex/replace` |
| [`regex/search`](../../std/regex/search.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/json/write` → `std/regex/search` |
| [`regex/split`](../../std/regex/split.ok) | [labs/recipe-regex/main.ok](labs/recipe-regex/main.ok) | `labs/recipe-regex/main` → `std/regex/split` |
| [`runtime/account`](../../std/runtime/account.ok) | [labs/recipe-system/main.ok](labs/recipe-system/main.ok) | `labs/recipe-system/main` → `std/os/account` → `std/runtime/account` |
| [`runtime/application`](../../std/runtime/application.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/terminal/session` → `std/runtime/application` |
| [`runtime/base32`](../../std/runtime/base32.ok) | [labs/recipe-encodings/main.ok](labs/recipe-encodings/main.ok) | `labs/recipe-encodings/main` → `std/encoding/base32` → `std/runtime/base32` |
| [`runtime/base64`](../../std/runtime/base64.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mime/word` → `std/encoding/base64` → `std/runtime/base64` |
| [`runtime/base85`](../../std/runtime/base85.ok) | [labs/recipe-encodings/main.ok](labs/recipe-encodings/main.ok) | `labs/recipe-encodings/main` → `std/encoding/base85` → `std/runtime/base85` |
| [`runtime/bigint`](../../std/runtime/bigint.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/numbers/bigint` → `std/runtime/bigint` |
| [`runtime/binary`](../../std/runtime/binary.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/url/value` → `std/encoding/binary` → `std/runtime/binary` |
| [`runtime/buffer`](../../std/runtime/buffer.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/json/write` → `std/io/buffer` → `std/runtime/buffer` |
| [`runtime/collation`](../../std/runtime/collation.ok) | [labs/example-localized-collation/main.ok](labs/example-localized-collation/main.ok) | `labs/example-localized-collation/main` → `std/locale/collation` → `std/runtime/collation` |
| [`runtime/collections`](../../std/runtime/collections.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/web/routing` → `std/http/routing` → `std/runtime/collections` |
| [`runtime/crypto`](../../std/runtime/crypto.ok) | [labs/recipe-sha256/main.ok](labs/recipe-sha256/main.ok) | `labs/recipe-sha256/main` → `std/runtime/crypto` |
| [`runtime/csv_state`](../../std/runtime/csv_state.ok) | [labs/recipe-csv-stream/main.ok](labs/recipe-csv-stream/main.ok) | `labs/recipe-csv-stream/main` → `std/csv/stream` → `std/runtime/csv_state` |
| [`runtime/database`](../../std/runtime/database.ok) | [apps/taxi.ok](apps/taxi.ok) | `apps/taxi` → `std/db/sqlite` → `std/runtime/database` |
| [`runtime/debug`](../../std/runtime/debug.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/check` → `std/runtime/debug` |
| [`runtime/decimal`](../../std/runtime/decimal.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/math/decimal` → `std/runtime/decimal` |
| [`runtime/diff`](../../std/runtime/diff.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/check` → `std/diff` → `std/runtime/diff` |
| [`runtime/environment`](../../std/runtime/environment.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/os/environment` → `std/runtime/environment` |
| [`runtime/format`](../../std/runtime/format.ok) | [labs/example-localized-currencies/main.ok](labs/example-localized-currencies/main.ok) | `labs/example-localized-currencies/main` → `std/locale/number` → `std/numbers/float` → `std/runtime/format` |
| [`runtime/fs`](../../std/runtime/fs.ok) | [apps/docs.ok](apps/docs.ok) | `apps/docs` → `std/fs/file` → `std/runtime/fs` |
| [`runtime/fs_root`](../../std/runtime/fs_root.ok) | [labs/example-http-static/main.ok](labs/example-http-static/main.ok) | `labs/example-http-static/main` → `std/fs/root` → `std/runtime/fs_root` |
| [`runtime/hex`](../../std/runtime/hex.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mime/part` → `std/encoding/hex` → `std/runtime/hex` |
| [`runtime/hpack`](../../std/runtime/hpack.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/http/server` → `std/http/http2_server` → `std/http/internal/hpack` → `std/http/internal/hpack_primitives` → `std/runtime/hpack` |
| [`runtime/html`](../../std/runtime/html.ok) | [apps/docs.ok](apps/docs.ok) | `apps/docs` → `std/html/entities` → `std/runtime/html` |
| [`runtime/idna`](../../std/runtime/idna.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/net/endpoint` → `std/runtime/idna` |
| [`runtime/ini`](../../std/runtime/ini.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/config/document` → `std/config/ini` → `std/runtime/ini` |
| [`runtime/input_state`](../../std/runtime/input_state.ok) | [apps/docs.ok](apps/docs.ok) | `apps/docs` → `std/fs/file` → `std/io/stdin` → `std/runtime/input_state` |
| [`runtime/io_decorators`](../../std/runtime/io_decorators.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/url/value` → `std/io/decorators` → `std/runtime/io_decorators` |
| [`runtime/json`](../../std/runtime/json.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/json/parse` → `std/runtime/json` |
| [`runtime/layout`](../../std/runtime/layout.ok) | [labs/recipe-text-unicode/main.ok](labs/recipe-text-unicode/main.ok) | `labs/recipe-text-unicode/main` → `std/text/layout` → `std/runtime/layout` |
| [`runtime/locale`](../../std/runtime/locale.ok) | [labs/recipe-locale-list/main.ok](labs/recipe-locale-list/main.ok) | `labs/recipe-locale-list/main` → `std/locale/system` → `std/runtime/locale` |
| [`runtime/math_aggregate`](../../std/runtime/math_aggregate.ok) | [labs/recipe-measurements/main.ok](labs/recipe-measurements/main.ok) | `labs/recipe-measurements/main` → `std/math/aggregate` → `std/runtime/math_aggregate` |
| [`runtime/net`](../../std/runtime/net.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/ip` → `std/runtime/net` |
| [`runtime/network`](../../std/runtime/network.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/dns` → `std/runtime/network` |
| [`runtime/ordering`](../../std/runtime/ordering.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/dns` → `std/collections/ordering` → `std/runtime/ordering` |
| [`runtime/pem`](../../std/runtime/pem.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/crypto/x509/trust` → `std/runtime/pem` |
| [`runtime/process`](../../std/runtime/process.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `support/arguments` → `std/cli/arguments` → `std/runtime/process` |
| [`runtime/public_suffix`](../../std/runtime/public_suffix.ok) | [labs/example-host-policy/main.ok](labs/example-host-policy/main.ok) | `labs/example-host-policy/main` → `std/net/public_suffix` → `std/runtime/public_suffix` |
| [`runtime/quoted_printable`](../../std/runtime/quoted_printable.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mime/part` → `std/mime/transfer` → `std/encoding/quoted_printable` → `std/runtime/quoted_printable` |
| [`runtime/random`](../../std/runtime/random.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/random/source` → `std/runtime/random` |
| [`runtime/random_distribution`](../../std/runtime/random_distribution.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `support/chat_room` → `std/uuid/generation` → `std/random/distribution` → `std/runtime/random_distribution` |
| [`runtime/regex`](../../std/runtime/regex.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/json/write` → `std/regex/pattern` → `std/runtime/regex` |
| [`runtime/secret`](../../std/runtime/secret.ok) | [labs/example-http-digest-https/main.ok](labs/example-http-digest-https/main.ok) | `labs/example-http-digest-https/main` → `std/crypto/secret` → `std/runtime/secret` |
| [`runtime/single_byte`](../../std/runtime/single_byte.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/http/websocket_client` → `std/http/http1_head` → `std/runtime/single_byte` |
| [`runtime/snapshot`](../../std/runtime/snapshot.ok) | [labs/example-testing-snapshots/main.ok](labs/example-testing-snapshots/main.ok) | `labs/example-testing-snapshots/main` → `std/testing/snapshot` → `std/runtime/snapshot` |
| [`runtime/statistics`](../../std/runtime/statistics.ok) | [labs/recipe-measurements/main.ok](labs/recipe-measurements/main.ok) | `labs/recipe-measurements/main` → `std/statistics` → `std/runtime/statistics` |
| [`runtime/stdio`](../../std/runtime/stdio.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/io/stdout` → `std/runtime/stdio` |
| [`runtime/system`](../../std/runtime/system.ok) | [apps/docs.ok](apps/docs.ok) | `apps/docs` → `std/fs/path` → `std/runtime/system` |
| [`runtime/template`](../../std/runtime/template.ok) | [labs/example-template-context/main.ok](labs/example-template-context/main.ok) | `labs/example-template-context/main` → `std/template/json` → `std/runtime/template` |
| [`runtime/testing`](../../std/runtime/testing.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/double` → `std/runtime/testing` |
| [`runtime/time`](../../std/runtime/time.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/time/clock` → `std/runtime/time` |
| [`runtime/time_format`](../../std/runtime/time_format.ok) | [labs/recipe-calendar/main.ok](labs/recipe-calendar/main.ok) | `labs/recipe-calendar/main` → `std/time/format` → `std/runtime/time_format` |
| [`runtime/time_zone`](../../std/runtime/time_zone.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mail/message` → `std/time/zone` → `std/runtime/time_zone` |
| [`runtime/tls_transport`](../../std/runtime/tls_transport.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/net/tls` → `std/runtime/tls_transport` |
| [`runtime/toml`](../../std/runtime/toml.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/toml/parse` → `std/runtime/toml` |
| [`runtime/unicode_character`](../../std/runtime/unicode_character.ok) | [labs/recipe-display-columns/main.ok](labs/recipe-display-columns/main.ok) | `labs/recipe-display-columns/main` → `std/terminal/width` → `std/runtime/unicode_character` |
| [`runtime/unicode_normalization`](../../std/runtime/unicode_normalization.ok) | [labs/recipe-text-unicode/main.ok](labs/recipe-text-unicode/main.ok) | `labs/recipe-text-unicode/main` → `std/unicode/normalization` → `std/runtime/unicode_normalization` |
| [`runtime/url_text`](../../std/runtime/url_text.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/url/value` → `std/runtime/url_text` |
| [`runtime/utf`](../../std/runtime/utf.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/encoding/utf` → `std/runtime/utf` |
| [`runtime/uuid`](../../std/runtime/uuid.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `support/chat_room` → `std/uuid/generation` → `std/runtime/uuid` |
| [`runtime/x509`](../../std/runtime/x509.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/crypto/x509/trust` → `std/runtime/x509` |
| [`size`](../../std/size.ok) | [apps/payroll.ok](apps/payroll.ok) | `apps/payroll` → `std/collections/set` → `std/size` |
| [`statistics`](../../std/statistics.ok) | [labs/recipe-measurements/main.ok](labs/recipe-measurements/main.ok) | `labs/recipe-measurements/main` → `std/statistics` |
| [`strings/format`](../../std/strings/format.ok) | [labs/example-rendering/main.ok](labs/example-rendering/main.ok) | `labs/example-rendering/main` → `std/strings/format` |
| [`strings/join`](../../std/strings/join.ok) | [labs/example-joined/main.ok](labs/example-joined/main.ok) | `labs/example-joined/main` → `std/strings/join` |
| [`strings/parse`](../../std/strings/parse.ok) | [apps/docs.ok](apps/docs.ok) | `apps/docs` → `std/strings/parse` |
| [`template/compile`](../../std/template/compile.ok) | [labs/example-template-json/main.ok](labs/example-template-json/main.ok) | `labs/example-template-json/main` → `std/template/compile` |
| [`template/html`](../../std/template/html.ok) | [labs/example-template-html/main.ok](labs/example-template-html/main.ok) | `labs/example-template-html/main` → `std/template/html` |
| [`template/html_library`](../../std/template/html_library.ok) | [labs/example-template-html-library/main.ok](labs/example-template-html-library/main.ok) | `labs/example-template-html-library/main` → `std/template/html_library` |
| [`template/json`](../../std/template/json.ok) | [labs/example-template-context/main.ok](labs/example-template-context/main.ok) | `labs/example-template-context/main` → `std/template/json` |
| [`template/library`](../../std/template/library.ok) | [labs/example-template-library/main.ok](labs/example-template-library/main.ok) | `labs/example-template-library/main` → `std/template/library` |
| [`template/model`](../../std/template/model.ok) | [labs/example-template-library/main.ok](labs/example-template-library/main.ok) | `labs/example-template-library/main` → `std/template/model` |
| [`template/path`](../../std/template/path.ok) | [labs/example-template-library/main.ok](labs/example-template-library/main.ok) | `labs/example-template-library/main` → `std/template/path` |
| [`template/render`](../../std/template/render.ok) | [labs/example-template-output/main.ok](labs/example-template-output/main.ok) | `labs/example-template-output/main` → `std/template/render` |
| [`template/sections`](../../std/template/sections.ok) | [labs/example-template-partials/main.ok](labs/example-template-partials/main.ok) | `labs/example-template-partials/main` → `std/template/sections` |
| [`template/tree`](../../std/template/tree.ok) | [labs/example-template-tree/main.ok](labs/example-template-tree/main.ok) | `labs/example-template-tree/main` → `std/template/tree` |
| [`terminal/capability`](../../std/terminal/capability.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/terminal/capability` |
| [`terminal/error`](../../std/terminal/error.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/terminal/session` → `std/terminal/error` |
| [`terminal/mode`](../../std/terminal/mode.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/terminal/session` → `std/terminal/mode` |
| [`terminal/prompt`](../../std/terminal/prompt.ok) | [labs/recipe-terminal-prompt/main.ok](labs/recipe-terminal-prompt/main.ok) | `labs/recipe-terminal-prompt/main` → `std/terminal/prompt` |
| [`terminal/screen`](../../std/terminal/screen.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/terminal/session` → `std/terminal/screen` |
| [`terminal/session`](../../std/terminal/session.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/terminal/session` |
| [`terminal/style`](../../std/terminal/style.ok) | [labs/example-application-cli/main.ok](labs/example-application-cli/main.ok) | `labs/example-application-cli/main` → `std/terminal/style` |
| [`terminal/width`](../../std/terminal/width.ok) | [labs/recipe-display-columns/main.ok](labs/recipe-display-columns/main.ok) | `labs/recipe-display-columns/main` → `std/terminal/width` |
| [`testing/check`](../../std/testing/check.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/check` |
| [`testing/context`](../../std/testing/context.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/check` → `std/testing/context` |
| [`testing/double`](../../std/testing/double.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/double` |
| [`testing/error`](../../std/testing/error.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/error` |
| [`testing/generation`](../../std/testing/generation.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/generation` |
| [`testing/property`](../../std/testing/property.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/property` |
| [`testing/report`](../../std/testing/report.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/report` |
| [`testing/result`](../../std/testing/result.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/result` |
| [`testing/runner`](../../std/testing/runner.ok) | [labs/recipe-runner-report/main.ok](labs/recipe-runner-report/main.ok) | `labs/recipe-runner-report/main` → `std/testing/runner` |
| [`testing/snapshot`](../../std/testing/snapshot.ok) | [labs/example-testing-snapshots/main.ok](labs/example-testing-snapshots/main.ok) | `labs/example-testing-snapshots/main` → `std/testing/snapshot` |
| [`testing/time`](../../std/testing/time.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/time` |
| [`text/layout`](../../std/text/layout.ok) | [labs/recipe-text-unicode/main.ok](labs/recipe-text-unicode/main.ok) | `labs/recipe-text-unicode/main` → `std/text/layout` |
| [`time/clock`](../../std/time/clock.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/time/clock` |
| [`time/date`](../../std/time/date.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mail/message` → `std/time/date` |
| [`time/day`](../../std/time/day.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mail/message` → `std/time/day` |
| [`time/duration`](../../std/time/duration.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/time/duration` |
| [`time/format`](../../std/time/format.ok) | [labs/recipe-calendar/main.ok](labs/recipe-calendar/main.ok) | `labs/recipe-calendar/main` → `std/time/format` |
| [`time/iso`](../../std/time/iso.ok) | [apps/payroll.ok](apps/payroll.ok) | `apps/payroll` → `std/time/iso` |
| [`time/local_datetime`](../../std/time/local_datetime.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mail/message` → `std/time/local_datetime` |
| [`time/rfc3339`](../../std/time/rfc3339.ok) | [labs/recipe-time-api/main.ok](labs/recipe-time-api/main.ok) | `labs/recipe-time-api/main` → `std/time/rfc3339` |
| [`time/zone`](../../std/time/zone.ok) | [apps/mail.ok](apps/mail.ok) | `apps/mail` → `std/mail/message` → `std/time/zone` |
| [`toml/parse`](../../std/toml/parse.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/toml/parse` |
| [`toml/value`](../../std/toml/value.ok) | [apps/formats.ok](apps/formats.ok) | `apps/formats` → `std/toml/parse` → `std/toml/value` |
| [`unicode/character`](../../std/unicode/character.ok) | [labs/recipe-text-unicode/main.ok](labs/recipe-text-unicode/main.ok) | `labs/recipe-text-unicode/main` → `std/unicode/character` |
| [`unicode/normalization`](../../std/unicode/normalization.ok) | [labs/recipe-text-unicode/main.ok](labs/recipe-text-unicode/main.ok) | `labs/recipe-text-unicode/main` → `std/unicode/normalization` |
| [`url/form`](../../std/url/form.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/web/request` → `std/url/form` |
| [`url/percent`](../../std/url/percent.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/url/value` → `std/url/percent` |
| [`url/query`](../../std/url/query.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/web/request` → `std/url/query` |
| [`url/value`](../../std/url/value.ok) | [apps/chat-client.ok](apps/chat-client.ok) | `apps/chat-client` → `std/url/value` |
| [`uuid/generation`](../../std/uuid/generation.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `support/chat_room` → `std/uuid/generation` |
| [`uuid/value`](../../std/uuid/value.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `support/chat_room` → `std/uuid/generation` → `std/uuid/value` |
| [`web/app`](../../std/web/app.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `support/http` → `std/web/app` |
| [`web/exception`](../../std/web/exception.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/web/exception` |
| [`web/internal/reply`](../../std/web/internal/reply.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/web/exception` → `std/web/internal/reply` |
| [`web/reply`](../../std/web/reply.ok) | [apps/docs.ok](apps/docs.ok) | `apps/docs` → `std/web/reply` |
| [`web/request`](../../std/web/request.ok) | [apps/chat.ok](apps/chat.ok) | `apps/chat` → `std/web/request` |
| [`web/routing`](../../std/web/routing.ok) | [apps/blog.ok](apps/blog.ok) | `apps/blog` → `std/web/routing` |
| [`xml/name`](../../std/xml/name.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/report` → `std/xml/name` |
| [`xml/node`](../../std/xml/node.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/report` → `std/xml/node` |
| [`xml/write`](../../std/xml/write.ok) | [apps/tests.ok](apps/tests.ok) | `apps/tests` → `std/testing/report` → `std/xml/node` → `std/xml/write` |

Повторить проверку: `./examples/cookbook/run coverage`. Обновить карту после изменения исходников: `python3 examples/cookbook/script/coverage.py --write`.
