# Stdlib Generics and Collections Roadmap

## Current status

`stdlib` 现在已经是可被真实项目消费的普通 workspace：

- `std.core`、`std.option`、`std.result`、`std.array`、`std.test` 都已存在
- generic `Option[T]` / `Result[T, E]` 已可执行
- `std.array` 已有 canonical length-generic access/query/count/aggregate helpers
- `std.array` 固定长度 access/query/count helper 已收敛为转调 canonical API 的兼容层
- dependency generic bridge 已支持 wrapper specialization 体内继续转调同模块 generic helper
- `std.test` 已提供 length-generic 数组断言 helpers，并用 downstream smoke 覆盖不同长度实例
- `ql project init --stdlib` 已能生成能直接跑 `check/build/run/test` 的项目模板

concrete `IntOption` / `BoolOption`、`IntResult` / `BoolResult`、3/4/5 fixed-arity helper 只是兼容面，不再是主方向。

## Next

1. 继续把 generic public API 的执行面补完整，优先修真正影响下游 smoke 的缺口。
2. 继续把 `std.array` 剩余 `reverse` / `repeat` 固定长度过渡面往 canonical length-generic API 收敛。
3. 继续把项目模板和 downstream smoke 保持在同一套真实 contract 上。
4. 继续把 method/value generic import、完整 monomorphization 留在明确的后续阶段。

## Rules

- 不再为新的 `foo3/foo4/foo5` helper 扩面，除非它能立即解锁 downstream smoke。
- 不把 generic stdlib API 写成“支持中”直到它能通过 downstream `ql check/build/run/test`。
- 如果 stdlib 暴露编译器/后端缺口，优先修编译器/后端，而不是继续降低库设计。
- 每个 stdlib 变更都要带 package-local 测试和至少一个 downstream consumer 测试。
- variadic syntax 是单独的语言设计门，不要在 stdlib 文档里把它伪装成已完成能力。

## Verification

```powershell
cargo run -q -p ql-cli -- project targets stdlib
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- test stdlib
```

## Migration direction

- generic APIs 逐步成为主路径
- concrete APIs 只保留兼容面
- collection APIs 代替重复参数复制
- variadic 设计等单独 gate 再进入实现
