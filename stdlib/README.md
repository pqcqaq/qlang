# Qlang Stdlib

`stdlib` is an ordinary Qlang workspace. It is not a compiler prelude and it is not published through a registry yet.

Current packages:

- `std.core`: integer and boolean helpers such as `max_int`, `min_int`, `max3_int`, `max4_int`, `max5_int`, `min3_int`, `min4_int`, `min5_int`, `sum3_int`, `sum4_int`, `sum5_int`, `product3_int`, `product4_int`, `product5_int`, `average2_int`, `average3_int`, `average4_int`, `average5_int`, `quotient_or_zero_int`, `remainder_or_zero_int`, `clamp_int`, `clamp_min_int`, `clamp_max_int`, `clamp_bounds_int`, `lower_bound_int`, `upper_bound_int`, `abs_int`, `abs_diff_int`, `range_span_int`, `distance_to_range_int`, `distance_to_bounds_int`, `sign_int`, `compare_int`, `median3_int`, `is_zero_int`, `is_nonzero_int`, `is_positive_int`, `is_nonnegative_int`, `is_negative_int`, `is_nonpositive_int`, `is_even_int`, `is_odd_int`, `in_range_int`, `in_exclusive_range_int`, `in_bounds_int`, `in_exclusive_bounds_int`, `is_outside_range_int`, `is_outside_bounds_int`, `is_ascending_int`, `is_ascending4_int`, `is_ascending5_int`, `is_strictly_ascending_int`, `is_strictly_ascending4_int`, `is_strictly_ascending5_int`, `is_descending_int`, `is_descending4_int`, `is_descending5_int`, `is_strictly_descending_int`, `is_strictly_descending4_int`, `is_strictly_descending5_int`, `is_divisible_by_int`, `has_remainder_int`, `is_factor_of_int`, `is_within_int`, `is_not_within_int`, `bool_to_int`, `all3_bool`, `all4_bool`, `all5_bool`, `any3_bool`, `any4_bool`, `any5_bool`, `none3_bool`, `none4_bool`, `none5_bool`, `not_bool`, `and_bool`, `or_bool`, `xor_bool`, and `implies_bool`.
- `std.option`: concrete `IntOption` / `BoolOption` helpers `some_int`, `none_int`, `is_some_int`, `is_none_int`, `unwrap_or_int`, `or_int`, `or_option_int`, `value_or_zero_int`, `some_bool`, `none_bool`, `is_some_bool`, `is_none_bool`, `unwrap_or_bool`, `or_option_bool`, `value_or_false_bool`, and `value_or_true_bool`.
- `std.result`: concrete `IntResult` / `BoolResult` helpers `ok_int`, `err_int`, `is_ok_int`, `is_err_int`, `unwrap_result_or_int`, `or_result_int`, `error_or_zero_int`, `error_to_option_int`, `ok_or_int`, `to_option_int`, `ok_bool`, `err_bool`, `is_ok_bool`, `is_err_bool`, `unwrap_result_or_bool`, `or_result_bool`, `error_or_zero_bool`, `error_to_option_bool`, `ok_or_bool`, and `to_option_bool`.
- `std.test`: smoke-test helpers `expect_true`, `expect_false`, `expect_bool_eq`, `expect_bool_ne`, `expect_bool_not`, `expect_bool_and`, `expect_bool_or`, `expect_bool_xor`, `expect_bool_all3`, `expect_bool_all4`, `expect_bool_all5`, `expect_bool_any3`, `expect_bool_any4`, `expect_bool_any5`, `expect_bool_none3`, `expect_bool_none4`, `expect_bool_none5`, `expect_bool_to_int`, `expect_int_eq`, `expect_int_ne`, `expect_int_gt`, `expect_int_ge`, `expect_int_lt`, `expect_int_le`, `expect_zero`, `expect_nonzero`, `expect_int_max`, `expect_int_min`, `expect_int_max3`, `expect_int_min3`, `expect_int_max4`, `expect_int_min4`, `expect_int_max5`, `expect_int_min5`, `expect_int_median3`, `expect_int_sum3`, `expect_int_sum4`, `expect_int_sum5`, `expect_int_product3`, `expect_int_product4`, `expect_int_product5`, `expect_int_average2`, `expect_int_average3`, `expect_int_average4`, `expect_int_average5`, `expect_int_sign`, `expect_int_compare`, `expect_int_abs`, `expect_int_abs_diff`, `expect_int_range_span`, `expect_int_lower_bound`, `expect_int_upper_bound`, `expect_int_quotient_or_zero`, `expect_int_remainder_or_zero`, `expect_int_has_remainder`, `expect_int_factor_of`, `expect_int_option_some`, `expect_int_option_none`, `expect_int_option_or`, `expect_int_option_ok_or`, `expect_int_option_ok_or_err`, `expect_bool_option_some`, `expect_bool_option_none`, `expect_bool_option_or`, `expect_bool_option_ok_or`, `expect_bool_option_ok_or_err`, `expect_int_result_ok`, `expect_int_result_err`, `expect_int_result_error_some`, `expect_int_result_error_none`, `expect_int_result_or`, `expect_int_result_to_option_some`, `expect_int_result_to_option_none`, `expect_bool_result_ok`, `expect_bool_result_err`, `expect_bool_result_error_some`, `expect_bool_result_error_none`, `expect_bool_result_or`, `expect_bool_result_to_option_some`, `expect_bool_result_to_option_none`, `is_status_ok`, `is_status_failed`, `merge_status`, `merge_status3`, `merge_status4`, `merge_status5`, `merge_status6`, `expect_status_ok`, `expect_status_failed`, `expect_int_between`, `expect_int_exclusive_between`, `expect_int_outside`, `expect_int_between_bounds`, `expect_int_exclusive_between_bounds`, `expect_int_outside_bounds`, `expect_int_clamp_min`, `expect_int_clamp_max`, `expect_int_clamped`, `expect_int_clamped_bounds`, `expect_int_distance_to_range`, `expect_int_distance_to_bounds`, `expect_int_ascending`, `expect_int_ascending4`, `expect_int_ascending5`, `expect_int_strictly_ascending`, `expect_int_strictly_ascending4`, `expect_int_strictly_ascending5`, `expect_int_descending`, `expect_int_descending4`, `expect_int_descending5`, `expect_int_strictly_descending`, `expect_int_strictly_descending4`, `expect_int_strictly_descending5`, `expect_int_even`, `expect_int_odd`, `expect_int_divisible_by`, `expect_int_within`, `expect_int_not_within`, `expect_int_positive`, `expect_int_negative`, `expect_int_nonnegative`, `expect_int_nonpositive`, and `expect_bool_implies`; `expect_*` helpers use `0` for pass and non-zero for failure. `std.test` depends on `std.option` and `std.result` for the carrier, conversion, and error extraction assertions above.

Use local dependencies with quoted TOML keys because the package names contain dots:

```toml
[dependencies]
"std.core" = "../stdlib/packages/core"
"std.option" = "../stdlib/packages/option"
"std.result" = "../stdlib/packages/result"
"std.test" = "../stdlib/packages/test"
```

Then import by package path:

```ql
use std.core.is_divisible_by_int as is_divisible_by_int
use std.option.unwrap_or_int as unwrap_or_int
use std.result.ok_int as result_ok_int
use std.test.expect_bool_eq as expect_bool_eq
```

To create a new package or workspace member that already consumes this stdlib:

```powershell
cargo run -q -p ql-cli -- project init D:\Projects\my-qlang-app --stdlib D:\Projects\language_q\stdlib
cargo run -q -p ql-cli -- project init D:\Projects\my-qlang-workspace --workspace --name app --stdlib D:\Projects\language_q\stdlib
```

Generated `--stdlib` projects depend on `std.core`, `std.option`, `std.result`, and `std.test`. `std.option` and `std.result` are intentionally concrete today: generic `Option[T]` / `Result[T, E]` and automatic prelude integration remain language/runtime work, while `IntOption` / `BoolOption` and `IntResult` / `BoolResult` are executable through the current dependency bridge. `std.test` now also ships carrier-specific assertions for `std.option` and `std.result`.
Generated smoke tests group assertion statuses with `merge_status4` / `merge_status5` / `merge_status6`, use carrier-specific `std.test` assertions for `std.option` / `std.result`, cover the `std.result` <-> `std.option` conversion and error extraction helpers through `std.test`, and return `expect_status_ok(...)` so larger tests can keep the same `0` pass / non-zero failure contract without one long status expression.
`std.test` also uses a package-aware smoke test that imports its own public helpers through `use std.test...` and exercises `std.option` / `std.result` assertions, so the assertion package is checked through the same surface as downstream users.

Verify from the repository root:

```powershell
cargo run -q -p ql-cli -- project targets stdlib
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- build stdlib
cargo run -q -p ql-cli -- test stdlib
```
