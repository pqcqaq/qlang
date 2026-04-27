# Qlang Stdlib

`stdlib` is an ordinary Qlang workspace. It is not a compiler prelude and it is not published through a registry yet.

Current packages:

- `std.core`: integer and boolean helpers such as `max_int`, `min_int`, `clamp_int`, `abs_int`, `sign_int`, `compare_int`, `is_zero_int`, `is_nonzero_int`, `is_positive_int`, `is_nonnegative_int`, `is_negative_int`, `is_nonpositive_int`, `is_even_int`, `is_odd_int`, `in_range_int`, `in_exclusive_range_int`, `is_divisible_by_int`, `bool_to_int`, `not_bool`, `and_bool`, `or_bool`, `xor_bool`, and `implies_bool`.
- `std.test`: smoke-test helpers `expect_true`, `expect_false`, `expect_bool_eq`, `expect_int_eq`, `expect_int_ne`, `expect_int_gt`, `expect_int_ge`, `expect_int_lt`, `expect_int_le`, `expect_zero`, `expect_nonzero`, `expect_int_between`, `expect_int_exclusive_between`, `expect_int_outside`, `expect_int_even`, `expect_int_odd`, `expect_int_divisible_by`, `expect_int_positive`, `expect_int_negative`, `expect_int_nonnegative`, `expect_int_nonpositive`, and `expect_bool_implies`; helpers return `0` for pass and non-zero for failure.

Use local dependencies with quoted TOML keys because the package names contain dots:

```toml
[dependencies]
"std.core" = "../stdlib/packages/core"
"std.test" = "../stdlib/packages/test"
```

Then import by package path:

```ql
use std.core.is_divisible_by_int as is_divisible_by_int
use std.test.expect_bool_eq as expect_bool_eq
```

To create a new package or workspace member that already consumes this stdlib:

```powershell
cargo run -q -p ql-cli -- project init D:\Projects\my-qlang-app --stdlib D:\Projects\language_q\stdlib
cargo run -q -p ql-cli -- project init D:\Projects\my-qlang-workspace --workspace --name app --stdlib D:\Projects\language_q\stdlib
```

Verify from the repository root:

```powershell
cargo run -q -p ql-cli -- project targets stdlib
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- build stdlib
cargo run -q -p ql-cli -- test stdlib
```
