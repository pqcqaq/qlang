# 类型系统

## 设计目标

Qlang 的类型系统目标：

1. 对业务代码友好
2. 对系统代码足够强
3. 对编译器优化足够清晰
4. 对 LSP 和重构工具足够稳定

## 核心类型构造

### 基础类型

- `Bool`
- `Int`, `I8`, `I16`, `I32`, `I64`, `ISize`
- `UInt`, `U8`, `U16`, `U32`, `U64`, `USize`
- `F32`, `F64`
- `Char`
- `String`
- `Bytes`
- `Void`

### 复合类型

- 数组 `[T; N]`
- 切片 `Slice[T]`
- 元组 `(A, B, C)`
- 结构体 `struct`
- 枚举 `enum`
- 可调用类型 `(A, B) -> C`
- 闭包类型，由编译器合成

### 标准代数类型

- `Option[T]`
- `Result[T, E]`
- `Never`

这两个类型默认进入 prelude。

`Never` 作为底类型，用于表示不会正常返回的控制流，例如 `panic`、`abort`、无限循环和穷尽性分析中的不可能分支。

## 函数与可调用类型

函数需要作为一等值，典型场景包括：

- `map` / `filter` / `fold`
- retry / middleware / handler pipeline
- 回调和事件系统
- 任务调度和并发组合

### 统一的可调用类型语法

Qlang 建议把源码层的函数类型统一写成：

```ql
(A, B) -> C
```

例如：

```ql
let parser: (String) -> Result[Int, ParseError]
let combine: (Int, Int) -> Int
let thunk: () -> String
```

这比 `fn(A, B) -> C` 更接近函数类型的直觉表示，也能减少与函数声明语法的视觉冲突。

### 哪些值可以赋给可调用类型

- 顶层函数
- 关联函数
- 非捕获闭包
- 捕获闭包

这些值都表现为“可调用值”，底层表示可以不同。

### 性能策略

“函数是一等值”不应默认引入性能惩罚。建议编译器策略：

- 静态可知的可调用值优先直接内联或静态调用
- 泛型上下文优先单态化
- 只有在值逃逸、装箱或擦除后，才退化为间接调用

## 闭包与捕获模型

闭包除了字面量语法，还需要明确捕获规则。

### 默认捕获

建议编译器按最小能力捕获：

- 只读使用时，按只读借用捕获
- 需要修改外部变量时，按可变借用捕获
- 当闭包逃逸、跨任务或显式要求所有权时，按 move 捕获

### 显式 `move`

```ql
let task = move () => writer.flush()
```

`move` 用于强制把依赖值移入闭包，特别适合：

- `spawn`
- 长生命周期回调
- 异步任务
- FFI 回调包装

### 闭包返回与逃逸

如果闭包被返回、存储到字段、跨线程传递或脱离当前栈帧，就需要更严格的生命周期与所有权要求。

## 高阶函数设计建议

高阶函数按常规能力处理。

```ql
fn map[T, U](items: List[T], f: (T) -> U) -> List[U]

fn retry[T](times: Int, op: () -> Result[T, Error]) -> Result[T, Error]

fn make_adder(delta: Int) -> (Int) -> Int {
    return (x) => x + delta
}
```

这类模式在 collections、middleware、pipeline、async 组合器里都非常常见。

## 方法值与方法引用

方法值会把 receiver 语义带进函数类型系统，需要单独控制范围。

### P0 结论

- 顶层函数和关联函数是一等值
- 闭包是一等值
- 实例方法引用不作为 P0 必备语法糖

```ql
let ctor: (UserId, String) -> User = User.new
```

这类写法支持；但像 `user.rename` 这种绑定实例方法后转成函数值的语法，不进入 P0。需要时可显式写成：

```ql
let rename = (next) => user.rename(next)
```

这样可以避免 receiver 可变性和 move 语义过早进入语法层。

## 异步可调用值

P0 不单独引入复杂的 async function type 语法。先把它视为“返回 future-like 值的可调用对象”，待 async runtime 和 effect 模型稳定后，再决定是否需要：

- `async (A) -> T`
- 或显式的 `Future[T]`

## 元组与多值返回

“多返回值”统一视为返回元组值。

```ql
fn split_version(text: String) -> Result[(Int, Int, Int), ParseError]
```

这样做的直接收益：

- 类型系统不需要额外发明“多返回参数”实体
- 与解构绑定、模式匹配、泛型和高阶函数天然兼容
- LSP、文档生成和重构工具只需要理解元组
- 错误模型不会被 Go 风格 `(value, err)` 稀释

## 何时不用元组

元组适合短小、顺序明确、局部使用的多值返回；如果返回值具备明确领域语义，应优先定义具名类型：

```ql
data struct Bounds {
    min: Int,
    max: Int,
}

fn bounds(items: List[Int]) -> Result[Bounds, EmptyError]
```

这样 API 的文档性和可读性更强。

## 泛型与约束

Qlang 使用参数化泛型和 trait 约束：

```ql
fn max[T: Ord](left: T, right: T) -> T {
    if left >= right { left } else { right }
}
```

约束语法应尽量简单，复杂约束放到 `where` 子句中，以保证签名可读性。

## trait / protocol 模型

Qlang 需要类似 Rust trait 和 Swift protocol 的抽象能力，但要控制语义负担：

- trait 用于行为约束和静态分发
- object-safe trait 用于有限动态分发
- 自动派生常见 trait，例如 `Eq`、`Hash`、`Debug`
- orphan rule 需要存在，但规则要尽量可解释

## 类型推断策略

推断是为了减少样板，不是为了制造谜语。建议策略：

- 局部变量允许推断
- 闭包参数和返回值尽量推断
- 公共 API 的返回值必须显式
- 跨模块边界的复杂泛型尽量显式

这能在“写起来快”和“读起来清楚”之间取得平衡。

## 流敏感类型系统

Qlang 应把流敏感类型分析作为核心能力，而不是后期优化。这一点综合借鉴 Kotlin 的 smart cast 和 TypeScript 的 narrowing：

- 类型检查器追踪 `if`、`match`、`guard`、提前返回后的事实
- 可空类型在非空判断后自动收窄
- 判别字段可驱动联合类型或枚举变体的安全访问
- 某些用户定义函数可声明为类型谓词，辅助收窄

示意：

```ql
fn is_ipv4(addr: IpAddr) -> addr is Ipv4Addr;
```

这类能力的收益很高，但必须建立在清晰的控制流图和稳定的 HIR/MIR 之上。

## `satisfies` 与保真推断

Qlang 应支持一种“检查但不擦除具体性”的能力，借鉴 TypeScript 的 `satisfies`：

- 校验对象、常量或配置是否满足某个目标类型
- 保留表达式本身更具体的字段信息和字面量信息
- 避免“为了做校验而把值整体抬升成宽类型”

这类机制非常适合配置对象、编译期表、路由定义和构建描述。

## 可空性

Qlang 不提供默认可空引用：

- 没有“什么都能是 null”的模型
- 需要缺失值就用 `Option[T]`
- FFI 接收到的空指针，需要在边界处转换成受控类型

## 效果与危险边界

效果系统要分阶段落地，不能一口吃满。

### P0

- `unsafe`
- `async`
- `ffi`

这三类边界先显式化，并纳入类型检查和文档生成。

### P1

- `throws` 风格的错误效果汇总
- `Send` / `Sync` 类并发能力约束

### P2

- 更完整的效果推断，如 `io`, `alloc`, `blocking`

## 类型系统中的红线

- 不引入默认动态类型
- 不允许隐式缩窄转换
- 不提供会破坏布局可预测性的隐藏装箱
- 不在语言核心里内建难以解释的“神奇协变/逆变捷径”
- 不引入默认结构类型系统取代名义类型系统

现代语言的友好，不是把规则取消，而是把规则设计得稳定、统一、能推理。
