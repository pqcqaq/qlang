# 2026-03-27 P2 Array Type Syntax

## Goal

将设计稿里已经公开使用的 fixed array type expr `[T; N]` 真正落到当前前端与语义链路，避免文档与实现继续分叉。

## Scope

- AST 新增 `TypeExprKind::Array`
- parser 支持 `[T; N]`
- formatter 保留数组长度源码文本
- HIR 新增 `TypeKind::Array`
- HIR lowering 将长度字面量转成统一语义长度
- resolver 递归解析数组元素类型
- `ql-typeck` 将声明数组类型 lowering 成 `Ty::Array`
- `ql-analysis` / query / hover 渲染数组类型签名

## Constraints

- 只做 fixed array type expr，不引入 slice / dynamic array / index protocol 抽象
- 保留 AST 中的长度源码文本，避免 formatter 把 `0x3`、`0b11` 之类字面量强制改写成十进制
- HIR 和 typeck 只消费已经标准化的语义长度，避免后续语义层重复解释源码字面量
- 继续保持保守边界：数组类型可声明、可比较、可索引；更一般的索引协议仍延后

## Tests

- parser: 函数参数 / 返回类型中的 `[Int; 0b11]` / `[String; 0x1]`
- formatter: `[Int; 0x3]` round-trip 稳定
- resolver: 数组元素 builtin type 递归解析，长度 lowering 为 `usize`
- typeck: 声明数组参数接受匹配字面量
- typeck: 数组长度不匹配时给出稳定 mismatch diagnostic
- analysis/query: hover 函数签名正确渲染 `[Int; 3]`
