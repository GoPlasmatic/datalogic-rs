# Playground

Try JSONLogic expressions right in your browser! This playground uses the same WebAssembly-compiled engine that powers the Rust library.

<div id="full-playground"></div>

## How to Use

1. **Logic**: Enter your JSONLogic expression in the Logic pane
2. **Data**: Enter the JSON data to evaluate against in the Data pane
3. **Run**: Press the Run button or use **Ctrl+Enter** (Cmd+Enter on Mac)
4. **Examples**: Use the dropdown to load pre-built examples

## Quick Reference

### Basic Operators

| Operator | Example | Description |
|----------|---------|-------------|
| `var` | `{"var": "x"}` | Access variable |
| `==` | `{"==": [1, 1]}` | Equality |
| `>`, `<`, `>=`, `<=` | `{">": [5, 3]}` | Comparison |
| `and`, `or` | `{"and": [true, true]}` | Logical |
| `if` | `{"if": [cond, then, else]}` | Conditional |
| `+`, `-`, `*`, `/` | `{"+": [1, 2]}` | Arithmetic |

### Array Operations

| Operator | Example | Description |
|----------|---------|-------------|
| `map` | `{"map": [arr, expr]}` | Transform elements |
| `filter` | `{"filter": [arr, cond]}` | Filter elements |
| `reduce` | `{"reduce": [arr, expr, init]}` | Reduce to value |
| `all`, `some`, `none` | `{"all": [arr, cond]}` | Check conditions |

### String Operations

| Operator | Example | Description |
|----------|---------|-------------|
| `cat` | `{"cat": ["a", "b"]}` | Concatenate |
| `substr` | `{"substr": ["hello", 0, 2]}` | Substring |
| `in` | `{"in": ["@", "a@b.com"]}` | Contains |

## Example: Feature Flag

Determine if a user has access to a premium feature:

```json
{
  "and": [
    {"==": [{"var": "user.plan"}, "premium"]},
    {">=": [{"var": "user.accountAge"}, 30]}
  ]
}
```

Data:
```json
{
  "user": {
    "plan": "premium",
    "accountAge": 45
  }
}
```

## Example: Dynamic Pricing

Calculate a discounted price based on quantity:

```json
{
  "if": [
    {">=": [{"var": "quantity"}, 100]},
    {"*": [{"var": "price"}, 0.8]},
    {"if": [
      {">=": [{"var": "quantity"}, 50]},
      {"*": [{"var": "price"}, 0.9]},
      {"var": "price"}
    ]}
  ]
}
```

Data:
```json
{
  "quantity": 75,
  "price": 100
}
```

## Learn More

- [Operators Overview](operators/overview.md) - Full operator documentation
- [Getting Started](getting-started/quick-start.md) - Using the library
- [Use Cases](use-cases/examples.md) - Real-world examples
