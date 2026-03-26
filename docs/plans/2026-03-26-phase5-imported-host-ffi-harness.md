# Phase 5 Imported Host FFI Harness

## 目标

把已经存在的 `extern "c"` 导入能力，从“能 parse / typecheck / lower / 生成 header”推进到“真实宿主 C 实现可被 Qlang 静态库调用”。

这一步的重点不是新增语法，而是把现有 imported ABI 路径放进端到端混编回归，避免后续继续只靠 IR snapshot 推断它是对的。

## 为什么先做这一步

P5 目前已经有：

- exported `extern "c"` function definition
- imported `extern "c"` declaration / extern block member
- `ql ffi header` 的 exports/imports/both surface
- build-side header sidecar

但真实 C 宿主回归还主要证明了“宿主调用 Qlang 导出符号”。这不等于“Qlang 也真的能反过来调用宿主提供的 C ABI”。

如果这里继续空着，后面一旦改 extern lowering、header surface 或 build orchestration，就容易在 imported path 上悄悄回归。

## 测试设计

继续沿用 `crates/ql-cli/tests/ffi.rs` 的真实工具链 harness，而不是新开一套平行测试系统。

新增两类静态库夹具：

- `extern "c" { ... }` 导入
- 顶层 `extern "c" fn ...` 导入

两者都由 Qlang 导出一个真实 C ABI 函数，再在函数体里调用宿主提供的 imported C symbol。

宿主侧 C harness：

- 通过生成的 combined header 拿到 imported/exported 两端声明
- 自己定义 imported host symbol
- 链接 Qlang `staticlib`
- 调用 Qlang 导出函数，最终验证 imported host symbol 确实被执行

## Fixture 元数据

为了不把 header 选择规则硬编码在测试源码里，`tests/ffi/pass/` 夹具现在支持可选元数据文件：

```text
<name>.header-surface
```

内容为：

- `exports`
- `imports`
- `both`

默认值仍然是 `exports`。

这样导出型夹具不用额外配置，而 imported-host 夹具可以显式声明 `both`，让宿主一次性 include 同一份 header 并同时看到 imported/exported ABI。

## 回归覆盖

这一步补齐的不是单个测试，而是三层一致性：

- `crates/ql-cli/tests/ffi.rs`
  - 真实 imported-host staticlib 混编
  - extern block 与 top-level extern 两种导入语法
- `crates/ql-cli/tests/codegen.rs`
  - build-side `both` header 黑盒快照
- `tests/ffi/pass/`
  - self-describing fixture surface metadata

## 当前不做

这一步仍然故意不扩张到：

- shared-library imported host harness
- platform-specific unresolved-symbol policy
- runtime loader glue
- bridge code generation

原因很直接：`staticlib` 的 imported-host linking 语义最稳定，最适合作为这一刀的闭环基础。

## 验证

本切片至少应通过：

```bash
cargo test -p ql-cli --test ffi
cargo test -p ql-cli --test codegen
```
