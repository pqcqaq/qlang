# Phase 8：`.qi` 接口产物与 Cross-File LSP 设计入口

## 目标

把 Phase 8 的顺序固定下来，避免直接从 same-file LSP 跳到“全量 workspace IDE”。

这轮的核心判断只有一条：

- 先做 `.qi` 接口产物和 package/workspace graph。
- 再让 `ql-analysis` / `ql-lsp` 消费 `.qi`。
- 最后再做 cross-file rename / code actions。

如果顺序反过来，LSP 会先长出一套只依赖源码扫描的临时语义，后面再接 `.qi` 时基本必然返工。

## 设计结论

### 1. `.qi` 是 Phase 8 的第一真相源

`.qi` 不是“为了像 TypeScript 那样有 declaration emit”，而是为了给 Qlang 建立稳定的公共接口边界。

V1 采用：

- 每个 package/module 输出可确定排序的文本型 `.qi`
- 语法风格保持 Qlang 自身表面，而不是 Rust/JSON 专用格式
- 只承载 public API 所需的类型与符号元数据
- 编译器、`ql-analysis`、`ql-lsp` 共享同一份 `.qi` 读取逻辑

V1 `.qi` 应覆盖：

- public `fn` / `extern "c"` callable signature
- public `struct` / `enum` / `trait` / `type alias`
- generic parameter surface
- public field / variant shape
- hover 需要的基础 doc/comment 文本

V1 明确保守关闭：

- function body
- impl body / method body
- cross-package const-eval 全量语义
- public `static` runtime layout / address 语义
- macro-like generated declarations

### 2. Cross-file LSP 必须建立在 `.qi` 之上

Phase 6 已经把 same-file truth surface 收口到 `QueryIndex -> LSP bridge` 这条线。Phase 8 不应该重新发明第二套跨文件语义。

因此新的顺序应是：

1. `qlang.toml` / workspace member / package reference graph
2. `.qi` emit 与 load
3. `ql-analysis` 扩成 “当前源码包 + 依赖 `.qi`” 的 query truth surface
4. `ql-lsp` 扩成 cross-file hover / definition / completion / references
5. 在 identity 与 workspace edit graph 稳定后，再做 cross-file rename

LSP V1 只开放：

- cross-file hover
- cross-file go to definition
- cross-file completion
- cross-file references

LSP V1 继续关闭：

- cross-file rename
- code actions
- call hierarchy
- project-wide semantic rewrite

### 3. `.qi` 要服务编译，不只服务编辑器

`.qi` 不能被设计成“LSP 专用缓存文件”。它首先是 compiler-facing interface artifact，其次才是 LSP 的依赖输入。

这意味着：

- `ql check` / `ql build` 可以消费依赖包 `.qi`
- dependency invalidation 先基于 `.qi` 变化判断
- `ql doc` 后续也应优先复用 `.qi` 的公共 API 面
- LSP 对依赖包 hover/completion 不需要重新解析全部源码

这样 Phase 8 的几条线才不会各自维护一份 package public surface。

## 当前推荐实现顺序

### Task A：package / workspace graph

先把 `qlang.toml` 的 package、workspace members、project references 语义固定下来。

交付目标：

- workspace member graph
- package name / root / dependency identity
- 一个明确的“当前包源码优先、依赖包 `.qi` 次之”的加载模型

### Task B：`.qi` emit V1

先打通最小 public interface emit，不碰更深的 cross-package const/static 语义。

已完成（2026-04-05）：

- `ql project emit-interface [file-or-dir] [-o <output>]` 已落地
- 当前仅支持 package manifest，默认扫描 `src/**/*.ql`
- emit 前逐文件走 `ql-analysis`，有 diagnostics 就失败
- `.qi` 当前输出 text-based public surface section，保留 public declaration 形状，去掉 body / value / field default

交付目标：

- `ql build` 或独立子命令能产出 `.qi`
- `.qi` 文本稳定、可排序、可快照测试
- 失败合同明确：当前无法进入 `.qi` 的 public surface 应显式报错，而不是静默丢失

### Task C：analysis package loader

把 `ql-analysis` 从 same-file `analyze_source` 扩成可消费 package graph 和 `.qi` 的更高层入口，但不推翻当前 same-file query API。

已完成（2026-04-05，最小入口）：

- `ql-analysis::analyze_package` 已落地
- package-aware `ql check <package-dir>` 现已通过该入口分析当前包源码，并加载 `[references].packages` 指向的 dependency `.qi`
- 当前 dependency `.qi` 已推进到 syntax-aware section parse：每个 interface module section 会进入 interface-mode AST
- `ql-analysis::analyze_package` 现已把 dependency `.qi` 的公开符号索引进 package-level truth surface：当前覆盖 top-level `fn` / `const` / `static` / `struct` / `enum` / `trait` / `type`，以及 public trait / `impl` / `extend` methods
- cross-file hover / definition / completion / references 仍未接上这些 indexed dependency symbols

交付目标：

- same-file query 保持兼容
- 对依赖 public symbol 的 identity 稳定
- query truth source 仍只有一套

### Task D：LSP cross-file V1

在 `.qi` 和 package graph 稳定后，只开放最小可交付的跨文件编辑器能力。

交付目标：

- import 后的 cross-file hover / definition / completion / references
- 仍不做 cross-file rename
- `DocumentStore` 旁新增 workspace/package 级缓存，但 bridge 继续只做协议映射

## 明确不做

本阶段入口设计里，下面这些都不应抢跑：

- 直接做 cross-file rename
- 为 LSP 单独做一套 dependency symbol index
- 把 `.qi` 设计成 Rust 风格内部元数据格式
- 借 `.qi` 一次性打开完整 module graph / package registry / publish
- 为了“编辑器体验完整”而提前补大而全的 code actions

## 下一步建议

按当前顺序，最适合的下一个实现切片是：

1. 固定 `qlang.toml` 的 workspace / project reference 最小合同
2. `ql-analysis` 扩成可消费依赖 `.qi` 的 package loader
3. 再进入 `ql-lsp` 的 cross-file 消费路径

这条顺序能保证 Qlang 做的是“语言与工具链一体化”，而不是先拼一个看起来能跳转的 LSP，再回头重做编译接口层。
