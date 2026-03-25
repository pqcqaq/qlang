# 编译器流水线

## 实现语言选择

推荐使用 Rust 实现 Qlang 工具链。不是因为“Rust 最潮”，而是因为它在这件事上工程收益最高：

- 内存安全和并发安全更适合长期维护大型编译器
- 生态中已有较成熟的 parsing、diagnostic、arena、interner、LSP、LLVM 绑定方案
- 对构建 CLI、增量缓存和测试工具也足够友好

## 总体流水线

```text
source
  -> lexer
  -> parser
  -> AST
  -> HIR lowering
  -> name resolution
  -> type / trait / effect checking
  -> MIR
  -> ownership / borrow / escape analysis
  -> monomorphization
  -> LLVM IR
  -> object / library / executable
```

## 分层原则

### AST

- 保留接近源码的结构
- 方便格式化器和语法级诊断
- 不承载复杂语义
- `item` / `type` / `stmt` / `pattern` / `expr` 节点都应直接携带 span，避免后续 diagnostics 与 LSP 反向逼迫 AST 重构
- 控制流头部表达式与普通表达式要允许有不同解析约束，例如 `if` / `while` / `for` / `match` 头部不能被结构体字面量歧义污染

当前前端实现已经按职责拆开 parser 入口：

- `item` 负责顶层声明
- `expr` 负责表达式和 postfix 规则
- `pattern` 负责绑定与 `match` 模式
- `stmt` 负责 block 和控制流语句
- 函数签名抽象可同时服务普通函数、trait item 和 extern item

这个边界要继续保持，避免未来把 parser 重新写回一个巨型文件。

### HIR

- 解析名称、作用域和类型引用
- 作为 LSP 语义查询的重要基础
- 对上层工具最友好

截至 2026-03-25，HIR 的第一层地基已经实际落地，而不是停留在文档里：

- 新增 `ql-hir` crate，负责 AST 到 HIR 的 lowering
- HIR 中的 `item` / `type` / `block` / `stmt` / `pattern` / `expr` / `local` 全部进入独立 arena
- 从第一天开始引入稳定 ID，而不是后续再为 LSP 和增量分析返工
- pattern 里的绑定名在 lowering 时就被转成 `LocalId`
- AST 现在额外保留 declaration name、generic param、regular param、pattern field、struct literal field、named call arg、closure param 的精确 name span，避免语义诊断继续依赖粗粒度 fallback
- HIR 会在 lowering 时主动正规化 surface shorthand：`Point { x }` 模式字段会变成真实 binding pattern，`Point { x }` 结构体字面量字段会变成真实 `Name("x")` 表达式
- HIR 仍然保留 span，供 diagnostics、后续 name resolution 和 IDE 查询复用

这一版 HIR 仍然是“语义前置层”，还不是完整名称解析结果。当前刻意没有在这里引入过重的 query system 或类型约束图，避免在 P2 初期把抽象提前做死。

### MIR

- 明确控制流和所有权动作
- 适合 borrow、escape、drop、effect lowering
- 适合后续优化和代码生成准备

这种分层能避免“一套 AST 硬扛一切”导致的后期崩塌。

## 查询系统

编译器、LSP 和文档生成都应该建立在统一查询系统上。推荐引入增量查询架构，核心思想：

- 每个分析结果都是可缓存 query
- 输入变更后只重新计算受影响节点
- 编译器命令行与 LSP 共用数据库

这样做的好处非常直接：

- `ql check` 和 `qlsp` 不会各自维护一套语义逻辑
- IDE 响应速度更可控
- 后续重构和诊断增强成本更低

## 诊断架构

诊断系统应单独成层，而不是散落在各模块里。建议包含：

- span 与文件映射
- 错误码
- 主错误与附加说明
- fix-it 建议
- 机器可读 JSON 输出

当前实现已经完成第一层抽离：

- 新增 `ql-diagnostics` crate，统一承载 `Diagnostic` / `Label` / renderer
- `ql check` 已不再只会打印 parser 错误，而是统一输出 parser 与 semantic diagnostics
- duplicate diagnostics 现在区分 primary / secondary label，header 会稳定锚定在真正的 duplicate 位置，而不是偶然取第一个 label
- 当前 duplicate 语义检查已经覆盖 top-level definition、generic parameter、function parameter、enum variant、trait/impl/extend method、pattern binding、struct field、struct-pattern field、struct-literal field、named call arg
- 现阶段先覆盖文本输出；错误码、fix-it 和 JSON 输出留给后续切片

## LLVM 后端边界

不要让 LLVM 污染整个编译器架构。建议：

- 语义分析和中间表示与 LLVM 解耦
- LLVM 只在 codegen 层出现
- 为未来可能的解释器、WASM 或自定义后端保留空间

## 测试策略

编译器项目不能只靠单元测试。必须同时具备：

- parser fixture / snapshot tests
- typecheck tests
- UI diagnostics tests
- codegen golden tests
- 运行时集成测试
- FFI 集成测试

当前状态：

- parser fixture tests 已稳定
- HIR lowering tests 已拆到 `crates/ql-hir/tests/`，并覆盖 shorthand normalization、closure param span、named call arg span
- semantic duplicate-diagnostics tests 已拆到 `crates/ql-typeck/tests/`，并按 duplicates / rendering 分组
- 精确 name span 与 shorthand lowering 回归已建立
- UI diagnostics snapshot harness 还未开始，这是 P2 后续要补的关键基础设施

这也是目录结构设计要前置考虑的原因。
