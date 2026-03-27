# Phase 6 Import-Alias Struct Field Hardening

## 背景

Phase 6 之前已经把 `local import alias -> local struct item` 接进了 struct field 的 truth surface：

- explicit field label 可以 hover / definition / references / rename
- field-driven shorthand rename 会继续映射回原 struct field symbol
- semantic tokens 理论上也会沿同一份 occurrence 数据自然成立

问题不在于“功能不存在”，而在于这条承诺在 analysis / LSP 两层的回归覆盖还不够完整。当前如果不把这些路径用测试锁住，后续继续改 query index、rename rewrite 或 LSP bridge 时，很容易无意间把 import-alias struct field 的 follow-through 打回只支持 query、不支持 references 或 semantic tokens 的半完成状态。

## 本次目标

- 不扩张语义边界
- 不引入 cross-file / import-graph 行为
- 只把现有 `local import alias -> local struct item` field truth surface 的回归覆盖补齐

具体要锁住的能力：

- analysis semantic tokens 会继续包含 import-alias struct field definition / explicit label / member use
- LSP references 会继续把 import-alias struct field label 映射回原 field definition
- LSP prepare rename / rename 会继续复用 analysis 的 field identity
- LSP semantic tokens 会继续把 import-alias struct field label 编码成 property token

## 实现方式

不新增新的 analysis 数据结构，也不在 LSP 层加任何 heuristics。

实现策略保持非常保守：

1. 继续把 `QueryIndex` 当作唯一语义真相源
2. 使用已有 explicit struct field label follow-through
3. 直接在 `ql-analysis/tests/queries.rs` 增加 semantic token regression
4. 直接在 `ql-lsp/tests/bridge.rs` 增加 references / prepare-rename+rename / semantic token regression
5. 同步阶段文档和计划索引，明确这一步是在补齐 coverage，而不是开放新的 symbol kind

## 非目标

- shorthand field token 本身切换为 field symbol
- foreign import alias struct field 语义
- cross-file references / rename / semantic tokens
- module graph / project index
- parse-error tolerant field completion

## 验证

- `cargo fmt --all`
- `cargo test -p ql-analysis --test queries`
- `cargo test -p ql-lsp --test bridge`
- `cargo test`
- `npm run build`

## 结果预期

完成后，`local import alias -> local struct item` 这条路径在 analysis / LSP 的 field surface 会更一致：

- query、references、rename、semantic tokens 都继续站在同一份 field identity 上
- 文档会明确这是“已实现并有回归保护”的能力
- 后续如果要继续扩 query/LSP，也可以在不回退这条路径的前提下推进
