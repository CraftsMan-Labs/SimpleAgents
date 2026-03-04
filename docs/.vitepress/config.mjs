import { defineConfig } from "vitepress";

export default defineConfig({
  title: "SimpleAgents",
  description:
    "A modular, extensible agent framework for building intelligent AI systems",
  cleanUrls: true,
  lastUpdated: true,
  base: "/",

  themeConfig: {
    nav: [
      { text: "Home", link: "/" },
      { text: "Start Here", link: "/QUICKSTART" },
      { text: "Docs Map", link: "/DOCS_MAP" },
      { text: "API", link: "/API" },
    ],

    sidebar: [
      {
        text: "Start Here",
        items: [
          { text: "Quick Start", link: "/QUICKSTART" },
          { text: "Usage Guide", link: "/USAGE" },
          { text: "Docs Map", link: "/DOCS_MAP" },
          { text: "Troubleshooting", link: "/TROUBLESHOOTING" },
        ],
      },
      {
        text: "Build and Operate",
        items: [
          { text: "Examples", link: "/EXAMPLES" },
          { text: "Development Guide", link: "/DEVELOPMENT" },
          { text: "Release Checklist", link: "/RELEASE_CHECKLIST" },
        ],
      },
      {
        text: "Workflow System",
        items: [
          { text: "YAML Workflow System", link: "/YAML_WORKFLOW_SYSTEM" },
          { text: "Workflow Capability Contract", link: "/WORKFLOW_CAPABILITY_CONTRACT" },
          { text: "Workflow Debugging UX", link: "/WORKFLOW_DEBUGGING" },
          { text: "Workflow Performance", link: "/WORKFLOW_PERFORMANCE" },
          { text: "Workflow Security", link: "/WORKFLOW_SECURITY" },
          { text: "Workflow DSL Migration Cookbook", link: "/WORKFLOW_DSL_MIGRATION_COOKBOOK" },
        ],
      },
      {
        text: "Language Bindings",
        items: [
          { text: "Python", link: "/BINDINGS_PYTHON" },
          { text: "Node.js / TypeScript", link: "/BINDINGS_NODE" },
          { text: "Go", link: "/BINDINGS_GO" },
          { text: "Capability Matrix", link: "/CAPABILITY_MATRIX" },
        ],
      },
      {
        text: "Architecture and Internals",
        items: [
          { text: "Architecture", link: "/ARCHITECTURE" },
          { text: "Rust Core Systems", link: "/RUST_CORE_SYSTEMS" },
          { text: "Tracing Architecture", link: "/TRACING_ARCHITECTURE" },
        ],
      },
      {
        text: "Reference",
        items: [
          { text: "API Surface", link: "/API" },
          { text: "Documentation Standards", link: "/DOCS_STANDARDS" },
        ],
      },
    ],

    outline: {
      level: [2, 3],
      label: "On this page",
    },
    search: {
      provider: "local",
    },
    editLink: {
      pattern:
        "https://github.com/CraftsMan-Labs/SimpleAgents/edit/main/docs/:path",
      text: "Suggest changes to this page",
    },
    lastUpdated: {
      text: "Last updated",
    },
    docFooter: {
      prev: "Previous",
      next: "Next",
    },
    socialLinks: [
      {
        icon: "github",
        link: "https://github.com/CraftsMan-Labs/SimpleAgents",
      },
    ],
    footer: {
      message: "Released under the Apache-2.0 License.",
      copyright: "Copyright © SimpleAgents contributors",
    },
  },
  markdown: {
    lineNumbers: true,
  },
  sitemap: {
    hostname: "https://docs.simpleagents.craftsmanlabs.net/",
  },
  head: [
    ["meta", { name: "theme-color", content: "#2563eb" }],
    [
      "meta",
      { name: "keywords", content: "simpleagents, rust, llm, ai agents, docs" },
    ],
  ],
  srcExclude: ["**/node_modules/**"],
});
