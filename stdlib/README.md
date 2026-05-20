# Qlang Stdlib

`stdlib` 是仓库内的普通 Qlang workspace。它不是内置 prelude，也不是 registry 包。

## 包

- `std.core`
- `std.option`
- `std.result`
- `std.array`
- `std.test`

## 示例

- `examples/starter` 是 `ql project init --stdlib` 使用的真实 starter 模板。

## 推荐 API

- `std.core` 的数组 helpers，例如 `sum_ints`、`product_ints`、`average_ints`、`max_ints`、`min_ints`、`median_ints`、`all_bools`、`is_ascending_ints`、`is_strictly_descending_ints`
- `std.option.Option[T]`
- `std.result.Result[T, E]`
- `std.array` 的 length-generic helpers，例如 `first_array`、`last_array`、`at_array_or`、`contains_array`、`count_array`、`len_array`、`reverse_array`、`repeat_array`、`average_int_array`
- `std.test` 的泛型 `expect_eq` / `expect_ne`、泛型数组 equality/access/query/reverse 断言、泛型 `expect_option_*` / `expect_result_*` 断言、Int/Bool 专用行为断言和 `merge_statuses[N]`

`std.array` 不再导出 `first3_array`、`reverse3_array`、`repeat3_array` 这类固定长度 helper；新代码只使用 length-generic API。重复数组使用语言级 `[value; N]`，标准库暴露 `repeat_array[T, N](value) -> [T; N]`。
`std.core` 的聚合、布尔聚合、顺序判断和中位数使用数组 API；固定 arity 的 `sum3_int`、`max4_int`、`median3_int`、`all5_bool`、`is_ascending4_int` 等历史包装已删除。
`std.test` 的 equality/array/option/result 断言统一走泛型 API，例如 `expect_eq[T]`、`expect_array_eq[T, N]`、`expect_array_contains[T, N]`、`expect_array_reverse[T, N]`、`expect_option_*`、`expect_result_*`；`expect_array_reverse` 会比较完整反转结果。历史 typed facade、固定 arity 的 `expect_*3/4/5` / `merge_status3/4/5/6`、以及 `is_status_*` / `expect_status_*` / `merge_status` 薄封装已删除。
当前 `Option.None`、`Result.Ok(...)`、`Result.Err(...)` 这类不能从调用点直接闭合类型参数的值，仍建议先绑定到显式类型局部变量，再传给泛型断言。

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
use std.test.expect_eq as expect_eq
```

## 创建项目

```powershell
cargo run -q -p ql-cli -- project init D:\Projects\my-qlang-app --stdlib D:\Projects\language_q\stdlib
cargo run -q -p ql-cli -- project init D:\Projects\my-qlang-workspace --workspace --name app --stdlib D:\Projects\language_q\stdlib
```

生成的 starter 直接复制 `examples/starter`，使用推荐的 generic `Option[T]`、`Result[T, E]`、length-generic array helpers、重复数组和 `std.test` 泛型断言；smoke test 直接覆盖数组 equality/reverse、`expect_option_*` 和 `expect_result_*`。

## 验证

```powershell
cargo run -q -p ql-cli -- project targets stdlib
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- check stdlib --package stdlib.starter --json
cargo run -q -p ql-cli -- project emit-interface --check stdlib
cargo run -q -p ql-cli -- project emit-interface --check stdlib --package stdlib.starter
cargo run -q -p ql-cli -- project emit-interface --changed-only --check stdlib --package stdlib.starter
cargo run -q -p ql-cli -- project status stdlib --json
cargo run -q -p ql-cli -- project status stdlib --package stdlib.starter --json
cargo run -q -p ql-cli -- project graph stdlib --json
cargo run -q -p ql-cli -- project graph stdlib --package stdlib.starter --json
cargo run -q -p ql-cli -- project targets stdlib --package stdlib.starter --json
cargo run -q -p ql-cli -- project dependencies stdlib --name stdlib.starter --json
cargo run -q -p ql-cli -- project dependencies stdlib --package stdlib.starter --json
cargo run -q -p ql-cli -- project dependents stdlib --name std.option --json
cargo run -q -p ql-cli -- project dependents stdlib --package std.option --json
cargo run -q -p ql-cli -- project dependents stdlib --name std.core --json
cargo run -q -p ql-cli -- project lock stdlib --json
cargo run -q -p ql-cli -- project lock stdlib --check --json
cargo run -q -p ql-cli -- build stdlib --list --json
cargo run -q -p ql-cli -- build stdlib --package stdlib.starter --json
cargo run -q -p ql-cli -- build stdlib --json
cargo run -q -p ql-cli -- run stdlib --list --json
cargo run -q -p ql-cli -- run stdlib --package stdlib.starter --json
cargo run -q -p ql-cli -- test stdlib --list --json
cargo run -q -p ql-cli -- test stdlib --list --json --package stdlib.starter
cargo run -q -p ql-cli -- test stdlib --package stdlib.starter --json
cargo run -q -p ql-cli -- test stdlib
```

集成门禁还覆盖复制出的 source-only stdlib：starter `build/run/test --list --json --package`、starter `build/run/test --package --json`、`project dependencies/dependents --package/--name --json`、同步接口后 `project graph/status/targets --package stdlib.starter --json`、写入 `qlang.lock`、`--check --json` up-to-date，以及修改 manifest 后的 stale failure JSON；stale 检查不会重写旧 lockfile。

事实源：`stdlib/packages/*/src/lib.ql`、`stdlib/examples/starter`、smoke tests 和生成的 `.qi`。
