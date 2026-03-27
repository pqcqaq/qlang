# Phase 4 LLVM 后端与原生产物地基

## 目标

Phase 4 的任务是把 Qlang 从“只有语义层”推进到“能落真实产物”的后端主干，同时把 CLI、driver、backend、toolchain 的边界做稳。

核心交付：

- `ql build`
- textual LLVM IR
- object / executable / static library / dynamic library emission
- toolchain boundary
- codegen regression harness

## 合并后的切片结论

### 1. Backend foundation

第一组切片先冻结了后端分层：

- `ql-driver`
- `ql-codegen-llvm`
- `ql build`
- 默认输出 `target/ql/<profile>/<stem>.ll`

关键设计：

- `ql-analysis` 不负责产物
- `ql-codegen-llvm` 不读文件、不做 CLI
- `ql-driver` 负责 build request -> analysis -> codegen -> artifact
- `ql-cli` 只做用户入口和错误码

### 2. Native artifact pipeline

这组切片把后端从 `.ll` 推到真实产物：

- `--emit obj`
- `--emit exe`
- `--emit staticlib`
- `--emit dylib`
- compiler / archiver / linker failure 时保留中间产物
- library mode / program mode 分离

### 3. Extern C direct-call foundation

这组切片把 extern C 真实接进 codegen：

- extern block member 与 top-level extern declaration 都进入统一 callable identity
- direct extern call lower 成 `declare` + `call`
- program mode 与 library mode 都打通
- extern call 参与 resolve/typeck/MIR/codegen 统一路径

### 4. Codegen harness

这组切片建立了最关键的回归基础：

- black-box codegen snapshots
- pass / fail fixture
- llvm-ir / obj / exe / dylib / staticlib 覆盖
- unsupported backend feature 走结构化 diagnostics
- first-class function value 不再 panic backend
- `match` / `for` 这类仍未进入 P4 支持矩阵的结构化 MIR terminator，现在也有显式的 backend / driver / CLI 失败回归，而不是只靠实现里的 `unsupported` 分支兜底
- `defer` 对应的 cleanup lowering 仍未实现，但 backend 现在会对完全重复的 unsupported diagnostics 做稳定去重；cleanup rejection 也已经补到 backend / driver / CLI 三层回归里

## 当前架构收益

P4 现在已经建立：

- 稳定的 build driver 边界
- 可解释的 toolchain failure model
- 可回归的 artifact pipeline
- 可扩展到 FFI 的后端地基

## 当前仍刻意保留的边界

- richer ABI / runtime glue
- closure / tuple / struct 的更广 lowering
- first-class function value lowering
- 更大规模 toolchain family 组合探测
- 更完整 shared-library surface 与 linkage/visibility 控制

## 归档

本阶段原始切片稿已归档到 [`/plans/archive/phase-4`](/plans/archive/index)。
