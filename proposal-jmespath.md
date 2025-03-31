# JMESPath Support for DataLogic: AST Extension Proposal

## Overview

This proposal outlines a plan to add JMESPath support to the DataLogic library by leveraging the existing AST architecture. Rather than introducing numerous new token types and operators, we'll map JMESPath expressions to existing core operators wherever possible and only extend the AST for operations with no current equivalent.

## Background

DataLogic currently implements JSONLogic using a core AST evaluation engine with a JSONLogic-specific parser. The architecture is plugin-based, allowing different expression languages to be parsed into the same AST representation. The core engine already includes powerful operators for data manipulation that can be reused for JMESPath operations.

## Goals

- Add complete JMESPath support to DataLogic
- Maximize reuse of existing operators for JMESPath operations
- Only extend the AST when absolutely necessary
- Ensure backward compatibility with current JSONLogic implementation
- Maintain performance and memory efficiency
- Avoid external dependencies

## Current Architecture

The current DataLogic architecture consists of:

1. A core AST model defined in `src/logic/token.rs` and `src/logic/ast.rs`
2. An evaluation engine in `src/logic/evaluator.rs`
3. A plugin-based parser system in `src/parser/mod.rs`
4. A JSONLogic parser in `src/parser/jsonlogic.rs`
5. Operator implementations in `src/logic/operators/`

## Operator Reuse Strategy

Before adding new token types or operators, we'll map JMESPath operations to existing core operators:

1. **Path access** (`foo.bar`): Use existing variable access mechanisms
2. **Array indexing** (`foo[0]`): Use existing array index operators
3. **Projections** (`foo[*]`): Map to existing array map/filter operators
4. **Filters** (`foo[?bar=='value']`): Use existing array filter operators
5. **Functions**: Map to existing operators where semantics match

## Minimal AST Extensions

Extend the core `Token` enum in `src/logic/token.rs` only for operations that can't be expressed with existing operators:

```rust
pub enum Token<'a> {
    // Existing token types...
    
    // New token types for JMESPath-specific operations
    Slice {
        array: &'a Token<'a>,
        start: Option<&'a Token<'a>>,
        end: Option<&'a Token<'a>>,
        step: Option<&'a Token<'a>>,
    },
    
    Pipe {
        left: &'a Token<'a>,
        right: &'a Token<'a>,
    },
    
    // Only if not mappable to existing object construction mechanisms
    MultiSelectHash {
        source: &'a Token<'a>,
        selections: Vec<(&'a str, &'a Token<'a>)>,
    },
}
```

Similarly, extend the `OperatorType` enum only for necessary new operations:

```rust
pub enum OperatorType {
    // Existing operator types...
    
    // JMESPath specific operators
    Slice,
    Pipe,
    
    // Only for JMESPath functions that can't map to existing operators
    JMESPathFunction(JMESPathFunction),
}

// Only for functions that don't map to existing operators
pub enum JMESPathFunction {
    // Example functions that might need custom implementation
    SortBy,
    Reverse,
    Merge,
    // Other unique JMESPath functions...
}
```

## JMESPath Parser Implementation

Create a new `jmespath.rs` file in the `src/parser` directory that translates JMESPath expressions to existing operators whenever possible:

```rust
pub struct JMESPathParser;

impl ExpressionParser for JMESPathParser {
    fn parse<'a>(&self, input: &str, arena: &'a DataArena) -> Result<&'a Token<'a>> {
        // Parse JMESPath expression and map to existing operators where possible
        parse_jmespath(input, arena)
    }

    fn format_name(&self) -> &'static str {
        "jmespath"
    }
}
```

## JMESPath to Core AST Mapping

The following table shows how JMESPath expressions map to existing core operators:

| JMESPath Feature | Core AST Mapping | Implementation |
|------------------|-----------------|----------------|
| Identifiers (`foo`) | `Token::Variable` | Use existing variable access |
| Subexpressions (`foo.bar`) | Chain of Variable or Property access | Use existing variable/property access |
| Index access (`foo[0]`) | `Token::Operator { op_type: Array(ArrayOp::Index) }` | Use existing array indexing |
| Array projections (`foo[*]`) | `Token::Operator { op_type: Array(ArrayOp::Map) }` | Use existing array map |
| Object projections (`foo.*`) | Map to appropriate object iteration operators | Use existing object/map operations |
| Filters (`foo[?bar=='value']`) | `Token::Operator { op_type: Array(ArrayOp::Filter) }` | Use existing array filter |
| List multi-select (`[foo, bar]`) | `Token::ArrayLiteral` | Use existing array literal construction |
| Hash multi-select (`{a: foo, b: bar}`) | Map to object construction if available | Use existing object construction if possible |
| Functions | Map to existing operators where semantics match | Reuse operators when possible |

## New Required Operations

The following JMESPath operations have no direct equivalent in the core AST and require extensions:

| JMESPath Feature | Required Extension | Reason |
|------------------|-------------------|--------|
| Slices (`foo[0:5]`) | `Token::Slice` | No equivalent slice operation in core |
| Pipes (`foo \| bar`) | `Token::Pipe` | Context passing between expressions not directly supported |
| Some function expressions | Custom functions | Some JMESPath functions have unique semantics |

## Implementation Plan

1. **Phase 1: JMESPath Parser & Mapping Analysis** (1 week)
   - Implement tokenizer for JMESPath syntax
   - Create detailed mapping between JMESPath operations and existing operators
   - Identify gaps requiring new operators
   
2. **Phase 2: Core Operator Mapping** (1 week)
   - Implement parser that maps JMESPath to existing operators
   - Test basic expression evaluation
   
3. **Phase 3: Minimal AST Extensions** (1 week)
   - Implement only necessary new operators (slice, pipe, etc.)
   - Extend evaluator to handle new token types
   
4. **Phase 4: Function Mapping & Extensions** (1 week)
   - Map JMESPath functions to existing operators where possible
   - Implement only unique JMESPath functions as new operators
   - Add comprehensive test suite and documentation

## Function Mapping Strategy

JMESPath has many built-in functions that should map to existing operators where possible:

| JMESPath Function | Core Operator Mapping |
|-------------------|----------------------|
| `abs()` | Map to existing arithmetic abs operator if available |
| `length()` | Map to existing array/string length operators |
| `contains()` | Map to existing string/array contains operators |
| `map()` | Use existing array map operator |
| `sort()` | Use existing array sort operator if available |
| `min()/max()` | Use existing min/max operators |

Only implement new function operators for JMESPath functions with no equivalent in the current system.

## Usage Example

Once implemented, users will be able to use JMESPath expressions as follows:

```rust
let dl = DataLogic::new();
dl.register_parser(Box::new(JMESPathParser));
dl.set_default_parser("jmespath");

// Use JMESPath expression
let result = dl.evaluate_str(
    "people[?age > 18].name", // JMESPath expression
    r#"{"people": [{"name": "Alice", "age": 25}, {"name": "Bob", "age": 16}]}"#,
    None
).unwrap();

// Result will be ["Alice"]
```

## Parser Implementation Strategy

The JMESPath parser will follow these steps:

1. Tokenize JMESPath expression
2. Build a JMESPath syntax tree
3. Transform the syntax tree to DataLogic tokens, preferring existing operators
4. Only create new token types for operations with no core equivalent

For example, a JMESPath expression like `people[?age > 18].name` would map to:
- Array filter operation (existing operator)
- Property access (existing operator)
- No new token types needed

## Conclusion

By maximizing the reuse of existing core operators, we can implement JMESPath support with minimal changes to the AST structure. This approach maintains architectural simplicity, leverages the existing evaluation engine, and ensures consistent behavior across expression languages. New token types and operators will only be introduced for JMESPath features that truly have no equivalent in the current system.
