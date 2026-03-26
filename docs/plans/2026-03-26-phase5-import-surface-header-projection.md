# P5 Import Surface Header Projection

## 目标

在已有 exported C header 生成路径上，补齐 imported `extern "c"` surface 的自动头文件投影，并允许一次性生成合并后的 import/export surface。

这一步的核心价值不是“多一个 CLI 选项”，而是把 Qlang 的 FFI header 从“只能描述我导出了什么”推进到“也能描述我依赖宿主提供什么”，从而让静态库、共享库和宿主侧 stub 的协作方式更稳定。

## CLI 设计

统一入口保持不变，只扩展 surface 选择：

```text
ql ffi header <file> [--surface exports|imports|both] [-o <output>]
```

默认行为保持兼容：

- `exports` 仍是默认值
- 不指定 `-o` 时：
  - `exports` -> `target/ql/ffi/<stem>.h`
  - `imports` -> `target/ql/ffi/<stem>.imports.h`
  - `both` -> `target/ql/ffi/<stem>.ffi.h`

这个命名规则的目的很直接：

- 保持现有 exported-surface 工作流不被破坏
- 让 import/both 产物语义一眼可见
- 允许同一个模块的三种头文件在同一构建目录里并存

## Surface 语义

### `exports`

投影对象：

- public 顶层 `extern "c"` 函数定义
- 必须存在 body

这条路径表达的是“Qlang 模块向宿主导出什么 ABI”。

### `imports`

投影对象：

- 顶层 `extern "c"` 函数声明
- `extern "c"` block 中的函数声明

这条路径表达的是“Qlang 模块要求宿主提供什么 ABI”。

这里刻意不要求 `pub`，因为 import surface 代表的是链接契约，不是对外公开 API。

### `both`

投影对象：

- 同时包含 `exports` 与 `imports`
- 按源码顺序稳定输出 declaration

这条路径主要服务于：

- 宿主一次性查看完整 FFI 面
- 生成集成用 umbrella header
- 调试 shared-library / static-library 的双向 ABI 契约

## 驱动层设计

`crates/ql-driver/src/ffi.rs` 这次新增了几个关键抽象：

- `CHeaderSurface`
- `CHeaderOptions.surface`
- `default_c_header_output_path_for_surface`
- 统一的 import/export 收集器

设计原则：

- 不在 CLI 层重新扫描语法
- 继续复用 `ql-analysis` 的 parse / HIR / resolve / typeck 结果
- 不把 extern block 特判逻辑散落到多个地方

实际策略是：

1. 顶层 `ItemKind::Function` 根据 `body`、`visibility` 和 `surface` 分类为 export/import/skip
2. `ItemKind::ExternBlock` 只在 `surface` 包含 imports 时展开
3. 统一复用同一套类型投影和 unsupported diagnostics 逻辑

## Include Guard 设计

一个容易被忽略但必须修掉的问题是 include guard。

如果 guard 仍然只根据输入 `.ql` 文件 stem 生成，那么：

- `foo.h`
- `foo.imports.h`
- `foo.ffi.h`

会得到同一个 guard，最终导致多个 surface 无法一起 include。

因此这次改为：

- guard 按最终输出头文件名生成

例如：

- `extern_c_surface.imports.h` -> `QLANG_EXTERN_C_SURFACE_IMPORTS_H`
- `extern_c_surface.ffi.h` -> `QLANG_EXTERN_C_SURFACE_FFI_H`

这样 export/import/both 头文件可以稳定共存。

## 测试回归

这次补齐了两层测试：

- `crates/ql-driver/src/ffi.rs`
  - 默认输出命名
  - import surface 生成
  - combined surface 生成
  - exported/imported 计数
- `crates/ql-cli/tests/ffi_header.rs`
  - export snapshot
  - import snapshot
  - both snapshot
  - invalid `--surface` CLI 诊断

新增夹具：

- `tests/ffi/header/extern_c_surface.ql`
- `tests/codegen/pass/extern_c_surface.imports.h`
- `tests/codegen/pass/extern_c_surface.ffi.h`

## 当前仍不做

这一步仍然故意不扩张到：

- struct / tuple / enum / callable ABI 头文件投影
- layout 验证与 richer ABI diagnostics
- bridge code generation
- exported symbol 的 linkage/visibility policy

原因很简单：

- 这些问题会把工作直接拖进布局、平台 ABI 和 runtime glue
- 当前更需要先把 `ql-driver` 的 FFI 头文件边界做稳

## 验证

本切片至少应通过：

```bash
cargo fmt
cargo test -p ql-driver ffi
cargo test -p ql-cli --test ffi_header
```
