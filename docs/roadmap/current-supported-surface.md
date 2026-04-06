# 当前支持基线

> 最后同步：2026-04-07

这页只保留“今天真实可依赖的能力边界”。  
详细切片过程、逐轮回归记录与旧版长文已归档到 [路线图归档](/roadmap/archive/index)。

## 真相源

当前基线以这几类文件为准：

- 实现：`crates/*`
- executable 真运行矩阵：`crates/ql-cli/tests/executable_examples.rs`
- library build / codegen pass 矩阵：`crates/ql-cli/tests/codegen.rs`、`crates/ql-cli/tests/string_codegen.rs`、`crates/ql-cli/tests/string_compare_codegen.rs`
- project/workspace graph 矩阵：`crates/ql-cli/tests/project_graph.rs`
- sync 样例：`ramdon_tests/executable_examples/`
- async 样例：`ramdon_tests/async_program_surface_examples/`

如果文档和这些文件不一致，以代码与回归矩阵为准，再回头修正文档。

## 一页结论

- Phase 1 到 Phase 6 地基已经落地：lexer、parser、formatter、diagnostics、HIR、resolve、typeck、MIR、borrowck、LLVM backend、driver、CLI、same-file LSP/query、same-file document symbol outline、以打开 package 为根的保守 `workspace/symbol` 搜索、FFI header projection 都已进入真实工程主干。
- 当前活跃主线是保守推进的 Phase 7：async/runtime/task-handle lowering、library/program build surface、Rust interop。
- Phase 8 的前三条入口切片已继续向前推进：仓库现已具备最小 `qlang.toml` manifest graph loader、`ql project graph` 调试入口、`.qi` V1 emit 入口 `ql project emit-interface`、build-side `.qi` 写出开关 `ql build --emit-interface`，以及 package-aware `ql check` 对引用 `.qi` 的 syntax-aware 加载；当前入口已覆盖 package directory、`qlang.toml`、包内源码文件路径，以及 workspace-only 根 manifest，而 `ql project emit-interface` 现在也已支持从 workspace-only 根 manifest 批量写出成员包的默认 `.qi`。`ql check --sync-interfaces` 现也可在分析前递归同步写出本地引用包默认 `.qi`，并且入口同样覆盖 package directory、`qlang.toml`、包内源码文件路径，以及 workspace-only 根 manifest，减少手工预生成步骤。`ql-analysis::analyze_package` 现也已把 dependency `.qi` 的公开符号收进 package 级查询面，并接通 imported dependency symbol 的最小 cross-file hover / definition / declaration / references、`use ...` 导入路径和平铺 / grouped import 位置里的 dependency package path segment / public symbol completion、dependency enum variant / explicit struct field-label 的最小非导入路径 completion contract，以及 dependency struct member-field / member-method token 的最小 completion contract；这条 receiver truth surface 除了 dependency-typed param、带 dependency type annotation 的 `let`、dependency struct literal `let` 结果、dependency method-call result、block tail result、结果稳定收敛到同一个 dependency struct 的 `if` / `match` 表达式结果及其 direct alias 之外，也已覆盖不经命名 local 的 direct dependency method-call / structured receiver，例如 `config.child().value`、`config.child().get()`、`(if ...).value`、`(match ...).get()`。显式 type-namespace 位置上的最小 `textDocument/typeDefinition` 也已接通，当前覆盖 same-file type use、generic type use、same-file local import type alias use、成功 package analysis 路径下的 dependency import/type-root `.qi` target、语法局部可恢复的 dependency struct value/root、dependency struct field token 到其 dependency public field type 的受控跳转，以及 dependency method token 到其 dependency public return type 的受控跳转；broken-source / semantic-error 场景下 parse-only 可恢复的 dependency import/type-root / dependency value root / dependency struct field token / dependency method token `typeDefinition` fallback 现在也都已接通。broken-source / semantic-error 场景下的 import-root / variant / struct-field / dependency struct member-field token / dependency struct member-method token 最小 hover / definition / declaration / references fallback 也已落地。真实 dependency build graph 与更广义 cross-file LSP 仍未开放。
- 外部稳定互操作边界仍是 C ABI；Rust 继续走 `build.rs + staticlib + header` 路线。
- async 已经不是“只有语法”，而是有真实 build、真实样例和真实回归的受控子集；但 broader async ABI、broader runtime semantics 仍然刻意关闭。
- sync backend 的首个 `String` build 子集现已进入真实 build surface：UTF-8 string literal 现可 lowering 为 `{ ptr, i64 }`，并经过 local binding、const/static materialization、普通参数传递、返回值、`==` / `!=` 比较与 fixed-shape aggregate transport 进入当前 LLVM/object build；string ordering compare、string-pattern match lowering 与 C header `String` 导出仍保持保守关闭。
- cleanup lowering 已不再是“全量关闭”：首个 `defer` + cleanup branch/match + 透明 `?` wrapper lowering 子集已进入真实 build 回归，并已接通 callable-value cleanup callee 与 cleanup guard-call 子路径的最小间接调用；capturing closure 当前也已打通 local alias 未重绑定前提下的 cleanup callee / cleanup guard-call 最小子集，并进一步开放 same-target cleanup control-flow root 与 cleanup block 内的局部 alias call/guard-call 子集；其中 statement-sequenced cleanup block 内的局部 mutable alias 现也开放 same-target reassign 后继续 direct call / guard-call，而 assignment-valued same-target mutable alias 现也可直接作为 cleanup callee / guard-call root，且 `if` / `match` 分支里只要最终仍收敛到同一个 same-target assignment-valued capturing-closure root，也已能继续 build；cleanup block 内的 `let chosen = alias = run` / `let chosen = alias = check` 这类 assignment-valued same-target binding 已进入当前 build surface，而 `let chosen = if ... { alias = run } else { run }` / `let chosen = match ... { ... }` 这类 control-flow 收敛到 same-target assignment-valued root 的 binding 现在也已可用；当前进一步开放了分支内 block-local alias tail binding，即 `let chosen = if ... { let alias = run; alias } else { run }`、`let chosen = match ... { true => { var alias = check; alias = check; alias }, false => check }`，以及同类的 direct cleanup callee / cleanup guard-call root 现在也已能继续 build；另外，statement-sequenced cleanup block 内的局部 mutable alias 现在还额外开放了首个 different-target mutable reassign 子集，即 `var alias = left; alias = right; alias(...)` / `var alias = left_check; alias = right_check; if alias(...) { ... }` 这类无 branch-join 的直线语句序列；同一类 block-local different-target mutable alias 现在也已可直接作为 cleanup callee / cleanup guard-call root 使用，即 `defer ({ var alias = left; alias = right; alias })(...)` / `defer if ({ var alias = left_check; alias = right_check; alias })(...) { ... }`；同一条 local mutable alias 的 different-target reassign 现在也已沿 shared local path 扩到 cleanup direct root、cleanup local binding root，以及 runtime `if` / `match` branch-join 子集；当前又进一步开放了 shared-local control-flow assignment-valued binding alias-chain root：cleanup `if` 已支持 `let chosen = if ... { alias = right } else { left }; let rebound = chosen; rebound(...)` 与对应 guard-call，cleanup bool `match` 已支持等价的 `let chosen = match choose() { true => alias = right, false => left }; let rebound = chosen; ...`，而 cleanup guarded `match` 现在也已接通首个子集，例如 `let chosen = match choose() { true if guard() => alias = right, false => left, _ => left }; let rebound = chosen; ...`；different-closure cleanup control-flow 也已开放首个子集：runtime `if` / `match` 现在可在 direct root / block-local alias tail / block binding / local alias chain root 之间选择不同 capturing closure 继续进入 cleanup callee / cleanup guard-call；其他 broader cleanup escape flow 仍保持保守拒绝。
- 普通 `?` lowering 已接入当前 codegen 路径，并已流入当前 shipped cleanup 子集；当前 user-facing build blocker 不再包含 `return helper()?` 或 `defer helper()?` 这类透明 question-mark 表达式。
- sync closure value surface 已从“仅 non-capturing”推进到首个 capturing 子集：当前开放 non-`move`、捕获 immutable same-function scalar binding，并允许原局部 direct ordinary call、local alias ordinary call（含 same-target mutable reassign）、local alias cleanup callee、local alias cleanup guard-call、assignment-valued cleanup callee / guard-call root、cleanup block 内局部 alias 的 direct call / guard-call（现含 statement-sequenced local mutable alias same-target reassign、statement-sequenced cleanup block 内局部 mutable alias 的 different-target reassign，以及 control-flow 分支内的 block-local alias tail binding），以及 ordinary `match` guard-call（现含 direct callee root、先绑定到 ordinary local 后再调用的 control-flow-selected root、block-local assignment-valued / control-flow-assignment-valued binding root，以及 different-closure control-flow 下的 block-local alias tail / block binding / local alias chain callee root）；runtime `if` / `match` 在 ordinary direct/local-binding/cleanup paths 上现也开放 same-target control-flow 子集，而 ordinary direct call、ordinary local binding root 及其后续 local alias chain 调用（现含后续未重写块）、ordinary `match` guard-call，以及 cleanup callee / cleanup guard-call 的 direct root / block-local alias tail / block binding / local alias chain root 子路径，当前都额外开放了首个 different-closure control-flow 子集；同一条 local mutable alias 的 different-target reassign 现在也已开放到 ordinary direct call、ordinary local binding root、ordinary control-flow assignment-valued direct/binding/alias-chain root、cleanup direct root、cleanup local binding root、ordinary `match` guard-call，以及 runtime `if` / `match` branch-join 子集；其他逃逸路径仍关闭。

## 当前已开放的构建表面

### CLI 与产物

- `ql check`
- `ql build --emit llvm-ir|obj|exe|dylib|staticlib`
- `ql ffi header`
- `ql fmt`
- `ql mir`
- `ql ownership`
- `ql project graph [file-or-dir]`
- `ql project emit-interface [file-or-dir] [-o <output>]`
- `ql runtime`

### 最小 manifest / workspace 子集

当前 Phase 8 已开放的最小工程面：

- `qlang.toml` upward discovery
- `[package].name`
- `[workspace].members`
- `[references].packages`
- `ql project graph [file-or-dir]` 文本输出当前 manifest graph
- `ql project emit-interface [file-or-dir] [-o <output>]`
- `ql build <file> --emit-interface`：在构建成功后顺手写出当前包的默认 `<package>.qi`
- `.qi` V1 emit：package manifest 仍默认扫描 `manifest_dir/src/**/*.ql`，逐文件通过 `ql-analysis` 后生成 `<package>.qi`；workspace-only manifest 上的 `ql project emit-interface` 则会顺序为每个 `[workspace].members` 成员写出默认 `<package>.qi`，该模式下 `-o/--output` 继续保持关闭
- `.qi` V1 文本当前只保留 public surface：去掉 function body、`const`/`static` value、struct field default，并按源文件分段写入 `// source: ...` section
- `ql check` 当前已在 package-aware 路径上加载 `[references].packages` 指向的 `<dependency>.qi`，当入口是 package directory、`qlang.toml`、包内源码文件路径，或 workspace-only 根 manifest 时都会走这条路径，并在缺失 interface artifact 时显式失败；workspace-only 根会顺序检查每个 `[workspace].members` 成员包
- `ql check --sync-interfaces` 当前会在 package-aware check 前递归扫描本地 `[references].packages`，按默认输出位置同步写出 dependency `<package>.qi`；这个开关目前支持 package directory、`qlang.toml` 路径、能向上发现 manifest 的包内源码文件路径，以及 workspace-only 根 manifest；workspace-only 根会对成员包批量执行，并对重复 dependency 写出做去重
- dependency `.qi` 当前已进入 syntax-aware load：每个 `// source: ...` section 会通过 interface-mode parser 解析为 AST，支持 bodyless `fn` / `impl` / `extend` 声明，以及无值的 `const` / `static` 接口声明
- `ql-analysis::analyze_package` 当前已把 dependency `.qi` 的公开符号索引进 `PackageAnalysis`：覆盖 top-level `fn` / `const` / `static` / `struct` / `enum` / `trait` / `type`，以及 public trait / `impl` / `extend` methods；索引当前保留 package name、source section path、symbol kind、name、detail 与 interface span
- `ql-analysis` / `ql-lsp` 当前已开放 imported dependency symbol 的最小 cross-file hover / definition / declaration / references：当当前文件里的 import binding 能唯一映射到 dependency `.qi` public symbol 时，hover 会显示 dependency declaration，go to definition 与 go to declaration 都会跳到 dependency `.qi` artifact 内的 declaration 位置，references 会在需要时把 dependency `.qi` declaration 与当前文件 import/use 位置一起返回；当前这一合同同样覆盖 grouped import alias 形态，例如 `use demo.dep.{exported as run}`
- `ql-analysis` / `ql-lsp` 当前也已开放显式 type-namespace 位置上的最小 `textDocument/typeDefinition`，并开始小步扩到 value-position 的受控子集：same-file type use 现可跳到同文件 struct / enum / trait / type alias / generic 定义；same-file `use LocalType as Alias` 这类 local import type alias use 当前会优先跳到底层本地类型定义；当打开文档可成功装载其 package 时，dependency import/type root（例如 `Buf[Int]`）当前也可跳到 dependency `.qi` artifact 内的 public type declaration；与此同时，对能从语法局部恢复出 dependency struct 类型的 value/root，value token 本身现在也可通过 `textDocument/typeDefinition` 跳到 dependency public struct declaration，而 `config.child`、`Cfg { child: ... }`、`let Cfg { child } = ...`、`config.child().value` 这类 dependency struct field token 现在也可在字段声明类型能唯一映射到 dependency public type 时跳到对应 `.qi` public type declaration；同一条 truth surface 上，`config.child()` / `built.child()` / `(match ...).next()` 这类 dependency method token 现在也可在其声明返回类型能唯一映射到 dependency public type 时跳到对应 `.qi` public type declaration；当当前文档存在 same-file semantic errors、但语法仍可恢复且 dependency package 已可加载时，这几条 dependency typeDefinition 也都会继续工作。当前这条能力仍不做更广义值位推断 type jump，也不扩到 broader non-type-context fallback
- `ql-analysis` / `ql-lsp` 当前也已开放 `use ...` 导入路径位置上的 dependency completion：例如 `use demo.d` 可补全到 `dep`，`use demo.dep.Bu` 与 `use demo.dep.{Bu}` 都可补全到 `.qi` 里的 `Buffer`；grouped import 空补全位当前也会跳过已经写过的项，避免重复提示；该能力当前只覆盖导入路径，不代表更广义 symbol-space cross-file completion 已完成
- `ql-lsp` 当前已把 `workspace/symbol` 扩到保守的 package-aware + dependency-aware 子集：只要当前有一个已打开文档能成功装载其 package，搜索就会扩到该 package 的全部源码 modules，以及已加载 dependency `.qi` artifact 里的 public symbols；同包里已打开且可成功分析的源码文档会优先覆盖磁盘版本。若 package analysis 失败，则回退为当前已打开且可成功分析的源码文档。当前仍不会搜索未打开且与活动 package 无关的其它 packages、full workspace graph，或未加载的 dependency artifacts
- 上述 dependency-only 回退当前还额外覆盖了一个编辑中断场景：如果当前文档暂时存在 same-file 语义错误，只要 package manifest 与 dependency `.qi` 仍可装载，真实 `ql-lsp` backend 现在会在 source analysis 成功前先尝试 `use ...` 导入路径上的 dependency path segment / public symbol completion、dependency enum import alias root 上的 variant completion，以及 dependency struct explicit field-label completion；同一条回退链现在也已开放 imported dependency local name 的最小 hover / definition / declaration / references 查询，覆盖 import binding token 本身，以及 parse-only 可恢复的 single-segment use site/type root（例如 `run(1)`、`Buf[Int]`）；dependency enum variant token、explicit / shorthand struct-field token，以及语法局部可恢复出的 dependency struct receiver/member token 现在也已在同一路径下补齐最小 hover / definition / declaration / references 回退，覆盖 named local、direct dependency method-call，以及收敛 block/if/match 结果的 direct receiver，而同一 receiver slice 上的 member-field / member-method completion 也可继续工作；它仍不扩展到 broader member query 或其它同文件语义查询
- dependency enum import alias root 的首个非导入路径 contract 现已开放：当 `use demo.dep.Command as Cmd` 唯一映射到 dependency `.qi` 里的 public enum 时，`Cmd.Re` 这类路径位置现在会补全 dependency variants，而 `Cmd.Retry` 这类已写出的 variant token 也已支持 cross-file hover / definition / declaration / references；当当前文档仅有 same-file 语义错误时，真实 `ql-lsp` backend 现在也会继续从该 variant token 提供 dependency hover / definition / declaration / references 回退。当前这条能力只覆盖 enum variant roots，还不代表更广义 dependency member completion 已完成
- dependency struct import alias root 的首个 field contract 现也已继续扩到 value/member path：当 `use demo.dep.Config as Cfg` 唯一映射到 dependency `.qi` 里的 public struct 时，`Cfg { fl: true }` / `let Cfg { fl: enabled } = built` 这类显式 struct literal / struct pattern 字段标签现在已支持 dependency public field completion，并会跳过同一字面量/模式里已经写过的 sibling 字段；struct literal / struct pattern 里已写出的字段 token 当前继续支持 cross-file hover / definition / declaration / references，覆盖 explicit 与 shorthand 两种写法；此外，对能从语法局部恢复出 dependency struct 类型的 value/member receiver（当前覆盖 dependency-typed param、带 dependency type annotation 的 `let`、dependency struct literal `let` 结果、dependency method-call result、block tail result、收敛到同一个 dependency struct 的 `if` / `match` 结果、它们的 direct alias，以及不经命名 local 的 direct dependency method-call / structured receiver），`config.value` / `built.value` / `current.value` / `config.child().value` 这类 member field token 现在也已接通同一条 dependency hover / definition / declaration / references 合同，而 `config.va` / `built.va` / `(if ...).va` 这类 syntax-local member prefix 也已接通最小 dependency field completion；更广义 member completion 仍未开放
- dependency struct method/member path 的首个 method contract 现也已补齐到 completion 与 broken-source fallback：当 `use demo.dep.Config as Cfg` 能唯一映射到 dependency public struct，并且 `.qi` 中存在对该 nominal struct 的唯一 public method（遵守 impl 优先、再看 extend 的当前同文件规则）时，`config.get()` / `built.get()` / `current.get()` / `config.child().get()` 这类 member method token 现在已在成功分析路径与 same-file semantic-error fallback 上都支持 cross-file hover / definition / declaration / references，而 `config.ge` / `built.ge` / `(match ...).ge` 这类 syntax-local member prefix 也已接通最小 dependency method completion；当前这条 receiver slice 也已继续扩到 block tail result、收敛到同一个 dependency struct 的 `if` / `match` 结果，以及不经命名 local 的 direct dependency method-call / structured receiver，但仍不扩展到 broader dependency member semantics

当前仍未开放：

- dependency `.qi` 到 rename 与更广义非导入路径 completion 的 cross-file 消费
- 真实 package build graph
- dependency invalidation
- 更广义的 cross-file query / rename / completion

### sync build 子集

当前 sync build surface 已稳定覆盖：

- 顶层 free function
- `unsafe fn` body
- `extern "c"` 顶层声明、extern block、顶层导出定义
- `main` 程序入口
- 标量整数 / `Bool` / `Void`
- 最小 `String` 值传递子集
  - UTF-8 string literal lowering 为 `{ ptr, i64 }`
  - direct literal local binding
  - same-file `const` / `static` string item materialization
  - 普通参数传递与返回值
  - `==` / `!=` compare lowering
  - tuple / fixed-array / non-generic struct aggregate transport
  - 仍未开放：ordered compare、string-pattern match、C header `String` 导出
- `let` / `var` 局部绑定；当前已支持 statement-level 显式类型标注 `let name: Type = value` / `var name: Type = value`，以及 tuple / struct destructuring（叶子当前限 binding / `_`）
- direct call 与 named arguments
- same-file `use ... as ...` function alias call
- 最小 first-class sync function value 子集
  - same-file sync function item
  - same-file `use ... as ...` function alias
  - transparently resolve 到 same-file sync function item 的 callable `const` / `static`，以及它们的 same-file `use ... as ...` alias
  - non-capturing sync closure-backed callable `const` / `static`，以及它们的 same-file `use ... as ...` alias；当前 public regression 先锁定 ordinary positional indirect call 子集
  - non-capturing sync closure value；当前 public regression 已锁定 ordinary positional indirect call 的最小子集：zero-arg 形态、显式 typed closure parameter 形态、由 statement-level local callable type annotation 驱动的 parameterized local 形态，以及由 call-site positional argument 反推参数类型的 parameterized local/immutable-alias 形态；当前 shipped cleanup / guard-call 子路径也已显式锁定 direct local non-capturing closure 的最小子集
  - capturing sync closure value 的首个受控子集：当前只开放 non-`move` + immutable same-function scalar binding capture + direct local ordinary call、local alias ordinary call（含 same-target mutable reassign）、assignment-valued same-target ordinary direct callee root、control-flow 收敛到 same-target assignment-valued / block-local alias tail ordinary callee root、ordinary local binding 后再调用的 control-flow-selected / block-local assignment-valued / control-flow-assignment-valued / block-local alias tail binding root / local alias chain、local alias cleanup callee、local alias cleanup guard-call、assignment-valued cleanup callee / guard-call root、cleanup block 内局部 alias 的 direct call / guard-call（现含 statement-sequenced local mutable alias same-target reassign、statement-sequenced cleanup block 内局部 mutable alias 的 different-target reassign、assignment-valued same-target binding、control-flow 收敛到 same-target assignment-valued root 的 binding，以及 control-flow 分支内的 block-local alias tail binding；同类 direct cleanup callee / cleanup guard-call root 也已开放），以及 ordinary `match` guard-call（现含 direct callee root、先绑定到 ordinary local 后再调用的 control-flow-selected root、block-local assignment-valued / control-flow-assignment-valued binding root，以及 different-closure control-flow 下的 block-local alias tail / block binding / local alias chain callee root）；runtime `if` / `match` 当前也已开放 same-target control-flow 子集，即所有分支最终都收敛到同一个已支持 capturing closure local/alias 时，可继续进入 ordinary/local binding call、cleanup callee 与 cleanup guard-call；ordinary direct call、ordinary local binding root 及其后续 local alias chain 调用（现含后续未重写块）、ordinary `match` guard-call，以及 cleanup callee / cleanup guard-call 这边，现在都额外接受 runtime `if` / `match` 选出的首个 different-closure callee root 子集，只要各分支都仍落在当前 shipped direct root / block-local alias tail / block binding / local alias chain root 子集；同一条 local mutable alias 的 different-target reassign 现在也已开放到 ordinary direct call、ordinary local binding root、ordinary control-flow assignment-valued direct/binding/alias-chain root、cleanup direct root、cleanup local binding root、ordinary `match` guard-call，以及 runtime `if` / `match` branch-join 子集；其他 broader callable-value flow 的路径仍保持关闭
  - runtime `if` / `match` callable value 子集：当前 ordinary local binding 与 cleanup value path 也可从 same-file function item / alias、function-item-backed callable `const` / `static` / alias，以及 closure-backed callable `const` / `static` / alias 里选出 indirect callee
  - ordinary call 可 direct call，或先绑定到 local 后再做 positional indirect call
  - ordinary `match` guard，以及当前 shipped cleanup call / guard-call 子路径，也可通过 function-item-backed callable local / callable `const` / `static` / same-file alias 进入 positional indirect call；当前 public regression 也已显式锁定 direct closure-backed callable `const` guard + closure-backed callable `static` cleanup、direct local non-capturing closure cleanup + guard，以及 direct local capturing closure cleanup + cleanup guard + ordinary guard（现含 control-flow-selected direct callee root、local binding root、block assignment-bound root，以及 cleanup different-closure direct-root / block-local-alias-tail-root / block-binding / local-alias-chain-root 子集）的最小子集
- 最小 first-class async function value 子集
  - same-file async function item
  - same-file `use ... as ...` async function alias
  - transparently resolve 到 same-file async function item 的 callable `const` / `static`，以及它们的 same-file `use ... as ...` alias
  - 当前 public regression 已锁定 `async fn` 内 ordinary direct call 或 ordinary local positional indirect call + `await` 子集
  - runtime `if` / `match` callable value 子集：当前 `async fn` 内 ordinary local binding + `await`，以及 cleanup value path 里的 `await callable(...)`，也可从 same-file async function item / alias 与 async callable `const` / `static` / alias 里选出 indirect callee
  - capturing closure value，以及 cleanup callee / guard-call 上的 async callable path 仍保持关闭
- fixed-shape `for`
  - fixed-array
  - homogeneous tuple
  - `binding` / `_` / tuple destructuring / struct destructuring loop pattern
  - projected root / direct call-root / same-file import-alias call-root / nested call-root / same-file import-alias nested call-root
  - block-valued / assignment-valued / runtime `if` / `match` valued projected root
  - parenthesized / unparenthesized inline projected root
  - same-file `const` / `static` root 及其 same-file alias
- 赋值表达式的当前可运行子集
  - mutable local
  - tuple literal index projection，以及 same-file `const` / `static` / `use ... as ...` alias、branch-selected const `if` / 最小 literal `match` item value、direct inline foldable `if` / `match` integer expression，和 immutable direct local alias 复用驱动的 foldable integer constant expression tuple index
  - struct field
  - fixed-array literal index projection
  - projected-root / nested projected-root / call-root nested projected-root / import-alias call-root nested projected-root / inline nested projected-root tuple / struct-field / fixed-array literal-index chains
  - assignment expr value form：当前已覆盖 direct call arg 与 valued block tail
- 动态数组索引赋值的当前可运行子集
  - non-`Task[...]` element arrays
  - nested dynamic array projections
  - projected-root dynamic array projections
  - direct-root / projected-root / call-root nested projected-root / import-alias call-root nested projected-root / inline nested projected-root assignment-expression result form
  - nested projected-root assignment-expression result form
- 普通表达式与 `if` / `while` 条件里的 same-file foldable `const` / `static`，包括 computed/projected item value，以及 foldable const `if` / 最小 literal `match` 选出的 branch-selected item value
- `Bool` `&&` / `||` / unary `!`
- 最小 literal `match` lowering
  - `Bool` / `Int` literal-path 子集
  - 其他 current-loadable scrutinee 的 catch-all-only 子集
- 当前 bool/scalar-comparison guard 子集
- bool guard path 现也接受同一批 ordinary local / param / `self` root 的 bool assignment expr value；当前已锁定 shipped cleanup `if` condition 公开回归
- direct resolved sync guard call 子集
  - guard-call arg value path 现也接受同一批 ordinary local / param / `self` root 的 assignment expr value，包括当前 loadable guard-call arg 子集
  - guard-call arg value path 现也接受最小 runtime `if` value 子集：当前已锁定 loadable guard-call arg 的 `if cond { ... } else { ... }` 形态
  - guard-call arg value path 现也接受最小 runtime `match` value 子集：当前已锁定 bool/int scrutinee + 既有 guard-match arm 子集上的 loadable guard-call arg 形态
  - guard-call callee root 现也接受最小 runtime `if` / `match` callable value 子集：当前已锁定由 same-file function item / alias、function-item-backed callable `const` / `static` / alias，以及 closure-backed callable `const` / `static` / alias 选出的 indirect callee 形态
  - callable-value positional indirect guard call 子集：当前已覆盖 callable local / callable `const` / `static` / same-file alias
  - inline aggregate guard-call arg / inline projection-root 子集
  - call-root / nested call-root guard 子集

### async library build 子集

当前 async library build 已稳定开放：

- `staticlib`
- 最小 async `dylib`
  - 仍要求公开导出面保持同步 `extern "c"` C ABI

当前 library-mode async 子集已有真实 pass matrix 覆盖：

- scalar / tuple / array / struct / nested aggregate `await`
- `Task[T]` flow、payload、projection consume / submit
- projected reinit、stable-dynamic path、guard-refined path
- fixed-shape `for await`
  - fixed-array
  - homogeneous tuple
  - task-array / task-tuple auto-await
  - same-file scalar `const` / `static` root、same-file scalar item alias，以及 scalar item-backed read-only projected root
  - same-file task-producing `const` / `static` root，以及 same-file task item alias root
  - projected / block-valued projected / assignment-valued projected / runtime `if` / `match` valued projected / call-root / awaited-aggregate / import-alias / inline / nested call-root
- 普通标量赋值表达式的当前可运行子集
  - mutable local
  - tuple literal index projection，以及 same-file `const` / `static` / `use ... as ...` alias、branch-selected const `if` / 最小 literal `match` item value、direct inline foldable `if` / `match` integer expression，和 immutable direct local alias 复用驱动的 foldable integer constant expression tuple index
  - struct field
  - fixed-array literal index projection
  - projected-root / nested projected-root / call-root nested projected-root / import-alias call-root nested projected-root / inline nested projected-root tuple / struct-field / fixed-array literal-index chains
- 动态 `Task[...]` 数组索引赋值的当前可运行子集
  - generic direct-root write-before-consume success path
  - generic projected-root write-before-consume success path
- async 普通标量动态数组索引赋值的当前可运行子集
  - direct-root non-`Task[...]` arrays
  - projected-root non-`Task[...]` arrays
  - direct-root / projected-root / call-root nested projected-root / import-alias call-root nested projected-root / inline nested projected-root assignment-expression result form
  - nested projected-root assignment-expression result form
- 最小 async `match` family
  - direct-call guard
  - projection guard
  - aggregate guard-call arg
  - inline aggregate / inline projection
  - nested call-root families

### async executable build 子集

当前 async executable 只开放：

- `BuildEmit::LlvmIr`
- `BuildEmit::Object`
- `BuildEmit::Executable`
- 程序入口限定为最小 `async fn main`
- `async unsafe fn` body 会沿当前最小 `async fn main` 子集一起 lowering

当前 program-mode async 子集真实覆盖：

- `Task[T]` 类型面
- direct async call / helper-returned task handle / `spawn` / `await`
  - regular-size helper-returned / forwarded task-handle flow
  - bound local task-handle `spawn`
  - regular-size aggregate params on direct `await` / `spawn`
  - zero-sized helper-returned / forwarded task-handle flow
  - zero-sized aggregate params on direct `await` / `spawn`
  - recursive aggregate params on direct `await` / `spawn`
- scalar 与 fixed-shape aggregate payload
  - tuple / fixed-array / non-generic struct
  - regular-size direct / spawn aggregate result family
  - zero-sized aggregate
  - direct / spawn recursive fixed-shape aggregate result
  - aggregate 内继续携带 `Task[T]`
  - regular-size tuple / array / nested aggregate task-handle payload family
  - nested task-handle payload
  - regular-size returned / nested / struct-carried task-handle shapes
  - zero-sized returned / nested / struct-carried task-handle shapes
- projected task-handle consume
  - tuple index
  - fixed-array literal index
  - struct field
  - regular-size fixed-array projected reinit / conditional reinit
  - zero-sized tuple / fixed-array / struct projection `await` / `spawn`
  - zero-sized tuple / fixed-array / struct projection reinit
  - direct call-root / awaited-aggregate / import-alias / inline / nested-call-root zero-sized consume
  - direct call-root / nested call-root / awaited-aggregate / inline aggregate
- conditional task-handle control flow
  - regular-size branch-local `spawn` + reinit
  - regular-size conditional async-call `spawn`
  - regular-size conditional helper-task `spawn`
  - guard-refined arithmetic static alias-sourced composed-dynamic forwarded helper `await` / direct queued `spawn`
  - zero-sized branch-local `spawn` + reinit
  - zero-sized conditional async-call `spawn`
  - zero-sized conditional helper-task `spawn`
- aliased projected-root aggregate repackage / submit
  - tuple / struct / nested aggregate repackage before `await`
  - fixed-array / nested fixed-array / helper-forwarded nested fixed-array repackage before `spawn`
  - source-root reinit 后的 same-file arithmetic item / same-file `use ... as ...` alias 驱动 direct alias-root、projected-root、alias-sourced composed-dynamic 与 guard-refined alias-sourced composed-dynamic 形态，包括 bundle-alias-forwarded、bundle-alias-inline-forwarded、queued-root-forwarded、queued-root-inline-forwarded、queued-root-alias-forwarded、queued-root-alias-inline-forwarded、queued-root-chain-forwarded、queued-root-chain-inline-forwarded、queued-local-alias、queued-local-chain、queued-local-forwarded、queued-local-inline-forwarded、bundle-chain-forwarded 与 bundle-chain-inline-forwarded await/spawn
- dynamic fixed-array `Task[...]` 的保守子集
  - generic dynamic sibling-safe consume / spawn
  - same immutable stable source path precise consume / reinit
  - projected-root stable dynamic reinit / conditional reinit
  - aliased / const-backed alias-root stable-dynamic reinit
  - composed / alias-sourced composed dynamic reinit
  - foldable integer arithmetic expression 回收到 concrete literal/projection path
  - direct inline foldable `if` / 最小 literal `match` integer expression 回收到 concrete literal path consume / reinit
  - direct / projected / aliased guard-refined dynamic reinit，包括 arithmetic-backed refined source 及其 same-file `use ... as ...` alias 包裹形态
  - same-file static/import-alias-backed projected-root dynamic reinit
  - same-file `const` / `static` / `use ... as ...` alias 回收到 literal/projection path，包括 computed/projected item value、branch-selected const `if` / 最小 literal `match` item value、foldable arithmetic item value，以及这些 item value 的 same-file `use ... as ...` alias 包裹形态
  - equality-guard refinement
  - projected-root / alias-root canonicalization
- fixed-shape `for await`
  - fixed-array
  - homogeneous tuple
  - task-array / task-tuple auto-await
  - same-file scalar `const` / `static` root、same-file scalar item alias，以及 scalar item-backed read-only projected root
  - same-file task-producing `const` / `static` root、same-file task item alias root，以及 projected task item root
  - projected / block-valued projected / assignment-valued projected / runtime `if` / `match` valued projected / call-root / awaited-aggregate / import-alias / inline / nested call-root
- runtime task-backed item value flow
  - same-file task-producing `const` / `static` item 与 same-file alias，当前也可经过 ordinary local binding、sync helper 参数/返回值，以及 runtime `if` / `match` 选值后，再进入 projected `await` / fixed-shape `for await`
- 普通表达式与 `if` / `while` 条件里的 same-file foldable `const` / `static`
  - 包括 computed/projected item value
  - 包括 foldable const `if` / 最小 literal `match` 选出的 branch-selected item value
- awaited `match` guard 子集
  - awaited scalar + direct-call guard
  - guard scalar/value path 现也接受最小 runtime `await` value 子集：当前已锁定 ordinary `match` guard 里的 awaited scalar comparison 形态，其中 `await` operand 可来自 runtime `if` / `match` 选出的 same-file async function item / alias 与 async callable `const` / `static` / alias direct callee root
  - awaited aggregate + projection guard；当前也已锁定 ordinary `match` guard 里的 awaited projected scalar comparison 形态，其中 aggregate-producing `await` operand 同样可来自 runtime `if` / `match` 选出的 same-file async function item / alias 与 async callable `const` / `static` / alias direct callee root
  - aggregate guard-call arg / call-backed aggregate arg；当前也已锁定 ordinary `match` guard 里的 awaited aggregate guard-call arg 与 awaited call-backed aggregate guard arg 形态，其中 aggregate-producing `await` operand 可来自 runtime `if` / `match` 选出的 same-file async function item / alias 与 async callable `const` / `static` / alias direct callee root
  - import-alias helper family；当前也已锁定 ordinary `match` guard 里的 awaited import-alias helper 形态，其中 helper arg 可直接承接 runtime `if` / `match` 选出的 same-file async function item / alias 与 async callable `const` / `static` / alias direct callee root 的 awaited aggregate value
  - inline aggregate arg / inline projection-root；当前也已锁定 ordinary `match` guard 里的 awaited inline projection-root 与 awaited inline aggregate arg 形态，例如 `State { value: (await ...).value }.value`、`matches((0, (await ...).value), ...)` 与 `contains([0, (await ...).value, 2], ...)`
  - nested call-root runtime projection family；当前也已锁定 ordinary `match` guard 里的 awaited nested call-root runtime projection 形态，其中 nested projected scalar 可来自 `wrap(await ...)` 这类 call-backed aggregate root，且 inner `await` operand 继续接受 runtime `if` / `match` 选出的 same-file async function item / alias 与 async callable `const` / `static` / alias direct callee root
  - nested call-root deeper inline-combo family；当前也已锁定最小 awaited inline-combo 形态，例如 `[wrap(await ...).slot.value, 0][offset(...)]`
- awaited `match` scrutinee 子集
  - direct awaited scrutinee + control-flow-root family；当前也已锁定 ordinary `match await ...` 形态，其中 awaited direct scrutinee 的 callee root 可来自 runtime `if` / `match` 选出的 same-file async function item / alias 与 async callable `const` / `static` / alias
  - awaited aggregate catch-all family；当前 ordinary `match await ...` 的 non-scalar current-loadable scrutinee 已不再只限单名 binding catch-all，而是开放 `_` / single binding / tuple destructuring / struct destructuring catch-all。当前已锁定 ordinary `match await ... { current => ... }`、`match await ... { (left, right) => ... }` 与 `match await ... { State { slot: Slot { value } } => ... }` 这类形态，现已覆盖 awaited struct / tuple / fixed-array aggregate scrutinee，其中 awaited root 同样可来自 runtime `if` / `match` 选出的 same-file async function item / alias 与 async callable `const` / `static` / alias
  - direct awaited projected scrutinee family；当前也已锁定 ordinary `match (await ...).value` 这类 direct projected scrutinee 形态，其中 awaited aggregate root 同样可来自 runtime `if` / `match` 选出的 same-file async function item / alias 与 async callable `const` / `static` / alias
  - helper / inline scrutinee family；当前也已锁定 ordinary `match` 里的 awaited helper scrutinee 与 awaited inline projection-root scrutinee 形态，例如 `match helper_alias(13, (await ...).slot.value)` 与 `match State { slot: Slot { value: (await ...).slot.value } }.slot.value`
  - nested call-root runtime projection / inline-combo scrutinee family；当前也已锁定 ordinary `match` 里的 `match wrap(await ...).slot.value` 与 `match [wrap(await ...).slot.value, 0][offset(...)]` 形态，其中 inner `await` operand 同样接受 runtime `if` / `match` 选出的 same-file async function item / alias 与 async callable `const` / `static` / alias direct callee root

### cleanup lowering 子集

当前 cleanup lowering 只开放首个受控子集：

- direct / call-backed `defer`
- 其中 call-backed `defer` 当前已覆盖 direct resolved callee，以及 callable local / callable `const` / `static` / same-file alias 驱动的 positional indirect callee；对 capturing sync closure，还额外开放了 local alias 未重绑定前提下的 cleanup callee 与 cleanup guard-call 最小子集，以及 cleanup block 内 `let alias = run` / `let alias = check` 这类局部 alias 的 direct call / guard-call；statement-sequenced cleanup block 现也已开放 `var alias = run; alias = run; alias(...)` / `var alias = check; alias = check; if alias(...) { ... }` 这类 same-target mutable reassign 形态，而 `var alias = left; alias = right; alias(...)` / `var alias = left_check; alias = right_check; if alias(...) { ... }` 这类无 branch-join 的 different-target mutable reassign 现也进入当前 cleanup block shipped surface；同一类 block-local different-target mutable alias 现也可直接作为 cleanup callee / guard-call root，即 `defer ({ var alias = left; alias = right; alias })(...)` / `defer if ({ var alias = left_check; alias = right_check; alias })(...) { ... }`；`(alias = run)(...)` / `if (alias = check)(...) { ... }` 这类 assignment-valued same-target mutable alias 现也可直接进入 cleanup callee / guard-call root；`let chosen = alias = run` / `let chosen = alias = check` 这类 cleanup block assignment-valued binding 现在也可继续把同一个 capturing closure 绑定给新局部，而 `let chosen = if ... { alias = run } else { run }` / `let chosen = match ... { ... }` 这类 control-flow 收敛到 same-target assignment-valued root 的 binding 也已接通；当前还进一步开放了 `defer (if ... { let alias = run; alias } else { run })(...)` 与 `defer if (match ... { true => { var alias = check; alias = check; alias }, false => check })(...) { ... }` 这类分支内 block-local alias tail 直接作为 cleanup callee / guard-call root 的形态；同一条 root 现在还可继续包进 cleanup `if` / `match` control-flow，只要各分支最终仍收敛到同一个 same-target assignment-valued capturing closure；runtime `if` / `match` typed-value path 当前也可选出 same-file function item / alias、function-item-backed callable `const` / `static` / alias，以及 closure-backed callable `const` / `static` / alias 作为 callable cleanup callee root，而 capturing closure 当前除 same-target cleanup control-flow 子集外，也已开放 different-closure cleanup control-flow 的首个 direct-root / block-local-alias-tail-root / block-binding / local-alias-chain-root 子集；同一条 local mutable alias 的 different-target reassign 现在也已沿 shared local path 打通 ordinary direct call、ordinary local binding root、cleanup direct root、cleanup local binding root、ordinary `match` guard-call，以及 runtime `if` / `match` branch-join 子集；在这条 shared-local 路径上，cleanup `if` assignment-valued binding 与 cleanup bool `match` assignment-valued binding 现在都已进一步开放后续 alias-chain，即 `let chosen = ...; let rebound = chosen; rebound(...)` 与对应 guard-call 形态已进入当前 build surface；其他 cleanup escape flow 与 broader cleanup control-flow 仍关闭
- statement-sequenced block wrapper：当前接受 binding / `_`、tuple destructuring、struct destructuring（叶子仍限 binding / `_`）的最小 `let` statement、已支持 cleanup expr statement、statement-level assignment expr、statement-level `while` / `loop` / `for`，外加可选 tail；当前已覆盖 direct cleanup body、cleanup `let` binding / destructuring block、cleanup guard / scrutinee block、cleanup call-arg value block，以及 rooted in ordinary local / param / `self` place family 的 local/field/tuple-index/fixed-array-index assignment expr statement
- cleanup value path 现也接受同一批 ordinary local / param / `self` place family root 的 assignment expr value，包括 direct cleanup call arg 与 valued cleanup block tail；当前仍限 local/field/tuple-index/fixed-array-index target path
- cleanup value path 现也接受最小 runtime `if` value 子集：当前已锁定 direct cleanup call arg 的 `if cond { ... } else { ... }` 形态
- cleanup value path 现也接受最小 runtime `match` value 子集：当前已锁定 direct cleanup call arg 的 bool/int scrutinee + 既有 cleanup-match arm 子集
- cleanup value path 现也接受最小 runtime `await` value 子集：当前已锁定 async body 内 direct cleanup call arg 的 `await task` 形态
- cleanup value path 现也接受最小 runtime `spawn` value 子集：当前已锁定 async body 内 direct cleanup call arg 的 `spawn worker(...)` / `spawn task` 形态
- statement-level cleanup `while`：当前开放 bool 条件 + 已支持 cleanup block body 的最小 lowering 子集，可在 cleanup block 内重复执行 direct / callable-backed call 路径，并支持 body-local `break` / `continue`（包括经由当前已开放 cleanup `if` branch 进入的 loop-exit path）
- statement-level cleanup `loop`：当前开放已支持 cleanup block body 的最小 lowering 子集，并支持 body-local `break` / `continue`（包括经由当前已开放 cleanup `if` branch 进入的 loop-exit path）
- statement-level cleanup `for`：当前开放 fixed array / homogeneous tuple iterable + binding / `_` / tuple destructuring / struct destructuring（叶子仍限 binding / `_`）pattern 的最小 lowering 子集，iterable 当前已覆盖 direct root、same-file `const` / `static` root 及其 same-file alias、item-backed read-only projected root、direct call-root、same-file import-alias call-root、nested call-root projected root，以及 transparent `?` wrapper 下的 projected root 形态；body 内可读取当前 item，并支持 body-local `break` / `continue`（包括经由当前已开放 cleanup `if` branch 进入的 loop-exit path）
- statement-level cleanup `for await`：当前在 async body 内开放 fixed array / homogeneous tuple iterable 的最小 lowering 子集；普通元素会直接逐项绑定，`Task[...]` 元素会复用既有 `await` + result-release 路径做逐项 auto-await，并支持 body-local `break` / `continue`；当前已锁定 direct local root、same-file scalar `const` / `static` root、same-file scalar item alias、same-file task-producing `const` / `static` root、same-file task item alias root、projected task item root、scalar item-backed read-only projected root、direct block-valued / assignment-valued / runtime `if` / `match` / awaited direct root、direct question-mark root、read-only projected root、assignment-valued projected root、block-valued projected root、direct call-root、same-file import-alias call-root、nested call-root projected root、awaited projected root、runtime `if` / `match` aggregate projected root、transparent `?` wrapper 下的 projected root，以及 inline array/tuple task root
- cleanup runtime task-backed item value flow
  - same-file task-producing `const` / `static` item 与 same-file alias，当前也可经过 cleanup local binding、sync helper 参数/返回值，以及 runtime `if` / `match` 选值后，再进入 projected `await` / fixed-shape cleanup `for await`
- cleanup aggregate value staging：cleanup `let` / valued block / projected-root materialization 现在会沿 tuple / array / struct literal 递归走 cleanup 自身的 value path；这意味着 awaited projected loadable value 现在可以先被装入 cleanup struct literal 字段，再继续被后续 cleanup `for await` / projected read 消费
- bool-guard 驱动的 cleanup `if` branch；当前已不再只限 call-backed expr，branch body 也可承载当前已开放的 cleanup block 语句子集，包括 local binding、nested control-flow value path 与 async `for await`；对应 bool/int guard call 子路径也已覆盖 callable local / callable `const` / `static` / same-file alias 驱动的 positional indirect call，并接受 runtime `if` / `match` 选出的 same-file function item / alias、function-item-backed callable `const` / `static` / alias，以及 closure-backed callable `const` / `static` / alias callee root
- cleanup bool guard path 现也接受最小 runtime `await` value 子集：当前已锁定 async body 内 `defer if await ready()`、cleanup `match` guard `true if await check(...)`、`defer if await (if` / `match` 选出的 same-file async function item / alias 与 async callable `const` / `static` / alias `)(...)`、`defer if helper_alias(..., await ...)` / `defer if State { value: (await ...).value }.value == ...` 这类 awaited helper / inline guard，以及 `defer if wrap(await ...).slot.value == ...` / cleanup `match` guard 里的 `[wrap(await ...).slot.value, 0][offset(...)]` 这类 awaited nested runtime projection / inline-combo guard 形态
- cleanup `match` branch 当前开放两类 scrutinee 子集：其一是 bool / int scrutinee + literal-or-path / wildcard-or-single-binding catch-all arms + optional bool guard；其二是 loadable 非标量 scrutinee + catch-all pattern（`_` / single binding / tuple destructuring / struct destructuring）+ optional bool guard。当前也不再只限 call-backed arm expr，arm body 可承载同一批已开放 cleanup block 语句子集，包括 binding arm body 与 async `for await`；cleanup scalar call-arg value 里的 call 子路径也已覆盖同一批 callable-value 间接调用，而 cleanup scrutinee path 现也接受最小 runtime `await` value 子集：当前已锁定 `defer match await ...`、`defer match (await ...).value { ... }`、`defer match helper_alias(..., await ...)`、`defer match State { value: (await ...).value }.value { ... }` 这类 awaited helper / inline scrutinee，以及 `defer match wrap(await ...).slot.value { ... }` / `defer match [wrap(await ...).slot.value, 0][offset(...)] { ... }` 这类 awaited nested runtime projection / inline-combo scrutinee 形态；此外，awaited struct / tuple / fixed-array aggregate scrutinee 现也可通过 single-binding catch-all 直接进入 cleanup body，例如 `defer match await ... { current => ... }`，而 awaited tuple / struct aggregate scrutinee 还可进一步直接在 catch-all arm 中解构，例如 `defer match await ... { (left, right) => ... }` / `State { slot: Slot { value } } => ...`，其中 awaited callee root 同样可来自 runtime `if` / `match` 选出的 same-file async function item / alias 与 async callable `const` / `static` / alias
- 透明 `?` wrapper，可包裹当前 shipped cleanup expr / guard / scrutinee 子路径
- cleanup value path 现也会复用既有 literal-source folding：cleanup `let` value、cleanup `for` iterable、cleanup `if` bool condition，以及 cleanup call-arg scalar/value path 当前都接受可折叠回既有 literal / aggregate root 的 `if` / 最小 literal `match` 根表达式
- 当前已锁定的用户面包括 direct cleanup `obj` build、callable-const-alias cleanup `obj` build、closure-backed callable global cleanup + guard `obj` build、local non-capturing closure cleanup + guard `obj` build、ordinary extended capturing-closure call-root `obj` build、local alias capturing closure cleanup + cleanup if/match guard + ordinary guard `obj` build、cleanup assignment-valued capturing-closure callee / guard root `obj` build、cleanup control-flow local-alias capturing-closure direct callee / guard root `obj` build、cleanup different-closure capturing-closure call-root `obj` build、cleanup block local capturing-closure alias `obj` build、cleanup block local mutable capturing-closure same-target reassign `obj` build、cleanup block assignment-valued capturing-closure binding `obj` build、cleanup block control-flow assignment-valued capturing-closure binding `obj` build、cleanup `if` shared-local control-flow capturing-closure alias-chain `obj` build、cleanup bool `match` shared-local control-flow capturing-closure alias-chain `obj` build、cleanup guarded `match` shared-local control-flow capturing-closure alias-chain `obj` build、cleanup `let` binding / destructuring block `obj` build、callable-guard-alias cleanup `match` `obj` build、binding-catch-all cleanup `match` `obj` build、statement-sequenced cleanup block `obj` build、statement-sequenced cleanup guard / scrutinee / call-arg value block（现含 runtime `await` / `spawn` task value，以及 awaited async callable control-flow callee root / awaited cleanup scrutinee / awaited helper guard / awaited inline guard root / awaited nested runtime projection guard / awaited helper-inline scrutinee / awaited nested runtime projection scrutinee / awaited aggregate single-binding scrutinee / awaited aggregate tuple-struct destructuring scrutinee）`obj` build、带 body-local `break` / `continue` 的 statement-level cleanup `while` / `loop` `obj` build、包含 tuple/struct 解构 pattern、const/static root、projected/call-root、alias call-root、nested call-root projected 与 transparent `?` wrapper projected root 形态在内的 fixed-shape statement-level cleanup `for` `obj` build、async body 内 fixed array / homogeneous tuple + task-element auto-await 子集的 cleanup `for await` `obj` build（现含 same-file scalar `const` / `static` root、same-file scalar item alias、same-file task-producing `const` / `static` root、same-file task item alias root、projected task item root、scalar item-backed read-only projected root、direct block-valued / assignment-valued / runtime `if` / `match` / awaited direct root、direct question-mark root、read-only projected root、assignment-valued projected root、block-valued projected root、direct call-root、same-file import-alias call-root、nested call-root projected root、awaited projected root、runtime `if` / `match` aggregate projected root、transparent `?` wrapper 下的 projected root 与 inline array/tuple task root）、guarded dynamic task-handle cleanup `staticlib` build、cleanup `match` `obj` build，以及 cleanup-internal question-mark `obj` build

### 透明 `?` lowering

当前透明 `?` 表达式会沿 inner operand 直接进入既有 codegen 路径：

- `match` + `?` 不再因为 question-mark 本身被 backend 拦截
- cleanup-adjacent 的 `return helper()?` / 普通 return path 也不再单独报 `?` lowering unsupported
- cleanup-internal 的 `defer helper()?` 也不再单独报 cleanup / `?` lowering unsupported

## 当前回归规模

截至当前代码：

- sync executable examples：`60`
- async executable examples：`222`

注意：

- async 目录文件编号从 `04` 编到 `225`，但真实 `.ql` 文件数是 `222`，不是 `225`
- `crates/ql-cli/tests/executable_examples.rs` 当前也只注册了 `222` 个 async executable case 和 `60` 个 sync executable case

## 当前明确未开放

- 更广义的 async executable / program bootstrap，除最小 `async fn main` 以外
- 更广义的 async `dylib` surface，尤其是公开 async ABI
- generalized `for await`，超出 fixed-array / homogeneous tuple 之外的 iterable
- 更广义的 runtime const/static/item-backed aggregate lowering，超出当前 async ordinary/cleanup value path 已锁定的 same-file task-backed item root、projected item-root，以及 local/helper/control-flow 传递子集之外仍未开放；当前 const item lowering 仍不会把 `worker(...)` 这类 runtime task-producing initializer 普遍提升为通用常量值
- broader cleanup lowering / cleanup codegen，超出当前 direct / call-backed `defer` + `if` / `match` + 透明 `?` wrapper cleanup 子集之外
- broader callable value lowering，超出当前 same-file sync function item / same-file alias / function-item-backed callable `const` / `static` 子集、closure-backed callable `const` / `static` 的 ordinary positional indirect-call 最小子集与 direct cleanup/guard item 子集、non-capturing sync closure value 的 ordinary positional indirect-call 最小子集与 direct local cleanup/guard 子集（zero-arg + explicit typed-parameter shape + statement-level local callable type-annotation shape + call-site positional-arg-inferred parameterized local/immutable-alias shape）、capturing sync closure value 的首个受控子集（non-`move` + immutable same-function scalar binding capture + ordinary local/same-target call roots、ordinary local binding root 及其后续 local alias chain 调用、ordinary control-flow assignment-valued direct/binding/alias-chain root、cleanup direct/guard/binding root 与 ordinary `match` guard-call root；其中 mutable alias 当前已开放 same-target reassign、ordinary/cleanup/ordinary-match-guard 路径上的 local different-target reassign、runtime `if` / `match` branch-join 子集，以及 statement-sequenced cleanup block 内局部 mutable alias 的 different-target reassign，而 different-closure control-flow 当前只开放 ordinary direct call、ordinary local binding root 及其后续 local alias chain 调用（含后续未重写块）、ordinary `match` guard-call，与 cleanup callee / cleanup guard-call 的 direct root / block-local alias tail / block binding / local alias chain root 子集），以及 same-file async function item / alias / callable `const` / `static` / same-file alias 的 ordinary local indirect-call + `await` 子集之外；capturing closure 的其他 cleanup escape flow，以及 cleanup 内更广义的 async control-flow 仍未开放
- cancellation / polling / drop semantics
- generic async ABI / layout substitution
- arbitrary dynamic overlap precision
- 更广义的 projection-sensitive partial move / partial-place ownership
- 超出当前 minimal subset 的 `match` lowering、guard shape 与 pattern discrimination

## 推荐阅读顺序

如果你要继续开发，建议按这个顺序恢复上下文：

1. [开发计划](/roadmap/development-plan)
2. [P1-P7 阶段总览](/roadmap/phase-progress)
3. [Phase 7 设计合并稿](/plans/phase-7-concurrency-and-rust-interop)
4. [工具链设计](/architecture/toolchain)
5. [路线图归档](/roadmap/archive/index)
