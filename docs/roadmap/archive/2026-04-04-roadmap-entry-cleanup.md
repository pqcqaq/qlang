# 2026-04-04 路线图入口整理与进度对账

这份归档只记录本轮对账与整理结果，不重复旧版长文。

## 对账基准

- 代码实现：`crates/*`
- executable 真运行矩阵：`crates/ql-cli/tests/executable_examples.rs`
- build/codegen pass 矩阵：`crates/ql-cli/tests/codegen.rs`
- sync 样例：`ramdon_tests/executable_examples/`
- async 样例：`ramdon_tests/async_program_surface_examples/`

## 本轮核对结果

- `crates/ql-cli/tests/executable_examples.rs` 当前注册 `60` 个 sync executable case 与 `222` 个 async executable case。
- `ramdon_tests/executable_examples/` 当前真实有 `60` 个 `.ql` 文件。
- `ramdon_tests/async_program_surface_examples/` 当前真实有 `222` 个 `.ql` 文件。
- async 目录最新编号是 `225`，但文件总数不是 `225`；当前入口文档已统一改成 `222`。

## 本轮确认的最近公开切片

以下 sync fixed-shape `for` family 现在都已经同时进入 executable 样例、driver 定向回归和 CLI `llvm-ir` / `obj` / `exe` pass matrix：

- direct call-root fixed-shape `for`
  - `fixtures/codegen/pass/for_call_root_fixed_shapes.ql`
  - `ramdon_tests/executable_examples/32_sync_for_call_root_fixed_shapes.ql`
- same-file import-alias call-root fixed-shape `for`
  - `fixtures/codegen/pass/import_alias_call_root_fixed_shapes.ql`
  - `ramdon_tests/executable_examples/33_sync_import_alias_call_root_fixed_shapes.ql`
- nested call-root fixed-shape `for`
  - `fixtures/codegen/pass/nested_call_root_fixed_shapes.ql`
  - `ramdon_tests/executable_examples/34_sync_nested_call_root_fixed_shapes.ql`
- same-file import-alias nested call-root fixed-shape `for`
  - `fixtures/codegen/pass/import_alias_nested_call_root_fixed_shapes.ql`
  - `ramdon_tests/executable_examples/35_sync_import_alias_nested_call_root_fixed_shapes.ql`

## 本轮整理动作

- 修正首页错误的 async executable 样例数量：`220 -> 222`
- 更新 [当前支持基线](/roadmap/current-supported-surface)，把 sync fixed-shape `for` 的 root 形态描述补齐到 direct/import-alias call-root、nested/import-alias nested call-root，以及 parenthesized/unparenthesized inline projected root
- 压缩 [P1-P7 阶段总览](/roadmap/phase-progress)，移除不适合继续留在入口页的逐轮流水账
- 收紧 [开发计划](/roadmap/development-plan) 中过长的 P7.4 Task 3 状态段，保留执行方向，细节回收到基线页和归档页

## 使用建议

- 日常开发先看：
  - [当前支持基线](/roadmap/current-supported-surface)
  - [开发计划](/roadmap/development-plan)
  - [P1-P7 阶段总览](/roadmap/phase-progress)
- 需要追溯逐轮细节时，再看：
  - [2026-04-03 当前支持基线详细归档](/roadmap/archive/2026-04-03-current-supported-surface-detailed)
  - [2026-04-03 P1-P7 阶段总览详细归档](/roadmap/archive/2026-04-03-phase-progress-detailed)
