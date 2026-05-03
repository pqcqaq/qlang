# 功能清单

这页是长期方向清单，不代表当前已经实现。当前能力只看 [当前支持基线](/roadmap/current-supported-surface)。

## P0：当前必须做稳

- 编译链路：lexer、parser、HIR、resolve、typeck、MIR、LLVM backend。
- CLI：`ql check/fmt/build/run/test/project/ffi`。
- 项目系统：manifest、workspace、本地依赖、lock、`.qi` interface。
- stdlib：`std.core`、`std.option`、`std.result`、`std.array`、`std.test`。
- generics：真实 direct-call specialization、数组长度泛型、stdlib 下游 smoke。
- LSP：diagnostics、hover、definition、references、completion、semantic tokens、formatting、code actions、document symbols、workspace symbols、基础 hierarchy。
- 质量：parser/codegen/diagnostics/LSP/stdlib/project 回归和真实项目 smoke。

## P1：可用性扩面

- 更完整 dependency-aware backend。
- 更完整 workspace diagnostics、references、rename、code actions。
- JSON 输出、CI、VSIX/release 分发。
- 文档生成、benchmark、profile 和多平台验证。
- C/Rust interop 的受控扩面。

## P2：后置能力

- registry、publish、version solving。
- 完整 trait solver、effect system、完整 async/runtime。
- 宏、反射、复杂元编程。
- 直接全量 C++ 互操作。
- 非 LLVM 后端。

## 维护规则

- 这个文件只列方向，不写实现细节。
- 已实现能力必须同步到 [当前支持基线](/roadmap/current-supported-surface)。
- 新能力进入计划前，先明确测试入口和真实项目验证方式。
