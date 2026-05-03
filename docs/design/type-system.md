# 类型系统

## 设计目标

Qlang 的类型系统要同时满足四件事：

1. 对业务代码友好
2. 对系统代码足够强
3. 对优化和 lowering 足够清晰
4. 对 LSP / 重构工具足够稳定

## 核心类型

- 基础类型：`Bool`、`Int`、各类整数、浮点、`Char`、`String`、`Bytes`、`Void`
- 复合类型：数组 `[T; N]`、切片 `Slice[T]`、元组 `(A, B)`、`struct`、`enum`
- 可调用类型：`(A, B) -> C`
- 底类型：`Never`

`Never` 用于 `panic`、`abort`、无限循环和穷尽性分析中的不可能分支。

## 标准代数类型

语言层设计上，`Option[T]` 和 `Result[T, E]` 应该是核心代数类型；当前仓库内可用的是普通 package `std.option` / `std.result`，不是 prelude。

- `std.option` 提供 `Option[T]`、`some`、`none_option`、`is_some`、`is_none`、`unwrap_or`、`or_option`
- `std.result` 提供 `Result[T, E]`、`ok`、`err`、`is_ok`、`is_err`、`unwrap_result_or`、`or_result`、`error_or`、`ok_or`、`to_option`、`error_to_option`
- concrete `IntOption` / `BoolOption` / `IntResult` / `BoolResult` 只保留为兼容面

## 可调用值

函数、关联函数和闭包都应该是可调用值。

推荐的源码层函数类型写法是：

```ql
(A, B) -> C
```

这比 `fn(A, B) -> C` 更适合作为类型注解，也更适合高阶函数和 LSP 显示。

### 规则

- 顶层函数、关联函数、非捕获闭包和捕获闭包都可以赋给可调用类型
- 只有在值逃逸、装箱或擦除后，才退化为间接调用
- 实例方法引用不作为 P0 必备语法糖

## 元组与多值返回

多返回值统一视为返回元组。

这样做的好处是：

- 不需要额外引入“多返回参数”实体
- 与解构绑定、模式匹配、泛型和高阶函数天然兼容
- LSP、文档和重构工具只需要理解元组

如果返回值有明确领域语义，应优先定义具名 `struct`。

## 泛型与约束

- 泛型采用参数化泛型
- 约束采用 trait 约束
- 简单约束写在签名里，复杂约束放 `where`
- 公共 API 的复杂泛型边界尽量保持显式

`std.option` / `std.result` / `std.array` 说明了当前语言面已经在支持 generic carrier 和 canonical collection API，但完整单态化、泛型 alias 和更宽泛型导入仍在推进中。

## trait / protocol

Qlang 需要行为抽象，但要保持语义可解释：

- trait 用于静态分发和能力约束
- object-safe trait 才考虑有限动态分发
- 常见 trait 可自动派生
- orphan rule 需要存在

## 推断策略

推断是为了减少样板，不是为了制造谜语：

- 局部变量允许推断
- 闭包参数和返回值尽量推断
- 公共 API 的返回值应尽量显式
- 跨模块边界的复杂泛型尽量显式

## 可空性

Qlang 不采用默认 `null` 模型。

- 缺失值用 `Option[T]`
- FFI 边界把空指针转换成受控类型

## 流敏感与保真推断

Qlang 应把流敏感分析作为核心能力：

- `if`、`match`、`guard` 和提前返回后的事实要进入类型检查
- 可空类型在非空判断后自动收窄
- 判别字段和模式匹配要能驱动安全访问
- 保真推断要校验但不擦除具体性，适合配置对象和编译期表

## 效果边界

P0 先显式化这些边界：

- `unsafe`
- `async`
- `ffi`

后续再扩：

- `throws`
- `Send` / `Sync`
- 更完整的 `io` / `alloc` / `blocking`

## 红线

- 不引入默认动态类型
- 不允许隐式缩窄转换
- 不提供破坏布局可预测性的隐藏装箱
- 不用“神奇协变/逆变捷径”替代清晰规则
- 不让结构类型系统替代名义类型系统
