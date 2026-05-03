# Qlang Stdlib

`stdlib` is an ordinary Qlang workspace. It is not a compiler prelude and it is not published through a registry yet.

This is real standard-library code for downstream packages, not only a test fixture. The current `IntOption` / `BoolOption`, `IntResult` / `BoolResult`, and fixed-arity helpers such as `sum3_int` are transitional compatibility APIs. Generic carrier types `Option[T]` and `Result[T, E]` are now executable in downstream package signatures and smoke tests, and `std.option` / `std.result` both have first executable generic helper slices. Uninstantiated generic function declarations can live in library packages and `.qi`; direct dependency and package-under-test builds also support multiple direct-call local specializations of a generic public free function when every generic parameter is inferred from primitive `Int` / `Bool` / `String` literals, simple numeric / boolean / comparison expressions, tuple / fixed-array literals, fixed-array literals whose known items and literal length are enough to bind `[T; N]` parameters, one-level tuple or fixed-array projections, simple `if` / `match` expressions whose result arms agree, explicit typed values, generic carrier values, single-field generic enum variant constructor expressions such as `Option.Some(42)`, named arguments reordered to declaration order, or explicit result context such as function return types and typed `let` / global / field initializers. This result-context path now covers zero-argument helpers such as `std.option.none_option` when the expected type is an explicit `Option[T]`. Non-direct generic helper calls still report `dependency-function-unsupported-generic` until full monomorphization lands.

Current packages:

- `std.core`: integer and boolean helpers such as `max_int`, `min_int`, `max3_int`, `max4_int`, `max5_int`, `min3_int`, `min4_int`, `min5_int`, `sum3_int`, `sum4_int`, `sum5_int`, `product3_int`, `product4_int`, `product5_int`, `average2_int`, `average3_int`, `average4_int`, `average5_int`, `quotient_or_zero_int`, `remainder_or_zero_int`, `clamp_int`, `clamp_min_int`, `clamp_max_int`, `clamp_bounds_int`, `lower_bound_int`, `upper_bound_int`, `abs_int`, `abs_diff_int`, `range_span_int`, `distance_to_range_int`, `distance_to_bounds_int`, `sign_int`, `compare_int`, `median3_int`, `is_zero_int`, `is_nonzero_int`, `is_positive_int`, `is_nonnegative_int`, `is_negative_int`, `is_nonpositive_int`, `is_even_int`, `is_odd_int`, `in_range_int`, `in_exclusive_range_int`, `in_bounds_int`, `in_exclusive_bounds_int`, `is_outside_range_int`, `is_outside_bounds_int`, `is_ascending_int`, `is_ascending4_int`, `is_ascending5_int`, `is_strictly_ascending_int`, `is_strictly_ascending4_int`, `is_strictly_ascending5_int`, `is_descending_int`, `is_descending4_int`, `is_descending5_int`, `is_strictly_descending_int`, `is_strictly_descending4_int`, `is_strictly_descending5_int`, `is_divisible_by_int`, `has_remainder_int`, `is_factor_of_int`, `is_within_int`, `is_not_within_int`, `bool_to_int`, `all3_bool`, `all4_bool`, `all5_bool`, `any3_bool`, `any4_bool`, `any5_bool`, `none3_bool`, `none4_bool`, `none5_bool`, `not_bool`, `and_bool`, `or_bool`, `xor_bool`, and `implies_bool`.
- `std.array`: canonical length-generic fixed-array helpers `first_array[T, N]`, `last_array[T, N]`, `at_array_or[T, N]`, `contains_array[T, N]`, `count_array[T, N]`, `sum_int_array[N]`, `product_int_array[N]`, `max_int_array[N]`, `min_int_array[N]`, `all_bool_array[N]`, `any_bool_array[N]`, and `none_bool_array[N]`; compatibility helpers keep the 3/4/5 accessor/query names plus the old fixed-length aggregate names. `contains_array` / `count_array` are equality-backed and currently smoke-covered for `Int`, `Bool`, and `String`, not user-defined equality. Reverse/repeat remain fixed-length until generic array construction over arbitrary `N` lands.
- `std.option`: executable generic carrier `Option[T]` plus generic helpers `some`, `none_option`, `is_some`, `is_none`, `unwrap_or`, and `or_option`; concrete `IntOption` / `BoolOption` helpers `some_int`, `none_int`, `is_some_int`, `is_none_int`, `unwrap_or_int`, `or_int`, `or_option_int`, `value_or_zero_int`, `some_bool`, `none_bool`, `is_some_bool`, `is_none_bool`, `unwrap_or_bool`, `or_option_bool`, `value_or_false_bool`, and `value_or_true_bool` remain as compatibility APIs. Generic `none_option()` requires an explicit `Option[T]` return type or typed initializer context. It is not named `none` because lowercase `none` is a language literal.
- `std.result`: executable generic carrier `Result[T, E]` plus generic helpers `ok`, `err`, `is_ok`, `is_err`, `unwrap_result_or`, `or_result`, `error_or`, `ok_or`, `to_option`, and `error_to_option`; concrete `IntResult` / `BoolResult` helpers `ok_int`, `err_int`, `is_ok_int`, `is_err_int`, `unwrap_result_or_int`, `or_result_int`, `error_or_zero_int`, `error_to_option_int`, `ok_or_int`, `to_option_int`, `ok_bool`, `err_bool`, `is_ok_bool`, `is_err_bool`, `unwrap_result_or_bool`, `or_result_bool`, `error_or_zero_bool`, `error_to_option_bool`, `ok_or_bool`, and `to_option_bool` remain as compatibility APIs. Generic `ok` / `err` require an explicit `Result[T, E]` return or typed initializer context for the side not fixed by the argument.
- `std.test`: smoke-test helpers for bool/int equality, ordering, ranges, arithmetic helpers, status merging, fixed-array assertions, and concrete/generic Option/Result carrier/conversion assertions. `expect_*` helpers use `0` for pass and non-zero for failure. Array assertions include first/last, fallback-index, reverse, repeat, contains, count, and Int/Bool aggregates, and reuse `std.array` rather than duplicating aggregate implementation. Treat `stdlib/packages/test/src/lib.ql` and emitted interface artifacts as the exhaustive API truth.

Use local dependencies with quoted TOML keys because the package names contain dots:

```toml
[dependencies]
"std.core" = "../stdlib/packages/core"
"std.option" = "../stdlib/packages/option"
"std.result" = "../stdlib/packages/result"
"std.array" = "../stdlib/packages/array"
"std.test" = "../stdlib/packages/test"
```

Then import by package path:

```ql
use std.core.is_divisible_by_int as is_divisible_by_int
use std.array.at_array_or as at_array_or
use std.array.contains_array as contains_array
use std.array.count_array as count_array
use std.array.first_array as first_array
use std.array.repeat3_array as repeat3_array
use std.array.reverse3_array as reverse3_array
use std.array.sum_int_array as sum_int_array
use std.option.some as option_some
use std.option.none_option as option_none
use std.option.unwrap_or as option_unwrap_or
use std.result.Result as Result
use std.result.ok as result_ok
use std.result.unwrap_result_or as result_unwrap_result_or
use std.test.expect_bool_eq as expect_bool_eq
```

To create a new package or workspace member that already consumes this stdlib:

```powershell
cargo run -q -p ql-cli -- project init D:\Projects\my-qlang-app --stdlib D:\Projects\language_q\stdlib
cargo run -q -p ql-cli -- project init D:\Projects\my-qlang-workspace --workspace --name app --stdlib D:\Projects\language_q\stdlib
```

Generated `--stdlib` projects depend on `std.core`, `std.option`, `std.result`, `std.array`, and `std.test`. `std.option` and `std.result` now expose executable generic carriers alongside their concrete helper APIs: direct dependency builds support explicit generic `struct` / `enum` instantiations in non-generic function signatures, and the generated smoke tests now exercise `Option[T]` / `Result[T, E]` through public `std.test` generic carrier assertions instead of local hand-written status functions. `std.option` exposes executable generic `some`, `none_option`, `is_some`, `is_none`, `unwrap_or`, and `or_option`; `std.result` exposes executable generic `ok`, `err`, `is_ok`, `is_err`, `unwrap_result_or`, `or_result`, `error_or`, `ok_or`, `to_option`, and `error_to_option`. Generated `--stdlib` package code now uses generic `std.option.some` / `std.option.unwrap_or`, generic `std.result.ok` / `std.result.Result` / `std.result.unwrap_result_or`, canonical length-generic `std.array.first_array`, `std.array.at_array_or`, `std.array.contains_array`, `std.array.count_array`, `std.array.sum_int_array`, and `std.array.all_bool_array`, plus fixed-length `std.array.reverse3_array` / `std.array.repeat3_array` while those construction APIs are still transitional. The generated code now keeps nested array-transform calls inline instead of adding typed intermediate variables solely for dependency generic inference. Generated smoke code consumes `std.option.none_option` on explicit `Option[Int]` / `Option[Bool]` paths, generic Result/Option conversion assertions, and `std.test` array assertion helpers for generic accessors, fallback-index, contains/count, Int/Bool aggregates, and transitional reverse/repeat calls. Full generic helper function monomorphization, automatic prelude integration, and method/value generic imports are still open, so the concrete Option/Result helpers remain compatibility APIs. `std.array` aggregates now use canonical `[Int; N]` / `[Bool; N]` APIs with 3/4/5 compatibility wrappers; reverse/repeat remain fixed-length until generic array construction over arbitrary `N` lands. `std.test` generic carrier assertions intentionally match `Option[T]` / `Result[T, E]` directly where needed and now uses `std.result` generic conversion helpers for typed `Int` / `Bool` smoke paths.
`project init --stdlib` prepares the stdlib dependency interface artifacts during scaffolding, so generated projects can run plain `ql check`, `ql build`, `ql run`, and `ql test` immediately.
Generated smoke tests group assertion statuses with `merge_status4` / `merge_status5` / `merge_status6`, use concrete and generic carrier-specific `std.test` assertions for `std.option` / `std.result`, cover concrete `std.array` fixed-array aggregate helpers plus generic fixed-array accessor, fallback-index, reverse, repeat, contains, and count calls through `std.test` array assertions, cover concrete and generic `std.result` <-> `std.option` conversion and error extraction helpers through `std.test`, and return `expect_status_ok(...)` so larger tests can keep the same `0` pass / non-zero failure contract without one long status expression.
`std.test` also uses a package-aware smoke test that imports its own public helpers through `use std.test...` and exercises `std.array`, `std.option`, and `std.result` assertions, so the assertion package is checked through the same surface as downstream users.

Verify from the repository root:

```powershell
cargo run -q -p ql-cli -- project targets stdlib
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- build stdlib
cargo run -q -p ql-cli -- test stdlib
```
