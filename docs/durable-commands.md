# Durable команды billing-кандидата

`application/intent.CommandTuple` кодирует поля как `длина:значение`, считая
Unicode-символы. Значения и их порядок сохраняются. Выбор полей является частью
конкретной команды; входной порядок JSON members на результат не влияет.

```okkama
import "application/intent" take CommandTuple;
let payload=CommandTuple().canonical(["c1","1","Accounts","billing@example.test"]);
```

Это модуль приложения из `improvements-roadmap/workspace/billing/app`, а не новый
глобальный формат stdlib. Один tuple с пустым полем отличается от пустого tuple:
`[""]` даёт `0:`, `[]` даёт пустую строку. Явный тип промежуточного результата
`map` позволяет обработать пустой input без вызова callback.

Существующие renewal/upgrade receipts используют прежние разделители. Например,
issue для `s1`/version1 остаётся `2:s1:1`; смена на другой красивый формат сломала
бы replay. `BillingService` теперь использует общий prefix encoder, сохраняя
этот layout. Новая версия проверена на записях, созданных отдельным процессом
с исходниками до IMP-10, включая Unicode и delimiter characters.

Durable scope задаёт SQL store: tenant, credential scope, operation и key.
Business result, replay record и audit фиксируются атомарно. Повтор того же
намерения возвращает прежний receipt; изменение payload при прежнем ключе даёт
409. Сам encoder не авторизует Principal и не выполняет commit.

Потеря ответа на COMMIT оставляет неопределённый исход. Если authoritative replay
с прежним ключом уже нашёл committed record, приложение может вернуть известный
receipt. Если такого подтверждения нет, API возвращает 503 problem details с
`detail: "commit outcome unknown"`. Отсутствие записи само по себе не объявляется
доказанным rollback. Сохраняйте исходную команду и ключ для reconciliation; не
подменяйте intent новой версией объекта или новым provider key.

Проверка IMP-10 использует локальные SQL-команды, два Okkama API процесса,
конкурирующие дубли и before/after COMMIT failpoints. Платёжные provider fixtures
остаются пустыми; это не обещание exactly-once для произвольного внешнего сервиса
и не автоматический retry вокруг provider HTTP.
