# 2026-03-27 P2 Array And Index Typing

## 背景

- 语法设计已经明确承诺数组字面量与索引访问。
- `ql-typeck` 之前对 `ExprKind::Array` 与 `ExprKind::Bracket` 只做递归检查后退化成 `Ty::Unknown`。
- 这会让局部类型推断、`ql-analysis` 类型查询和后续索引协议设计都缺少一个稳定的最小语义面。

## 目标

- 在不引入完整索引协议的前提下，为数组字面量与一层 tuple/array 索引补上 first-pass typing。
- 保持实现可扩展，不把未来 user-defined indexing / map / string indexing 提前锁死。

## 本次范围

已实现：

- 内部 `Ty::Array { element, len }`
- homogeneous array literal inference
- array literal item type mismatch diagnostics
- array indexing 的 element projection
- array index 必须为 `Int` 的 first-pass diagnostics
- constant tuple indexing
- tuple constant index out-of-bounds diagnostics
- `ql-analysis` 暴露这组新的 expr/local type 查询结果

刻意不实现：

- 源码层数组 type expr（例如 `[T; N]`）的 parser / HIR / lowering
- map / string / user-defined index protocol
- dynamic tuple index 的完整语义
- multi-index / slice 语法语义

## 设计

### `Ty::Array`

- 只作为当前语义层的内部类型表示使用。
- 这一步先服务 local/expr typing、diagnostics 和 analysis query，不要求源码类型语法已经完备。

### Array literal inference

算法：

1. 顺序检查每个元素表达式。
2. 选取当前第一个非 `Unknown` 元素类型作为候选 element type。
3. 后续元素若与候选不兼容，报 `array literal item has type mismatch`。
4. 最终返回 `Ty::Array { element, len }`。

### Bracket typing

- `Ty::Array`
  - 单 index 时要求 index 类型为 `Int`
  - 返回 element type
- `Ty::Tuple`
  - 仅当 index 是整数字面量时做 constant tuple indexing
  - 越界时报 tuple out-of-bounds
  - 动态 tuple index 继续返回 `Unknown`
- 其它 target type
  - 继续保守退化为 `Unknown`
  - 不提前宣称完整 index protocol

## 测试

- `crates/ql-typeck/tests/typing.rs`
  - array literal + array indexing 正向
  - tuple constant indexing 正向
  - array literal item mismatch
  - non-int array index
  - tuple out-of-bounds
  - dynamic tuple indexing defer boundary
- `crates/ql-analysis/src/lib.rs`
  - array/index expr 与 local type query 暴露

## 验证

- `cargo test -p ql-typeck --test typing`
- `cargo test -p ql-analysis --lib exposes_array_and_index_types_for_queries`
- `cargo test`
- `npm run build` in `docs`

## 后续安全下一步

- 若继续补表达式 typing，可优先考虑：
  - 通用 index protocol 边界设计
  - 更多 binary operator 规则
  - `unknown` / deferred constraint 收紧
- 在这之前，不要把 arbitrary non-array target 的 bracket 访问强行升级成硬错误。
