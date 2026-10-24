# Веб-примеры Okkama

Пять самостоятельных сценариев по задачам [Cro examples](https://github.com/ash/rakupp/tree/main/examples/modules/cro). Каждый запускает свой HTTP-сервер на `127.0.0.1:0`, узнаёт выбранный порт, делает запросы клиентом Okkama и завершает сервер через `defer` и `FutureGroup`. Запуски не требуют внешнего сервиса или настройки порта.

| Программа | Команда | Что проверяется |
|---|---|---|
| [service.ok](service.ok) | `./examples/gallery/run modules/web/service` | `/`, параметр `/greet/:name`, текстовый ответ |
| [rest-api.ok](rest-api.ok) | `./examples/gallery/run modules/web/rest-api` | JSON-книги, SQLite в памяти, POST/GET/DELETE, `201` и Location, typed path, query, headers, redirect, cookie, `400/404/418/500` |
| [forms.ok](forms.ok) | `./examples/gallery/run modules/web/forms` | urlencoded, multipart-поля, файловая часть, обязательное поле и `422` |
| [web-features.ok](web-features.ok) | `./examples/gallery/run modules/web/web-features` | Middleware до/после, Scope, wildcard, static, HTML-шаблон, chunked-ответ, SSE, параллельные запросы, двоичные upload/download |
| [websocket-chat.ok](websocket-chat.ok) | `./examples/gallery/run modules/web/websocket-chat` | Текстовое и двоичное echo, три клиента JSON-комнаты, общая последовательность, join/leave и same-origin |

## Сервер и клиент

[support.ok](support.ok) связывает `WebApplication` с тестовым loopback-сервером. `DemoClient` открывает реальный `Http1Session` на каждый запрос, передаёт поля и тело, дочитывает ответ с лимитом 2 MiB и закрывает транспорт. `ReplySnapshot` хранит байты ответа и заголовки; UTF-8 декодируется при вызове `text()`.

Поэтому строки `urlencoded: 200 ...` или `create: 201 ...` — результат настоящего HTTP-обмена. Для двоичного ответа используется `bytes()`.

REST API хранит книги через `SqliteMemory`. Контроллер получает узкий `DbExecuting`; SQL параметризован. Для преобразования строк используется общий [QueryJson](../../../cookbook/support/sql.ok) из соседнего кукбука. Redirect проверяется явно: клиент наблюдает Location и выполняет следующий запрос. Cookie также явно передаётся из ответа в следующий запрос.

## Файлы и потоки

`web-features` читает [CSS](static/site.css) через `StaticFiles` и `FileRoot`, а [шаблон](templates/page.html) — через `HtmlTemplate`. Подстановка `fresh <today>` превращается в `fresh &lt;today&gt;`.

`/count` передаёт пять строк чанками, `/events` — события `tick` с данными `alpha`, `beta`, `gamma`. `ChunkInput` владеет очередью и буфером текущего чанка, соблюдая лимит чтения. В двоичном сценарии передаются 8192 байта с повторяющимися значениями 0..255; после скачивания сравниваются сами байты.

Обёртка `ObservedHandling` записывает пути в ограниченный канал и добавляет `X-Served-By`. После запросов сценарий проверяет количество записей и первый путь.

## Комната чата

[chat-room.ok](chat-room.ok) — актор с каналом команд. `RoomState` содержит неизменяемые список участников и номер последовательности. Join, leave и публикация возвращают следующую версию. Подписчики получают JSON `{seq,from,text}` через собственные ограниченные очереди.

Порядок сценария фиксируется чтением событий:

1. Ann, Bob и Cat подключаются по очереди.
2. Все трое получают сообщение Ann под номером 4.
3. Cat закрывает соединение; оставшиеся получают `cat left` под номером 5.
4. Bob отправляет сообщение под номером 6.
5. Ann выходит; Bob наблюдает событие под номером 7 и завершает соединение.

Синхронизация происходит по сообщениям, без задержек «на всякий случай». Политика Upgrade использует явный trusted URL, подпротокол `gallery-chat` и `SameOriginWebSockets`.

## Проверка и браузерные проекты

```sh
./examples/gallery/run check --only modules/web/forms
./examples/gallery/run check --only modules/web/websocket-chat
```

Постоянно работающие приложения с интерфейсом браузера находятся рядом: [блог](../../../cookbook/web/blog/README.md) и [книжный магазин](../../../cookbook/web/bookstore/README.md). Эти пять файлов показывают полный сетевой сценарий в одном запуске. [Общая галерея](../../README.md).
