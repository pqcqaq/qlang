# 2026-03-26 P5 Dynamic Library Emission

## 背景

到这一步为止，Qlang 已经有了：

- 稳定的顶层 `extern "c"` 函数定义导出
- `staticlib` 构建路径
- `ql ffi header` 头文件投影

缺口在于 exported C surface 还只能以静态库形态交付。对很多宿主工程来说，这意味着：

- 调试和分发成本更高
- 无法直接验证 shared-library 场景下的链接行为
- Phase 5 的“最小 C 互操作闭环”仍然缺一条重要交付形态

但这里不能直接把“任意共享库”一口气做满。那样会同时把 symbol visibility、平台差异、runtime glue、linker family discovery 和 ABI 承诺一起卷进来，风险太高。

## 设计决策

当前这刀选择做一个受约束的 `dylib` slice：

- CLI 形态：`ql build <file> --emit dylib`
- codegen 形态：沿用现有 library mode
- 导出约束：至少存在一个 public 顶层 `extern "c"` 函数定义
- 平台命名：
  - Windows: `<stem>.dll`
  - macOS: `lib<stem>.dylib`
  - Linux/Unix: `lib<stem>.so`

这样做的好处是：

1. 不扩张语言面，只扩张 artifact pipeline。
2. shared-library 输出仍然绑定在已经验证过的 exported C ABI surface 上。
3. 可以把风险集中在 driver/toolchain 层，而不是把 parser、HIR、typeck、MIR 和 codegen 重新搅动一遍。

## 实现边界

实现拆成三层：

### 1. Driver

- `BuildEmit` 新增 `DynamicLibrary`
- `build_file` 在 `emit == dylib` 时：
  - 先从 HIR 投影 exported C symbol 列表
  - 如果为空，直接返回 invalid input
  - 否则走和 object/exe/staticlib 一致的中间 `.codegen.ll` / `.codegen.obj/.o` 路径

### 2. Toolchain

- 新增 `link_object_to_dynamic_library`
- macOS 使用 `-dynamiclib`
- 其他平台使用 `-shared`
- Windows 会把 exported symbol 列表转成重复的 `/EXPORT:<symbol>` linker 参数

### 3. CLI / 黑盒测试

- `ql build --emit dylib` 正式进入 CLI 参数解析
- 单测覆盖 CLI 到 driver 的动态库路径
- 黑盒 codegen harness 同时覆盖：
  - `dylib` 成功构建
  - 没有 exported C symbol 时拒绝构建

## 刻意不做的部分

这一刀仍然明确不做：

- 任意 shared-library surface
- richer ABI layout / visibility 策略
- import surface 的自动头文件投影（后续已由 P5 import-surface header projection 补上）
- runtime startup object
- 更完整的 linker-family 发现逻辑

也就是说，当前 `dylib` 不是“Qlang 已经支持完整动态库生态”，而是“在已有 exported C ABI 地基上，把共享库交付形态补齐”。

## 验证要求

这一刀要求至少覆盖四类验证：

- `cargo test`
- `cargo clippy --workspace --all-targets -- -D warnings`
- 黑盒 `codegen` 快照
- 真实 `ql build --emit dylib` smoke run

在 Windows 上，还应额外确认 DLL 导出表里能看到预期符号，例如 `q_add`。
