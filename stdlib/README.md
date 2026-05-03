# Qlang Stdlib

`stdlib` 是仓库内的普通 Qlang workspace，不是编译器内置 prelude，也不是 registry 包。

## 现在怎么用

仓库内当前提供 5 个包：

- `std.core`
- `std.array`
- `std.option`
- `std.result`
- `std.test`

推荐优先使用：

- `std.option.Option[T]` 和 `std.result.Result[T, E]`
- `std.array` 的 canonical length-generic helpers
- `std.test` 的断言 helpers

保留的 concrete API 和固定参数 helper 只是兼容面，不是继续扩张方向。

## 本地依赖

带点的包名需要 quoted TOML key：

```toml
[dependencies]
"std.core" = "../stdlib/packages/core"
"std.option" = "../stdlib/packages/option"
"std.result" = "../stdlib/packages/result"
"std.array" = "../stdlib/packages/array"
"std.test" = "../stdlib/packages/test"
```

Qlang 源码里按 package path 导入：

```ql
use std.array.first_array as first_array
use std.array.sum_int_array as sum_int_array
use std.option.none_option as none_option
use std.option.some as option_some
use std.result.ok as result_ok
use std.result.unwrap_result_or as result_unwrap_result_or
use std.test.expect_bool_eq as expect_bool_eq
```

## 创建项目

```powershell
cargo run -q -p ql-cli -- project init D:\Projects\my-qlang-app --stdlib D:\Projects\language_q\stdlib
cargo run -q -p ql-cli -- project init D:\Projects\my-qlang-workspace --workspace --name app --stdlib D:\Projects\language_q\stdlib
```

生成的项目会直接依赖 `std.core`、`std.option`、`std.result`、`std.array`、`std.test`，并生成可直接运行的 smoke tests。

## 验证

```powershell
cargo run -q -p ql-cli -- project targets stdlib
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- build stdlib
cargo run -q -p ql-cli -- test stdlib
```

## 事实源

以 `stdlib/packages/*/src/lib.ql`、对应 smoke tests 和生成的 `.qi` 为准。README 只保留入口信息和迁移方向。
