# Эффекты и чтение cap

Cap сохраняет связь с конкретным владельцем. Каждое чтение computed field заново
выполняет его тело. Оно может обратиться к объекту с эффектами или вернуть Future.
Cap сам по себе не обещает чистоту, авторизацию или отдельный снимок данных.

Если нужны две отдельные актуальные проверки, читайте поле дважды:

```okkama
let first=await view::observation;
let second=await view::observation;
```

Если нужен один результат для двух потребителей, явно сохраните один Future:

```okkama
let pending=view::observation;
let first=await pending;
let second=await pending;
```

В исполняемых примерах `workspace/billing/experiments/effects_http_cap.ok` и
`effects_http_single_future.ok` это соответственно два и один настоящий HTTP
request. Это не общий cache cap и не обещание об eager/lazy запуске Future,
который вообще не awaited. Ошибки и отмена сохраняют обычную семантику Future.

Для исторического значения скопируйте нужные поля в отдельный доменный объект.
`ContactSnapshot` в billing-кандидате читает tenant/id/name/email/address по одному
разу, валидирует эти же локальные копии и хранит строки. Копирование effectful
cap тоже является наблюдением; порядок и число чтений должны быть явными.

`BillingClock` задаёт форму `now() is int`, а не проверяемое обещание чистоты.
`LoggingClock` законно пишет marker при каждом вызове. В реальном `BillingService`
чтение `await storage.now()` уже вынесено в оркестрацию: вычислитель `Renewal`
получает `FixedClock` с конкретным значением. Не заменяйте эту границу догадкой,
что все реализации роли ведут себя как FixedClock.

R16 содержит исследовательский checker вручную подготовленного typed IR. Его
алгебра различает I/O, state, time, random, log, suspend, throw, divergence и
cancellation; неизвестный foreign primitive получает консервативный TOP.
Детерминированное частичное вычисление может бросить ошибку или не завершиться;
async сам по себе не означает внешний I/O. Checker не является частью stock
компилятора и не извлекает граф из исходников. Его повторно используемая копия
находится в `improvements-roadmap/workspace/effects/manual-ir`; происхождение
зафиксировано в BASELINE.json. IMP-26 отвечает за ограниченное source extraction.
Положительный результат на ручном IR не подтверждает соответствие реальному коду
и не устанавливает права доступа, финансовую атомарность или commit certainty.
