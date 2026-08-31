# Structured Objects (Templating)

Use JSONLogic as a templating engine with templating mode.

> Requires `feature = "templating"`. The mode is off by default. Some
> examples on this page also use operators behind other Rust features:
> `??` needs `ext-control` and `length` needs `ext-string`. Every language
> binding ships with all operator features enabled.

## Enabling Structure Preservation

```rust
use datalogic_rs::Engine;

// Enable templating mode
let engine = Engine::builder().with_templating(true).build();

// Combine with custom configuration
let engine = Engine::builder()
    .with_config(my_config)
    .with_templating(true)
    .build();
```

## How It Works

In normal mode, unknown keys in a JSON object are treated as errors (or as
custom operators when one is registered). With structure preservation
enabled, unknown keys become literal output fields.

**Normal mode:**
```json
{ "user": { "var": "name" } }
// Error: "user" is not a known operator
```

**Structure preservation mode:**
```json
{ "user": { "var": "name" } }
// Result: { "user": "Alice" }
```

## Emitting Keys That Are Operator Names

A single-key object is always an operator invocation, so a key that happens
to name an operator is swallowed:

```json
{ "type": { "var": "x" } }
// Result: "number"  — the `type` operator ran; no key was emitted
```

That makes roughly 60 names unusable as output keys on their own: `type`,
`map`, `filter`, `if`, `keys`, `values`, `entries`, `length`, `in`, `sort`,
`now`, `try`, `cat`, `+`, `==`, and so on. Registered custom operators
shadow keys the same way.

Opt into an escape prefix to get them back:

```rust
use datalogic_rs::Engine;

let engine = Engine::builder()
    .with_templating(true)
    .with_template_key_escape('$')
    .build();
```

Exactly one leading prefix is stripped from every template key, and an
escaped key is never resolved as an operator:

| Template key | Output key |
|--------------|------------|
| `$type`      | `type`     |
| `$$type`     | `$type`    |
| `$$$type`    | `$$type`   |
| `$foo`       | `foo`      |
| `type`       | not a key: still the `type` operator |

```json
{ "$type": { "var": "x" }, "$map": 2, "plain": 3 }
// Result: { "type": 1, "map": 2, "plain": 3 }
```

Stripping is uniform across arities, so a key means the same thing whether
or not it has siblings. Three things worth knowing:

- **The setting is off by default and opt-in.** Without it, `$`-prefixed
  keys pass through verbatim exactly as before. Turning it on does change
  templates that currently emit literal `$` keys: write those as `$$key`.
- **Nested keys can collide after stripping.** `{"$a": 1, "a": 2}` emits the
  key `a` twice. The engine keeps duplicate pairs (as `keys` / `values` /
  `entries` already do); converting the result to JSON collapses them
  last-wins.
- **A bare sigil strips to the empty key.** `{"$": 1}` emits `{"": 1}`.

### Choosing the prefix

The prefix is a `char`, not a fixed `$`, because `$` already begins real
keys in MongoDB documents and JSON Schema output (`$ref`, `$id`). If your
payloads are `$`-heavy, pick something they never use and leave your `$`
keys untouched:

```rust
let engine = Engine::builder()
    .with_templating(true)
    .with_template_key_escape('~')  // `~type` -> `type`; `$ref` untouched
    .build();
```

A custom operator whose name starts with the prefix becomes unreachable
while the escape is on (the escape wins). Rename it, or choose a different
prefix.

The setting is inert without templating mode: outside it every single-key
object is an operator invocation, so there is nothing to escape into.

## Basic Templating

```rust
use datalogic_rs::Engine;

let engine = Engine::builder().with_templating(true).build();

let template = r#"{
    "greeting": {"cat": ["Hello, ", {"var": "name"}, "!"]},
    "isAdmin": {"==": [{"var": "role"}, "admin"]}
}"#;
let data = r#"{"name": "Alice", "role": "admin"}"#;

let result = engine.eval_str(template, data).unwrap();
// {"greeting":"Hello, Alice!","isAdmin":true}
```

## Nested Structures

Structure preservation works at any depth:

```rust
let template = r#"{
    "user": {
        "profile": {
            "displayName": {"var": "firstName"},
            "email": {"var": "userEmail"},
            "verified": true
        },
        "settings": {
            "theme": {"??": [{"var": "preferredTheme"}, "light"]},
            "notifications": {"var": "notificationsEnabled"}
        }
    },
    "metadata": {
        "version": "1.0"
    }
}"#;

let data = r#"{
    "firstName": "Bob",
    "userEmail": "bob@example.com",
    "notificationsEnabled": true
}"#;

let result = engine.eval_str(template, data).unwrap();
```

## Arrays in Templates

Arrays are processed element by element:

```rust
let template = r#"{
    "items": [
        {"name": "Item 1", "price": {"var": "price1"}},
        {"name": "Item 2", "price": {"var": "price2"}}
    ],
    "total": {"+": [{"var": "price1"}, {"var": "price2"}]}
}"#;

let data = r#"{"price1": 10, "price2": 20}"#;

let result = engine.eval_str(template, data).unwrap();
```

## Dynamic Arrays with Map

Generate arrays dynamically using `map`:

```rust
let template = r#"{
    "users": {
        "map": [
            {"var": "userList"},
            {
                "id": {"var": "id"},
                "name": {"var": "name"},
                "isActive": {"var": "active"}
            }
        ]
    }
}"#;

let data = r#"{
    "userList": [
        {"id": 1, "name": "Alice", "active": true},
        {"id": 2, "name": "Bob", "active": false}
    ]
}"#;

let result = engine.eval_str(template, data).unwrap();
```

## The `preserve` Operator Was Removed

In v4 there was an explicit `preserve` operator that wrapped a value to
prevent further evaluation. **v5 removed it.** Wrap-as-output is exactly
what templating mode already does for objects, and literal scalars
/ arrays already pass through inline. If you need to emit a JSON object
verbatim from a rule, enable `with_templating(true)` and write the object
directly.

## Use Cases

### API Response Transformation

```rust
let template = r#"{
    "success": true,
    "data": {
        "user": {
            "id": {"var": "userId"},
            "profile": {
                "name": {"cat": [{"var": "firstName"}, " ", {"var": "lastName"}]},
                "avatar": {"cat": ["https://cdn.example.com/", {"var": "avatarId"}, ".jpg"]}
            }
        }
    }
}"#;
```

### Document Generation

```rust
let template = r#"{
    "invoice": {
        "number": {"cat": ["INV-", {"var": "invoiceId"}]},
        "customer": {
            "name": {"var": "customerName"},
            "address": {"var": "customerAddress"}
        },
        "items": {"var": "lineItems"},
        "total": {
            "reduce": [
                {"var": "lineItems"},
                {"+": [{"var": "accumulator"}, {"var": "current.amount"}]},
                0
            ]
        }
    }
}"#;
```

### Configuration Templating

```rust
let template = r#"{
    "database": {
        "host": {"??": [{"var": "DB_HOST"}, "localhost"]},
        "port": {"??": [{"var": "DB_PORT"}, 5432]},
        "name": {"var": "DB_NAME"},
        "ssl": {"==": [{"var": "ENV"}, "production"]}
    },
    "cache": {
        "enabled": {"var": "CACHE_ENABLED"},
        "ttl": {"if": [
            {"==": [{"var": "ENV"}, "development"]},
            60,
            3600
        ]}
    }
}"#;
```

### Dynamic Forms

```rust
let template = r#"{
    "form": {
        "title": {"var": "formTitle"},
        "fields": {
            "map": [
                {"var": "fieldDefinitions"},
                {
                    "name": {"var": "name"},
                    "type": {"var": "type"},
                    "required": {"var": "required"},
                    "label": {"cat": [{"var": "name"}, {"if": [{"var": "required"}, " *", ""]}]}
                }
            ]
        }
    }
}"#;
```

## Mixing Operators and Structure

You can mix operators and structure freely:

```rust
let template = r#"{
    "type": "response",
    "version": "2.0",

    "status": {"if": [
        {"var": "success"},
        "ok",
        "error"
    ]},

    "data": {"if": [
        {"var": "success"},
        {
            "result": {"var": "data"},
            "count": {"length": {"var": "data"}}
        },
        {
            "error": {"var": "errorMessage"},
            "code": {"var": "errorCode"}
        }
    ]}
}"#;
```
