# Stdlib Generics and Collections Roadmap

目标是让 `stdlib` 成为真实项目可消费的普通 Qlang workspace，而不是测试夹具。

## 已落地

- 包：`std.core`、`std.option`、`std.result`、`std.array`、`std.test`。
- generic carrier：`Option[T]`、`Result[T, E]`。
- `std.core` 有 canonical length-generic aggregate/order helpers：`sum_ints`、`product_ints`、`average_ints`、`max_ints`、`min_ints`、`all_bools`、`any_bools`、`none_bools`、`is_ascending_ints`、`is_descending_ints`。
- `std.array` 有 canonical length-generic access/query/count/aggregate helpers、`reverse_array[T, N]` 和 `repeat_array[T, N]`。
- 数组长度泛型参数可作为 `Int` 值读取。
- 重复数组字面量 `[value; N]` 支持整数字面量长度和数组长度泛型。
- dependency generic bridge 支持 wrapper specialization 内继续直调同模块 generic helper。
- 单文件和 project 入口共用本地 generic free function direct-call specialization。
- `std.test` 聚合断言和状态合并已使用 length-generic 数组入口。
- `ql project init --stdlib` 已生成可 `check/run/test` 的模板。

## 下一步顺序

1. 继续修语言和后端能力，让剩余固定 arity 兼容层逐步退场。
2. 为每个 public stdlib API 补 package-local 测试和 downstream consumer smoke。
3. 扩 method/value generic import 和非 direct-call generic 值前，先补清楚 monomorphization contract。

## 规则

- 不新增 `foo3/foo4/foo5` API；先补语言能力，再用 canonical generic API 表达。
- 不把测试 helper 当作标准库 API。
- 不把 variadic 写进 stdlib 文档，直到语言语法和后端都落地。
- 实现未通过 downstream `ql check/build/run/test` 前，不宣称可用。

## 验证

```powershell
cargo run -q -p ql-cli -- project targets stdlib
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- test stdlib
```
