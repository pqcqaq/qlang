# Phase 5 `extern "c"` Definition Export Foundation

## 背景

P4 已经把 `extern "c"` declaration direct-call 打通到了真实后端，但这还只解决了“Qlang 调 C”。

Phase 5 的第一刀更应该先解决另一半：

- Qlang 能否生成一个带稳定 C 符号的本地函数
- 这个函数能否沿着现有 `ql build --emit staticlib` 路径进入真实产物

如果继续把这层留空，后面的 `tests/ffi`、头文件生成、Rust/C shim 互操作都会被迫绕过真实编译管线。

## 目标

- 支持顶层 `extern "c"` 函数定义保留 body
- 支持它们经过 HIR / resolve / typeck / MIR / codegen / driver 的真实流水线
- 让 LLVM IR 使用稳定 C 符号名，而不是内部 mangled name

## 明确不做

- 不做 header generation
- 不做 ABI 布局检查
- 不做 dynamic library
- 不做 per-symbol visibility/linkage policy
- 不做完整 `tests/ffi` C 侧集成 harness
- 不把 program-mode `main` 改造成 exported C ABI entrypoint

## 设计决策

### 1. 语法层

顶层 `extern "abi" fn ...` 过去只允许 declaration。

这一刀改为：

- extern block 内的函数仍然只允许 declaration
- 顶层 `extern "abi" fn ...` 改为允许 optional body

这样不会破坏 extern block 的“只承载导入声明”边界，但能给顶层 exported definition 留出语法空间。

### 2. 语义层

带 body 的 `extern "c"` 顶层函数继续复用普通函数路径：

- HIR 仍然是 `ItemKind::Function`
- resolve / typeck / MIR 不需要引入新的特殊 item kind

也就是说，这刀的核心不是新增一套“FFI function pipeline”，而是允许 ABI 信息穿过现有函数 pipeline。

### 3. 后端层

当前后端规则改为：

- declaration + `extern "c"` -> `declare @symbol`
- definition + `extern "c"` -> `define @symbol`
- definition + non-`c` ABI -> 结构化 unsupported diagnostic

顶层 `extern "c"` 定义使用稳定 C 符号名：

- `extern "c" pub fn q_add(...) { ... }` -> `define ... @q_add(...)`

默认 Qlang 函数仍保留内部 mangled name。

### 4. 入口边界

program-mode 用户入口 `main` 仍然要求默认 Qlang ABI。

原因很简单：

- 当前 native build pipeline 还会生成宿主 `@main` wrapper
- 如果用户态 `main` 同时要求 `extern "c"`，符号层会和宿主入口冲突

所以现阶段的规则是：

- exported C ABI entrypoint 用独立 helper
- 默认 `main` 继续作为 Qlang 用户入口

## 测试面

这一刀至少要锁住四层回归：

1. parser：顶层 `extern "c"` definition 可解析
2. formatter：该语法可稳定 round-trip
3. codegen：显式 `extern "c"` definition 使用稳定符号名
4. driver / CLI：`staticlib` 与黑盒 LLVM IR snapshot 均覆盖这条路径

## 后续顺序

这刀完成后，P5 更合理的后续顺序是：

1. 建立 `tests/ffi` 的真实 C 侧集成 harness
2. 增加头文件生成和最小 `ql ffi` 输出面
3. 补 ABI / layout diagnostics
4. 再讨论 dynamic library 与更完整 runtime glue
