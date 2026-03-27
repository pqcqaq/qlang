# Phase 6 Field Rename Shorthand Expansion

## 背景

在同文件 rename 已经支持 item / local / generic / import 之后，field 仍然是一个悬空缺口：

- field declaration、member access、显式 struct literal / pattern 字段标签已经进入统一 query surface
- 但 field rename 仍然关闭

真正的卡点不在显式字段标签，而在 shorthand：

- `Point { x }`
- `Point { x } => ...`

这类 token 在 hover / definition 语义上仍然应该落在 local/binding 上，否则会和已有 local 查询面冲突；但如果 field rename 完全忽略这些 site，又会留下不完整的重命名结果。

## 目标

这一刀的目标很窄：

- 打开 local struct field 的 same-file rename
- 保持 shorthand token 的 hover / definition 语义不变
- 在 rename 时把 shorthand site 自动扩写成显式标签

明确不做：

- method rename
- receiver rename
- builtin type rename
- 从 shorthand token 本身直接触发的 field rename
- cross-file rename

## 设计

### 1. Query Surface 不改语义归属

`Point { x }` 里的 `x` 仍然是 local/binding symbol，不把它强行改成 field occurrence。

原因很直接：

- 这个 token 同时承担 field label 和 local/binding 两个角色
- 如果直接把它改成 field occurrence，会破坏现有 local hover / definition / rename

所以这次不改 hover truth，只改 rename edit 生成。

### 2. Field Rename 额外收集 shorthand rewrite

`QueryIndex` 现在除了普通 occurrence 之外，还会为 local struct field 额外记录：

- shorthand struct literal site
- shorthand struct pattern site

这些 site 不进入普通 references 分组，而是作为 rename-only rewrite 使用。

当用户从 source-backed field symbol 发起 rename 时：

1. 先复用原有 field occurrence edits
2. 再追加 shorthand rewrite edit
3. 把 `x` 重写为 `coord_x: x`

这样既保证 rename 结果完整，又不需要改动现有 shorthand token 的 hover 语义。

## 测试

这次切片补了两层回归：

- `crates/ql-analysis/tests/queries.rs`
  - 显式字段标签现在可以直接 prepare/rename
  - local struct field rename 会自动把 shorthand literal / pattern site 扩写成显式标签
  - shorthand token 自身仍保持 local hover
- `crates/ql-lsp/tests/bridge.rs`
  - field rename 的 workspace edit 会包含 shorthand expansion

验证命令：

```bash
cargo fmt --all
cargo test -p ql-analysis --test queries
cargo test -p ql-lsp --test bridge
cargo test
cargo clippy --workspace --all-targets -- -D warnings
cd docs
npm run build
```

## 结果与边界

现在 local struct field 已经进入 same-file rename 集合。

但仍然有几个边界保持保守：

- shorthand token 本身仍然是 local/binding query 入口
- references 结果当前不会把 shorthand token 作为 field occurrence 展示
- method rename 仍未开放
- cross-file rename 仍未开放

也就是说，这一刀解决的是“field rename 终于能完整改动同文件里常见的 shorthand site”，不是“field query surface 已经完全统一”。
