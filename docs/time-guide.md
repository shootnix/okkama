# Время и даты Okkama: подробное руководство

Модули `time/*` разделяют физическую длительность, абсолютный момент, монотонное
измерение, календарную дату, локальные часы и часовой пояс. Это не архитектурная
церемония: у этих величин разные правила арифметики.

## 1. Карта понятий

| Тип | Что означает | Пример |
|---|---|---|
| `Duration` | точное количество секунд | 1500 мс |
| `Instant` | точка глобальной временной шкалы | timestamp события |
| `MonotonicInstant` | показание часов для измерения интервалов | deadline |
| `Date` | календарная дата | 2026-08-11 |
| `TimeOfDay` | время суток | 14:30:00 |
| `LocalDateTime` | дата + часы без зоны | 2026-08-11 14:30 |
| `TimeZone` | правила offset/DST | Europe/Berlin |
| `ZonedDateTime` | instant, наблюдаемый в зоне | тот же момент в Berlin |

Главное следствие: `LocalDateTime` сам по себе не является `Instant`. Во время
осеннего перевода часов одно локальное время может соответствовать двум instant, а
во время весеннего — ни одному.

## 2. `time/duration`

### Точное создание

```okkama
let delay = Duration(value: rat(3, 2), unit: DurationUnit.second);
let day = Duration(value: 1.rat(), unit: DurationUnit.day);
```

Единицы: nanosecond, microsecond, millisecond, second, minute, hour, day.
`day` здесь ровно 86 400 секунд, а не календарный день в часовой зоне.

`Duration` хранит точное рациональное количество секунд с наносекундным разрешением:

```okkama
delay.seconds();               // rat(3, 2)
delay.nanoseconds().string();  // "1500000000"
```

Дробная наносекунда запрещена, округления нет. Ноль и отрицательные длительности
допустимы как значения; конкретная операция вроде `wait` может вводить более узкое
ограничение.

### Арифметика

```okkama
let total = first.plus(second);
let delta = total.minus(first);
let doubled is Duration = value ScaledDuration(
    origin: first,
    factor: 2.rat(),
);
```

Методы возвращают новые значения. Сравнение идёт через `Comparing<Duration>`.

Каноническая lossless-строка:

```okkama
let text = DurationString(origin: delay).string(); // "3/2s"
```

Примитивы намеренно не имеют `5.seconds()`: stdlib-тип создаётся явным
конструктором и не загрязняет базовый runtime API.

Ошибочная точность бросает `InvalidDuration`.

## 3. `time/clock`

### `Instant`

`Instant` — абсолютная точка, представленная длительностью от Unix epoch:

```okkama
let now = SystemClock().instant();
let later = now.plus(delay);
let elapsed = later.minus(now);
```

`unix_duration()` нужен сериализаторам и time-zone преобразованиям. Для пользовательской
строки выбирайте `Rfc3339`, а не печатайте внутренние секунды.

### `MonotonicInstant`

Монотонные часы предназначены для интервалов:

```okkama
let clock = SystemClock();
let started = clock.monotonic_instant();
await clock.wait(delay);
let elapsed = clock.monotonic_instant().minus(started);
```

Они не прыгают из-за синхронизации системных часов и не конвертируются в civil time.
Их `reading()` имеет смысл только внутри той же временной шкалы runtime-а.

Используйте:

- `Instant` — timestamp, журнал, обмен с внешней системой;
- `MonotonicInstant` — timeout, benchmark, retry deadline.

### `Timing`

```okkama
role Timing {
    instant() is Instant;
    monotonic_instant() is MonotonicInstant;
    async wait(duration is Duration) is Timing;
}
```

Принимайте роль в бизнес-объектах вместо создания `SystemClock` внутри. Тогда тест
может передать детерминированные часы.

```okkama
object Retrying {
    has clock is Timing;

    Retrying(clock is Timing) { self.clock = clock; }

    async run() is Retrying {
        await clock.wait(Duration(
            value: 100.rat(),
            unit: DurationUnit.millisecond,
        ));
        self;
    }
}
```

Недопустимое ожидание бросает `InvalidWait`. Ожидание является cancellation point.

## 4. `time/date`

```okkama
let date = Date(year: 2026, month: 8, day: 11);
date.year();
date.month();
date.day();
date.weekday(); // Weekday.tuesday
```

`Weekday` идёт от monday до sunday. Конструктор проверяет реальность даты, включая
високосные годы и диапазон лет 1..9999; ошибка — `InvalidDate`.

### Сдвиг и расстояние

```okkama
let tomorrow is Date = value ShiftedDate(origin: date, days: 1);
let count is int = value DaysBetween(left: date, right: tomorrow);
```

`ShiftedDate` выполняет календарный сдвиг на целые даты. `DaysBetween(left,right)`
положителен, если `right` позже `left`.

Внутренние `DateOrdinal` и `OrdinalDate` обеспечивают точную арифметику; обычному
коду чаще нужны `ShiftedDate` и `DaysBetween`.

## 5. `time/day`

```okkama
let time = TimeOfDay(
    hour: 14,
    minute: 30,
    second: 45,
    nanosecond: 125000000,
);
```

Диапазоны:

- hour: 0..23;
- minute: 0..59;
- second: 0..59;
- nanosecond: 0..999999999.

Leap second `23:59:60` не моделируется. Некорректное значение бросает
`InvalidTimeOfDay`. Сравнение лексикографическое по полям.

`TimeOfDay` не знает дату и зону. Нельзя спросить у него timestamp или DST.

## 6. `time/local_datetime`

```okkama
let local = LocalDateTime(
    date: Date(year: 2026, month: 8, day: 11),
    time: TimeOfDay(hour: 23, minute: 30, second: 0, nanosecond: 0),
);
```

`date()` и `time()` возвращают составные значения. `plus(duration)` переносит
через границы суток:

```okkama
let shifted = local.plus(Duration(
    value: 2.rat(),
    unit: DurationUnit.hour,
)); // 2026-08-12 01:30
```

Но эта арифметика считает каждый local day равным 86 400 секундам. Она не знает о
DST. Разность двух `LocalDateTime` имеет ту же naive-семантику.

Если пользователь просит «через два реальных часа», сначала работайте с `Instant`.
Если просит «завтра в то же местное время», сдвигайте `Date`, затем снова разрешайте
локальное время через `TimeZone`.

## 7. `time/zone`

### Именованная зона

```okkama
let zone is TimeZone = Zone(name: "America/New_York");
```

`Zone` использует только закреплённую IANA tzdata, сейчас 2026c. Она не читает
настройку зоны или tzdata хоста. Это обеспечивает одинаковый результат на Perl,
Rust и VM runtime.

```okkama
let version = TimeZoneDatabaseVersion().string();
```

Версию полезно сохранять в диагностике воспроизводимого расчёта.

### Фиксированный offset

```okkama
let utc_plus_five is TimeZone = FixedZone(
    name: "UTC+05",
    offset: Duration(value: 5.rat(), unit: DurationUnit.hour),
);
```

У `FixedZone` нет DST; `daylight_saving` всегда false, а abbreviation равна имени.
Она подходит для протокольного offset, но не заменяет реальную географическую зону.

### Из instant в локальное время

Это преобразование однозначно:

```okkama
let observed = ZonedDateTime(instant: now, zone: zone);
let local = observed.local_datetime();
let offset = observed.offset();
let metadata = ZoneObservation(origin: observed);
Stdout().writeln(metadata.abbreviation());
if metadata.daylight_saving() {
    mark_summer_time();
}
```

### Из локального времени в instant

Это преобразование может быть неоднозначно:

```okkama
let instant is Instant = value InstantFromLocal(
    origin: local,
    zone: zone,
    fold: Fold.earlier,
);
```

`Fold.earlier` и `Fold.later` выбирают один из двух instant при осеннем повторении
часов. Во время весеннего gap локального времени не существует, и возникает
`MissingLocalTime`. Не «исправляйте» gap молча сдвигом на час: такая политика должна
быть явным решением приложения.

`UnknownTimeZone` означает неизвестное IANA-имя; `InvalidTimeZone` — нарушение
инварианта зоны или недопустимый диапазон.

## 8. `time/iso`

Этот модуль — строгие представления календарной даты и local date-time без offset.

```okkama
let text = IsoDate(origin: date).string(); // "2026-08-11"
let parsed is Date = value DateFromIso(origin: "2026-08-11");
```

```okkama
let text = IsoLocalDateTime(origin: local).string();
let parsed is LocalDateTime = value LocalDateTimeFromIso(
    origin: "2026-08-11T12:30:45.125",
);
```

Профиль строг:

- дата ровно `YYYY-MM-DD`;
- local datetime требует uppercase `T` и секунды;
- fraction имеет 1..9 цифр;
- вывод убирает нулевую fraction и конечные нули;
- whitespace, offset, zone name, lowercase `t`, сокращённая точность и expanded
  years отвергаются.

Ошибки: `InvalidIsoDate`, `InvalidIsoLocalDateTime`.

Offset-bearing timestamp относится к `time/rfc3339`, а не к этому модулю.

## 9. `time/rfc3339`

### Форматирование instant

```okkama
let utc = Duration(value: 0.rat(), unit: DurationUnit.second);
let text = Rfc3339(origin: now, offset: utc).string();
// Например: 2026-08-11T12:30:45.125Z
```

Offset задаёт желаемое представление того же instant. Он должен быть целым числом
минут с модулем меньше 24 часов. Нулевой offset канонически печатается `Z`.

### Парсинг

```okkama
let instant is Instant = value InstantFromRfc3339(
    origin: "2026-08-11T12:30:45.125+05:00",
);
```

Требуются uppercase `T`, секунды и `Z` либо `±HH:MM`. Fraction — до 9 цифр и не
округляется. RFC `-00:00` принимается как нулевое смещение с неизвестным
происхождением offset; после превращения в `Instant` эта provenance не хранится.

Неверный timestamp бросает `InvalidRfc3339`, неверный offset —
`InvalidRfc3339Offset`.

## 10. `time/format`

Модуль предназначен для явного нестандартного формата без ambient locale.

```okkama
let text = FormattedDate(
    origin: date,
    pattern: "%d/%m/%Y",
).string();

let parsed is Date = value DateFromFormat(
    origin: "11/08/2026",
    pattern: "%d/%m/%Y",
);
```

Local date-time:

```okkama
let text = FormattedLocalDateTime(
    origin: local,
    pattern: "%Y-%m-%d %H:%M:%S.%f",
).string();

let parsed is LocalDateTime = value LocalDateTimeFromFormat(
    origin: "2026-08-11 14:30:45.125000000",
    pattern: "%Y-%m-%d %H:%M:%S.%f",
);
```

Директивы:

| Директива | Значение | Ширина |
|---|---|---:|
| `%Y` | год | 4 |
| `%m` | месяц | 2 |
| `%d` | день | 2 |
| `%H` | час | 2 |
| `%M` | минута | 2 |
| `%S` | секунда | 2 |
| `%f` | наносекунда | 9 |
| `%%` | символ `%` | 1 |

Каждый другой Unicode scalar — литерал. Имена месяцев/дней и locale-зависимые
директивы отсутствуют, пока locale не станет явным значением. Date pattern запрещает
time-директивы. Парсер требует все поля даты; local datetime также требует hour,
minute, second, а nanosecond по умолчанию ноль.

Ошибки паттерна и входа несут offset: `InvalidTimePattern`,
`InvalidFormattedDate`, `InvalidFormattedLocalDateTime`.

Для обменного формата предпочитайте ISO/RFC 3339. `time/format` нужен для заранее
согласованного пользовательского или legacy-представления.

## 11. Практические рецепты

### Измерить операцию

```okkama
let clock = SystemClock();
let started = clock.monotonic_instant();
await operation();
let elapsed = clock.monotonic_instant().minus(started);
Stdout().writeln(DurationString(origin: elapsed).string());
```

### Сохранить timestamp события

```okkama
let occurred = clock.instant();
let encoded = Rfc3339(
    origin: occurred,
    offset: Duration(value: 0.rat(), unit: DurationUnit.second),
).string();
```

Хранение в UTC исключает неоднозначность; нужная зона применяется при показе.

### Показать instant пользователю в его зоне

```okkama
let view = ZonedDateTime(
    instant: occurred,
    zone: Zone(name: user.time_zone()),
);
let text = FormattedLocalDateTime(
    origin: view.local_datetime(),
    pattern: "%d/%m/%Y %H:%M:%S",
).string();
Stdout().writeln(text, " ", ZoneObservation(origin: view).abbreviation());
```

### «Завтра в 09:00»

```okkama
let tomorrow is Date = value ShiftedDate(origin: today, days: 1);
let local = LocalDateTime(
    date: tomorrow,
    time: TimeOfDay(hour: 9, minute: 0, second: 0, nanosecond: 0),
);
let scheduled is Instant = value InstantFromLocal(
    origin: local,
    zone: user_zone,
    fold: Fold.earlier,
);
```

Приложение должно решить, что делать с `MissingLocalTime` в DST gap.

### Deadline для timeout

```okkama
let deadline = clock.monotonic_instant().plus(limit);
let result = await Before(
    origin: operation,
    deadline: deadline,
    clock: clock,
).result();
```

Не вычисляйте deadline от civil `Instant`: системные часы могут быть скорректированы.

## 12. Частые ошибки

1. Не считайте `Duration(day)` календарным днём в зоне.
2. Не превращайте `LocalDateTime` в `Instant` без `TimeZone` и политики fold.
3. Не используйте `Instant` для измерения timeout — берите monotonic time.
4. Не преобразуйте `MonotonicInstant` в дату.
5. Не читайте timezone/locale из ambient state.
6. Не используйте `FixedZone` вместо IANA zone для будущих гражданских дат.
7. Не игнорируйте `MissingLocalTime` и неоднозначный fold.
8. Не используйте custom format как обменный, если подходит RFC 3339.
9. Не округляйте дробные наносекунды молча.

## 13. Карта выбора

| Нужно | Используйте |
|---|---|
| точный интервал | `Duration` |
| масштабировать интервал | `ScaledDuration` |
| timestamp сейчас | `Timing.instant()` |
| измерить время / deadline | `Timing.monotonic_instant()` |
| подождать | `Timing.wait()` |
| календарная дата | `Date` |
| время суток | `TimeOfDay` |
| дата и часы без зоны | `LocalDateTime` |
| IANA zone | `Zone` |
| постоянный offset | `FixedZone` |
| показать instant в зоне | `ZonedDateTime` |
| разрешить local в instant | `InstantFromLocal` |
| ISO-дата/local datetime | `time/iso` |
| timestamp с offset | `time/rfc3339` |
| явный custom pattern | `time/format` |
