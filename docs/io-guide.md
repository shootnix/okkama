# Ввод-вывод Okkama: подробное руководство

Модули `io/*` задают маленькие контракты для потоков байтов и текста. Они ничего
не знают о путях, процессах или сокетах: файловая система и процессы лишь реализуют
те же роли. Благодаря этому буфер, стандартный ввод и вывод процесса можно
декорировать одинаково.

## 1. Байты и текст — разные слои

Главное правило API: байты нельзя неявно смешивать со строками.

- `Reading` и `Writing` работают с байтами;
- `ReadingText` и `WritingText` работают со строками;
- декодирование всегда имеет известную кодировку;
- конец байтового потока — пустой `@byte`;
- конец построчного текстового потока — `EndOfInput` после выдачи последней строки.

Это предотвращает зависимость от локали процесса, случайное двойное кодирование и
потерю произвольных бинарных данных.

## 2. Контракты `io/stream`

### `ByteSource`

```okkama
role ByteSource {
    bytes() is @byte;
}
```

Это конечный снимок байтов, который можно передать в `Writing.write`. Для массива
используйте `ByteSnapshot` из `io/decorators`. В выражении `<-` встроенные `string`
и `@byte` адаптируются к `ByteSource` контекстно; постоянного метода приведения на
базовых типах не появляется.

### `Writing`

```okkama
role Writing {
    write(source is ByteSource) is void;
    flush() is void;
}
```

`write` имеет all-or-error семантику: если назначение приняло не все байты,
возникает `IncompleteWrite`, чей `written()` сообщает количество принятых байтов.
`flush()` проталкивает накопленные данные нижележащему ресурсу.

### `Reading`

```okkama
role Reading {
    bytes(limit is int) is @byte;
}
```

`limit` — верхняя граница одного результата, а не требование заполнить весь массив.
Можно получить меньше байтов. Пустой массив означает устойчивый EOF. Неположительный
лимит бросает `InvalidReadLimit`.

Правильный цикл чтения:

```okkama
for {
    let chunk = input.bytes(65536);
    last if chunk.length() == 0;
    consume(chunk);
}
```

### Текстовые роли

`WritingText.write(...string)` записывает аргументы подряд, а `writeln(...string)`
после всех аргументов добавляет ровно один LF.

`ReadingText.line()` читает одну физическую строку, а `text()` — остаток целиком.
Конкретный адаптер определяет обработку CRLF и кодировку.

### `Closing`

```okkama
closed() is bool
close()
```

`close()` у стандартных реализаций идемпотентен. Закрытие удобно ставить в `defer`:

```okkama
let input = resource.input();
defer input.close();
```

## 3. `io/stdout`

`Stdout` и `Stderr` реализуют и байтовую запись, и текстовую запись.

```okkama
let output = Stdout();
output.write("progress: ");
output.writeln("done");
output.flush();
```

Потоковая форма сохраняется:

```okkama
Stdout() <- "answer = " <- 42.string() <- "\n";
```

Оператор `<-` левоассоциативен и вычисляет каждый операнд ровно один раз. Это
важно, если выражение справа имеет эффект.

`Stderr` используется так же:

```okkama
Stderr().writeln("configuration is invalid");
```

Нет псевдонимов `print`, `println`, `printf` или `writef`: один набор имён не даёт
несколько слегка отличающихся способов сделать одно и то же. Форматирование
строится отдельными объектами, после чего строка передаётся в `write`/`writeln`.

## 4. `io/stdin`

### Бинарное чтение

`Stdin` — обычный `Reading`:

```okkama
let input = Stdin();
for {
    let chunk = input.bytes(8192);
    last if chunk.length() == 0;
    digest.write(ByteSnapshot(origin: chunk));
}
```

### UTF-8 текст

`Utf8Input` адаптирует `Reading` к `ReadingText` и `Closing`:

```okkama
let input = Utf8Input(origin: Stdin());
defer input.close();

try {
    for {
        let line = input.line();
        process(line);
    }
}
catch error is EndOfInput {
    // Нормальное завершение.
}
```

`line()` удаляет LF и предшествующий CR. Последняя непустая строка без перевода
строки всё равно возвращается. Только следующий вызов бросает `EndOfInput`.

Если остаток гарантированно мал, можно прочитать его целиком:

```okkama
let document = input.text();
```

Не используйте `text()` для потенциально огромного или бесконечного входа.
Некорректный UTF-8 приводит к типизированной ошибке декодирования, закрытый адаптер
— к `ClosedInput`.

## 5. `io/buffer`

`ByteBuffer` хранит изменяемое состояние, но не сваливает чтение, запись, закрытие
и наблюдение содержимого в один гигантский объект. Он выдаёт узкие capability:

```okkama
let buffer = ByteBuffer();
let writing = buffer.writing();
let reading = buffer.reading();
let content = buffer.content();
let closing = buffer.closing();
defer closing.close();
```

Запись:

```okkama
writing.write(ByteSnapshot(origin: payload));
writing.flush();
```

Чтение:

```okkama
let first = reading.bytes(1024);
let second = reading.bytes(1024);
```

Наблюдение без сдвига позиции чтения:

```okkama
let snapshot = content.bytes();
let byte_count = content.size();
```

`BufferContent` отражает текущее содержимое. `bytes()` возвращает снимок, который
не изменится от последующей записи. После закрытия операции ресурса бросают
`ClosedBuffer`. Отдельного `reset()` нет: когда нужен новый жизненный цикл, создайте
новый `ByteBuffer`.

Текстовый буфер составляется, а не вводится отдельным типом: байтовый буфер плюс
UTF-8 адаптеры чтения/записи.

## 6. `io/decorators`

### `BufferedReading`

```okkama
let buffered = BufferedReading(origin: slow_input, capacity: 65536);
defer buffered.close();
let header = buffered.bytes(16);
```

Без `capacity` используется библиотечный размер. Буфер уменьшает число обращений к
нижнему источнику и никогда не возвращает больше запрошенного лимита. Закрытие
закрывает нижележащий ресурс. Неположительная ёмкость недопустима.

### `LimitedReading`

Ограничивает суммарное число выдаваемых байтов:

```okkama
let body = LimitedReading(origin: connection, limit: content_length);
```

После `limit` байтов возвращает устойчивый EOF, даже если у исходного потока есть
данные. Это удобно для фреймов, вложений и безопасных парсеров.

### `ChainedReading`

Последовательно читает несколько источников:

```okkama
let complete = ChainedReading(origins: parts);
let chunk = complete.bytes(4096);
```

Пустой источник пропускается. Один вызов может собрать байты из нескольких соседних
источников, но не превышает `size`.

### `ByteSnapshot`

Адаптирует `@byte` к `ByteSource` и копирует вход, чтобы последующие изменения
массива не меняли уже переданное значение:

```okkama
destination.write(ByteSnapshot(origin: bytes));
```

### `TeeWriting`

Копирует каждую запись во все назначения в порядке последовательности:

```okkama
let tee = TeeWriting(destinations: outputs);
tee.write(ByteSnapshot(origin: packet));
tee.flush();
```

Это не транзакция. Если третье назначение упало, первые два уже могли получить
данные. Ошибка не откатывает предыдущие записи.

### `Discarding`

Принимает и забывает байты:

```okkama
let sink = Discarding();
sink.write(ByteSnapshot(origin: payload));
```

Полезен в тестах, замерах и для явно ненужной ветки вывода.

## 7. Копирование потока без загрузки целиком

```okkama
copy(source is Reading, destination is Writing) is void {
    for {
        let chunk = source.bytes(65536);
        last if chunk.length() == 0;
        destination.write(ByteSnapshot(origin: chunk));
    }
    destination.flush();
}
```

Память этой операции ограничена размером порции плюс внутренними буферами. Она
подходит и для файла в 20 ГБ.

## 8. Ошибки и владение

Ожидаемые ошибки:

- `InvalidReadLimit` — лимит чтения не положителен;
- `IncompleteWrite` — нижний слой записал только часть значения;
- `EndOfInput` — текст полностью выдан;
- `ClosedInput`, `ClosedReading`, `ClosedBuffer` — операция после закрытия;
- `InvalidReading` — недопустимая настройка декоратора.

Объект, создавший ресурс, обычно отвечает за его закрытие. Декоратор с `Closing`
владеет своим origin; объект без `Closing` не должен неожиданно закрывать его.
Проверяйте конкретный контракт, а не предполагайте владение по названию.

## 9. Частые ошибки

1. Пустой байтовый результат — EOF, а не «попробуй ещё раз».
2. Не просите ноль байтов: это ошибка API.
3. Не декодируйте произвольные байты неявно.
4. Не используйте `ReadingText.text()` для неограниченного входа.
5. Не считайте `TeeWriting` атомарной транзакцией.
6. Не забывайте `flush()` перед моментом, когда данные обязаны быть видимы.
7. Закрывайте ресурсы через `defer`, если их жизненный цикл ограничен блоком.

## 10. Карта выбора

| Нужно | Используйте |
|---|---|
| читать байты | `Reading` |
| писать байты | `Writing` + `ByteSource` |
| читать UTF-8 stdin по строкам | `Utf8Input(Stdin())` |
| писать пользователю | `Stdout` |
| писать диагностику | `Stderr` |
| буфер в памяти | `ByteBuffer` и его capability |
| ограничить длину входа | `LimitedReading` |
| объединить входы | `ChainedReading` |
| уменьшить число чтений | `BufferedReading` |
| дублировать вывод | `TeeWriting` |
| отбросить вывод | `Discarding` |

## Текстовый адаптер v4

`io/text.Utf8Output(origin is Writing)` реализует `WritingText`: `write(...string)`
и `writeln(...string) is void`. Он пишет строгий UTF-8 через переданную capability,
не закрывает и не flush-ит её. Для памяти передайте `ByteBuffer().writing()`.
Файлы предоставляют эти роли через `File.binary()` и `File.text()`;
сам владелец `File` предоставляет закрытие. `encoding/pem.PemBlock` теперь
реализует `ByteSource.bytes()` и передаётся напрямую в `Writing.write()`.
