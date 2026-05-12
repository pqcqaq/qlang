# Stdlib Generics and Collections Roadmap

目标是让 `stdlib` 成为真实项目可消费的普通 Qlang workspace，而不是测试夹具。

## 已落地

- 包：`std.core`、`std.option`、`std.result`、`std.array`、`std.test`。
- generic carrier：`Option[T]`、`Result[T, E]`。
- `std.core` 聚合、布尔聚合、顺序判断和中位数只保留 canonical length-generic helpers：`sum_ints`、`product_ints`、`average_ints`、`max_ints`、`min_ints`、`median_ints`、`all_bools`、`any_bools`、`none_bools`、`is_ascending_ints`、`is_descending_ints`。
- `std.array` 有 canonical length-generic access/query/count/aggregate helpers、`reverse_array[T, N]` 和 `repeat_array[T, N]`。
- `std.option` / `std.result` 只保留 generic carrier API；`IntOption` / `BoolOption` / `IntResult` / `BoolResult` 等 concrete carrier API 已删除。
- 数组长度泛型参数可作为 `Int` 值读取。
- 重复数组字面量 `[value; N]` 支持整数字面量长度和数组长度泛型。
- dependency generic bridge 支持 wrapper specialization 内继续直调同模块 generic helper。
- dependency generic bridge 可从外层调用参数/返回上下文推断嵌套 direct-call specialization。
- 单文件和 project 入口共用本地 generic free function direct-call specialization。
- `std.test` 聚合断言、顺序断言和状态合并已使用 length-generic 数组入口。
- package-local smoke 的状态聚合已使用 length-generic 数组 helper，不再保留 `sum4` / `sum6` 这类测试内固定 arity helper。
- `std.test` 已有 generic `expect_option_*` / `expect_result_*` 断言，package-local smoke 直接覆盖 generic carrier 语义。
- `ql project init --stdlib` 已生成可 `check/run/test` 的模板。

## 下一步顺序

1. 继续修 generic monomorphization 和 dependency-aware backend，让测试和真实项目能直接消费 generic public functions。
2. 为每个 public stdlib API 补 package-local 测试和 downstream consumer smoke。
3. 扩 method/value generic import 和非 direct-call generic 值前，先补清楚 monomorphization contract。

## 规则

- 不新增 `foo3/foo4/foo5` API；先补语言能力，再用 canonical generic API 表达。
- 不把测试 helper 当作标准库 API；测试内聚合也不得新增固定 arity helper。
- 不把 variadic 写进 stdlib 文档，直到语言语法和后端都落地。
- 实现未通过 downstream `ql check/build/run/test` 前，不宣称可用。

## 验证

```powershell
cargo run -q -p ql-cli -- project targets stdlib
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- test stdlib
```
