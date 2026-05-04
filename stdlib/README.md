# Qlang Stdlib

`stdlib` 是仓库内的普通 Qlang workspace。它不是内置 prelude，也不是 registry 包。

## 包

- `std.core`
- `std.option`
- `std.result`
- `std.array`
- `std.test`

## 推荐 API

- `std.option.Option[T]`
- `std.result.Result[T, E]`
- `std.array` 的 length-generic helpers，例如 `first_array`、`last_array`、`at_array_or`、`contains_array`、`count_array`、`len_array`
- `std.test` 的普通断言和数组断言 helpers

固定长度数组 helper 和 concrete carrier 只保留兼容，不再扩张；LSP 会在补全、hover 和 semantic tokens 中把这些兼容 API 标记为 deprecated。

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

Qlang 源码按 package path 导入：

```ql
use std.array.len_array as len_array
use std.option.some as option_some
use std.result.ok as result_ok
use std.test.expect_bool_eq as expect_bool_eq
```

## 创建项目

```powershell
cargo run -q -p ql-cli -- project init D:\Projects\my-qlang-app --stdlib D:\Projects\language_q\stdlib
cargo run -q -p ql-cli -- project init D:\Projects\my-qlang-workspace --workspace --name app --stdlib D:\Projects\language_q\stdlib
```

生成的 starter 使用推荐的 generic `Option[T]`、`Result[T, E]` 和 length-generic array helpers；固定 arity / concrete helper 只作为兼容层保留。

## 验证

```powershell
cargo run -q -p ql-cli -- project targets stdlib
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- build stdlib
cargo run -q -p ql-cli -- test stdlib
```

事实源：`stdlib/packages/*/src/lib.ql`、smoke tests 和生成的 `.qi`。
