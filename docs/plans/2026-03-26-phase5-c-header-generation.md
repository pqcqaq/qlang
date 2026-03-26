# Phase 5 Minimal C Header Generation

> 更新（2026-03-26 后续）：这里记录的是 export-surface 头文件生成的首个切片。后续已经补上 import/both surface 投影，见 [P5 Import Surface Header Projection](/plans/2026-03-26-phase5-import-surface-header-projection)。

## 为什么现在做这一刀

P5 的前两步已经把两件关键事情打通了：

- 顶层 `extern "c"` 定义会 lower 成稳定 C 符号
- 真实 C 宿主可以链接 Qlang `staticlib`

但如果停在这里，C 侧调用者仍然需要手写 prototype。这样会留下三个问题：

1. Qlang 导出符号和 C 侧声明之间没有统一来源
2. 测试会绕开真实工具链能力，继续依赖手写胶水
3. 后续 Rust/C shim 互操作会缺少一个可扩展的桥接入口

所以这一刀的目标不是“完整 FFI 绑定生成”，而是先补一个最小、可维护、能被真实消费的 exported API header surface。

## 范围收敛

当前只支持：

- 顶层 `extern "c"` 函数定义
- 必须 `pub`
- 必须有 body
- 参数和返回类型必须能映射到当前已支持的 C 标量/指针类型

这个初始切片当时明确不做：

- struct / tuple / enum / callable / closure ABI
- 布局校验
- bridge code generation
- visibility/linkage 精细控制

这样能保证生成的 header 和当前真实可构建的 `staticlib` surface 对齐，而不会过早承诺还没做完的 ABI 能力。

## 分层设计

### `ql-driver`

新增 `emit_c_header(path, options)`：

- 负责读取源码
- 复用 `ql-analysis`
- 从 HIR + resolution 中筛选 exported C API
- 把支持矩阵收敛成结构化 diagnostics
- 输出确定性的 `.h` 文件

CLI 不直接实现任何类型映射或源码扫描逻辑。

### `ql-cli`

新增：

```text
ql ffi header <file> [-o <output>]
```

当前默认输出路径：

```text
target/ql/ffi/<stem>.h
```

CLI 只负责：

- 参数解析
- driver 调用
- diagnostics / IO error 渲染

## 当前支持的类型映射

| Qlang | C |
| ---- | ---- |
| `Bool` | `bool` |
| `Void` | `void` |
| `Int` / `I64` / `ISize` | `int64_t` |
| `UInt` / `U64` / `USize` | `uint64_t` |
| `I32` | `int32_t` |
| `U32` | `uint32_t` |
| `I16` | `int16_t` |
| `U16` | `uint16_t` |
| `I8` | `int8_t` |
| `U8` | `uint8_t` |
| `F32` | `float` |
| `F64` | `double` |
| `*T` / `*const T` | 对应 `T*` / `const T*`，递归套用已支持 pointee |

当前输出结构固定为：

- include guard
- `#include <stdbool.h>`
- `#include <stdint.h>`
- `extern "C"` wrapper
- 按源码顺序输出 exported function declaration

## 测试面

新增三层回归：

1. `crates/ql-driver/src/ffi.rs`
   - 类型映射
   - 默认输出路径
   - unsupported export diagnostics
   - “至少存在一个 public exported function” 约束
2. `crates/ql-cli/tests/ffi_header.rs`
   - 黑盒 header snapshot
   - failing signature regression
3. `crates/ql-cli/tests/ffi.rs`
   - 真实 C harness 不再手写 prototype
   - 改为先执行 `ql ffi header`
   - 再让 C 宿主通过生成头文件编译并链接 Qlang `staticlib`

这意味着头文件生成已经不只是“能输出一个文件”，而是进入了真实端到端链路。

## 下一步

在这个切片之后，P5 当时更合理的扩展方向是：

1. import surface 的 header projection
2. richer ABI/layout diagnostics
3. Rust/C shim 示例与桥接代码生成
4. visibility/linkage control

但这几步都应该建立在当前 exported header surface 之上继续扩展，而不是重新把逻辑塞回 CLI。
