# Sparrow VM

Самостоятельный исполнитель `.spb`. Не зависит от компилятора Okkama,
IR, лексера, парсера, Java или исходников пользовательской программы.
Зависимости: `okkama-sparrow` (байткод/верификатор), `sparrow-runtime`
(значения, операции, планировщик, natives) и Cranelift (JIT).

## Отдельный бинарник

Из `okkama-rs`:

```sh
cargo build --offline --release -p sparrow-cli
./target/release/sparrow --help
./target/release/sparrow check out.spb
./target/release/sparrow out.spb
```

Компилятор создаёт файл отдельно:

```sh
cargo run --offline -p okkama-cli -- build examples/sparrow-control-flow.ok --target sparrow -o out.spb
cargo run --offline -p sparrow-cli -- out.spb
```

Пример возвращает 42 как код выхода. `run` проверяет SPB 9 и исполняет его в
текущем процессе с аргументами программы; `test` исполняет тесты файла,
собранного с `-t`; `check` проверяет без запуска (бесконечный цикл не мешает
проверке); `--assemble`/`--disassemble` переводят между SPB и текстом
ассемблера ([sparrow-isa.md](../../docs/sparrow-isa.md)). Загрузчик
ограничивает чтение 256 MiB + один контрольный байт. Неперехваченный отказ
программы — диагностика в stderr и код 1; ошибки формата — 1, аргументов — 2,
чтения — 3. Результат main обязан помещаться в 0..255.

## Встраивание

```rust,ignore
let program = okkama_sparrow::decode(&bytes)?;
let value = sparrow_vm::execute_with_arguments(&program, arguments)?;
println!("{value}");
```

`execute` проверяет модель и создаёт отдельный стек кадров на каждый запуск;
`run_tests` исполняет тестовый профиль, `report` даёт текст отказа как у JVM.
`Error::InvalidProgram` обозначает неверный байткод; `Error::Operation`
содержит функцию, индекс инструкции и ошибку операции. Ошибки не содержат типов IR
или фиктивных source spans. Отображение позиции в исходнике — будущий source map.

`okkama_runtime::sparrow` удалён: CLI обращается к `sparrow_vm` напрямую.
Результат — `sparrow_vm::IntValue` из независимого `sparrow-runtime`.
Его нельзя передавать как `ir::IntValue`. Ошибки и состояние планировщика
также независимы. CLI переводит только координаты стека для source-диагностики.

## Границы модулей

- `okkama-runtime-abi`: стабильные native ID, сигнатуры и NativeFailure.
- `sparrow-runtime`: значения, операции `ops`, кучи, регионы, отказы,
  планировщик, natives, intrinsics, тесты и воркеры Sparrow.
  Не зависит от формата программы или исполнителя.
- `okkama-sparrow`: модель программы, SPB codec, verifier и ассемблер; без движка.
- `sparrow-vm`: кадры, переходы, вызовы и исполнение инструкций.
- `sparrow-cli`: файлы, аргументы, вывод диагностики и process exit status.
- `okkama-runtime`: IR-интерпретатор и source-диагностика; использует своё `okkama-runtime-core`.

Значения — собственные `Value` Sparrow (`Rc` с итеративным освобождением); циклы
ссылок собирает сборщик циклов `sparrow_runtime::gc`. Строки,
объекты, successor, массивы, словари, Future, регионы, natives и intrinsics
исполняются через `sparrow-runtime`. Состояние stdin (буфер и EOF) принадлежит одному
запуску VM.

## JIT

Горячие функции (1000 вызовов или обратных переходов) компилируются в машинный
код Cranelift (`src/jit.rs`); вход — в начале функции или в цели обратного
перехода (OSR). Регистры — те же слоты `Value`, инструкции без быстрого пути
исполняет `evaluate` интерпретатора, поэтому поведение не меняется.
Функция больше 2^14 регистров или с машинным кадром больше 1 МиБ не
компилируется; вызов функции больше 512 регистров идёт через помощник.

| Переменная | Действие |
|---|---|
| `OKKAMA_JIT=off` | JIT выключен |
| `OKKAMA_JIT=eager` | каждая функция компилируется при первом вызове (режим проверки) |
| `OKKAMA_JIT_THRESHOLD=N` | порог горячести вместо 1000 (смешанное исполнение и OSR в проверках) |
| `OKKAMA_JIT_ONLY=a,b` | компилировать только эти функции (символы через запятую) |
| `OKKAMA_JIT_SKIP=a,b` | не компилировать эти функции |
| `OKKAMA_JIT_DUMP=a,b` | печатать CLIF этих функций |
| `OKKAMA_JIT_DEBUG=1` | журнал компиляции: время, размер CLIF, причины отказа |
| `OKKAMA_JIT_STATS=1` | в конце: скомпилировано, отказано, входов в машинный код, из них OSR; выходы в помощники по инструкциям |

`OKKAMA_JIT_ONLY`/`SKIP` нужны для бисекции: какая скомпилированная функция
меняет результат. Устройство — в
[sparrow-runtime-roadmap.md](../../docs/sparrow-runtime-roadmap.md#jit--машинный-код-sparrow-реализован-24102026).

## Проверки независимости

```sh
cargo tree --offline -p sparrow-cli
cargo test --offline -p sparrow-cli -p sparrow-vm -p sparrow-runtime
python3 tools/check-sparrow-boundary.py
```

В дереве нет `okkama-compiler`, `okkama-runtime` и `okkama-runtime-core`. Тесты отдельного бинарника
создают байткод напрямую и запускают VM с пустым PATH. Тесты движка проверяют
SPB 9, глубокую рекурсию (200 000 кадров), цикл SPB → ASM → SPB, обработчики
отказов, точные целые за пределами 128 бит, ошибки и горячий код (OSR,
переполнение в машинном коде, обработчики в скомпилированной функции). Общая регрессия Okkama дополнительно
сравнивает исполнение IR и этой VM через совместимый адаптер.
