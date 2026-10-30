# Changelog

Все заметные изменения Okkama. Формат — [Keep a Changelog](https://keepachangelog.com/ru/1.1.0/),
версии — [SemVer](https://semver.org/lang/ru/). Версия релиза — в `okkama-rs/Cargo.toml`
и в теге `vX.Y.Z`.

## [Unreleased]

## [0.2.0] — 2026-10-30

### Добавлено

- **Место и стек вызовов неперехваченной ошибки** на движках IR и Sparrow (включая
  JIT): строка исходника с подчёркиванием и стек от места отказа к `main`. Рекурсия
  схлопывается (`(×500)`), длинный стек показывается началом и концом.
  `sparrow` и `okkama exec` без исходников печатают стек в терминах Sparrow
  (функция, номер и текст инструкции). `OKKAMA_TRACE=off` — только сам отказ.
- **`debug/dumper`** — дамп любого значения по мотивам Data::Printer:
  `Dumper(value).type()`, `.kind()`, `.size()`, `.fields()`, `.all()`,
  `.max_depth(n)`, `.max_items(n)`. Показывает поля объектов (и приватные),
  элементы массивов и пары словарей; одинаковый текст на IR, Sparrow и JVM.
  Natives `runtime/debug`: `data_kind`, `data_type`, `data_size`, `data_leaf`,
  `data_names`, `data_keys`, `data_items`.
- **`okkama repl`** — интерактивные проверки на IR-интерпретаторе: инструкции,
  выражения (значение печатает `Dumper`), объявления и импорты; переменные живут
  между вводами, побочные эффекты прошлых вводов не повторяются. Команды
  `:vars`, `:decls`, `:reset`, `:help`, `:quit`.
- Пример `examples/debug-dumper.ok`.

### Изменено

- Спецификация §6.7: описан `debug/dumper`, который, в отличие от `Dump`,
  показывает поля объектов.
- OIR-модуль несёт отладочные сведения (`DebugInfo`): исходник каждой функции.
- `tools/engines/parity.py` сравнивает движки с `OKKAMA_TRACE=off`.

### Известные ограничения

- JVM пока печатает ошибку без места и стека.
- В REPL замыкания и Future между вводами не сохраняются; `await` на верхнем
  уровне ввода не поддерживается; истории строк нет.

## 0.1.0

Первая версия Rust-реализации: компилятор (лексер, парсер, анализ, OIR),
движки IR, Sparrow (VM с JIT) и JVM, стандартная библиотека, LSP, `fmt`, `lint`,
менеджер проектов `blade`, пакеты для macOS и Linux.

[Unreleased]: https://github.com/shootnix/okkama/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/shootnix/okkama/releases/tag/v0.2.0
