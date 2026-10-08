# API Reference

API documentation for the `@goplasmatic/datalogic-wasm` WebAssembly package.

The package exports the `builtinOperatorNames` function, four classes (`Engine`, `Rule`, `Session`, `DataHandle`), and, on the `web` target only, the default-exported `init` loader and `initSync`. It also still exports the deprecated `evaluate` and `evaluateWithTrace` functions and the `CompiledRule` class; see [Deprecated APIs](#deprecated-apis). Rules, data and results cross the boundary as JSON strings, except where a method below says otherwise; see [Input/Output Types](#inputoutput-types).

## Functions

### `init()`

Initialize the WebAssembly module. Required before using any other export in browser/bundler environments (the `web` target). It is the module's default export.

```typescript
type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

function init(options?: { module_or_path: InitInput | Promise<InitInput> }): Promise<InitOutput>;
```

**Parameters:**
- `options.module_or_path` (optional) - Custom WASM source (URL, Response, BufferSource, or a compiled `WebAssembly.Module`). Defaults to `datalogic_wasm_bg.wasm` next to the JS glue.

**Returns:** Promise that resolves when initialization is complete

**Example:**
```javascript
import init from '@goplasmatic/datalogic-wasm';

// Standard initialization
await init();

// Custom WASM location
await init({ module_or_path: '/custom/path/datalogic_wasm_bg.wasm' });
```

Passing the source positionally (`init('/path/to.wasm')`) still works, but wasm-bindgen deprecates it and logs a console warning; use the object form. The web target also exports `initSync({ module })`, which instantiates synchronously from bytes or a compiled `WebAssembly.Module` you already hold (in a Worker, for example).

> Node.js does not require initialization, and `init` is not a function there. The package's `node` export condition resolves the bare specifier to the CommonJS `nodejs` target, so a default import (`import init from '@goplasmatic/datalogic-wasm'`) binds `init` to the module namespace object and `await init()` throws `TypeError: init is not a function`. Code that has to run in both places should guard the call:
>
> ```javascript
> import init, { Engine } from '@goplasmatic/datalogic-wasm';
> if (typeof init === 'function') await init(); // no-op on Node
> ```
>
> Or import the Node target (`@goplasmatic/datalogic-wasm/nodejs`) and skip `init`.

---

### `builtinOperatorNames()`

Every built-in operator name this WASM build accepts, in the engine's registry order.

```typescript
function builtinOperatorNames(): string[];
```

**Returns:** Array of operator keys. The list includes the input aliases (`var` for `val`, `?:` for `if`, `match` for `switch`), so it has 87 entries for the 84 built-in operators. It is derived from the compiler's own lookup table, so tooling (editors, linters, palettes) can validate rules against the engine instead of a hand-maintained list. The list excludes custom operators (see [`engine.customOperatorNames()`](#customoperatornames-string)) and lists every family whatever an engine's `families` option says; [`engine.operators()`](#operators-string) follows the engine.

**Example:**
```javascript
import { builtinOperatorNames } from '@goplasmatic/datalogic-wasm';

const names = builtinOperatorNames();
names.length;                  // 87
names.includes('group_by');    // true
names.includes('var');         // true (alias of val)
names.includes('preserve');    // false (removed in v5)
```

---

## Classes

### `Engine`

A configurable engine: templating mode, an evaluation config, operator families, and custom operators. It compiles rules into [`Rule`](#rule) objects and opens [`Session`](#session)s.

#### Constructor

```typescript
new Engine(options?: {
  templating?: boolean;
  templateKeyEscape?: string;
  customOperators?: Record<string, (argsJson: string) => string>;
  config?: string | object;
  strictOperatorNames?: boolean;
  families?: string[];
})
```

**Parameters:**
- `templating` - Make templating mode the default for every rule this engine compiles (multi-key objects compile to output-shaping templates). `compileTemplate` / `compileStrict` choose per rule.
- `templateKeyEscape` - Single-character prefix that marks a template key as a literal output field rather than an operator invocation. Unset by default. With `'$'`, `{"$type": ...}` emits the key `type` instead of running the `type` operator, and `{"$$type": ...}` emits a literal `$type`. Only meaningful in templating mode; anything other than a one-character string throws. See [Structured Objects](../advanced/structured-objects.md#emitting-keys-that-are-operator-names)
- `customOperators` - Map of operator name to callback. Each callback receives the pre-evaluated arguments as a JSON-array string and must return a JSON-value string (`null`/`undefined` count as JSON `null`). A thrown exception or a non-string return becomes a runtime evaluation error. **Built-ins win:** registering a built-in name (`+`, `if`, `var`, ...) has no effect.
- `config` - Evaluation config; see [Engine configuration](#engine-configuration)
- `strictOperatorNames` - When `true`, a custom operator named like a built-in (aliases included) throws a `ConfigurationError` instead of being registered and never running. Defaults to `false`.
- `families` - The operator families the engine has besides the JSONLogic core: `"DateTime"`, `"ExtString"`, `"ExtArray"`, `"ExtObject"`, `"ExtControl"`, `"ErrorHandling"`, `"ExtMath"`, `"Tensor"`, `"Flagd"` (the `family` of each [`operators()`](#operators-string) row). Unset, the engine has every family. A family left out is not there for this engine: its names compile as unknown operators (failing at evaluation with `InvalidOperator`, and errors in `check`), count as output fields in templating mode, and are free for a custom operator.

**Throws:** `ConfigurationError` for an unknown config key or value, an unknown family name, or a built-in name under `strictOperatorNames`; `ParseError` (`stage: "parse-options"`) for a malformed options bag.

**Example:**
```javascript
import init, { Engine } from '@goplasmatic/datalogic-wasm';
await init();

const engine = new Engine({
  customOperators: {
    double: (argsJson) => String(JSON.parse(argsJson)[0] * 2),
  },
});
engine.evalStr('{"double": [21]}', '{}'); // "42"

const stringsOnly = new Engine({ families: ['ExtString'] });
stringsOnly.evalStr('{"upper": ["a"]}', '{}'); // '"A"'
stringsOnly.evalStr('{"now": []}', '{}');      // throws Error { name: "InvalidOperator", operator: "now" }
```

#### Methods

##### `compile(logic: string): Rule`

Compile a rule against this engine (its config and custom operators apply), in the engine's templating mode.

**Throws:** `ParseError` for malformed JSON; `InvalidOperator` for a multi-key object outside templating mode. `compile` does not reject unknown operator names: they surface as an `InvalidOperator` error when you evaluate the rule. Use [`compileChecked`](#compilecheckedlogic-string-rule) to refuse them up front.

```javascript
const rule = engine.compile('{">=": [{"var": "age"}, 18]}');
rule.evaluate('{"age": 21}'); // "true"
```

##### `compileTemplate(logic: string): Rule`

Compile `logic` in templating mode, whatever this engine was built with: a multi-key object is an output template and an unknown key an output field.

```javascript
const engine = new Engine();
engine.compileTemplate('{"name": {"var": "user"}, "active": true}').evaluate('{"user": "Alice"}');
// '{"name":"Alice","active":true}'
```

##### `compileStrict(logic: string): Rule`

Compile `logic` outside templating mode, whatever this engine was built with: a multi-key object is an error.

##### `compileChecked(logic: string): Rule`

Compile `logic` in the engine's mode only if [`check`](#checklogic-string-mode-string-string) finds no error diagnostic.

**Throws:** an `Error` named `CompileError` whose `diagnostics` property holds every problem (the same objects `check` returns).

```javascript
try {
  engine.compileChecked('{"if": [{"bogus": 1}, {"map": [1]}]}');
} catch (e) {
  e.name;                                  // "CompileError"
  e.diagnostics.map((d) => d.pointer);     // ["/if/0", "/if/1"]
}
```

##### `check(logic: string, mode?: string): string`

Every problem the engine can see in `logic` before it runs, in one pass, as a JSON array of diagnostics. `mode` is `"engine"` (the default: the engine's own templating mode), `"strict"` or `"template"`; any other value throws a `ParseError`.

```typescript
interface Diagnostic {
  code: string;               // "UnknownOperator", "NotAnOperator", "ArgumentForm", "ArgumentCount",
                              // "InvalidTimezone", "SimilarToOperator", "Unparsable", "Compile", ...
  severity: 'error' | 'warning';
  message: string;            // names the operator involved
  pointer: string;            // RFC 6901 JSON Pointer into the rule; "" is the whole rule
  operator: string | null;
}
```

Errors are what will fail: an object with several keys outside templating mode, an unknown operator (with a "did you mean" suggestion one edit away), `and` / `or` / `if` without an argument array, an argument count the operator rejects, a literal timezone that does not exist. Warnings run but are probably mistakes: arguments an operator never evaluates, and in a template an output key one edit away from an operator name. Malformed JSON is an `Unparsable` diagnostic, not a throw.

```javascript
JSON.parse(engine.check('{"if": [true, {"vr": "x"}, {"map": [1]}]}'));
// [
//   { code: "UnknownOperator", severity: "error", message: "unknown operator `vr`; did you mean `var`?",
//     pointer: "/if/1", operator: "vr" },
//   { code: "ArgumentCount", severity: "error", message: "`map` takes exactly 2 arguments, not 1",
//     pointer: "/if/2", operator: "map" }
// ]

engine.check('{"a": {"var": "x"}, "b": 1}', 'template'); // "[]"
```

[Rule Analysis](../advanced/rule-analysis.md) covers the diagnostics, check modes, rule facts and the operator catalogue across the engine and every binding.

##### `evalStr(logic: string, data: string): string`

One-shot: compile `logic` and evaluate it against `data`, returning the result as a JSON string.

```javascript
engine.evalStr('{"var": "name"}', '{"name": "Alice"}');          // "\"Alice\""
engine.evalStr('{"map": [[1,2,3], {"+": [{"var": ""}, 1]}]}', '{}'); // "[2,3,4]"
```

##### `evalMetered(logic: string, data: string, budget?: number): string`

One-shot metered evaluation. Returns a JSON string `{"result": <value>, "ops": <number>}`. `budget` caps the operations the rule may charge for this call; omit it to use the engine's `config.ops_budget`, and omit both to meter without a cap. A budget must be a whole number >= 1. Crossing it throws an `Error` named `BudgetExceeded` carrying `budget` and `spent`; see [Operation Budget](../advanced/operation-budget.md).

```javascript
engine.evalMetered('{"map":[{"var":"xs"},{"*":[{"var":""},2]}]}', '{"xs":[1,2,3]}');
// '{"result":[2,4,6],"ops":10}'
```

##### `evaluateWithTrace(logic: string, data: string, mode?: string): string`

Evaluate with a step-by-step execution trace, honoring this engine's templating flag, config, and custom operators. `mode` is `"engine"` (the default), `"strict"` or `"template"`, as for `check`, so a rule you compile with `compileTemplate` can be traced as one. The engine compiles the rule with optimization disabled so every operator records a step; use it for debugging, not hot paths.

**Returns:** JSON string containing a `TracedResult`:

```typescript
interface TracedResult {
  result: any;                        // Evaluation result (null on failure)
  expression_tree: ExpressionNode;    // Compile-time tree of the expression
  steps: Step[];                      // Execution steps, in evaluation order
  pointers?: Record<string, string>;  // Node id -> JSON Pointer into the rule as written
  error?: string;                     // Present on failure: the message
  structured_error?: StructuredError; // Present on failure: the structured form
}

interface ExpressionNode {
  id: number;                 // Compiled-node id; matches Step.node_id
  expression: string;         // Source text of this node
  children: ExpressionNode[];
}

interface Step {
  step_id: number;
  node_id: number;
  context: any;           // Data context the node saw
  result: any | null;     // The node's result; null when the step failed
  error: string | null;   // Failure message; null when the step succeeded
  iteration_index?: number;
  iteration_total?: number;
}

interface StructuredError {
  type: string;           // Stable tag: "Thrown", "InvalidOperator", "ParseError", ...
  message: string;
  operator?: string;      // Innermost failing operator
  node_ids?: number[];    // Breadcrumb from the failure site toward the root
  // plus kind-specific extras: thrown, variable, index, length, ...
}
```

Things to know about the shape:

- **Literals never record a step.** Only operator nodes (including `var`) appear in `steps`, so a rule with literal operands has fewer steps than it has JSON nodes.
- **Node ids are assigned at compile time, children first**, so the root of the tree is the highest id. `id: 0` with an empty `expression` is only the placeholder returned when compilation itself fails.
- **`pointers` places each node in the rule as written**: every node id maps to the RFC 6901 JSON Pointer of the rule value it was compiled from (`"/and/1"`, `""` for the root). It is absent when the rule does not compile.
- **`result` keeps its object key order.**

Failures do not throw: `result` is `null`, `error` carries the message, and `structured_error` the structured form. An unknown `mode` does throw (`ParseError`).

> The `TracedResult` JSON layout is the JavaScript-side wire shape, shared by every binding and described by `schemas/trace.v1.json` in the repository. On the Rust side it is produced from
> a `datalogic_rs::TracedRun<String>` (see the
> [Rust API reference](../rust/api-reference.md#tracedrunr-feature--trace)).

**Example:**
```javascript
const trace = engine.evaluateWithTrace(
  '{"and": [true, {"var": "x"}]}',
  '{"x": false}'
);

const data = JSON.parse(trace);
console.log(data.result);             // false
console.log(data.steps.length);       // 2 (var lookup, then and; the literal true records no step)
console.log(data.expression_tree.id); // 4 (the root; the var node is id 3)
console.log(data.pointers['3']);      // "/and/1"

// A failing rule reports the error inside the envelope instead of throwing
const failed = JSON.parse(engine.evaluateWithTrace('{"throw": "boom"}', '{}'));
console.log(failed.result);                 // null
console.log(failed.error);                  // 'Thrown: {"type":"boom"} (in operator: throw)'
console.log(failed.structured_error.type);  // "Thrown"

const strictEngine = new Engine({ config: { preset: 'strict' } });
const run = JSON.parse(strictEngine.evaluateWithTrace('{"+": [null, 1]}', '{}'));
run.structured_error.thrown; // { type: "NaN" }
```

##### `session(): Session`

Open a hot-loop [`Session`](#session) bound to this engine.

##### `operators(): string`

Every built-in operator this engine evaluates (following its `families`), as a JSON array in the schema of the [operator catalogue](../operators/operators.json):

| Field | Meaning |
|-------|---------|
| `name`, `aliases` | Canonical name and the other keys that call it (`val` / `["var"]`) |
| `family`, `feature` | Operator family (`"Core"`, `"DateTime"`, ...) and the Cargo feature that gates it (`null` for the core) |
| `min_args`, `max_args` | Argument counts the operator reads (`max_args: null` is unbounded) |
| `reads_context` | Whether it reads the data context |
| `effect` | `"pure"`, `"clock"` (`now`), `"throws"` or `"catches"` |
| `cost` | What its work is proportional to: `"node"`, `"bytes"`, `"per_item"`, `"n_log_n"`, `"quadratic"`, `"elements"` |
| `scoped_arg` | Which argument runs once per element under a pushed frame (an index, `"last"`, or `null`) |

```javascript
JSON.parse(engine.operators()).find((op) => op.name === 'reduce');
// { name: "reduce", aliases: [], family: "Core", feature: null, min_args: 2, max_args: 3,
//   reads_context: false, effect: "pure", cost: "per_item", scoped_arg: 1 }
```

##### `truthy(value: string): boolean`

Whether the JSON `value` is truthy under this engine's configured truthiness (the rules `if`, `and`, `or` and `session.evaluateTruthy` apply). Under the default rules an empty object is falsy, like an empty array. Malformed JSON throws a `ParseError`.

```javascript
engine.truthy('{}');  // false
engine.truthy('[0]'); // true
engine.truthy('"0"'); // true (a non-empty string)
```

##### `customOperatorNames(): string[]`

Names of the custom operators registered on this engine (order is not guaranteed). The module-level [`builtinOperatorNames()`](#builtinoperatornames) lists the built-ins.

```javascript
engine.customOperatorNames();       // ["double"]
new Engine({}).customOperatorNames(); // []
```

##### `free(): void`

Release the engine's WASM memory eagerly. A `FinalizationRegistry` also reclaims every class in this package when the JS object is garbage-collected, but that is best-effort; call `free()` on objects you create often (per render, per request). A freed object throws on use.

---

### `Rule`

A rule compiled by `engine.compile()` (or `compileTemplate`, `compileStrict`, `compileChecked`). It keeps access to its engine's custom operators and config.

- `evaluate(data: string): string` - evaluate against a JSON data string
- `evaluateData(handle: DataHandle): string` - evaluate against a [`DataHandle`](#datahandle)
- `evaluateMetered(data: string, budget?: number): string` - evaluate under an operation budget, returning `{"result": ..., "ops": N}` as for [`evalMetered`](#evalmeteredlogic-string-data-string-budget-number-string)
- `facts(): string` - what the rule reads and calls, as JSON (below)
- `free(): void`

```javascript
const rule = engine.compile('{">=": [{"var": "user.age"}, 18]}');
rule.evaluate('{"user": {"age": 21}}'); // "true"
```

`facts()` describes the compiled rule, after the optimizer, without running it:

```javascript
JSON.parse(engine.compile('{"+": [{"var": "a.b"}, {"var": "c"}]}').facts());
// { reads: [["a", "b"], ["c"]], computed_reads: false, reads_complete: true,
//   reads_data: true, operators: ["+", "val"], custom_operators: [], deterministic: true }
```

| Field | Meaning |
|-------|---------|
| `reads` | Data paths read from the root, each as its segments, sorted, with any path a shorter one covers dropped. Reads inside iterator bodies and `try` catch arms resolve against the element or the error and are left out |
| `computed_reads` | A path is only known at runtime (`{"var": {"cat": [...]}}`) |
| `reads_complete` | `reads` is everything the rule can read: no computed path, no custom operator |
| `reads_data` | The result depends on the data at all |
| `operators` | Canonical built-in names the rule uses (`val` for `var`), matching `engine.operators()` |
| `custom_operators` | Custom operator names the rule calls |
| `deterministic` | `false` for `now` and any custom operator |

A branch the optimizer folded away is neither read nor listed.

---

### `Session`

The hot-loop tier. A session owns one bump arena and resets it at the start of each evaluation, so a tight loop reuses the same memory instead of allocating a fresh arena per call (which is what `rule.evaluate()` does). The session returns results as owned strings, so they stay valid across later calls.

```javascript
const engine = new Engine({});
const rule = engine.compile('{">=": [{"var": "user.age"}, 18]}');
const session = engine.session();
const handle = new DataHandle('{"user": {"age": 34}}');

session.evaluate(rule, '{"user": {"age": 34}}'); // "true"  (JSON string)
session.evaluateData(rule, handle);              // "true"  (JSON string, no parse per call)
session.evaluateBool(rule, handle);              // true    (real boolean)
session.evaluateTruthy(rule, handle);            // true
```

#### Session methods

- `evaluate(rule: Rule, data: string): string` - evaluate against a JSON data string, reusing the arena
- `evaluateData(rule: Rule, handle: DataHandle): string` - the hot path: arena reuse and no per-call data work
- `evaluateMetered(rule: Rule, data: string, budget?: number): string` - metered evaluation with arena reuse, returning `{"result": ..., "ops": N}`
- `evaluateBool(rule: Rule, handle: DataHandle): boolean` - result must be a strict JSON boolean; anything else throws an `Error` named `TypeMismatch` (for example `"result is not a boolean (got number)"`)
- `evaluateInt(rule: Rule, handle: DataHandle): number` - a whole JSON number a JS number holds exactly (`|n| <= 2^53 - 1`); otherwise throws `TypeMismatch`
- `evaluateFloat(rule: Rule, handle: DataHandle): number` - accepts any JSON number; otherwise throws `TypeMismatch`
- `evaluateTruthy(rule: Rule, handle: DataHandle): boolean` - collapses any result through the engine's truthiness rules (the same coercion `if`/`and`/`or` apply); never type-mismatches
- `evaluateBatch(rule: Rule, handles: DataHandle[])` - one rule against many handles
- `evaluateMany(rules: Rule[], handle: DataHandle)` - many rules against one handle (the rule-set / feature-flag shape)
- `reset(): void` - reset the arena (optional; every `evaluate*` call resets first)
- `allocatedBytes(): number` - bytes held by the arena's chunks
- `free(): void`

`evaluateNumber(rule, handle)` is the deprecated name of `evaluateFloat`.

The two batch methods return one `Promise.allSettled`-style plain object per item, in order. Item failures never fail the call:

```typescript
type BatchOutcome =
  | { status: 'fulfilled'; value: string }                                   // result as a JSON string
  | { status: 'rejected'; reason: { tag: string; message: string; operator?: string } };
```

```javascript
const young = new DataHandle('{"user": {"age": 12}}');
session.evaluateBatch(rule, [handle, young]);
// [{ status: "fulfilled", value: "true" }, { status: "fulfilled", value: "false" }]

const age = engine.compile('{"var": "user.age"}');
session.evaluateMany([rule, age], handle);
// [{ status: "fulfilled", value: "true" }, { status: "fulfilled", value: "34" }]

session.evaluateMany([rule, 'nope'], handle)[1];
// { status: "rejected", reason: { tag: "InvalidArgument", message: "rules[1] is not a Rule" } }
```

Sessions are single-threaded like everything else in this package: use a session inside the Worker that created it.

---

### `DataHandle`

An immutable, pre-parsed JSON document resident in WASM linear memory. Every string-taking method copies the data across the JS/WASM boundary and re-parses it on each call; a handle pays that once, so it is the right tool when you evaluate the same payload more than once (rule sets, bulk scoring).

```typescript
new DataHandle(json: string)
```

**Throws:** An `Error` named `ParseError` on malformed JSON.

- `allocatedBytes` (getter) - bytes held by the handle's backing arena (input copy + parsed tree)
- `free(): void` - release the resident copy after the last evaluation

Evaluation never consumes a handle, and handles are independent of any engine: one handle can feed rules and sessions of different engines, as long as everything lives in the same module instance.

```javascript
const handle = new DataHandle('{"user": {"age": 34}}');
rule.evaluateData(handle);           // "true"
session.evaluateData(rule, handle);  // "true"
handle.free();
```

---

## Engine configuration

`new Engine({ config })` accepts an optional evaluation config, as a JSON string or a plain object. All keys are optional; unknown keys or values throw a `ConfigurationError`. See [Configuration](../advanced/configuration.md) for what each option means.

| Key | Value |
|-----|-------|
| `preset` | `"default"` \| `"safe_arithmetic"` \| `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"` \| `"ignore_value"` \| `"coerce_to_zero"` \| `"return_null"` |
| `division_by_zero` | `"return_saturated"` \| `"throw_error"` \| `"return_null"` \| `"return_infinity"` |
| `loose_equality_errors` | boolean |
| `missing_var` | `"null"` (default: a missing variable reads as `null`) \| `"error"` (throws `VariableNotFound`) |
| `truthy_evaluator` | `"javascript"` \| `"python"` \| `"strict_boolean"` |
| `numeric_coercion` | object of booleans: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |
| `ops_budget` | integer >= 1, or `null` for unbounded (caps the work one evaluation may do; crossing it throws `BudgetExceeded`) |

`preset` applies first; the remaining keys override it individually.

```javascript
// The default engine evaluates {"+": [null, 1]} to "1"; strict throws instead.
const engine = new Engine({ config: { preset: 'strict' } });
engine.evalStr('{"+": [null, 1]}', '{}');
// throws Error { name: "Thrown", thrown: { type: "NaN" }, operator: "+", ... }

new Engine({ config: { presett: 'strict' } });
// throws Error { name: "ConfigurationError", message: 'Configuration error: unknown config key "presett"' }

// A typo in a data path fails instead of reading null
const checked = new Engine({ config: { missing_var: 'error' } });
checked.evalStr('{"var": "user.nmae"}', '{"user": {"name": "Ana"}}');
// throws Error { name: "VariableNotFound", variable: "user.nmae", operator: "var", ... }
checked.evalStr('{"var": ["user.nmae", "anonymous"]}', '{"user": {"name": "Ana"}}'); // '"anonymous"'
```

Under `missing_var: "error"`, a `var` / `val` default, a present `null`, `missing`, `missing_some` and `exists` are not misses, and `try` catches the error.

---

## Type Definitions

### Input/Output Types

Rules, data and most results cross the boundary as JSON strings. Parse results for use:

```typescript
// Input: JSON strings
const logic: string = JSON.stringify({ "==": [1, 1] });
const data: string = JSON.stringify({ x: 42 });

// Output: a JSON string
const result: string = engine.evalStr(logic, data);
const parsed: boolean = JSON.parse(result); // true
```

The exceptions return JS values: `builtinOperatorNames()` and `customOperatorNames()` (string arrays), `truthy()` and the typed session methods (booleans and numbers), the batch methods (arrays of outcome objects, whose `value` is still a JSON string), and `DataHandle.allocatedBytes` / `session.allocatedBytes()` (numbers).

### Templating Mode

In templating mode:
- Unknown object keys become output fields
- Only recognized operators are evaluated
- Useful for JSON templating

```javascript
const engine = new Engine();

// Without templating - "result" is treated as an operator name
engine.evalStr('{"result": {"var": "x"}}', '{"x": 1}');
// throws Error { name: "InvalidOperator", operator: "result", ... }

// With templating - "result" becomes an output field
engine.compileTemplate('{"result": {"var": "x"}}').evaluate('{"x": 1}');
// '{"result":1}'
new Engine({ templating: true }).evalStr('{"result": {"var": "x"}}', '{"x": 1}');
// '{"result":1}'
```

---

## Error Handling

Every function and method throws a real `Error` object (`e instanceof Error` is `true`), with the structured fields attached as own properties:

| Property | Contents |
|----------|----------|
| `name` | Stable error-kind tag: `"ParseError"`, `"InvalidOperator"`, `"InvalidArguments"`, `"VariableNotFound"`, `"Thrown"`, `"IndexOutOfBounds"`, `"TypeError"`, `"ConfigurationError"`, `"BudgetExceeded"`, `"Custom"`, ... plus `"TypeMismatch"` from the typed session methods and `"CompileError"` from `compileChecked` |
| `message` | Human-readable message, including `(in operator: ...)` when the failing operator is known |
| `type` | Same tag as `name` (mirrors the wire JSON) |
| `operator` | Innermost failing operator, custom operators included (runtime errors only) |
| `node_ids` | Breadcrumb of compiled-node ids from the failure site toward the root (runtime errors only) |
| variant extras | Kind-specific fields: `thrown` (Thrown, as a parsed JS value), `variable` (VariableNotFound), `index` / `length` (IndexOutOfBounds), `budget` / `spent` (BudgetExceeded), `diagnostics` (CompileError), `stage` (boundary input errors, for example `"parse-data"`) |
| `detailJson` | The structured error as a JSON string (what 5.0.0 used as the rejection value) |

```javascript
try {
  engine.evalStr('{"invalid json', '{}');
} catch (e) {
  e instanceof Error; // true
  e.name;             // "ParseError"
  e.message;          // "Parse error: json parse error at byte 14: unexpected end of input"
}

try {
  engine.evalStr('{"throw": "limit_exceeded"}', '{}');
} catch (e) {
  e.name;     // "Thrown"
  e.thrown;   // { type: "limit_exceeded" }
  e.operator; // "throw"
  e.node_ids; // [2]
}
```

Common error kinds:
- `ParseError`: invalid JSON in the logic, data, value or options, or an invalid budget or mode
- `CompileError`: `compileChecked` found an error diagnostic; `diagnostics` lists them
- `InvalidOperator`: an unknown operator name (raised at evaluation, not at compile), or a multi-key object when templating is off
- `Thrown`: an explicit `throw`, or arithmetic that produced `NaN` (`{"+": ["abc", 1]}`)
- `InvalidArguments`: wrong argument shape for an operator (for example an unknown `date_diff` unit)
- `VariableNotFound`: a missing variable on an engine configured with `missing_var: "error"`
- `ConfigurationError`: an unknown config key or value, an unknown family, or a refused custom operator name
- `BudgetExceeded`: the evaluation charged past its operation budget
- `TypeMismatch`: `session.evaluateBool` / `evaluateInt` / `evaluateFloat` got a result of another type

Missing data is not an error by default: `{"var": "missing"}` evaluates to `null` unless the engine sets `missing_var: "error"`.

---

## Deprecated APIs

These exports still work in 5.x and are removed in 6.0. The package's TypeScript declarations mark them `@deprecated`.

| Deprecated | Use instead |
|------------|-------------|
| `evaluate(logic, data, templating)` | `new Engine({ templating }).evalStr(logic, data)`; build the engine once |
| `evaluateWithTrace(logic, data, templating)` | `engine.evaluateWithTrace(logic, data, mode?)`, which also applies the engine's config and custom operators |
| `new CompiledRule(logic, templating, config?, templateKeyEscape?)` | `new Engine({ templating, config, templateKeyEscape }).compile(logic)` |
| `session.evaluateNumber(rule, handle)` | `session.evaluateFloat(rule, handle)` |

### `evaluate()`

```typescript
function evaluate(logic: string, data: string, templating: boolean): string;
```

Builds a default engine (with `templating` as given) on every call and evaluates `logic` against `data`, returning a JSON string.

### `evaluateWithTrace()`

```typescript
function evaluateWithTrace(logic: string, data: string, templating: boolean): string;
```

Returns the same `TracedResult` envelope as [`engine.evaluateWithTrace()`](#evaluatewithtracelogic-string-data-string-mode-string-string), under default engine settings.

### `CompiledRule`

```typescript
new CompiledRule(logic: string, templating: boolean, config?: string | object, templateKeyEscape?: string)
```

A compiled rule with its own internal engine, so it cannot use custom operators. Methods: `evaluate(data: string): string`, `evaluateData(handle: DataHandle): string`, `free(): void`. `session.evaluateMany` does not accept a `CompiledRule`; it takes `Rule`s from `engine.compile()`.

---

## Performance Tips

1. **Compile once for repeated evaluation:**
   ```javascript
   // Slow: recompiles each time
   for (const user of users) {
     engine.evalStr(logic, JSON.stringify(user));
   }

   // Fast: compile once
   const rule = engine.compile(logic);
   for (const user of users) {
     rule.evaluate(JSON.stringify(user));
   }
   ```

2. **Initialize once at startup (browser/bundler), and build one engine:**
   ```javascript
   // Application entry point
   await init();
   export const engine = new Engine();
   // Now use engine.compile / engine.evalStr anywhere
   ```

3. **Reuse `Rule` instances, and free the ones you stop using:**
   ```javascript
   // Store compiled rules
   const rules = {
     isAdult: engine.compile('{">=": [{"var": "age"}, 18]}'),
     isPremium: engine.compile('{"==": [{"var": "tier"}, "premium"]}'),
   };

   // Rules created per request or per render should be released
   rule.free();
   ```

4. **Parse a payload once when it feeds several evaluations:** a `DataHandle` plus a `Session` skips the per-call copy and parse; on kilobyte payloads that is most of the round trip.
