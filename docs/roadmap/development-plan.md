# 开发计划

关联文档：

- [当前支持基线](/roadmap/current-supported-surface)
- [阶段总览](/roadmap/phase-progress)
- [工具链设计](/architecture/toolchain)
- [`/plans/`](/plans/)

这页只保留当前判断、优先级和 checkpoint，不再记录长流水账。

## 当前判断

- Phase 1 到 Phase 6 的编译器地基已经足够稳定，当前主矛盾不是前端骨架缺失。
- 当前最大缺口是“真实项目闭环”还不完整：manifest 太薄、依赖图太窄、project-aware build 还不够宽、LSP 也还没在真实 workspace 中完全做实。
- 因此接下来的主线继续优先工程能力，而不是继续扩语言表面。

## 优先级

| 优先级 | 主题 | 当前目标 |
| --- | --- | --- |
| P0 | 真实项目工作流 | package/workspace 根目录的 `build/run/test/check` 稳定可用 |
| P1 | manifest 与依赖工程化 | 把最小 `qlang.toml` 升级成真实工程 manifest，继续推进 dependency-aware build |
| P2 | 基础 LSP 与可复现工具链 | 做实 workspace symbol / navigation / semantic tokens，并补 lock / JSON / CI / 分发 |
| P3 | 高级 IDE 与语言扩面 | cross-file rename / workspace edits / code actions，以及更宽 async/runtime/语言能力后置 |

## 当前 Checkpoints

### A. 项目工作流闭环

目标：

- 让 package/workspace 根目录能直接稳定执行 `ql build`、`ql run`、`ql test`。
- 让脚手架、README、primer、支持页保持一致。

完成标准：

- 新脚手架能开箱进入 `graph/check/build/run/test`。
- 已创建的 workspace 能继续用 `ql project add` 扩成员，而不是只能一次性 `init`。
- target 发现、profile 规则、默认输出目录和测试入口都有明确文档和回归保护。

### B. manifest 与 dependency-aware build

目标：

- 把当前最小 manifest 升级成真实工程 manifest。
- 让本地依赖不只服务 `.qi` 和排序，也真实参与 build/test/run 图。

完成标准：

- package/workspace 只靠 manifest 就能完成 target 发现和依赖装载。
- dependency-aware build 不再只停留在 direct dependency public `extern "c"` 这一条窄路径。

### C. 基础 LSP 工程化

目标：

- 在真实 workspace 里把 definition / references / workspace symbol / semantic tokens 做到可依赖。
- 继续坚持 analysis / project 单一事实面，不让 LSP 自己发明第二套语义。

完成标准：

- healthy workspace 下基础导航和高亮稳定工作。
- VSCode 文档、支持页和插件 README 与实现边界一致。

说明：

- same-file rename 继续保留。
- cross-file rename / workspace edits 等高级重构要等项目模型更稳之后再做。

### D. 可复现构建与团队协作

目标：

- 补齐 `qlang.lock`、JSON 输出、CI 入口和工具链分发约定。
- 让项目能进入脚本和团队协作，而不只是本地试验。

完成标准：

- workspace 级 `check/build/test` 可稳定进入 CI。
- `ql` / `qlsp` / VSIX 的安装与版本绑定有清晰文档。

## 明确后置

- cross-file rename / workspace edits / code actions
- 更宽的 async/runtime/Rust interop 扩面
- 新语法糖和更远的类型系统能力
- registry / publish workflow

## 交付规则

- 入口文档只写结论和边界，不写长流水账。
- 任何用户可见能力都必须同时具备：实现、回归、文档入口。
- 文档与实现冲突时，先修正文档，不在入口页预告“即将支持”。
