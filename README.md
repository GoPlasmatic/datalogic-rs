# datalogic-rs

A Rust implementation of [JSONLogic](http://jsonlogic.com) that provides a way to write portable logic rules as JSON.

## Overview

`datalogic-rs` implements the complete JSONLogic specification, allowing you to create, share, and evaluate rules across different platforms while staying true to Rust's safety and performance principles.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
datalogic-rs = "0.1.0"
```

## Quick Example

```rust
use datalogic_rs::JsonLogic;
use serde_json::json;

fn main() {
    let logic = JsonLogic::new();

    // Complex discount rule:
    // - 20% off if cart total > $100 AND user is premium member
    // - OR 10% off if cart has more than 5 items
    let discount_rule = json!({
        "or": [
            {"and": [
                {">": [{"var": "cart.total"}, 100]},
                {"==": [{"var": "user.membership"}, "premium"]}
            ]},
            {">": [{"var": "cart.item_count"}, 5]}
        ]
    });

    let customer_data = json!({
        "cart": {
            "total": 120.00,
            "item_count": 3
        },
        "user": {
            "membership": "premium"
        }
    });

    let applies_for_discount = logic.apply(&discount_rule, &customer_data).unwrap();
    assert_eq!(applies_for_discount, json!(true));
}
```

## Supported Operations

This implementation supports all standard JSONLogic operations including:

- Basic operators (`==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=`)
- Logic operators (`!`, `!!`, `or`, `and`, `if`)
- Numeric operations (`+`, `-`, `*`, `/`, `%`)
- Array operations (`merge`)
- String operations (`cat`, `substr`)
- Data access (`var`)

For detailed documentation of operations and examples, visit [jsonlogic.com](http://jsonlogic.com).

## Features

- ✅ Complete implementation of JSONLogic specification
- 🚀 Zero-copy JSON deserialization
- 🛡️ Type-safe rule evaluation
- 🧪 Comprehensive test suite matching official JSONLogic tests

## Testing

The library includes a comprehensive test suite that verifies compatibility with the official JSONLogic test cases:

```bash
cargo test
```

## License

Licensed under Apache-2.0

## References

- [JSONLogic Documentation](http://jsonlogic.com)
- [Official Test Cases](http://jsonlogic.com/tests.json)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.
