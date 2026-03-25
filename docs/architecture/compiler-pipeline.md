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
  -> name resolution
  -> HIR
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

这也是目录结构设计要前置考虑的原因。
