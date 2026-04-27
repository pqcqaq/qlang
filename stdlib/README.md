# Qlang Stdlib

`stdlib` is an ordinary Qlang workspace. It is not a compiler prelude and it is not published through a registry yet.

Current packages:

- `std.core`: integer and boolean helpers such as `max_int`, `min_int`, `clamp_int`, `abs_int`, `sign_int`, `is_even_int`, `is_odd_int`, `in_range_int`, and `bool_to_int`.
- `std.test`: smoke-test helpers `expect_true`, `expect_false`, `expect_int_eq`, `expect_zero`, and `expect_nonzero`; helpers return `0` for pass and non-zero for failure.

Use local dependencies with quoted TOML keys because the package names contain dots:

```toml
[dependencies]
"std.core" = "../stdlib/packages/core"
"std.test" = "../stdlib/packages/test"
```

Then import by package path:

```ql
use std.core.in_range_int as in_range_int
use std.test.expect_true as expect_true
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
