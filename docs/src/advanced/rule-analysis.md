# Rule Analysis

You can inspect a rule without evaluating it, since 5.8.0:

- [`Engine::check`](#checking-a-rule-enginecheck) lists every problem the
  engine can see before the rule runs, and `Engine::compile_checked`
  refuses a rule that has any error.
- [`Logic::facts`](#what-a-rule-reads-logicfacts) reports which data paths
  a compiled rule reads, which operators it calls, and whether its result
  depends on anything besides its data.
- [`Engine::operators`](#the-operator-catalogue-engineoperators) describes
  every built-in operator the engine evaluates, and
  [`operators.json`](#operatorsjson) is the same catalogue as a file.

All three read the operator table that drives compilation and dispatch,
so they report what the engine does. Every binding exposes them; see
[In the bindings](#in-the-bindings).

## Checking a rule: `Engine::check`

```rust
pub fn check<R: IntoLogic>(&self, rule: R, mode: CheckMode) -> Vec<Diagnostic>;
pub fn compile_checked<R: IntoLogic>(&self, rule: R) -> Result<Logic, CompileError>;
```

`check` walks the rule once and returns every problem in rule order,
including those in branches a given input never reaches. `compile` stops
at the first problem that prevents compiling and leaves a call that cannot
succeed to fail at evaluation; `check` reports both kinds up front.

```rust
use datalogic_rs::{CheckMode, DiagnosticCode, Engine, Severity};

let engine = Engine::new();
let diags = engine.check(
    r#"{"if": [{"vr": "user.age"}, {"map": [{"var": "items"}]}, {"!": [true, false]}]}"#,
    CheckMode::Engine,
);
for d in &diags {
    println!("{d}");
}
// error at /if/0: unknown operator `vr`; did you mean `var`?
// error at /if/1: `map` takes exactly 2 arguments, not 1
// warning at /if/2: `!` reads 0 to 1 argument; the other 1 argument is never evaluated

assert_eq!(diags[0].code, DiagnosticCode::UnknownOperator);
assert_eq!(diags[0].pointer, "/if/0");
assert_eq!(diags[2].severity, Severity::Warning);
```

### Diagnostics

Each `Diagnostic` carries:

| Field | Meaning |
|-------|---------|
| `code` | A `DiagnosticCode` (table below). |
| `severity` | `Severity::Error`: the rule fails to compile, or fails whenever this part runs. `Severity::Warning`: it runs, but probably not as meant. |
| `message` | One sentence naming the operator involved. |
| `pointer` | An [RFC 6901](https://www.rfc-editor.org/rfc/rfc6901) JSON Pointer into the rule, to the object or argument at fault. `""` is the whole rule. |
| `operator` | The operator (or key) the problem is about, when there is one. |

Its `Display` form is `<severity> at <pointer>: <message>`, with `/` for
the whole rule.

| `DiagnosticCode` | Severity | Reported for |
|------------------|----------|--------------|
| `Unparsable` | error | The rule is not valid JSON. |
| `UnknownOperator` | error | A single-key object whose key no operator of this engine answers to, with a "did you mean" suggestion one edit away. |
| `NotAnOperator` | error | An object with several keys outside templating mode. |
| `ArgumentForm` | error | `and`, `or` or `if` given one argument instead of an array. |
| `ArgumentCount` | error / warning | An argument count the operator rejects (error, read from the operator table or from a custom operator's [declared count](custom-operators.md#declaring-what-the-engine-may-assume)), or arguments the operator never evaluates (warning). |
| `InvalidTimezone` | error | A literal timezone name that is not in the timezone database. |
| `SimilarToOperator` | warning | In a template, an output key one edit away from an operator name. |
| `Compile` | error | Any other reason the compiler rejects the rule, such as nesting deeper than 256 levels. |
| `OperatorCheck` | as returned | A custom operator's own [`check`](custom-operators.md#validating-calls-before-they-run) rejected the call. |

Two contexts lower what gets reported. Inside an argument the operator
never evaluates, only problems that stop the rule compiling are reported.
Inside a `try` arm whose errors the `try` catches, errors are reported as
warnings, since they cannot fail the rule.

`CheckMode`, `Diagnostic`, `DiagnosticCode`, `Severity` and `CompileError`
are `#[non_exhaustive]`, so a `match` on a code needs a wildcard arm.

### Check modes

`CheckMode` chooses the mode a rule is read in:

| Mode | Reads the rule as |
|------|-------------------|
| `CheckMode::Engine` | The engine's own mode, as `compile` would. |
| `CheckMode::Strict` | Outside templating mode, as `compile_strict` would. |
| `CheckMode::Template` | In templating mode, as `compile_template` would. This mode needs no `templating` feature. |

```rust
let template = r#"{"name": {"var": "user.name"}, "tier": {"uper": {"var": "user.tier"}}}"#;

// Not a rule: several keys outside templating mode.
assert_eq!(engine.check(template, CheckMode::Strict)[0].code, DiagnosticCode::NotAnOperator);

// A valid template, with one suspicious key.
let diags = engine.check(template, CheckMode::Template);
assert_eq!(diags[0].code, DiagnosticCode::SimilarToOperator);
// warning at /tier: `uper` is an output field here; did you mean the operator `upper`?
```

### `compile_checked`

`compile_checked` compiles a rule only when `check` (in the engine's own
mode) finds no error. Otherwise it returns a `CompileError` whose
`diagnostics` lists every problem, warnings included. A rule with only
warnings compiles.

```rust
let err = engine.compile_checked(r#"{"and": true}"#).unwrap_err();
assert_eq!(err.diagnostics[0].code, DiagnosticCode::ArgumentForm);
println!("{err}");
// rule has 1 error(s)
//   error at /: `and` takes its arguments as an array

// `compile` accepts the same rule, and it fails when it runs.
assert!(engine.compile(r#"{"and": true}"#).is_ok());
```

`CompileError` implements `std::error::Error`. A rule that does not parse
gives one `Unparsable` diagnostic.

## What a rule reads: `Logic::facts`

```rust
pub fn facts(&self) -> Facts;
```

`facts` walks the compiled rule once, without evaluating it, and returns:

| Method | Returns |
|--------|---------|
| `reads()` | `&[DataPath]`: the data paths read from the root of the input, sorted, without duplicates, and without any path a listed path covers. |
| `has_computed_reads()` | Whether some `var` / `val` path, or `missing` / `missing_some` / `exists` path list, is computed at evaluation time. |
| `reads_complete()` | Whether `reads()` is everything the rule can read: no computed path, and no custom operator that may read the context. |
| `reads_data()` | Whether the result can depend on the data at all. |
| `operators()` | The built-in operators the rule calls, by canonical name (`val` for `var`), sorted. The names match `OperatorInfo::name`. |
| `custom_operators()` | The custom operators the rule calls, sorted. |
| `is_deterministic()` | `false` when the rule calls `now`, or a custom operator that does not declare itself deterministic. `throw` is deterministic. |

```rust
use datalogic_rs::Engine;

let engine = Engine::new();
let rule = engine.compile(
    r#"{"and": [{">=": [{"var": "user.age"}, 18]},
                {"in": [{"var": "user.country"}, ["DE", "FR"]]}]}"#,
)?;
let facts = rule.facts();

let paths: Vec<String> = facts.reads().iter().map(ToString::to_string).collect();
assert_eq!(paths, ["user.age", "user.country"]);
assert!(facts.reads_complete());   // nothing else can be read
assert!(facts.is_deterministic());
```

With `reads_complete()` true, evaluating against data pruned to those
paths gives the same result as the whole document, so you can fetch only
those fields from a data store. With `is_deterministic()` also true,
their values make a cache key for the result.

How to read the answers:

- **Reads are root reads.** An iterator body reads the current element,
  and a `try` catch arm reads the caught error, so those reads are not
  listed; the iterator's source is, and it covers them. A level marker
  that climbs back to the root (`{"val": [[1], "rate"]}` inside one
  `map`) is listed.
- **Covered reads are dropped.** Reading `user` observes `user.name`, so
  only `user` is listed. The empty path (`{"var": ""}`) is the whole data
  context.
- **Segments, not dotted strings.** `{"var": "a.b"}` reads `["a", "b"]`;
  `{"val": "a.b"}` reads the single key `["a.b"]`. `DataPath`'s `Display`
  joins segments with dots, so use `segments()` when keys may contain
  them. `is_root()` tests for the empty path and `a.covers(&b)` for an
  ancestor.
- **Complete or a lower bound.** A computed path
  (`{"var": {"var": "key"}}`) sets `has_computed_reads()`, and a custom
  operator can read the whole context through `EvalContext::root_input`
  unless its [`info`](custom-operators.md#declaring-what-the-engine-may-assume)
  says otherwise. Either makes `reads_complete()` false, and `reads()` is
  then a lower bound.
- **The compiled rule, after the optimizer.** A branch that constant
  folding removed is not read, and an operator it folded away is not
  listed: `{"if": [true, {"var": "a"}, {"var": "b"}]}` reads only `a`. An
  engine built `with_constant_folding(false)` can report more for the
  same rule; both answers are sound for the rule they describe.

`facts()` walks the tree on every call; keep the result if you need it
repeatedly. The engine uses the same facts itself: on the engine that
compiled a rule whose reads are complete, an owned, `serde_json` or
`Roots` input is brought into the arena only along the paths the rule
reads.

## The operator catalogue: `Engine::operators`

```rust
pub fn operators(&self) -> impl Iterator<Item = OperatorInfo>;
```

`operators()` yields one `OperatorInfo` per built-in operator the engine
evaluates: every operator compiled into the build, limited to the
engine's [families](configuration.md#operator-families). Custom operators
are not included; `custom_operator_names()` and
`custom_operator_info(name)` describe those.

| Field | Meaning |
|-------|---------|
| `name` | Canonical name (`"val"`, `"if"`, `"switch"`). |
| `aliases` | Other names the compiler accepts (`"var"`, `"?:"`, `"match"`). |
| `family` | `"Core"`, `"DateTime"`, `"ExtString"`, ... (`Family::name()`). |
| `feature` | The Cargo feature that gates the family, or `None` for the core. |
| `min_args`, `max_args` | The argument counts the operator's row declares: `==` reads 2 or more (`2`, `None`), `map` exactly 2. `min_args` is `0` where the row declares no minimum (`val`, `+`, `cat`), and `max_args` is `None` where it declares no maximum. |
| `reads_context` | Whether it reads the data context (`val`, `missing`, `missing_some`, `exists`, `fractional`). |
| `effect` | `"pure"`, `"clock"` (`now`), `"throws"` (`throw`) or `"catches"` (`try`). |
| `cost` | What its work is proportional to: `"node"`, `"bytes"`, `"per_item"`, `"n_log_n"`, `"quadratic"` or `"elements"`. |
| `scoped_arg` | The argument that runs under a pushed context frame, if any: `ScopedArg::Index(i)` (`map`'s body is `Index(1)`) or `ScopedArg::LastOfMany` (`try`'s catch arm). |

```rust
use datalogic_rs::{Engine, ScopedArg};

let engine = Engine::new();
let map = engine.operators().find(|op| op.name == "map").unwrap();
assert_eq!((map.min_args, map.max_args), (2, Some(2)));
assert_eq!(map.scoped_arg, Some(ScopedArg::Index(1)));

// Which operators read the clock? (`now` needs the `datetime` feature.)
let clock: Vec<&str> = engine.operators().filter(|op| op.effect == "clock").map(|op| op.name).collect();
assert_eq!(clock, ["now"]);
```

`OperatorInfo` and `ScopedArg` are `#[non_exhaustive]`.

### `operators.json`

[`docs/src/operators/operators.json`](../operators/operators.json) is the
catalogue for a build with every family compiled in, one object per
operator with the fields above (`scoped_arg` is a number, `"last"` or
`null`). A snapshot test renders it from `Engine::operators()` and fails
when the file is out of date, together with the feature table in the
[Operators Overview](../operators/overview.md); regenerate both with:

```bash
UPDATE_OPERATORS_JSON=1 cargo test -p datalogic-rs --all-features --test operators_json_test
```

The bindings' `operators()` returns the same entries for the engine at
hand, and the React editor's registry test checks its picker entries and
argument counts against the file. `schemas/operators.v1.json` in the
repository is its JSON Schema.

## In the bindings

Every binding reads a rule in a mode (engine, strict or template), and
returns diagnostics, facts and the catalogue in the JSON shapes above
(`schemas/diagnostics.v1.json`, `schemas/facts.v1.json`,
`schemas/operators.v1.json`). Facts arrive as
`{reads, computed_reads, reads_complete, reads_data, operators, custom_operators, deterministic}`,
with each read path as its list of segments.

| Binding | Equivalents |
|---------|-------------|
| Node | `engine.check(rule, mode?)`, `engine.compileChecked(rule)` (throws `errorType: "CompileError"` with `diagnostics`), `rule.facts()`, `engine.operators()`, as JS values. |
| WASM | `engine.check(logic, mode?)`, `engine.compileChecked(logic)` (throws an `Error` named `CompileError` with `diagnostics`), `rule.facts()`, `engine.operators()`, as JSON strings. |
| Python | `engine.check(rule, mode=None)`, `engine.compile_checked(rule)` (raises `CompileError` with `.diagnostics`), `rule.facts()`, `engine.operators()`, as typed dicts. |
| Go | `engine.Check(ruleJSON, datalogic.ModeEngine)`, `engine.CompileChecked(ruleJSON)` (an `*Error` with `Type` `"CompileError"` and `DiagnosticsJSON`), `rule.Facts()`, `engine.Operators()`. |
| JVM | `engine.check(ruleJson, CompileMode.ENGINE)`, `engine.compileChecked(ruleJson)` (`DatalogicException` with `diagnosticsJson()`), `rule.facts()`, `engine.operators()`. |
| .NET | `engine.Check(ruleJson, CompileMode.Engine)`, `engine.CompileChecked(ruleJson)` (`DatalogicException` with `DiagnosticsJson`), `rule.Facts()`, `engine.Operators()`. |
| PHP | `$engine->check($ruleJson, Native::MODE_ENGINE)`, `$engine->compileChecked($ruleJson)` (`ParseException` with `$diagnosticsJson`), `$rule->facts()`, `$engine->operators()`. |
| C ABI | `datalogic_engine_check`, `datalogic_engine_compile_checked` with `datalogic_error_diagnostics_json`, `datalogic_rule_facts`, `datalogic_engine_operators`; check `datalogic_abi_minor() >= 1` before calling them. |

The mode argument is a string in Node, WASM and Python (`"engine"`,
`"strict"`, `"template"`) and an enum or constant elsewhere. A custom
operator registered from a binding declares no argument count and no
`check`, so the checker accepts every call to it.
