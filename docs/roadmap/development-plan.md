# 开发计划

关联文档：

- [当前支持基线](/roadmap/current-supported-surface)
- [阶段总览](/roadmap/phase-progress)
- [工具链设计](/architecture/toolchain)
- [`/plans/`](/plans/)

这页只保留当前判断、优先级和最近的交付方向。

## 当前判断

- 编译器主链路已经够用，当前瓶颈不在“前端还没搭起来”，而在“真实项目还不够稳”。
- 主线继续围绕可真实使用性：package/workspace、dependency-aware build、stdlib、LSP 和分发。
- `stdlib` 是 P0 可用性的一部分，不是后置扩面项。
- concrete/fixed-arity helper 只保留为兼容面，新的设计优先 generic carrier 和集合式 API。
- 每轮都尽量交付生产代码、回归和文档的一个可验证切片，不再按日期承诺整条主线。

## 优先级

| 优先级 | 主题 | 目标 |
| --- | --- | --- |
| P0 | 可用性 MVP | 让 Qlang 能稳定支撑小型本地 workspace 项目 |
| P1 | stdlib / generics / dependency backend | 让 `stdlib`、generic carrier 和跨包执行真正可用 |
| P2 | 基础 LSP / VSCode | 让真实 workspace 里的导航、高亮、补全可依赖 |
| P3 | 分发 / CI / 团队接入 | 补齐安装、lock、JSON、CI 和 VSIX 分发 |
| P4 | 高级 IDE 与语言扩面 | cross-file rename、workspace edits、更完整 code actions，以及更宽 runtime/语言能力 |

## 现在做什么

- 继续把 generic `Option[T]` / `Result[T, E]`、canonical `std.array` helpers 和 downstream smoke 收紧到稳定可用；固定长度数组 helper 只保留薄兼容层。
- 继续补真实项目常见的 direct local dependency value/type/member 路径。
- 继续把 `qlsp` 对真实 workspace 的基础导航和编辑器能力补齐。
- 继续把文档压缩成“当前事实”，不再把历史推导写回入口页。

## 明确后置

- 自动 prelude
- registry / publish / version solving
- 更宽的 async/runtime/Rust interop
- 新语法糖和更远的类型系统扩面
- 可变参数语法

## 交付规则

- 入口文档只写结论和边界，不写长流水账。
- 任何用户可见能力都必须同时具备实现、回归和文档入口。
- 纯文档改动不算功能推进。
- 文档与实现冲突时，先修正文档。
