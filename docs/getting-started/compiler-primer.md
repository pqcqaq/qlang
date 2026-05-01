# 编译器、术语与生态入门

## 这篇文档写给谁

这篇文档面向第一次系统接触“编程语言 / 编译器 / 工具链”的读者。

目标有两个：

- 提供一张稳定的术语与分层地图
- 把通用编译器概念对到 Qlang 当前仓库结构

## 先记住一张总地图

一个现代编译型语言项目通常包含以下层次：

```text
源码
  -> 词法分析（Lexer）
  -> 语法分析（Parser）
  -> AST
  -> 名称解析（Resolve）
  -> 类型检查（Type Check）
  -> 更适合分析/生成代码的中间表示（HIR / MIR / IR）
  -> 所有权 / 控制流 / 优化等分析
  -> 代码生成（Codegen）
  -> 目标产物（.ll / .o / .obj / .a / .lib / .so / .dll / 可执行文件）

同时还会有：
  - 诊断系统（错误信息、span、fix-it）
  - 格式化器（Formatter）
  - LSP / 编辑器支持
  - 运行时（Runtime）
  - 标准库（Stdlib）
  - 构建 / 测试 / 文档 / FFI 工具
```

## 1. 编译器到底在做什么

编译器的核心任务可以先分成四件事：

1. 把人写的源码变成机器和工具都能稳定理解的结构。
2. 检查代码有没有语法和语义上的问题。
3. 生成目标产物，比如可执行文件、静态库、动态库，或者更低层的中间代码。
4. 给开发者提供可靠的错误信息、跳转、补全、重构等工具能力。

现代编译器项目通常还承担：

- 语言规则的落地实现
- IDE 语义服务的真相源
- 诊断与修复建议
- 构建与互操作边界
- 测试和回归验证

## 2. 一条完整编译流水线，按层理解

### 2.1 源码、词法、Token

源码首先是一段文本。编译器不会一开始就理解“函数”“类型”“作用域”这种高层概念，它通常先做词法分析，把文本切成更容易处理的最小单元，也就是 `Token`。

例如这行代码：

```ql
let answer = add(40, 2)
```

可能会被切成：

- `let`
- `answer`
- `=`
- `add`
- `(`
- `40`
- `,`
- `2`
- `)`

这一步主要解决“字符长什么样”而不是“这段代码是什么意思”。所以 lexer 关心的是：

- 标识符
- 关键字
- 数字 / 字符串字面量
- 运算符和分隔符
- 注释和空白
- 每个 token 的源码位置

### 2.2 语法分析和 AST

有了 token 之后，parser 会按照语言语法把它们组织成树状结构，通常叫 `AST`，也就是抽象语法树。

你可以把 AST 理解成“更接近源码原貌的结构化表示”。它的重点不是做语义判断，而是回答：

- 这里是函数声明还是变量声明
- 这里是 `if` 表达式还是 `match`
- 这里谁包着谁
- 哪些部分属于参数列表、返回类型、函数体

AST 常常是 formatter、parser diagnostics 和后续 lowering 的基础。

### 2.3 名称解析、作用域和符号

只有 AST 还不够。编译器还要知道：

- 变量 `x` 指的是哪个 `x`
- 这里的 `User` 是类型名、模块名，还是别的东西
- `self` 在当前上下文是否合法
- 某个名字是局部绑定、参数、导入项，还是顶层定义

这一步通常叫 `名称解析` 或 `name resolution`。它依赖 `作用域（scope）` 规则，把“源码里长得一样的名字”对应到真实的定义点。

没有这一步，后面的类型检查、跳转定义、查找引用都很难做稳。

### 2.4 类型检查和类型推断

类型检查的目标不是“让代码看起来更学术”，而是提前发现一大类逻辑错误，例如：

- 把字符串传给需要整数的位置
- 条件表达式不是布尔值
- 调用了一个不可调用的值
- 返回值类型和函数签名不一致

很多现代语言还会做一定程度的类型推断，也就是少写一些显式类型，让编译器根据上下文补出来。但推断不等于不要规则，它只是把重复劳动交给编译器。

### 2.5 HIR、MIR、IR：为什么还要中间表示

初学者最容易困惑的一件事是：为什么有了 AST 还不够，为什么还要 HIR、MIR、IR？

原因很简单：不同阶段需要的数据形态不一样。

- `AST` 更接近源码，适合保留表面结构。
- `HIR` 通常表示“更适合做语义分析的高层中间表示”，会把一些语法糖正规化。
- `MIR` 通常表示“更适合做控制流、所有权、数据流分析的中层表示”。
- `IR` 则是更靠近后端或目标平台的中间表示，例如 LLVM IR。

一个稳定的编译器项目，通常不会拿 AST 直接做完所有事情，而是逐层把问题转成更适合该层解决的形式。

### 2.6 控制流图、所有权分析、优化

当代码进入更适合分析的中间表示之后，编译器会做更多“不是直接写在源码里，但对正确性和性能很重要”的工作，比如：

- 控制流图（CFG）分析
- 所有权 / 借用 / 移动后使用检查
- 生命周期相关约束
- 死代码消除
- 常量折叠
- 内联、逃逸分析、单态化等

不是每个语言都会做同样的分析，但思路相同：先把代码变成容易分析的形式，再做更可靠的推理。

### 2.7 诊断系统

现代编译器的诊断系统不是“失败时打印一行错误”。好的诊断通常至少要有：

- 错误种类
- 稳定的位置范围（span）
- 主问题和次要提示
- 必要时的修复建议

一旦诊断系统做得太晚或太弱，LSP、测试快照、CLI 输出都会变差。

### 2.8 代码生成、目标文件和链接

通过前面的语义阶段后，编译器才会进入代码生成。

典型路径是：

```text
语言中间表示
  -> 目标后端 IR（例如 LLVM IR）
  -> 对象文件（.o / .obj）
  -> 链接器
  -> 可执行文件或库
```

这里会出现几个很常见的产物：

- `.ll`：LLVM IR 文本
- `.o` / `.obj`：对象文件
- `.a` / `.lib`：静态库
- `.so` / `.dll` / `.dylib`：动态库
- 可执行文件：最终程序

### 2.9 运行时、标准库和 FFI

很多人第一次学编译器时只盯着 parser 和 codegen，但真正能落地的语言项目还必须处理：

- 标准库怎么组织
- 运行时要不要存在、负责什么
- 如何和 C / C++ / Rust 等宿主世界互操作
- 构建系统如何交付库、头文件、链接参数

这就是为什么“编译器项目”往往会自然扩展成“语言工具链项目”。

## 3. 常见专有名词速查

### 3.1 前端和语言表面

| 名词 | 简单解释 | 你可以怎么理解 |
| --- | --- | --- |
| 词法分析 / Lexer | 把源码文本切成 token | 先按“字形”切块 |
| Token | 词法单元 | 关键字、标识符、数字、符号等最小块 |
| 语法分析 / Parser | 把 token 组织成语法结构 | 按规则搭树 |
| 语法 / Syntax | 代码怎么写才算合法 | 表面形状规则 |
| 语义 / Semantics | 代码到底是什么意思 | 运行和类型上的真实规则 |
| Grammar | 语法规则集合 | parser 参考的“句法说明书” |
| AST | 抽象语法树 | 接近源码原貌的结构化树 |
| 优先级 / Precedence | 运算谁先结合 | 例如乘法先于加法 |
| 结合性 / Associativity | 同优先级运算如何分组 | 例如左结合、右结合 |
| 语法糖 / Syntactic Sugar | 写法更方便的表面能力 | 底层通常会被展开成更基本形式 |

### 3.2 语义和中间表示

| 名词 | 简单解释 | 你可以怎么理解 |
| --- | --- | --- |
| Scope | 作用域 | 一个名字在哪些地方可见 |
| Symbol | 符号 | 一个可被引用的定义实体 |
| Name Resolution | 名称解析 | 把名字绑到真正定义上 |
| Type Checking | 类型检查 | 检查类型是否匹配 |
| Type Inference | 类型推断 | 让编译器补出部分类型 |
| HIR | 高层中间表示 | 比 AST 更适合语义分析 |
| MIR | 中层中间表示 | 比 HIR 更适合控制流和所有权分析 |
| IR | 中间表示 | 介于源码和目标代码之间的表示 |
| Lowering | 降低 / 下沉表示层级 | 把一种表示转成更适合下一层的表示 |
| Desugaring | 语法糖展开 | 把“好写的表面语法”还原成基础结构 |
| Pass | 编译阶段中的一次处理 | 例如一个 analyze pass 或 optimize pass |
| CFG | 控制流图 | 程序可能如何跳转的图 |
| SSA | 静态单赋值形式 | 一种常见 IR 组织方式，便于优化 |
| Monomorphization | 单态化 | 把泛型为具体类型生成专门代码 |
| Ownership | 所有权 | 谁负责一个值的生命周期 |
| Borrow Checking | 借用检查 | 检查别名、可变性、生命周期是否合法 |

### 3.3 后端、产物和互操作

| 名词 | 简单解释 | 你可以怎么理解 |
| --- | --- | --- |
| Codegen | 代码生成 | 把中间表示翻译到目标后端 |
| Backend | 后端 | 更靠近目标平台的一层 |
| Object File | 对象文件 | 还没链接成最终程序的机器码片段 |
| Linker | 链接器 | 把多个对象文件和库拼成最终产物 |
| Static Library | 静态库 | 构建时被打进目标程序 |
| Dynamic Library | 动态库 | 运行时加载的库 |
| ABI | 应用二进制接口 | 二进制层面的调用约定和布局规则 |
| FFI | 外部函数接口 | 不同语言之间的调用边界 |
| Runtime | 运行时 | 程序运行时需要的一层支持 |
| Stdlib | 标准库 | 语言官方提供的基础库 |
| AOT | 预先编译 | 先编译成目标产物再运行 |
| JIT | 即时编译 | 运行时边执行边编译 |

### 3.4 工程、工具和生态

| 名词 | 简单解释 | 你可以怎么理解 |
| --- | --- | --- |
| Diagnostics | 诊断系统 | 错误、警告、note、help 的统一输出 |
| Span | 源码位置范围 | 错误和跳转锚点的基础 |
| Formatter | 格式化器 | 把代码排成统一风格 |
| LSP | 语言服务器协议 | 编辑器和语言服务之间的协议 |
| Incremental Compilation | 增量编译 | 只重新编译受影响的部分 |
| Fixture | 测试夹具 | 测试输入样例 |
| Snapshot Test | 快照测试 | 锁定某个输出结果防回归 |
| Workspace | 工作区 | 多包 / 多 crate / 多模块工程的统一组织 |
| Package Manager | 包管理器 | 管依赖、版本、发布和锁文件 |
| RFC | 设计提案机制 | 重大变化先写提案再落地 |

## 4. 初学者最常见的误区

- `Parser` 只是前端早期的一层，不等于完整编译器。
- `AST` 更接近源码形状，稳定语义信息通常要在 resolve、typeck、HIR、MIR 之后才能得到。
- `LLVM` 是重要后端基础设施，但不决定语言语法、类型系统、所有权规则、LSP、诊断边界和工具链体验。
- 诊断系统不等于“能报错”；还需要稳定 span、主次标签、友好文案、可测试输出，以及 CLI/LSP 复用。
- 标准库与运行时不是一回事；前者偏 API，后者偏运行支撑。
- 高质量 LSP 依赖稳定分析结果、查询边界和统一语义真相源。

## 5. 一门编程语言项目的完整生态

如果你把“编译器项目”理解成“一个把源码变成机器码的程序”，很容易低估一门语言真正需要的工程量。一个比较完整的语言生态，通常至少包括下面这些部分：

| 生态部件 | 作用 | 为什么重要 |
| --- | --- | --- |
| 编译器前端 | lexer、parser、AST、diagnostics | 决定语言能不能被稳定读懂 |
| 语义层 | resolve、typeck、query system | 决定规则能否解释、工具能否共用 |
| 中层表示 | HIR、MIR、IR | 决定分析和后端能否长期维护 |
| 后端 | codegen、链接、产物输出 | 决定语言能否真的交付程序或库 |
| CLI | `build`、`check`、`fmt`、`test` | 决定日常开发是否顺手 |
| Formatter | 统一代码风格 | 降低风格分裂和 review 成本 |
| LSP | hover、definition、references、completion | 决定编辑器体验是否现代 |
| Runtime / Stdlib | 基础 API、资源模型、并发模型 | 决定语言是否能写真实工程 |
| FFI / ABI 工具 | 头文件、桥接、链接辅助 | 决定能否接入现实世界 |
| 测试体系 | unit、fixture、snapshot、integration、ffi | 决定项目能否长期稳定演进 |
| 文档与 RFC | 愿景、规范、设计提案、路线图 | 决定决策是否能沉淀和复盘 |
| 示例与基准 | example、benchmark、template | 决定用户是否能学、团队是否能调优 |

## 6. 编译器领域常见技术路线

### 6.1 解释器、编译器、转译器

- `解释器`：直接执行或逐步求值源码/字节码。
- `编译器`：把源码翻译成另一种更低层、更接近机器的产物。
- `转译器`：把一种高级语言翻译成另一种高级语言，例如 TypeScript 到 JavaScript。

这些路线可以并存；一个语言项目可以同时拥有解释执行、AOT 编译和 LSP。

### 6.2 手写前端 vs 生成器

- 手写 lexer / parser：控制力强，适合需要精确诊断和长期演化的语言项目。
- 生成器：上手快，但在错误恢复、诊断和增量控制上有时没那么灵活。

选择取决于项目目标。

### 6.3 自己写后端 vs 站在 LLVM 上

- 自写后端：自由度高，但成本极大。
- LLVM 后端：能快速获得成熟的 IR、优化和目标平台支持。

很多新语言都会先站在 LLVM 上，把精力放在语言规则、语义分析和工具链边界上。

### 6.4 GC、手工内存、所有权推断

不同语言项目在内存模型上路线差异很大：

- GC 路线：开发体验通常更轻，但运行时成本和 FFI 成本可能更高。
- 手工内存路线：更贴近底层，但心智负担大。
- 所有权 / 借用 / 推断路线：试图在安全和性能之间取得平衡。

这类选择会反向影响语法、类型系统、运行时、诊断和工具链。

## 7. 把这些概念映射到 Qlang 仓库

Qlang 当前的实现，正好可以当作一张“编译器地图对应实物”的例子。

| 概念 | Qlang 中的目录 / crate | 当前职责 |
| --- | --- | --- |
| 源码位置与 span | `crates/ql-span` | 统一位置范围和行列换算基础 |
| Lexer | `crates/ql-lexer` | 把源码切成 token |
| AST | `crates/ql-ast` | 源码导向语法树定义 |
| Parser | `crates/ql-parser` | 递归下降解析和 parser diagnostics |
| Formatter | `crates/ql-fmt` | 基于 AST 的格式化 |
| 统一诊断模型 | `crates/ql-diagnostics` | parser / semantic / backend 共用诊断结构和渲染 |
| HIR | `crates/ql-hir` | AST 到更适合语义分析的高层中间表示 |
| 名称解析 | `crates/ql-resolve` | 作用域图和 resolution map |
| 类型检查 | `crates/ql-typeck` | first-pass typing 和语义诊断 |
| 统一分析入口 | `crates/ql-analysis` | 把 parse / HIR / resolve / typeck / query 串起来 |
| MIR | `crates/ql-mir` | 控制流、cleanup、closure facts 更稳定的中层表示 |
| Ownership 分析 | `crates/ql-borrowck` | 当前的 moved-state、cleanup、closure capture 事实分析 |
| 运行时 / async hook 合同 | `crates/ql-runtime` | task/executor 抽象与 runtime hook ABI 合同 |
| 项目/workspace 与 `.qi` 接口 | `crates/ql-project` | manifest graph、默认 `.qi` 路径/状态、interface load/render |
| LLVM 后端 | `crates/ql-codegen-llvm` | 受控子集的 LLVM IR 生成 |
| Build / FFI 编排 | `crates/ql-driver` | `build`、header emit、工具链调用、产物落盘 |
| CLI | `crates/ql-cli` | `ql check`、`ql build`、`ql run`、`ql test`、`ql project`（含 `init/graph/emit-interface`）、`ql ffi`、`ql fmt`、`ql mir`、`ql ownership`、`ql runtime` |
| LSP | `crates/ql-lsp` | hover、definition、references、completion、rename、`workspace/symbol` 等语言服务端能力 |
| 黑盒诊断测试 | `tests/ui` | 锁定最终 CLI 诊断输出 |
| 黑盒 codegen / FFI 测试 | `tests/codegen`、`tests/ffi` | 锁定产物和真实 C / Rust 宿主互操作行为 |
| executable smoke 语料 | `ramdon_tests/` | committed sync / async 可执行 smoke corpus |
| 设计与路线图 | `docs/` | 愿景、语言设计、架构、路线图、阶段进展 |

## 8. Qlang 当前阶段，放到总地图里看

截至当前文档站同步状态，Qlang 已经完成或建立了这些地基：

- P1：前端最小闭环
- P2：HIR、名称解析、first-pass typing、统一诊断、最小查询/LSP
- P3：结构化 MIR、ownership facts、cleanup-aware 分析、closure groundwork
- P4：LLVM 后端地基、`ql build`、对象文件 / 可执行文件 / 静态库 / 动态库路径
- P5：最小可用 C FFI 闭环，包括头文件生成和部分 shared/static library 配套能力
- P6：same-file query / rename / completion / semantic tokens / LSP parity
- P7：继续保守扩 async/runtime/staticlib/Rust interop 主线
- P8：package/workspace manifest、`.qi` interface artifact、dependency-backed cross-file tooling 已进入真实实现；project-aware `build/run/test` 也已能走 direct dependency public `extern "c"` 调用这条窄执行路径

### 8.1 现在如何用 CLI 起一个最小工作区

当前 `ql-cli` 已经可以直接生成最小 package / workspace 骨架，不再要求先手写 `qlang.toml`。

初始化 package：

```bash
ql project init demo-package
```

初始化一个显式依赖仓库内最小 `stdlib` 的 package：

```bash
ql project init demo-package --stdlib path/to/language_q/stdlib
```

这会在新 package 的 `[dependencies]` 中写入 quoted-key 形式的 `std.core` / `std.option` / `std.test` 本地依赖，并生成直接消费 `std.core` / `std.option` / `std.test` 的 `src/lib.ql`、`src/main.ql` 与 `tests/smoke.ql`；workspace 初始化也支持同一个 `--stdlib <path>` 选项。

初始化 workspace：

```bash
ql project init demo-workspace --workspace --name app
```

这会生成：

```text
demo-workspace/
  qlang.toml
  packages/
    app/
      qlang.toml
      src/lib.ql
      src/main.ql
      tests/smoke.ql
```

单 package 初始化同样会生成 `src/lib.ql`、`src/main.ql` 与 `tests/smoke.ql`。

生成后可以立刻执行：

```bash
ql project graph demo-workspace
ql check demo-workspace
```

如果本机有 clang-style toolchain，还可以继续执行：

```bash
ql build demo-workspace
ql run demo-workspace
ql test demo-workspace
```

但这还不是“完整跨包语义可用”。当前工作区里真正稳定可依赖的跨包执行，只覆盖 direct dependency 的 bridgeable public `const/static` values、受限 public top-level free function、public `extern "c"` 符号，以及这些 bridgeable public `struct` 上的受限 public receiver method forwarder（含不可变局部 alias 的 method value direct call）；更宽的跨包 lowering 仍在后续阶段。

继续阅读建议：

1. [项目愿景](/vision)
2. [编译器流水线](/architecture/compiler-pipeline)
3. [实现算法与分层边界](/architecture/implementation-algorithms)
4. [工具链设计](/architecture/toolchain)
5. [P1-P8 阶段总览](/roadmap/phase-progress)
6. [开发计划](/roadmap/development-plan)

## 9. 初学者建议怎么学这件事

如果你是从零开始，建议按下面顺序学。

### 第一步：先搞清“层”

先把这些层背熟：

- lexer
- parser
- AST
- resolve
- typeck
- MIR / IR
- codegen
- linker
- runtime / stdlib / FFI / LSP

### 第二步：把“源码到产物”的路径想通

至少要能讲清：

- 源码怎么变成 token
- token 怎么变成 AST
- AST 怎么进入语义层
- 语义层怎么把名字和类型定下来
- 中间表示为什么需要分层
- 最终怎么变成对象文件、库和可执行文件

### 第三步：开始看一个真实仓库

Qlang 很适合做这一步，因为它的 crate 边界比较清楚。建议顺序：

1. 先看 [编译器流水线](/architecture/compiler-pipeline)
2. 再看 [实现算法与分层边界](/architecture/implementation-algorithms)
3. 然后对照 `crates/` 目录逐个理解
4. 最后再去看某个具体 phase 的设计和实现

### 第四步：把编译器当成系统工程

后面会越来越频繁地遇到这些问题：

- 诊断怎么设计
- 测试怎么防回归
- LSP 怎么复用语义真相源
- ABI / FFI 怎么做稳
- 文档和 RFC 怎么沉淀

这些内容也属于编译器工程的一部分。
