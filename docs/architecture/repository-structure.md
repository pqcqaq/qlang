# 仓库目录结构

## 设计目标

Qlang 仓库结构必须同时服务于四类工作：

1. 语言规范和设计演进
2. 编译器与工具链实现
3. 标准库与运行时实现
4. 示例、测试、基准、文档与生态模板

如果目录从一开始没有分层清楚，后面一定会出现以下问题：

- 规范和实现互相污染
- LSP、格式化器、编译器共享代码困难
- 测试资源散落，难以回归
- 文档和 RFC 无法沉淀

如果要看这些目录在当前实现里分别承载什么算法与分层责任，继续看：

- [实现算法与分层边界](/architecture/implementation-algorithms)

## 推荐结构

```text
.
├─ docs/                      # VitePress 文档站，面向设计、路线图、指南
├─ spec/                      # 语言规范草案与正式规范
├─ rfcs/                      # 语言与工具链演进提案
├─ crates/                    # Rust workspace：编译器和工具链源码
│  ├─ ql-cli
│  ├─ ql-driver
│  ├─ ql-lexer
│  ├─ ql-parser
│  ├─ ql-ast
│  ├─ ql-hir
│  ├─ ql-resolve
│  ├─ ql-typeck
│  ├─ ql-mir
│  ├─ ql-borrowck
│  ├─ ql-codegen-llvm
│  ├─ ql-diagnostics
│  ├─ ql-incremental
│  ├─ ql-fmt
│  ├─ ql-doc
│  ├─ ql-lsp
│  └─ ql-test-harness
├─ runtime/                   # 运行时支持、启动代码、FFI shim
├─ stdlib/                    # 标准库源码
│  ├─ core
│  ├─ alloc
│  ├─ io
│  ├─ net
│  ├─ async
│  ├─ ffi
│  └─ test
├─ examples/                  # 最小示例与最佳实践
├─ packages/                  # 演示用工作区包、模板项目
├─ tests/                     # 黑盒与金丝雀测试
│  ├─ ui
│  ├─ parser
│  ├─ typecheck
│  ├─ compile-pass
│  ├─ compile-fail
│  ├─ run-pass
│  ├─ ffi
│  ├─ lsp
│  └─ fmt
├─ benchmarks/                # 编译器与运行时基准
├─ fixtures/                  # 测试夹具、头文件、示例输入
├─ scripts/                   # 自动化脚本、发布、生成器
├─ .github/workflows/         # CI/CD
├─ Cargo.toml                 # Rust workspace
└─ qlang.toml                 # Qlang workspace / package manifest
```

## 当前已落地结构（2026-03-28）

当前仓库已经不是早期前端样机，而是覆盖 P1-P6 主干并进入 Phase 7 async/runtime/staticlib/Rust interop 的真实工作区。当前根目录里最关键的已落地部分是：

```text
.
├─ docs/                      # VitePress 文档站
├─ crates/
│  ├─ ql-span                 # span 与基础定位
│  ├─ ql-ast                  # 源码导向 AST
│  ├─ ql-lexer                # 手写 lexer
│  ├─ ql-parser               # 按 item / expr / pattern / stmt 拆分的 parser
│  ├─ ql-diagnostics          # 通用 diagnostics 结构与文本渲染
│  ├─ ql-fmt                  # 基于 AST 的 formatter
│  ├─ ql-analysis             # 统一 parse / HIR / resolve / typeck / query / runtime-requirement 入口
│  ├─ ql-hir                  # AST -> HIR lowering 与稳定 ID arena
│  ├─ ql-mir                  # Phase 3 结构化 MIR 与 cleanup / CFG 基础层
│  ├─ ql-borrowck             # Phase 3 ownership facts 与显式消费诊断
│  ├─ ql-resolve              # 作用域图与保守名称解析
│  ├─ ql-typeck               # Phase 2 初始语义检查
│  ├─ ql-runtime              # Phase 7 最小 runtime / executor 抽象
│  ├─ ql-driver               # Phase 4 build orchestration 边界
│  ├─ ql-codegen-llvm         # Phase 4 文本 LLVM IR 后端地基
│  ├─ ql-lsp                  # qlsp：hover / definition / same-file references / completion / rename / semantic tokens
│  └─ ql-cli                  # `ql check` / `ql build` / `ql ffi` / `ql fmt` / `ql mir` / `ql ownership` / `ql runtime`
├─ examples/
│  ├─ ffi-c                   # 真实 C host + combined header 静态链接 Qlang `staticlib`
│  ├─ ffi-c-dylib             # 真实 C host + runtime loader 加载 Qlang `dylib`
│  └─ ffi-rust                # Cargo host + build.rs 静态链接 Qlang `staticlib`
├─ tests/
│  ├─ ui                      # CLI 黑盒 diagnostics 快照
│  ├─ codegen                 # build / codegen / artifact 黑盒快照
│  └─ ffi                     # 真实 C / Rust 宿主互操作夹具
└─ fixtures/
   ├─ parser                  # parser / formatter 回归输入
   └─ codegen                 # backend / artifact / async staticlib 夹具
```

也就是说，当前目录结构承载的是一个真实的编译器/工具链工作区，而不只是前端实验场：前端、语义、中端、后端、FFI、LSP、runtime、示例和文档站都已经进入同一仓库主干。

当前仓库根测试目录也已经不再只有占位设计：

- `tests/ui/` 已承载 CLI 黑盒 diagnostics 快照
- `tests/codegen/` 已承载 backend / artifact 黑盒快照
- `tests/ffi/` 已承载真实 C 宿主集成夹具，并通过 `ql build --header-output` 生成的头文件驱动 C harness 编译
- `tests/ffi/pass/*.header-surface` 现在还能为单个夹具声明 `exports|imports|both` header surface，避免把 FFI harness 特判散回测试源码

当前测试组织也有一条明确约束：

- parser 夹具回归继续放在 `fixtures/` + `crates/ql-parser/tests/`
- `ql-hir`、`ql-resolve` 与 `ql-typeck` 的细粒度语义回归优先放在各自 crate 的 `tests/` 目录，避免继续把大块测试堆回源码文件尾部
- `ql-mir` 的结构与 cleanup / CFG 回归也应留在 `crates/ql-mir/tests/`，避免把中间表示 snapshot 混回 CLI 黑盒层
- `ql-borrowck` 的 moved-state / consume-event / merge 回归应留在 `crates/ql-borrowck/tests/`，锁定 ownership slice 的边界而不是把语义噪声塞进 CLI 黑盒
- `ql-codegen-llvm` 的 IR 结构与 unsupported diagnostics 优先留在 crate 自己的测试里，避免 P4 一开始就把 backend 回归全部堆进黑盒 CLI 层
- `ql-codegen-llvm` 的 crate-local unit tests 继续按 `crates/ql-codegen-llvm/src/tests/*.rs` 分模块维护；`src/lib.rs` 只保留实现与少量 test-only helper，避免再回到单文件混排上万行实现+回归的状态
- `ql-driver` 的输出路径和产物落盘测试也应留在 crate-local tests，避免把 build orchestration 和参数解析耦死
- `ql-driver` 的 exported C header 投影和类型映射回归也应留在 crate-local tests，避免把 FFI surface 逻辑塞回 CLI 层
- 仓库根 `tests/` 保留给后续 CLI 黑盒、UI diagnostics、codegen、FFI 这类跨 crate 测试

## 为什么这样分

### `docs/` 和 `spec/` 分离

- `docs/` 面向导航、说明、路线图和教程
- `spec/` 面向可审查的语言规范文本

这样能避免“文档写得像规范，规范写得像博客”。

### `rfcs/` 单独存在

RFC 是设计演进机制，不应混在普通文档里。后续每个重大决策，例如错误处理模型、trait object、宏系统、包注册中心，都应走 RFC。

### `crates/` 细分而不是一个大 compiler 目录

这有四个好处：

- 编译依赖清晰
- 测试粒度清晰
- LSP、格式化器、文档工具能重用中间层
- 避免巨大 crate 变成“无法重构的黑箱”

### `tests/` 与 `fixtures/` 分离

测试断言和测试输入需要分层管理。特别是 UI 诊断测试、FFI 测试和 LSP 测试，输入资源会很多，不分开后期会极乱。

当前前端实现还额外验证了一个工程约束：仓库根目录执行 `ql check .` 时，不应把 `fixtures/`、构建输出目录和临时测试目录一起扫进去。否则 fail fixture 和杂项目录会污染真实的项目检查结果。

也就是说：

- `fixtures/` 继续保留为显式回归输入目录
- `ql check fixtures/...` 这种显式调用仍然成立
- 但“扫描整个仓库”的命令路径应优先面向真实源码目录，而不是测试资源目录

## 初期就要预留的目录

即使 P0 不全部实现，也建议尽早预留：

- `rfcs/`
- `tests/ui`
- `tests/ffi`
- `examples/ffi-c`
- `examples/ffi-rust`
- `benchmarks/`

这几类目录直接决定项目后续是否有演进机制、质量机制和互操作验证机制。

当前状态补充：

- `examples/ffi-c` 已经落地为真实可运行示例，直接展示 stable C ABI + combined header + `staticlib` 的宿主调用路径
- `examples/ffi-c-dylib` 已经落地为真实可运行示例，直接展示 runtime-loaded shared library 的宿主调用路径
- `examples/ffi-rust` 已经落地为真实可运行示例，而不再只是预留目录
- 这三份示例都故意保持在稳定 C ABI 范围内：`ffi-c` / `ffi-rust` 锁定 `staticlib` 工作流，`ffi-c-dylib` 锁定 runtime-loaded shared-library 工作流，避免过早承诺 Rust-specific wrapper、import-library policy 或更复杂 runtime 语义
