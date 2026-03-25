# Phase 4 `extern "c"` Direct Call 设计

## 背景

P4 现在已经有了：

- `ql build --emit llvm-ir`
- `--emit obj`
- `--emit exe`
- `--emit staticlib`
- 黑盒 codegen golden harness

但当前还有一个非常具体的缺口：语言表面已经支持 `extern "c"` 顶层声明和 `extern` block，前端也能把它们 parse / fmt 出来，但后端还停留在：

- codegen 直接拒绝任何显式 ABI
- `extern` block 内的函数名在 resolve/typeck 上没有独立 callable identity
- `ql check` 对这类调用拿不到稳定的参数类型检查

如果继续把这层空着，后面无论做 FFI、runtime glue 还是更完整的链接，都要回头重拆“函数声明到底如何被引用”这一层抽象。

## 设计目标

- 支持 direct `extern "c"` function call lowering
- 覆盖两种声明来源：
  - 顶层 `extern "c" fn foo(...)`
  - `extern "c" { fn foo(...) }`
- 不把 runtime startup、导出 ABI、动态库一起塞进这一刀
- 把 callable identity 从“只有 `ItemId`”提升到“可引用函数声明”

## 核心抽象

新增 HIR 级 `FunctionRef`：

```text
FunctionRef::Item(ItemId)
FunctionRef::ExternBlockMember { block: ItemId, index: usize }
```

这样可以统一表达：

- 顶层 free function
- extern block 内部函数声明

而不必把 extern block 人工 flatten 成新的顶层 item，也不必让 resolve/typeck/codegen 各自再发明一套“函数定位方式”。

## 分层设计

### `ql-hir`

- 暴露 `FunctionRef`
- `Module::function(FunctionRef)` 统一返回 `hir::Function`
- `Module::function_owner_item(FunctionRef)` 提供 owner item 访问

### `ql-resolve`

- `ValueResolution` 新增 `Function(FunctionRef)`
- 顶层函数绑定到 `FunctionRef::Item`
- extern block 函数绑定到 `FunctionRef::ExternBlockMember`

### `ql-typeck`

- callable signature 不再假设“函数一定是 `ItemId`”
- extern block direct call 现在也参与参数个数和参数类型检查

### `ql-mir`

- `Constant::Function { function: FunctionRef, name }`
- direct call 在 MIR 层统一引用函数声明，而不是只会引用顶层 item

### `ql-codegen-llvm`

- 签名表改为按 `FunctionRef` 建立
- 有 body 的函数渲染成 `define`
- 无 body 且 `extern "c"` 的函数渲染成 `declare`
- direct call 可以调用上述 `declare`
- 当前仍只支持 `extern "c"`，其他 ABI 返回结构化 diagnostics

## 明确不做

本切片仍然不做：

- `extern "c"` 导出函数定义
- runtime startup object
- C ABI struct / tuple / aggregate passing
- 动态库
- 更复杂 ABI family

这刀的目标不是“FFI 全做完”，而是先把“外部函数声明如何进入编译管线”固定成稳定抽象。

## 验证

这一刀至少要覆盖：

- resolve：extern block function name -> callable identity
- typeck：extern block direct call 参数诊断
- analysis query：hover / definition 能命中 extern declaration
- codegen crate：`declare @symbol` + `call @symbol`
- CLI 黑盒 snapshots：
  - pass: extern C direct-call LLVM IR
  - fail: unsupported non-C ABI declaration

## 结果

完成后，P4 会从“只能处理纯 Qlang free function”推进到“可以直接引用 C ABI 声明并把调用留给宿主链接阶段”，这正是后续 FFI/runtime 路线需要的第一块真实地基。
