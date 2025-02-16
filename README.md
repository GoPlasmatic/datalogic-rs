# datalogic-rs

[![CI Status](https://github.com/codetiger/datalogic-rs/actions/workflows/crate-publish.yml/badge.svg)](https://github.com/codetiger/datalogic-rs/actions?query=crate-publish)
[![Documentation](https://docs.rs/datalogic-rs/badge.svg)](https://docs.rs/datalogic-rs)
[![crates.io](https://img.shields.io/crates/v/datalogic-rs.svg)](https://crates.io/crates/datalogic-rs)
[![Downloads](https://img.shields.io/crates/d/datalogic-rs)](https://crates.io/crates/datalogic-rs)

A **lightweight, high-performance** Rust implementation of [JSONLogic](http://jsonlogic.com), optimized for **rule-based decision-making** and **dynamic expressions**.

✨ **Why `datalogic-rs`?**
- 🏆 **Fully JSONLogic-compliant** (100% test coverage)
- 🚀 **Fast & lightweight**: Zero-copy JSON parsing, minimal allocations
- 🔒 **Thread-safe**: Designed for parallel execution
- ⚡ **Optimized for production**: Static dispatch, caching, and rule optimization

---

## **📦 Installation**

Add `datalogic-rs` to your `Cargo.toml`:

```toml
[dependencies]
datalogic-rs = "2.0.17"
```

---

## **🚀 Quick Start: Evaluating JSONLogic Rules**

```rust
use datalogic_rs::{JsonLogic, Rule};
use serde_json::json;

fn main() {
    let rule = Rule::from_value(&json!({
        "if": [
            {">": [{"var": "cart.total"}, 100]},
            "Eligible for discount",
            "No discount"
        ]
    })).unwrap();

    let data = json!({"cart": {"total": 120}});
    let result = JsonLogic::apply(&rule, &data).unwrap();
    
    assert_eq!(result, json!("Eligible for discount"));
}
```

---

## **🛠️ Features**
### **✅ Supported Operations**
| Category   | Operators |
|------------|----------|
| **Comparisons** | `==`, `===`, `!=`, `!==`, `>`, `>=`, `<`, `<=` |
| **Logic**  | `and`, `or`, `if`, `!`, `!!` |
| **Math**  | `+`, `-`, `*`, `/`, `%`, `min`, `max` |
| **Arrays** | `map`, `filter`, `reduce`, `all`, `none`, `some`, `merge` |
| **Strings** | `substr`, `cat`, `in` |
| **Data Handling** | `var`, `missing`, `missing_some` |

### **💡 Advanced Features**
- **Static Optimization**: Rules are optimized at compile-time for faster execution.
- **Error Handling**: The `try` operator prevents rule evaluation failures.
- **Memory Efficiency**: Zero-copy JSON deserialization with **SmallVec**.
- **Type Coercion**: JSONLogic-compliant automatic type conversions.

---

## **🎯 Use Cases**
`datalogic-rs` is ideal for **rule-based decision engines** in:
- **Feature flagging** (Enable features dynamically based on user attributes)
- **Dynamic pricing** (Apply discounts or surge pricing based on conditions)
- **Fraud detection** (Evaluate transaction risk using JSON-based rules)
- **Form validation** (Check field dependencies dynamically)

---

## **📊 Performance**
**Benchmark results show** `datalogic-rs` is **2x faster** than other JSONLogic implementations, thanks to:
- Static operator dispatch
- Optimized rule execution
- Smart caching for reusable expressions

To run benchmarks:
```bash
cargo bench
```

---

## **🛠️ Contributing**
We welcome contributions! See the [CONTRIBUTING.md](./CONTRIBUTING.md) for details.

📜 **License**: Apache-2.0

---

### **🚀 Next Steps**
✅ Try out `datalogic-rs` today!  
📖 Check out the [docs.rs documentation](https://docs.rs/datalogic-rs)  
⭐ Star the [GitHub repository](https://github.com/json-logic/datalogic-rs) if you find it useful!

