# 当前支持基线

> 最后同步：2026-05-04

这页记录当前可依赖能力。更细节的行为以代码和回归测试为准。

## 真相源

- 实现：`crates/*`、`stdlib/packages/*`
- CLI / project 回归：`crates/ql-cli/tests/*`
- analysis / LSP 回归：`crates/ql-analysis/tests/*`、`crates/ql-lsp/tests/*`
- executable smoke：`ramdon_tests/*`

## 已支持

### 编译器和运行

- `lexer -> parser -> HIR -> resolve -> typeck -> MIR -> LLVM` 主链路可用。
- LLVM 输出覆盖 `llvm-ir`、`asm`、`obj`、`exe`、`dylib`、`staticlib`。
- `if` 表达式要求分支类型合流；语句形式只检查分支内部语义。
- async/runtime 只有最小保守子集。

### CLI 和项目

- `ql check/fmt/mir/ownership/runtime/build/run/test/project/ffi` 已实现。
- `ql project init/add/remove/status/dependencies/dependents/targets/graph/lock/emit-interface` 已可维护本地 workspace。
- `ql build/run/test/check` 支持 project-aware 入口和常用 `--json`、`--list`、`--package`、`--target`。
- 单文件 `ql build/run/test file.ql` 可复用本地 generic free function direct-call specialization。
- `ql project init --stdlib` 能生成依赖 `std.core`、`std.option`、`std.result`、`std.array`、`std.test` 的项目。

### stdlib 和依赖桥接

- 跨包执行仍是保守切片：public const/static、受限 free function、`extern "c"`、部分 type/value bridge、受限 receiver method。
- public/local generic free function 支持 direct-call 多实例 specialization。
- 数组长度泛型参数可在函数体内作为 `Int` 值读取。
- dependency generic specialization 能递归处理同依赖模块内的 generic helper 直调。
- `Option[T]`、`Result[T, E]`、`std.array` length-generic helpers、`std.test` assertions 已有真实 smoke。
- 固定长度数组 helper 和 concrete carrier 只保留兼容层，不再作为主方向扩张。

### LSP 和 VSCode

- same-file：hover、definition、declaration、typeDefinition、references、documentHighlight、completion、semantic tokens、formatting、codeAction、codeLens、callHierarchy、typeHierarchy、rename。
- workspace：`workspace/symbol`、`implementation`、open-doc dependency navigation、保守 workspace rename。
- stdlib 兼容 API 会在补全和依赖 import hover 中提示 deprecated，并排在推荐 generic / length-generic API 后面。
- formatting：document/range/on-type formatting 复用 `ql fmt`。
- VSCode 插件是 thin client，不自带 `qlsp`。

## 未支持

- 完整 dependency-aware backend
- 完整 generic monomorphization、泛型 alias lowering、自动 prelude
- registry、version solving、publish workflow
- release 和 VSCode Marketplace 分发
- 完整 workspace-wide rename/refactor/code actions/references index
- 完整 trait solver、effect system、async/runtime 语言面

## 主要缺口

- `ql-cli` 主链路仍过度集中，`check/build/run/test/project build` 需要抽成共享 project pipeline。
- `ql test` 仍有测试专用 bridge/source override 路径，需要用 parity 回归证明与 `build/run` 依赖语义一致。
- LSP 还不是稳定 workspace service；diagnostics、references、rename、symbols 需要统一 workspace index。
- stdlib 仍保留固定 arity 和 concrete carrier 兼容层，推荐 API 必须继续向 generic/length-generic 收敛。
- `project init --stdlib` 已生成简洁 starter，但模板仍直接绑定当前 stdlib 包和函数名。

## 继续阅读

- [开发计划](/roadmap/development-plan)
- [阶段总览](/roadmap/phase-progress)
- [工具链设计](/architecture/toolchain)
