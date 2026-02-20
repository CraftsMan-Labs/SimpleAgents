import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'SimpleAgents',
  description: 'A modular, extensible agent framework for building intelligent AI systems',
  cleanUrls: true,
  lastUpdated: true,
  base: '/SimpleAgents/',

  themeConfig: {
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Start Here', link: '/QUICKSTART' },
      { text: 'Docs Map', link: '/DOCS_MAP' },
      { text: 'API', link: '/API' }
    ],

    sidebar: [
      {
        text: 'Getting Started',
        items: [
          { text: 'Quick Start', link: '/QUICKSTART' },
          { text: 'Usage Guide', link: '/USAGE' },
          { text: 'Docs Map', link: '/DOCS_MAP' }
        ]
      },
      {
        text: 'Language Bindings',
        items: [
          { text: 'Python', link: '/BINDINGS_PYTHON' },
          { text: 'Node.js / TypeScript', link: '/BINDINGS_NODE' },
          { text: 'Go', link: '/BINDINGS_GO' }
        ]
      },
      {
        text: 'Core Guides',
        items: [
          { text: 'Examples', link: '/EXAMPLES' },
          { text: 'YAML Workflow System', link: '/YAML_WORKFLOW_SYSTEM' },
          { text: 'Architecture', link: '/ARCHITECTURE' },
          { text: 'Rust Core Systems', link: '/RUST_CORE_SYSTEMS' },
          { text: 'Development Guide', link: '/DEVELOPMENT' }
        ]
      },
      {
        text: 'Reference',
        items: [
          { text: 'API Surface', link: '/API' },
          { text: 'Documentation Standards', link: '/DOCS_STANDARDS' }
        ]
      }
    ],

    outline: {
      level: [2, 3],
      label: 'On this page'
    },
    search: {
      provider: 'local'
    },
    editLink: {
      pattern: 'https://github.com/rishub/SimpleAgents/edit/main/docs/:path',
      text: 'Suggest changes to this page'
    },
    lastUpdated: {
      text: 'Last updated'
    },
    docFooter: {
      prev: 'Previous',
      next: 'Next'
    },
    socialLinks: [
      { icon: 'github', link: 'https://github.com/rishub/SimpleAgents' }
    ],
    footer: {
      message: 'Released under the Apache-2.0 License.',
      copyright: 'Copyright © SimpleAgents contributors'
    }
  },
  markdown: {
    lineNumbers: true
  },
  sitemap: {
    hostname: 'https://rishub.github.io/SimpleAgents/'
  },
  head: [
    ['meta', { name: 'theme-color', content: '#2563eb' }],
    ['meta', { name: 'keywords', content: 'simpleagents, rust, llm, ai agents, docs' }]
  ],
  srcExclude: [
    '**/node_modules/**'
  ]
})
