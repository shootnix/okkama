Да. И тут, по-моему, есть действительно интересная цель: не сделать язык, на котором можно писать хороший код, а сделать язык, на котором существенно труднее написать архитектурно плохой код.

Но одно ограничение сразу: никакой язык не заставит человека или модель правильно выбрать границы Invoice, Order, Payment, дать нормальные имена и понять предметную область. То есть «каждая программа априори лучше Rust» в абсолютном смысле недостижимо. Зато можно сделать очень большой класс типичного промышленного говнокода невыразимым или заметным компилятору.

И для эпохи AI это, пожалуй, даже важнее, чем раньше. Модель прекрасно генерирует много локально правдоподобного кода. Проблема начинается через 500 тысяч строк, когда появляются скрытые зависимости, дублирование правил, десяток способов сделать одну вещь и магические обходы архитектуры.

Я бы строил Okkama вокруг четырёх свойств:

локальность рассуждения → явность зависимостей → единственный нормальный способ выразить вещь → автоматическая защита архитектурных границ.

У вас большая часть первого слоя уже есть. Но я вижу ещё несколько очень сильных ограничений, которые стоит исследовать.

⸻

1. Никакого ambient state вообще

Это, возможно, самое полезное ограничение для промышленного кода.

Запретить непосредственно из произвольного кода:

global variables
environment variables
current time
random
filesystem
network
process environment
system properties
thread locals
global configuration
service locator
singleton registry

Не потому что ими нельзя пользоваться, а потому что они доступны только через объект-capability.

Например, не:

let now = Clock.now();

а:

object Expiration {
    has clock is Clock;
    expired(deadline is Time) is bool {
        clock.now().after(deadline);
    }
}

Аналогично:

FileSystem
Random
Environment
Database
Output
Clock

передаются обычным объектным путём.

Получаем мощнейшее свойство:

Глядя на состояние объекта и аргументы action, можно понять, от чего его поведение вообще способно зависеть.

Для AI-кода это огромный выигрыш. Модель не сможет через пять уровней внезапно полезть в environment или глобальный singleton.

⸻

2. Constructors и fixup сделать гарантированно чистыми

Это я бы очень серьёзно рассмотрел.

Сейчас философски мы и так не хотим:

User(...) {
    database.write(...);
    http.get(...);
}

Можно сделать это невозможным.

Конструктор:

только связывает состояние

fixup:

только проверяет состояние

Никаких I/O, clock, random, spawn, database, файлов.

То есть:

object User {
    has email is Email;
    User(:email);
    fixup {
        confirm email.valid();
    }
}

Если создание требует внешнего мира:

UserLoading(repository).user(id)

или другой объект.

Это означает:

Конструирование объекта никогда не является скрытой операцией над системой.

Очень сильная гарантия.

⸻

3. Effectful code должен быть виден

Вот здесь, возможно, у нас ещё не хватает целого измерения языка.

Сейчас:

invoice.total()
file.content()
clock.now()
repository.commit(...)

синтаксически всё actions.

Но первые могут быть чистыми, остальные зависят от внешнего мира.

Я бы не обязательно вводил тяжёлую effect system как в академических языках, но позволил бы компилятору знать:

pure
filesystem
network
clock
random
process
storage

Причём effects можно выводить автоматически из capability graph, а не заставлять программиста писать:

foo() effects io, fs, network

Например:

Invoice.payment()
    effects: none
Payments.execute()
    effects: storage
HttpGateway.request()
    effects: network

А потом важнейшее правило:

Effect-free object не может неожиданно начать иметь effect из-за изменения глубоко внутри call tree, не изменив свой публичный effect contract.

Это уже защита от одной из главных болезней больших систем.

Ты меняешь:

price() is Money

и внутри кто-то решил сходить в базу.

Компилятор говорит:

price() changed observable effect contract:
none -> storage

Вот это чрезвычайно полезно.

⸻

4. Domain-модули можно сделать действительно pure

Это развитие предыдущего пункта.

Не надо хардкодить Clean Architecture в язык словами domain/application/infrastructure.

Но можно позволить:

module billing/domain v1.0.0 pure;

И тогда внутри него невозможно импортировать:

io
network
clock
random
process
mutable capabilities

Можно только работать со значениями и доменными объектами.

Получается реально enforceable dependency rule:

domain
   ↑
application
   ↑
infrastructure

Не потому что архитектор написал красивую картинку Confluence.

Компилятор физически не даёт domain уйти наружу.

Для миллионной кодовой базы — очень серьёзная вещь.

⸻

5. Запретить cyclic module dependencies

Я бы вообще не позволял циклы на уровне модулей.

billing → users → notifications → billing

Нет.

Компилятор:

Module dependency cycle:
billing → users → notifications → billing

Исправляйте архитектуру.

В небольшом коде cycle бывает удобен. В огромном коде он постепенно превращает систему в кашу.

Если Okkama ориентирован именно на большие системы, module graph должен быть DAG.

Это также сильно помогает AI: любой агент работает в понятном направлении зависимостей.

⸻

6. Public API должен быть существенно строже private implementation

Вот это очень интересная идея.

Внутри action можно пользоваться:

int
string
bool

Но в публичном API доменного модуля compiler/linter может становиться гораздо строже.

Например, считать подозрительным:

charge(
    user is string,
    amount is int,
    currency is string
)

и предпочитать:

charge(
    user is UserId,
    amount is Money
)

Я не уверен, что надо запретить primitives в public API вообще — это может стать идиотизмом.

Но отдельный industrial profile мог бы требовать semantic types.

Например:

warning/error:
Public action contains three primitive parameters.
Consider domain objects.

Для AI это прекрасно: модель не будет бесконечно плодить stringly typed API.

⸻

7. Bool blindness тоже можно ограничить

Очень типичный API:

create(true, false, true)

или:

send(message, urgent is bool, retry is bool)

В большой системе это дерьмо.

Можно ввести правило:

bool разрешён как результат predicate, локальное состояние и внутреннее поле, но публичные boolean configuration arguments требуют named argument или enum.

То есть допустимо:

send(
    message,
    urgent: true,
);

но лучше:

send(
    message,
    priority: Priority.high,
);

Я бы это делал industrial lint, не частью type system.

⸻

8. Публичные actions с большим количеством параметров — архитектурная ошибка

Например:

create(
    name,
    surname,
    street,
    city,
    zip,
    country,
    phone,
    email,
    age
)

Это почти наверняка означает отсутствие объекта.

Можно дать compiler policy:

Public action has 9 parameters.
Maximum recommended: 4.

И аналогично constructor.

Опять же — не ядро языка, а strict profile.

Очень важная мысль:

Не все ограничения должны быть grammar/type-system restrictions.

Я бы сделал:

language rules
+
industrial compiler profile

Например:

okkama --profile industrial

И CI не может пройти с архитектурными warning’ами.

⸻

9. Не позволять «мешки данных» технически

Вот здесь можно пойти дальше обычного EO.

Например, compiler может видеть объект:

object User {
    has name is string;
    has age is int;
    has email is string;
    name() is string { name; }
    age() is int { age; }
    email() is string { email; }
}

Это практически DTO.

Можно делать diagnostic:

Object User exposes 100% of its stored state without meaningful behavior.
Possible data bag.

Это эвристика, поэтому не язык.

Но в industrial profile — чрезвычайно полезно.

То же самое:

Manager
Service
Helper
Util
Processor
Handler

не запрещать по имени, но анализировать behavior/state ratio.

Compiler архитектурно знает про objects гораздо больше, чем обычный linter Java.

Этим надо пользоваться.

⸻

10. Запретить «глухие» wrappers

AI особенно любит такое:

object PaymentService {
    has repository is Repository;
    pay(payment is Payment) is void {
        repository.pay(payment);
    }
}

Потом:

PaymentManager
PaymentProcessor
PaymentHandler
PaymentService
PaymentFacade

и пять слоёв passthrough.

Можно анализировать delegation:

Action only forwards all arguments unchanged and adds neither policy nor abstraction.

Один decorator — нормально.

Пять бессмысленных forwarding layers — diagnostic.

Опять: industrial analyzer.

⸻

11. Сделать architectural dependency policy частью проекта

Вот это мне особенно нравится.

Например:

architecture.ok

условно:

layer domain {
    modules billing/domain, users/domain;
    allow domain;
    forbid effects;
}
layer application {
    allow domain;
}
layer infrastructure {
    allow application, domain;
}

Не обязательно такой синтаксис.

Главное:

CI знает архитектуру

и агент не может случайно сделать:

billing/domain
    imports
billing/postgresql

Это не договорённость, это ошибка сборки.

Для больших AI-generated codebases это может оказаться killer feature.

⸻

12. API versioning встроить в компилятор

У вас уже есть:

module billing v1.2.0;

Из этого можно сделать не декоративную строку.

Compiler может хранить public API предыдущей версии.

Если модель поменяла:

pay(Payment) is Receipt

на:

pay(Payment, Context) is Receipt

но оставила:

v1.2.1

получаем:

Breaking public API change requires major version increment.

И наоборот:

public API compatible

Для системы в миллион строк это очень мощная защита от AI-рефакторингов.

⸻

13. Ограниченная метапрограммируемость — практически никакой

Вот тут я бы держался нынешней линии.

Никаких:

runtime reflection
method_missing
monkey patch
open classes
eval
dynamic proxy magic
annotation processors doing arbitrary code
implicit DI magic
operator overloading by users
automatic serialization by field reflection

Потому что всё это уничтожает главное свойство:

Код локально означает то, что написано.

AI особенно любит магию, когда она доступна: «удобно» сгенерировал framework внутри framework.

В Okkama желательно просто не дать этого сделать.

⸻

14. Один canonical way

Это, возможно, вообще главный принцип AI-era language.

Если одну вещь можно написать пятнадцатью способами, модель будет использовать все пятнадцать.

Например, не иметь одновременно:

anonymous action syntax A
lambda syntax B
closure syntax C
method reference syntax D

Один способ.

Не:

for
foreach
map as statement
iterator loop
stream loop

без необходимости.

Не:

cast A
as
to
.convert()

Один mechanism.

Это не просто эстетика.

На миллионной кодовой базе уменьшается энтропия representation.

То есть два разработчика и три AI-агента скорее напишут похожий код.

Это чудовищно ценно.

⸻

15. Errors: никаких catch-all и никаких игнорируемых failures

У вас это уже частично есть, но я бы дожал.

Например:

catch error is Error

возможно, слишком широко для domain/application code.

Можно industrial profile заставить ловить конкретные типы:

catch conflict is VersionConflict
catch storage is StorageFailure

А catch Error разрешать только:

application boundary
logging boundary
test runner
main

То же самое с ignored Error.

То есть:

try {
   ...
}
catch error is Error {
}

компилятор просто запрещает.

AI очень любит «обработать» ошибку её уничтожением.

⸻

16. Resource objects сделать affine

Это ещё одна область, где Rust силён, но можно взять только нужную часть без borrow checker на весь язык.

Для обычных immutable objects aliases нормальны.

Но для:

File
Socket
Transaction
Lock
Process

можно пометить runtime/library type как resource capability.

Для них:

один owning scope
destroy ровно один раз
нельзя сохранить куда угодно
нельзя случайно скопировать ownership

То есть affine только resources, а не весь object world.

Получаем большую часть полезной resource-safety Rust без того, чтобы заставлять бизнес-код бороться с lifetime parameters.

⸻

17. Determinism by default

Очень недооценённая промышленная штука.

Например:

Map iteration order
hash randomization
parallel reductions
serialization order
float formatting
timezone
locale

Язык/stdlib должны давать deterministic semantics там, где это разумно.

AI-code + nondeterminism = ад для диагностики.

Например:

map.items()

имеет определённый порядок или требует явного:

map.unordered()

а не зависит от реализации JVM/Perl/Rust.

⸻

18. Contracts как first-class часть API

Вот это я бы тоже исследовал.

Сейчас:

payment(amount is Money) is ^Invoice {
    confirm amount.positive() else ...
}

Контракт спрятан внутри тела.

Можно представить:

payment(amount is Money) is ^Invoice
    requires amount.positive()
    ensures result.paid() >= self.paid()
{
    ...
}

Я не предлагаю сейчас немедленно добавлять этот синтаксис, но концептуально это мощно.

Публичный action содержит:

signature
preconditions
postconditions
effects
failure types

Тогда compiler/IDE/AI видит контракт без чтения implementation.

Для миллионов строк — очень сильная штука.

Можно даже начать без formal verification: contracts выполняются runtime/test и экспортируются metadata.

⸻

19. Закрытый набор failure types у публичного API

Аналог checked exceptions, только без Java-кошмара.

Например:

payment(...) is ^Invoice
    throws CurrencyMismatch, Overpayment, InvalidState

Не для каждого внутреннего helper.

Но public module boundary может иметь явный failure contract.

Тогда изменение:

теперь payment может кинуть NetworkFailure

невозможно тихо протащить наружу.

Опять же это делает evolution контролируемой.

⸻

20. Самая радикальная идея: запретить effectful domain successors

Например:

payment(...) is ^Invoice

по определению обязан быть pure.

То есть successor action:

не может читать файл
не может network
не может clock
не может random

Он только:

receiver state + arguments → successor

Это сделало бы domain state transitions реально математически детерминированными.

Если нужен clock:

let now = clock.now();
let revised = subscription.expired(now);

А не:

subscription.expire_if_needed();

где внутри спрятано текущее время.

Вот это я считаю очень сильным кандидатом для Okkama.

⸻

Если всё собрать, получается довольно интересная «конституция»

Я бы разделил её на три уровня.

Непоколебимые правила языка

* immutable published objects;
* successor для изменения доменного состояния;
* no null;
* no inheritance;
* no runtime reflection;
* private fields;
* explicit dependencies;
* explicit result types;
* typed failures;
* exhaustive/strict control constructs;
* no implicit conversions;
* deterministic semantics;
* affine resource ownership;
* constructors/fixup без внешних effects;
* no ambient capabilities.

Проверяемые архитектурные свойства

* module graph без циклов;
* pure modules;
* effect boundaries;
* semantic version/API compatibility;
* declared public failure/effect contracts;
* architecture dependency rules.

Industrial profile / compiler quality rules

* data-bag detection;
* useless wrappers;
* primitive obsession;
* excessive parameters;
* boolean blindness;
* huge actions;
* excessive nesting;
* catch-all;
* ignored errors;
* overly broad public API;
* duplicated domain predicates.

И вот последний слой особенно важен для AI.

Потому что не нужно делать язык уродливым только для того, чтобы запретить каждый плохой стиль грамматикой. Можно сделать compiler достаточно умным, чтобы CI говорил:

This is valid Okkama,
but not acceptable industrial Okkama.

Например:

O301 Domain object exposes 5/5 fields through trivial accessors.
O317 Public action accepts 7 primitive parameters.
O422 Module billing/domain acquired storage effect.
O511 Public API changed without major version increment.
O604 Error is caught and discarded.

И build падает.

Вот где, по-моему, появляется настоящий смысл Okkama в эпоху Codex/Claude

Не:

«AI сможет писать Okkama лучше, чем Rust».

А:

AI получает гораздо более узкое пространство допустимых решений, а компилятор не позволяет ему постепенно размыть архитектуру проекта.

Это уже очень сильная гипотеза.

И я бы именно её проверял следующим экспериментом: дать агенту 20 последовательных изменений одного проекта и не говорить ему «пиши красиво». Только требования. После каждого изменения смотреть, во что превращается архитектура Go и во что превращается архитектура Okkama.

Если ограничения языка реально работают, разница должна проявляться не на первой версии, а где-нибудь на пятнадцатой. Вот это был бы гораздо более показательный trial, чем ещё один маленький CRUD.