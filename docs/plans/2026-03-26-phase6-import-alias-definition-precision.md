# Phase 6 Import Alias Definition Precision

## 背景

在 Phase 6 已经补上 same-file references、variant/member/field precision 和保守 rename 脚手架之后，import alias 仍然停留在“字符串级伪符号”状态：

- hover 能工作
- 同文件 references 能按导入路径分组
- 但没有本地 definition span
- 也不能进入同文件 rename

这会留下一个明显不一致点：

- `Map`、`Counter`、`Retry`、`value` 等名字已经有 source-backed identity
- 只有 import alias 还停留在 `SymbolKey::Import(String)` 这种弱语义层次

如果继续在这个状态上扩 completion 或跨文件 rename，后面还得回头重做 import identity。

## 目标

这一刀只解决一个问题：

- 让 import alias 成为 source-backed symbol

具体要求：

- parser 保留 import name / alias 的精确 span
- resolver 不再只返回裸 `Path`，而是返回带定义点的 `ImportBinding`
- `ql-analysis::QueryIndex` 用 `ImportBinding` 作为统一 symbol key
- LSP 直接复用 analysis 结果获得 hover / definition / references / rename

明确不做：

- 跨文件 rename
- 更深 module-path 语义
- imported struct field identity
- ambiguous import / prelude / package graph 规则

## 设计

### AST / Parser

`UseDecl` 与 `UseItem` 现在补上：

- `alias_span`
- `name_span`

这样 grouped import 与 direct import 都能在语法层拿到真实定义 token，而不是后续靠字符串搜索回推。

### Resolver

新增 `ImportBinding`：

- `path`
- `local_name`
- `definition_span`

`seed_imports` 在 module scope 里先构造 `ImportBinding`，再同时塞进值命名空间和类型命名空间。

这带来两个收益：

- import alias 的值/类型 resolution 共享同一份 source-backed 身份
- query/typeck/MIR 不再需要自行猜“这个 import 的定义点应该在哪里”

### Query Index

`SymbolKey::Import` 现在从 `String` 升级为 `ImportBinding`。

`QueryIndexBuilder` 先索引 import definition，再索引 use-site。

这样 import alias 会自然获得：

- declaration occurrence
- use occurrence
- definition target
- references grouping
- rename edit 集合

并且仍然保持和其他语义查询一样的行为：所有 IDE 能力都只消费 `QueryIndex`，不会在 LSP 层再写一套 import 语义遍历。

## 测试

这次切片补了四层回归：

- parser：import name / alias span
- resolver：type/value import resolution 返回 `ImportBinding`
- analysis：import hover / definition / references / rename
- LSP：prepare rename / rename workspace edit

验证命令：

```bash
cargo fmt --all
cargo test -p ql-resolve -p ql-analysis -p ql-lsp
cargo clippy -p ql-resolve -p ql-analysis -p ql-lsp --all-targets -- -D warnings
cargo test
cargo clippy --workspace --all-targets -- -D warnings
cd docs
npm run build
```

## 结果与边界

现在 import alias 已经进入同文件 source-backed rename 集合。

仍然刻意不开放的内容：

- field rename
- method rename
- receiver rename
- builtin type rename
- shorthand field rename
- cross-file rename
- package-level import graph 语义

也就是说，这一刀补的是“import alias 终于和其他 source-backed symbol 站在同一层”，不是“完整模块系统已经做好”。
