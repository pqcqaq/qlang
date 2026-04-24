---
layout: home

hero:
  name: Qlang
  text: 独立设计的编译型系统语言
  tagline: 当前实现以 Rust workspace 交付；开发主线聚焦真实项目工作流、基础 LSP 和工具链闭环。
  actions:
    - theme: brand
      text: 当前支持基线
      link: /roadmap/current-supported-surface
    - theme: alt
      text: 开发计划
      link: /roadmap/development-plan

features:
  - title: 安装方式
    details: 当前只有 source build + local VSIX 这一路径；还没有预编译 release 或 Marketplace 分发。
  - title: 当前重点
    details: package/workspace、`.qi`、local dependencies、project-aware build/run/test、dependency-backed LSP。
  - title: 当前边界
    details: 跨包执行当前稳定覆盖 direct local dependency 的 bridgeable public `const/static` values、受限 public top-level free function、public `extern "c"` 符号、被这些签名直接引用的 public 非泛型 `struct` / `enum`，以及这些 bridgeable public `struct` 上的受限 public receiver method；rename 以 same-file 为默认保守面，并已覆盖 source-backed dependency member / enum variant 与 workspace root symbol 的受限 workspace edits。
  - title: 真相源
    details: 如果文档与实现冲突，以 `crates/*` 和回归测试为准。
---

## 项目状态

- Phase 1 到 Phase 6 已稳定落地。
- Phase 7 正在收口 async/runtime/build 的最小可用子集。
- Phase 8 正在把 package/workspace 和真实项目工作流做实。
- 入口文档只保留结论；详细切片留在 `docs/plans/`、实现代码和测试里。

## 建议阅读顺序

1. [当前支持基线](/roadmap/current-supported-surface)
2. [安装与版本配套](/getting-started/install)
3. [开发计划](/roadmap/development-plan)
4. [阶段总览](/roadmap/phase-progress)
5. [编译器入门](/getting-started/compiler-primer)
6. [VSCode 插件](/getting-started/vscode-extension)

## 开发文档约定

- 入口页只写当前结论、优先级和边界。
- 长流水账不再进入 README、首页、路线页和支持页。
- 详细实现细节以架构文档、设计稿、测试和提交历史承载。
