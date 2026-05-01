# 语法草案

## 目标

这一版提供 Qlang 的表层语法骨架，供 parser、formatter、LSP、语义分析和文档生成共享。语法目标：

1. 熟悉，能让有 C / Go / Kotlin / TypeScript / Rust 背景的人快速读懂
2. 稳定，避免到处是边角规则
3. 可工具化，方便 parser、formatter、LSP 和 code action
4. 少仪式感，把高频复杂度移交给编译器

## 总体风格

### 语法气质

当前定稿方向：

- 模块路径使用 `.`，不用 `::`
- 泛型参数使用 `[]`，不用 `<>`
- 闭包使用 `=>`
- 实例化以命名字段和关联构造函数为主
- 方法系统使用 `impl`，但 receiver 规则更简化

### 一个完整示例

```ql
package http.server

use std.io
use std.net.{TcpListener, TcpStream}

pub data struct Config {
    host: String,
    port: Int,
    backlog: Int = 1024,
}

pub enum HttpError {
    Io(io.Error),
    InvalidRequest(String),
}

impl Config {
    pub fn addr(self) -> String {
        f"{self.host}:{self.port}"
    }
}

fn parse_port(text: String) -> Result[Int, HttpError] {
    match text.parse_int() {
        Some(value) if value > 0 => Ok(value),
        _ => Err(HttpError.InvalidRequest("invalid port")),
    }
}

pub async fn serve(config: Config) -> Result[Void, HttpError] {
    let listener = TcpListener.bind(config.addr())?;

    for await stream in listener.incoming() {
        spawn handle(stream?);
    }

    Ok(())
}

async fn handle(stream: TcpStream) -> Result[Void, HttpError] {
    defer stream.close()
    Ok(())
}
```

## 1. 词法系统

### 标识符

- 标识符支持 Unicode 字母、数字和下划线
- 不能以数字开头
- 保留字不能直接用作标识符
- 允许通过转义标识符保留与外部系统的互操作能力，草案形式为 `` `type` ``

### 命名约定

这不是强制语法，但建议作为官方风格：

- 包、模块、文件名：`snake_case`
- 局部变量、函数、字段：`snake_case`
- 类型、trait、枚举变体：`PascalCase`
- 常量：`SCREAMING_SNAKE_CASE`

### 注释

```ql
// 行注释

/*
   块注释
*/

/// 文档注释，附着到下一个声明
```

### 保留字

当前建议保留字集合：

- `package`
- `use`
- `pub`
- `const`
- `static`
- `let`
- `var`
- `fn`
- `async`
- `await`
- `spawn`
- `defer`
- `return`
- `break`
- `continue`
- `if`
- `else`
- `match`
- `for`
- `while`
- `loop`
- `in`
- `where`
- `struct`
- `data`
- `enum`
- `trait`
- `impl`
- `extend`
- `type`
- `opaque`
- `extern`
- `unsafe`
- `is`
- `as`
- `satisfies`
- `none`
- `true`
- `false`

### 字面量

当前建议支持：

- 整数字面量：`0`, `42`, `0xff`, `0b1010`
- 浮点字面量：`3.14`, `1e9`
- 布尔字面量：`true`, `false`
- 字符字面量：`'a'`
- 字符串字面量：`"hello"`
- 插值字符串：`f"hello {name}"`
- 空值字面量：`none`

- `none` 只代表 `Option[T]` 的空分支
- `f"..."` 作为唯一官方插值字符串写法

实现状态：`none` / prelude `Option[T]` / `Result[T, E]` 仍是语法设计目标；当前可执行 stdlib 只开放普通 package `std.option` / `std.result` 里的 concrete `IntOption` / `BoolOption` / `IntResult` / `BoolResult`。

## 2. 模块与导入

### 包声明

每个源文件可显式声明自己的模块路径：

```ql
package compiler.parser
```

规则：

- 包路径和文件系统层级一一映射
- 一个包是构建、文档和可见性的基本单位
- 后续 package manifest 统一由 `qlang.toml` 管理

### 导入

```ql
use std.io
use std.net.TcpListener
use std.net.{TcpListener, TcpStream}
use std.collections.HashMap as Map
```

规则：

- 使用 `.` 作为命名路径分隔符
- 支持分组导入
- 支持 `as` 重命名
- wildcard import 不进入 P0

### 可见性

P0 仅定义：

- 默认私有
- `pub` 对包外可见

P1 可以再考虑：

- `pub(package)`
- `pub(module)`

## 3. 声明系统

### 顶层声明

当前完整集合：

- `const`
- `static`
- `type`
- `opaque type`
- `struct`
- `data struct`
- `enum`
- `trait`
- `impl`
- `extend`
- `fn`
- `extern`

### 常量与静态值

```ql
const DEFAULT_PORT: Int = 8080
static BUILD_LABEL: String = "dev"
```

建议：

- `const` 用于编译期常量
- `static` 用于有固定地址的全局值
- 全局可变值不是 P0 推荐路径

### 类型别名与不透明类型

```ql
type UserMap = HashMap[String, User]
opaque type UserId = U64
```

规则：

- `type` 是纯别名
- `opaque type` 用于创建零成本但具备类型隔离的领域类型

## 4. 类型声明

### `struct`

用于普通名义类型。

```ql
pub struct Socket {
    fd: Int,
    open: Bool = true,
}
```

规则：

- 使用命名字段
- 字段可以声明默认值
- 未提供默认值的字段必须在实例化时显式给出

### `data struct`

用于数据承载类型，编译器自动生成高频样板。

```ql
pub data struct User {
    id: UserId,
    name: String,
    age: Int = 0,
}
```

默认派生：

- `Eq`
- `Hash`
- `Debug`
- 解构支持
- `copy(...)`

### `enum`

Qlang 的 `enum` 是代数数据类型，不只是整数标签。

```ql
pub enum Message {
    Ping,
    Text(String),
    Move { x: Int, y: Int },
    Error { code: Int, reason: String },
}
```

支持：

- 无负载变体
- 元组式负载
- 命名字段负载

### `trait`

```ql
pub trait Writer {
    fn write(var self, bytes: Bytes) -> Result[Int, IoError]
    fn flush(var self) -> Result[Void, IoError]
}
```

设计原则：

- 小 trait 优先
- trait 是抽象边界，不是大杂烩命名空间

## 5. 泛型系统表层语法

### 泛型参数

```ql
fn max[T: Ord](left: T, right: T) -> T

struct Box[T] {
    value: T,
}
```

### `where` 子句

```ql
fn merge[K, V](left: Map[K, V], right: Map[K, V]) -> Map[K, V]
where
    K: Eq + Hash,
    V: Clone
```

### 类型实例化

Qlang 的类型应用统一使用 `[]`：

```ql
let users: List[User]
let cache = HashMap[String, Int].new()
let box = Box[Int] { value: 42 }
```

规则：

- 类型位置显式写 `Type[Args]`
- 值构造时若上下文足够，可省略部分类型参数，由编译器推断
- 公共 API 建议在复杂泛型处保持显式

## 6. 值实例化系统

这是你特别提到要补全的部分。Qlang 不应把“创建对象”设计得杂乱无章，应该有统一规则。

### 结构体实例化

```ql
let user = User {
    id: UserId.from(1),
    name: "Lin",
    age: 18,
}
```

支持字段简写：

```ql
let user = User { id, name, age }
```

字段默认值生效时，可省略该字段：

```ql
let user = User {
    id: UserId.from(1),
    name: "Lin",
}
```

### 数据实例复制

借鉴 Kotlin data class：

```ql
let older = user.copy(age: user.age + 1)
```

这取代 Rust 风格的 struct update 语法，减少样板和解析复杂度。

### 枚举变体实例化

```ql
let ping = Message.Ping
let text = Message.Text("hello")
let moved = Message.Move { x: 10, y: 20 }
```

### 关联构造函数

实例化复杂类型时，推荐关联函数而不是过多特殊语法：

```ql
impl UserId {
    pub fn from(raw: U64) -> UserId {
        raw as UserId
    }
}

let id = UserId.from(1)
let map = HashMap[String, Int].new()
```

### 集合字面量

P0 建议支持：

```ql
let nums = [1, 2, 3]
let scores = ["alice": 10, "bob": 8]
let pair = (1, "a")
```

规则：

- `[]` 同时承担数组和映射字面量
- 若元素为 `key: value` 形式，则推断为 map literal
- 具体容器类型可由上下文或标准字面量协议决定

## 7. 函数与方法系统

这一节除了声明普通函数，还要覆盖一个完整语言必须具备的能力：

- 函数作为参数传递
- 函数作为返回值
- 函数存入变量和字段
- 闭包捕获外部环境
- 关联函数与实例方法的边界

### 函数声明

```ql
pub fn add(left: Int, right: Int) -> Int {
    left + right
}
```

参数规则：

- 形参名在前，类型在后
- 返回类型使用 `->`
- 函数体默认是块表达式

### 函数类型

源码层统一使用：

```ql
(A, B) -> C
```

示例：

```ql
let parse: (String) -> Result[Int, ParseError] = parse_port
let no_arg: () -> String = read_name
let reducer: (Int, Int) -> Int = add
```

这意味着 Qlang 明确支持函数作为一等值。

### 函数作为参数

```ql
fn map[T, U](items: List[T], f: (T) -> U) -> List[U]

fn retry[T](times: Int, op: () -> Result[T, Error]) -> Result[T, Error]
```

这是现代语言的基础能力，不应缺席。

### 函数作为返回值

```ql
fn make_adder(delta: Int) -> (Int) -> Int {
    return (x) => x + delta
}
```

### 函数作为字段

```ql
struct Route {
    path: String,
    handle: (Request) -> Response,
}
```

这对 middleware、router、scheduler、test hook 都很重要。

### 命名参数与默认参数

```ql
fn connect(host: String, port: Int = 8080, tls: Bool = true) -> Connection

let conn = connect(host: "localhost", tls: false)
```

规则：

- 默认参数在调用点静态展开
- 公共 API 推荐命名参数
- 同一调用中，不允许“位置参数夹杂在命名参数后面”

### 闭包

Qlang 采用 `=>`：

```ql
let double = (x) => x * 2
let sum = (a, b) => a + b
let handler = (req) => {
    log(req.path)
    Ok(Response.ok())
}
```

闭包参数与返回值尽量由上下文推断。

### 闭包捕获

```ql
let prefix = "hello"
let greet = (name) => f"{prefix}, {name}"

let send_task = move () => client.flush()
```

规则：

- 默认按最小需要捕获
- `move` 强制把依赖值移入闭包
- 跨任务、逃逸闭包和 FFI 回调通常需要 `move`

### 方法定义

方法通过 `impl` 块定义。

```ql
impl User {
    pub fn new(id: UserId, name: String) -> User {
        User { id, name }
    }

    pub fn display_name(self) -> String {
        self.name
    }

    pub fn rename(var self, next: String) -> Void {
        self.name = next
    }

    pub fn into_json(move self) -> String {
        Json.encode(self)
    }
}
```

说明：

- `Self` 表示当前 `impl` 正在实现的类型
- 关联函数推荐优先返回 `Self`，减少重复书写具体类型名

### receiver 规则

这是实例方法系统的核心。

- 首参数若为 `self`，则为实例方法
- 首参数若不是 `self`，则为关联函数
- `self` 表示只读接收者
- `var self` 表示可变接收者
- `move self` 表示消费接收者

这样开发者能表达三种最重要的语义：

1. 读
2. 改
3. 吃掉并转移所有权

### 调用方式

```ql
let name = user.display_name()
user.rename("NewName")
let json = user.into_json()

let id = UserId.from(1)
let user = User.new(id, "Lin")
```

### 关联函数作为值

```ql
let factory: (UserId, String) -> User = User.new
```

### 实例方法引用

P0 不把“绑定实例方法直接当值”做成核心语法糖。也就是说，不默认支持：

```ql
// P0 不作为主语法
let rename = user.rename
```

需要时应显式写成闭包：

```ql
let rename = (next) => user.rename(next)
```

这样 receiver 的只读、可变、消费语义更清楚。

### 扩展方法

```ql
extend String {
    fn to_port(self) -> Result[Int, ParseError] {
        self.parse_int().ok_or(ParseError.InvalidPort)
    }
}
```

规则：

- 必须显式导入
- 成员方法优先
- 不能静默覆盖原类型已有成员

## 8. 表达式系统

Qlang 应尽量保持“一个统一的表达式世界”。

### 基础表达式

- 字面量
- 标识符与路径
- 元组
- 数组 / map 字面量
- 结构体 / 枚举实例化
- 块表达式
- 函数调用
- 方法调用
- 字段访问
- 索引访问
- 闭包

### 块表达式

```ql
let port = {
    let raw = env.get("PORT")
    raw.parse_int().unwrap_or(8080)
}
```

规则：

- 块最后一个表达式是返回值
- 显式 `return` 直接结束当前函数

### 字段访问与索引

```ql
user.name
matrix[row][col]
config["host"]
```

### 调用与链式调用

```ql
response.body.trim().to_upper()
```

Qlang 允许链式调用，但不计划引入过度 DSL 化的隐式管道。

### 类型测试与转换

```ql
if value is TextMessage {
    return value.text
}

let bytes = ptr as *const U8
```

规则：

- `is` 用于运行期类型判定或判别式收窄
- `as` 用于显式转换
- 不允许隐式缩窄转换

### `satisfies`

```ql
let cfg = {
    host: "127.0.0.1",
    port: 8080,
} satisfies ServerConfig
```

语义：

- 做编译期形状或约束校验
- 不改变表达式更精细的推断信息

## 9. 语句与控制流

### 变量绑定

```ql
let name = "qlang"
var retries = 0
```

### 解构绑定

Qlang 应支持元组和数据类型的解构绑定：

```ql
let (host, port) = parse_addr("127.0.0.1:8080")?
let Point { x, y } = point
```

这也是“多返回值”在调用侧最自然的承接方式。

规则：

- `let` 当前仅接受不可失败模式
- 可能失败的模式绑定统一放进 `match`，避免再额外发明半套 `if let` / `while let` 语法而让 P0 复杂度膨胀

### 是否引入 `:=`

当前建议是不把 `:=` 放进核心语法。

原因：

- `let` / `var` 已经清楚表达了“声明且是否可变”
- `:=` 会重新引入“这是新声明还是赋值”的判断成本
- 它容易制造 shadowing 和局部重声明歧义
- 节省的字符很少，但会增加 parser、formatter、LSP 和 code action 的语义分支

所以 Qlang 在这个点上的取向是：**声明保持明确，不为了几字符简写牺牲一致性**。

### 赋值

```ql
retries = retries + 1
user.name = "new"
```

### `if`

```ql
let result = if port > 0 {
    "ok"
} else {
    "bad"
}
```

### `match`

```ql
match msg {
    Message.Ping => "ping",
    Message.Text(text) => text,
    Message.Move { x, y } => f"{x},{y}",
    Message.Error { reason, .. } => reason,
}
```

### 循环

```ql
for item in items {
    log(item)
}

for await line in stream {
    log(line)
}

while retries < 3 {
    retries = retries + 1
}

loop {
    if done() {
        break
    }
}
```

### 控制语句

```ql
return value
break
continue
defer socket.close()
```

## 10. 多返回值

Qlang 应支持“函数返回多个值”，但不单独设计 Go 风格的特殊返回机制，而是统一建立在元组之上。

### 基本形式

```ql
fn div_rem(left: Int, right: Int) -> (Int, Int) {
    return (left / right, left % right)
}

let (q, r) = div_rem(10, 3)
```

### 为什么这样设计

- 语义统一，多个值本质上就是一个元组值
- 不需要额外发明一套“多返回参数”规则
- 直接复用已有的解构、模式匹配和类型推断
- 对高阶函数、泛型和工具链来说都更简单

### 与错误处理的关系

Qlang 不建议走 Go 风格的 `(value, err)` 返回方式。错误仍然统一走：

```ql
Result[T, E]
```

例如：

```ql
fn parse_addr(text: String) -> Result[(String, Int), ParseError]
```

而不是：

```ql
// 不推荐方向
fn parse_addr(text: String) -> (String, Int, ParseError)
```

### 何时返回元组，何时返回 `data struct`

- 临时性、局部性、顺序明确的多值返回：用元组
- 有明确领域语义、字段较多、需要文档性的返回：用 `data struct`

例如：

```ql
fn bounds(items: List[Int]) -> Result[(Int, Int), EmptyError]

data struct ParseResult {
    host: String,
    port: Int,
    tls: Bool,
}

fn parse_server(text: String) -> Result[ParseResult, ParseError]
```

## 11. 模式系统

模式系统是类型系统和控制流系统的桥梁。

### 支持的模式

- `_`
- 文字匹配
- 变量绑定
- 元组模式
- 结构体模式
- 枚举变体模式
- 解构 + 守卫条件

### 示例

```ql
match result {
    Ok(value) => value,
    Err(err) if err.retryable() => retry(err),
    Err(err) => fail(err),
}

match point {
    Point { x: 0, y } => y,
    Point { x, y } => x + y,
}
```

### smart cast / 收窄

```ql
if user_opt != none {
    log(user_opt.name)
}

if msg is Message.Text {
    log(msg.text)
}
```

类型检查器应沿控制流保留这些事实。

## 12. 异步、错误与危险边界

### 错误传播

```ql
fn read_port() -> Result[Int, ConfigError] {
    let text = fs.read_to_string("port.txt")?
    text.parse_int().map_err(ConfigError.InvalidPort)
}
```

### 异步

```ql
pub async fn fetch_user(id: UserId) -> Result[User, HttpError] {
    let res = await client.get(f"/users/{id}")
    Ok(await res.json[User]())
}
```

### 并发

```ql
let task = spawn fetch_user(id)
let user = await task?
```

当前语义草案中，异步任务句柄也可以显式出现在类型位置：

```ql
fn schedule(id: UserId) -> Task[Result[User, HttpError]] {
    return fetch_user(id)
}

async fn main(id: UserId) -> Result[User, HttpError] {
    return await schedule(id)
}
```

### `unsafe`

```ql
unsafe fn from_raw(ptr: *const U8, len: USize) -> Bytes

unsafe {
    let bytes = from_raw(ptr, len)
}
```

### `extern`

```ql
extern "c" {
    fn strlen(ptr: *const U8) -> USize
}

extern "c" pub fn q_add(left: I32, right: I32) -> I32
```

## 13. 属性与测试入口

为了避免重型注解系统，P0 只建议有限 built-in attributes：

```ql
@test
fn parse_port_works() {
    assert_eq(parse_port("8080"), Ok(8080))
}

@deprecated("use parse_port")
fn old_parse_port(text: String) -> Int
```

原则：

- attribute 是编译器内建能力
- 不作为开放元编程系统
- 不参与任意代码展开

## 14. 完整构造清单

这一版语法系统已经明确列出以下构造：

- 包与导入
- 可见性
- 常量、静态值、类型别名、不透明类型
- `struct`、`data struct`、`enum`、`trait`
- 泛型参数与 `where`
- 类型实例化和值实例化
- 关联函数、实例方法、扩展方法
- 字面量、字段访问、调用、索引、闭包、块表达式
- `if`、`match`、`for`、`for await`、`while`、`loop`
- 元组解构与基于元组的多返回值
- 模式匹配、守卫、smart cast
- `async`、`await`、`spawn`
- `Result` + `?`
- `unsafe` 和 `extern`
- 有限 built-in attributes

这意味着 Qlang 已经有一套可以支持 parser 设计、语义建模和后续 RFC 继续细化的完整表层骨架。

## 15. 暂不进入 P0 的语法能力

- `:=` 声明语法糖
- 自定义操作符
- 重型宏系统
- 开放注解元编程
- 隐式类型转换链
- wildcard import
- tuple struct
- 模板字符串标签函数
- 默认结构类型

这些都不是“永远不要”，而是“不应先于主干能力落地”。
