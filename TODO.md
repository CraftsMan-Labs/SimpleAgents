# SimpleAgents Project TODO

> **Status**: Phase 3 (Response Healing) - Nearly Complete 🚀
> **Current Version**: 0.1.0
> **Last Updated**: 2026-01-23
> **Overall Progress**: 43% (2.95/7 phases complete)

This is the **single source of truth** for all project tasks and progress tracking.

---

## 📊 Progress Overview

| Phase | Status | Progress | Tests | Completion Date |
|-------|--------|----------|-------|----------------|
| **Research** | ✅ Complete | 100% | 4 documents | Jan 15-23, 2026 |
| **Phase 1: Foundation** | ✅ Complete | 100% | 114 tests | Week 1-2 |
| **Phase 2: Providers** | ✅ Complete | 100% | 203 tests | Week 3-4, Jan 23 |
| **Phase 3: Healing** | 🚧 In Progress | 95% | 150 tests | Week 5-6 (Day 1-8 done) |
| **Phase 4: Router** | 📅 Planned | 0% | - | Week 7 |
| **Phase 5: Core** | 📅 Planned | 0% | - | Week 8 |
| **Phase 6-7: CLI & Bindings** | 📅 Planned | 0% | - | Week 9-12 |

**Current Focus**: Phase 3 - Response Healing (Parser + Coercion + Streaming + Annotations done, Integration next)

---

## 🎯 Project Vision

Build a **production-ready, extensible Rust framework** for LLM interactions with:
- Multi-provider support (OpenAI, Anthropic, OpenRouter) ✅
- Automatic failover and retry logic ✅
- Response healing (fix malformed JSON from LLMs) 📅 NEXT
- Transparent coercion tracking ✅
- Enterprise-grade security ✅
- Full observability 🚧

---

## ✅ Completed Phases

### Research Phase ✅ (January 15-23, 2026)

Comprehensive analysis of LiteLLM (24,500+ lines) and BAML (20,000+ lines) completed.

**Deliverables**:
- `research/litellm-analysis.md` - 115 providers, routing strategies, reliability patterns
- `research/baml-analysis.md` - Jsonish parser, coercion engine, streaming
- `research/baml-moat.md` - Competitive analysis
- `research/implementation-plan.md` - 12-week roadmap
- `research/mvp-scope-update.md` - MVP scope refinement

**See**: [research/README.md](research/README.md) for complete documentation.

---

### Phase 1: Foundation ✅ (Week 1-2)

**Crate**: `simple-agents-types`
**Status**: Production-ready
**Tests**: 114 passing (83 unit + 11 integration + 20 doctests)

**What Was Built**:
- Complete type system (Message, CompletionRequest, CompletionResponse)
- API key handling with security (never logged, constant-time comparison)
- Builder patterns for ergonomic APIs
- CoercionFlag system for transparency tracking
- Comprehensive error hierarchy

**Report**: See `crates/simple-agents-types/TODO.md` for detailed completion status.

---

### Phase 2: Provider Integration ✅ (Week 3-4, completed Jan 23)

**Crates**: `simple-agents-providers`, `simple-agents-cache`
**Status**: Production-ready
**Tests**: 203 passing (171 unit/integration + 32 doctests) + 16 ignored

**What Was Built**:
- **Providers**: OpenAI, Anthropic, OpenRouter with 3-phase architecture
- **Reliability**: Retry with exponential backoff + jitter, rate limiting
- **Caching**: InMemoryCache with LRU eviction, TTL, Blake3 hashing
- **Infrastructure**: HTTP/2 connection pooling, async traits, error hierarchy
- **Examples**: 7 complete examples demonstrating all features

**Report**: See [PHASE2_COMPLETION_REPORT.md](PHASE2_COMPLETION_REPORT.md) for comprehensive details.

---

## 📋 Current Work

### Phase 3: Response Healing 🚧 IN PROGRESS (Week 5-6)

**Goal**: Implement BAML-inspired JSON healing for malformed LLM outputs
**Crate**: `simple-agents-healing`
**Timeline**: Estimated 2 weeks (Jan 27 - Feb 7, 2026)
**Reference**: [research/baml-analysis.md](research/baml-analysis.md)
**Started**: Jan 23, 2026
**Status**: Day 1 Complete - 35 tests passing, 0 clippy warnings

#### Major Components

**1. Jsonish Parser** (~5,000 lines to port)
- Strip & Fix phase: Remove markdown, fix commas, normalize quotes
- Standard Parse: Try serde_json (fast path)
- Lenient Parse: Character-by-character state machine
- Handle incomplete JSON, unquoted keys, mixed quotes

**2. Type Coercion Engine** (~3,000 lines to port)
- Type coercion: string→int, float→int, with confidence scoring
- Fuzzy field matching: case-insensitive, snake_case ↔ camelCase
- Union resolution with best-match selection
- Default value injection for missing optional fields

**3. Flag System & Confidence Scoring**
- Track every transformation (StrippedMarkdown, FixedTrailingComma, TypeCoercion, etc.)
- Confidence scoring: 1.0 = perfect, reduces with each fix
- Transparency: Users know exactly what was changed

**4. Streaming Parser with Partial Types**
- Incremental JSON parsing from incomplete buffers
- Partial value extraction: `Option<T>` for all fields
- Progressive emission during streaming

**5. Streaming Annotations**
- `@@stream.not_null` - Don't emit until non-null
- `@@stream.done` - Only emit when complete
- Field-level control over emission timing

#### Task Breakdown

**Week 5: Parser & Coercion**
- [x] Create `crates/simple-agents-healing` crate ✅ (Day 1)
- [x] Implement Jsonish parser (3-phase architecture) ✅ (Day 1-3)
  - [x] Strip & Fix phase (markdown, commas, quotes, BOM, control chars) ✅
  - [x] Standard Parse phase (serde_json fast path) ✅
  - [x] Lenient Parse phase (full state machine) ✅ (Day 2-3)
    - [x] Character-by-character state machine ✅
    - [x] All string delimiter types (", ', """, ```) ✅
    - [x] Comment support (// and /* */) ✅
    - [x] Auto-completion of unclosed structures ✅
    - [x] Unquoted key support ✅
    - [x] Escape sequence handling ✅
- [x] Add confidence scoring system ✅ (Day 1)
- [x] Add flag tracking system ✅ (Day 1)
- [x] Unit tests for parser (50+ tests) ✅ - 58/50+ tests passing (Day 1-3)
- [x] Implement type coercion engine ✅ (Day 4-5)
- [x] Implement fuzzy field matching ✅ (Day 4-5)
- [x] Unit tests for coercion (30+ tests) ✅ - 35/30+ tests passing (Day 4-5)

**Week 6: Streaming & Integration**
- [x] Implement streaming parser (incremental) ✅ (Day 6-7)
- [x] Generate partial types (derive macro) ✅ (Day 6)
  - [x] Create simple-agents-macros crate ✅
  - [x] Implement #[derive(PartialType)] ✅
  - [x] from_partial() and merge() methods ✅
- [x] Implement streaming annotations ✅ (Day 8)
  - [x] StreamAnnotation enum (Normal, NotNull, Done) ✅
  - [x] Integration with Field schema ✅
- [x] Add union resolution logic ✅ (Already done in Week 5, Day 4-5)
- [ ] Integration with providers (streaming support in Provider trait)
- [ ] Property-based tests (malformed JSON corpus)
- [ ] Examples demonstrating healing
- [ ] Documentation and benchmarks

#### Success Criteria

- [x] Can parse markdown-wrapped JSON: ` ```json {...} ``` ` ✅
- [x] Can fix trailing commas: `{"key": "value",}` ✅
- [x] Can handle incomplete JSON (unclosed strings, objects, arrays) ✅ (Day 2-3)
- [x] Can parse unquoted keys: `{key: "value"}` ✅ (Day 2-3)
- [x] Can handle comments (// and /* */) ✅ (Day 2-3)
- [x] Can parse multiple string delimiter types ✅ (Day 2-3)
- [x] Can coerce types: `"42"` → `42` (with flag) ✅ (Day 4-5)
- [x] Can match fuzzy fields: `userName` → `user_name` ✅ (Day 4-5)
- [x] Confidence scores accurate (0.0-1.0 range) ✅
- [x] All transformations tracked via flags ✅
- [x] Streaming emits partial types progressively ✅ (Week 6, Day 6-8)
- [x] 150+ tests passing (target was 80+) ✅ (Day 1-8)
- [x] Zero clippy warnings ✅
- [x] Documentation complete (parser + coercion + streaming done) ✅

**Dependencies**:
- Research phase ✅
- Phase 1 (types) ✅
- Phase 2 (providers) ✅

**Next After Phase 3**: Phase 4 (Router) - routing strategies, fallback chains

---

## 🚀 Upcoming Work

### Phase 4: Router & Reliability (Week 7)

**Goal**: Implement routing strategies and fallback chains
**Crate**: `simple-agents-router`

**Major Tasks**:
- [ ] Round-robin routing strategy
- [ ] Latency-based routing
- [ ] Cost-based routing
- [ ] Fallback chains (provider → provider failover)
- [ ] Circuit breaker pattern
- [ ] Health tracking and monitoring
- [ ] Integration with retry logic
- [ ] Examples and tests

**Reference**: [research/litellm-analysis.md](research/litellm-analysis.md) - Routing section

---

### Phase 5: Core API (Week 8)

**Goal**: Unified client API bringing everything together
**Crate**: `simple-agents-core`

**Major Tasks**:
- [ ] `SimpleAgentsClient` main API
- [ ] Provider management and registration
- [ ] Cache integration (transparent)
- [ ] Router integration (automatic)
- [ ] Healing integration (automatic)
- [ ] Middleware system (logging, metrics, tracing)
- [ ] Builder pattern for configuration
- [ ] End-to-end integration tests
- [ ] Examples and documentation

---

### Phase 6: CLI & Tools (Week 9-10)

**Goal**: Command-line tool for testing and debugging
**Crate**: `simple-agents-cli`

**Major Tasks**:
- [ ] `complete` command - single completion
- [ ] `chat` command - interactive chat
- [ ] `benchmark` command - performance testing
- [ ] `test-provider` command - provider health checks
- [ ] Configuration file support (TOML/YAML)
- [ ] Output formatting (JSON, markdown, plain)
- [ ] Examples and documentation

---

### Phase 7: Language Bindings (Week 11-12) - OPTIONAL

**Goal**: Python, TypeScript, Go bindings
**Crates**: `simple-agents-ffi`, `simple-agents-py`

**Major Tasks**:
- [ ] C FFI layer (safe, opaque pointers)
- [ ] Python bindings (PyO3)
- [ ] TypeScript bindings (napi-rs)
- [ ] Go bindings (cgo)
- [ ] FFI contract tests
- [ ] Language-specific examples
- [ ] Documentation per language

---

## 📊 Metrics

### Current Status

| Metric | Current | Target (MVP) |
|--------|---------|--------------|
| **Crates Created** | 5 of 9 | 7 of 9 |
| **Tests Written** | 366 passing | 300+ ✅ |
| **Providers** | 3 | 3 ✅ |
| **Code Analyzed** | 44,500+ lines | N/A |
| **Documentation** | 170+ pages | 200+ |
| **Clippy Warnings** | 0 | 0 ✅ |
| **Phase Completion** | 48% | 100% |

### Test Breakdown

| Crate | Unit Tests | Integration Tests | Doctests | Total |
|-------|------------|-------------------|----------|-------|
| simple-agents-cache | 13 | 0 | 2 | 15 |
| simple-agents-healing | 55 | 77 | 18 | 150 |
| simple-agents-macros | 13 | 0 | 2 (ignored) | 13 |
| simple-agents-providers | 57 (3 ignored) | 0 (5 ignored) | 9 | 66 |
| simple-agents-types | 90 | 11 | 21 (8 ignored) | 122 |
| **TOTAL** | **228** | **88** | **52** | **366** |

---

## 🎯 Next Immediate Actions

**Week 5, Day 2-3**: ✅ COMPLETED - Lenient Parser

✅ **Full state machine implementation**
   - Character-by-character parsing with state tracking
   - Handles incomplete JSON, unquoted keys
   - Tracks parser state (in_string, in_object, in_array, etc.)
   - Auto-closes unclosed structures
   - Handles escaped characters

✅ **String parsing complexity**
   - Multiple string delimiter support (", ', """, ```)
   - Escape sequence handling
   - Boolean and null literals
   - Comment support (// and /* */)

✅ **Additional parser tests**
   - 58 tests total (exceeded 50+ target)
   - 13 unit tests + 41 integration tests + 4 doctests
   - Zero clippy warnings

**Week 5, Day 4-5**: ✅ COMPLETED - Type Coercion Engine

✅ **Coercion engine implementation**
   - String → Number coercion with confidence scoring
   - Full Jaro-Winkler fuzzy field matching
   - Five-tier field matching (exact, alias, case, snake/camel, fuzzy)
   - Default value injection
   - Union resolution with best-match selection
   - Array and nested object coercion

✅ **Coercion tests**
   - 35 unit tests (exceeded 30+ target)
   - 13 coercion-specific tests + 10 string utils + 5 schema + 7 integration
   - Zero clippy warnings
   - Full API documentation

**Week 6, Day 6-8**: ✅ COMPLETED - Streaming & Partial Types

✅ **Proc macro crate created**
   - Created simple-agents-macros crate
   - Implemented #[derive(PartialType)] macro
   - Generates partial types with Option<T> for all fields
   - from_partial() and merge() methods
   - 13 tests passing, zero warnings

✅ **Streaming parser implementation**
   - StreamingParser with incremental parsing
   - PartialExtractor for progressive value extraction
   - Handles incomplete JSON buffers
   - Works with existing healing parser
   - 22 streaming integration tests

✅ **Streaming annotations**
   - StreamAnnotation enum (Normal, NotNull, Done)
   - Integration with Field schema system
   - with_stream_annotation() builder method
   - Full serde support
   - 14 annotation tests

✅ **Test expansion**
   - 150 total tests in simple-agents-healing (up from 93)
   - 366 total tests across all crates (up from 296)
   - Zero clippy warnings maintained
   - Full documentation

**Week 6, Day 9+**: Integration & Completion (NEXT)

1. **Provider streaming integration** (2-3 hours)
   - [x] Add execute_stream() to Provider trait ✅ (Already exists)
   - [x] Implement for OpenAI ✅ (Already exists)
   - [x] Implement for Anthropic ✅ (Just completed)
   - [x] Implement for OpenRouter ✅ (Already exists)
   - [x] Return Stream<CompletionChunk> ✅
   - [x] Backpressure and error handling ✅

2. **Property-based tests** (1-2 hours)
   - Use proptest for fuzzing
   - Verify parser never panics on arbitrary input
   - 10+ property tests

3. **Examples & documentation** (2-3 hours)
   - Streaming examples with partial types
   - Streaming with annotations example
   - Provider streaming example
   - Benchmarks for parser performance

**End of Week 6 Target**:
- ✅ Streaming parser working (passes 20+ tests) - 22/20 DONE ✅
- ✅ Partial types generated via macro - DONE ✅
- ✅ Streaming annotations implemented - DONE ✅
- [ ] Provider streaming support (execute_stream)
- [ ] Property-based tests (10+)
- [ ] Examples and benchmarks

**Status**: Week 6 is 75% complete. Phase 3 now 95% complete.

---

## 🔗 Key References

### For Phase 3 (Response Healing)

| Topic | Document | Section/Lines |
|-------|----------|---------------|
| **Jsonish Parser** | research/baml-analysis.md | Parsing section (~5,000 lines) |
| **Coercion Engine** | research/baml-analysis.md | Coercion section (~3,000 lines) |
| **Confidence Scoring** | research/baml-analysis.md | Flag system |
| **Streaming Parser** | research/baml-analysis.md | Streaming section |
| **Partial Types** | research/mvp-scope-update.md | Lines 72-136 |
| **Design Patterns** | CODING_GUIDELINES.md | Lines 1360-1710 |

### General References

- **Provider Patterns**: research/litellm-analysis.md (Lines 32-286)
- **Error Handling**: CODING_GUIDELINES.md (Lines 186-320)
- **Testing Standards**: CODING_GUIDELINES.md (Lines 752-898)
- **Performance Guidelines**: CODING_GUIDELINES.md (Lines 615-748)

---

## 📝 Notes

- **Phase 2 Completion Report**: See [PHASE2_COMPLETION_REPORT.md](PHASE2_COMPLETION_REPORT.md) for detailed analysis
- **Per-Crate TODOs**: Each crate has its own TODO.md for detailed task tracking
- **Research Documents**: All research in `research/` directory with 100+ pages of analysis

**Last Updated**: 2026-01-23 (Phase 3, Week 6 Day 6-8 Complete) | **Next Review**: 2026-01-24 (Week 6 - Integration & Examples)
