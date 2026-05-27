# 开发计划

> 最后同步：2026-05-28

只记录当前开发顺序和可执行约束。不写日期承诺，不写流水账。

## 当前判断

- Qlang 已有编译器、CLI、项目系统、LSP 和 stdlib 地基。
- 当前瓶颈是生产可用性，不是重做语言骨架。
- stdlib、workspace、LSP、测试和分发必须一起推进。
- 每轮交付一个可验证切片：代码、回归、文档、提交。

## 推进顺序

| 顺序 | 主题 | 目标 |
| --- | --- | --- |
| 1 | 项目可用性 | `ql project/check/build/run/test` 在真实本地 workspace 中稳定工作 |
| 2 | stdlib | 收口 `std.core`、`std.option`、`std.result`、`std.array`、`std.test` 的 public API，并扩大 downstream smoke |
| 3 | generics/backend | 补齐 generic monomorphization 和 dependency-aware backend |
| 4 | LSP/VSCode | 补齐真实 workspace 下的导航、高亮、补全、格式化、code action |
| 5 | 分发准备 | release、VSIX、CI、JSON 输出、安装文档 |
| 6 | 语言扩面 | 更宽 async/runtime、trait/effect、workspace-wide refactor |

## 当前工作项

- `ql-cli` 主入口已收口到命令分发，check/build/run/test 的主要 pipeline、reporting、target selection/discovery、execution 和 project build execution 已模块化；dependency bridge 顶层模块、dependency generic bridge/rendering/instantiations、通用 CLI utility、CLI scan/path helper、project manifest path/reporting、project graph/dependencies/status/interface reporting、single-source input、check command/reporting、run pipeline/target/reporting/execution/project path、test command/discovery/execution、project init/templates/member edits/lock/interface/reference interface emit reporting/targets/target build prep、project emit-interface preflight/package/workspace emit/check handling、build command/failure/json/output/plan/project execution/single-source reporting/build reporting 测试已从生产模块外置，build reporting build-result rendering、failure envelope/detail builder、workspace target loading/selection、project target selector、project targets selection/rendering/path scope/loader 已从主文件拆出，`ql-cli/src` 生产文件不再保留内联 `mod tests`。下一步继续拆 CLI 大边界，所有移动必须保持行为不变并跑真实 workspace smoke。
- 关键输出写入已统一走同目录临时文件替换和输出路径锁，覆盖 artifact、header、source、manifest、lockfile、interface 以及 `run/test` 执行期 executable。后续并发问题继续按“先保护真实产物，再补 CLI smoke”的顺序处理。
- `project init --stdlib` starter 是 downstream 可用性入口；package/workspace 的 `check/build/run/test`、interface sync/check、graph/status/targets/dependencies/dependents、lock stale failure 和 JSON/listing 合同都必须持续覆盖。
- `ql test` 已覆盖 package-under-test、direct dependency 和 local generic source override 的组合路径；后续重点是继续与 build/run 的 selector、profile、JSON failure 合同保持一致。
- stdlib public API 继续优先用泛型、数组长度泛型和语言能力表达；新增 API 必须进入 package-local smoke 或 starter/downstream smoke。
- LSP 已共享 request context 并拆出主要 request family；下一步是稳定 workspace index/cache 生命周期，减少重复扫描。
- README、roadmap、stdlib、VSCode 文档必须跟实现同步；实现未落地时文档写成未支持。
- JSON 输出继续覆盖成功和 preflight/render/selection failure；新增命令路径必须同时考虑文本和 JSON 合同。

## 明确后置

- 自动 prelude
- registry、publish、version solving
- 完整 generic monomorphization
- 完整 workspace-wide rename/refactor/index
- 更宽 async/runtime/Rust interop
- 可变参数语法

## 代码整理规则

- 不新增 `foo3/foo4/foo5` 这类固定 arity API；先补泛型、数组初始化或可变参数等语言能力。
- compiler regression fixture 也遵守同一规则；用 length-generic wrapper 加具体 caller 触发实例化，不用固定 arity 假 API。
- 不在 LSP 里复制语义规则；缺能力先补 `ql-analysis` query。
- 不把测试 fixture 写进生产路径。
- 大文件拆分必须行为不变，并用现有回归证明。
- 文档发现实现缺口时，优先修实现；实现未落地时，文档必须写成未支持。
