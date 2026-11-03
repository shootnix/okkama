# Sparrow runtime

**Дальнейшая разработка:** [roadmap нового Sparrow с нуля](../../docs/sparrow-new-runtime-roadmap.md).
Текущий crate — выделенная копия старого ядра; его наличие не означает, что новый runtime реализован.

Самостоятельная реализация значений, heap/GC, регионов, планировщика,
native-адаптеров и intrinsics для `sparrow-vm`. Исходная копия
`okkama-runtime-core` выделена на ветке `nom`; дальнейшие изменения независимы.
Нет re-export, include исходников или зависимости от IR runtime.

Первый шаг сохраняет прежние Rc, сборщик циклов, регионы и destroy.
Это архитектурное разделение, не оптимизация и не новая модель памяти.
Диагностические features перенесены отдельно: для эксперимента Sparrow
использовать `sparrow-runtime/no-regions-bench`, `sparrow-runtime/sqlite-inline-bench`
и `sparrow-runtime/allocation-scopes`, а не features IR-runtime.

Общими остаются `okkama-runtime-abi` (описания natives), `okkama-native`
и сторонние библиотеки. Native-адаптеры, которые принимают Value, скопированы
сюда: они пока не являются общей независимой прослойкой. Статические таблицы
Unicode/IDNA/tzdb всё ещё включаются из каталога JVM ресурсов компилятора
при сборке; Cargo-зависимости от компилятора нет. Это оставшаяся связь с
раскладкой исходников, не с исполнением IR.

`okkama` включает два runtime; отдельный бинарник `sparrow` — только этот.
Ошибки и состояния задач между runtime не передаются. Изменение копии
требует тестов Sparrow; исправления общей семантики при необходимости
переносятся в оба runtime явно.

Из `okkama-rs`:

```sh
python3 tools/check-sparrow-boundary.py
RUST_MIN_STACK=16777216 cargo test --offline -p sparrow-runtime -p sparrow-vm -p sparrow-cli
cargo build --offline --release -p sparrow-cli
```
