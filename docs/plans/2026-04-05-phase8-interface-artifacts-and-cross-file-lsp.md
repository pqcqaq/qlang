# Phase 8：`.qi` 接口产物与 Cross-File LSP

## Goal

Phase 8 的顺序是：

1. 先稳定 package/workspace graph 和 `.qi` 接口产物。
2. 再让 `ql-analysis` / `ql-lsp` 消费 `.qi` 和源码。
3. 最后逐步开放受限 workspace rename / code actions。

不反过来做，是为了避免 LSP 先长出一套临时源码扫描语义，后面再接 `.qi` 时返工。

## `.qi` contract

`.qi` 是 compiler-facing public interface artifact，不是 LSP 专用缓存。

V1 目标：

- 每个 package/module 输出稳定文本接口
- 只承载 public API 需要的类型和符号元数据
- 编译器、analysis、LSP 共用同一份读取逻辑

V1 覆盖：

- public `fn` / `extern "c"` signature
- public `struct` / `enum` / `trait` / `type alias`
- generic parameter surface
- public field / variant shape
- hover 需要的基础 doc/comment 文本

V1 明确不覆盖：

- function body
- impl / method body
- 完整跨包 const-eval
- registry / publish

## Current status

- `ql project emit-interface` 已落地。
- `ql-analysis::analyze_package` 已能消费 package graph 和依赖 `.qi`。
- dependency-backed hover / definition / declaration / references / completion 已接入当前支持切片。
- source-preferred local dependency tooling 已优先读取 open documents。
- 受限 workspace rename / code actions 已在后续切片中开放一部分。

## Remaining work

- 更完整的 package identity 和 invalidation
- 更宽的 dependency-backed completion / references
- workspace-wide index
- 更完整的 workspace rename / code actions
- `.qi` 与 doc generation / build cache 的后续复用

## Boundaries

- 不为 LSP 单独做第二套 dependency symbol index
- 不把 `.qi` 改成 Rust 内部元数据格式
- 不借 `.qi` 一次性打开完整 module graph / registry / publish
- 不为了“看起来完整”提前做大而全 code actions
