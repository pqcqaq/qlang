# 2026-03-27 P2 Array Literal Guidance And Index Normalization

## Goal

在 fixed array type expr 已经落地之后，继续补齐 first-pass typing 的两个一致性问题：

1. expected fixed-array context 应该能直接约束 array literal，而不是只在外层报一个笼统的 array mismatch；
2. constant tuple indexing 应该与语言其他整数字面量语义保持一致，支持 `0x` / `0b` / `0o` / `_`。

## Scope

- `ql-typeck` 在 `check_expr` 中把 expected array type 继续传入 array literal
- `check_array_literal` 复用 expected fixed-array element type 做 item checking
- tuple constant indexing 改为复用统一的 lexer-style integer literal parser
- `ql-analysis` 暴露 tuple hex index 的稳定 expr/local type

## Constraints

- 不引入 slice / dynamic array / general index protocol
- 不提前做更完整的 array constraint solving
- 继续保持 `unknown` 作为错误恢复阀门，但在已有 expected type 时尽量减少外层级联 mismatch

## Tests

- typeck: `pair[0x1]` 作为 constant tuple index 正常通过
- typeck: `[Int; 2]` expected context 下，`["x", "y"]` 直接报 array literal item mismatch
- analysis: tuple hex index 的 expr/local type 稳定为被索引元素类型
- 继续保留既有 declared array length mismatch 回归
