# Phase 4 可执行产物设计

## 为什么这一刀不能直接“补个 exe”

P4.2 已经有了：

- `ql build`
- 文本 LLVM IR 输出
- `--emit obj`
- clang-style toolchain discovery

但如果现在直接在 driver 里拼一条 `clang input.ll -o app`，会留下两个后续很难处理的问题：

1. 当前用户态 `main` 是 Qlang 语义入口，不等价于宿主 ABI 入口
2. 一旦 link 失败，现有错误模型只能保留 `.codegen.ll`，无法把下一阶段的中间产物一起暴露出来

所以 P4.3 的核心不是“新增一个 emit 枚举值”，而是先把“语言入口”和“宿主入口”分开，再把 object 与 executable 产物放进同一条稳定管线。

## 候选方案

### 方案 A：直接用 clang 从 `.ll` 一步出可执行文件

优点：

- 实现最短
- 代码改动面小

缺点：

- 入口 ABI 问题被掩盖，而不是被解决
- 无法稳定保留 `.obj` 级中间产物
- 后续 runtime startup、额外对象文件、FFI 桥接都会逼迫 driver 返工

### 方案 B：继续维持当前 IR 形态，只在 driver 侧临时拼装入口

优点：

- `ql-codegen-llvm` 改动较少

缺点：

- driver 会开始理解函数语义和入口返回规则
- 语言语义边界被重新污染到 orchestration 层

### 方案 C：在 codegen 层显式区分“Qlang entry”与“host entry”，driver 统一走两阶段产物

优点：

- `ql-codegen-llvm` 继续负责 ABI 相关 IR 形态
- `ql-driver` 只负责 artifact pipeline
- object / exe 可以共享同一条中间产物路径
- 未来补 runtime object、link args、cross target 时不用改动高层边界

缺点：

- 会让当前 `.ll` 输出从“直接把用户 main 叫 main”变为“内部 entry + host wrapper”

推荐采用方案 C。

## 设计结论

### 1. 入口分层

`ql-codegen-llvm` 不再把用户写的 `main` 直接映射成 LLVM 符号 `@main`。

改为：

- 用户态入口 lower 成内部符号，例如 `@ql_1_main`
- 额外生成一个宿主 ABI wrapper：`@main`

当前 wrapper 规则：

- 若 Qlang `main` 返回 `Void`：
  - 调用内部入口
  - 返回宿主 `i32 0`
- 若 Qlang `main` 返回 `Int`：
  - 调用内部入口
  - 将当前 `i64` 结果截断为宿主 `i32` 退出码

这样 `.ll`、`.obj`、`.exe` 都共享同一份“可链接入口”语义，而不是每个产物各自长一套特例。

### 2. 产物流水线

P4.3 统一采用：

```text
source
  -> ql-analysis
  -> ql-codegen-llvm
  -> textual LLVM IR
  -> clang -c -x ir
  -> intermediate object
  -> clang <object> -o <exe>
  -> executable
```

也就是说：

- `--emit llvm-ir`：直接落 `.ll`
- `--emit obj`：`.ll -> obj`
- `--emit exe`：`.ll -> obj -> exe`

当前仍然只用 clang-style driver，不急着在这一刀引入独立 linker discovery。等 runtime / FFI / 平台分支更明确后，再把 link.exe、lld-link、cc、ld64 的差异抽出来。

### 3. 错误与调试模型

`BuildError::Toolchain` 不再只保存一个 `intermediate_ir`，而是保存 `preserved_artifacts`：

- compile fail：保留 `.codegen.ll`
- link fail：保留 `.codegen.ll` + `.codegen.obj/.o`

CLI 统一打印这些保留产物路径。

这样做的收益是：

- 失败时用户能直接拿到真正参与下一阶段的工件
- 后续如果再加入 runtime object、startup stub、import library，也不需要重写错误模型

### 4. 当前刻意不做的事情

P4.3 仍然不做：

- 独立 linker family discovery
- staticlib / dylib
- startup runtime object
- `extern "c"` 完整 ABI
- struct / tuple / closure lowering
- 多对象增量构建

这一刀的目标只是把“从单文件入口到可执行产物”的骨架固定住。

## 对后续阶段的影响

P4.3 完成后，下一步可以沿着下面的方向继续扩：

- P4.4：runtime startup / extern ABI / extern symbol linking
- P4.5：更完整的 target / linker 配置面
- P5：C FFI 和标准库地基

关键点在于，后续每一刀都只是在既有 artifact pipeline 上追加能力，而不是回头重写 `ql-driver` 和入口 lowering。这样才能保证 Phase 4 真正成为后续原生产物路线的稳定地基。
