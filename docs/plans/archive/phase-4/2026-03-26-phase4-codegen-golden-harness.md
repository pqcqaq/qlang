# Phase 4 Codegen Golden Harness 设计

## 背景

P4 现在已经有了真实的 native artifact pipeline：

- `llvm-ir`
- `obj`
- `exe`
- `staticlib`

但当前回归面仍然主要靠 crate 内部测试中的 substring 断言。这对于“后端刚起步”是够用的，但一旦后续继续补：

- 更多 lowering 规则
- 更多 toolchain 分支
- 更复杂的失败路径

只靠 crate-local 测试会很快出现两个问题：

1. 很难证明真实 `ql build` CLI 路径没有回归
2. 很难锁定“最终产物长什么样”，只能锁住零散片段

所以这一刀要补的不是新 artifact，而是 P4 自己的黑盒回归地基。

## 设计目标

- 用真实 `ql` 二进制驱动 `ql build`
- 为 `llvm-ir`、`obj`、`exe`、`staticlib` 建立稳定快照
- 让 mock toolchain 路径进入黑盒回归，而不是只停留在单元测试
- 为 future lowering 保留可持续扩容的 fixture/snapshot 结构

## 结构

新增根目录测试资源：

```text
tests/
  codegen/
    pass/
      minimal_build.ll
      minimal_build.obj.txt
      minimal_build.exe.txt
      minimal_library.staticlib.txt
    fail/
      unsupported_closure_build.ql
      unsupported_closure_build.stderr
```

说明：

- 源码输入继续复用 `fixtures/codegen/pass/`
- 黑盒失败夹具单独放在 `tests/codegen/fail/`
- 期望输出放在 `tests/codegen/pass/`，由集成测试驱动真实 `ql` 后与之对比

## harness 行为

新增 `crates/ql-cli/tests/codegen.rs`：

- 逐个执行 pass case
- 为每个 case 创建独立临时输出目录
- 必要时注入 mock compiler / mock archiver
- 在静态库场景下按平台固定 archiver CLI 风格：Windows 走 `lib`，Unix 走 `ar`
- 对成功 case 断言：
  - 退出码为 `0`
  - `stderr` 为空
  - 产物内容与快照一致
  - 中间 `.codegen.*` 工件已清理
- 对失败 case 断言：
  - 退出码为 `1`
  - `stdout` 为空
  - `stderr` 与快照一致

## 快照策略

### LLVM IR

LLVM IR 快照允许带少量模板化占位，例如：

- `{{TARGET_TRIPLE}}`

这样可以同时兼容 Windows / Linux，而不需要给每个平台复制一份大快照。

### mock artifacts

`obj` / `exe` / `staticlib` 当前仍使用 mock toolchain black-box 回归，因此快照可以先锁成：

- `mock-object`
- `mock-executable`
- `mock-staticlib`

这类快照不是为了证明“产物可运行”，而是为了锁定：

- CLI 选项接线
- toolchain env 覆盖
- program/library mode 选择
- archive / link 路径是否走到了真正的 artifact 输出

## 为什么现在优先做这一刀

比起继续补 `dylib` 或 runtime glue，这一刀更适合现在做，因为：

- 不会提前把 ABI/runtime 设计绑死
- 直接提高后续每一步的可回归性
- 能把已经完成的 P4.1 / P4.2 / P4.3 / P4.4 骨架真正锁住

这也是 P4 路线图里 “基础 codegen golden tests” 的第一次真正落地，而不是继续停留在规划文字里。
