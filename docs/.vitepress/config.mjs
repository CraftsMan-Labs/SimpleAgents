import { defineConfig } from "vitepress";

export default defineConfig({
  title: "SimpleAgents",
  description:
    "Every agentic SaaS is a config. Define your AI product as a YAML workflow, run in Python or TypeScript.",
  cleanUrls: true,
  lastUpdated: true,
  base: "/",

  themeConfig: {
    nav: [
      { text: "Home", link: "/" },
      { text: "Quickstart", link: "/WORKFLOW_QUICKSTART" },
      { text: "Examples", link: "/EXAMPLES" },
      { text: "Docs Map", link: "/DOCS_MAP" },
    ],

    sidebar: [
      {
        text: "Get Started",
        items: [
          { text: "Workflow Quickstart", link: "/WORKFLOW_QUICKSTART" },
          { text: "Examples", link: "/EXAMPLES" },
          { text: "Troubleshooting", link: "/TROUBLESHOOTING" },
        ],
      },
      {
        text: "YAML Workflows",
        items: [
          { text: "YAML Workflow System", link: "/YAML_WORKFLOW_SYSTEM" },
        ],
      },
      {
        text: "Language Bindings",
        items: [
          { text: "Python", link: "/BINDINGS_PYTHON" },
          { text: "Node.js / TypeScript", link: "/BINDINGS_NODE" },
          { text: "Browser / WASM", link: "/BINDINGS_WASM" },
        ],
      },
      {
        text: "Observability",
        items: [
          { text: "Tracing & Observability", link: "/TRACING_ARCHITECTURE" },
        ],
      },
      {
        text: "Rust",
        items: [
          { text: "Rust Quick Start", link: "/QUICKSTART" },
          { text: "Rust Usage Guide", link: "/USAGE" },
        ],
      },
      {
        text: "Contributing",
        items: [
          { text: "Development Guide", link: "/DEVELOPMENT" },
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
      { name: "keywords", content: "simpleagents, yaml, llm, workflow, python, typescript, rust" },
    ],
  ],
  srcExclude: ["**/node_modules/**"],
});
