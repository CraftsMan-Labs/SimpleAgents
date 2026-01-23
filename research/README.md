# SimpleAgents Research Documentation

This folder contains comprehensive research and analysis that informed the design of SimpleAgents - a Rust-based LLM gateway combining LiteLLM's multi-provider capabilities with BAML's response healing features.

---

## Research Documents

### 1. **implementation-plan.md**
**Comprehensive implementation plan for SimpleAgents**

**Contents**:
- Complete architecture design
- Workspace structure (9 Rust crates)
- Core components (Provider abstraction, Response healing, Routing, Caching)
- FFI layer and language bindings strategy
- 12-week implementation timeline
- Testing and verification strategy
- Success criteria and trade-offs

**When to read**: Start here to understand the overall project architecture and implementation approach.

---

### 2. **litellm-analysis.md**
**Deep dive into LiteLLM's architecture**

**Key Findings**:
- Two-tier architecture (SDK + Proxy)
- Provider abstraction via transformation pattern
- Router with 5 different strategies (round-robin, latency-based, cost-based, etc.)
- HTTP handler with connection pooling
- Multiple cache backends (in-memory, Redis, disk, S3)
- Comprehensive error hierarchy
- ~24,500 lines of core Python code

**Insights for SimpleAgents**:
- Keep provider abstraction pattern (adapt to Rust traits)
- Simplify routing for MVP (start with round-robin)
- Focus on quality over quantity for providers
- Configuration-driven design is critical
- Error handling needs to be provider-aware

**When to read**: When implementing provider abstraction, routing, or caching features.

---

### 3. **baml-analysis.md**
**Analysis of BAML's response healing system**

**Key Findings**:
- "Jsonish" parser: incremental state machine that handles malformed JSON
- Schema-aligned coercion with fuzzy matching
- Flag system tracks every transformation
- Confidence scoring (0.0-1.0 based on # of fixes)
- Streaming support with partial types
- Works with all LLMs (not just those with tool calling)

**Response Healing Features**:
- Markdown wrapping removal
- Trailing comma fixes
- Single/double quote normalization
- Unquoted key handling
- Auto-completion of partial structures
- Field matching (exact, case-insensitive, fuzzy)
- Type coercion (string → int, float → int, etc.)
- Union resolution with scoring

**Insights for SimpleAgents**:
- Flexible JSON parser is the core differentiator
- Flag transparency builds user trust
- Confidence scoring lets users set thresholds
- Streaming requires `Option<T>` partial types
- Union resolution can be expensive (optimize with hints)

**When to read**: When implementing the healing system or JSON parser.

---

## Implementation Priority

Based on the research, the recommended implementation order:

### Phase 1: Foundation (Week 1-2)
**Read**: implementation-plan.md (Foundation section)
- Setup workspace structure
- Define core types and traits
- HTTP client wrapper
- Configuration system

### Phase 2: Provider Abstraction (Week 3-4)
**Read**: litellm-analysis.md (Provider Abstraction Pattern section)
- Implement Provider trait
- OpenAI provider (validates architecture)
- Anthropic provider
- Basic retry logic

### Phase 3: Response Healing (Week 5-6)
**Read**: baml-analysis.md (full document)
- Jsonish parser with state machine
- Coercion engine
- Flag system and scoring
- Streaming parser with partial types
- Streaming annotations (`@@stream.not_null`, `@@stream.done`)

### Phase 4: Routing & Reliability (Week 7)
**Read**: litellm-analysis.md (Router System section)
- Round-robin router
- Fallback chain
- Enhanced retry logic
- In-memory cache

### Phase 5: API & Bindings (Week 8-10)
**Read**: implementation-plan.md (FFI Layer, Language Bindings sections)
- Core client API
- FFI layer (C-compatible)
- Python bindings (PyO3)

### Phase 6: CLI & Polish (Week 11-12)
**Read**: implementation-plan.md (CLI Tool section)
- CLI commands
- Documentation
- CI/CD

---

## Key Decisions & Trade-offs

### Architectural Decisions

1. **Provider Abstraction**
   - **Decision**: Async trait with transformation methods
   - **Rationale**: Clean separation, testable, extensible
   - **Source**: litellm-analysis.md

2. **Response Healing Default**
   - **Decision**: Lenient by default, strict mode optional
   - **Rationale**: Core value prop is handling malformed outputs
   - **Source**: baml-analysis.md

3. **Language Bindings**
   - **Decision**: FFI bindings (C-compatible + language wrappers)
   - **Rationale**: Single source of truth, faster to build
   - **Source**: implementation-plan.md

4. **Routing Strategy**
   - **Decision**: Round-robin for MVP, pluggable architecture
   - **Rationale**: Simple, predictable, room to grow
   - **Source**: litellm-analysis.md

5. **Caching**
   - **Decision**: In-memory for MVP, Redis later
   - **Rationale**: Good enough for most use cases, simpler
   - **Source**: litellm-analysis.md

### Technology Choices

| Component | Technology | Rationale | Source |
|-----------|-----------|-----------|---------|
| Async Runtime | `tokio` | Industry standard, mature | implementation-plan.md |
| HTTP Client | `reqwest` | Async, connection pooling, streaming | litellm-analysis.md |
| Error Handling | `thiserror` | Clean, idiomatic Rust | implementation-plan.md |
| Python Bindings | `PyO3` | Native extension, async support | implementation-plan.md |
| Node Bindings | `napi-rs` | N-API stable, TypeScript generation | implementation-plan.md |
| CLI | `clap` | Feature-rich, derive macros | implementation-plan.md |
| Concurrency | `dashmap` | Lock-free, concurrent HashMap | implementation-plan.md |

---

## MVP Scope

### Must Have
- ✅ 3 providers (OpenAI, Anthropic, OpenRouter)
- ✅ Basic JSON healing (markdown, trailing commas, type coercion)
- ✅ Retry logic with exponential backoff
- ✅ Round-robin routing
- ✅ In-memory cache
- ✅ Completion API (sync + async, streaming)
- ✅ Streaming with partial types and annotations
- ✅ Schema derive macros with streaming support
- ✅ Python bindings (PyO3)
- ✅ CLI (`complete`, `chat` commands)

### Post-MVP
- Advanced coercion (fuzzy matching, union resolution)
- Redis cache
- Latency/cost-based routing
- Go and TypeScript bindings
- Advanced validation macros
- Metrics/observability

---

## Critical Files to Implement

Based on research, these are the most important files (in order):

1. **`crates/simple-agents-types/src/lib.rs`**
   - Foundation for entire project
   - Core types, traits, errors

2. **`crates/simple-agents-providers/src/openai.rs`**
   - First provider implementation
   - Validates provider abstraction design

3. **`crates/simple-agents-healing/src/parser.rs`**
   - Jsonish parser (state machine)
   - Core differentiator

4. **`crates/simple-agents-healing/src/streaming.rs`**
   - Streaming parser with partial types
   - Incremental JSON extraction

5. **`crates/simple-agents-macros/src/lib.rs`**
   - Schema derive macro
   - Partial type generation
   - Streaming annotations

6. **`crates/simple-agents-core/src/client.rs`**
   - Main client API
   - User-facing interface

7. **`Cargo.toml`**
   - Workspace setup
   - Dependency management

---

## Research Methodology

### LiteLLM Analysis
- Explored 115 provider implementations
- Analyzed ~24,500 lines of core code
- Studied router (8,334 lines), utils (8,930 lines), main (7,258 lines)
- Examined caching strategies across 4 backends
- Reviewed exception hierarchy and error handling

### BAML Analysis
- Deep dive into jsonish parser (~5,000 lines Rust)
- Analyzed coercion engine (~3,000 lines Rust)
- Studied flag system and confidence scoring
- Examined streaming implementation
- Reviewed multi-provider client system

### Total Research Coverage
- **LiteLLM**: 100+ providers, all core features
- **BAML**: Complete parsing/coercion pipeline
- **Combined insights**: 50,000+ lines of code analyzed

---

## Questions or Clarifications

If you need more details on any aspect:

1. **Provider Implementation**: See litellm-analysis.md sections on specific providers
2. **Response Healing**: See baml-analysis.md for step-by-step parsing flow
3. **Architecture Decisions**: See implementation-plan.md Trade-offs section
4. **Testing Strategy**: See implementation-plan.md Testing Strategy section
5. **Performance**: Both analyses include performance considerations

---

## Success Metrics

**MVP Complete When**:
- ✅ Developer can make API call in <10 lines of Rust
- ✅ Handles 90%+ of malformed JSON cases
- ✅ All tests pass (unit, integration, FFI)
- ✅ Documentation complete (README, examples, API docs)
- ✅ Python bindings work (sync + async)

**Example - Simple API Call**:
```rust
use simple_agents::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let client = SimpleAgentsClient::builder()
        .provider("openai")
        .api_key(env::var("OPENAI_API_KEY")?)
        .build()?;

    let response = client.completion()
        .model("gpt-4")
        .messages(vec![Message::user("Hello!")])
        .send()
        .await?;

    println!("{}", response.choices[0].message.content);
    Ok(())
}
```

---

## Next Steps

1. Read `implementation-plan.md` for overall architecture
2. Review `litellm-analysis.md` for provider patterns
3. Study `baml-analysis.md` for healing system details
4. Start implementation with foundation (Week 1-2)
5. Iterate based on learnings

**Estimated Timeline**: 12 weeks to MVP

---

## Repository Structure

```
SimpleAgents/
├── research/                          # ← You are here
│   ├── README.md                      # This file
│   ├── implementation-plan.md         # Overall architecture and plan
│   ├── litellm-analysis.md           # LiteLLM deep dive
│   └── baml-analysis.md              # BAML deep dive
├── crates/                            # Rust workspace (to be created)
│   ├── simple-agents-core/
│   ├── simple-agents-providers/
│   ├── simple-agents-healing/
│   ├── simple-agents-router/
│   ├── simple-agents-cache/
│   ├── simple-agents-types/
│   ├── simple-agents-ffi/
│   ├── simple-agents-macros/
│   └── simple-agents-cli/
├── bindings/                          # Language bindings (to be created)
│   ├── python/
│   ├── node/
│   └── go/
└── Cargo.toml                         # Workspace root (to be created)
```

---

**Research Completed**: 2026-01-15
**Total Research Documents**: 3 (implementation plan + 2 system analyses)
**Total Pages**: ~100+ pages of detailed documentation
**Ready for**: Implementation Phase 1 (Foundation)
