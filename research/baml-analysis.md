# BAML (Boundary ML) Architecture Analysis

**Research Date**: 2026-01-15
**Repository**: `/Users/rishub/Desktop/projects/personal/learning/baml/`

---

## Executive Summary

BAML (Basically a Made-up Language) is a domain-specific language (DSL) for building reliable AI workflows. Its core philosophy: **"Prompts are functions"** - every prompt is a typed function with inputs and outputs. The standout feature is **response healing**: sophisticated JSON parsing that handles malformed LLM outputs through a flexible "jsonish" parser and schema-aligned coercion system.

**Key Innovation**: Instead of failing on broken JSON, BAML tracks transformations via a flag system and provides confidence scores, making LLM integrations robust in production.

---

## What is BAML?

### Core Philosophy

1. **Prompts as Typed Functions**
   - Define prompts with input/output types
   - Compile-time validation
   - Runtime type checking

2. **Schema Engineering > Prompt Engineering**
   - Focus on strong type definitions
   - Let the framework handle formatting
   - Less string manipulation, more structure

3. **Universal Integration**
   - Write prompts once in BAML
   - Call from any language (Python, TypeScript, Ruby, Go)
   - Generated clients for each language

4. **Built-in Reliability**
   - Handle malformed outputs automatically
   - Streaming with partial types
   - Confidence scoring

### BAML Language Example

```baml
// Define a function (prompt)
function ExtractResume {
  input: string  // Raw resume text
  output: Resume

  prompt #"
    Extract structured information from this resume:
    {{ input }}

    Return JSON matching this format:
    {{ ctx.output_format }}
  "#
}

// Define output schema
class Resume {
  name string
  email string?  // Optional field
  experience Experience[] @description("Work history")
  skills string[] @alias(abilities)  // Field alias

  @@stream.not_null  // Don't emit in streaming until populated
}

class Experience {
  company string
  title string
  years int @assert({{ _ >= 0 && _ <= 50 }})  // Validation
}

// Client configuration
client<llm> GPT4 {
  provider openai
  options {
    model "gpt-4"
    api_key env.OPENAI_API_KEY
  }
}
```

---

## Response Healing System

### The "Crown Jewel" - Jsonish Parser

**Location**: `/engine/baml-lib/jsonish/src/jsonish/parser/fixing_parser/`

#### Why It Exists

LLMs frequently return malformed JSON:
- Missing closing quotes/brackets
- Trailing commas
- Single quotes instead of double quotes
- Markdown wrapping (```json ... ```)
- Comments in JSON
- Unquoted keys
- Partial/streaming responses

Standard `JSON.parse()` or `serde_json` would fail. BAML's parser succeeds.

#### How It Works

**Incremental State Machine** that tracks:

```rust
pub struct FixingParser {
    state: ParserState,
    depth: usize,          // Bracket/brace nesting level
    buffer: Vec<char>,     // Accumulated characters
    completion: CompletionState,  // Complete, Incomplete, Pending
}

pub enum ParserState {
    ExpectValue,
    InString(StringState),
    InNumber,
    InArray,
    InObject,
    InKey,
    // ... more states
}

pub enum CompletionState {
    Complete,    // Value fully parsed
    Incomplete,  // Partial (e.g., "unclosed string)
    Pending,     // Waiting for more data
}
```

**Parsing Phases**:

1. **Strip Markdown**
   ```rust
   fn strip_markdown(&mut self, input: &str) -> String {
       // Remove ```json\n ... \n``` wrappers
       // Remove ``` ... ``` wrappers
       // Set flag if stripped
   }
   ```

2. **Fix Common Issues**
   ```rust
   fn fix_common_issues(&self, input: &str) -> String {
       // 1. Remove trailing commas: [1, 2,] → [1, 2]
       // 2. Convert single quotes: {'key': 'value'} → {"key": "value"}
       // 3. Quote unquoted keys: {key: "value"} → {"key": "value"}
       // 4. Handle unclosed strings (best effort)
       // Set flags for each fix
   }
   ```

3. **Try Standard Parser**
   ```rust
   if let Ok(value) = serde_json::from_str(&fixed) {
       return Ok(JsonValue::from_serde(value));
   }
   ```

4. **Lenient State Machine Parse**
   ```rust
   fn lenient_parse(&mut self, input: &str) -> Result<JsonValue> {
       // Character-by-character parsing
       for ch in input.chars() {
           match self.state {
               ParserState::ExpectValue => {
                   match ch {
                       '"' => self.state = ParserState::InString,
                       '[' => self.state = ParserState::InArray,
                       '{' => self.state = ParserState::InObject,
                       // ... handle numbers, booleans, null
                   }
               }
               ParserState::InString(ref mut str_state) => {
                   // Handle escaped characters
                   // Track quote types (single, double, triple)
                   // Determine string completion
               }
               // ... other states
           }
       }

       // Auto-close unclosed structures
       self.auto_complete()
   }
   ```

#### String Parsing Complexity

**Multiple String Types Supported**:
```rust
pub enum StringDelimiter {
    Double,      // "string"
    Single,      // 'string'
    Triple,      // """multiline"""
    Backtick,    // `template`
    Unquoted,    // bare_value (for keys)
}
```

**Escape Handling**:
- Tracks escaped characters (`\"`, `\'`, `\\`)
- Handles Unicode escapes (`\u0041`)
- Smart quote detection (is `"` end of string or escaped?)

#### Auto-Completion

When parsing ends with incomplete structures:

```rust
fn auto_complete(&mut self) -> JsonValue {
    // Close unclosed arrays: [1, 2, 3 → [1, 2, 3]
    // Close unclosed objects: {"key": "val → {"key": "val"}
    // Complete partial strings: {"key": "val → {"key": "val"}

    // Mark completion state as Incomplete
    self.completion = CompletionState::Incomplete;
}
```

---

## Detailed Jsonish Parser Algorithm

### Architecture Overview

The jsonish parser uses a **multi-strategy parsing approach** with fallback mechanisms. It attempts parsing in order of strictness:

```
Input String
    ↓
1. Try Standard JSON (serde_json) ← Fast path
    ↓ (fails)
2. Try Markdown Extraction ← Strip ```json``` wrappers
    ↓ (fails)
3. Try Multi-JSON Extraction ← Find multiple JSON objects
    ↓ (fails)
4. Try Fixing Parser ← STATE MACHINE (the crown jewel)
    ↓ (fails)
5. Return as String ← Last resort
```

**Key Files**:
- `/engine/baml-lib/jsonish/src/jsonish/parser/entry.rs` - Orchestrator
- `/engine/baml-lib/jsonish/src/jsonish/parser/fixing_parser.rs` - Main loop
- `/engine/baml-lib/jsonish/src/jsonish/parser/fixing_parser/json_parse_state.rs` - State machine
- `/engine/baml-lib/jsonish/src/jsonish/parser/fixing_parser/json_collection.rs` - Data structures
- `/engine/baml-lib/jsonish/src/jsonish/parser/markdown_parser.rs` - Markdown stripper

---

### Phase 1: Entry Point (Multi-Strategy Orchestration)

**Location**: `entry.rs:15-243`

```rust
pub fn parse_func(str: &str, options: ParseOptions, is_done: bool) -> Result<Value>
```

**Strategy Order**:

1. **Fast Path - Standard JSON** (`entry.rs:25-55`)
   ```rust
   match serde_json::from_str(str) {
       Ok(v) => return Ok(Value::AnyOf(vec![v], str.to_string())),
       Err(e) => { /* Continue to next strategy */ }
   }
   ```
   - Uses `serde_json` for well-formed JSON
   - If successful, marks completion state and returns immediately
   - **Optimization**: Avoids expensive state machine for valid JSON

2. **Markdown Block Extraction** (`entry.rs:57-137`)
   ```rust
   if options.allow_markdown_json {
       match markdown_parser::parse(str, &options) {
           Ok(items) => { /* Extract code blocks */ }
       }
   }
   ```
   - Regex-based extraction of ` ```json ... ``` ` blocks
   - Returns `Value::Markdown(tag, content, state)`
   - Handles multiple code blocks in one string

3. **Multi-JSON Object Finder** (`entry.rs:139-174`)
   ```rust
   if options.all_finding_all_json_objects {
       match multi_json_parser::parse(str, &options) {
           Ok(items) => { /* Return array of objects */ }
       }
   }
   ```
   - Searches for multiple JSON objects in text
   - Returns array of all found objects
   - Useful for streaming or concatenated outputs

4. **Fixing Parser (State Machine)** (`entry.rs:176-224`)
   ```rust
   if options.allow_fixes {
       match fixing_parser::parse(str, &options) {
           Ok(items) => {
               // Return Value::FixedJson(value, fixes)
           }
       }
   }
   ```
   - **The main algorithm** - handles malformed JSON
   - Tracks all transformations as `Fixes` flags
   - Returns `Value::FixedJson(value, vec![Fixes])`

5. **String Fallback** (`entry.rs:226-235`)
   ```rust
   if options.allow_as_string {
       return Ok(Value::String(str.to_string(), completion_state));
   }
   ```
   - Last resort: treat entire input as string
   - Respects `is_done` flag for completion state

**Key Innovation**: The `Value::AnyOf(Vec<Value>, String)` type represents multiple interpretations:
```rust
// Example: "42" could be:
Value::AnyOf(vec![
    Value::Number(42, Complete),      // Parsed as number
    Value::String("42", Complete),    // Parsed as string
], "42".to_string())
```

---

### Phase 2: Markdown Parser (Preprocessing)

**Location**: `markdown_parser.rs:29-122`

**Purpose**: Extract JSON from markdown code blocks before parsing.

**Algorithm**:

```rust
static MD_TAG_START: Regex = regex!(r"(?m)^[ \t]*```([a-zA-Z0-9 ]+)(?:\n|$)");
static MD_TAG_END: Regex = regex!(r"(?m)^[ \t]*```(?:\n|$)");

pub fn parse(str: &str) -> Result<Vec<MarkdownResult>> {
    let mut values = vec![];
    let mut remaining = str;

    while let Some(cap) = MD_TAG_START.find(remaining) {
        let tag = cap.as_str();  // e.g., "```json"
        let after_start = &remaining[cap.end()..];

        // Find all potential closing fences
        let ends: Vec<_> = MD_TAG_END.find_iter(after_start).collect();

        // Prefer the FIRST closing fence that yields successful parse
        for end in ends {
            let candidate = after_start[..end.start()].trim();
            if let Ok(v) = entry::parse_func(candidate, options, false) {
                values.push(MarkdownResult::CodeBlock(tag, v));
                break;
            }
        }

        remaining = &remaining[end..];
    }

    Ok(values)
}
```

**Key Insight**: Greedily tries to match the shortest valid code block. This prevents accidentally consuming nested ` ``` ` sequences inside strings.

**Example**:
```
Input:
```json
{"code": "```\ninner\n```"}
```

```json
{"second": "block"}
```

Output:
- Block 1: {"code": "```\ninner\n```"}  ← Correctly parsed
- Block 2: {"second": "block"}
```

---

### Phase 3: Fixing Parser (Main Loop)

**Location**: `fixing_parser.rs:11-98`

**Entry Point**:
```rust
pub fn parse(str: &str, options: &ParseOptions) -> Result<Vec<(Value, Vec<Fixes>)>>
```

**Algorithm**:

```rust
let mut state = JsonParseState::new();
let mut chars = str.char_indices().peekable();

// 1. CHARACTER-BY-CHARACTER PROCESSING
while let Some((idx, c)) = chars.next() {
    let peekable = str[idx + c.len_utf8()..].char_indices().peekable();

    match state.process_token(c, peekable) {
        Ok(increments) => {
            // Skip ahead N characters (for multi-char tokens like """)
            for _ in 0..increments {
                chars.next();
            }
        }
        Err(e) => return Err(e),
    }
}

// 2. AUTO-COMPLETE UNCLOSED STRUCTURES
while !state.collection_stack.is_empty() {
    state.complete_collection(CompletionState::Incomplete);
}

// 3. DETERMINE WHAT TO RETURN
match state.completed_values.len() {
    0 => Err(anyhow!("No JSON objects found")),
    1 => Ok(vec![(value, fixes)]),
    _ => {
        // Multiple values: filter for objects/arrays or wrap strings
        if state.completed_values.iter().all(|f| f.0 == "string") {
            Ok(vec![(Value::Array(strings), vec![Fixes::InferredArray])])
        } else {
            Ok(values.filter(|f| f.0 == "Object" || f.0 == "Array"))
        }
    }
}
```

**Key Operations**:
1. **Process each character** through state machine
2. **Peek ahead** to make lookahead decisions
3. **Auto-complete** partial structures at end
4. **Infer arrays** from multiple string values

---

### Phase 4: State Machine (Core Algorithm)

**Location**: `json_parse_state.rs:22-875`

**State Structure**:

```rust
pub struct JsonParseState {
    /// Stack of nested collections being built (for nesting like {[{...}]})
    collection_stack: Vec<(JsonCollection, Vec<Fixes>)>,

    /// Completed values (popped from stack)
    completed_values: Vec<(&'static str, Value, Vec<Fixes>)>,

    /// Incremental quote tracking (O(1) instead of O(n²))
    string_quote_tracking: StringQuoteTracking,
}

#[derive(Debug)]
pub enum JsonCollection {
    Object(Vec<String>, Vec<Value>, CompletionState),  // Keys, Values, State
    Array(Vec<Value>, CompletionState),
    QuotedString(String, CompletionState),             // "..."
    TripleQuotedString(String, CompletionState),       // """..."""
    SingleQuotedString(String, CompletionState),       // '...'
    BacktickString(String, CompletionState),           // `...`
    TripleBacktickString { lang, path, content },      // ```lang\n...```
    UnquotedString(String, CompletionState),           // bareword, 42, true
    TrailingComment(String, CompletionState),          // // comment
    BlockComment(String, CompletionState),             // /* comment */
}
```

**Main State Machine** (`json_parse_state.rs:493-731`):

```rust
pub fn process_token(
    &mut self,
    token: char,
    mut next: Peekable<impl Iterator<Item = (usize, char)>>,
) -> Result<usize>
```

**State Transitions**:

```
STACK TOP STATE              TOKEN       ACTION
─────────────────────────────────────────────────────────────
Object                       '}'         Complete collection
Object                       ','         Ignore (separator)
Object                       ':'         Ignore (separator)
Object                       other       find_any_starting_value()

Array                        ']'         Complete collection
Array                        ','         Ignore (separator)
Array                        other       find_any_starting_value()

QuotedString                 '"'         Check if should close → Complete
QuotedString                 '\\'        Handle escape sequences
QuotedString                 other       Consume character

TripleQuotedString           '"'         Check if """ → Complete
TripleQuotedString           other       Consume character

SingleQuotedString           '\''        Check if should close → Complete
SingleQuotedString           other       Consume character

UnquotedString               any         Consume, then check termination

TrailingComment              '\n'        Complete collection
TrailingComment              other       Consume character

BlockComment                 '*'         Check if */ → Complete
BlockComment                 other       Consume character

None (empty stack)           any         find_any_starting_value()
```

**Value Start Detection** (`json_parse_state.rs:734-867`):

```rust
fn find_any_starting_value(&mut self, token: char, next: Peekable) -> Result<usize> {
    match token {
        '{' => push(Object([], [], Incomplete)),
        '[' => push(Array([], Incomplete)),

        '"' => {
            // Check for triple quotes
            if next matches "\"\"" {
                push(TripleQuotedString);
                return Ok(2);  // Skip 2 more characters
            } else {
                reset_quote_tracking();
                push(QuotedString);
            }
        }

        '\'' => push(SingleQuotedString),

        '`' => {
            // Check for triple backticks
            if next matches "``" {
                push(TripleBacktickString);
                return Ok(2);
            } else {
                push(BacktickString);
            }
        }

        '/' => {
            if next matches '/' {
                push(TrailingComment);
                return Ok(1);
            }
            if next matches '*' {
                push(BlockComment);
                return Ok(1);
            }
            // Otherwise, treat as unquoted string (e.g., path)
            push(UnquotedString("/"));
        }

        whitespace => { /* Ignore */ }

        other => {
            push(UnquotedString(other));
            if should_close_unescaped_string(next) {
                complete_collection();
            }
        }
    }
}
```

---

### Phase 5: String Closing Heuristics

**The Hardest Part**: Determining when an unquoted or badly-quoted string should end.

#### A. Quoted String Closing (`json_parse_state.rs:376-491`)

**Quote Tracking Optimization** (`json_parse_state.rs:9-79`):

```rust
#[derive(Default)]
struct StringQuoteTracking {
    trailing_backslashes: usize,      // Count of consecutive '\' at end
    unescaped_quote_count: usize,     // Count of quotes NOT escaped
}

fn update_quote_tracking(&mut self, token: char) {
    if token == '\\' {
        self.trailing_backslashes += 1;
    } else {
        if token == '"' {
            // Quote is unescaped if preceded by EVEN number of backslashes
            if self.trailing_backslashes.is_multiple_of(2) {
                self.unescaped_quote_count += 1;
            }
        }
        self.trailing_backslashes = 0;
    }
}
```

**Closing Logic**:
```rust
fn should_close_string(&mut self, next: Peekable, closing_char: char) -> bool {
    // Determine context: are we in object key, object value, or array?
    let (in_object_key, in_object_value, in_array) = determine_context();

    // Use pre-computed quote count (O(1), not O(n))
    let closing_char_count = if closing_char == '"' {
        self.string_quote_tracking.unescaped_quote_count
    } else {
        0
    };

    match next.peek() {
        Some(':') | Some('}') if in_object_key => true,  // Key ended
        Some(',') if in_object_value || in_array => {
            // Close if even number of quotes (balanced)
            closing_char_count % 2 == 0
        }
        Some('}') if in_object_value => true,
        Some(']') if in_array => true,
        Some(' ' | '\t' | '\n') => {
            // Look ahead through whitespace for structural characters
            loop {
                match consume_whitespace_and_peek() {
                    '}' | ':' | ',' | ']' => return true,
                    '/' => {
                        // Could be comment
                        if peek() matches "//" or "/*" => return true
                    }
                    _ => return false,
                }
            }
        }
        Some(c) if c == closing_char => false,  // More quotes coming
        Some('{' | '"' | '\'' | '[') => !has_parent_object,
        _ => false,
    }
}
```

**Key Insight**: The parser uses **contextual awareness** (am I in object key, value, or array?) combined with **lookahead** to decide string termination.

#### B. Unquoted String Closing (`json_parse_state.rs:187-374`)

**Most Complex Logic** - handles bare identifiers, numbers, booleans, and malformed strings.

```rust
fn should_close_unescaped_string(
    &mut self,
    next: Peekable<impl Iterator<Item = (usize, char)>>,
) -> CloseStringResult  // Returns: Close(offset, CompletionState) | Continue
```

**Algorithm by Context**:

```rust
let pos = determine_position();  // InNothing, InObjectKey, InObjectValue, InArray

match pos {
    InNothing => {
        // Not inside any structure - look for start of new structure
        for (idx, c) in next {
            match c {
                '{' | '[' => return Close(idx, Complete),  // New structure
                _ => consume(c),  // Keep accumulating
            }
        }
        Close(end, Incomplete)  // EOF reached
    }

    InObjectKey => {
        // Key must end with ':'
        for (idx, c) in next {
            match c {
                ':' => return Close(idx, Complete),
                _ => consume(c),
            }
        }
        Close(end, Incomplete)
    }

    InObjectValue => {
        // Value can end with ',' or '}'
        for (idx, c) in next {
            match c {
                ',' => {
                    // COMPLEX HEURISTIC: Is this a real comma or part of value?

                    // Check if current value looks like a JSON primitive
                    let is_numeric = current_value.parse::<f64>().is_ok();
                    let is_bool = matches!(current_value, "true" | "false");
                    let is_null = current_value == "null";
                    let is_identifier = !current_value.contains(" ")
                                     && !current_value.contains("(");

                    if is_numeric || is_bool || is_null || is_identifier {
                        // Look ahead after comma
                        match peek_after_comma() {
                            '\n' => return Close(idx, Complete),
                            ' ' => {
                                // Check for comment or new key
                                if peek_through_whitespace() matches "//" | "/*" | '"' {
                                    return Close(idx, Complete);
                                }
                                // Otherwise, comma is part of value
                                consume(c);
                            }
                            _ => consume(c),
                        }
                    }
                }
                '}' => return Close(idx, Complete),
                _ => consume(c),
            }
        }
        Close(end, Incomplete)
    }

    InArray => {
        // Array element ends with ',' or ']'
        for (idx, c) in next {
            match c {
                ',' | ']' => return Close(idx, Complete),
                _ => consume(c),
            }
        }
        Close(end, Incomplete)
    }
}
```

**Tricky Case**: Distinguishing between commas that separate values vs. commas in text:
```json
// Should close at comma:
{"value": hello world, "next": ...}
            ↑ close here

// Should NOT close at comma:
{"value": hello, world more text, "next": ...}
                ↑ part of value
```

**Heuristic**:
1. If value looks like JSON primitive (number, boolean, null, identifier) → close
2. If next char is newline or whitespace followed by comment/quote → close
3. Otherwise → keep accumulating

---

### Phase 6: Collection Completion

**Location**: `json_parse_state.rs:82-127`

```rust
pub fn complete_collection(&mut self, completion_state: CompletionState) {
    let (collection, fixes) = self.collection_stack.pop().unwrap();

    let name = collection.name();  // "Object", "Array", "String", etc.
    let mut value: Value = collection.into();  // Convert to Value enum

    // If marked Complete, recursively mark all children Complete
    if completion_state == CompletionState::Complete {
        value.complete_deeply();
    }

    // Decide where to put the value
    if let Some((parent, _)) = self.collection_stack.last_mut() {
        // Has parent collection - add to it
        match parent {
            JsonCollection::Object(keys, values, _) => {
                if keys.len() == values.len() {
                    // Expecting key
                    keys.push(value.to_string());
                } else {
                    // Expecting value
                    values.push(value);
                }
            }
            JsonCollection::Array(values, _) => {
                values.push(value);
            }
            _ => panic!("Invalid parent collection"),
        }
    } else {
        // No parent - this is a top-level value
        self.completed_values.push((name, value, fixes));
    }
}
```

**Key Operations**:
1. **Pop** collection from stack
2. **Convert** `JsonCollection` → `Value`
3. **Mark completion** (recursively if complete)
4. **Add to parent** or save as completed value

---

### Phase 7: Collection Conversion

**Location**: `json_collection.rs:68-126`

```rust
impl From<JsonCollection> for Option<Value> {
    fn from(collection: JsonCollection) -> Option<Value> {
        match collection {
            // Comments are discarded
            TrailingComment(..) | BlockComment(..) => None,

            Object(keys, values, state) => {
                let pairs = keys.into_iter().zip(values).collect();
                Some(Value::Object(pairs, state))
            }

            Array(values, state) => {
                Some(Value::Array(values, state))
            }

            QuotedString(s, state) => {
                Some(Value::String(s, state))
            }

            TripleQuotedString(s, state) => {
                // Dedent the content
                Some(Value::String(dedent(&s), state))
            }

            TripleBacktickString { content, .. } => {
                // Strip first line (language specifier), dedent rest
                let (_, code) = content.0.split_once("\n")?;
                Some(Value::String(dedent(code), content.1))
            }

            UnquotedString(s, state) => {
                let s = s.trim();
                // Try parsing as primitive
                if s == "true" => Some(Value::Boolean(true)),
                else if s == "false" => Some(Value::Boolean(false)),
                else if s == "null" => Some(Value::Null),
                else if let Ok(n) = s.parse::<i64>() => {
                    Some(Value::Number(n.into(), state))
                }
                else if let Ok(n) = s.parse::<f64>() => {
                    Some(Value::Number(n.into(), state))
                }
                else => Some(Value::String(s.into(), state))
            }

            // ... other conversions
        }
    }
}
```

**Transformations**:
- **Dedenting**: Triple-quoted strings and code blocks
- **Primitive parsing**: `"true"` → `Boolean(true)`, `"42"` → `Number(42)`
- **Comment removal**: Comments return `None`
- **String trimming**: Unquoted strings are trimmed

---

### Key Algorithmic Insights

#### 1. **Incremental Quote Tracking** (O(1) vs O(n²))

**Problem**: Determining if a quote closes a string requires counting preceding backslashes:
```
"foo\"bar"    ← \" is escaped (1 backslash, odd)
"foo\\"       ← \\ is escaped backslash, " closes (2 backslashes, even)
```

**Naive Approach** (O(n²)):
```rust
fn is_quote_escaped(s: &str, pos: usize) -> bool {
    let mut backslashes = 0;
    let mut i = pos - 1;
    while i >= 0 && s.chars().nth(i) == '\\' {
        backslashes += 1;
        i -= 1;
    }
    backslashes % 2 == 1  // Odd = escaped
}
```
For each quote, scan backwards → O(n) per quote → O(n²) total.

**BAML's Approach** (O(1)):
```rust
struct StringQuoteTracking {
    trailing_backslashes: usize,
    unescaped_quote_count: usize,
}

// Called once per character
fn update_quote_tracking(&mut self, token: char) {
    if token == '\\' {
        self.trailing_backslashes += 1;
    } else {
        if token == '"' && self.trailing_backslashes.is_multiple_of(2) {
            self.unescaped_quote_count += 1;
        }
        self.trailing_backslashes = 0;
    }
}
```
Constant time per character → O(n) total. **100x faster for long strings.**

#### 2. **Context-Aware Termination**

The parser maintains a **stack of contexts**:
```
Stack: [Object, Array, QuotedString]
         ↑       ↑       ↑
       root   parent   current
```

Termination decisions use parent context:
```json
{
  "key": value with spaces,  ← Comma terminates (in object value)
  "array": [item one, two]   ← Comma doesn't terminate (in array, need ])
}
```

#### 3. **Lookahead for Disambiguation**

When encountering ambiguous tokens, parser peeks ahead:
```
Current: '/'
Next:    '/' → Treat as comment start
Next:    '*' → Treat as block comment start
Next:    'a' → Treat as unquoted string (path like /api/endpoint)
```

#### 4. **Auto-Completion Strategy**

At EOF, unclosed structures are forcibly completed:
```rust
// Input: [1, 2, {"key": "val
//
// Stack at EOF:
//   [Object(["key"], [String("val", Incomplete)])]
//   [Array([Number(1), Number(2)])]
//
// Auto-complete process:
while !stack.is_empty() {
    complete_collection(Incomplete);
}
//
// Result: [1, 2, {"key": "val"}]  ← All closed, marked Incomplete
```

#### 5. **Multi-Character Token Lookahead**

Some tokens require looking ahead multiple characters:
```rust
'"' → Check if next 2 are also '"'
      If yes: TripleQuotedString, skip 2 chars
      If no: QuotedString

'`' → Check if next 2 are also '`'
      If yes: TripleBacktickString, skip 2 chars
      If no: BacktickString
```

**Return value**: `Ok(usize)` = number of extra characters to skip.

---

## Learning the Jsonish Parser Algorithm

This section breaks down the algorithm step-by-step so you can understand and implement it yourself.

---

### Level 0: The Problem We're Solving

**Goal**: Parse JSON that LLMs produce, which is often broken.

**Why Standard Parsers Fail**:

```python
# LLM Output Example 1:
```json
{"name": "Alice", "age": 30}
```

# Standard parser sees:
"```json\n{\"name\": \"Alice\", \"age\": 30}\n```"
# ❌ Not valid JSON (has markdown wrapper)

# LLM Output Example 2:
{"name": "Bob", "skills": ["python", "rust"

# Standard parser:
# ❌ SyntaxError: Unexpected end of JSON input (missing ]])

# LLM Output Example 3:
{'name': 'Charlie', 'role': 'engineer'}

# Standard parser:
# ❌ SyntaxError: Expecting property name enclosed in double quotes
```

**Our Solution**: Build a "healing" parser that fixes these issues automatically.

---

### Level 1: The Stack-Based Approach

#### Core Concept: A Stack of "Collections Being Built"

Think of parsing JSON like building with Lego blocks. You start pieces, nest them, and complete them.

**The Stack**:
```
When you see {    → Push "Object" onto stack
When you see [    → Push "Array" onto stack
When you see "    → Push "String" onto stack
When you see }    → Pop "Object" from stack (completed!)
When you see ]    → Pop "Array" from stack (completed!)
When you see "    → Pop "String" from stack (completed!)
```

**Example Walkthrough**: Parse `{"name": "Alice"}`

```
Input:  { " n a m e " : " A l i c e " }
Step:   1 2 3 4 5 6 7 8 9...

Step 1: See '{'
  Stack: [Object]
  Object has: keys=[], values=[]

Step 2: See '"'
  Stack: [Object, String]
  String is empty: ""

Step 3-6: See 'n', 'a', 'm', 'e'
  Stack: [Object, String]
  String grows: "n" → "na" → "nam" → "name"

Step 7: See '"'
  Stack: [Object]  ← String popped!
  Object now has: keys=["name"], values=[]

Step 8: See ':'
  Stack: [Object]
  (Ignore colons, they just separate key from value)

Step 9: See '"'
  Stack: [Object, String]
  String is empty: ""

Step 10-14: See 'A', 'l', 'i', 'c', 'e'
  Stack: [Object, String]
  String grows: "A" → "Al" → "Ali" → "Alic" → "Alice"

Step 15: See '"'
  Stack: [Object]  ← String popped!
  Object now has: keys=["name"], values=["Alice"]

Step 16: See '}'
  Stack: []  ← Object popped! We're done!
  Result: {"name": "Alice"}
```

**Key Insight**: The stack tells us WHERE we are in the structure.

---

### Level 2: Handling the Easy Fixes

#### Fix 1: Markdown Wrappers

**Problem**:
```
```json
{"data": 123}
```
```

**Solution**: Strip the ` ```json ` and ` ``` ` before parsing.

**Algorithm**:
```rust
fn strip_markdown(input: &str) -> &str {
    // Look for ```json or ``` at start
    if input.starts_with("```") {
        // Find the first newline (end of opening fence)
        let start = input.find('\n').unwrap_or(0) + 1;

        // Find the closing ```
        let end = input[start..].rfind("```").unwrap_or(input.len());

        return &input[start..start + end];
    }
    input
}
```

**Example**:
```
Input:  "```json\n{\"x\": 1}\n```"
After:  "{\"x\": 1}"
```

#### Fix 2: Trailing Commas

**Problem**: `{"a": 1, "b": 2,}`  ← Extra comma before `}`

**Solution**: When we see `}` or `]`, check if the last character was `,`. If so, ignore it.

**Algorithm**:
```rust
fn process_closing_bracket(&mut self, bracket: char) {
    // Just complete the collection - trailing comma is ignored naturally
    // because we only look at keys and values, not separators
    self.complete_collection();
}
```

**Why This Works**: We never store commas in our collections. We only store keys and values. So trailing commas are automatically ignored.

#### Fix 3: Single Quotes

**Problem**: `{'name': 'Alice'}`  ← Single quotes instead of double

**Solution**: Track single-quoted strings the same as double-quoted.

**Algorithm**:
```rust
match current_char {
    '"' => push_to_stack(QuotedString),      // Double quotes
    '\'' => push_to_stack(SingleQuotedString), // Single quotes
}
```

**Example**:
```
Input:  {'name': 'Alice'}
Parse:  {"name": "Alice"}  ← Converted to standard JSON
```

---

### Level 3: The Tricky Part - Knowing When Strings End

This is the HARDEST problem in the entire parser.

#### Problem Statement

**Given**: You're inside a string and you see a quote character.
**Question**: Does this quote END the string, or is it PART of the string?

**Examples**:

```json
// Case 1: Quote ends string
{"name": "Alice", "age": 30}
              ↑ This quote ENDS the string

// Case 2: Quote is escaped (part of string)
{"quote": "She said \"hello\""}
                     ↑ This quote is PART of the string

// Case 3: LLM messed up escaping
{"text": "He said "hello" to me", "next": ...}
                  ↑ Should this end the string?
```

#### Solution Part 1: Track Backslashes

**Rule**: A quote is escaped if preceded by ODD number of backslashes.

```
"foo\"bar"     ← 1 backslash (odd)  → quote is ESCAPED
"foo\\"        ← 2 backslashes (even) → quote is NOT escaped
"foo\\\"bar"   ← 3 backslashes (odd)  → quote is ESCAPED
```

**Naive Implementation** (O(n²) - slow):
```rust
fn is_escaped(s: &str, quote_pos: usize) -> bool {
    let mut backslashes = 0;
    let mut i = quote_pos - 1;

    // Count backslashes going backwards
    while i >= 0 && s[i] == '\\' {
        backslashes += 1;
        i -= 1;
    }

    backslashes % 2 == 1  // Odd = escaped
}
```

**Why It's Slow**: For every quote, we scan backwards. If you have 1000 quotes in a string, you scan backwards 1000 times.

**Optimized Implementation** (O(n) - fast):
```rust
struct QuoteTracker {
    consecutive_backslashes: usize,
}

impl QuoteTracker {
    // Called once per character as we move forward
    fn process_char(&mut self, ch: char) {
        if ch == '\\' {
            self.consecutive_backslashes += 1;
        } else {
            // Check if this quote is escaped
            if ch == '"' {
                let is_escaped = self.consecutive_backslashes % 2 == 1;
                // Use this information...
            }
            self.consecutive_backslashes = 0;  // Reset
        }
    }
}
```

**Why It's Fast**: We track backslashes as we go. No need to scan backwards. Just check the count we've been maintaining.

#### Solution Part 2: Context Awareness

Even if a quote is NOT escaped, we still might not want to close the string. We need to look at the CONTEXT.

**Rule**: Check what comes AFTER the quote.

```rust
fn should_close_string(&self, next_char: char) -> bool {
    // We're building an object key
    if in_object_key() {
        match next_char {
            ':' => true,   // "key": ...  ← Definitely end of key
            '}' => true,   // "key"}      ← End of object, so end of key
            _ => false,
        }
    }

    // We're building an object value
    else if in_object_value() {
        match next_char {
            ',' => true,   // "value", "next": ... ← End of value
            '}' => true,   // "value"}             ← End of object
            _ => false,
        }
    }

    // We're building an array element
    else if in_array() {
        match next_char {
            ',' => true,   // "elem", ...  ← Next element
            ']' => true,   // "elem"]      ← End of array
            _ => false,
        }
    }
}
```

**How Do We Know the Context?**

Look at the second-to-last item on the stack:

```
Stack: [Object, String]
         ↑       ↑
      parent  current

Parent is Object → We're either in a key or value
- If keys.len() == values.len() → Expecting key
- If keys.len() == values.len() + 1 → Expecting value
```

**Example**:

```json
{"name": "Alice has a "cat" named Fluffy", "age": 30}
```

```
Position 1: See first quote after "name":
  Stack: [Object, String]
  String contains: "Alice has a "
  Next char: 'c'
  Context: In object value
  Decision: 'c' is not ',' or '}', so DON'T close

Position 2: See second quote before "cat":
  Stack: [Object, String]
  String contains: "Alice has a "cat"
  Next char: ' '
  Context: In object value
  Decision: ' ' is not ',' or '}', so DON'T close

Position 3: See quote after "Fluffy":
  Stack: [Object, String]
  String contains: "Alice has a "cat" named Fluffy"
  Next char: ','
  Context: In object value
  Decision: ',' means next key coming, so CLOSE! ✓
```

#### Solution Part 3: Lookahead Through Whitespace

Sometimes there's whitespace between the quote and the structural character.

```json
{"name": "Alice"    ,    "age": 30}
              ↑     ↑
           quote  comma
```

**Algorithm**:
```rust
fn should_close_string(&self, mut next: Iterator) -> bool {
    // Skip whitespace
    while let Some(ch) = next.peek() {
        if ch.is_whitespace() {
            next.next();  // Skip it
            continue;
        }

        // Now check the first non-whitespace character
        return matches!(ch, ',' | '}' | ']' | ':');
    }

    // Reached end of string
    true
}
```

---

### Level 4: Unquoted Strings (The Hardest Part)

#### The Problem

LLMs sometimes forget quotes entirely:

```json
{"name": Alice, "role": engineer, "age": 30}
         ↑           ↑
    No quotes!   No quotes!
```

**Question**: How do we know "Alice" is a complete value?

**Answer**: Look for terminating characters: `,`, `}`, `]`

#### Basic Algorithm

```rust
fn parse_unquoted_string(&mut self, start_char: char) -> String {
    let mut result = String::from(start_char);

    loop {
        let ch = peek_next_char();

        match ch {
            // Structural characters terminate the string
            ',' | '}' | ']' | ':' => break,

            // Whitespace might terminate (need lookahead)
            ' ' | '\t' | '\n' => {
                if next_structural_char_is_close() {
                    break;
                } else {
                    result.push(ch);
                }
            }

            // Everything else is part of the string
            _ => result.push(ch),
        }
    }

    result
}
```

#### The Ambiguity Problem

**Tricky Case**: Commas can be INSIDE values or BETWEEN values.

```json
// Case 1: Comma is separator
{"city": Seattle, "state": WA}
                ↑ STOP here

// Case 2: Comma is part of value
{"address": 123 Main St, Apt 4, "zip": 98101}
                       ↑ DON'T stop here!
                                ↑ Stop here
```

**How to Decide?**

Use heuristics:

```rust
fn should_comma_terminate(&self, current_value: &str) -> bool {
    // If value looks like a JSON primitive, comma terminates
    let looks_like_primitive =
        current_value.parse::<f64>().is_ok() ||  // Number
        current_value == "true" ||
        current_value == "false" ||
        current_value == "null" ||
        !current_value.contains(' ');  // Single word

    if looks_like_primitive {
        return true;  // Comma ends the value
    }

    // Value has spaces - might be multiple words
    // Look ahead: if next thing is a quote or comment, comma ends
    let next = peek_after_comma();
    if next == '"' || next == '/' {  // " starts key, / starts comment
        return true;
    }

    // Otherwise, comma is probably part of the value
    false
}
```

**Example Walkthrough**:

```json
{"city": Seattle, "state": WA}
```

```
Step 1: Start parsing unquoted string "Seattle"
  Current: "Seattle"
  Next: ','
  Heuristic: "Seattle" has no spaces → looks like identifier
  Decision: Comma terminates → Close string

Step 2: Start parsing unquoted string "WA"
  Current: "WA"
  Next: '}'
  Decision: } always terminates → Close string
```

```json
{"address": 123 Main St, Apt 4, "zip": 98101}
```

```
Step 1: Start parsing unquoted string
  Current: "123"
  Next: ' ' (space)
  Heuristic: "123" looks like number, but next is space not comma
  Decision: Continue, add space

Step 2: Continue parsing
  Current: "123 Main"
  Next: ' '
  Decision: Continue

Step 3: Continue parsing
  Current: "123 Main St"
  Next: ','
  Heuristic: Has spaces, so not a primitive
  Look ahead after comma: ' ' → 'A' (Apt)
  'A' is not '"' or '/', so comma is part of value
  Decision: Continue, add comma

Step 4: Continue parsing
  Current: "123 Main St, Apt"
  Next: ' '
  Decision: Continue

Step 5: Continue parsing
  Current: "123 Main St, Apt 4"
  Next: ','
  Look ahead: ' ' → '"' (starts "zip")
  Found a quote! Comma must separate values
  Decision: Close string here
```

---

### Level 5: Auto-Completion (Handling Incomplete Input)

#### The Problem

LLMs often stop mid-output (streaming, token limit, etc.):

```json
{"name": "Alice", "items": [1, 2, 3
```

Missing: `]`, `}`

#### The Solution

When input ends, forcibly close all open collections:

```rust
fn finish_parsing(&mut self) {
    // Close everything on the stack
    while !self.stack.is_empty() {
        self.complete_collection(CompletionState::Incomplete);
    }
}
```

**Example**:

```
Input: {"name": "Alice", "items": [1, 2, 3
                                            ↑ Input ends here

Stack at EOF:
  [Object{"name": "Alice", "items": ...}, Array[1, 2, 3]]

Step 1: Pop Array
  Array marked as Incomplete
  Result so far: [1, 2, 3]  (incomplete)
  Add to parent Object: {"name": "Alice", "items": [1, 2, 3]}

Step 2: Pop Object
  Object marked as Incomplete
  Final result: {"name": "Alice", "items": [1, 2, 3]}
                                                       ↑ Missing ]} added
```

#### Tracking Completion State

Each value has a `CompletionState`:

```rust
enum CompletionState {
    Complete,    // Value fully formed
    Incomplete,  // Value was auto-completed
}
```

```rust
enum Value {
    String(String, CompletionState),
    Number(f64, CompletionState),
    Object(Vec<(String, Value)>, CompletionState),
    Array(Vec<Value>, CompletionState),
    // ...
}
```

**Usage**:

```rust
let result = parse(r#"{"name": "Alice", "age": 30"#);  // Missing }

match result {
    Value::Object(fields, CompletionState::Incomplete) => {
        println!("Warning: Object was incomplete");
        // Maybe retry, or ask LLM to continue
    }
    Value::Object(fields, CompletionState::Complete) => {
        println!("Success: Complete object");
    }
}
```

---

### Level 6: Putting It All Together

#### The Complete Algorithm

```rust
struct JsonParser {
    stack: Vec<Collection>,           // What we're building
    completed: Vec<Value>,             // What we've finished
    quote_tracker: QuoteTracker,       // Track backslashes
}

impl JsonParser {
    fn parse(input: &str) -> Result<Value> {
        let mut parser = Self::new();

        // 1. Try fast path first
        if let Ok(v) = serde_json::from_str(input) {
            return Ok(v);
        }

        // 2. Try stripping markdown
        let input = strip_markdown(input);

        // 3. Parse character by character
        for (i, ch) in input.chars().enumerate() {
            parser.process_char(ch, &input[i+1..]);
        }

        // 4. Auto-complete anything left open
        while !parser.stack.is_empty() {
            parser.complete_collection(Incomplete);
        }

        // 5. Return result
        Ok(parser.completed.pop().unwrap())
    }

    fn process_char(&mut self, ch: char, rest: &str) {
        match self.stack.last() {
            None => {
                // Not inside anything - look for start of value
                match ch {
                    '{' => self.stack.push(Object::new()),
                    '[' => self.stack.push(Array::new()),
                    '"' => self.stack.push(String::new()),
                    _ => self.stack.push(UnquotedString::from(ch)),
                }
            }

            Some(Object) => {
                match ch {
                    '}' => self.complete_collection(Complete),
                    ',' | ':' => { /* Ignore separators */ }
                    _ => self.start_new_value(ch),
                }
            }

            Some(Array) => {
                match ch {
                    ']' => self.complete_collection(Complete),
                    ',' => { /* Ignore separators */ }
                    _ => self.start_new_value(ch),
                }
            }

            Some(String) => {
                self.quote_tracker.process(ch);

                if ch == '"' && self.should_close_string(rest) {
                    self.complete_collection(Complete);
                } else {
                    self.current_string().push(ch);
                }
            }

            Some(UnquotedString) => {
                if self.should_terminate_unquoted(ch, rest) {
                    self.complete_collection(Complete);
                    self.process_char(ch, rest);  // Reprocess this char
                } else {
                    self.current_string().push(ch);
                }
            }
        }
    }
}
```

#### Visual Example: Complete Parse

```
Input: {"name": "Alice", "age": 30}

Char | Stack State                           | Action
─────┼─────────────────────────────────────┼──────────────────────
{    | [Object{}]                          | Push Object
"    | [Object{}, String""]                | Push String
n    | [Object{}, String"n"]               | Append to string
a    | [Object{}, String"na"]              | Append to string
m    | [Object{}, String"nam"]             | Append to string
e    | [Object{}, String"name"]            | Append to string
"    | [Object{"name": ...}]               | Close String, add as key
:    | [Object{"name": ...}]               | Ignore separator
"    | [Object{...}, String""]             | Push String
A    | [Object{...}, String"A"]            | Append to string
l    | [Object{...}, String"Al"]           | Append to string
i    | [Object{...}, String"Ali"]          | Append to string
c    | [Object{...}, String"Alic"]         | Append to string
e    | [Object{...}, String"Alice"]        | Append to string
"    | [Object{"name": "Alice", ...}]      | Close String, add as value
,    | [Object{"name": "Alice", ...}]      | Ignore separator
"    | [Object{...}, String""]             | Push String
a    | [Object{...}, String"a"]            | Append to string
g    | [Object{...}, String"ag"]           | Append to string
e    | [Object{...}, String"age"]          | Append to string
"    | [Object{..."age": ...}]             | Close String, add as key
:    | [Object{..."age": ...}]             | Ignore separator
3    | [Object{...}, Unquoted"3"]          | Push Unquoted
0    | [Object{...}, Unquoted"30"]         | Append to unquoted
}    | []                                   | Close Unquoted(30), Close Object

Final: {"name": "Alice", "age": 30}
```

---

### Level 7: Key Takeaways for Implementation

#### 1. Use a Stack

The stack is your friend. It tells you:
- What you're currently building
- What context you're in (object key vs value vs array)
- What to do when you see closing brackets

#### 2. Character-by-Character Processing

Don't try to tokenize first. Process one character at a time and make decisions based on:
- Current state (top of stack)
- Current character
- Lookahead (peek at next characters)

#### 3. Lookahead is Essential

Many decisions require peeking ahead:
- Is `,` a separator or part of a value?
- Does `"` close a string or is it escaped?
- Is `"""` a triple-quoted string?

#### 4. Context Awareness

The same character means different things in different contexts:
- `,` in object → next key coming
- `,` in array → next element coming
- `,` in unquoted string → maybe part of value?

#### 5. Heuristics for Ambiguity

When you can't be sure, use heuristics:
- Does it look like a number? → Probably complete value
- Does it have spaces? → Probably multi-word value
- What comes next? → Peek ahead

#### 6. Track Metadata

Don't just track values, track metadata:
- `CompletionState` → Was this auto-completed?
- `Fixes` → What did we fix?
- `Position` → Where in input (for error messages)

#### 7. Fast Path for Valid JSON

Always try standard parser first:
```rust
if let Ok(v) = serde_json::from_str(input) {
    return Ok(v);  // Fast path!
}
// Otherwise, use our complex parser
```

---

### Exercises to Test Understanding

#### Exercise 1: Manual Parsing

Parse this by hand using the stack approach:

```json
{"x": [1, 2], "y": 3}
```

Show the stack state after each character.

<details>
<summary>Solution</summary>

```
{ → [Object{}]
" → [Object{}, String]
x → [Object{}, String"x"]
" → [Object{"x": ...}]
: → [Object{"x": ...}]
[ → [Object{...}, Array[]]
1 → [Object{...}, Array[], Unquoted]
  → [Object{...}, Array[1]]
, → [Object{...}, Array[1]]
2 → [Object{...}, Array[1], Unquoted]
] → [Object{"x": [1, 2]}]
, → [Object{"x": [1, 2]}]
" → [Object{...}, String]
y → [Object{...}, String"y"]
" → [Object{"x": [1, 2], "y": ...}]
: → [Object{"x": [1, 2], "y": ...}]
3 → [Object{...}, Unquoted]
} → [{"x": [1, 2], "y": 3}]
```
</details>

#### Exercise 2: Should Close String?

For each case, decide if the quote should close the string:

```json
// Case A
{"name": "Alice", "age": 30}
              ↑ Close? YES/NO

// Case B
{"quote": "He said \"hi\""}
                   ↑ Close? YES/NO

// Case C
{"text": "Line 1\n Line 2", "x": 1}
                          ↑ Close? YES/NO

// Case D
{"broken": "foo"bar", "next": 2}
                ↑ Close? YES/NO
```

<details>
<summary>Solutions</summary>

A: YES - Next char is ',', we're in object value
B: NO - Backslash escapes the quote
C: YES - Next char is ',', we're in object value
D: NO - Next char is 'b', not structural character (this is broken JSON, but we keep accumulating)
</details>

#### Exercise 3: Comma Disambiguation

Decide if comma should terminate the unquoted string:

```json
// Case A
{"city": Seattle, "state": WA}
                ↑ Terminate? YES/NO

// Case B
{"address": 123 Main St, Apt 4, "zip": 98101}
                       ↑ Terminate? YES/NO

// Case C
{"numbers": 1, 2, 3, "total": 6}
             ↑ Terminate? YES/NO
```

<details>
<summary>Solutions</summary>

A: YES - "Seattle" is single word (identifier), comma followed by quote
B: NO - "123 Main St" has spaces (multi-word), comma NOT followed by quote
C: YES - "1" is number (primitive), comma followed by number
</details>

---

## Schema-Aligned Parsing (Coercion Engine)

**Location**: `/engine/baml-lib/jsonish/src/deserializer/coercer/`

### Three-Phase Process

1. **Parse**: Extract structured data (jsonish parser)
2. **Coerce**: Match to target schema with fallbacks
3. **Score**: Track how many "fixes" were applied

### Coercion Strategies

#### 1. Type Coercion (`coerce_primitive.rs`)

```rust
// String → Int
"42" → 42

// Float → Int
42.7 → 42 (with flag: FloatToInt)

// String → Bool
"true" → true
"1" → true
"yes" → true (fuzzy match)

// Any → String (fallback)
{"key": "value"} → "{\"key\": \"value\"}" (JSON stringified)
```

#### 2. Field Matching (`coerce_object.rs`)

**Priority Order**:

1. **Exact match**: Field name matches exactly
2. **Case-insensitive**: "firstName" matches "firstname"
3. **Alias**: "skills" matches "abilities" (via `@alias(abilities)`)
4. **Snake/Camel conversion**: "first_name" matches "firstName"
5. **Fuzzy match**: Levenshtein distance (e.g., "usrName" → "userName")

```rust
fn find_field(&self, object: &JsonObject, field_name: &str) -> Option<&JsonValue> {
    // Try exact match
    if let Some(val) = object.get(field_name) {
        return Some(val);
    }

    // Try case-insensitive
    for (key, val) in object.iter() {
        if key.eq_ignore_ascii_case(field_name) {
            return Some(val);
        }
    }

    // Try snake/camel conversion
    let snake = to_snake_case(field_name);
    let camel = to_camel_case(field_name);
    if let Some(val) = object.get(&snake).or_else(|| object.get(&camel)) {
        return Some(val);
    }

    // Try fuzzy match (Jaro-Winkler distance)
    let matches: Vec<_> = object.keys()
        .filter(|k| jaro_winkler(k, field_name) > 0.8)
        .collect();

    if matches.len() == 1 {
        return object.get(matches[0]);
    }

    None
}
```

#### 3. Union Resolution (`coerce_union.rs`)

For union types like `Result = Success | Error`:

```rust
fn coerce_union(&self, value: JsonValue, schemas: &[Schema]) -> Result<CoercionResult> {
    let mut results = Vec::new();

    // Try each variant
    for schema in schemas {
        let mut flags = Vec::new();
        match self.coerce_to_schema(value.clone(), schema, &mut flags) {
            Ok(coerced) => {
                let score = calculate_confidence(&flags);
                results.push((coerced, score, flags));
            }
            Err(_) => continue,
        }
    }

    // Pick the variant with highest confidence (lowest score = fewer fixes)
    results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());

    results.into_iter().next()
        .ok_or(CoercionError::NoMatchingVariant)
}
```

**Optimization - "Hint" System**:
If array elements are homogeneous (all same type), try the same variant first for subsequent elements.

```rust
// Array: [{"type": "user", ...}, {"type": "user", ...}]
// After first element matches "User" variant, try User first for rest
```

#### 4. Default Values

```rust
// Schema definition
class Config {
  timeout int @default(30)
  retries int @default(3)
}

// Parsing
// Input: {}
// Output: Config { timeout: 30, retries: 3 }
// Flags: [UsedDefaultValue("timeout"), UsedDefaultValue("retries")]
```

#### 5. Array Wrapping/Unwrapping

```rust
// Schema expects: string[]
// Input: "single string"
// Output: ["single string"]
// Flag: WrappedInArray

// Schema expects: string
// Input: ["single string"]
// Output: "single string"
// Flag: UnwrappedFromArray
```

---

## Flag System

**Location**: `/engine/baml-lib/jsonish/src/deserializer/deserialize_flags.rs`

### Purpose

Track **every transformation** for:
- Transparency (users see what was changed)
- Confidence scoring (more flags = lower confidence)
- Debugging (understand why parsing succeeded/failed)

### Flag Types

```rust
pub enum Flag {
    // Parsing flags
    ObjectFromMarkdown(i32),           // Stripped ``` wrappers
    ObjectFromFixedJson(Vec<Fixes>),   // Fixed trailing commas, quotes, etc.

    // Type coercion flags
    StringToBool(String),              // "true" → true
    StringToInt(String),               // "42" → 42
    FloatToInt(f64),                   // 42.7 → 42
    IntToFloat(i64),                   // 42 → 42.0

    // Field matching flags
    FuzzyMatch(String, String),        // "usrName" matched "userName"
    AliasMatch(String, String),        // "abilities" matched "skills" (@alias)
    CaseInsensitiveMatch(String),      // "FIRSTNAME" matched "firstName"

    // Default values
    DefaultFromNoValue,                // Used default value for missing field

    // Union resolution
    UnionMatch(usize, Vec<Flag>),      // Matched variant #2, with sub-flags

    // Array operations
    WrappedInArray,                    // Wrapped single value in array
    UnwrappedFromArray,                // Unwrapped single-element array

    // Completeness
    Incomplete,                        // Value is partial (streaming)

    // Failures (still track)
    FailedToParse(String),
    MissingRequiredField(String),
    TypeMismatch { expected: String, found: String },
}
```

### Confidence Scoring

```rust
fn calculate_confidence(flags: &[Flag]) -> f32 {
    let mut score = 1.0;

    for flag in flags {
        score -= match flag {
            Flag::ObjectFromMarkdown(_) => 0.05,       // Minor
            Flag::StringToInt(_) => 0.1,                // Type coercion
            Flag::FuzzyMatch(_, _) => 0.15,             // Fuzzy matching
            Flag::DefaultFromNoValue => 0.2,            // Missing field
            Flag::TypeMismatch { .. } => 0.5,           // Major issue
            Flag::Incomplete => 0.3,                    // Partial data
            _ => 0.0,
        };
    }

    score.max(0.0)  // Clamp to [0, 1]
}
```

**Usage**:
```rust
let result = parser.parse_and_coerce(input, schema)?;

if result.confidence < 0.7 {
    // Low confidence, maybe retry or fallback
    warn!("Low confidence parse: {:?}", result.flags);
}

// Log all transformations for debugging
for flag in result.flags {
    debug!("Transformation: {:?}", flag);
}
```

---

## Streaming Support

**Location**: `/engine/baml-lib/jsonish/src/jsonish/parser/stream.rs`

### Incremental Parsing

Challenge: Parse **incomplete JSON** as it streams in.

```rust
pub struct StreamingParser {
    parser: FixingParser,
    buffer: String,
    emitted_index: usize,  // Track what we've already emitted
}

impl StreamingParser {
    pub fn feed(&mut self, chunk: &str) -> Vec<PartialValue> {
        self.buffer.push_str(chunk);

        // Try to parse current buffer
        match self.parser.parse_incremental(&self.buffer) {
            ParseState::Complete { value } => {
                // Full object parsed
                vec![PartialValue::Complete(value)]
            }
            ParseState::Incomplete { depth, .. } => {
                // Extract completed prefix
                self.extract_partial()
            }
            ParseState::Invalid { .. } => {
                vec![]  // Need more data
            }
        }
    }

    fn extract_partial(&mut self) -> Vec<PartialValue> {
        // For arrays: extract completed elements
        // Example: "[{\"id\":1},{\"id\":2},{\"id\":"
        //          ↑ Can emit first two objects

        // For objects: extract completed fields
        // Example: "{\"name\":\"Alice\",\"age\":"
        //          ↑ Can emit name field

        let extracted = self.find_completed_values();
        self.emitted_index += extracted.len();
        extracted
    }
}
```

### Partial Types

BAML generates **partial types** for streaming:

```baml
// Original type
class Resume {
  name string
  email string
  skills string[]
}

// Generated partial type (all fields Optional)
class PartialResume {
  name: Option<String>,
  email: Option<String>,
  skills: Option<Vec<String>>,
}
```

**Streaming Annotations**:

```baml
class StreamedData {
  id int @@stream.not_null      // Don't emit until non-null
  status string @@stream.done   // Only emit when complete
  items Item[]                  // Emit as items arrive
}
```

**Usage**:

```python
stream = client.stream.ExtractData(input_text)

async for partial in stream:
    # partial: PartialData (all fields Optional)
    if partial.id is not None:
        print(f"ID: {partial.id}")

    if partial.items:
        for item in partial.items:
            print(f"Item: {item}")

# Get final result (all required fields populated)
final = await stream.get_final_response()
```

---

## Integration with LLMs

**Location**: `/engine/baml-lib/llm-client/src/clients/`

### Multi-Provider Support

```rust
pub enum LLMProvider {
    OpenAI,
    Anthropic,
    GoogleAI,
    VertexAI,
    AWSBedrock,
    Azure,
    OpenAICompatible,  // Ollama, OpenRouter, VLLM, etc.
}
```

### Client Configuration (BAML DSL)

```baml
client<llm> GPT4 {
  provider openai
  options {
    model "gpt-4"
    api_key env.OPENAI_API_KEY
    http {
      connect_timeout_ms 5000
      request_timeout_ms 30000
    }
  }
}

client<llm> Claude {
  provider anthropic
  options {
    model "claude-3-opus-20240229"
    api_key env.ANTHROPIC_API_KEY
  }
}

// Fallback strategy
client<llm> Resilient {
  provider fallback
  options {
    strategy [GPT4, Claude, GPT35Turbo]
  }
}

// Retry policy
retry_policy Aggressive {
  max_retries 3
  strategy {
    type exponential_backoff
    initial_delay_ms 100
    max_delay_ms 10000
    multiplier 2.0
  }
}
```

### Prompt Rendering

**Jinja2 Templates**:

```baml
function Summarize {
  input: Article
  output: Summary

  prompt #"
    {% if input.title %}
    Title: {{ input.title }}
    {% endif %}

    Content:
    {{ input.content }}

    Summarize the above article in {{ input.max_words | default(100) }} words.

    Return JSON:
    {{ ctx.output_format }}
  "#
}
```

**Automatic Schema Injection**:
```
{{ ctx.output_format }} →
{
  "summary": "string",
  "key_points": ["string"],
  "sentiment": "positive" | "negative" | "neutral"
}
```

### Native Tool Calling vs SAP

BAML supports **two modes**:

1. **Native Tool Calling** (models that support it)
   - OpenAI function calling
   - Anthropic tool use
   - Directly use provider's structured output

2. **Schema-Aligned Parsing** (all models)
   - Inject schema in prompt
   - Parse LLM's text response
   - Coerce to target schema
   - Works with any model (even those without tool calling)

This makes BAML work with **all LLMs**, not just those with native structured output.

---

## Type System

**Location**: `/engine/baml-lib/baml-types/src/`

### Primitive Types

```baml
string   // UTF-8 text
int      // 64-bit integer
float    // 64-bit float
bool     // true/false
null     // null value
```

### Composite Types

```baml
// Class (object)
class Person {
  name string
  age int
}

// Enum
enum Status {
  Pending
  Approved
  Rejected
}

// List
string[]
Person[]

// Map
map<string, int>

// Union
string | int | null
Success | Error
```

### Type Modifiers

```baml
// Optional
string?
int?

// Required (default)
string
int

// Literal
"specific_value"   // Only this exact string allowed
42                 // Only this exact number
```

### Metadata Attributes

```baml
class User {
  id int @description("Unique user identifier")
  email string @alias(email_address)
  age int @assert({{ _ >= 0 && _ <= 150 }})
  role string @check({{ _ in ["admin", "user", "guest"] }})

  @@stream.not_null     // Class-level: don't emit until all required fields set
  @@stream.done         // Class-level: only emit when fully complete
}
```

### Validation

```baml
class Config {
  timeout int @assert({{ _ > 0 && _ <= 3600 }})
  retries int @assert({{ _ >= 0 && _ <= 10 }})
  url string @check({{ _.starts_with("https://") }})
}
```

**Jinja2 Expressions**:
- `{{ _ }}` refers to the field value
- Full Jinja2 syntax available
- Runs at runtime after parsing

---

## Generated Clients

BAML compiles to **language-specific clients**:

### Python

```python
from baml_client import b

# Synchronous
result: Resume = b.ExtractResume(resume_text)

# Async
result: Resume = await b.ExtractResume(resume_text)

# Streaming
stream = b.stream.ExtractResume(resume_text)
async for partial in stream:
    # partial: PartialResume (all Optional fields)
    if partial.name:
        print(f"Name: {partial.name}")

final: Resume = await stream.get_final_response()
```

### TypeScript

```typescript
import { b } from './baml_client';

// Async
const result: Resume = await b.ExtractResume(resumeText);

// Streaming
const stream = b.stream.ExtractResume(resumeText);
for await (const partial of stream) {
  // partial: Partial<Resume>
  if (partial.name) {
    console.log(`Name: ${partial.name}`);
  }
}

const final = await stream.finalValue();
```

### Ruby

```ruby
require 'baml_client'

# Synchronous
result = Baml.ExtractResume(resume_text)
puts result.name

# Streaming
Baml.stream.ExtractResume(resume_text) do |partial|
  puts "Name: #{partial.name}" if partial.name
end
```

### Go

```go
import "github.com/yourorg/baml-client"

// Synchronous
result, err := baml.ExtractResume(resumeText)
if err != nil {
    log.Fatal(err)
}
fmt.Println(result.Name)
```

---

## Key Insights for SimpleAgents (Rust)

### 1. **Flexible JSON Parser is Essential**

The jsonish parser is BAML's killer feature:
- State machine for incremental parsing
- Auto-completion of partial structures
- Multiple string delimiter support
- Handles all common LLM mistakes

**Rust Implementation**:
- Can use similar state machine approach
- Leverage Rust's pattern matching
- Zero-copy parsing where possible

### 2. **Coercion Must Be Configurable**

Users need control over strictness:
- Strict mode: fail on any coercion
- Lenient mode (default): try hard to parse
- Confidence threshold: reject if too many fixes

**Rust Implementation**:
- Builder pattern for configuration
- Feature flags for coercion strategies
- Transparent flag system

### 3. **Streaming Requires Partial Types**

Can't wait for full response in streaming:
- Need to emit partial data
- All fields become `Option<T>`
- Track completion state

**Rust Implementation**:
- Use `Option<T>` for partial types
- Derive macros could auto-generate partial types
- Async streams with `futures::Stream`

### 4. **Union Resolution is Complex**

Trying all variants and scoring:
- Can be expensive (n variants = n parse attempts)
- Caching/memoization helps
- Hint system optimizes array parsing

**Rust Implementation**:
- Use `rayon` for parallel variant checking
- Cache schema representations
- Smart heuristics for variant selection

### 5. **Type System Should Be Rich**

Support for:
- Primitives, composites, unions
- Optional and required fields
- Defaults and validation
- Aliases

**Rust Implementation**:
- Proc macros for schema definition
- Compile-time validation where possible
- Runtime checks for dynamic validation

### 6. **Provider Abstraction**

Support both:
- Native tool calling (when available)
- Text-based parsing (fallback)

**Rust Implementation**:
- Provider trait with capabilities
- Automatic mode selection
- Schema injection for text mode

---

## Recommendations for SimpleAgents (Rust)

### Must Have (MVP)

1. **Jsonish Parser**
   - Markdown stripping
   - Trailing comma fixes
   - Basic auto-completion
   - Flag tracking

2. **Basic Coercion**
   - Primitive type coercion
   - Field matching (exact, case-insensitive)
   - Default values

3. **Flag System**
   - Track all transformations
   - Basic confidence scoring

4. **Streaming**
   - Incremental parsing
   - Partial types with `Option<T>`

### Nice to Have (Post-MVP)

1. **Advanced Coercion**
   - Fuzzy field matching (Levenshtein)
   - Union resolution with scoring
   - Array wrapping/unwrapping

2. **Rich Type System**
   - Derive macros for schemas
   - Custom validation (Jinja2-like)
   - Literal types

3. **Multiple String Delimiters**
   - Single quotes, triple quotes
   - Template strings

4. **Constraint System**
   - `@assert`, `@check` attributes
   - Custom validation functions

### Avoid for MVP

1. Full Jinja2 templating (use simpler alternatives)
2. Multiple provider fallbacks (focus on quality)
3. Complex DSL (use Rust structs + macros instead)
4. Web UI / Playground

---

## Code Statistics

- **Jsonish Parser**: ~5,000 lines (Rust)
- **Coercion Engine**: ~3,000 lines (Rust)
- **Type System**: ~2,000 lines (Rust)
- **LLM Clients**: ~4,000 lines (Rust)
- **Code Generation**: ~6,000 lines (Rust)

**Total Core**: ~20,000 lines of Rust

---

## Critical Files for Reference

1. `/engine/baml-lib/jsonish/src/jsonish/parser/fixing_parser/mod.rs`
   - State machine parser
   - Core parsing logic

2. `/engine/baml-lib/jsonish/src/deserializer/coercer/coerce_union.rs`
   - Union resolution
   - Variant scoring

3. `/engine/baml-lib/jsonish/src/deserializer/deserialize_flags.rs`
   - Flag definitions
   - Confidence scoring

4. `/engine/baml-lib/jsonish/src/jsonish/parser/stream.rs`
   - Streaming support
   - Partial value extraction

5. `/engine/baml-lib/llm-client/src/clients/`
   - Provider implementations
   - Client configuration

---

## Conclusion

BAML's response healing is the **key differentiator** for production LLM applications. The jsonish parser + coercion engine make structured outputs reliable across any model. SimpleAgents should prioritize building a robust parsing system with transparent flag tracking, as this is what will make it valuable in real-world use.

**Key Takeaway**: It's not about perfect parsing, it's about **transparent healing with confidence scores** so users can make informed decisions about whether to accept, retry, or fallback.
