# Native ABI

Общий контракт provider-вызовов для генератора, загрузчика и обоих исполнителей.
Crate не зависит от компилятора, IR, VM, runtime-значений или внешних библиотек.

`Native` связывает канонический символ `модуль@major::имя/арность` со стабильным
wire ID и сигнатурой. Существующие ID нельзя переименовывать или переиспользовать.
Неизвестный символ отвергается при генерации/выполнении IR; неизвестный ID —
при чтении SPB. Декларация исходника не может подменить сигнатуру provider.

| ID | Символ | Параметры | Результат |
|---|---|---|---|
| 1 | runtime/utf@1::utf8_encode/1 | string | @byte |
| 2 | runtime/utf@1::utf8_decode/1 | @byte | string |
| 3 | runtime/utf@1::utf8_valid_prefix/1 | @byte | int |
| 4 | runtime/utf@1::utf16_encode/2 | string, string | @byte |
| 5 | runtime/utf@1::utf16_decode/2 | @byte, string | string |
| 6 | runtime/utf@1::utf16_valid_prefix/2 | @byte, string | int |
| 7 | runtime/utf@1::utf32_encode/2 | string, string | @byte |
| 8 | runtime/utf@1::utf32_decode/2 | @byte, string | string |
| 9 | runtime/utf@1::utf32_valid_prefix/2 | @byte, string | int |
| 10 | runtime/stdio@1::stdout_write/1 | @byte | int |
| 11 | runtime/stdio@1::stderr_write/1 | @byte | int |
| 12 | runtime/stdio@1::stdin_read/1 | int | @byte |
| 13 | runtime/stdio@1::stdin_read_line/0 | — | @byte |

`NativeFailure` сохраняет четыре поля языка: domain, code, operation, message.
Это ожидаемый отказ provider, не повреждение VM. Реализация provider находится
в `okkama-runtime-core::native`. Перехват отказа кодом Okkama потребует unwind;
сейчас оба исполнителя возвращают структурированный отказ Rust-хосту.

Stdio передаёт сырые байты, возвращает число принятых байтов и не добавляет
переводы строк. Ошибки I/O используют domain `core.stdio` и portable code.
Чтение ограничено положительным точным limit и порцией до 65536 байтов;
неположительный limit возвращает `invalid_input`. Read-line сохраняет LF/CR
и делит вход по LF, включая последнюю строку без LF. Контекст запуска хранит
остаток буфера и стабильный EOF, общий для обоих способов чтения.
