# Stdlib Generics and Collections Roadmap

目标是让 `stdlib` 成为真实项目可消费的普通 Qlang workspace，而不是测试夹具。

## 已落地

- 包：`std.core`、`std.option`、`std.result`、`std.array`、`std.test`。
- generic carrier：`Option[T]`、`Result[T, E]`。
- `std.array` 有 canonical length-generic access/query/count/aggregate helpers。
- 数组长度泛型参数可作为 `Int` 值读取。
- dependency generic bridge 支持 wrapper specialization 内继续直调同模块 generic helper。
- 单文件和 project 入口共用本地 generic free function direct-call specialization。
- `std.test` 已有普通断言和 length-generic 数组断言。
- `ql project init --stdlib` 已生成可 `check/run/test` 的模板。

## 下一步顺序

1. 修语言和后端能力，减少 stdlib 为绕路而写的固定 arity API。
2. 把剩余数组 helper 迁移到 canonical length-generic API。
3. 为每个 public stdlib API 补 package-local 测试和 downstream consumer smoke。
4. 扩 method/value generic import 和非 direct-call generic 值前，先补清楚 monomorphization contract。

## 规则

- 不新增 `foo3/foo4/foo5` API，除非它只是兼容层并立即解锁真实 smoke。
- 不把测试 helper 当作标准库 API。
- 不把 variadic 写进 stdlib 文档，直到语言语法和后端都落地。
- 实现未通过 downstream `ql check/build/run/test` 前，不宣称可用。

## 验证

```powershell
cargo run -q -p ql-cli -- project targets stdlib
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- test stdlib
```
