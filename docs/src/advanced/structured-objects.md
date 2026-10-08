# Structured Objects (Templating)

Templating mode turns JSONLogic into a templating engine: an object with
several keys becomes an output object, and a key that names no operator
becomes an output field.

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

### Per compile

You can also choose the mode for one compile, on any engine:
`engine.compile_template(rule)` compiles in templating mode and
`engine.compile_strict(rule)` without it, whatever the engine was built
with. The engine's custom operators, template key escape and folding
setting still apply, so one engine with one set of operator registrations
can compile output templates and strict conditions:

```rust
use datalogic_rs::Engine;

let engine = Engine::new(); // built without templating

let template = engine.compile_template(r#"{"user": {"var": "name"}, "source": "api"}"#)?;
let out = engine.session().eval_str(&template, r#"{"name": "ana"}"#)?;
assert_eq!(out, r#"{"user":"ana","source":"api"}"#);

// A condition stays strict: an object with several keys is a compile
// error, and an unknown key is an unknown operator, not an output field.
let condition = engine.compile_strict(r#"{"==": [{"var": "tier"}, "gold"]}"#)?;
assert!(engine.compile_strict(r#"{"tier": "gold", "active": true}"#).is_err());
```

`compile_template` needs the `templating` feature; `compile_strict` is in
every build. The bindings name the pair `compileTemplate` /
`compileStrict` (Python `compile_template` / `compile_strict`, Go and .NET
`CompileTemplate` / `CompileStrict`). `Engine::check` takes the same
choice as `CheckMode::Template` or `CheckMode::Strict`; see
[Rule Analysis](rule-analysis.md#check-modes).

## How It Works

In normal mode, the engine treats an unknown key in a JSON object as an
unknown operator (or as a custom operator when one is registered under
that name), and an object with several keys as a compile error. With
structure preservation enabled, unknown keys become literal output fields.

**Normal mode:**
```json
{ "user": { "var": "name" } }
// Evaluating fails: Invalid operator: user
```

**Structure preservation mode:**
```json
{ "user": { "var": "name" } }
// Result: { "user": "Alice" }
```

## Emitting Keys That Are Operator Names

In templating mode, a single-key object whose key names an operator is an
operator call, so the key is swallowed:

```json
{ "type": { "var": "x" } }
// Result: "number"  (the `type` operator ran; no key was emitted)
```

That makes every built-in operator name unusable as an output key on its
own (87 names, aliases included, with every operator feature on): `type`,
`map`, `filter`, `if`, `keys`, `values`, `entries`, `length`, `in`, `sort`,
`now`, `try`, `cat`, `+`, `==`, and so on. Registered custom operators
shadow keys the same way. A key with siblings is always an output field:
`{"type": X, "other": 1}` emits both keys.

Opt into an escape prefix to get them back:

```rust
use datalogic_rs::Engine;

let engine = Engine::builder()
    .with_templating(true)
    .with_template_key_escape('$')
    .build();
```

The engine strips exactly one leading prefix from every template key and
never resolves an escaped key as an operator:

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
or not it has siblings. Three caveats:

- **The setting is off by default.** Without it, `$`-prefixed keys pass
  through verbatim. Turning it on changes templates that emit literal `$`
  keys: write those as `$$key`.
- **Keys can collide after stripping.** `{"$a": 1, "a": 2}` emits the key
  `a` twice. The engine keeps duplicate pairs (as `keys` / `values` /
  `entries` do), and so does the JSON text `eval_str` returns; converting
  the result to a `serde_json::Value` keeps the last.
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

The escape applies to rules compiled in templating mode: on an engine
built `with_templating(true)`, or through `compile_template` on any engine
that has an escape set. A rule compiled strictly treats every single-key
object as an operator call, so there is nothing to escape into.

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

The engine processes arrays element by element:

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

v4 had an explicit `preserve` operator that wrapped a value to prevent
further evaluation. **v5 removed it.** Templating mode emits objects as
output, and literal scalars and arrays pass through inline. To emit a
JSON object verbatim from a rule, enable `with_templating(true)` (or use
`compile_template`) and write the object.

## Reading a Compiled Template Back

`Logic::to_json()` writes a compiled rule or template back as JSON that
compiles to the same rule. Keys, paths, custom operator names and `throw`
types that hold quotes, backslashes or control characters come out
escaped, escaped template keys keep their prefix (`{"$type": ..}`), and a
`{"val": "a.b"}` read (the single key `"a.b"`) stays in the `val` form
rather than turning into `{"var": "a.b"}`, which reads `a` then `b`. The
output describes the rule after constant folding, so a folded
subexpression appears as its value.

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

Operators and literal structure mix at any level:

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
