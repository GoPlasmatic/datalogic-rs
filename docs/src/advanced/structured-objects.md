# Structured Objects (Templating)

Use JSONLogic as a templating engine with structure preservation mode.

## Enabling Structure Preservation

```rust
use datalogic_rs::DataLogic;

// Enable structure preservation
let engine = DataLogic::with_preserve_structure();

// Or combine with configuration
let engine = DataLogic::with_config_and_structure(config, true);
```

## How It Works

In normal mode, unknown keys in a JSON object are treated as errors (or custom operators). With structure preservation enabled, unknown keys become literal output fields.

**Normal mode:**
```json
{ "user": { "var": "name" } }
// Error: "user" is not a known operator
```

**Structure preservation mode:**
```json
{ "user": { "var": "name" } }
// Result: { "user": "Alice" }
// "user" is preserved as an output key
```

## Basic Templating

```rust
use datalogic_rs::DataLogic;
use serde_json::json;

let engine = DataLogic::with_preserve_structure();

let template = json!({
    "greeting": { "cat": ["Hello, ", { "var": "name" }, "!"] },
    "timestamp": { "now": [] },
    "isAdmin": { "==": [{ "var": "role" }, "admin"] }
});

let compiled = engine.compile(&template).unwrap();
let result = engine.evaluate_owned(&compiled, json!({
    "name": "Alice",
    "role": "admin"
})).unwrap();

// Result:
// {
//     "greeting": "Hello, Alice!",
//     "timestamp": 1704067200000,
//     "isAdmin": true
// }
```

## Nested Structures

Structure preservation works at any depth:

```rust
let template = json!({
    "user": {
        "profile": {
            "displayName": { "var": "firstName" },
            "email": { "var": "userEmail" },
            "verified": true
        },
        "settings": {
            "theme": { "??": [{ "var": "preferredTheme" }, "light"] },
            "notifications": { "var": "notificationsEnabled" }
        }
    },
    "metadata": {
        "generatedAt": { "now": [] },
        "version": "1.0"
    }
});

let result = engine.evaluate_owned(&compiled, json!({
    "firstName": "Bob",
    "userEmail": "bob@example.com",
    "notificationsEnabled": true
})).unwrap();

// Result:
// {
//     "user": {
//         "profile": {
//             "displayName": "Bob",
//             "email": "bob@example.com",
//             "verified": true
//         },
//         "settings": {
//             "theme": "light",
//             "notifications": true
//         }
//     },
//     "metadata": {
//         "generatedAt": 1704067200000,
//         "version": "1.0"
//     }
// }
```

## Arrays in Templates

Arrays are processed element by element:

```rust
let template = json!({
    "items": [
        { "name": "Item 1", "price": { "var": "price1" } },
        { "name": "Item 2", "price": { "var": "price2" } }
    ],
    "total": { "+": [{ "var": "price1" }, { "var": "price2" }] }
});

let result = engine.evaluate_owned(&compiled, json!({
    "price1": 10,
    "price2": 20
})).unwrap();

// Result:
// {
//     "items": [
//         { "name": "Item 1", "price": 10 },
//         { "name": "Item 2", "price": 20 }
//     ],
//     "total": 30
// }
```

## Dynamic Arrays with Map

Generate arrays dynamically using `map`:

```rust
let template = json!({
    "users": {
        "map": [
            { "var": "userList" },
            {
                "id": { "var": "id" },
                "name": { "var": "name" },
                "isActive": { "var": "active" }
            }
        ]
    }
});

let result = engine.evaluate_owned(&compiled, json!({
    "userList": [
        { "id": 1, "name": "Alice", "active": true },
        { "id": 2, "name": "Bob", "active": false }
    ]
})).unwrap();

// Result:
// {
//     "users": [
//         { "id": 1, "name": "Alice", "isActive": true },
//         { "id": 2, "name": "Bob", "isActive": false }
//     ]
// }
```

## The preserve Operator

Use the `preserve` operator to explicitly preserve a value without evaluation:

```rust
let template = json!({
    "data": { "var": "input" },
    "literal": { "preserve": { "var": "this is literal" } }
});

// Result:
// {
//     "data": <value of input>,
//     "literal": { "var": "this is literal" }
// }
```

## Use Cases

### API Response Transformation

```rust
let template = json!({
    "success": true,
    "data": {
        "user": {
            "id": { "var": "userId" },
            "profile": {
                "name": { "cat": [{ "var": "firstName" }, " ", { "var": "lastName" }] },
                "avatar": { "cat": ["https://cdn.example.com/", { "var": "avatarId" }, ".jpg"] }
            }
        }
    },
    "timestamp": { "now": [] }
});
```

### Document Generation

```rust
let template = json!({
    "invoice": {
        "number": { "cat": ["INV-", { "var": "invoiceId" }] },
        "date": { "format_date": [{ "now": [] }, "%Y-%m-%d"] },
        "customer": {
            "name": { "var": "customerName" },
            "address": { "var": "customerAddress" }
        },
        "items": { "var": "lineItems" },
        "total": {
            "reduce": [
                { "var": "lineItems" },
                { "+": [{ "var": "accumulator" }, { "var": "current.amount" }] },
                0
            ]
        }
    }
});
```

### Configuration Templating

```rust
let template = json!({
    "database": {
        "host": { "??": [{ "var": "DB_HOST" }, "localhost"] },
        "port": { "??": [{ "var": "DB_PORT" }, 5432] },
        "name": { "var": "DB_NAME" },
        "ssl": { "==": [{ "var": "ENV" }, "production"] }
    },
    "cache": {
        "enabled": { "var": "CACHE_ENABLED" },
        "ttl": { "if": [
            { "==": [{ "var": "ENV" }, "development"] },
            60,
            3600
        ]}
    }
});
```

### Dynamic Forms

```rust
let template = json!({
    "form": {
        "title": { "var": "formTitle" },
        "fields": {
            "map": [
                { "var": "fieldDefinitions" },
                {
                    "name": { "var": "name" },
                    "type": { "var": "type" },
                    "required": { "var": "required" },
                    "label": { "cat": [{ "var": "name" }, { "if": [{ "var": "required" }, " *", ""] }] }
                }
            ]
        }
    }
});
```

## Mixing Operators and Structure

You can mix operators and structure freely:

```rust
let template = json!({
    // Static structure
    "type": "response",
    "version": "2.0",

    // Conditional structure
    "status": { "if": [
        { "var": "success" },
        "ok",
        "error"
    ]},

    // Dynamic content
    "data": { "if": [
        { "var": "success" },
        {
            "result": { "var": "data" },
            "count": { "length": { "var": "data" } }
        },
        {
            "error": { "var": "errorMessage" },
            "code": { "var": "errorCode" }
        }
    ]}
});
```
