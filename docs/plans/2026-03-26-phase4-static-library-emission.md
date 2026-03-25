# Phase 4 静态库产物设计

## 背景

P4 现在已经具备：

- 文本 LLVM IR 输出
- `--emit obj`
- `--emit exe`
- clang-style compiler discovery
- host `main` wrapper

但这条链路仍然默认“这是一个程序”，因为当前后端入口模型仍然以顶层 `main` 为中心。这会直接卡住 P4 路线图里的另一半交付物：静态库。

如果现在为了补 `staticlib` 只是临时放宽检查，允许“没有 `main` 也往下走”，后面会留下两个长期问题：

1. codegen 入口语义混乱，程序模式和库模式共用一套隐式规则
2. toolchain 层只有 compiler，没有 archive 工具边界，后续 `lib.exe` / `llvm-ar` / `ar` 很难继续扩

所以这一刀不是简单加一个 emit 值，而是把 “program vs library” 与 “compiler vs archiver” 两条边界补完整。

## 候选方案

### 方案 A：维持当前 codegen 入口模型，只在 driver 放宽 `main` 检查

优点：

- 改动最少

缺点：

- driver 会开始理解语义入口，而不是只做构建编排
- `ql-codegen-llvm` 仍然默认所有产物都是程序
- 后续 runtime / FFI / package build 时还得返工

### 方案 B：保留当前 program mode，并新增明确的 library mode

优点：

- 入口语义清晰
- `staticlib` 可以没有 `main`
- LLVM IR backend 对“是否生成 host wrapper”有明确责任边界

缺点：

- 需要调整 `emit_module` 的调用面和测试

推荐采用方案 B。

## 设计结论

### 1. codegen mode

`ql-codegen-llvm` 新增两种模式：

- `Program`
  - 需要顶层 `main`
  - 用户态 `main` lower 成内部符号
  - 生成宿主 ABI `@main` wrapper
- `Library`
  - 不要求存在 `main`
  - 不生成宿主 `@main`
  - 导出所有当前可 lower 的顶层 free function

这样 `main` 规则只属于程序模式，不会污染静态库构建。

### 2. artifact 映射

当前映射定义为：

- `llvm-ir` -> `Program`
- `obj` -> `Program`
- `exe` -> `Program`
- `staticlib` -> `Library`

后续如果需要再扩：

- `llvm-ir --crate-type staticlib`
- `obj --crate-type lib`

可以在这个模型上继续长，而不是回头拆接口。

### 3. toolchain split

`ql-driver/toolchain` 从只有 clang，扩成：

- compiler：clang-style invocation
- archiver：`llvm-ar` / `ar` / `llvm-lib` / `lib.exe`

当前范围只做静态库打包，不急着引入更复杂的 linker family/driver family 矩阵。

### 4. staticlib pipeline

```text
source
  -> ql-analysis
  -> ql-codegen-llvm (Library mode)
  -> textual LLVM IR
  -> clang -c -x ir
  -> intermediate object
  -> archiver
  -> static library
```

默认输出名：

- Windows: `<stem>.lib`
- Unix: `lib<stem>.a`

### 5. 错误模型

静态库构建失败时：

- compile fail：保留 `.codegen.ll`
- archive fail：保留 `.codegen.ll` + `.codegen.obj/.o`

仍复用统一的 `preserved_artifacts`，避免再发明一套独立错误结构。

## 测试要求

这一刀必须新增：

- `ql-codegen-llvm`：library mode regression test
- `ql-driver`：`staticlib` 成功 / 归档失败测试
- `ql-cli`：`--emit staticlib` 路径测试
- CLI smoke：在 mock clang + mock archiver 下真实跑通

## 当前刻意不做

- 多文件 archive
- import library
- dynamic library
- package/workspace 级 build graph
- runtime object 自动合并

本切片目标是把 P4 的原生产物边界补齐到“程序产物 + 静态库产物”都能在现有单文件后端上稳定表达。
