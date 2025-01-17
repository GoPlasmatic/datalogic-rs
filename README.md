# datalogic-rs

[![CI Status](https://github.com/codetiger/datalogic-rs/actions/workflows/crate-publish.yml/badge.svg)](https://github.com/codetiger/datalogic-rs/actions?query=crate-publish)
[![Documentation](https://docs.rs/datalogic-rs/badge.svg)](https://docs.rs/datalogic-rs)
[![crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs)

A high-performance Rust implementation of [JSONLogic](http://jsonlogic.com) that provides a way to write portable logic rules as JSON. Fully compliant with the JSONLogic specification and optimized for production use.

## Overview

`datalogic-rs` offers a complete, thread-safe implementation of the JSONLogic specification with:

- 💯 100% compliance with official JSONLogic test suite
- 🛡️ Strong type safety and error handling
- 📦 Zero external runtime dependencies (only serde_json)
- 🚀 Optimized performance with zero-copy deserialization
- 🧵 Thread-safe design using Arc for operator sharing

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
datalogic-rs = "2.0.0"
```

## Quick Example

```rust
use datalogic_rs::JsonLogic;
use serde_json::json;

fn main() {
    // Complex discount rule example
    let discount_rule = json!({
        "if": [
            {"and": [
                {">": [{"var": "cart.total"}, 100]},
                {"==": [{"var": "user.membership"}, "premium"]}
            ]},
            {"*": [{"var": "cart.total"}, 0.75]}, // 25% discount
            {"*": [{"var": "cart.total"}, 1.0]}   // no discount
        ]
    });

    let data = json!({
        "cart": {
            "total": 120.00
        },
        "user": {
            "membership": "premium"
        }
    });

    let rule = Rule::from_value(&discount_rule).unwrap();
    let price = JsonLogic::apply(&rule, &data).unwrap();
    assert_eq!(price, json!(90.0)); // 25% off 120
}
```

## Supported Operations

All JSONLogic operations are supported:

- Comparison: `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=`
- Logic: `!`, `!!`, `or`, `and`, `if`, `?:`
- Numeric: `+`, `-`, `*`, `/`, `%`, `min`, `max`
- Array: `map`, `filter`, `reduce`, `all`, `none`, `some`, `merge`  
- String: `substr`, `cat`, `in`
- Data: `var`, `missing`, `missing_some`
- Custom: `preserve` for data preservation

## Performance

The library is optimized for production use with:

- Efficient operator dispatch using Arc
- Zero-copy JSON handling
- Optional auto-traversal of nested rules
- Comprehensive benchmarking suite

## Testing

100% compatibility with official JSONLogic tests:

```bash
cargo test        # Run unit tests
cargo bench      # Run performance benchmarks
```

## License

Licensed under Apache-2.0

## Contributing

Contributions are welcome! The codebase has extensive documentation and test coverage.