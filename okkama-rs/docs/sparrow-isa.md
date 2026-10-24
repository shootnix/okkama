# Sparrow: формат SPB 9 и набор инструкций

Sparrow — регистровая VM Okkama. Путь компиляции такой же, как у JVM:
`source → AST → OIR-1 → Sparrow (asm) → байткод SPB`. Генератор
(`okkama-compiler/src/codegen/sparrow`) переводит проверенный OIR в модель
`okkama_sparrow::Program`; VM (`sparrow-vm`) исполняет её над общим
рантаймом `okkama-runtime-core` — значения, операции (`ops`), планировщик
задач, регионы, natives, intrinsics и протокол воркеров те же, что у
исполнителя IR (как JVM-байткод вызывает `okkama.rt`). Компилятор,
исходники и Java для запуска не нужны.

```sh
okkama build app.ok --target sparrow -o app.spb      # -t — тестовый профиль
sparrow app.spb [ARGS...]                            # или okkama exec app.spb
sparrow test app.spb
sparrow --disassemble app.spb -o app.spa
sparrow --assemble app.spa -o app.spb
okkama run --engine sparrow app.ok [ARGS...]         # -t — тесты
```

## Программа

Файл несёт всю программу с импортированной std: таблицы типов, enum,
ролей (с сигнатурами методов), объектов (поля, свидетельства ролей,
`destroy`, системные объекты), native objects, caps, функций, точку входа и
список тестов. Индексы таблиц позиционные: `t3`, `enum0`, `role1`, `obj2`,
`cap0`, `f5`.

Функция: символ, признаки `async`, `native "символ"` или
`intrinsic "имя" [объекты] [функции]` (у них нет кода), типы регистров,
регистры параметров, тип результата (`void`), код и таблица обработчиков.

- Значение SSA `%n` — регистр `rn` кадра; кадр — регистры и номер инструкции.
- Блоки раскладываются подряд. Переход `@pc(rA <- rB, …)` — к инструкции с
  параллельной записью параметров блока: сначала читаются все источники.
- Значение, которое OIR передаёт последним параметром цели, пишет сама
  инструкция: пойманное значение `catch`, элемент `arr.lookup`/`map.lookup`,
  отказ в регистр обработчика.
- `handler start end @pc(…) rF` — цель отказа любой инструкции блока
  `[start, end)`, включая его терминатор (`unwind` OIR).
- Вызовы интерпретатора — собственный стек кадров VM, без рекурсии Rust;
  горячие функции исполняет машинный код (JIT) на стеке задачи, а без запаса
  стека — снова интерпретатор. Задачи `async` —
  сопрограммы планировщика (холодный Future, `await`, `fut.spawn`).
- `region.leave`/`region.unwind` вызывают `destroy` идентичностей по одному
  кадром-границей и исполняются заново после каждого.

## Инструкции

Порядок операндов — порядок полей; первый — назначение (`r` или `_`, если
результата нет). Полный список — `Instruction::MNEMONICS`.

| Группа | Инструкции |
|---|---|
| Значения | `const` (`int`, `bool`, `byte`, `float` биты, `char` код, `str`), `move`, `undef`, `any.box`, `cvt` (цель — тип назначения) |
| `int` | `int.add sub mul mod and or xor shl shr ushr neg not abs`, `int.cmp`, `sign`, `bits`, `byte.from` |
| `byte`, `char`, `bool` | `byte.and or xor shl shr ushr not cmp`, `char.cmp`, `bool.not cmp` |
| `float`, `rat` | `float.add sub mul div mod pow neg abs sqrt cmp`, `rat.new add sub mul div mod neg abs cmp part` |
| Строки | `str.concat cmp len slice split replace replace_first trim chomp utf8` |
| Объекты, роли | `obj.new init get next`, `role.wrap narrow`, `val.cmp`, `enum.case cmp`, `range.new part` |
| Коллекции | `arr.new len get slice join`, `builder.new push append range finish`, `map.new len has get keys values`, `mbuilder.new put finish` |
| Вызовы | `call`, `fn.closure`, `call.fn`, `call.role`, `call.cap`, `await`, `fut.spawn`, `fut cancel|done|cancelled`, `worker.spawn`, `worker.bind` |
| Регионы, отказы | `region.enter adopt transfer track leave unwind`, `fail.secondary caused` |
| Переходы | `jmp`, `br`, `ret`, `panic`, `trap`, `unreachable`, `throw`, `raise`, `catch`, `arr.lookup`, `map.lookup` |

## Кодировка SPB 9

Магия `SPRW`, версия u16 big-endian (9), затем строки модуля, таблицы и
функции. Числа — беззнаковый LEB128 (каноническая запись), строки — длина и
UTF-8, инструкция — код операции и операнды по полям. Кодирование
детерминировано; декодер отвергает лишние байты, другие версии и
неканонические числа и проверяет программу. Файлы SPB 1–8 не читаются:
программу нужно пересобрать.

## Проверка

До исполнения проверяются ссылки всех таблиц, регистры, цели переходов,
обработчики (упорядочены, не пересекаются), отсутствие кода у native и
intrinsic, завершённость кода (нет выхода за конец) и что каждый читаемый
регистр записан на всех путях к чтению (анализ потока данных, с учётом
записей на рёбрах и обработчиков). Типы операндов проверяет исполнение:
несоответствие — ошибка состояния VM, как у исполнителя IR.

## Ассемблер

`disassemble` печатает программу так, что `assemble` восстанавливает её
точно (цикл SPB → ASM → SPB сохраняет байты). Комментарии — `;` до конца
строки. Номера инструкций `N:` проверяются при разборе.

```text
function f1 "factorial" {
    registers [t0, t0, t0, t0, t0]
    params [r0]
    result t0
    0: const r1, int 1
    1: int.cmp le, r2, r0, r1
    2: br r2, @3, @4
    3: ret r1
    4: int.sub r3, r0, r1
    5: call r4, f1, [r3]
    6: int.mul r4, r0, r4
    7: ret r4
}
```
