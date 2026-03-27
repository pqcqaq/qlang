# Phase 4 对象文件产物设计

## 为什么现在做这一刀

P4.1 已经把 `ql build`、`ql-driver` 和 `ql-codegen-llvm` 的后端边界固定下来，但当前产物仍然只有 `.ll`。如果下一步直接把“链接器、可执行文件、runtime glue”一起堆进去，`ql-driver` 会很快重新变成一层难以维护的大泥球。

因此 P4.2 只解决一个问题：

- 在保持现有 LLVM IR foundation 不变的前提下，补上 object-file emission

## 设计目标

- `ql-driver` 继续负责构建编排，不把外部进程调用塞进 CLI
- object emission 复用 `.ll -> obj` 的管线，而不是重写 codegen
- 外部工具链调用必须可测试，不能强依赖当前开发机真的装了 clang
- 为后续 `--emit exe`、linker discovery、cross target 预留稳定抽象

## 分层

```text
ql-cli
  -> ql-driver/build
       -> ql-analysis
       -> ql-codegen-llvm
       -> ql-driver/toolchain
            -> external clang-style compiler
```

约束：

- `ql-codegen-llvm` 仍然只负责文本 LLVM IR
- `.ll -> obj` 由 toolchain 层负责
- `ql-driver` 只知道“需要哪种 artifact”，不知道 clang 命令细节

## 工具链抽象

新增 `ProgramInvocation`：

- `program`
- `args_prefix`

这样测试时可以注入：

- Windows: `powershell.exe -File mock-clang.ps1`
- Unix: `/bin/sh mock-clang.sh`

而正式环境仍然可以走：

- `clang`
- 未来的显式路径覆盖

新增 `ToolchainOptions`：

- 当前先只包含 `clang`
- 后续可扩成 `lld-link`、`link.exe`、`ar`、`llvm-lib`

新增 `ToolchainError`：

- `NotFound`
- `InvocationFailed`

这类失败不是源码 span 问题，所以不强行塞进 `Diagnostic`，而是走 build/toolchain error。

## object emission 流程

1. `ql-driver` 先照旧生成 LLVM IR 文本
2. 若 `emit == llvm-ir`，直接落盘
3. 若 `emit == obj`：
   - 先把 IR 写到中间 `.codegen.ll`
   - 发现 toolchain
   - 调用 clang 风格命令：`clang -c -x ir input.ll -o output.obj`
   - 成功后删除中间 `.codegen.ll`
   - 失败时保留中间文件，便于调试

## 当前范围

本切片只做：

- `--emit obj`
- object 默认扩展名
- toolchain discovery
- toolchain mock tests

本切片仍然不做：

- `--emit exe`
- linker driver
- import library / static library
- target triple 配置面
- runtime startup object

## 测试策略

- `ql-driver` 测试 object emission 成功路径
- `ql-driver` 测试 toolchain failure 路径
- `ql-cli` 测试 `BuildOptions` 走 object 路径
- 手工 smoke test 用 mock toolchain 驱动真实 `ql build --emit obj`

这样做可以让 P4 在没有系统 clang 的开发环境里继续推进，同时不牺牲后续真正接 clang/lld 的工程可维护性。
