import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'SimpleAgents',
  description: 'A modular, extensible agent framework for building intelligent AI systems',
  
  base: '/SimpleAgents/',
  
  themeConfig: {
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Quick Start', link: '/QUICKSTART' },
      { text: 'API', link: '/API' }
    ],

    sidebar: [
      {
        text: 'Getting Started',
        items: [
          { text: 'Quick Start', link: '/QUICKSTART' },
          { text: 'Usage', link: '/USAGE' }
        ]
      },
      {
        text: 'Development',
        items: [
          { text: 'Development Guide', link: '/DEVELOPMENT' },
          { text: 'Architecture', link: '/ARCHITECTURE' },
          { text: 'API Reference', link: '/API' }
        ]
      },
      {
        text: 'Resources',
        items: [
          { text: 'Examples', link: '/EXAMPLES' }
        ]
      }
    ],

    socialLinks: [
      { icon: 'github', link: 'https://github.com/rishub/SimpleAgents' }
    ]
  }
})
