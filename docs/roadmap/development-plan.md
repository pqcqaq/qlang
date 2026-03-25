# 开发计划

## 总体原则

开发顺序必须围绕“尽早形成真实闭环”展开。对 Qlang 来说，闭环不是只有一个 parser 能跑，而是至少满足：

1. 能写一个最小程序
2. 能编译到原生可执行文件
3. 能跑测试
4. 能给出像样错误信息
5. 能在编辑器里工作
6. 能调用一个 C 函数

## Phase 0: 预研冻结

### 目标

- 固化语言定位
- 固化仓库结构
- 固化 P0/P1/P2 边界
- 明确高风险技术假设

### 交付物

- 本文档站
- 初版功能清单
- 初版路线图
- 初版架构图和 RFC 流程草案

### 出口标准

- 设计原则不再大幅摇摆
- 后续能按 RFC 继续演进

## Phase 1: 编译器前端最小闭环

### 目标

- 建立 Rust workspace
- 实现 lexer、parser、AST
- 支持基础语法和模块系统
- 冻结 P0 语法基线，包括类型实例化、值实例化、解构绑定、可调用类型与元组返回
- 跑通 parser 回归测试

### 交付物

- `ql check` 能解析项目
- 语法错误有稳定 span
- `qfmt` 能对最小语法集工作

### 出口标准

- hello world 级程序可被解析和格式化
- parser / formatter 回归测试体系建立

### 当前切片拆分

为了避免 Phase 1 被做成一个“不断往 parser 里堆规则”的大泥球，前端开发要拆成更小的可验证切片：

1. workspace 与基础前端骨架
2. 基础声明、类型与表达式
3. 控制流、模式匹配与 formatter 稳定化
4. 顶层声明补全与错误恢复增强
5. 为 HIR lowering 预留更清晰的前端边界

### 当前状态（2026-03-25）

已完成：

- Rust workspace 与基础 crate 拆分
- `ql-span` / `ql-ast` / `ql-lexer` / `ql-parser` / `ql-fmt` / `ql-cli`
- package / use / const / static / type / opaque type / fn / trait / impl / extend / extern
- struct / data struct / enum
- 泛型参数、`where`、泛型类型应用、可调用类型
- 闭包、结构体字面量、基础运算表达式
- `unsafe fn` 和 `unsafe { ... }`
- `if` / `match` 表达式
- `while` / `loop` / `for` / `for await`
- richer pattern 支持：路径模式、tuple-struct 模式、struct 模式、字面量模式
- parser 从单文件拆分为 `item` / `expr` / `pattern` / `stmt` 模块
- 函数签名模型已统一，可覆盖 free function、trait method、extern function
- parser fixture 与 formatter 稳定性测试覆盖到控制流和 Phase 1 声明切片
- Phase 1 评审缺口已补齐：`*const` 原始指针类型、转义标识符、下划线前缀绑定、`pub extern` block round-trip、非法数字后缀诊断
- 前端基础抽象已加固：AST 节点级 span、控制流头部表达式歧义隔离、单元素 tuple 类型/表达式 round-trip、`ql check` 目录扫描过滤 fixture 与工具输出目录

当前验证集：

- `cargo fmt`
- `cargo test`
- `cargo run -p ql-cli -- check fixtures/parser/pass/basic.ql`
- `cargo run -p ql-cli -- check fixtures/parser/pass/control_flow.ql`
- `cargo run -p ql-cli -- check fixtures/parser/pass/phase1_declarations.ql`
- `cargo run -p ql-cli -- check target/review/if_empty_block.ql`
- `cargo run -p ql-cli -- check target/review/while_empty_block.ql`
- `cargo run -p ql-cli -- check target/review/match_empty_arms.ql`
- `cargo run -p ql-cli -- check target/review/one_tuple_type.ql`
- `cargo run -p ql-cli -- fmt fixtures/parser/pass/phase1_declarations.ql`
- `cargo run -p ql-cli -- fmt target/review/one_tuple_type.ql`
- `npm run build` in `docs/`

Phase 1 已完成。

## Phase 2: 语义分析与类型检查

### 目标

- 引入名称解析和 HIR
- 支持基础类型、`struct`、`enum`、`match`
- 支持 `Result` / `Option`
- 支持一等函数、闭包、元组、多返回值与解构的第一版类型检查
- 完成第一版类型检查

### 交付物

- `ql check` 能完成类型检查
- UI diagnostics 测试框架可用
- LSP 可提供 hover 和 diagnostics

### 出口标准

- 中小示例可获得可信的类型错误提示
- HIR 作为语义单一事实源建立

### 当前状态（2026-03-25）

P2 已完成第一切片，也就是“语义地基”而不是“完整类型系统”：

- 新增 `ql-diagnostics`
- 新增 `ql-hir`
- 新增 `ql-resolve`
- 新增 `ql-typeck`
- `ql check` 已切到 parser -> HIR lowering -> resolve -> semantic checks 的新流水线
- HIR arenas、稳定 ID、local binding lowering 已建立
- lexical scope graph 已建立，覆盖 module / callable / block / closure / match arm / for-loop binding scope
- best-effort 名称解析已落地，当前覆盖 local、param、receiver `self`、generic、import alias、builtin type、struct literal root、pattern path root
- 精确 identifier name span 已打通到 AST / HIR / diagnostics
- struct pattern / struct literal shorthand 会在 HIR lowering 时正规化成真实语义节点
- 第一批语义诊断已经稳定回归：
  - top-level duplicate definition
  - duplicate generic parameter
  - duplicate function parameter
  - duplicate enum variant
  - duplicate method in trait / impl / extend block
  - duplicate pattern binding
  - duplicate struct / struct-pattern / struct-literal field
  - duplicate named call argument
  - positional argument after named arguments
  - invalid use of `self` outside a method receiver scope
- 第一批 first-pass typing 也已落地：
  - return-value type mismatch
  - `if` / `while` / match guard 的 `Bool` 条件检查
  - callable 调用的参数个数与参数类型检查
  - top-level `const` / `static` value 引用会参与后续表达式 typing
  - tuple-based multi-return destructuring 的第一层约束
  - direct closure against expected callable type 的 first-pass checking
  - struct literal 的 unknown-field / missing-field / field-type 检查
  - calling non-callable values
- 当前 unresolved global / unresolved type 仍然刻意不做激进报错，等 import / module / prelude 语义成熟后再收紧
- 默认参数目前仍停留在语言设计文档，尚未进入 AST / HIR / type checking 的已实现范围

当前验证集新增：

- `cargo fmt`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test`
- `cargo run -p ql-cli -- check fixtures/parser/pass/basic.ql`
- `cargo run -p ql-cli -- check fixtures/parser/pass/control_flow.ql`
- `cargo run -p ql-cli -- check fixtures/parser/pass/phase1_declarations.ql`
- 手工负例验证：struct pattern shorthand duplicate binding
- 手工负例验证：duplicate enum variant
- 手工负例验证：duplicate named call argument
- 手工负例验证：duplicate trait / impl / extend method
- `npm run build` in `docs/`

### P2 下一切片

在当前切片之后，下一步不应直接跳到 LLVM 或 borrow checking，而应继续把语义层补完整：

1. 继续补足表达式 typing 覆盖面：成员调用、索引协议、枚举 / 结构体模式约束、更多 binary operator 规则
2. 引入更明确的 `unknown` / deferred-constraint 边界，避免后续 inference 重构 `ql-typeck`
3. 建立 UI diagnostics snapshot harness，把 parser / resolve / typeck 的最终渲染输出一起锁住
4. 为后续 LSP 明确定义类型查询和符号查询接口

## Phase 3: 所有权与内存语义

### 目标

- 建立 MIR
- 实现移动检查和基础借用推断
- 引入 drop 语义和 `defer`
- 明确 `unsafe` 边界

### 交付物

- 值语义和移动规则可工作
- 基础资源释放模型成立
- 所有权相关 diagnostics 可解释

### 出口标准

- 常见资源管理场景可编译并运行
- 无需显式生命周期即可覆盖大多数样例

## Phase 4: LLVM 后端与原生产物

### 目标

- 从 MIR lowering 到 LLVM IR
- 生成对象文件、静态库、可执行文件
- 打通链接驱动

### 交付物

- `ql build`
- debug / release profile
- 基础 codegen golden tests

### 出口标准

- hello world、简单容器、简单匹配逻辑可运行
- Windows 和 Linux 至少打通一种 CI

## Phase 5: C FFI 与运行时地基

### 目标

- 支持 `extern "c"` 导入导出
- 建立 FFI 安全包装模式
- 标准库 `core`、`alloc`、`io` 打底

### 交付物

- 调用 libc 或自定义 C 库示例
- `tests/ffi` 成体系
- `ql ffi` 最小桥接能力

### 出口标准

- Qlang 可以作为真实本地库参与链接
- FFI 边界报错可定位

## Phase 6: LSP、格式化器与开发体验

### 目标

- 基于统一语义数据库实现 `qlsp`
- 补全、跳转、引用、rename、semantic tokens
- `qfmt` 风格冻结

### 交付物

- VS Code 最小扩展
- LSP 场景回归测试
- code action 雏形

### 出口标准

- 用编辑器写 Qlang 不再“只能靠编译试错”

## Phase 7: 并发、异步与 Rust 互操作

### 目标

- 支持 `async fn`、`await`、`spawn`
- 提供 executor 抽象
- 通过 C ABI 打通 Rust 互操作工作流

### 交付物

- async 标准库雏形
- `examples/ffi-rust`
- 并发能力 trait 检查

### 出口标准

- 能写基本网络服务样例
- Rust 混编路径清晰可复现

## Phase 8: 文档、包管理与工作区增强

### 目标

- `ql doc`
- lockfile
- feature flags
- 模板项目生成

### 交付物

- API 文档生成站点
- 多包 workspace 示例
- 项目模板

### 出口标准

- 新用户能从脚手架到文档再到编译运行形成顺畅体验

## Phase 9: 深水区能力

### 目标

- 更完整效果系统
- actor 标准库
- 更深的 C++ 互操作
- 增量编译和性能优化完善

### 交付物

- RFC 驱动的高级特性演进
- 性能基线与回归体系
- 更成熟的生态支撑

### 出口标准

- 项目进入“可持续迭代生态期”，而不是只靠主线作者硬推

## 每阶段都必须交付的横向事项

- 文档更新
- RFC 或 ADR 沉淀
- 回归测试补全
- CI 更新
- 示例代码补全

如果少了这些横向事项，项目会形成“代码前进，工程后退”的假繁荣。
