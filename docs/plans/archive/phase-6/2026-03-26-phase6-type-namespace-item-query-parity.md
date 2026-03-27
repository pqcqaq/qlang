# Phase 6 Type-Namespace Item Query Parity

## 背景

在补完 `type alias` / `struct` / `enum` / `trait` 的 same-file rename coverage 之后，Phase 6 还剩一个很典型的一致性缺口：

- rename 已经有 analysis + LSP 的端到端回归
- 但 references / semantic tokens 还没有针对这组 type-namespace item 的显式 parity regression

实现上，这组 item 本来就复用同一份 `QueryIndex` item identity。问题不是“功能不存在”，而是如果没有回归锁住，后续继续调整 occurrence indexing 或 LSP bridge 时，很容易出现 query 还能跳转、但 references 少一个 use-site，或者 semantic token 分类悄悄退化的静默回退。

## 本次目标

- 不新增新的 symbol kind
- 不修改 QueryIndex 的语义边界
- 不扩张到 cross-file / module graph
- 只补齐 `type alias` / `struct` / `enum` / `trait` 的 same-file references / semantic-token parity coverage

## 实现方式

直接复用现有实现：

1. 在 `ql-analysis/tests/queries.rs` 增加 type-namespace item references regression
2. 在 `ql-analysis/tests/queries.rs` 增加 type-namespace item semantic-token regression
3. 在 `ql-lsp/tests/bridge.rs` 增加 references bridge regression
4. 在 `ql-lsp/tests/bridge.rs` 增加 semantic token bridge regression
5. 文档同步把这组 parity 从“隐含存在”提升为“显式声明且有回归保护”

## 调试记录

这一步还顺带确认了一个现有行为边界：

- enum item references 不只覆盖 return-type uses
- `Mode.Ready` 里的 enum root `Mode` 也已经进入同一份 item identity

因此 references 回归最终按真实 occurrence 面锁成 5 个 enum references，而不是 4 个。

## 非目标

- 新的 completion surface
- cross-file references / semantic tokens
- module-path deeper semantics
- shorthand field token 的 field-symbol 语义
- ambiguous member / parse-error tolerant surface

## 验证

- `cargo test -p ql-analysis --test queries type_namespace_item_reference_queries_follow_same_file_identity -- --exact`
- `cargo test -p ql-analysis --test queries semantic_tokens_follow_type_namespace_item_surface -- --exact`
- `cargo test -p ql-lsp --test bridge references_bridge_follow_type_namespace_item_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge semantic_tokens_bridge_maps_type_namespace_item_surface -- --exact`
- 后续补全 `cargo fmt --all`
- 后续补全 `cargo test`
- 后续补全 `cargo clippy --workspace --all-targets -- -D warnings`
- 后续补全 `npm run build`

## 结果

完成后，Phase 6 里 `type alias` / `struct` / `enum` / `trait` 这组 type-namespace item 会形成更完整的 same-file parity：

- rename 已锁住
- references 已锁住
- semantic tokens 已锁住

这样后续继续扩 query / LSP 时，可以更稳地依赖这组 item identity，而不是每次都担心编辑器层和 analysis 层走散。
