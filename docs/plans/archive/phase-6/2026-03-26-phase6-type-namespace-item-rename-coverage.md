# Phase 6 Type-Namespace Item Rename Coverage

## 背景

Phase 6 的 same-file rename 从一开始就把一组 item-kind 作为“可安全开放”的范围：

- `type alias`
- `struct`
- `enum`
- `trait`

实现上，这些名字都复用同一份 `QueryIndex` item identity，并通过 named-type-root / item occurrence 进入 rename surface。问题不在于功能缺失，而在于之前没有一组明确的 analysis / LSP regression 去把这四类 item-kind 端到端锁住。

如果没有这组回归，后续继续调整 query surface、occurrence 记录或 LSP bridge 时，很容易出现“某一类 item rename 在 analysis 里还是对的，但在协议桥接里悄悄少改一个 use-site”的静默回退。

## 本次目标

- 不新增新的 rename kind
- 不扩张到 cross-file / module graph
- 不修改 `QueryIndex` 的语义边界
- 只补齐 `type alias` / `struct` / `enum` / `trait` 的 same-file rename regression coverage

## 实现方式

直接复用现有实现：

1. 在 `ql-analysis/tests/queries.rs` 增加一组 type-namespace item rename regression
2. 在 `ql-lsp/tests/bridge.rs` 增加对应的 prepare-rename / rename workspace-edit regression
3. 文档同步把这组 item-kind 从“文档声明已支持”提升为“文档声明且已有端到端回归保护”

## 非目标

- field / variant 之外的新 member-like symbol
- cross-file rename
- import-graph rename
- ambiguous member rename
- shorthand field token 的 field-symbol rename

## 验证

- `cargo test -p ql-analysis --test queries rename_queries_follow_supported_type_namespace_item_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge rename_bridge_supports_type_namespace_item_symbols -- --exact`
- 后续补全 `cargo fmt --all`
- 后续补全 `cargo test`
- 后续补全 `cargo clippy --workspace --all-targets -- -D warnings`
- 后续补全 `npm run build`

## 结果

完成后，Phase 6 同文件 rename 的“已开放 item-kind”会更明确：

- `type alias` / `struct` / `enum` / `trait` 都有 analysis + LSP 的显式 regression protection
- 这组 rename 继续完全复用统一 `QueryIndex`
- 后续扩 query / LSP 时，可以更放心地在不破坏既有 item rename 的前提下推进
