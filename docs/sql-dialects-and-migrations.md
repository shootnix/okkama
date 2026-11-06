# SQL-диалекты и миграции текущего приложения

Одинаковая форма `INSERT ... ON CONFLICT DO NOTHING` работает в используемых
SQLite и PostgreSQL для ожидаемого повтора по уникальному ключу. Проверки CHECK,
NOT NULL и внешние ключи сохраняют ошибки. Приложение формирует эту SQL-строку
явно; исследовательский PG-адаптер передаёт Statement.text без переписывания
ключевых слов внутри строковых литералов.

Механический bridge связывает параметры отдельно. Его текущая грамматика
заменяет qmark вне single/double quotes, а literal percent удваивается для
psycopg внутри и вне этих кавычек. Это объявленная грамматика конкретного
адаптера; SQL-операторы/комментарии/quoted bodies требуют своих явных контрактов.

`std/db/sqlite/script.SqliteScript` выполняет SQL через настоящий SQLite parser,
с лимитами1MiB и256 statements и возможностью работы с concrete SqliteTransaction.
Portable Transaction предоставляет свой узкий набор операций. Backend-specific
script capability выбирается владельцем композиции; правила версии/backfill
остаются прикладными. Текущий runner использует доверенный schema marker format.

IMP-16 хранит версии providers,
происхождение SQL-фикстур и реальные проверки fresh/history/repeat/rollback.
Фаза R15 schema1→3 и финальная address/startup-фаза IMP30 описываются отдельно.
