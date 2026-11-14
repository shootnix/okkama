# Общая поддержка Sparrow и компилятора

`okkama-runtime-core` не зависит от компилятора, OIR, байткода или VM.
После удаления IR-движка (#584) здесь остаются:

- `native`, `data`, `host`, `stdout`: провайдеры std и контекст запуска,
  доступные Sparrow через `host/native_origin.rs`, а также общие операции ОС.
- `sched`: промисы native-провайдеров и `pump`, который вызывает мост Sparrow.
  Стековые корутины IR, `run_main`/`run_test` и зависимость `corosensei` удалены.
  Владение объектами программы, GC, async-инструкции и process workers реализует
  `sparrow-core`; IR-регионы, intrinsic Host и сборщик циклов удалены.
- `Value`, `heap`, `failure`, `range`: аргументы и результаты native bridge,
  handles провайдеров и структурированные отказы. Глубокие цепочки значений
  освобождаются итеративно.
- `IntValue`, `bignum`, `integer`, `rat`, `float`, `scalar`, `strings`,
  `testing`: точная арифметика и общие контракты Sparrow и native-провайдеров.
- `ops`: вычисление констант компилятора (`analysis/check/consts.rs`);
  `budget`: общий counting allocator и выбор области учёта памяти Sparrow.

`okkama_compiler::ir::IntValue` остаётся реэкспортом. Это поддержка промежуточного
представления компилятора, а не отдельный движок исполнения.

Типы моста сохраняют `Rc` для значений и opaque handles. Состояние промисов
и контекст провайдеров сбрасываются на границе запуска Sparrow. Его собственная
куча и GC живут в `sparrow-core`.
