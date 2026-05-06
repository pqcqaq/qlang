# Qlang Stdlib

`stdlib` 是仓库内的普通 Qlang workspace。它不是内置 prelude，也不是 registry 包。

## 包

- `std.core`
- `std.option`
- `std.result`
- `std.array`
- `std.test`

## 推荐 API

- `std.core` 的数组 helpers，例如 `sum_ints`、`product_ints`、`average_ints`、`max_ints`、`min_ints`、`all_bools`、`is_ascending_ints`、`is_strictly_descending_ints`
- `std.option.Option[T]`
- `std.result.Result[T, E]`
- `std.array` 的 length-generic helpers，例如 `first_array`、`last_array`、`at_array_or`、`contains_array`、`count_array`、`len_array`、`reverse_array`、`repeat_array`、`average_int_array`
- `std.test` 的普通断言、数组断言和 `merge_statuses` 状态合并 helper

`std.array` 不再导出 `first3_array`、`reverse3_array`、`repeat3_array` 这类固定长度 helper；新代码只使用 length-generic API。重复数组使用语言级 `[value; N]`，标准库暴露 `repeat_array[T, N](value) -> [T; N]`。
`std.core` 里的 `sum3_int`、`max4_int`、`all5_bool`、`is_ascending4_int` 等固定 arity 名称只作为兼容包装保留，新代码应传数组给泛型 API。
`std.test` 的聚合断言和状态合并使用数组 API；固定 arity 的 `expect_*3/4/5` 聚合断言和 `merge_status3/4/5/6` 已删除。

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

生成的 starter 使用推荐的 generic `Option[T]`、`Result[T, E]` 和 length-generic array helpers。

## 验证

```powershell
cargo run -q -p ql-cli -- project targets stdlib
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- build stdlib
cargo run -q -p ql-cli -- test stdlib
```

事实源：`stdlib/packages/*/src/lib.ql`、smoke tests 和生成的 `.qi`。
