# 工具链设计

## 目标

工具链围绕统一 CLI、共享 analysis 边界和 project-aware 工作流组织。实现分层见 [实现算法与分层边界](/architecture/implementation-algorithms)。

## 命令入口

统一入口是 `ql`。当前已实现：

- `ql check`
- `ql fmt`
- `ql mir`
- `ql ownership`
- `ql runtime`
- `ql build`
- `ql run`
- `ql test`
- `ql project`
- `ql ffi`

`ql runtime` 输出 runtime 要求和 hook ABI 检查信息；`ql run` 按 executable build 规则构建并执行程序。

## 关键子工具

### `ql build`

职责：

- 读取单个 `.ql` 文件，或从 package/workspace 入口解析 build targets
- 复用 analysis 完成 parse / HIR / resolve / typeck / MIR
- 在无语义错误时调用 LLVM codegen

常用形态：

```powershell
ql build demo.ql
ql build demo --emit llvm-ir
ql build demo --profile release
ql build workspace --package app --target src/main.ql
ql build workspace --json
```

输出以 `target/ql/<profile>/...` 为默认根。project-aware build 会按 package/workspace 语义解析本地依赖，并保留当前支持的 dependency bridge 切片。

### `ql run`

- 复用 `ql build`
- 构建后执行目标程序
- 支持 project-aware 入口、`--list`、`--json`

### `ql test`

- 支持单文件、package 和 workspace 入口
- 递归执行 `tests/**/*.ql`
- 支持 project-aware smoke / UI test 语义和 `--target`

### `ql project`

已落地的子命令以实际仓库行为为准，核心能力包括：

- `init`
- `add` / `remove`
- `add-dependency` / `remove-dependency`
- `status`
- `dependencies` / `dependents`
- `targets`
- `graph`
- `lock`
- `emit-interface`

## 包与工作区

当前 manifest 仍是最小可用子集：

- `[package].name`
- `[workspace].members`
- `[dependencies]`
- `[references].packages`
- `[lib].path`
- `[[bin]].path`
- `[profile].default`

项目模型要尽早覆盖工作区成员、目标、接口产物和本地依赖，因为编译器、标准库和 LSP 都依赖同一份图。

## 标准库

仓库内 `stdlib` 现在是普通 Qlang workspace，不是内置 prelude。

已使用的主路径是：

- `std.core`
- `std.option`
- `std.result`
- `std.array`
- `std.test`

推荐原则：

- generic carrier 和 canonical array API 优先
- concrete wrappers 仅保留为兼容面
- 生成项目模板必须能直接跑 `ql check/build/run/test`

## 接口产物

Qlang 需要为每个包输出公共接口产物 `.qi`，用于：

- 供下游类型检查
- 供 LSP 导航和补全
- 避免每次都重新解析全部依赖源码

这相当于把 TypeScript 的 declaration emit / project references 经验改造成编译型语言的工程能力。

## 编辑器体验

LSP 的目标是从第一阶段就服务真实开发，而不是只做“能连上”：

- 补全必须基于真实类型
- 报错位置必须稳定
- rename 必须能处理当前支持的 workspace 切片
- code action 必须能服务导入、`match`、stub 和常见修复

当前已支持的 workspace-aware 子集以 [当前支持基线](/roadmap/current-supported-surface) 为准。

## 发布与生态

当前已经有 `qlang.lock` 和机器可消费的 JSON 输出，但生态层仍是后置项。

后续还要继续补：

- registry / version solving
- 更完整的 reproducible build 输入面
- binary caching
- doc hosting
- template generator

## 当前已验证命令

```powershell
cargo test
cargo run -p ql-cli -- check fixtures/codegen/pass/minimal_build.ql
cargo run -p ql-cli -- run fixtures/codegen/pass/minimal_build.ql
cargo run -q -p ql-cli -- check --sync-interfaces stdlib
cargo run -q -p ql-cli -- test stdlib
```

这份文档只描述当前工具链合同，不维护完整能力流水账。
