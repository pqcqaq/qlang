# 开发计划

> 最后同步：2026-05-06

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

- 抽 `ql-cli` project pipeline，统一 `check/build/run/test/project build` 的 request context、target selection、dependency/interface prep 和 reporting。
- 收紧 `ql test`，继续把测试专用 bridge/source override 抽进共享 project pipeline，并扩大 dependency consumer smoke 覆盖。
- 建立 LSP workspace index，让 diagnostics、references、rename、symbols 和 semantic tokens 走同一份分析缓存。
- 收紧 `stdlib` generic carrier、length-generic array helpers 和 `std.test` downstream smoke；`std.array` 固定 arity wrapper 直接删除，非数组 legacy 只在没有语言级替代时保留。
- 把 `project init --stdlib` 模板迁向 versioned stdlib example，避免 CLI 直接绑定 stdlib 内部 API。
- 保持 README、roadmap、stdlib、VSCode 文档与实现同步。

## 明确后置

- 自动 prelude
- registry、publish、version solving
- 完整 generic monomorphization
- 完整 workspace-wide rename/refactor/index
- 更宽 async/runtime/Rust interop
- 可变参数语法

## 代码整理规则

- 不新增 `foo3/foo4/foo5` 这类固定 arity API；先补泛型、数组初始化或可变参数等语言能力。
- 不在 LSP 里复制语义规则；缺能力先补 `ql-analysis` query。
- 不把测试 fixture 写进生产路径。
- 大文件拆分必须行为不变，并用现有回归证明。
- 文档发现实现缺口时，优先修实现；实现未落地时，文档必须写成未支持。
