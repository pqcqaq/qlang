# 跨语言借鉴

## 目标

Qlang 不应该只是“像 Rust 的另一门语言”。它需要主动吸收 Kotlin、Go、TypeScript 已经证明有效的工程经验，但只拿真正能改善系统语言体验的部分，不照搬它们各自的历史包袱。

这里的核心判断是：

- 借 Kotlin 的开发者友好性
- 借 Go 的工具链纪律和工程直白性
- 借 TypeScript 的流式类型分析和大型项目组织能力

## 从 Kotlin 借什么

### 1. Smart cast / 流敏感类型收窄

Kotlin 的 smart cast 证明了一件事：很多“显式写类型转换”的痛苦，本质上是编译器没有把控制流信息利用起来。Qlang 应直接吸收这一点：

- `if x != none` 后，`x` 在分支内自动视为非空
- `if value is Foo` 后，`value` 在分支内自动收窄为 `Foo`
- `match` 分支和提前 `return` 之后，收窄信息继续生效

这会显著减少样板代码，也是“把复杂留给编译器”的典型落点。

### 2. Data class 风格的数据建模

Kotlin 的 data class 之所以成功，不是因为语法短，而是因为它把“数据承载类型”的高频样板直接交给编译器生成。Qlang 适合引入：

- `data struct`
- 自动派生 `Eq`、`Hash`、`Debug`
- `copy(...)`
- 解构

这会让配置对象、消息对象、领域模型写起来明显更顺。

### 3. 扩展方法

Kotlin 的 extension 非常适合增强可读性，但也容易被滥用。Qlang 可以借鉴它，同时加约束：

- 支持模块级 `extend`
- 扩展方法必须显式导入
- 成员方法优先于扩展方法

这样既能让 API 更贴近领域语言，又不会演变成全局隐式魔法。

### 4. 命名参数和默认参数

这点 Kotlin 的工程收益非常直接，尤其适合配置型、服务端和工具链 API。Qlang 应继续保留这项设计，并让它成为公共 API 的推荐风格。

## 从 Go 借什么

### 1. 工具链单一真相源

Go 的最大优点之一不是语法，而是“官方路径极窄”。Qlang 应明确：

- 只有一个官方 formatter
- 只有一个官方 build/test/doc/workspace 路径
- 项目初始化、测试、格式化、文档生成都通过 `ql` 统一入口

这能直接降低生态碎片化。

### 2. 小接口哲学

Go 的接口通常很小，这使抽象更稳定。Qlang 的 trait 设计也应该吸收这一点：

- 小 trait 优先
- 能力 trait 优先于“上帝接口”
- 常见模式尽量围绕 `Reader`、`Writer`、`Hasher` 这种能力建模

### 3. `defer` 和资源清理直觉

Go 的 `defer` 对工程代码非常有效。Qlang 已经引入这个方向，应该继续强化，使其与析构模型、`unsafe` 资源包装和异步取消语义兼容。

### 4. 包与项目组织纪律

Go 的包命名、注释、测试和模块习惯值得借鉴。Qlang 应明确风格约束：

- 包名短、小写、语义明确
- 官方注释格式和文档提取规范统一
- `ql test`、`ql bench`、`ql doc` 是一等工作流，不是附属工具

### 5. 取消与上下文传播

Go 的 `context` 不是语法糖，而是工程约束。Qlang 虽然会走结构化并发路线，但仍应借鉴“取消原因、超时、向下游传递上下文”的思想，并把它标准化到 async 运行时里。

## 从 TypeScript 借什么

### 1. 控制流驱动的类型收窄

TypeScript 在 discriminated unions 和 narrowing 上非常成功。Qlang 虽然是编译型系统语言，也完全值得吸收：

- 基于 `match` 和 `if` 的分支收窄
- 标签联合 / 判别字段驱动的自动推导
- 更聪明的可空性传播

这和 Kotlin 的 smart cast 相结合，会成为 Qlang 类型体验的重要差异化。

### 2. `satisfies` 操作符

TypeScript 的 `satisfies` 很值得借鉴，因为它能“检查某个值满足某个形状”，同时保留表达式更具体的推断结果。Qlang 非常适合引入：

```ql
let config = {
    host: "127.0.0.1",
    port: 8080,
} satisfies ServerConfig;
```

这对于配置对象、编译期常量、构建脚本和 DSL 都很有价值。

### 3. 项目引用与接口声明产物

TypeScript 的 project references 说明，大型项目的可维护性不只来自语言语法，还来自编译图管理。Qlang 应借鉴这一点：

- 工作区成员间支持显式引用
- 编译器输出公共 API 元数据文件，例如 `.qi`
- 下游依赖优先读取接口产物，而不是重复解析全部源码

这会直接改善增量编译和 LSP 响应性能。

### 4. 编辑器体验前置

TypeScript 的成功很大程度上来自语言服务质量。Qlang 也必须保持同样的工程纪律：

- 编译器与 LSP 共用语义数据库
- 补全、跳转、rename、diagnostics 来自同一真相源
- 公共 API 元数据既服务编译，也服务 IDE

## Qlang 最终吸收的增强点

综合下来，我建议把 Qlang 的强化方向收敛为八条：

1. 引入 Kotlin + TypeScript 风格的流敏感类型收窄
2. 引入 `data struct` 与 `copy(...)` 等数据建模能力
3. 保留并强化命名参数、默认参数和扩展方法
4. 保持 Go 风格的单一官方工具链与统一格式化
5. 保持小 trait 哲学，避免抽象膨胀
6. 在 async 体系里引入明确的取消上下文模型
7. 引入 TypeScript 风格的 `satisfies` 操作符
8. 借鉴 project references，设计 `.qi` 公共接口产物

## 明确不借什么

- 不借 Kotlin 的重 DSL 倾向作为默认写法
- 不借 Go 的弱表达性错误模型，Qlang 仍坚持 `Result`
- 不借 TypeScript 的结构类型默认化和可选过度宽松性
- 不借任何会削弱 ABI 稳定性和内存模型清晰度的动态特性

Qlang 的方向不是“把三门语言混起来”，而是“用它们已经验证过的优点，增强一门系统级语言的工程体验”。
