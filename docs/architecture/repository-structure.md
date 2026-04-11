# 仓库目录结构

## 设计目标

仓库目录按四类职责组织：

1. 语言规范和设计演进
2. 编译器与工具链实现
3. 标准库与运行时实现
4. 示例、测试、基准、文档与生态模板

实现分层见 [实现算法与分层边界](/architecture/implementation-algorithms)。

## 当前顶层分层

这页只描述当前仓库里真实存在、且已经承担职责的目录；不会再把未来可能拆出的顶级目录写成现状。

```text
.
├─ docs/                      # VitePress 文档站，承载设计、架构、路线图与教程
├─ crates/                    # Rust workspace：编译器、project/workspace、runtime 与工具层源码
├─ examples/                  # 已提交的 C / Rust 宿主互操作示例
├─ tests/                     # 跨 crate 黑盒 / 集成 / FFI 测试
├─ fixtures/                  # parser / codegen / backend 夹具
├─ ramdon_tests/              # 已提交的 executable smoke 语料
├─ Cargo.toml                 # Rust workspace manifest
└─ README.md                  # 仓库入口说明
```

## 当前已落地结构（2026-04-06）

截至 2026-04-06，仓库主干已覆盖 P1-P6，并继续推进 Phase 7 async/runtime/staticlib/Rust interop 与 Phase 8 `.qi`/dependency/cross-file tooling。当前主要目录如下：

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
│  ├─ ql-project              # package/workspace manifest graph、默认 `.qi` 路径/状态与 interface artifact 工具层
│  ├─ ql-driver               # Phase 4 build orchestration 边界
│  ├─ ql-codegen-llvm         # Phase 4 文本 LLVM IR 后端地基
│  ├─ ql-lsp                  # qlsp：same-file query + dependency-backed cross-file hover/definition/references/completion + `workspace/symbol`
│  └─ ql-cli                  # `ql check` / `ql build` / `ql project` / `ql ffi` / `ql fmt` / `ql mir` / `ql ownership` / `ql runtime`
├─ examples/
│  ├─ ffi-c                   # 真实 C host + combined header 静态链接 Qlang `staticlib`
│  ├─ ffi-c-dylib             # 真实 C host + runtime loader 加载 Qlang `dylib`
│  └─ ffi-rust                # Cargo host + build.rs 静态链接 Qlang `staticlib`
├─ ramdon_tests/
│  ├─ executable_examples     # 已提交的 sync executable smoke 样例
│  └─ async_program_surface_examples # 已提交的 async executable smoke 样例
├─ tests/
│  ├─ ui                      # CLI 黑盒 diagnostics 快照
│  ├─ codegen                 # build / codegen / artifact 黑盒快照
│  └─ ffi                     # 真实 C / Rust 宿主互操作夹具
└─ fixtures/
   ├─ parser                  # parser / formatter 回归输入
   └─ codegen                 # backend / artifact / async staticlib 夹具
```

根测试目录职责：

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

## 分层说明

### `docs/`

- 当前仓库还没有独立顶级 `spec/` 目录。
- 语言规范、愿景、架构和路线图目前都统一沉淀在 `docs/` 内。
- 如果后续需要把规范文本做成更正式、可版本化的独立工件，再拆出 `spec/` 会更符合现状。

### `rfcs/`

- 当前仓库还没有独立顶级 `rfcs/` 目录。
- 重大设计变化目前主要沉淀在 `docs/plans/`、合并设计稿与路线图文档里。
- 如果后续进入更正式的提案/审议流程，再单独落地 `rfcs/` 会更合适。

### `crates/`

当前采用多 crate 结构，便于依赖分层、测试隔离，以及 LSP/formatter/project tooling 复用中间层。

### `tests/` 与 `fixtures/`

测试断言与测试输入分开维护；仓库根目录执行 `ql check .` 时，会跳过 `fixtures/`、构建输出目录和临时测试目录。

## 后续可能新增的顶级目录

如果项目后续进入更重的规范、RFC、基准或模板阶段，可以再单独落地这些目录：

- `spec/`
- `rfcs/`
- `benchmarks/`
- `packages/`
- `stdlib/`

这些目录尚未落地，不应在其他文档中写成当前根目录事实。

当前状态补充：

- `examples/ffi-c` 已经落地为真实可运行示例，直接展示 stable C ABI + combined header + `staticlib` 的宿主调用路径
- `examples/ffi-c-dylib` 已经落地为真实可运行示例，直接展示 runtime-loaded shared library 的宿主调用路径
- `examples/ffi-rust` 已经落地为真实可运行示例，而不再只是预留目录
- 这三份示例都故意保持在稳定 C ABI 范围内：`ffi-c` / `ffi-rust` 锁定 `staticlib` 工作流，`ffi-c-dylib` 锁定 runtime-loaded shared-library 工作流，避免过早承诺 Rust-specific wrapper、import-library policy 或更复杂 runtime 语义
