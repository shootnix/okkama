# Статические диагностики Okkama

Десять маленьких программ, в каждой — одна намеренная ошибка. Они помогают увидеть, какие контракты анализатор проверяет до запуска main. Проверяющий скрипт сопоставляет фактическое сообщение с ожидаемым фрагментом из catalog.json.

```sh
./examples/gallery/run check --analysis-only
```

| Файл | Проверяемый контракт | Ожидаемый фрагмент сообщения |
|---|---|---|
| [discarded-successor.ok](discarded-successor.ok) | Потерянный результат действия-преемника | `Successor action result must be used` |
| [duplicate-action.ok](duplicate-action.ok) | Два действия с одинаковыми параметрами | `overloads must declare the same return type` |
| [duplicate-field.ok](duplicate-field.ok) | Повторное объявление поля | `already declared` |
| [duplicate-object.ok](duplicate-object.ok) | Повторное объявление объекта | `already declared` |
| [duplicate-role.ok](duplicate-role.ok) | Повторное объявление роли | `already declared` |
| [generic-arity.ok](generic-arity.ok) | Неверное число аргументов generic-типа | `expects 1 type argument` |
| [invalid-main.ok](invalid-main.ok) | Неверная сигнатура точки входа | `Entry point` |
| [role-conformance.ok](role-conformance.ok) | Неполная реализация роли | `does not implement action` |
| [role-signature.ok](role-signature.ok) | Несовместимая сигнатура реализации роли | `must return int` |
| [unknown-type.ok](unknown-type.ok) | Необъявленный тип поля | `undeclared type` |

Основные программы галереи служат примерами корректного кода. Например, результат successor сохраняется в новом связывании, объекты явно реализуют свои роли, а `main` имеет типизированную сигнатуру `main() is int`.

Результаты всех файлов, включая корректные помощники: [analysis-checks.json](../analysis-checks.json). Этот набор — Okkama-адаптация идеи [диагностической галереи Rakupp](https://github.com/ash/rakupp/tree/main/examples/lint). [Общий каталог](../README.md).
