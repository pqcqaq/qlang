# Phase 5 Real FFI Integration Harness

## 背景

P5 的第一刀已经把顶层 `extern "c"` 导出定义接进了真实后端，但这还不够。

如果没有真实宿主侧回归，项目仍然会存在一个常见风险：

- parser 通过
- typeck 通过
- LLVM IR 看起来正确
- 真正被 C 链接和调用时才暴露问题

所以第二刀不该继续补抽象，而应该先把真实集成测试面建起来。

## 目标

- 建立 `tests/ffi/` 目录作为真实宿主集成夹具入口
- 用真实 `ql build --emit staticlib` 构建导出 C 符号的 Qlang 库
- 用真实 C harness 链接并运行该库

## 实现边界

当前 harness 刻意保持克制：

- 已覆盖静态库直连与共享库运行时装载两条路径
- 先只覆盖简单 `extern "c"` 标量函数导出
- 先只覆盖 C 宿主，不同时引入 Rust/C++ 宿主

## 当前测试流

当前静态库流：

1. 读取 `tests/ffi/pass/<name>.ql`
2. 读取匹配的 `tests/ffi/pass/<name>.c`
3. 调用真实 `ql build --emit staticlib`
4. 用 clang-style compiler 编译并链接 C harness
5. 运行生成的宿主可执行文件
6. 以进程退出码作为 smoke 验证结果

当前共享库流：

1. 读取 `tests/ffi/pass/<name>.ql`
2. 读取匹配的 `tests/ffi/pass/<name>.shared.c`
3. 调用真实 `ql build --emit dylib`
4. 调用 `ql ffi header`
5. 用 clang-style compiler 编译 loader-style C harness
6. 宿主进程通过 `LoadLibraryA` / `dlopen` 显式装载共享库并解析导出符号
7. 以进程退出码作为 smoke 验证结果

## 为什么允许“工具链缺失时跳过”

这层测试依赖真实宿主工具链，而不是 mock wrapper。

为了保持主干测试稳定，当前策略是：

- 如果 `QLANG_CLANG` / `QLANG_AR` 未设置，且 PATH 上也找不到 clang-style compiler / archiver，则测试直接跳过
- 一旦工具链存在，就运行真实端到端回归

这不是降低标准，而是避免把“机器缺依赖”伪装成“编译器语义失败”。

## 后续扩展顺序

这层 harness 建好之后，合理的下一步是：

1. 增加更多 exported symbol case
2. 增加 struct / pointer / buffer 边界 case
3. 增加错误路径和 ABI mismatch diagnostics fixture
4. 再扩展到 Rust via C ABI 的宿主集成样例
