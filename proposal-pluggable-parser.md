# Proposal: Pluggable Expression Evaluation Architecture for datalogic-rs

## Overview

This document outlines an architecture for enhancing the `datalogic-rs` library into a versatile expression evaluation engine. By introducing a pluggable parser system, the library will support multiple expression languages (like JSONLogic, JSONata, and others) through a unified evaluation engine, optimizing performance and extensibility.

## Goals

- **Extensibility:** Support multiple expression syntaxes seamlessly.
- **Maintainability:** Clearly separate parsing logic from core evaluation logic.
- **Performance:** Share an optimized evaluation engine across expression formats.
- **Ease of Use:** Provide intuitive APIs for parsing, evaluation, and extension.
- **Backward Compatibility:** Maintain JSONLogic support as a default parser.

## Architectural Components

### 1. Core Evaluation Engine

The evaluation engine remains agnostic of expression syntax and operates on a unified internal representation of rules.

- Optimized with arena allocation (`DataArena`) for memory efficiency.
- Evaluates `Rule` objects constructed by parsers or programmatically via `RuleBuilder`.

```rust
pub trait ExpressionEvaluator {
    fn evaluate(&self, rule: &Rule, data: &Value) -> Result<Value, EvalError>;
}
```

### 2. RuleBuilder

A fluent API that constructs internal `Rule` representations programmatically, essential for both manual rule construction and parser implementations.

- Uses arena-backed allocation for efficiency.

```rust
let rule = builder
    .compare()
    .greater_than()
    .var("score")
    .value(50)
    .build();
```

### 3. Parser Trait and Implementations

Defines a common interface for parsing various expression syntaxes into the internal `Rule` representation.

```rust
pub trait ExpressionParser {
    fn parse(&self, input: &str, builder: &RuleBuilder) -> Result<Rule, ParseError>;
    fn format_name(&self) -> &'static str;
}
```

#### Example JSONLogic Parser

```rust
pub struct JsonLogicParser;

impl ExpressionParser for JsonLogicParser {
    fn parse(&self, input: &str, builder: &RuleBuilder) -> Result<Rule, ParseError> {
        // Parsing logic from JSONLogic syntax to internal Rule
    }

    fn format_name(&self) -> &'static str {
        "jsonlogic"
    }
}
```

### 4. Parser Registry

Manages registration and selection of parsers.

```rust
pub struct ParserRegistry {
    parsers: HashMap<String, Box<dyn ExpressionParser>>,
    default_parser: String,
}
```

### 5. DataLogic Main Interface

Centralizes access to parsing, rule construction, and evaluation.

```rust
pub struct DataLogic {
    arena: DataArena,
    evaluator: Box<dyn ExpressionEvaluator>,
    parsers: ParserRegistry,
}

impl DataLogic {
    pub fn new() -> Self {
        // Initialization with default JSONLogic parser
    }

    pub fn register_parser(&mut self, parser: Box<dyn ExpressionParser>) {
        // Parser registration logic
    }

    pub fn parse(&self, source: &str, format: Option<&str>) -> Result<Rule, ParseError> {
        // Parsing logic based on format
    }

    pub fn evaluate(&self, rule: &Rule, data: &Value) -> Result<Value, EvalError> {
        self.evaluator.evaluate(rule, data)
    }
}
```

## Usage Examples

### Default Usage (JSONLogic)

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let logic = DataLogic::new();

let data = json!({"score": 75});
let result = logic.parse("{\">\": [{\"var\": \"score\"}, 50]}", None)
                  .and_then(|rule| logic.evaluate(&rule, &data));

assert_eq!(result.unwrap(), json!(true));
```

### Registering and Using Additional Parsers

```rust
use datalogic_rs::{DataLogic, JsonataParser};

let mut logic = DataLogic::new();
logic.register_parser(Box::new(JsonataParser));

let data = json!({"score": 75});
let jsonata_expr = "score > 50";

let result = logic.parse(jsonata_expr, Some("jsonata"))
                  .and_then(|rule| logic.evaluate(&rule, &data));

assert_eq!(result.unwrap(), json!(true));
```

### Custom Parser Implementation

```rust
struct SimpleExprParser;

impl ExpressionParser for SimpleExprParser {
    fn parse(&self, input: &str, builder: &RuleBuilder) -> Result<Rule, ParseError> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        if parts.len() == 3 && parts[1] == ">" {
            Ok(builder
                .compare()
                .greater_than()
                .var(parts[0])
                .value(parts[2].parse().map_err(|_| ParseError)?)
                .build())
        } else {
            Err(ParseError)
        }
    }

    fn format_name(&self) -> &'static str { "simple" }
}

logic.register_parser(Box::new(SimpleExprParser));
```

## Benefits

- **Architectural Clarity:** Clear separation between parsing, rule construction, and evaluation.
- **Flexibility:** Easy to extend with new parsers and expression formats.
- **Performance:** Shared evaluation engine ensures consistent performance.
- **User-Friendly:** Simplifies creating and using expressions across different languages.

## Implementation Roadmap

1. Develop the parser trait and registry.
2. Refactor existing JSONLogic parsing into a parser implementation.
3. Establish `DataLogic` as the primary interface.
4. Document and provide comprehensive usage examples.
5. Add additional parsers (e.g., JSONata, JMESPath) incrementally.

By adopting this architecture, `datalogic-rs` will become a robust, extensible library, suitable for diverse applications requiring dynamic expression evaluation.

