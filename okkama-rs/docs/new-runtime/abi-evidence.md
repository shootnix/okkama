# ABI → проверки

Сверка 2026-11-03 по текущему `crates/sparrow-engine`, не по статусам старого
inventory. Контракт: [abi.md](abi.md), порядок: [execution-order.md](execution-order.md).
Пути в таблице относительно `crates/sparrow-engine`.

`PASS текущий` означает точечный прогон в текущей серии работы на этом хосте.
`Тест найден` означает только найденное покрытие; не заменяет прочтение тела,
проверку безопасного запуска и результат. Наличие одного теста не закрывает фазу.

| Граница / инвариант | Реализация | Проверка | Доказательство / остаток |
|---|---|---|---|
| SPB decode → verified plan → cold/JIT | `lib.rs`, `validate.rs` | `tests/bootstrap.rs::decoded_spb_has_matching_cold_and_native_results` | PASS текущий, обе ветки, min/max i64 |
| Неподдержанные операнды и неверные регистры отклоняются | `validate.rs` | `tests/bootstrap.rs::malformed_and_unsupported_programs_do_not_fall_back` | PASS текущий; не полный набор негативных verifier-кейсов |
| C entry, Slot payload/tag, PC/fuel/result | `execution.rs::RunState`, `jit.rs::Compiled` | `tests/jit_regions.rs::region_edges_preserve_parallel_moves_big_tags_and_exact_instruction_budgets` | PASS текущий; layout использует repr(C)/offset_of; другие архитектуры NOT_RUN |
| Статус 11 REGION_TRANSFER не означает host yield | `execution.rs::resume_inner`, `jit.rs` | тот же `region_edges_preserve_parallel_moves_big_tags_and_exact_instruction_budgets` | PASS текущий, сравнение cold/JIT по каждому instruction budget |
| Отказ JIT — terminal engine error, без fallback | `jit.rs::Compiled::new`, `Task::resume` | `execution.rs::parked_task_tests::jit_compile_failure_is_terminal_without_fallback_and_releases_dead_values` | PASS текущий: отказ регионов 0/64, повторный resume, GC, новая сессия; реальные allocator/OS failures не инъецировались |
| CodeMemory живёт до конца сессии | `jit.rs::CodeMemory`, Code ownership | `tests/futures.rs::session_keeps_machine_code_alive_after_the_executable_handle_is_dropped` | PASS текущий; цикл ограничен 128 шагами, неожиданное Waiting завершает тест ошибкой |
| В continuation сохраняются только живые значения | `execution.rs`, `liveness.rs` | `tests/calls.rs::ordinary_call_saves_only_live_caller_values` | PASS текущий; 1000 регистров, 1 сохранённое значение |
| Аргументы читаются до переиспользования окна | `dispatch_call`, scratch arguments | `tests/calls.rs::tail_call_arguments_are_captured_before_frame_reuse` | PASS текущий; конечный тест |
| Spill только живых значений при yield | `liveness.rs`, `jit.rs` | `tests/bootstrap.rs::only_live_values_cross_a_suspension_boundary` | PASS текущий; конечный тест |
| Старое значение dst живо на exceptional edge | exceptional liveness, unwind | `tests/failures.rs::thrown_payload_unwinds_flat_calls_and_preserves_old_result_register` | PASS текущий; helper ограничен 4096 VM-инструкциями |
| Handler-only значения spill перед trap | JIT guard, handler transfer | `tests/failures.rs::native_unbound_guard_spills_handler_only_values` | PASS текущий; тот же ограниченный helper |
| Cleanup continuation хранит primary и roots | `resource.rs`, `execution.rs::Cleanup` | `tests/resources.rs::unwind_preserves_primary_failure_while_running_every_destroy`, `destroy_is_lifo_and_its_state_survives_yields_and_gc` | Тесты найдены; полная семантика вложенных ресурсов остаётся фазой 5 |
| Native output публикуется однократно, ошибка кэшируется | `native_io.rs::native_io_settle` | `native_io.rs::tests::publication_error_is_cached_as_a_terminal_language_failure`, `heap_can_collect_during_io_and_publishes_completion_only_once` | Исторический точечный прогон описан в roadmap; не полный аудит провайдеров |
| Отмена до/после публикации; cleanup secondary | `native_io.rs`, `execution.rs` | `cancellation_before_publication_discards_output_and_runs_cleanup_once`, `cancellation_after_publication_preserves_value_without_cleanup`, `native_cancel_waits_for_cleanup_once_and_preserves_secondary_failure` | Тесты найдены; реальные OS workers требуют изоляции |
| Pending/wakeup и уведомление всех waiters | native Request и Future watch | `native_io.rs::tests::completion_wakes_each_registered_waiter_once` | Тест найден; длительные wakeup/cancel stress не выполнялись |
| VM fuel и cooperative Tokio budget | `async_host.rs::drive_task` | `tests/async_host.rs::ready_spawn_transitions_yield_without_ending_eager_startup` | Ранее проверенный конечный тест; один долгий native-вызов budget не прерывает |
| Нет зависимости от старого runtime | Cargo production graph | `tools/check-sparrow-boundary.py` (относительно okkama-rs) | PASS текущий; анализ графа не заменяет весь аудит launcher/fallback |

## Оставшиеся обязательства

- Владение CodeMemory подтверждено конечным тестом. Другие ещё не запускавшиеся
  error/lifetime проверки проверять на конечность до их запуска.
- Отдельно сверить типизированные/native/async переходы и все категории roots.
  Не закрывать общий ABI-пункт только по числовому каркасу.
- Поддерживаемые платформы и реальные ошибки executable-memory allocation/
  finalization требуют отдельного подтверждения. Ошибки Cranelift передаются
  наружу по коду; этот факт слабее проверки каждого реального OS failure mode.
- Полная матрица инструкций и контрактов фазы 0, все native lifecycle и
  изолированные workers остаются открытыми независимо от этой таблицы.

## Статические проверки

`CARGO_BUILD_JOBS=1 cargo clippy --offline -p sparrow-engine --lib --tests --no-deps -- -D warnings`
прошёл. Вариант без `--no-deps` остановился на существующем `clippy::type_complexity`
в `crates/okkama-sparrow/src/verify.rs:264`. Зависимость не изменялась ради этой
проверки; полного чистого Clippy для workspace не заявлено.

## Дополнительная серия точечных проверок

Вызов для каждой строки: `CARGO_BUILD_JOBS=1 cargo test --offline -p sparrow-engine
--test <target> <name> -- --exact --test-threads=1`, последовательно.
Все перечисленные проверки прошли:

| Target | Name |
|---|---|
| calls | ordinary_call_saves_only_live_caller_values |
| calls | tail_call_arguments_are_captured_before_frame_reuse |
| bootstrap | only_live_values_cross_a_suspension_boundary |
| futures | session_keeps_machine_code_alive_after_the_executable_handle_is_dropped |
| failures | thrown_payload_unwinds_flat_calls_and_preserves_old_result_register |
| failures | native_unbound_guard_spills_handler_only_values |
| failures | rethrow_reaches_outer_handler_with_the_same_payload |
| failures | real_compiler_try_catch_fixture_executes_without_legacy_runtime |
| failures | compiler_resource_cleanup_throws_into_the_surrounding_catch |
| failures | compiler_defer_failure_precedes_destroy_and_returned_resource_escapes |
| failures | caught_errors_do_not_evaluate_their_text_method |
| failures | unreachable_is_supported_as_a_terminator_but_is_not_a_catchable_failure |

Дополнительно `--lib suspended_task_roots_survive_peer_collection_and_drop_releases_them
-- --test-threads=1`: PASS (0.03s), cold/JIT, peer GC, окончание и освобождение root-set.
Итого в этой дополнительной серии 13 PASS. Общий helper восьми перечисленных
failure-тестов ограничен 4096 VM-инструкциями; все восемь проверены после изменения.
Машинное время одной native-инструкции этим не ограничивается; эти сценарии
не содержат внешних native workers. Production runtime в этой серии не менялся.

## Проверки retained roots

Следующая серия: 7 PASS, также последовательно, offline, один build job:

| Target | Name | Что подтверждено |
|---|---|---|
| calls | closure_captures_survive_source_changes_calls_and_collection | Вычисленный bigint жив через capture и сохранённый caller; 500 небольших аллокаций; тест ограничен 4096 шагами |
| resources | destroy_is_lifo_and_its_state_survives_yields_and_gc | Cleanup cursor/region и outer frame сохраняют данные при GC |
| resources | cleanup_continues_after_throw_and_reports_the_first_failure | Cleanup продолжается после ошибки |
| resources | unwind_preserves_primary_failure_while_running_every_destroy | Primary failure сохраняется при destroy и GC |
| lib | failure_publication_retains_native_domain_code_and_operation | Только Ready Future удерживает native failure и строки; после удаления корня failure/payload освобождаются |
| lib | caught_failure_keeps_payload_and_descendants_alive | Thrown payload и его потомки сохраняются; после удаления failure-root освобождаются |
| lib | causes_and_secondary_failures_preserve_identity_and_trace_cycles | Цепочки causes/secondary и циклы сохраняют идентичность и собираются |

Resource helper ограничен 4096 VM-инструкциями, все три его потребителя проверены.
Это небольшие in-process проверки, не длительный memory/stress и не приёмка
всех native handles. JSON/native-handle arenas и полная async-root матрица
остаются отдельными обязательствами. Изменения этой серии затрагивают только тесты.
