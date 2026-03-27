# 2026-03-27 P2 Comparison Compatibility Tightening

## Goal

把 first-pass binary operator 规则再收紧一小步，让 comparison 与 arithmetic 的 numeric 兼容性边界保持一致。

## Scope

- 为 comparison operator 增加同类型 numeric compatibility 要求
- 保持 `unknown` 的错误恢复路径不扩张
- 抽出一层小的 numeric compatibility helper，避免 arithmetic / comparison 规则继续分叉

## Why now

当前 arithmetic 已经要求 numeric operands 兼容，但 comparison 仍然只要求“两边都是 numeric”，这会让 `Int < UInt` 之类的表达式在 first-pass typing 里被错误接受，边界不一致。

## Tests

- compatible numeric comparison 通过
- incompatible numeric comparison 失败，并给出稳定 diagnostic
- 既有 equality / bool-condition 回归继续通过

## Constraint

- 不引入自动 numeric promotion / coercion
- 不改变 `unknown` 的恢复策略
- 只修正现有 first-pass rule 的一致性
