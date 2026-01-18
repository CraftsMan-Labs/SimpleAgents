# SimpleAgents Project TODO

> **Status**: Foundation Phase Complete ✅
> **Current Version**: 0.1.0
> **Last Updated**: 2026-01-16

This is the master TODO file for the entire SimpleAgents project. For detailed task breakdowns, see individual crate TODO files.

---

## 🎯 Project Vision

Build a **production-ready, extensible Rust framework** for LLM interactions with:
- Multi-provider support (OpenAI, Anthropic, etc.)
- Automatic failover and retry logic
- Response healing (fix malformed JSON from LLMs)
- Transparent coercion tracking
- Enterprise-grade security
- Full observability

---

## 📦 Crate Structure

```
SimpleAgents/
├── simple-agents-types      ✅ COMPLETE (Week 1-2)
├── simple-agents-providers   📅 TODO (Week 3-4)
├── simple-agents-healing     📅 TODO (Week 5-6)
├── simple-agents-router      📅 TODO (Week 7)
├── simple-agents-core        📅 TODO (Week 8)
├── simple-agents-cli         📅 TODO (Week 9-10)
└── simple-agents-py          📅 OPTIONAL (Week 11-12)
```

---

## ✅ COMPLETED WORK

### Phase 1: Foundation - `simple-agents-types` ✅

**Status**: 100% Complete
**Location**: `crates/simple-agents-types/`
**Duration**: Week 1-2

#### What Was Built
- ✅ Complete type system for LLM interactions
- ✅ 12 modules with full implementations
- ✅ 114 passing tests (83 unit + 11 integration + 20 doctests)
- ✅ Zero clippy warnings
- ✅ Full documentation with examples
- ✅ Security-first design (API keys never logged)
- ✅ Transparency tracking (all coercions recorded)

#### Key Achievements
- 🔒 API keys never leak (always show `[REDACTED]`)
- 📊 Full transparency via `CoercionFlag`
- 🧪 Comprehensive testing (114 tests)
- 📚 Complete documentation
- ⚡ Zero-cost abstractions
- 🔧 All types are Send + Sync
- 🎨 Clean builder patterns
- ✨ Production-ready code quality

**See**: `crates/simple-agents-types/TODO.md` for detailed task list

---

## 📋 UPCOMING WORK

### Phase 2: Providers (Week 3-4) 📅 NEXT

**Goal**: Implement actual LLM provider integrations
**Crate**: `simple-agents-providers`

#### Major Tasks
- [ ] Set up providers crate structure
- [ ] **OpenAI Provider**
  - [ ] Request/response transformation
  - [ ] Streaming support
  - [ ] Function calling
  - [ ] Vision support
  - [ ] Error mapping
- [ ] **Anthropic Provider**
  - [ ] Claude API integration
  - [ ] Streaming support
  - [ ] Error mapping
- [ ] Integration tests
- [ ] Documentation

---

### Phase 3: Healing (Week 5-6) 📅

**Goal**: Fix malformed JSON responses from LLMs
**Crate**: `simple-agents-healing`

#### Major Tasks
- [ ] JSON healing parser
- [ ] Type coercion engine
- [ ] Fuzzy field matching
- [ ] Confidence scoring
- [ ] Tests with real-world malformed JSON

---

### Phase 4: Router (Week 7) 📅

**Goal**: Implement retry, fallback, and routing logic
**Crate**: `simple-agents-router`

#### Major Tasks
- [ ] Routing strategies (priority, round-robin, latency-based)
- [ ] Retry logic with exponential backoff
- [ ] Provider fallback chain
- [ ] Circuit breaker pattern
- [ ] Health tracking

---

### Phase 5: Core (Week 8) 📅

**Goal**: Unified client API bringing everything together
**Crate**: `simple-agents-core`

#### Major Tasks
- [ ] `SimpleAgentsClient` main API
- [ ] Provider management
- [ ] Cache integration
- [ ] Router integration
- [ ] Healing integration
- [ ] Middleware system
- [ ] End-to-end integration tests

---

### Phase 6: CLI & Tools (Week 9-10) 📅

**Goal**: Command-line tool for testing and debugging
**Crate**: `simple-agents-cli`

---

### Phase 7: Python Bindings (Week 11-12) 📅 OPTIONAL

**Goal**: Python library for SimpleAgents
**Crate**: `simple-agents-py`

---

## 📊 Progress Tracking

| Phase | Status | Progress | ETA |
|-------|--------|----------|-----|
| Phase 1: Foundation | ✅ Complete | 100% | Done |
| Phase 2: Providers | 📅 Planned | 0% | Week 3-4 |
| Phase 3: Healing | 📅 Planned | 0% | Week 5-6 |
| Phase 4: Router | 📅 Planned | 0% | Week 7 |
| Phase 5: Core | 📅 Planned | 0% | Week 8 |
| Phase 6: CLI | 📅 Planned | 0% | Week 9-10 |

**Overall Progress**: 1/6 core phases complete (17%)

---

## 🚀 What Works Now

```rust
use simple_agents_types::prelude::*;

// Build requests
let request = CompletionRequest::builder()
    .model("gpt-4")
    .message(Message::user("Hello!"))
    .temperature(0.7)
    .build()?;

// Secure API keys
let key = ApiKey::new("sk-...")?;
// Never logged: ApiKey([REDACTED])

// Track coercions
let result = CoercionResult::new(data)
    .with_flag(CoercionFlag::StrippedMarkdown);
```

---

## 🎯 Next Milestone

**Phase 2: Provider Integration** (Week 3-4)

After this, you'll be able to actually call LLM APIs!

---

**Next Action**: Begin Phase 2 - Implement OpenAI Provider

For detailed task breakdowns, see:
- `crates/simple-agents-types/TODO.md` - Foundation (complete)
- More crate TODOs coming as we build them
