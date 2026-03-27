# Phase 6 Find References Query Surface

## 目标

在不重写现有 `ql-analysis` 查询层与 `qlsp` 桥接层的前提下，补齐同文件 `find references` 能力。

这一步的核心不是“把 LSP 请求接上”，而是先给 query index 一个稳定、可扩展的 symbol identity 模型。否则后续 rename、completion、member 语义继续扩展时，很容易把位置查询做成一堆互相复制的特判。

## 设计约束

- 语义 occurrence 的索引和归并必须留在 `ql-analysis`
- `ql-lsp` 只做协议桥接，不重复遍历 HIR / resolve / typeck
- 当前只承诺同文件 references
- 不伪造 import alias / builtin type 的 definition span
- 不借这一步提前宣称 member / method / variant / module-path 查询已完整

## Query Identity 设计

`QueryIndex` 继续保留“先登记定义、再登记 use-site”的两阶段结构，但每个 occurrence 新增稳定 `SymbolKey`：

- `Item(ItemId)`
- `Function(FunctionRef)`
- `Local(LocalId)`
- `Param(ParamBinding)`
- `Generic(GenericBinding)`
- `SelfValue(ScopeId)`
- `BuiltinType(BuiltinType)`
- `Import(String)`
- `DefinitionSpan(Span)`

为什么需要 `DefinitionSpan(Span)`：

- 顶层 item / extern block function 已有稳定语义 ID
- trait / impl / extend method declaration 当前还没有统一的 resolver identity
- 但这些 declaration 仍需要在 query index 中自洽地被索引
- 用 declaration span 作为保守 fallback，可以避免为了 references 过早重构 resolver / HIR identity

## 查询算法

当前算法保持克制：

1. `index_definitions`
   - 先登记 item / function / param / generic / self / local 的定义 occurrence
2. `index_uses`
   - 再遍历 type / pattern / expr，把 use-site 绑定到前面登记的 `SymbolData`
3. 为每个 occurrence 保存 `SymbolKey`
4. 统一按 `(span.len(), span.start, span.end)` 排序
5. `symbol_at(offset)` / `definition_at(offset)` / `references_at(offset)` 都先命中最窄 span
6. `references_at(offset)` 通过 `SymbolKey` 收集同组 occurrence，按源码顺序排序，并按 span 去重

这样做的收益：

- hover / definition / references 共用同一份语义索引
- 后续 rename 可以直接复用 references 结果，而不是再写一套“找同名符号”逻辑
- import alias / builtin type 这类“无本地定义点”的实体仍可稳定参与 same-file references

## LSP 桥接

`qlsp` 的改动只做三件事：

- capability 宣告 `references_provider`
- 新增 `textDocument/references`
- 在 `bridge.rs` 中把 `analysis.references_at(offset)` 转成 `Vec<Location>`

协议层额外处理只有一项：

- 尊重 `ReferenceContext.includeDeclaration`

这保持了当前架构边界：

- compiler 负责语义 identity 和 occurrence 分组
- LSP 负责 UTF-16 position、range/location、includeDeclaration 过滤

## 回归覆盖

新增回归重点覆盖三类场景：

- parameter / local：definition + use-site 成组返回
- extern block function：declaration + call-site 成组返回
- import alias：无 definition span，但多个 use-site 仍能稳定归组

LSP bridge 回归额外锁定：

- `includeDeclaration = true`
- `includeDeclaration = false`

## 当前仍不做

这一步故意不扩到：

- 跨文件 references
- rename
- completion
- member / method / variant / module-path 的更深语义查询
- import declaration 自身的 source-backed occurrence

这些都需要更完整的模块图、符号导出面与更细粒度语义 identity，不能为了一个 references 能力在这一刀里提前耦死。
