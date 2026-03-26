# Phase 5 Build-Sidecar C Header

## 目标

在已有 `ql ffi header` 基础上，把最常见的 library 交付路径收敛成一次命令：

- `ql build` 产出 `dylib` / `staticlib`
- 可选地同时产出对应的 C header
- 不让用户再手动跑第二次 `ql ffi header`

这一步的重点不是“再加几个 CLI flag”，而是把 FFI 交付面真正挂到 `ql-driver` 的 build orchestration 上，形成一个稳定、可测试、后续还能继续扩展的 artifact model。

## CLI 形态

新增 build 侧参数：

```text
ql build <file> --emit dylib|staticlib [--header] [--header-surface exports|imports|both] [--header-output <output>]
```

约束：

- `--header` 生成默认 `exports` surface
- `--header-surface` 与 `--header-output` 会隐式启用 header generation
- 只允许用于 `dylib` / `staticlib`
- `llvm-ir` / `obj` / `exe` 上直接返回 invalid input

默认命名：

- 主 library artifact 仍按现有规则命名
- sidecar header 放在 library artifact 同目录
- 文件名使用源码 stem，而不是 library 文件名
  - `libffi_export.so` -> `ffi_export.h`
  - `math.lib` + `imports` -> `math.imports.h`

## Driver 设计

这一步不引入新的 pipeline crate，也不在 CLI 里手写二次调度，而是直接扩展 `crates/ql-driver/src/build.rs`：

- `BuildOptions` 新增 `c_header: Option<BuildCHeaderOptions>`
- `BuildArtifact` 新增 `c_header: Option<CHeaderArtifact>`
- `build_file` 继续只做一次 `analyze_source`
- build-side header 通过 `emit_c_header_from_analysis` 复用已有 `source + Analysis`

这样做的原因：

- 避免 build 路径重新 parse / resolve / typeck
- header 投影逻辑继续只存在于 `ffi.rs`
- CLI 仍保持薄，只负责参数解析和结果打印

## 错误模型

这里必须比单独的 `ql ffi header` 更严格，因为它属于 build 结果的一部分。

规则：

- 先拒绝非法组合：
  - 非 library emit
  - `--header-output` 与主 artifact 输出路径相同
- library 主产物成功后，再生成 sidecar header
- 如果 sidecar 失败：
  - 删除刚生成的 library artifact
  - 把 header 错误映射回 `BuildError`

这样可以避免对用户暴露“库已经生成，但 header 失败”的半成功状态。

## 测试策略

需要同时覆盖三层：

- `crates/ql-driver/src/build.rs`
  - 默认 sidecar 命名
  - dynamic/static library 成功附带 header
  - 非 library emit 拒绝
  - header 输出路径冲突拒绝
  - sidecar 失败时回收主 artifact
- `crates/ql-cli/tests/codegen.rs`
  - `dylib + --header` 默认 export header 快照
  - `staticlib + --header-surface imports` 默认 import header 快照
  - `exe --header` 失败快照
- `crates/ql-cli/tests/ffi.rs`
  - 真实 C harness 改成直接消费 `ql build --header-output`

## 当前不做

这一步仍然故意不扩张到：

- bridge code generation
- symbol visibility/linkage policy
- richer ABI/layout validation
- language-package 级别的多产物 build graph

原因很直接：当前更需要先把“单文件 library + sidecar header”的抽象边界钉稳。

## 验证

本切片至少应通过：

```bash
cargo fmt
cargo test -p ql-driver --lib
cargo test -p ql-cli --test codegen
cargo test -p ql-cli --test ffi
cargo test -p ql-cli --test ffi_header
```
