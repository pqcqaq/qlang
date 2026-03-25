# 工具链设计

## 目标

Qlang 不是“只有一个编译器二进制”的项目，而是一整套开发体验工程。工具链应围绕统一 CLI 和共享语义数据库展开。

## 统一入口

建议统一入口命令为 `ql`，由它驱动各子工具：

- `ql new`
- `ql init`
- `ql build`
- `ql run`
- `ql test`
- `ql check`
- `ql fmt`
- `ql doc`
- `ql bench`
- `ql clean`
- `ql ffi`

这样可以减少工具数量膨胀，让使用者把它当成一个系统，而不是一堆松散命令。

## 子工具

### `qlsp`

LSP 服务端，复用编译器 HIR 与查询系统，支持：

- go to definition
- find references
- hover
- completion
- semantic tokens
- rename
- code action
- diagnostics

### `qfmt`

格式化器必须尽早做，并尽量做到：

- 输出稳定
- 风格单一
- 对 AST 变化敏感度低

现代语言生态一旦放任格式风格分裂，后面会一直付成本。

当前阶段 `qfmt` 已覆盖的语法切片包括：

- 基础声明：`const`、`static`、`type`、`opaque type`
- 可调用声明：`fn`、`trait` method、`impl`、`extend`、`extern`
- 类型表达式：named type、tuple、callable type、声明泛型、`where`
- 表达式：调用、成员访问、结构体字面量、闭包、`unsafe`、`if`、`match`
- 控制流：`while`、`loop`、`for`、`for await`
- 模式：tuple、path、tuple-struct、struct、字面量、`_`

Phase 1 结束后，`qfmt` 的下一步重点不是增加风格选项，而是跟随后续 HIR / diagnostics 演进，保持语法扩展时的稳定输出与可维护实现。

### `qdoc`

文档生成器负责：

- 从公共 API 提取签名
- 展示效果、错误、trait 约束和 FFI 标记
- 输出静态站点内容

### 测试工具

`ql test` 不只是运行单元测试，还应逐步支持：

- UI tests
- doc tests
- integration tests
- benchmark harness

## 包与工作区

Qlang 应提供统一 manifest，例如 `qlang.toml`，支持：

- package metadata
- dependencies
- features
- build profiles
- ffi libraries
- workspace members

工作区模型必须在早期就纳入，因为编译器、标准库、示例、FFI 包和工具链本身都会依赖它。

借鉴 TypeScript 的 project references，Qlang 还应支持显式项目引用图：

- 工作区成员能声明上游接口依赖
- 增量构建优先基于接口产物判断失效范围
- LSP 可直接消费依赖包的公共 API 元数据

## 接口产物

Qlang 建议为每个包输出公共接口产物，例如 `.qi` 文件：

- 包含公共类型、函数签名、trait、effect、布局约束等元数据
- 供下游类型检查和 LSP 使用
- 避免每次都重新解析全部依赖源码

这相当于把 TypeScript 的 declaration emit 和 project references 经验，转化为适合编译型系统语言的工程能力。

## 编辑器体验

LSP 的目标不是“有就行”，而是从第一阶段就支撑日常开发：

- 补全要基于真实类型，不是纯文本猜测
- 报错位置要稳定
- rename 要有跨文件可信度
- code action 要能生成 `match` 分支、导入、trait stub

Qlang 的目标不是“有一个能用的 LSP”，而是像 TypeScript 一样，把语言服务当成语言本体的一部分来设计。

## 发布与生态

P1 之后可以逐步加入：

- package registry
- lockfile
- binary caching
- doc hosting
- template generator

但这些必须建立在前面的语义和构建基础上，而不是为了“看起来像成熟生态”提前堆功能。
