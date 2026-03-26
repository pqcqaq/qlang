import { defineConfig } from "vitepress";

export default defineConfig({
  lang: "zh-CN",
  title: "Qlang",
  description: "一个面向 LLVM、强调安全、开发体验与多语言互操作的编译型语言预研文档站。",
  lastUpdated: true,
  themeConfig: {
    nav: [
      { text: "愿景", link: "/vision" },
      { text: "阶段总览", link: "/roadmap/phase-progress" },
      { text: "语言设计", link: "/design/principles" },
      { text: "架构", link: "/architecture/compiler-pipeline" },
      { text: "路线图", link: "/roadmap/development-plan" }
    ],
    sidebar: [
      {
        text: "总览",
        items: [
          { text: "首页", link: "/" },
          { text: "项目愿景", link: "/vision" },
          { text: "P1-P4 阶段总览", link: "/roadmap/phase-progress" }
        ]
      },
      {
        text: "语言设计",
        items: [
          { text: "设计原则", link: "/design/principles" },
          { text: "跨语言借鉴", link: "/design/influences" },
          { text: "语法草案", link: "/design/syntax" },
          { text: "类型系统", link: "/design/type-system" },
          { text: "运行时与内存", link: "/design/runtime-memory" },
          { text: "并发模型", link: "/design/concurrency" },
          { text: "互操作设计", link: "/design/interop" }
        ]
      },
      {
        text: "架构",
        items: [
          { text: "编译器流水线", link: "/architecture/compiler-pipeline" },
          { text: "实现算法与分层边界", link: "/architecture/implementation-algorithms" },
          { text: "工具链设计", link: "/architecture/toolchain" },
          { text: "仓库目录结构", link: "/architecture/repository-structure" }
        ]
      },
      {
        text: "路线图",
        items: [
          { text: "功能清单", link: "/roadmap/feature-list" },
          { text: "P1-P4 阶段总览", link: "/roadmap/phase-progress" },
          { text: "开发计划", link: "/roadmap/development-plan" }
        ]
      },
      {
        text: "预研沉淀",
        items: [
          {
            text: "2026-03-25 设计稿",
            link: "/plans/2026-03-25-qlang-design"
          },
          {
            text: "2026-03-26 P3.1 MIR 设计",
            link: "/plans/2026-03-26-phase3-mir-foundation"
          },
          {
            text: "2026-03-26 P3.2 Ownership 设计",
            link: "/plans/2026-03-26-phase3-ownership-facts"
          },
          {
            text: "2026-03-26 P3.3 Cleanup Ownership",
            link: "/plans/2026-03-26-phase3-cleanup-aware-ownership"
          },
          {
            text: "2026-03-26 P3.3b Move Closure Capture",
            link: "/plans/2026-03-26-phase3-move-closure-captures"
          },
          {
            text: "2026-03-26 P3.3c MIR Closure Captures",
            link: "/plans/2026-03-26-phase3-explicit-closure-captures"
          },
          {
            text: "2026-03-26 P3.3d Closure Escape Facts",
            link: "/plans/2026-03-26-phase3-closure-escape-facts"
          },
          {
            text: "2026-03-26 P5 Extern C Export",
            link: "/plans/2026-03-26-phase5-extern-c-definition-exports"
          },
          {
            text: "2026-03-26 P5 FFI Harness",
            link: "/plans/2026-03-26-phase5-ffi-integration-harness"
          },
          {
            text: "2026-03-26 P5 C Header Generation",
            link: "/plans/2026-03-26-phase5-c-header-generation"
          },
          {
            text: "2026-03-26 P5 Import Surface Header Projection",
            link: "/plans/2026-03-26-phase5-import-surface-header-projection"
          },
          {
            text: "2026-03-26 P5 Build Sidecar Header",
            link: "/plans/2026-03-26-phase5-build-sidecar-c-header"
          },
          {
            text: "2026-03-26 P5 Imported Host FFI Harness",
            link: "/plans/2026-03-26-phase5-imported-host-ffi-harness"
          },
          {
            text: "2026-03-26 P6 Find References",
            link: "/plans/2026-03-26-phase6-find-references-query-surface"
          },
          {
            text: "2026-03-26 P5 Dynamic Library Emission",
            link: "/plans/2026-03-26-phase5-dynamic-library-emission"
          },
          {
            text: "2026-03-26 P5 Shared Library FFI Harness",
            link: "/plans/2026-03-26-phase5-shared-library-ffi-harness"
          }
        ]
      }
    ],
    search: {
      provider: "local"
    },
    outline: {
      level: [2, 3]
    },
    footer: {
      message: "Qlang research repository",
      copyright: "Copyright 2026"
    }
  }
});
