# Troubleshooting

Common issues and solutions for datalogic-rs.

## Rust Issues

### "Invalid operator: xyz"

**Cause:** The rule calls an operator name the engine does not know.
`compile` accepts the call, and the error appears when evaluation reaches
it. The name may be misspelled, belong to an operator family this build
or engine leaves out (a Cargo feature, or `EngineBuilder::with_families`),
or be meant as an output key.

**Solutions:**

1. Check the operator name spelling (operators are case-sensitive).
2. Register a custom operator on the builder.
3. Compile in templating mode (requires `feature = "templating"`); unknown
   keys then become literal output fields.

```rust
// Option 1: Fix spelling
let logic = r#"{"and": [...]}"#;  // not "AND"

// Option 2: Custom operator
let engine = datalogic_rs::Engine::builder()
    .add_operator("xyz", XyzOperator)
    .build();

// Option 3: Templating mode (feature = "templating"), for the engine
// or for one compile
# #[cfg(feature = "templating")]
let engine = datalogic_rs::Engine::builder().with_templating(true).build();
# #[cfg(feature = "templating")]
let template = datalogic_rs::Engine::new().compile_template(logic)?;
```

To catch the error before any input reaches it, compile with
`Engine::compile_checked` or run `Engine::check`, which also suggests a
known name one edit away (see
[Finding every problem in a rule](#finding-every-problem-in-a-rule)).

### A template key runs as an operator instead of being emitted

**Cause:** This is the inverse of the error above, and it is quieter: you
get no error, only the wrong result. In templating mode a single-key
object whose key names an operator is an operator call, so a key that
happens to name a built-in runs the operator instead of becoming an output
field.

```json
{ "type": { "var": "x" } }
// against {"x": 1}  ->  "number"   (the `type` operator ran)
// expected           ->  {"type": 1}
```

Every built-in operator name is affected (87 names, aliases included,
with every operator feature on): `type`, `map`, `filter`, `if`, `keys`,
`values`, `entries`, `length`, `in`, `sort`, `now`, `try`, `cat`, `+`,
`==` and the rest of the operator table, plus any custom operator you
registered. The same key behaves differently with siblings:
`{"type": X, "other": 1}` emits both keys, because multi-key object keys
are always literal.

**Solution:** enable the key escape and prefix the key with it. The engine
strips exactly one leading prefix and never resolves an escaped key as an
operator.

```rust
# #[cfg(feature = "templating")]
let engine = datalogic_rs::Engine::builder()
    .with_templating(true)
    .with_template_key_escape('$')
    .build();
```

```json
{ "$type": { "var": "x" } }   // -> {"type": 1}
{ "$$type": 1 }               // -> {"$type": 1}  (doubling escapes the sigil)
```

The prefix is a `char`, not a fixed `$`, so payloads that already use `$`
keys (MongoDB documents, JSON Schema output) can pick `~` or `#` instead.
The setting is off by default, requires `feature = "templating"`, and
applies only to rules compiled in templating mode (on a templating engine
or through `compile_template`). See
[Structured Objects](./advanced/structured-objects.md#emitting-keys-that-are-operator-names)
for the full rules.

### A misspelled path returns `null` instead of failing

**Cause:** JSONLogic reads a path that finds nothing as `null`, so
`{"var": "usr.name"}` against `{"user": {"name": "ada"}}` is `null`, and
the `null` flows on into comparisons and arithmetic.

**Solution:** if your data has a known shape, make a miss an error:

```rust
use datalogic_rs::{Engine, EvaluationConfig, MissingVar};

let engine = Engine::builder()
    .with_config(EvaluationConfig::default().with_missing_var(MissingVar::Error))
    .build();

let err = engine.eval_str(r#"{"var": "usr.name"}"#, r#"{"user": {"name": "ada"}}"#).unwrap_err();
assert_eq!(err.to_string(), "Variable not found: usr.name (in operator: var)");
```

The bindings take the same setting as the config key
`"missing_var": "error"`. Iterators run their general path under this
setting, so `map`, `filter` and `reduce` over fields are slower (see
[Performance](performance.md#iterator-fast-paths)).

### "Variable not found"

**Cause:** The engine is configured with `MissingVar::Error`, and a `var`
or `val` read found nothing at its path. Under the default
`MissingVar::Null` the read is `null` and raises nothing.

**Solutions:**

1. Check the variable path spelling.
2. Give the read a default: a read with a default is not a miss.
3. Test with `missing` or `exists` first (they never raise this error),
   or wrap the read in `try`.

```json
{"var": ["user.name", "Anonymous"]}

{"if": [
    {"missing": ["user.name"]},
    "No name",
    {"var": "user.name"}
]}

{"try": [{"var": "user.name"}, "No name"]}
```

### Unexpected `NaN` / `Thrown` errors from arithmetic

**Cause:** A value in an arithmetic operation does not coerce to a number.
The default `NanHandling::ThrowError` raises a `Thrown` error carrying
`{"type": "NaN"}`. The strings `"NaN"`, `"inf"` and `"infinity"`, and
numeric literals that overflow `f64`, do not coerce either.

**Solution:** Configure NaN handling:

```rust
use datalogic_rs::{Engine, EvaluationConfig, NanHandling};

let config = EvaluationConfig::default()
    .with_arithmetic_nan_handling(NanHandling::IgnoreValue); // or CoerceToZero, ReturnNull
let engine = Engine::builder().with_config(config).build();
```

`IgnoreValue` skips the value (`{"*": [2, "x", 3]}` is `6`), `CoerceToZero`
substitutes 0 (`0`), and `ReturnNull` makes the whole operation `null`.

### Matching on the kind of an error

Switch on `Error::code()`, which returns an `ErrorCode` (`Copy`, `Eq`,
`Hash`, present in every build), instead of comparing `Error::tag()`
strings or matching `ErrorKind` with its payload:

```rust
use datalogic_rs::{Engine, ErrorCode};

let engine = Engine::new();
match engine.eval_str(r#"{"throw": "rejected"}"#, "null") {
    Ok(out) => println!("{out}"),
    Err(e) => match e.code() {
        ErrorCode::Thrown => println!("rule threw {:?}", e.thrown_value()),
        ErrorCode::BudgetExceeded => println!("too expensive"),
        _ => println!("failed: {e}"),
    },
}
```

`ErrorCode::as_str()` gives the name `tag()` returns, which is also the
name the bindings put on the wire. That name parses back with
`str::parse`, and `ErrorCode::ALL` lists every code. `ErrorCode` is
`#[non_exhaustive]`, so keep a wildcard arm.

### `Error::operator()` names a nested operator

`Error::operator()` (and the serialized error's `"operator"` field) names
the innermost operator that failed, custom operators included, and plain
and traced runs agree. `{"if": [true, {"+": [1, {"abs": ["x"]}]}, 0]}`
reports `abs`. Releases before 5.8.0 reported the rule's root operator
unless a deeper site set one, so code that expected the root (`if` here)
needs updating. `Error::node_ids()` still carries the whole breadcrumb
from the failing node to the root, and `Error::resolve_path(&logic)` turns
it into named steps.

### Finding every problem in a rule

`compile` stops at the first problem and leaves calls that cannot succeed
to fail at evaluation. `Engine::check` reports every problem it can see,
in every branch, each with an RFC 6901 JSON Pointer into the rule:

```rust
use datalogic_rs::{CheckMode, Engine};

let engine = Engine::new();
let rule = r#"{"if": [true, {"vr": "x"}, {"map": [1]}]}"#;
for d in engine.check(rule, CheckMode::Engine) {
    println!("{d}");
}
// error at /if/1: unknown operator `vr`; did you mean `var`?
// error at /if/2: `map` takes exactly 2 arguments, not 1
```

`Engine::compile_checked` compiles only a rule with no error diagnostic
and otherwise returns them all in a `CompileError`. See
[Rule Analysis](advanced/rule-analysis.md#checking-a-rule-enginecheck).

### A rule behaves differently on another engine

Any engine can evaluate a `Logic` compiled on another. Constant folding
runs under the compiling engine's number coercion, NaN and division
handling, loose equality and truthiness, so the engine that evaluates the
rule recompiles it (once per distinct setting, cached on the rule) when
those settings differ, and the result follows the evaluating engine. Other
differences remain:

- Custom operators: the rule finds the evaluating engine's operator by
  name, which may be a different implementation.
- Operator families: the rule keeps the operators it was compiled with,
  whatever families the evaluating engine has.
- Speed: the rule is evaluated without [input projection](performance.md#input-projection).

`Logic::compiled_on(&engine)` tells you whether that engine compiled the
rule. In 6.0 a rule evaluates only on its own engine.

### An object with a `datetime` key is not treated as a datetime

Only a single-key object, `{"datetime": "..."}` or `{"timestamp": "..."}`,
is the boundary form of a datetime or duration. A record that has such a
field next to others (`{"datetime": "2024-01-01T00:00:00Z", "id": 1}`) is
ordinary data: `==`, `===`, `in` and `distinct` compare all of its
fields, and `type` reports `"object"`. Before 5.8.0 any object carrying
the key counted as a datetime. To compare the timestamps, read the field:
`{"==": [{"var": "a.datetime"}, {"var": "b.datetime"}]}`.

### "the trait bound `T: CustomOperator` is not satisfied" / `Send`-`Sync` errors

**Cause:** The custom operator type is not `Send + Sync`.

**Solution:** Use thread-safe primitives. Avoid `Rc`, `RefCell`, etc., in
operator state; wrap shared state in `Arc<Mutex<_>>` or atomics.

### v4 method calls fail to compile in v5

**Cause:** v5 renamed the public surface (`DataLogic` → `Engine`,
`CompiledLogic` → `Logic`, `Operator` → `CustomOperator`,
`evaluate_*` → `eval_*`, etc.) and removed the pre-release `compat`
shim. v5 is a hard cliff: there is no transitional feature flag.

**Solutions:**

- Follow the conceptual overview in the [Migration Guide](migration.md)
  and the per-call cookbook in the repo-root `MIGRATION.md`.
- Common mappings:
  - `DataLogic::with_config(c)` → `Engine::builder().with_config(c).build()`
  - `engine.evaluate_json(rule, data)` → `engine.eval_str(rule, data)`
    (or `datalogic_rs::eval_str(rule, data)` for the zero-config path)
  - `engine.evaluate_owned(&compiled, data)` → `let v: serde_json::Value = engine.session().eval_into(&compiled, &data)?`
    (requires `feature = "serde_json"`; `Engine::eval_into` takes a rule
    source, not a compiled `&Logic`)
  - `engine.evaluate_json_with_trace(rule, data)` → `engine.trace().eval_str(rule, data)` returning `TracedRun<String>`

### Slow compilation

**Cause:** Large or deeply nested expressions.

**Solutions:**

- Compile once, evaluate many times.
- Break expressions into smaller composable pieces.
- Use `feature = "trace"` to see which sub-expressions run and how often
  (the step log carries iteration counts, not timings); for timing, use a
  sampling profiler such as perf or Instruments (see
  [Performance](performance.md#profiling))

```rust
let compiled = engine.compile(rule).unwrap();
let mut session = engine.session();
for data in dataset {
    session.eval_str(&compiled, data)?;
    session.reset();
}
```

---

## JavaScript / WASM Issues

### "TypeError: Cannot read properties of undefined"

**Cause:** On the `web` target, the code called into the module before
`init()` finished loading it. The message names an internal export, for
example `(reading '__wbindgen_add_to_stack_pointer')`.

**Solution:** Await `init()` once before creating an `Engine`:

```javascript
import init, { Engine } from '@goplasmatic/datalogic-wasm';

await init();
const engine = new Engine();
engine.evalStr(logic, data);
```

### "TypeError: init is not a function"

**Cause:** Under Node the package resolves to its `nodejs` target, which
loads the module on import and has no `init` loader; a default import
binds the module namespace instead.

**Solutions:**

```javascript
// Code shared between browser and Node: guard the call
import init, { Engine } from '@goplasmatic/datalogic-wasm';
if (typeof init === 'function') await init();

// Node.js only: no init needed
const { Engine } = require('@goplasmatic/datalogic-wasm');
```

On a Node server, the native `@goplasmatic/datalogic-node` package is
faster than WASM; see [Performance](performance.md).

### "Failed to fetch" in browser

**Cause:** The browser cannot load the `.wasm` file. On the `web` target,
`init()` fetches `datalogic_wasm_bg.wasm` from next to
`datalogic_wasm.js` (resolved against `import.meta.url`) unless you pass
one: `init({ module_or_path: url })`.

**Solutions:**

1. Check that your bundler copies the `.wasm` file into the build output.
2. Check that your server serves it at that URL, with `Content-Type:
   application/wasm` (without it the loader falls back to a slower path
   and logs a warning).
3. Check CORS headers if loading from a CDN.

For Webpack:

```javascript
// webpack.config.js
module.exports = {
  experiments: {
    asyncWebAssembly: true,
  },
};
```

### Results are strings, not values

**Cause:** The WASM binding takes and returns JSON strings, not native
values.

**Solution:** Parse the result:

```javascript
const resultString = engine.evalStr(logic, data);
const result = JSON.parse(resultString);
```

### Performance issues

**Cause:** Recompiling rules repeatedly.

**Solution:** Compile once with `engine.compile` and evaluate the `Rule`:

```javascript
const engine = new Engine();
const rule = engine.compile(logic);
for (const item of items) {
  rule.evaluate(JSON.stringify(item));
}
```

The `CompiledRule` class and the free `evaluate` / `evaluateWithTrace`
functions are deprecated and removed in 6.0. Build an `Engine` and use
`compile`, `evalStr` and `engine.evaluateWithTrace`.

---

## React UI Issues

### "ResizeObserver loop completed with undelivered notifications"

**Cause:** The container size changes rapidly. The browser reports the
skipped notifications; the warning is usually harmless.

### Editor shows blank / empty

**Causes:**

1. Container has no dimensions
2. CSS not imported
3. Expression is null

**Solutions:**

```tsx
<div style={{ width: '100%', height: '500px' }}>
  <DataLogicEditor value={expression} />
</div>
```

```tsx
import '@goplasmatic/datalogic-ui/styles.css';
```

### Debugger controls not showing

**Cause:** `data` prop not provided.

**Solution:**

```tsx
<DataLogicEditor
  value={expression}
  data={{ x: 1, y: 2 }}
/>
```

With `data` the toolbar gains the debugger controls (play/pause, step, and a
step timeline). Values appear as you step: the current node shows its context
and result in a bubble. Nodes show no results until you step.

### SSR / Hydration errors in Next.js

**Cause:** The editor loads the WASM engine in the browser; server
rendering cannot run it.

**Solution:** Use a client component with dynamic import:

```tsx
'use client';

import dynamic from 'next/dynamic';

const DataLogicEditor = dynamic(
  () => import('@goplasmatic/datalogic-ui').then(mod => mod.DataLogicEditor),
  { ssr: false }
);
```

---

## Build Issues

### WASM build fails

**Cause:** Missing wasm-pack or target.

**Solution:**

```bash
cargo install wasm-pack
rustup target add wasm32-unknown-unknown
cd bindings/wasm && ./build.sh
```

### TypeScript errors with imports

```json
{
  "compilerOptions": {
    "moduleResolution": "bundler",
    "allowSyntheticDefaultImports": true
  }
}
```

### Bundler can't find WASM file

```javascript
// Webpack: enable async WASM
experiments: { asyncWebAssembly: true }
```

---

## Getting Help

If you can't resolve an issue:

1. Check [existing issues](https://github.com/GoPlasmatic/datalogic-rs/issues)
2. Create a minimal reproduction
3. Open a new issue with:
   - datalogic-rs version
   - Environment (Rust / Node / Browser)
   - Minimal code to reproduce
   - Expected vs actual behavior
