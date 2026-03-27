# Phase 5 C FFI 与宿主互操作地基

## 目标

Phase 5 的任务是建立最小但真实可用的 C ABI 互操作闭环，让 Qlang 不只是“能生成库”，而是能被 C 宿主稳定消费，也能稳定声明和调用外部 C ABI。

核心交付：

- extern C export
- extern C import
- `ql ffi header`
- import / export / both surface projection
- build-side sidecar header
- real C-host integration harness

## 合并后的切片结论

### 1. Export surface

这一组切片把顶层 `extern "c"` 函数定义推进为真实导出能力：

- parser / formatter 支持带 body 的顶层 `extern "c"` 定义
- codegen 输出稳定导出符号
- `staticlib` / `dylib` 可承载导出 surface

### 2. Dynamic library and shared-library surface

这一组切片把 shared library 做成真实工程能力：

- `ql build --emit dylib`
- explicit exported C symbol requirements
- Windows `/EXPORT:` handling
- dynamic library black-box regressions

### 3. Header generation

这一组切片把 header 从“补充脚本”收敛成工具链一部分：

- `ql ffi header`
- export header
- import header
- combined header
- 默认输出路径和 surface-specific suffix 已稳定

### 4. Build-side header projection

这一组切片把 header 绑定到 `ql build`：

- `--header`
- `--header-surface`
- `--header-output`
- header 与 library artifact 使用同一份分析快照
- header 失败时回滚刚生成的库产物

### 5. Real host integration

这一组切片把 FFI 能力推进到真正的端到端验证：

- C static-link harness
- C dynamic-load harness
- imported-host callback harness
- Rust static-link harness（通过稳定 C ABI 直接消费 Qlang 导出）
- Rust static-link harness 也已覆盖“宿主提供 callback，Qlang 反向调用宿主符号”的最小导入路径
- per-fixture `.header-surface` metadata

## 当前架构收益

P5 现在已经建立：

- Qlang <-> C 的最小闭环
- header 与 library artifact 的统一 truth surface
- 真实宿主集成测试，而不是只做字符串快照
- Rust 宿主现在也已有最小静态链接闭环，可直接复用现有 C ABI surface
- Rust 宿主现在不仅能调用 Qlang 导出，也能为 Qlang 的 `extern "c"` import 提供最小 callback 实现

## 当前仍刻意保留的边界

- complex aggregate ABI
- C++ direct binding generation
- Rust-specific wrapper generation
- auto safe wrapper generation
- richer ABI diagnostics and layout validation
- 更复杂 runtime / ownership 穿边界语义

## 归档

本阶段原始切片稿已归档到 [`/plans/archive/phase-5`](/plans/archive/index)。
