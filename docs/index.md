---
layout: home

hero:
  name: SimpleAgents
  text: Every agentic SaaS is a config
  tagline: Define your entire AI product as a YAML workflow. Run it in Python or TypeScript. Ship today.
  actions:
    - theme: brand
      text: Get Started in 5 Minutes
      link: /WORKFLOW_QUICKSTART
    - theme: alt
      text: Examples
      link: /EXAMPLES
    - theme: alt
      text: View on GitHub
      link: https://github.com/CraftsMan-Labs/SimpleAgents

features:
  - title: Your product is a YAML
    details: Classification, routing, extraction, generation -- describe your workflow as a graph config. No framework. No lock-in.
  - title: 10 lines to production
    details: pip install or npm install. Point at your YAML. Run. Stream. Done.
  - title: Multimodal out of the box
    details: Text, images, streaming, structured JSON -- all from the same YAML config.
  - title: Observability included
    details: Langfuse or Jaeger with one env block. OpenTelemetry native.
  - title: Self-healing JSON
    details: LLM outputs truncated JSON? Auto-healed. Schema coercion built-in.
  - title: Rust-fast, everywhere
    details: Rust engine. Python and TypeScript bindings. WASM for the browser. Same config runs everywhere.
---

## The Idea

Every agentic SaaS -- email classifiers, document processors, intake systems, interview bots, support agents -- is fundamentally the same thing: an LLM workflow with structured outputs and deterministic routing.

SimpleAgents lets you define that workflow as a YAML config and run it with a single command. No framework to learn. No abstractions to fight. Just a graph of nodes and edges.

## Choose Your Path

- **I want my first working workflow** -> [Workflow Quickstart](/WORKFLOW_QUICKSTART)
- **I want runnable examples** -> [Examples](/EXAMPLES)
- **I want Python API details** -> [Python Binding](/BINDINGS_PYTHON)
- **I want TypeScript / Node API details** -> [Node.js Binding](/BINDINGS_NODE)
- **I want to understand YAML routing and workers** -> [YAML Workflow System](/YAML_WORKFLOW_SYSTEM)
- **I want workflow evals (golden outputs + judge in code)** -> [Workflow evals](/YAML_WORKFLOW_SYSTEM#workflow-evals) and language bindings ([Python](/BINDINGS_PYTHON), [Node](/BINDINGS_NODE))
- **I want human-in-the-loop steps** -> [`human_input` in YAML](/YAML_WORKFLOW_SYSTEM#human_input), [Workflow Quickstart](/WORKFLOW_QUICKSTART) (Python), [Node.js Binding](/BINDINGS_NODE) (TypeScript)
- **I want tracing with Langfuse / Jaeger** -> [Tracing & Observability](/TRACING_ARCHITECTURE)
- **I want to use Rust directly** -> [Rust Quick Start](/QUICKSTART)
- **I want to contribute** -> [Development Guide](/DEVELOPMENT)

## Use It When

- You want structured LLM workflows with deterministic routing
- You want one YAML config that runs in Python or TypeScript
- You want custom worker steps and tracing without building a framework around it

If you only need one prompt call and no workflow, start with the language bindings first.
