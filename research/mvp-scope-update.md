# MVP Scope Update: Partial Types & Streaming Annotations

**Date**: 2026-01-15
**Status**: ✅ Updated

---

## Changes Made

### Moved to MVP (from Post-MVP)

1. **Partial Types for Streaming**
   - Auto-generated `Partial<T>` versions of all schema types
   - All fields become `Option<T>` during streaming
   - Enables progressive result extraction

2. **Streaming Annotations**
   - `#[schema(stream_not_null)]` - Don't emit field until non-null
   - `#[schema(stream_done)]` - Only emit field when complete
   - Fine-grained control over streaming behavior

---

## Rationale

### Why This Change?

1. **Core Value Proposition**
   - Streaming with structured output is a key differentiator
   - Partial types are essential for usable streaming API
   - Without them, streaming only works for unstructured text

2. **User Experience**
   - Developers expect streaming to work with schemas, not just raw text
   - Partial types enable progressive rendering (show results as they arrive)
   - Annotations provide control over streaming behavior

3. **Implementation Dependencies**
   - Streaming parser already planned for Week 5-6
   - Derive macros already planned for MVP
   - Incremental cost: ~2-3 days of additional work

4. **Market Expectation**
   - BAML provides this out-of-the-box
   - Competitors support structured streaming
   - MVP without this would feel incomplete

---

## Updated MVP Scope

### Week 5-6: Response Healing (UPDATED)

**New Items**:
5. Implement partial types (all fields as `Option<T>`)
6. Add streaming annotations (`@@stream.not_null`, `@@stream.done`)

**New Critical File**:
- `crates/simple-agents-healing/src/partial_types.rs`

### Week 8: Core API (UPDATED)

**Additional Testing**:
- Streaming with structured schemas
- Partial type extraction
- Annotation behavior validation

---

## Implementation Details

### 1. Partial Type Generation

**Derive Macro Enhancement**:

```rust
#[derive(Schema)]
pub struct Character {
    pub name: String,
    pub age: u32,
    pub abilities: Vec<String>,
}

// Auto-generates:
#[derive(Debug)]
pub struct PartialCharacter {
    pub name: Option<String>,
    pub age: Option<u32>,
    pub abilities: Option<Vec<String>>,
}
```

**Macro Implementation** (~200 lines):
```rust
#[proc_macro_derive(Schema, attributes(schema))]
pub fn derive_schema(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Generate main impl
    let main_impl = generate_schema_impl(&input);

    // Generate partial type
    let partial_type = generate_partial_type(&input);

    quote! {
        #main_impl
        #partial_type
    }
    .into()
}

fn generate_partial_type(input: &DeriveInput) -> TokenStream2 {
    let name = &input.ident;
    let partial_name = format_ident!("Partial{}", name);

    let fields = match &input.data {
        Data::Struct(data) => &data.fields,
        _ => panic!("Schema only supports structs"),
    };

    let partial_fields = fields.iter().map(|f| {
        let name = &f.ident;
        let ty = &f.ty;
        quote! {
            pub #name: Option<#ty>
        }
    });

    quote! {
        #[derive(Debug, Clone, Default)]
        pub struct #partial_name {
            #(#partial_fields),*
        }
    }
}
```

### 2. Streaming Annotations

**Attribute Parsing**:

```rust
#[derive(Schema)]
pub struct StreamedResponse {
    #[schema(stream_not_null)]
    pub id: String,

    #[schema(stream_done)]
    pub status: String,

    pub items: Vec<Item>,
}
```

**Streaming Behavior** (~150 lines):

```rust
pub struct StreamingExtractor {
    annotations: HashMap<String, StreamAnnotation>,
}

pub enum StreamAnnotation {
    NotNull,   // Don't emit until non-null
    Done,      // Only emit when complete
    Normal,    // Emit as soon as available
}

impl StreamingExtractor {
    pub fn should_emit_field(&self, field: &str, value: &Value) -> bool {
        match self.annotations.get(field) {
            Some(StreamAnnotation::NotNull) => {
                !matches!(value, Value::Null | Value::Missing)
            }
            Some(StreamAnnotation::Done) => {
                value.is_complete()
            }
            _ => true,
        }
    }

    pub fn extract_partial(&mut self, json: &str) -> PartialValue {
        let parsed = self.parser.parse_incremental(json);

        let mut partial = PartialValue::default();

        for (field, value) in parsed.fields() {
            if self.should_emit_field(field, value) {
                partial.set_field(field, value);
            }
        }

        partial
    }
}
```

### 3. Streaming API

**User-Facing API**:

```rust
// Example: Stream structured output
let mut stream = client
    .completion()
    .model("gpt-4")
    .messages(vec![Message::user("Generate character")])
    .response_format::<Character>()
    .stream()
    .await?;

// Receive partial results
while let Some(partial) = stream.next().await {
    let partial: PartialCharacter = partial?;

    // Only set fields are Some
    if let Some(name) = &partial.name {
        println!("Name: {}", name);
    }

    // Arrays can be partially filled
    if let Some(abilities) = &partial.abilities {
        println!("Abilities so far: {:?}", abilities);
    }
}

// Final complete result
let character: Character = stream.finalize()?;
```

---

## Timeline Impact

### Original MVP: 12 weeks

### Updated MVP: 12 weeks (unchanged)

**How?**

Week 5-6 was already allocated for streaming parser. Adding partial types and annotations is:
- ~2 days for macro enhancement
- ~2 days for annotation parsing
- ~1 day for additional testing

Total: ~5 days, absorbed within 2-week buffer.

---

## Updated Success Criteria

### MVP Complete When:

✅ Can make completions to OpenAI, Anthropic, OpenRouter
✅ Handles malformed JSON responses (90%+ of common cases)
✅ Retry logic works with exponential backoff
✅ Fallback chain successfully fails over
✅ In-memory cache reduces duplicate calls
✅ **Streaming works with structured schemas** ← NEW
✅ **Partial types generated automatically** ← NEW
✅ **Streaming annotations control emission** ← NEW
✅ Python bindings work (sync + async)
✅ CLI can complete single requests and run interactive chat
✅ All tests pass (unit, integration, FFI)
✅ Documentation complete (README, examples, API docs)
✅ CI/CD pipeline green

---

## Benefits of This Change

### For Users

1. **Better UX**: See structured results as they stream
2. **Progressive Rendering**: Display partial data immediately
3. **Fine Control**: Annotations let users control what emits when
4. **Type Safety**: Compiler ensures correct handling of partial data

### For Implementation

1. **Clear Scope**: Well-defined boundary (macros + parser)
2. **Reuses Work**: Builds on existing streaming parser
3. **Test Coverage**: Natural test cases from BAML research
4. **Future-Proof**: Foundation for advanced features later

### For Adoption

1. **Feature Parity**: Matches BAML capabilities
2. **Competitive**: Unique among Rust LLM libraries
3. **Marketing**: "Streaming structured output" is a strong hook
4. **Documentation**: Clear, compelling examples

---

## Risk Mitigation

### Potential Risks

1. **Complexity**: Macro generation can be tricky
   - **Mitigation**: Use `syn` and `quote`, well-tested patterns
   - **Fallback**: Start with simple macro, enhance iteratively

2. **Performance**: Partial extraction overhead
   - **Mitigation**: Benchmark early, optimize hot paths
   - **Fallback**: Make streaming annotations optional

3. **API Confusion**: Users might misuse partial types
   - **Mitigation**: Clear documentation, compile-time hints
   - **Fallback**: Runtime warnings for common mistakes

---

## Updated File Structure

```
crates/
├── simple-agents-healing/
│   ├── src/
│   │   ├── parser.rs              # Jsonish parser
│   │   ├── coercion.rs            # Type coercion
│   │   ├── streaming.rs           # NEW: Streaming extractor
│   │   └── partial_types.rs       # NEW: Partial type utilities
│   └── Cargo.toml
│
├── simple-agents-macros/
│   ├── src/
│   │   ├── lib.rs                 # UPDATED: Partial generation
│   │   ├── schema.rs              # Schema derive
│   │   └── streaming.rs           # NEW: Annotation parsing
│   └── Cargo.toml
│
└── simple-agents-core/
    ├── src/
    │   ├── client.rs              # UPDATED: Streaming API
    │   └── streaming/
    │       ├── mod.rs             # NEW: Streaming module
    │       └── stream_response.rs # NEW: Stream wrapper
    └── Cargo.toml
```

---

## Documentation Updates

### New Examples Needed

1. **Basic Streaming with Schema**
   ```rust
   // examples/streaming_basic.rs
   ```

2. **Streaming Annotations**
   ```rust
   // examples/streaming_annotations.rs
   ```

3. **Progressive Rendering**
   ```rust
   // examples/progressive_ui.rs
   ```

### Updated Docs

- `README.md`: Add streaming section
- `docs/streaming.md`: Comprehensive guide
- `docs/macros.md`: Document annotations
- API docs: `#[derive(Schema)]` with examples

---

## Conclusion

Moving partial types and streaming annotations to MVP:

✅ **Aligns with core value proposition** (healing + structured output)
✅ **Improves user experience** (streaming actually useful)
✅ **Reasonable scope increase** (~5 days within buffer)
✅ **Strong competitive position** (matches BAML, exceeds alternatives)
✅ **Foundation for future** (enables advanced features later)

**Recommendation**: Proceed with updated MVP scope.

---

**Updated Documents**:
- ✅ `research/implementation-plan.md` - Updated Week 5-6, MVP scope, Type System
- ✅ `research/README.md` - Updated Phase 3, MVP scope, Critical Files
- ✅ `research/mvp-scope-update.md` - This document

**Next Steps**:
1. Review and approve scope change
2. Begin implementation (Week 1-2: Foundation)
3. Prioritize macro development in Week 5
4. Test streaming thoroughly in Week 8
