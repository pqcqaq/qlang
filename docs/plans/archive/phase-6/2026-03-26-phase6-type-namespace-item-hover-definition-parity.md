# Phase 6 Type-Namespace Item Hover Definition Parity

## 背景

在补完 `type alias` / `struct` / `enum` / `trait` 的 same-file rename，以及 references / semantic-token parity 之后，Phase 6 对这组 type-namespace item 还缺最后一块很基础的 editor-facing 回归：

- hover
- definition

实现上，这些能力早就已经站在同一份 `QueryIndex` item identity 上。问题只是没有显式 regression 去证明 analysis 和 LSP 仍然会把这组 item 映射回同一份 definition span 和 detail 文本。

如果不把这层锁住，后续继续调整 hover rendering 或 definition bridge 时，很容易出现“references / rename 还是对的，但 hover/detail 或 go-to-definition 悄悄漂移”的静默回退。

## 本次目标

- 不新增新的 symbol kind
- 不扩张到 cross-file / module graph
- 不修改 `QueryIndex` 或 LSP bridge 的语义边界
- 只补齐 `type alias` / `struct` / `enum` / `trait` 的 same-file hover / definition parity coverage

## 实现方式

直接复用现有实现：

1. 在 `ql-analysis/tests/queries.rs` 增加 type-namespace item hover + definition regression
2. 在 `ql-lsp/tests/bridge.rs` 增加对应的 hover markdown + definition location regression
3. 文档同步把这组 parity 从“隐含存在”提升为“显式声明且有回归保护”

## 非目标

- 新的 completion / rename 能力
- cross-file definition
- module-path deeper semantics
- shorthand field token 的 field-symbol 语义
- ambiguous member / parse-error tolerant surface

## 验证

- `cargo test -p ql-analysis --test queries hover_and_definition_queries_follow_type_namespace_item_symbols -- --exact`
- `cargo test -p ql-lsp --test bridge hover_and_definition_bridge_follow_type_namespace_item_symbols -- --exact`
- 后续补全 `cargo fmt --all`
- 后续补全 `cargo test`
- 后续补全 `cargo clippy --workspace --all-targets -- -D warnings`
- 后续补全 `npm run build`

## 结果

完成后，Phase 6 里 `type alias` / `struct` / `enum` / `trait` 这组 type-namespace item 在 same-file 场景下会形成更完整的 parity：

- hover 已锁住
- definition 已锁住
- references 已锁住
- semantic tokens 已锁住
- rename 已锁住

后续继续扩 query / LSP 时，可以更稳地依赖这组 item identity，而不是反复担心导航、悬浮和重命名各走一套真相源。
