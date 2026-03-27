# Phase 5 Shared-Library FFI Harness

## 背景

在 `ql build --emit dylib` 落地之后，项目已经可以产出共享库，但验证仍然停留在：

- black-box mock toolchain 快照
- 单次真实 `ql build --emit dylib` smoke run

这还不够。对 FFI 来说，真正有价值的不是“文件生成成功”，而是“宿主能不能真的装载并调用导出符号”。

## 目标

- 让 `tests/ffi/` 不再只覆盖 `staticlib`
- 增加共享库宿主回归
- 保持 ABI surface 仍然受约束，不顺手扩张语言面

## 设计

当前共享库 harness 选择 runtime loading，而不是平台特定链接方案：

- Windows: `LoadLibraryA` + `GetProcAddress`
- Unix: `dlopen` + `dlsym`

这样做的原因：

1. 不要求 import library / `.lib` / `rpath` / install-name 等平台细节先到位。
2. 能直接验证“导出表里是否真的有预期符号”。
3. 不会把当前切片膨胀成更完整的 linker/runtime 方案。

## 测试流

1. `ql build <file> --emit dylib`
2. `ql ffi header <file>`
3. 用真实 clang-style compiler 编译共享库宿主 C harness
4. 宿主进程接收共享库绝对路径参数
5. 在进程内显式装载库并解析导出符号
6. 调用导出函数并以退出码断言结果

## 刻意不做

这一步仍然不做：

- import library 生成
- 平台级安装/分发方案
- richer ABI/layout coverage
- 自动生成 loader bridge

它只回答一个更基础的问题：Qlang 现在产出的共享库，是否已经能被真实 C 宿主稳定装载并调用。
