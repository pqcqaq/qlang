# Phase 6 Variant Query Precision

## 目标

在已经补上 member query precision 之后，把 enum variant declaration / pattern use / constructor use 也纳入同一套 query surface。

这一步仍然不是直接做 rename 或 completion，而是继续补强“符号 identity + 精确 token span”这条地基。没有这层稳定事实，后续 IDE 能力只会不断堆特判。

## 这一步解决的真实缺口

前一轮已经能稳定处理：

- struct field member token
- unique impl / extend method member token

但 enum variant 仍然停留在“root enum item 可以被解析，variant 自己没有稳定 identity”的状态。结果是：

- `Command.Retry(...)` 这类 constructor use 只能查到 `Command`
- `Command.Config { ... }` 这类 struct-variant literal 只能查到 enum root
- `match command { Command.Retry(v) => ... }` 这种 pattern use 也不能精确落到 `Retry`

所以这一轮不是“补一个 hover case”，而是把 variant 也放进统一 occurrence/indexing 模型里。

## Path 位置信息基础

为了避免继续靠 source slice 猜 token，本轮先给 `ql_ast::Path` 补上 `segment_spans`：

- parser 现在会在 `parse_path()` 里记录每个 segment 的独立 span
- resolver 里从表达式重建 path 时，也会保留 name/member 的 segment span
- root-only 的 synthetic path 仍允许使用 `Path::new(...)` 构造，span 默认为 empty，不破坏现有辅助构造器

这让 query 层可以同时拿到：

- root segment span
- tail segment span

后续如果继续做 module-path deeper query，这层基础还可以继续复用。

## 查询层落地

`ql-analysis::QueryIndex` 现在新增：

- `SymbolKind::Variant`
- `SymbolKey::Variant`
- enum variant definition indexing
- enum variant use indexing，当前覆盖：
  - `Command.Retry(...)`
  - `Command.Config { ... }`
  - `Command.Retry(...)` / `Command.Config { ... }` 形式的 pattern use

索引策略保持和已有设计一致：

- root enum item 仍然保留自己的 occurrence
- variant tail token 再追加一个更窄的 occurrence
- `symbol_at(offset)` 通过 span 排序优先命中更窄 token，所以光标落在 `Retry` / `Config` 上时会得到 variant，而不是 enum root

## 当前边界

这一步依然故意不宣称完整 path 语义已完成：

- 还没做 module-path deeper query
- 还没做 import alias 下的跨模块 variant 精确映射
- 还没做 rename / completion
- ambiguous method/member 仍保持保守策略

也就是说，这一轮补的是“source-backed enum variant precision”，不是“完整路径系统”。

## 回归覆盖

新增/更新回归包括：

- parser fixture：path segment span 会精确保留到 variant pattern / variant struct literal
- analysis query：variant declaration / constructor use / pattern use 会共享 definition 与 references
- LSP bridge：variant hover markdown 会显示 `variant` kind、detail 与 enum type

## 后续建议

下一步更合理的方向不是跳去做跨文件 rename，而是继续沿着同一条数据面推进：

1. module-path deeper query
2. ambiguous member handling strategy
3. rename/completion scaffolding，直接复用 `QueryIndex`
