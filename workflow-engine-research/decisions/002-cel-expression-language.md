# ADR-002: CEL as Primary Expression Language

## Status
Accepted

## Context
Workflows need to evaluate expressions for:
- Conditional branching (switch nodes)
- Edge conditions
- Data transformations
- Filter predicates

Requirements:
- **Portable**: Work across Rust, Python, Go, TypeScript
- **Safe**: Sandboxed execution, no arbitrary code
- **Expressive**: Support JSON path, comparisons, logic, math
- **Fast**: Low overhead for hot paths

## Decision
Use **CEL (Common Expression Language)** as the primary expression evaluator, with a pluggable evaluator trait to support alternatives.

CEL features we'll use:
- **JSON operations**: `$.nodes.analyze.output.sentiment`
- **Comparisons**: `confidence > 0.8`
- **Logic**: `sentiment == "positive" && confidence > 0.7`
- **Math**: `(total + 1) * 2`
- **Functions**: `size(items) > 10`

Integration via:
- **Rust**: FFI to cel-go or native `cel-interpreter` crate
- **Caching**: Parse expressions once, reuse Program

## Alternatives Considered

### 1. **Rhai (Rust-native scripting)**
- **Pros**: Native Rust, easy integration, good performance
- **Cons**: Not portable to other languages, less industry adoption
- **Verdict**: Good alternative, but CEL is more portable

### 2. **JavaScript via QuickJS/V8**
- **Pros**: Extremely expressive, widely known
- **Cons**: Security concerns, heavy runtime, not sandboxed by default
- **Verdict**: Too powerful and risky for simple expressions

### 3. **Custom Expression Language**
- **Pros**: Optimized for our use case
- **Cons**: Requires parser, testing, documentation
- **Verdict**: Reinventing the wheel

### 4. **JSONata**
- **Pros**: JSON-focused, powerful transformations
- **Cons**: Less familiar, no multi-language support
- **Verdict**: Good for transforms, but CEL is more general

### 5. **JMESPath**
- **Pros**: Simple JSON querying
- **Cons**: Read-only, no logic or math
- **Verdict**: Too limited for conditions

## Consequences

### Positive
- **Industry standard**: Used by Kubernetes, Google Cloud
- **Multi-language**: Implementations in Go, Java, C++, Rust (via FFI)
- **Sandboxed**: No file I/O, network, or arbitrary code execution
- **Type-safe**: Type checking at parse time
- **Cacheable**: Parse once, evaluate many times

### Negative
- **FFI overhead**: Rust → Go bridge for cel-go (if using FFI)
- **Learning curve**: Users need to learn CEL syntax
- **Limited stdlib**: Fewer functions than JavaScript

## Implementation Notes

### Rust Integration Option 1: FFI to cel-go

```rust
// Use cgo to call cel-go
extern "C" {
    fn cel_parse(expr: *const c_char) -> *mut CelProgram;
    fn cel_eval(program: *mut CelProgram, context: *const c_char) -> *mut c_char;
}

pub struct CelEvaluator {
    cache: Arc<RwLock<HashMap<String, *mut CelProgram>>>,
}

impl CelEvaluator {
    async fn evaluate(&self, expr: &Expression, ctx: &EvaluationContext) -> Result<Value> {
        let program = self.parse_or_cached(expr).await?;

        let context_json = serde_json::to_string(&ctx)?;
        let result_cstr = unsafe {
            cel_eval(program, CString::new(context_json)?.as_ptr())
        };

        let result_str = unsafe { CStr::from_ptr(result_cstr).to_str()? };
        let value = serde_json::from_str(result_str)?;

        Ok(value)
    }
}
```

### Rust Integration Option 2: Native cel-interpreter crate

```rust
use cel_interpreter::{Context, Program};

pub struct CelEvaluator {
    cache: Arc<RwLock<HashMap<String, Program>>>,
}

impl CelEvaluator {
    async fn evaluate(&self, expr: &Expression, ctx: &EvaluationContext) -> Result<Value> {
        let program = self.parse_or_cached(expr).await?;

        let cel_ctx = Context::from(ctx.state.clone());
        let result = program.execute(&cel_ctx)?;

        Ok(result)
    }
}
```

### Example Expressions

```yaml
# Conditional
condition: '$.nodes.analyze.output.sentiment == "positive"'

# Comparison
condition: '$.nodes.analyze.output.confidence > 0.8'

# Logic
condition: 'sentiment == "positive" && confidence > 0.7'

# Math
expression: '($.nodes.count.output + 1) * 2'

# Functions
condition: 'size($.nodes.fetch.output.items) > 10'

# Ternary
expression: 'confidence > 0.8 ? "high" : "low"'
```

### Pluggable Evaluator Trait

```rust
#[async_trait]
pub trait ExpressionEvaluator: Send + Sync {
    async fn evaluate(&self, expr: &Expression, ctx: &EvaluationContext) -> Result<Value>;
    fn validate(&self, expr: &Expression) -> Result<()>;
}

// Users can provide custom evaluators
pub struct RhaiEvaluator { /* ... */ }
impl ExpressionEvaluator for RhaiEvaluator { /* ... */ }

pub struct JavaScriptEvaluator { /* ... */ }
impl ExpressionEvaluator for JavaScriptEvaluator { /* ... */ }
```

### Validation

```rust
// Validate at workflow compile time
let evaluator = CelEvaluator::new();
for node in &workflow.nodes {
    if let Some(condition) = &node.condition {
        evaluator.validate(condition)?;
    }
}
```

## Migration Path

- **Phase 1**: Implement CEL evaluator
- **Phase 2**: Add Rhai as alternative for advanced users
- **Phase 3**: Allow per-node evaluator selection

## Related Decisions
- ADR-001: Canonical IR Format
- ADR-003: State Scoping Model
