# SimpleAgents TODO

## Healing System Integration for Structured Outputs

### ✅ Completed

#### Phase 1: Foundation
- [x] Add HealingMetadata to response types (`CompletionResponse`)
- [x] Implement schema converter (JSON Schema → Healing Schema)
- [x] Implement healing integration layer (`HealingIntegration`, `HealingConfig`)

#### Phase 2: Provider Integration
- [x] Update OpenAI provider with healing fallback
  - Added `healing` field and `with_healing()` builder method
  - Added request context storage for schema extraction
  - Updated `transform_response()` with try-catch-heal pattern
- [x] Update Anthropic provider with healing fallback
  - Added `healing` field and `with_healing()` builder method
  - Added request context storage for schema extraction
  - Updated `transform_response()` with try-catch-heal pattern

#### Phase 3: Streaming Support
- [x] Implement streaming structured output support (`StructuredStream` wrapper)
- [x] Add `ProviderStructuredExt` trait for structured streaming

#### Phase 4: Testing & Examples
- [x] Create healing fallback example (`examples/healing_fallback.rs`)
- [x] Create streaming structured example (`examples/streaming_structured.rs`)
- [x] Create comprehensive integration tests (`tests/healing_integration_tests.rs`)

## Future Enhancements

### Provider Support
- [ ] Add OpenRouter provider healing support
- [ ] Add support for more providers (Gemini, Cohere, etc.)

### Streaming Enhancements
- [ ] Progressive partial object updates during streaming
- [ ] Stream-time validation and early error detection
- [ ] Backpressure handling for slow consumers

### Configuration
- [ ] Environment variable configuration for healing
  - `SIMPLE_AGENTS_HEALING_ENABLED`
  - `SIMPLE_AGENTS_HEALING_MIN_CONFIDENCE`
- [ ] Per-request healing configuration override
- [ ] Healing metrics and observability

### Advanced Features
- [ ] Schema caching and reuse
- [ ] Custom coercion rules
- [ ] Healing strategy selection (strict/balanced/lenient)
- [ ] Response replay for debugging
- [ ] Confidence calibration and tuning

### Documentation
- [ ] Add healing system architecture documentation
- [ ] Create troubleshooting guide
- [ ] Add performance benchmarks
- [ ] Create migration guide for existing users

### Testing
- [ ] Property-based testing for schema converter
- [ ] Fuzzing for parser resilience
- [ ] Load testing for concurrent healing
- [ ] Integration tests with real API responses

## Notes

### Implementation Decisions
- **Opt-in by default**: Healing must be explicitly enabled via `with_healing()`
- **Transparent**: All transformations tracked in `HealingMetadata`
- **Fast path first**: Native parsing attempted before healing (zero overhead when successful)
- **Provider-agnostic**: Same healing logic works across all providers
- **Thread-safe**: Request context stored in `Arc<Mutex<>>` for concurrent access

### Performance Targets
- Schema conversion: <1ms (cached after first use)
- Healing overhead: <10ms (95th percentile)
- Streaming latency: <5% increase vs non-structured
- Memory overhead: <100KB per request
