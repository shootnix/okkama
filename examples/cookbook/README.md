# Okkama: учебные приложения и лаборатории stdlib

Набор запускаемых программ к [книге стандартной библиотеки](../../docs/Okkama-Stdlib-RU.md). Здесь 10 прикладных программ, два самостоятельных веб-проекта и 158 лабораторий по рецептам книги. В каждом `.ok` есть вводная документация: задача, использование и результат.

Алгоритмические задачи, фракталы, интерпретаторы и сетевые сценарии по каталогу Rakupp собраны отдельно: [галерея Okkama](../gallery/README.md).

## Начало работы

Команды ниже выполняются из корня репозитория. Launcher выбирает `std` из текущего checkout и вызывает существующий `okkama-pl/okkama` (Perl 5.42). Для скриптов проверки и построения карты используется Python 3.9+ из стандартной поставки; сами приложения написаны на Okkama.

```sh
./examples/cookbook/run help
./examples/cookbook/run payroll demo
./examples/cookbook/run formats demo toml
./examples/cookbook/run tests
```

| Программа | Практическая задача | Код |
|---|---|---|
| `mail` | Чтение локального почтового ящика и создание MIME-писем | [mail.ok](apps/mail.ok) |
| `taxi` | Учёт водителей, поездок и дневной выручки | [taxi.ok](apps/taxi.ok) |
| `payroll` | История смен, расчёт оплаты и отчёты | [payroll.ok](apps/payroll.ok) |
| `formats` | Преобразование складских данных JSON ↔ CSV ↔ TOML | [formats.ok](apps/formats.ok) |
| `docs` | Локальное веб-приложение документации Okkama | [docs.ok](apps/docs.ok) |
| `bookstore` | CLI каталога, покупки и истории заказов | [bookstore.ok](apps/bookstore.ok) |
| `blog` | CLI публикаций и чтения блога | [blog.ok](apps/blog.ok) |
| `chat` | Браузерный чат с WebSocket-рассылкой | [chat.ok](apps/chat.ok) |
| `chat-client` | Терминальный клиент того же чата | [chat-client.ok](apps/chat-client.ok) |
| `tests` | Собственный набор тестов, свойств, doubles и отчётов | [tests.ok](apps/tests.ok) |

Самостоятельные веб-приложения находятся отдельно: [книжный магазин](web/bookstore/README.md) и [блог](web/blog/README.md). У каждого свои `main.ok`, `model.ok`, `pages.ok`, launcher и README. Модели общие с соответствующим CLI, поэтому терминал и браузер работают с одними данными.

## Почтовый клиент

Прочитайте письмо или mbox, создайте своё письмо и снова откройте его:

```sh
./examples/cookbook/run mail read examples/cookbook/data/mail.eml
./examples/cookbook/run mail inbox examples/cookbook/data/inbox.mbox
./examples/cookbook/run mail compose /tmp/hello.eml reader@example.test 'Привет!' 'Завтра встречаемся в 10:00.'
./examples/cookbook/run mail read /tmp/hello.eml
```

Работают модули `mail/message`, `mail/mbox`, MIME, encoded-word, UTF-8 и файловый IO. Заголовки и тело разбираются отдельно; при создании письма используются CRLF и UTF-8 с transfer encoding `8bit`. Выход — локальный `.eml` или читаемый текст. Рецепты сетевой почты находятся в [лабораториях](LABS.md).

## Клиент базы таксопарка

```sh
./examples/cookbook/run taxi init /tmp/taxi.sqlite
./examples/cookbook/run taxi driver-add /tmp/taxi.sqlite 'Ада'
./examples/cookbook/run taxi trip-add /tmp/taxi.sqlite 1 2026-10-18 30 1200
./examples/cookbook/run taxi report /tmp/taxi.sqlite
./examples/cookbook/run taxi report /tmp/taxi.sqlite 2026-10-18
```

В новой базе первый водитель получает `id=1`. Поездка длится 30 минут и приносит 1200 центов. Отчёт JSON группируется по водителю: число поездок, минуты, выручка. SQLite проверяет связь поездки с водителем; SQL передаёт параметры отдельно. `RowReader` и `QueryJson` превращают результат запроса в предметные значения и JSON.

## Расчёт зарплаты и история смен

```sh
./examples/cookbook/run payroll demo
./examples/cookbook/run payroll init /tmp/shifts.csv
./examples/cookbook/run payroll add /tmp/shifts.csv 'Ада' 2026-10-18 480 2500
./examples/cookbook/run payroll add /tmp/shifts.csv 'Ада' 2026-10-19 90 2500
./examples/cookbook/run payroll report /tmp/shifts.csv
./examples/cookbook/run payroll report /tmp/shifts.csv 'Ада' 2026-10-19
./examples/cookbook/run payroll daily /tmp/shifts.csv
./examples/cookbook/run payroll history /tmp/shifts.csv
```

CSV-схема: `employee,day,minutes,rate_cents`. Ставка указана в центах за час: 480 минут при ставке 2500 дают 20000 центов. Каждая смена считается точно через `BigInt`; дробные центы округляются half-even, затем суммы складываются. Фильтры отчёта — сотрудник и ISO-дата; пустая строка вместо сотрудника выбирает всех за день. История показывает исходный журнал. Пример: `report FILE '' 2026-10-18`.

`daily` группирует отчёты по датам. `report` возвращает минуты работы, целые `hours` с `remaining_minutes` и `amount_cents` как десятичную строку, сохраняя точность больших сумм. `init` создаёт новый журнал и отклоняет уже существующий файл; `add` дописывает смену после проверки её полей.

## JSON, CSV и TOML

```sh
./examples/cookbook/run formats demo json
./examples/cookbook/run formats demo csv
./examples/cookbook/run formats demo toml
./examples/cookbook/run formats convert csv json examples/cookbook/data/stock.csv
./examples/cookbook/run formats convert json toml examples/cookbook/data/stock.json > /tmp/stock.toml
./examples/cookbook/run formats convert toml csv /tmp/stock.toml
```

Общая модель — `StockItem(name,quantity)`, количество от 0 до 1000000. JSON содержит массив объектов, CSV — колонки `name,quantity`, TOML — два массива `names` и `quantities` одинаковой длины. Все девять сочетаний входного и выходного формата проходят проверку обратного преобразования. Конвертер печатает результат в stdout; файлом назначения управляет вызывающая оболочка.

## Веб-приложение документации

```sh
./examples/cookbook/run docs 8080
```

Откройте [документацию](http://127.0.0.1:8080/). Маршруты `/language`, `/stdlib`, `/cookbook` показывают соответствующие локальные Markdown-документы как экранированный текст страницами по 8192 символа с переходами «Назад» и «Далее» (`?page=0`, `?page=1`, …). Подходящие модули: `web/app`, `web/routing`, `web/reply`, `html/entities`, файловый IO.

## Книжный магазин и блог в браузере

Каждый проект запускается самостоятельно и создаёт таблицы автоматически:

```sh
./examples/cookbook/web/bookstore/run /tmp/books.sqlite 8081
```

Откройте [магазин](http://127.0.0.1:8081/): каталог, остатки, кнопка покупки, история заказов. Подробности API и транзакции: [README магазина](web/bookstore/README.md).

В другом терминале:

```sh
./examples/cookbook/web/blog/run /tmp/blog.sqlite 8082
```

Откройте [блог](http://127.0.0.1:8082/): лента, форма публикации, отдельная страница записи. Подробности: [README блога](web/blog/README.md).

Те же данные доступны CLI:

```sh
./examples/cookbook/run bookstore books /tmp/books.sqlite
./examples/cookbook/run bookstore buy /tmp/books.sqlite 1 2
./examples/cookbook/run bookstore orders /tmp/books.sqlite
./examples/cookbook/run blog publish /tmp/blog.sqlite 'Первая запись' 'Привет, Okkama!'
./examples/cookbook/run blog list /tmp/blog.sqlite
```

Для создания базы только из CLI есть `bookstore init DB` и `blog init DB`. Режимы `bookstore serve DB [PORT]` и `blog serve DB [PORT]` также доступны для уже созданной базы.

## WebSocket-чат

```sh
./examples/cookbook/run chat 8083
```

Откройте [чат](http://127.0.0.1:8083/) в двух вкладках. Сообщения рассылаются всем подключённым участникам, включая отправителя. Можно подключиться из терминала:

```sh
./examples/cookbook/run chat-client 8083 'Ада: привет из терминала'
```

Клиент получает приветствие, отправляет сообщение, читает ответ и завершает соединение. Сервер использует подпротокол `okkama-chat`, проверяет Origin, ограничивает сообщение 2000 символами. Команды комнаты проходят через `Channel<RoomCommand>`; каждый переход возвращает следующую неизменяемую версию `RoomState`. Жизнью фоновых задач владеет `FutureGroup`.

Все веб-серверы слушают `127.0.0.1`, печатают `PORT N`, принимают `0` для автоматического выбора порта. Завершение из терминала — `Ctrl+C`.

## Тестовый набор и отчёты

```sh
./examples/cookbook/run tests
./examples/cookbook/run tests --report=json
./examples/cookbook/run tests --report=tap
./examples/cookbook/run tests --report=junit
```

Восемь тестов показывают табличные случаи округления, property testing с фиксированным seed, roundtrip форматов, persistent generic-накопление, запись вызовов, matching ошибок, управляемые часы и Human/TAP/JSON/JUnit-отчёты. Это готовый пример построения тестового набора на встроенном test runner и stdlib.

## Все модули и лаборатории

[LABS.md](LABS.md) содержит 158 отдельных программ из книги: задача, команда, входные условия и ссылка на объяснение. Например:

```sh
./examples/cookbook/run lab recipe-lists
./examples/cookbook/run lab recipe-db-row-read
```

[Карта использования](COVERAGE.md) связывает все 506 модулей `std` с реальными импортами приложений и лабораторий, включая транзитивные пути через внутренние и runtime-модули. Это карта зависимостей исходников. Выполненные прикладные проверки отдельно фиксируются в [checks.json](checks.json), анализ всех точек входа — в [source-checks.json](source-checks.json).

## Проверка набора

```sh
./examples/cookbook/run verify
./examples/cookbook/run coverage
perl examples/cookbook/script/check_sources.pl
```

Последняя команда требует Perl 5.42. `verify` создаёт временные файлы и базы, запускает собственные серверы на свободных loopback-портах и проверяет CLI, SQLite, HTTP и WebSocket. Серверные процессы завершаются после проверок. Лаборатории с интерактивными входами и внешними сервисами запускаются отдельно по своим условиям.

## Общие помощники

- [arguments.ok](support/arguments.ok) — аргументы командной строки и проверка чисел.
- [files.ok](support/files.ok) — ограниченное чтение, запись и добавление в файл.
- [http.ok](support/http.ok) — запуск локального сервера и общий HTML-каркас.
- [sql.ok](support/sql.ok) — типизированный SQL → JSON и persistent `Collected<T>`.
- [chat_room.ok](support/chat_room.ok) — команды и неизменяемые состояния комнаты.
