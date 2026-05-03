# 开发计划

> 最后同步：2026-05-03

这页只记录当前开发顺序。不写日期承诺，不写流水账。

## 当前判断

- Qlang 已有编译器、CLI、项目系统、LSP 和 stdlib 地基。
- 当前瓶颈是生产可用性，不是重新设计语言骨架。
- stdlib、workspace、LSP、测试和分发必须一起推进。
- 每轮交付一个可验证切片：代码、回归、文档、提交。

## 推进顺序

| 顺序 | 主题 | 目标 |
| --- | --- | --- |
| 1 | 项目可用性 | `ql project/check/build/run/test` 在真实本地 workspace 中稳定工作 |
| 2 | stdlib | `std.core`、`std.option`、`std.result`、`std.array`、`std.test` 形成可消费 API |
| 3 | generics/backend | 减少固定 arity helper，优先修语言和后端能力 |
| 4 | LSP/VSCode | 补齐真实 workspace 下的导航、高亮、补全、格式化、code action |
| 5 | 分发准备 | release、VSIX、CI、JSON 输出、安装文档 |
| 6 | 语言扩面 | 更宽 async/runtime、trait/effect、workspace-wide refactor |

## 当前工作项

- 收紧 `stdlib` generic carrier、length-generic array helpers 和 `std.test` downstream smoke。
- 补 direct local dependency 的 value/type/member 执行路径。
- 扩展 `qlsp` 的 workspace navigation、diagnostics、code action 和 hierarchy 能力。
- 整理大文件和补丁式兼容层，拆分前先锁定回归。
- 保持 README、roadmap、stdlib、VSCode 文档与实现同步。

## 明确后置

- 自动 prelude
- registry、publish、version solving
- 完整 generic monomorphization
- 完整 workspace-wide rename/refactor/index
- 更宽 async/runtime/Rust interop
- 可变参数语法

## 代码整理规则

- 不新增 `foo3/foo4/foo5` 这类固定 arity API，除非它只是兼容层并立即解锁 smoke。
- 不在 LSP 里复制语义规则；缺能力先补 `ql-analysis` query。
- 不把测试 fixture 写进生产路径。
- 大文件拆分必须行为不变，并用现有回归证明。
- 文档发现实现缺口时，优先修实现；实现未落地时，文档必须写成未支持。
