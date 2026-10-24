# Покрытие синтаксиса

Таблица связывает каждую конструкцию актуального синтаксиса с источником
правила, реализацией и тестами. По ней можно проверить полноту: у каждой
строки есть код и хотя бы один тест.

Обозначения источников:

- **G** — `docs/okkama-grammar-1.0.md` (продукция);
- **S** — `docs/okkama-spec-11072026.md` (раздел);
- **P** — метод `okkama-pl/lib/Okkama/Parser.pm` / `Lexer.pm`;
- **D-NNN** — решение владельца языка в
  `conformance/contracts/DISCREPANCY-LEDGER.md`.

Пути реализации даны относительно `crates/okkama-compiler/src/syntax/`, пути
тестов — относительно `crates/okkama-compiler/` (`lexer/tests.rs` лежит в
`src/syntax/`). Кроме перечисленных тестов, каждую конструкцию проверяют ещё
три вещи:

- фикстура `tests/fixtures/all-syntax.ok`, которая содержит все виды
  AST-узлов (`printer::the_fixture_exercises_every_node_kind`);
- корпус `std/`, `examples/` и `conformance/cases` (`tests/corpus.rs`);
- дифференциальное сравнение с Perl ([compatibility.md](compatibility.md)).

## Лексика

| Конструкция | Источник правила | Реализация | Тесты |
|---|---|---|---|
| Идентификаторы `[A-Za-z_][A-Za-z0-9_]*`, только ASCII | G `BINDING_NAME`/`IDENT`/`TYPE_NAME`; S §4.1; P `Lexer::identifier` | `lexer::Lexer::word`, `token::is_word` | `lexer/tests.rs`: `contextual_and_former_words_are_identifiers`, `unexpected_characters_are_reported_and_skipped` |
| Ключевые слова (точное написание) | S §4.1; P `%KEYWORDS` | `token::TokenKind::keyword` | `lexer/tests.rs::active_keywords`, `token::tests::keywords_are_exact_spellings` |
| Удалённые и отложенные слова и операторы: `isa super none null nil abstract protected with finally ? ?. **` | S §4.3; P `%RESERVED`, `reserved_message` | `TokenKind::is_reserved`, `parser::Parser::reject_reserved` | `lexer/tests.rs::reserved_words_tokenize_distinctly`; `parser_statements::reserved_words_cannot_be_bindings`; `parser_declarations::type_references` (`User?`) |
| Не зарезервированы: `optional setters mutable var` | D-005 | лексер выдаёт `IDENTIFIER` | `lexer/tests.rs::contextual_and_former_words_are_identifiers`; `parser_statements::reserved_words_cannot_be_bindings` |
| Операторы и longest match (`<- < <= <<`, `.. ..= ...`, `> >= >> >>>`, `:: :`, `& &&`, `\| \|\|`, `-> -`, `?. ?`, `** *`) | P `Lexer::scan_token`; G примечание к `INT` | `lexer::Lexer::scan_token` | `lexer/tests.rs`: `longest_match_for_overlapping_operators`, `stream_write_versus_less_than_negative`, `ampersand_forms_and_projection_hash`, `module_header_punctuation` |
| Десятичные целые, `_` только между цифрами | G `INT`; P `number`, `number_part` | `Lexer::number`, `Lexer::digits_into` | `lexer/tests.rs`: `integer_literals_keep_digits_and_radix`, `underscores_only_between_decimal_digits` |
| `0b`/`0B`/`0x`/`0X`, не больше `INT64_MAX` | G `INT` и примечание; P `number` | `Lexer::based_number`, `IntLiteral::to_u128` | `lexer/tests.rs`: `malformed_based_literals_are_errors`, `based_literals_are_limited_to_int64_max` |
| Float `digits.digits` (без экспоненты) | G `FLOAT`; P `number` | `Lexer::number` | `lexer/tests.rs::float_literals_need_digits_after_the_dot` |
| DECIMAL-токен | G `DECIMAL = implementation_defined`; S §4.2; D-045 (отложено) | не реализован: ни одна реализация его не выдаёт | см. [compatibility.md](compatibility.md) |
| Строки `"..."`, escapes `\n \r \t \" \\`, многострочные | S §4.2; P `string` | `Lexer::string`, `decode_escape` | `lexer/tests.rs`: `string_escapes_decode_and_keep_lexeme`, `literal_errors`, `bad_escape_does_not_turn_the_rest_into_code` |
| Char `'x'`, escapes `\n \r \t \' \\`, ровно один символ после декодирования | S §4.2; P `char_literal` | `Lexer::char_literal` | `lexer/tests.rs`: `char_escapes_decode_before_the_length_check`, `literal_errors` |
| Fenced raw string ```` ```...``` ```` | G `RAW_STRING`; S §4.2; P `fenced_raw_string` | `Lexer::fenced_string` | `lexer/tests.rs`: `fenced_raw_strings`, `fenced_raw_string_errors` |
| CR внутри raw string отбрасывается | D-030 | `Lexer::fenced_string` | `lexer/tests.rs::fenced_raw_strings_discard_carriage_returns` |
| `endl` — псевдоним `"\n"` | P `Lexer::identifier` | `Lexer::word` (`StringForm::Endl`) | `lexer/tests.rs::endl_is_an_exact_newline_alias`; `parser_expressions::literal_forms` |
| Интерполяция строк | S §4.2: интерполяции нет | — (в языке нет) | `lexer/tests.rs::fenced_raw_strings` (текст без обработки) |
| Комментарии `//`, `/* */` без вложенности | P `scan_token`; LEX.COMMENTS | `Lexer::line_comment`, `Lexer::block_comment` | `lexer/tests.rs::comments_are_skipped_and_do_not_nest` |
| Пробелы: пробел, `\t`, `\r`, `\n`; прочее — ошибка | P `scan_token`; D-030 (CRLF) | `Lexer::scan_token` | `lexer/tests.rs::whitespace_includes_cr_and_tabs_only` |
| Позиции: байтовые span, строки по LF, столбцы в символах | (решение проекта, см. architecture.md) | `source::SourceFile::location` | `source::tests`; `lexer/tests.rs`: `token_spans_map_to_lines_and_columns`, `line_accounting_across_multiline_literals_and_comments`; `printer::spans_are_byte_ranges_with_line_columns` |
| EOF, завершимость лексера | — | `Lexer::run` | `lexer/tests.rs`: `eof_is_always_last_with_an_empty_span`, `spans_are_contiguous_with_source_spelling`, `lexer_terminates_on_arbitrary_bytes` |

## Структура модуля

| Конструкция | Источник правила | Реализация | Тесты |
|---|---|---|---|
| `module a/b vX.Y.Z;` первым; сегменты — контекстные имена | G `module_decl`, `module_name`; S §5.1; P `module_declaration`, `consume_module_name_part` | `parser/mod.rs`: `module_declaration`, `consume_module_name_part` | `parser_declarations`: `module_header_forms`, `module_header_errors` |
| Версия (`v1.2.3` лексируется как `v1` `.` `2.3`) | G `version`; P `version` | `parser/mod.rs::version` | `parser_declarations`: `module_header_forms`, `module_header_errors` |
| `import "p" as a take x, Y;` | G `import_decl`; S §5.2; P `import_declaration` | `parser/mod.rs::import_declaration` | `parser_declarations`: `imports`, `import_errors` |
| Импорты только до объявлений | P `module_declaration_item` | `decl.rs::module_item` | `parser_declarations::import_errors` |
| На уровне модуля только `main()` без параметров (или `async main()`) | G ограничения; S §3; P `module_declaration_item` | `decl.rs::module_item` | `parser_declarations::only_main_is_declared_at_module_scope` |
| Разбор до EOF, без «хвоста» | G `program … EOF` | `parser/mod.rs::parse_program` | `parser_robustness::trailing_garbage_is_rejected` |

## Объявления

| Конструкция | Источник правила | Реализация | Тесты |
|---|---|---|---|
| `object Name<T: Bound> does R, S { }` | G `object_decl`, `generic_params`, `does_clause`; S §11.1, §6.5; P `object_declaration` | `decl.rs::object_declaration`, `types.rs::generic_params` | `parser_declarations`: `generic_declarations`, `object_members` |
| Ограничения generic-параметров (не пусто, тип, без дубликатов и затенения; bounds только у объектов) | G ограничения; P `generic_type_parameter_list`, `role_declaration` | `types.rs::generic_params`, `decl.rs::role_declaration` | `parser_declarations::generic_declarations` |
| `has a, b is T = default;` (тип и default необязательны) | G `has_decl`; S §11.2; P `has_declaration` | `decl.rs::has_declaration` | `parser_declarations::object_members` |
| `has(` — метка действия, `has x` — состояние | G (контекстные имена); P `object_member` | `decl.rs::object_member` | `parser_declarations`: `object_members`, `roles_fakes_and_tests` |
| Полный конструктор `Name(p is T) { }` | G `constructor_decl`; S §11.3; P `constructor_declaration` | `decl.rs::constructor_declaration` | `parser_declarations::constructor_forms_stay_distinct` |
| Shorthand `Name(:a, :b);` и `Name(:a) { }` | G `field_param_list`; S §11.3; P `constructor_field_shorthand` | `decl.rs::constructor_shorthand` | `parser_declarations`: `constructor_forms_stay_distinct`, `constructor_and_member_errors` |
| Висящая запятая в shorthand | G `trailing_comma?`; conformance `L01.gram.constructor.shorthand-trailing-comma` | `decl.rs::constructor_shorthand` | `parser_declarations::shorthand_accepts_the_grammar_trailing_comma` |
| `Name();` (нулевой shorthand) отличается от `Name() {}` | S §11.3; P `constructor_declaration` | `decl.rs::constructor_declaration` | `parser_declarations::constructor_forms_stay_distinct` |
| Имя конструктора совпадает с объектом | G ограничения; P `constructor_declaration` | `decl.rs::constructor_declaration` | `parser_declarations::constructor_and_member_errors` |
| `destroy { }` | G `destroy_block`; S §11.5 | `decl.rs::object_member` | `parser_declarations`: `object_members`, `roles_fakes_and_tests` |
| `private:` — секция до конца тела | G `private_section`; S §11.7; P `object_body` | `decl.rs::private_section` (узел `PrivateSection`) | `parser_declarations`: `object_members`, `constructor_and_member_errors` |
| `#projected`, `#projected(alias)`; удалённый `&` | G `projection_action_decl`; S §11.3; P `projection_directive` | `decl.rs::projection_directive` | `parser_declarations`: `object_members`, `constructor_and_member_errors` |
| Действие `async? name(params) is R { }`, обязательный результат | G `action_decl`, `result_spec`; S §11.6; P `action_declaration` | `decl.rs::action_declaration`, `types.rs::result_spec` | `parser_declarations`: `object_members`, `constructor_and_member_errors` |
| Контекстные метки действий (`is(...)`, `has(...)`, `spawn`) | G `CONTEXT_IDENT`; S §4.1; P `consume_action_name` | `parser/mod.rs::consume_action_name` | `parser_declarations`: `object_members`, `native_declarations` |
| Запрет `with` и `with_*` в именах действий | G; S §4.1; P `reject_wither_action_name` | `parser/mod.rs::reject_wither` | `parser_declarations::with_names_are_forbidden_in_action_positions` |
| Параметры, `...T` (variadic, один, последний, без default); висящая запятая | G `param_list`, `param`; S §11.8.1; P `parameter_list`, `parameter` | `decl.rs::parameter_list`, `decl.rs::parameter` | `parser_declarations::variadic_parameters` |
| `cap Name { f is T { } }` в объекте | G `cap_decl`; S §11.2.1; P `cap_declaration` | `decl.rs::cap_declaration`, `cap_fields` | `parser_declarations::caps_and_owner_field_reads` |
| `cap Owner::Name { }` на уровне модуля; `Owner::field` внутри | G `external_cap_decl`, `owner_field_read`; P `cap_declaration`, `finish_postfix` | `decl.rs::cap_declaration`, `expr.rs::finish_postfix` | `parser_declarations::caps_and_owner_field_reads` |
| `role R<T> { sig; fake { } test "" { } }` | G `role_decl`, `role_member`; S §12; P `role_declaration`, `role_member`, `action_signature` | `decl.rs`: `role_declaration`, `role_member`, `action_signature` | `parser_declarations::roles_fakes_and_tests` |
| `fake { members }` | G `fake_block`; S §13; P `fake_block` | `decl.rs::fake_block` | `parser_declarations`: `roles_fakes_and_tests`, `production_mode_skips_test_and_fake_bodies_but_keeps_the_nodes` |
| `enum Name = (a, b);` | G `enum_decl`; S §15; P `enum_declaration` | `decl.rs::enum_declaration` | `parser_declarations::enums` |
| `test "n" { }`, `async test "n" { }` (модуль, объект, роль) | G `test_block`; S §14; P `test_block`, `async_test_block` | `decl.rs`: `test_block`, `async_test_block` | `parser_declarations::object_members`; `prune::tests` |
| Production: тела test/fake без разбора (лексика проверяется) | S §14; P `skip_braced_body` | `decl.rs::skip_braced_body`, `ParseMode::Production` | `parser_declarations::production_mode_skips_test_and_fake_bodies_but_keeps_the_nodes`; `cli::production_mode_and_pruning` |
| Удаление test/fake | S §14 | `prune::strip_test_items` | `prune::tests::pruning_removes_exactly_tests_and_fakes` |
| `native async? name(p) is R;` и `native object Name;` (только на уровне модуля) | G `native_*`; S §5.3; P `native_action_declaration`, `native_object_declaration`, `reject_native_outside_module` | `decl.rs`: `native_declaration`, `native_object`, `reject_native_outside_module` | `parser_declarations::native_declarations` |
| Удалённые `view` | P `check_view_declaration` | `decl.rs::check_view_declaration` | `parser_declarations::removed_object_views` |
| Удалённые `allow …;` и `while` (теперь обычные идентификаторы) | `docs/allow-removal.md`; GRAM.REMOVED-SYNTAX; P `t/lexer/01-grammar-1.0-tokens.t` | общие правила членов и выражений; отдельного токена нет | `parser_declarations::removed_allow_and_while_forms_stay_invalid`; `lexer/tests.rs::contextual_and_former_words_are_identifiers` |

## Типы

| Конструкция | Источник правила | Реализация | Тесты |
|---|---|---|---|
| Имена, квалифицированные `io.Package`, generic `Map<K, V>` | G `named_type`, `qualified_type`, `generic_args`; S §6 | `types.rs::named_type` | `parser_declarations::type_references` |
| `@T`, `%K:V` | G `collection_type`; S §6.3 | `types.rs::type_expr_inner` | `parser_declarations::type_references` |
| `(A, B) is R`, `() is void`, `async (A) is R` | G `action_type`; S §6.5.1 | `types.rs::action_type` | `parser_declarations::type_references`; `parser_expressions::anonymous_actions` |
| `Owner::Cap` | G `named_type` (`::`) | `types.rs::named_type` | `parser_declarations::caps_and_owner_field_reads` |
| `^T` (successor) только в результате | G `successor_type`; S §6.4 | `types.rs::result_spec` | `parser_declarations::roles_fakes_and_tests`; `parser_expressions::anonymous_action_errors` |
| `void` только как маркер результата | G примечание; P `type` | `types.rs::type_expr_inner` | `parser_declarations::type_references` |
| Разрезание `>>`, `>>>`, `>=` при закрытии generic | G примечание к `INT`; P `consume` | `parser/mod.rs`: `consume`, `split_greater` | `parser_expressions::generic_closers_split_compound_tokens` |

## Операторы

| Конструкция | Источник правила | Реализация | Тесты |
|---|---|---|---|
| `let x is T = e;` (имя любого регистра) | G `let_stmt`; S §9.2, §4.3 | `stmt.rs::let_statement` | `parser_statements`: `let_defer_confirm_and_bindings`, `reserved_words_cannot_be_bindings` |
| `defer e;` | G `defer_stmt`; S §19 | `stmt.rs::defer_statement` | `parser_statements::let_defer_confirm_and_bindings` |
| `confirm c else throw e;` / `else panic e;` | G `confirm_stmt`; S §9.4 | `stmt.rs::confirm_statement` | `parser_statements`: `let_defer_confirm_and_bindings`, `statement_errors` |
| `throw e;`, `throw;` | G `throw_stmt`; S §9.5 | `stmt.rs::throw_statement` | `parser_statements::try_throw_and_return` |
| `try { } catch n is T { } …`; без `finally` | G `try_stmt`, `catch_clause`; S §9.6; P `finish_try`, `catch_clause` | `stmt.rs`: `finish_try`, `catch_clause` | `parser_statements::try_throw_and_return` |
| `return e;`, `return;` | G `return_stmt` | `stmt.rs::return_statement` | `parser_statements::try_throw_and_return` |
| `^field = e;` | G `successor_assign`; S §9.13 | `stmt.rs::successor_assignment` | `parser_statements`: `let_defer_confirm_and_bindings`, `statement_errors` |
| `{ … }` как оператор | G `block_stmt`; S §9.3 | `stmt.rs::block_statement` | `parser_statements::nested_block_statements` |
| `name = e;`, `self.name = e;`; `other.x = e` отвергается | G `field_bind_stmt`; S §9.12; P `is_member_field_bind` | `stmt.rs`: `field_bind`, `self_field_bind`, `is_member_field_bind` | `parser_statements`: `let_defer_confirm_and_bindings`, `statement_errors` |
| `if c { } else if … else { }` | G `if_stmt`; S §9.7 | `stmt.rs::if_statement` | `parser_statements::if_statements_chain_else_if` |
| `match e { case a, b { } else { } }` | G `match_stmt`; S §9.8 | `stmt.rs::finish_match` | `parser_statements::match_statements` |
| `for src -> a, b { }`, `for src -> chunk[N] { }` | G `for_stmt`, `pattern`; S §9.9 | `stmt.rs`: `for_statement`, `for_binding` | `parser_statements::for_statements_own_the_top_level_arrow` |
| `loop { }` (с `;` или без), `next if c;`, `last v if c;` | G `loop_stmt`, `next_stmt`, `last_stmt`; S §8.13, §9.10 | `stmt.rs`: `loop_statement`, `next_statement`, `last_statement` | `parser_statements::loop_next_and_last` |
| `e if c;` | G `expr_stmt`, `postfix_if` | `stmt.rs::expression_statement` | `parser_statements::postfix_if_on_expression_statements` |

## Выражения

| Конструкция | Источник правила | Реализация | Тесты |
|---|---|---|---|
| Приоритеты и левая ассоциативность `\|\| && <- == != .. ..= < <= > >= \| ^ & << >> >>> + - * / %` | G лестница `logic_or…factor`; S §8 | `expr.rs`: `binary`, `binary_operator` (precedence climbing) | `parser_expressions`: `arithmetic_precedence_and_left_associativity`, `bitwise_and_shift_precedence`, `comparison_equality_and_logic_levels`, `ranges_sit_between_equality_and_comparison` |
| Диапазон не цепляется (`a..b..c`) | G `range_expr`; S §8.6 | `expr.rs::binary` | `parser_expressions::ranges_sit_between_equality_and_comparison` |
| `sink <- a <- b`, `<- endl` | G `stream_write`; S §8.4.1 | `expr.rs::binary` (`StreamWrite`) | `parser_expressions`: `stream_write_chains_left_and_binds_below_equality`, `stream_write_with_endl_and_unpacked_array_cast` |
| Префиксы `! - ~` | G `unary` | `expr.rs`: `unary`, `operand` | `parser_expressions::unary_operators_bind_tighter_than_binary` |
| `await e`, `spawn call`, `spawn(…)` как вызов | G `await_expr`, `spawn_expr`; S §18, §18.1 | `expr.rs::unary` | `parser_expressions::async_await_and_spawn` |
| `panic e` (полное выражение); `panic;` удалён | G `primary`; S §9.5 | `expr.rs`: `unary`, `panic_value` | `parser_expressions::panic_takes_a_whole_expression` |
| Именованная проекция `prefix Type(…)`, `prefix mod.Type(…)` | G `named_constructor_projection`; S §11.3; P `starts_named_constructor_projection` | `expr.rs`: `starts_named_projection`, `named_projection` | `parser_expressions`: `named_constructor_projections`, `removed_projection_spellings_have_migration_errors` |
| Постфиксы: вызов, `.name`, `::name`, `[i] else f`, `...` | G `postfix_op`; S §8.9 | `expr.rs::finish_postfix` | `parser_expressions`: `postfix_chains_left_to_right`, `unpack_applies_to_any_postfix_operand` |
| Аргументы: позиционные, `name: e`, висящая запятая | G `arg_list`; S §8.10 | `expr.rs`: `arguments`, `argument` | `parser_expressions::calls_arguments_and_trailing_commas` |
| Касты `byte(112)` (обычный вызов) и `@byte(…)`, `@int/@float/@string` | G `array_cast`; S §6.6.1 | `expr.rs::array_cast` | `parser_expressions`: `array_casts_have_a_closed_target_set`, `calls_arguments_and_trailing_commas` |
| `.map() -> x { }`, `.filter() -> x { }`, `.reduce(i) -> a, x { }` | G `arrow_suffix`; S §6.8; P `finish_functional_expression` | `expr.rs`: `expression`, `finish_functional` | `parser_expressions`: `functional_suffixes`, `functional_suffix_errors` |
| `(a is T) is R { }` (анонимное действие) | G `anonymous_action`; S §8.10.2 | `expr.rs`: `parenthesized`, `anonymous_action` | `parser_expressions`: `anonymous_actions`, `anonymous_action_errors` |
| `[]`, `[:]`, `[a, b,]`, `[k: v,]` | G `bracket_lit`; S §8.7, §8.8 | `expr.rs::bracket_literal` | `parser_expressions::collection_literals` |
| `if c { } else { }` как выражение (без `else if`) | G `if_expr`; S §8.11 | `expr.rs::if_expression` | `parser_expressions::if_match_loop_and_try_expressions` |
| `match`, `loop`, `try` как выражения | G `match_expr`, `loop_expr`, `try_expr` | `expr.rs`: `match_expression`, `primary`, `try_expression` | `parser_expressions::if_match_loop_and_try_expressions` |
| Литералы, `true`/`false`, `self`, имена | G `literal`, `primary` | `expr.rs`: `literal`, `primary` | `parser_expressions::literal_forms` |
| Удалённое: `&Type()`, `Type:(…)`, стрелочные лямбды, `{ "a": 1 }` | G ограничения; P `unary`, `finish_postfix`, `argument` | `expr.rs`: `unary`, `finish_postfix`, `argument` | `parser_expressions`: `removed_projection_spellings_have_migration_errors`, `functional_suffix_errors`, `collection_literals` |

## Разрешение неоднозначных последовательностей

| Последовательность | Решение | Где | Тест |
|---|---|---|---|
| `has(` / `has x` в теле объекта | `has(` — действие, иначе состояние | `decl.rs::object_member` | `parser_declarations::object_members` |
| `(` в выражении | `()` или `(name is` — анонимное действие, иначе группировка | `expr.rs::parenthesized` | `parser_expressions::anonymous_actions` |
| `of Max(1)` / `of(Max(1))` | идентификатор, тип и `(` подряд — проекция | `expr.rs::starts_named_projection` | `parser_expressions::named_constructor_projections` |
| `spawn(…)` / `spawn f()` | `spawn(` — вызов действия `spawn` | `expr.rs::unary` | `parser_expressions::async_await_and_spawn` |
| `for x.map() -> v { }` | источник `for` разбирается до уровня `logic_or`, стрелка относится к `for` | `stmt.rs::for_statement` | `parser_statements::for_statements_own_the_top_level_arrow` |
| `last if c;` / `last (if …);` | `if` сразу после `last` — условие | `stmt.rs::last_statement` | `parser_statements::loop_next_and_last` |
| `Box<Box<int>>` / `a >> b` | `>>` разрезается только парсером типа | `parser/mod.rs::split_greater` | `parser_expressions::generic_closers_split_compound_tokens` |
| `a<-b` / `a < -b` | решает лексер (longest match) | `lexer::scan_token` | `parser_expressions::stream_write_chains_left_and_binds_below_equality` |
| `async test "…"` / `async test(…)` | `async` модифицирует объявление, только если дальше имя и `(` | `parser/mod.rs::check_async_modifier` | `parser_declarations::object_members` |
| `cap Name {` / `cap()` | `cap` — объявление, только если за ним идентификатор | `decl.rs::check_cap_declaration` | `parser_declarations::caps_and_owner_field_reads` |
| `[e` | массив или map решает первый разделитель (`,` или `:`) | `expr.rs::bracket_literal` | `parser_expressions::collection_literals` |
| `if` в начале оператора | оператор `if` (`;` после него — ошибка); выражение — только в позиции значения | `stmt.rs::statement` | `parser_statements::nested_block_statements` |
| `a[i] else f == g` | fallback — полное выражение | `expr.rs::finish_postfix` | `parser_expressions::postfix_chains_left_to_right` |
| `Owner::x` | `OwnerField` только в теле `cap Owner::…`, иначе `Select` | `expr.rs::finish_postfix` | `parser_declarations::caps_and_owner_field_reads` |

## Устойчивость

| Требование | Реализация | Тесты |
|---|---|---|
| Без panic и зависаний на любом вводе | `nested`, ограниченные циклы лексера | `parser_robustness`: `every_prefix_of_a_program_terminates`, `deleting_any_single_token_terminates`, `random_token_soup_terminates`; `lexer/tests.rs::lexer_terminates_on_arbitrary_bytes` |
| Глубокая вложенность — диагностика, а не переполнение стека | `Parser::nested`, `ParseOptions::max_nesting` | `parser_robustness`: `deep_nesting_is_reported_not_overflowed`, `nesting_up_to_the_limit_fits_a_small_stack`, `nesting_limit_is_configurable` |
| Незавершённый ввод | все `consume` | `parser_robustness::incomplete_inputs_end_with_a_diagnostic` |
| Точная позиция ошибки | `Diagnostic { span, found }` | `parser_robustness::syntax_errors_point_at_the_offending_token`; `cli::syntax_errors_exit_one_with_nothing_on_stdout` |
| Все лексические ошибки до разбора | `syntax::parse` | `parser_robustness::lexical_errors_prevent_parsing_and_are_all_reported` |
