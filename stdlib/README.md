# Qlang Stdlib

`stdlib` is an ordinary Qlang workspace. It is not a compiler prelude and it is not published through a registry yet.

Current packages:

- `std.core`: integer and boolean helpers such as `max_int`, `min_int`, `clamp_int`, `abs_int`, and `bool_to_int`.
- `std.test`: smoke-test helpers `expect_true` and `expect_int_eq`; helpers return `0` for pass and non-zero for failure.

Use local dependencies with quoted TOML keys because the package names contain dots:

```toml
[dependencies]
"std.core" = "../stdlib/packages/core"
"std.test" = "../stdlib/packages/test"
```

Then import by package path:

```ql
use std.core.max_int as max_int
use std.test.expect_int_eq as expect_int_eq
```

Verify from the repository root:

```powershell
cargo run -q -p ql-cli -- project targets stdlib
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- build stdlib
cargo run -q -p ql-cli -- test stdlib
```
