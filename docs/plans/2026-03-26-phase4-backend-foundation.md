# Phase 4 后端地基设计

## 背景

P3 现在已经提供了可查询的 `HIR -> MIR -> ownership facts` 基线，但 P4 不能直接把“完整 LLVM 后端、对象文件、链接器驱动、运行时 ABI”一次性糊在一起做。那样最容易形成两个长期问题：

1. 后端逻辑被塞进 `ql-cli` 或 `ql-analysis`，语义层和产物层耦死
2. 一开始就依赖系统 LLVM 安装，导致开发、测试和 CI 都不稳定

2026-03-26 在当前开发机上实际探测到：

- `clang --version` 不可用
- `llvm-as --version` 不可用

这说明 P4 的第一刀不能假设“机器上已经有完整 LLVM 工具链”。因此第一切片要先把后端边界和 LLVM IR 产物闭环做出来，再把对象文件、可执行文件、链接器探测放到下一刀。

## 候选方案

### 方案 A：直接用 `inkwell` 输出对象文件

优点：

- API 相对 `llvm-sys` 更友好
- 可以直接走 `TargetMachine` 写对象文件

缺点：

- 仍然强依赖本机 LLVM 版本和安装方式
- Windows 环境配置和 CI 会比现在脆弱很多
- 当前阶段会把“后端语义”与“系统工具链可用性”绑在一起

### 方案 B：直接用 `llvm-sys`

优点：

- 控制力最高

缺点：

- 当前阶段工程收益最低
- unsafe 面积和维护成本都明显偏高
- 不适合作为 P4 第一切片

### 方案 C：先用纯 Rust 生成文本 LLVM IR，再补 object/link driver

优点：

- 不依赖当前机器预装 LLVM
- 单元测试、golden test、CLI smoke test 都能稳定运行
- 后端边界可以先冻结，不会把 LLVM 绑定细节污染到语义层

缺点：

- 第一切片不能立即承诺“完整原生可执行文件产物”
- 需要后续再补 toolchain discovery 和链接步骤

推荐采用方案 C。

## 第一切片目标

P4.1 只解决一件事：把一个受控的 MIR 子集稳定 lower 成 `.ll` 文件，并通过统一 `ql build` 驱动落地。

本切片交付：

- 新增 `ql-driver`，负责构建请求、文件读取、分析调用、产物落盘
- 新增 `ql-codegen-llvm`，负责 MIR 子集到文本 LLVM IR 的 lowering
- 新增 `ql build`
- 默认输出 `target/ql/<profile>/<stem>.ll`
- 建立 codegen 单元测试、driver 测试、CLI smoke test

本切片明确不做：

- object / exe / staticlib
- 系统 LLVM/Clang 探测
- runtime ABI
- 闭包环境 lowering
- 结构体布局和聚合类型 codegen
- 方法、trait、impl、extern ABI 完整支持
- debug info

## 分层设计

```text
ql-cli
  -> ql-driver
       -> ql-analysis
            -> HIR / resolve / typeck / MIR
       -> ql-codegen-llvm
            -> textual LLVM IR
```

约束如下：

- `ql-analysis` 继续只负责语义和中间表示，不负责产物
- `ql-codegen-llvm` 不读取文件，不打印 CLI 输出，不做目录扫描
- `ql-driver` 负责把“源码文件 -> 分析 -> codegen -> 写产物”串起来
- `ql-cli` 只负责参数解析、错误码和最终用户交互

这条边界比“先把 build 直接塞进 CLI”更适合后续继续扩成：

- `ql run`
- profile 管理
- 目标三元组和链接器选择
- workspace/package 构建

## P4.1 支持矩阵

入口约束：

- 只接受顶层 free function `main`
- `main` 暂时必须无参数
- `main` 返回类型暂时只支持 `Int` 或 `Void`

函数约束：

- 仅支持顶层 free function body
- 不支持 receiver method、trait method、impl method、extend method
- 不支持泛型、`async fn`、`unsafe fn`、非默认 ABI

类型约束：

- 支持 `Bool`
- 支持整数标量：`Int` / `UInt` / `I8` / `I16` / `I32` / `I64` / `ISize` / `U8` / `U16` / `U32` / `U64` / `USize`
- 支持 `Void`
- 当前不支持 `String`、`Bytes`、pointer、tuple、callable value、struct/enum 聚合

MIR 约束：

- `StatementKind::Assign`，且目标 `Place` 不能有 projection
- `StatementKind::BindPattern`，且 pattern 必须是单一 binding
- `StatementKind::Eval`
- `StatementKind::StorageLive` / `StorageDead` 作为 no-op 保留
- `TerminatorKind::Goto`
- `TerminatorKind::Branch`
- `TerminatorKind::Return`
- `TerminatorKind::Terminate`

Rvalue 约束：

- `Use`
- `Call`，但 callee 目前只支持 direct top-level function item
- `Binary`
- `Unary::Neg`

明确报错的高风险语义：

- closure
- tuple/array/struct aggregate
- match / for / cleanup
- member / index / field projection
- indirect callable
- string literal

## LLVM IR 生成策略

P4.1 不追求 SSA 漂亮度，优先追求正确边界和易维护实现。

具体策略：

- 每个 MIR local 在函数入口生成一个 `alloca`
- regular param 进入函数后立刻 `store` 到对应 local slot
- 后续表达式通过 `load` / `store` 操作这些 slot
- block 直接映射为 LLVM label
- 临时值使用简单 `%tN` 编号
- 当前不引入 mem2reg、自定义优化或 phi 构造

这样做的理由很直接：

- 先让 MIR 和 LLVM IR 一一对应，便于调试和 golden test
- 不把“优化 IR”问题提前到 P4.1
- 为后续 object emission 和 verifier 接入保留稳定结构

## 错误模型

P4.1 的失败分三类：

1. 读取/写入文件失败
2. 分析阶段已有 diagnostics
3. codegen unsupported

其中第 3 类必须继续走 `Diagnostic`，而不是返回一串裸字符串。原因是后续：

- CLI 需要统一渲染
- LSP / build server 将来可能复用
- codegen fail fixture 需要稳定 snapshot

## 测试策略

本切片新增三层测试：

- `ql-codegen-llvm` 单元测试：锁定 IR 结构和 unsupported diagnostics
- `ql-driver` 测试：锁定默认输出路径、文件写入和错误透传
- `ql-cli` smoke test：锁定 `ql build` 命令接线

golden test 先不做复杂 snapshot harness，当前先用稳定 substring 断言；等 object / exe / linker driver 落地后，再把 `tests/codegen/` 扩成真正的 compile-pass / compile-fail 体系。

## 后续切片

P4.2：

- toolchain discovery
- `--emit obj`
- `--emit asm`
- 基础 target triple / profile 选项

P4.3：

- `--emit exe`
- linker driver
- Windows / Linux 至少一条可验证路径

P4.4：

- 聚合类型布局
- extern ABI
- runtime glue
- codegen golden tests 扩容

P4 的关键不是“今天就把 LLVM 全做完”，而是现在先把后端边界做对，保证后续每一刀都能叠加，而不是返工。
