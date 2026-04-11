import { defineConfig } from "vitepress";

export default defineConfig({
  lang: "zh-CN",
  title: "Qlang",
  description: "一个面向 LLVM、强调安全、开发体验与多语言互操作的编译型语言预研文档站。",
  lastUpdated: true,
  themeConfig: {
    nav: [
      { text: "愿景", link: "/vision" },
      { text: "入门", link: "/getting-started/compiler-primer" },
      { text: "阶段总览", link: "/roadmap/phase-progress" },
      { text: "语言设计", link: "/design/principles" },
      { text: "架构", link: "/architecture/compiler-pipeline" },
      { text: "路线图", link: "/roadmap/development-plan" },
      { text: "设计稿", link: "/plans/" }
    ],
    sidebar: [
      {
        text: "总览",
        items: [
          { text: "首页", link: "/" },
          { text: "项目愿景", link: "/vision" },
          { text: "P1-P6 阶段总览", link: "/roadmap/phase-progress" }
        ]
      },
      {
        text: "入门",
        items: [
          {
            text: "编译器、术语与生态入门",
            link: "/getting-started/compiler-primer"
          }
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
          { text: "P1-P6 阶段总览", link: "/roadmap/phase-progress" },
          { text: "开发计划", link: "/roadmap/development-plan" }
        ]
      },
      {
        text: "阶段设计稿",
        items: [
          { text: "设计稿总览", link: "/plans/" },
          { text: "Phase 0 设计冻结", link: "/plans/phase-0-design-freeze" },
          { text: "Phase 2 语义与类型检查", link: "/plans/phase-2-semantic-and-typing" },
          { text: "Phase 3 MIR 与所有权", link: "/plans/phase-3-mir-and-ownership" },
          { text: "Phase 4 后端与产物", link: "/plans/phase-4-backend-and-artifacts" },
          { text: "Phase 5 C FFI 与宿主互操作", link: "/plans/phase-5-ffi-and-c-abi" },
          { text: "Phase 6 LSP 与编辑器语义", link: "/plans/phase-6-lsp-and-editor-experience" }
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
