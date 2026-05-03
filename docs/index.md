---
layout: home

hero:
  name: Qlang
  text: 编译型系统语言
  tagline: 当前主线是把源码构建、本地项目、stdlib、LSP 和工具链闭环做稳。
  actions:
    - theme: brand
      text: 当前支持基线
      link: /roadmap/current-supported-surface
    - theme: alt
      text: 开发计划
      link: /roadmap/development-plan

features:
  - title: 可用方式
    details: 目前使用 source build、cargo install 和本地 VSIX。还没有 release、Marketplace 或 registry。
  - title: 当前重点
    details: project/workspace、dependency-aware build、stdlib generics、workspace LSP、安装与分发准备。
  - title: 当前边界
    details: 跨包执行和 workspace tooling 仍是保守切片。具体能力以当前支持基线为准。
  - title: 真相源
    details: 如果文档与实现冲突，以 crates、stdlib、回归测试和 smoke 为准。
---

## 阅读顺序

1. [当前支持基线](/roadmap/current-supported-surface)
2. [安装与版本配套](/getting-started/install)
3. [开发计划](/roadmap/development-plan)
4. [阶段总览](/roadmap/phase-progress)
5. [VSCode 插件](/getting-started/vscode-extension)
6. [编译器入门](/getting-started/compiler-primer)

## 文档规则

- 入口页只写当前事实、边界和下一步顺序。
- 历史推导放进 `docs/plans/archive/` 或提交历史。
- 用户可见能力必须同时有实现、回归和文档入口。
