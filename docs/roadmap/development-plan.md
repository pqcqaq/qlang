# 开发计划

> 最后同步：2026-05-17

只记录当前开发顺序和可执行约束。不写日期承诺，不写流水账。

## 当前判断

- Qlang 已有编译器、CLI、项目系统、LSP 和 stdlib 地基。
- 当前瓶颈是生产可用性，不是重做语言骨架。
- stdlib、workspace、LSP、测试和分发必须一起推进。
- 每轮交付一个可验证切片：代码、回归、文档、提交。

## 推进顺序

| 顺序 | 主题 | 目标 |
| --- | --- | --- |
| 1 | 项目可用性 | `ql project/check/build/run/test` 在真实本地 workspace 中稳定工作 |
| 2 | stdlib | 收口 `std.core`、`std.option`、`std.result`、`std.array`、`std.test` 的 public API，并扩大 downstream smoke |
| 3 | generics/backend | 补齐 generic monomorphization 和 dependency-aware backend |
| 4 | LSP/VSCode | 补齐真实 workspace 下的导航、高亮、补全、格式化、code action |
| 5 | 分发准备 | release、VSIX、CI、JSON 输出、安装文档 |
| 6 | 语言扩面 | 更宽 async/runtime、trait/effect、workspace-wide refactor |

## 当前工作项

- 继续收口 `ql-cli` project pipeline，优先扩真实 workspace/package smoke，而不是堆只测内部函数的单元测试。
- `project init --stdlib` starter 已覆盖 package/workspace 的 `check/build/run/test`、`emit-interface`、`emit-interface --check`、package `graph/status --json`、package `dependencies` 文本、JSON 和 `--name` JSON、package `dependents --name --json`、workspace `graph/status --json --package`、workspace `dependencies --name` 文本和 JSON、以及关键 JSON 输出。下一步继续把它作为 downstream 可用性入口维护。
- `ql project emit-interface` 的 standalone package source-path 正向回归已覆盖普通、`--check`、`--changed-only` 和 `--changed-only --check` 组合；后续只在实际回归暴露新的 selector/reporting 缺口时继续补强，确保接口产物入口和 `check/build` 共用的包解析合同一致。
- `ql test` 的 package-under-test/local-generic source override 已覆盖 package path 和直接 project test file；下一步继续收紧 dependency/source override 组合路径，并纳入共享 project pipeline。
- 继续扩大 stdlib package-local smoke、stdlib examples 和 downstream consumers 对 generic public API 的覆盖。
- LSP 下一步重点是把 diagnostics、formatting、folding/selection、signatureHelp/inlayHint、codeLens 等请求收敛到统一 workspace analysis/cache 边界。
- README、roadmap、stdlib、VSCode 文档必须跟实现同步；实现未落地时文档写成未支持。
- JSON 输出要覆盖成功和 preflight/render/selection failure，保证 CLI 能被工具链稳定消费。

## 明确后置

- 自动 prelude
- registry、publish、version solving
- 完整 generic monomorphization
- 完整 workspace-wide rename/refactor/index
- 更宽 async/runtime/Rust interop
- 可变参数语法

## 代码整理规则

- 不新增 `foo3/foo4/foo5` 这类固定 arity API；先补泛型、数组初始化或可变参数等语言能力。
- compiler regression fixture 也遵守同一规则；用 length-generic wrapper 加具体 caller 触发实例化，不用固定 arity 假 API。
- 不在 LSP 里复制语义规则；缺能力先补 `ql-analysis` query。
- 不把测试 fixture 写进生产路径。
- 大文件拆分必须行为不变，并用现有回归证明。
- 文档发现实现缺口时，优先修实现；实现未落地时，文档必须写成未支持。
