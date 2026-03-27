# Phase 6 Member Completion

## 背景

lexical-scope completion 落地之后，`textDocument/completion` 已经不再是空壳，但仍然只覆盖：

- value scope
- type scope

这意味着像 `counter.read()` 这种已经成功解析、并且 receiver type 稳定的 member token，仍然拿不到真实候选。

这一步不应该直接跳去做“`obj.` 也能补”的 parse-error tolerant completion，因为当前 parser / HIR / query surface 还没有为 incomplete member token 建立可靠语义边界。更稳的路径是先把“已经解析成功的 member token”接进统一 completion 数据面。

## 目标

本次切片要完成：

- 在 `ql-analysis` 中支持 parsed member-token completion
- 只基于稳定 receiver type 产出候选
- 继续复用 `SymbolData`
- 保持 impl / extend / field 的现有优先级

本次切片不做：

- `obj.` 这类 incomplete member token completion
- ambiguous member completion
- cross-file completion
- method rename

## 设计

`QueryIndex` 在原有 lexical-scope completion 之外，新增一组 member completion site。

每个 site 只记录：

- member token 的 span
- 该 span 上可安全暴露的 completion items

候选生成算法：

1. 找到 `ExprKind::Member`
2. 读取 receiver expression 的 `Ty`
3. 只有 `Ty::Item` 才继续
4. 先收集 impl method
5. 再收集 extend method
6. 最后补 struct field
7. 同名多 candidate 直接丢弃，不伪造 ambiguous completion
8. 同名 method / field 冲突时继续沿用当前 member 选择优先级，让 method 覆盖 field

## 已落地结果

- `ql-analysis::completions_at(offset)` 现在会优先命中 member completion site
- `qlsp` 的 `textDocument/completion` 现在不仅支持 lexical scope，也支持 parsed member token
- LSP bridge 继续只做前缀过滤和 text edit 生成

## 新增回归

- analysis：稳定 receiver type 上同时返回 field + method 候选
- analysis：impl method 优先于 extend method，ambiguous extend candidate 被跳过
- LSP：member completion 的前缀过滤与 text edit

## 当前边界

这一步之后，completion 的现状是：

- same-file lexical scope：已支持
- same-file parsed member token：已支持
- ambiguous member completion：未支持
- parse-error tolerant dot-trigger completion：未支持
- cross-file completion：未支持

## 下一步

更合理的后续方向是：

1. 补 module-path / import graph 上的 completion 精度
2. 研究 parse-error tolerant member completion，需要 parser / HIR / query surface 先给出稳定边界
3. 在 project/package indexing 建立之后，再讨论 cross-file completion / rename
