# datalogic-rs

[![CI Status](https://github.com/codetiger/datalogic-rs/actions/workflows/crate-publish.yml/badge.svg)](https://github.com/codetiger/datalogic-rs/actions?query=crate-publish)
[![Documentation](https://docs.rs/datalogic-rs/badge.svg)](https://docs.rs/datalogic-rs)
[![crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs)

A high-performance Rust implementation of [JSONLogic](http://jsonlogic.com) that provides a way to write portable logic rules as JSON. Fully compliant with the JSONLogic specification and optimized for production use.

## Overview

`datalogic-rs` offers a complete, thread-safe implementation of the JSONLogic specification with:

- 💯 100% compliance with official JSONLogic test suite
- 🛡️ Strong type safety and comprehensive error handling
- 📦 Minimal dependencies (only serde_json, thiserror, smallvec)
- 🚀 Zero-copy deserialization and optimized rule evaluation
- 🧵 Thread-safe design with static operators
- 🔄 Smart rule optimization and caching

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
datalogic-rs = "2.0.15"
```

## Quick Example

```rust
use datalogic_rs::{JsonLogic, Rule};
use serde_json::json;

fn main() {
    // Complex discount rule example
    let discount_rule = Rule::from_value(&json!({
        "if": [
            {"and": [
                {">": [{"var": "cart.total"}, 100]},
                {"==": [{"var": "user.membership"}, "premium"]}
            ]},
            {"*": [{"var": "cart.total"}, 0.75]}, // 25% discount
            {"*": [{"var": "cart.total"}, 1.0]}   // no discount
        ]
    })).unwrap();

    let data = json!({
        "cart": {
            "total": 120.00
        },
        "user": {
            "membership": "premium"
        }
    });

    let result = JsonLogic::apply(&discount_rule, &data).unwrap();
    assert_eq!(result, json!(90.0)); // 25% off 120
}
```

## Features

### Core Operations
- **Comparison**: `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=`
- **Logic**: `!`, `!!`, `or`, `and`, `if`, `?:`
- **Numeric**: `+`, `-`, `*`, `/`, `%`, `min`, `max`
- **Array**: `map`, `filter`, `reduce`, `all`, `none`, `some`, `merge`
- **String**: `substr`, `cat`, `in`
- **Data**: `var`, `missing`, `missing_some`

### Advanced Features
- **Static Optimization**: Rules are optimized during compilation
- **Error Recovery**: `try` operator for handling evaluation errors
- **Data Preservation**: `preserve` operator for maintaining data structure
- **Zero-Copy Design**: Efficient memory usage with minimal allocation
- **Type Coercion**: Consistent type handling following JSONLogic spec

## Performance

The library is heavily optimized for production use with:

- Static operator dispatch
- Zero-copy JSON deserialization
- Smart rule optimization
- Efficient memory management with SmallVec
- Comprehensive benchmarking suite

## Testing

100% compatibility with official JSONLogic tests:

```bash
cargo test    # Run unit tests
cargo bench   # Run performance benchmarks
```

## License

Licensed under Apache-2.0

## Contributing

Contributions are welcome! The codebase has extensive documentation and test coverage.