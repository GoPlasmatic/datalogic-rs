# API Reference

API documentation for the `@goplasmatic/datalogic-wasm` WebAssembly package.

The package exports three functions (`evaluate`, `evaluateWithTrace`, `builtinOperatorNames`), five classes (`CompiledRule`, `Engine`, `Rule`, `Session`, `DataHandle`), and, on the `web` target only, the default-exported `init` loader. Every function takes and returns JSON strings; see [Input/Output Types](#inputoutput-types).

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

Passing the source positionally (`init('/path/to.wasm')`) still works but is deprecated by wasm-bindgen and logs a console warning; use the object form.

> **Note:** Node.js does not require initialization, and `init` is not a function there. The package's `node` export condition resolves the bare specifier to the CommonJS `nodejs` target, so a default import (`import init from '@goplasmatic/datalogic-wasm'`) binds `init` to the module namespace object and `await init()` throws `TypeError: init is not a function`. Code that has to run in both places should guard the call:
>
> ```javascript
> import init, { evaluate } from '@goplasmatic/datalogic-wasm';
> if (typeof init === 'function') await init(); // no-op on Node
> ```
>
> Or import the Node target explicitly (`@goplasmatic/datalogic-wasm/nodejs`) and skip `init` altogether.

---

### `evaluate()`

Evaluate a JSONLogic expression against data.

```typescript
function evaluate(logic: string, data: string, templating: boolean): string;
```

**Parameters:**
- `logic` - JSON string containing the JSONLogic expression
- `data` - JSON string containing the data context
- `templating` - Enable templating mode (multi-key objects compile to output-shaping templates with embedded JSONLogic)

**Returns:** JSON string containing the result

**Throws:** An `Error` object with structured fields (`name`, `operator`, `node_ids`, ...) if parsing or evaluation fails; see [Error Handling](#error-handling)

**Examples:**
```javascript
// Simple comparison
evaluate('{"==": [1, 1]}', '{}', false); // "true"

// Variable access
evaluate('{"var": "name"}', '{"name": "Alice"}', false); // "\"Alice\""

// Arithmetic
evaluate('{"+": [1, 2, 3]}', '{}', false); // "6"

// Array operations
evaluate('{"map": [[1,2,3], {"+": [{"var": ""}, 1]}]}', '{}', false); // "[2,3,4]"

// Templating mode
evaluate(
  '{"result": {"var": "x"}, "computed": {"+": [1, 2]}}',
  '{"x": 42}',
  true
); // '{"result":42,"computed":3}'
```

---

### `evaluateWithTrace()`

Evaluate with detailed execution trace for debugging.

```typescript
function evaluateWithTrace(logic: string, data: string, templating: boolean): string;
```

**Parameters:** Same as `evaluate()`

**Returns:** JSON string containing a `TracedResult`:

```typescript
interface TracedResult {
  result: any;                        // Evaluation result (null on failure)
  expression_tree: ExpressionNode;    // Compile-time tree of the expression
  steps: Step[];                      // Execution steps, in evaluation order
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
  operator?: string;      // Outermost failing operator
  node_ids?: number[];    // Breadcrumb from the failure site toward the root
  // plus kind-specific extras: thrown, index, length, ...
}
```

Two things to know about the shape:

- **Literals never record a step.** Only operator nodes (including `var`) appear in `steps`, so a rule with literal operands has fewer steps than it has JSON nodes.
- **Node ids are assigned at compile time, children first**, so the root of the tree is the highest id. `id: 0` with an empty `expression` is only the placeholder returned when compilation itself fails.

Failures do not throw: `result` is `null`, `error` carries the message, and `structured_error` the structured form.

> The `TracedResult` JSON layout is the JavaScript-side wire shape and is
> stable across the v4 → v5 cutover. On the Rust side it is produced from
> a `datalogic_rs::TracedRun<String>` (see the
> [Rust API reference](../rust/api-reference.md#tracedrunr-feature--trace)).

**Example:**
```javascript
const trace = evaluateWithTrace(
  '{"and": [true, {"var": "x"}]}',
  '{"x": false}',
  false
);

const data = JSON.parse(trace);
console.log(data.result);           // false
console.log(data.steps.length);     // 2 (var lookup, then and; the literal true records no step)
console.log(data.expression_tree.id); // 4 (the root; the var node is id 3)

// A failing rule reports the error inside the envelope instead of throwing
const failed = JSON.parse(evaluateWithTrace('{"throw": "boom"}', '{}', false));
console.log(failed.result);                 // null
console.log(failed.error);                  // 'Thrown: {"type":"boom"} (in operator: throw)'
console.log(failed.structured_error.type);  // "Thrown"
```

`evaluateWithTrace` uses default engine settings. To trace with a custom configuration or custom operators, build an [`Engine`](#engine) and call [`engine.evaluateWithTrace()`](#evaluatewithtracelogic-string-data-string-string).

---

### `builtinOperatorNames()`

Every built-in operator name this WASM build accepts, in the engine's registry order.

```typescript
function builtinOperatorNames(): string[];
```

**Returns:** Array of operator keys. The list includes the input aliases (`var` for `val`, `?:` for `if`, `match` for `switch`), so it has 67 entries for the 64 built-in operators. It is derived from the compiler's own lookup table, so tooling (editors, linters, palettes) can validate rules against the engine instead of a hand-maintained list. Custom operators are not included; see [`engine.customOperatorNames()`](#customoperatornames-string).

**Example:**
```javascript
import { builtinOperatorNames } from '@goplasmatic/datalogic-wasm';

const names = builtinOperatorNames();
names.length;                  // 67
names.includes('group_by');    // true
names.includes('var');         // true (alias of val)
names.includes('preserve');    // false (removed in v5)
```

---

## Classes

### `CompiledRule`

Pre-compiled rule for efficient repeated evaluation. `CompiledRule` builds its own engine internally, so it cannot use custom operators; for those, use [`Engine`](#engine) + `engine.compile()`.

#### Constructor

```typescript
new CompiledRule(logic: string, templating: boolean, config?: string | object)
```

**Parameters:**
- `logic` - JSON string containing the JSONLogic expression
- `templating` - Enable templating mode
- `config` (optional) - Evaluation config for this rule's engine, as a JSON string or a plain object. Same keys as [Engine configuration](#engine-configuration).

**Throws:** If the logic is invalid JSON or contains compilation errors (for example a multi-key object when templating is off), or a `ConfigurationError` for an unknown config key. Unknown operator names are not rejected here; they surface as an `InvalidOperator` error when the rule is evaluated.

**Example:**
```javascript
const rule = new CompiledRule('{">=": [{"var": "age"}, 18]}', false);

// Strict semantics without constructing an Engine
const strict = new CompiledRule('{"+": [null, 1]}', false, { preset: 'strict' });
strict.evaluate('{}'); // throws Error { name: "Thrown", thrown: { type: "NaN" }, operator: "+" }
```

#### Methods

##### `evaluate(data: string): string`

Evaluate the compiled rule against data.

**Parameters:**
- `data` - JSON string containing the data context

**Returns:** JSON string containing the result

**Example:**
```javascript
const rule = new CompiledRule('{"+": [{"var": "a"}, {"var": "b"}]}', false);

rule.evaluate('{"a": 1, "b": 2}');  // "3"
rule.evaluate('{"a": 10, "b": 20}'); // "30"
```

##### `evaluateData(handle: DataHandle): string`

Evaluate against a pre-parsed [`DataHandle`](#datahandle) instead of a string: no data copy or parse per call.

```javascript
const handle = new DataHandle('{"a": 1, "b": 2}');
rule.evaluateData(handle); // "3"
```

##### `free(): void`

Release the rule's WASM memory eagerly. Every class in this package is also reclaimed by a `FinalizationRegistry` when the JS object is garbage-collected, but that is best-effort; call `free()` when you are done with a rule you create often (per render, per request). A freed rule throws on use.

> **Tracing a compiled rule:** `CompiledRule` has no trace method. For execution traces, call the standalone `evaluateWithTrace(logic, data, templating)` function, or `engine.evaluateWithTrace(logic, data)` on an [`Engine`](#engine) when you need custom config or operators. Both recompile per call but return the full `TracedResult` shape.

---

### `Engine`

A configurable engine: templating mode, an evaluation config, and custom operators. Compiles rules into [`Rule`](#rule) objects and opens [`Session`](#session)s.

#### Constructor

```typescript
new Engine(options?: {
  templating?: boolean;
  customOperators?: Record<string, (argsJson: string) => string>;
  config?: string | object;
})
```

**Parameters:**
- `templating` - Enable templating mode for every rule this engine compiles
- `customOperators` - Map of operator name to callback. Each callback receives the pre-evaluated arguments as a JSON-array string and must return a JSON-value string (`null`/`undefined` count as JSON `null`). A thrown exception or a non-string return becomes a runtime evaluation error. **Built-ins win:** registering a built-in name (`+`, `if`, `var`, ...) has no effect.
- `config` - Evaluation config; see [Engine configuration](#engine-configuration)

**Throws:** `ConfigurationError` for an unknown config key or value; `ParseError` for a malformed options bag.

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
```

#### Methods

##### `compile(logic: string): Rule`

Compile a rule against this engine (its config and custom operators apply). Same compile-time checks as `new CompiledRule`.

##### `evalStr(logic: string, data: string): string`

One-shot: compile `logic` and evaluate it against `data`, returning the result as a JSON string.

##### `session(): Session`

Open a hot-loop [`Session`](#session) bound to this engine.

##### `evaluateWithTrace(logic: string, data: string): string`

Evaluate with an execution trace, honoring this engine's templating flag, config, and custom operators. Same `TracedResult` envelope as the top-level [`evaluateWithTrace()`](#evaluatewithtrace); failures are reported inside the envelope, not thrown.

```javascript
const run = JSON.parse(engine.evaluateWithTrace('{"double": [21]}', '{}'));
run.result;       // 42
run.steps.length; // 1

const strictEngine = new Engine({ config: { preset: 'strict' } });
const failed = JSON.parse(strictEngine.evaluateWithTrace('{"+": [null, 1]}', '{}'));
failed.result;                 // null
failed.structured_error.type;  // "Thrown"
failed.structured_error.thrown; // { type: "NaN" }
```

##### `customOperatorNames(): string[]`

Names of the custom operators registered on this engine (order is not guaranteed). Built-ins are listed by the module-level [`builtinOperatorNames()`](#builtinoperatornames).

```javascript
engine.customOperatorNames();       // ["double"]
new Engine({}).customOperatorNames(); // []
```

##### `free(): void`

Release the engine eagerly (see [`CompiledRule.free()`](#free-void)).

---

### `Rule`

A rule compiled by `engine.compile()`. Unlike `CompiledRule`, it keeps access to its engine's custom operators and config.

- `evaluate(data: string): string` - evaluate against a JSON data string
- `evaluateData(handle: DataHandle): string` - evaluate against a [`DataHandle`](#datahandle)
- `free(): void`

```javascript
const rule = engine.compile('{">=": [{"var": "user.age"}, 18]}');
rule.evaluate('{"user": {"age": 21}}'); // "true"
```

---

### `Session`

The hot-loop tier. A session owns one bump arena and resets it at the start of each evaluation, so a tight loop reuses the same memory instead of allocating a fresh arena per call (which is what `rule.evaluate()` does). Results are returned as owned strings, so they stay valid across later calls.

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

#### Methods

- `evaluate(rule: Rule, data: string): string` - evaluate against a JSON data string, reusing the arena
- `evaluateData(rule: Rule, handle: DataHandle): string` - the hot path: arena reuse and no per-call data work
- `evaluateBool(rule: Rule, handle: DataHandle): boolean` - result must be a strict JSON boolean; anything else throws an `Error` named `TypeMismatch` (for example `"result is not a boolean (got number)"`)
- `evaluateNumber(rule: Rule, handle: DataHandle): number` - accepts any JSON number; otherwise throws `TypeMismatch`
- `evaluateTruthy(rule: Rule, handle: DataHandle): boolean` - collapses any result through the engine's truthiness rules (the same coercion `if`/`and`/`or` apply); never type-mismatches
- `evaluateBatch(rule: Rule, handles: DataHandle[])` - one rule against many handles
- `evaluateMany(rules: Rule[], handle: DataHandle)` - many rules against one handle (the rule-set / feature-flag shape; `rules` must be `engine.compile()` Rules, not `CompiledRule`s)
- `reset(): void` - reset the arena explicitly (optional; every `evaluate*` call resets first)
- `allocatedBytes(): number` - bytes currently held by the arena's chunks
- `free(): void`

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

An immutable, pre-parsed JSON document resident in WASM linear memory. Every string-taking method copies the data across the JS/WASM boundary and re-parses it on each call; a handle pays that once, so it is the right tool when the same payload is evaluated more than once (rule sets, bulk scoring).

```typescript
new DataHandle(json: string)
```

**Throws:** An `Error` named `ParseError` on malformed JSON.

- `allocatedBytes` (getter) - bytes held by the handle's backing arena (input copy + parsed tree)
- `free(): void` - release the resident copy after the last evaluation

Handles are never consumed by evaluation and are independent of any engine: one handle can feed rules and sessions of different engines, as long as everything lives in the same module instance.

```javascript
const handle = new DataHandle('{"user": {"age": 34}}');
rule.evaluateData(handle);           // "true"
session.evaluateData(rule, handle);  // "true"
handle.free();
```

---

## Engine configuration

Both `new Engine({ config })` and `new CompiledRule(logic, templating, config)` accept an optional evaluation config, as a JSON string or a plain object. All keys are optional; unknown keys or values throw a `ConfigurationError`. See [Configuration](../advanced/configuration.md) for what each option means.

| Key | Value |
|-----|-------|
| `preset` | `"default"` \| `"safe_arithmetic"` \| `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"` \| `"ignore_value"` \| `"coerce_to_zero"` \| `"return_null"` |
| `division_by_zero` | `"return_saturated"` \| `"throw_error"` \| `"return_null"` \| `"return_infinity"` |
| `loose_equality_errors` | boolean |
| `truthy_evaluator` | `"javascript"` \| `"python"` \| `"strict_boolean"` |
| `numeric_coercion` | object of booleans: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |

`preset` applies first; the remaining keys override it individually.

```javascript
// The default engine evaluates {"+": [null, 1]} to "1"; strict throws instead.
const engine = new Engine({ config: { preset: 'strict' } });
engine.evalStr('{"+": [null, 1]}', '{}');
// throws Error { name: "Thrown", thrown: { type: "NaN" }, operator: "+", ... }

new Engine({ config: { presett: 'strict' } });
// throws Error { name: "ConfigurationError", message: 'Configuration error: unknown config key "presett"' }
```

---

## Type Definitions

### Input/Output Types

All functions accept and return JSON strings. Parse results for use:

```typescript
// Input: Always JSON strings
const logic: string = JSON.stringify({ "==": [1, 1] });
const data: string = JSON.stringify({ x: 42 });

// Output: Always JSON strings
const result: string = evaluate(logic, data, false);
const parsed: boolean = JSON.parse(result); // true
```

### Templating Mode

When `templating` is `true`:
- Unknown object keys become output fields
- Only recognized operators are evaluated
- Useful for JSON templating

```javascript
// Without templating - "result" is treated as an operator name
evaluate('{"result": {"var": "x"}}', '{"x": 1}', false);
// throws Error { name: "InvalidOperator", operator: "result", ... }

// With templating - "result" becomes output field
evaluate('{"result": {"var": "x"}}', '{"x": 1}', true);
// '{"result":1}'
```

---

## Error Handling

Every function and method throws a real `Error` object (`e instanceof Error` is `true`), with the structured fields attached as own properties:

| Property | Contents |
|----------|----------|
| `name` | Stable error-kind tag: `"ParseError"`, `"InvalidOperator"`, `"InvalidArguments"`, `"Thrown"`, `"IndexOutOfBounds"`, `"TypeError"`, `"ConfigurationError"`, `"Custom"`, ... plus `"TypeMismatch"` from the typed session methods |
| `message` | Human-readable message, including `(in operator: ...)` when the failing operator is known |
| `type` | Same tag as `name` (mirrors the wire JSON) |
| `operator` | Outermost failing operator (runtime errors only) |
| `node_ids` | Breadcrumb of compiled-node ids from the failure site toward the root (runtime errors only) |
| variant extras | Kind-specific fields: `thrown` (Thrown, as a parsed JS value), `index` / `length` (IndexOutOfBounds), `stage` (boundary input errors, for example `"parse-data"`) |
| `detailJson` | The structured error as a JSON string (what 5.0.0 used as the rejection value) |

```javascript
try {
  evaluate('{"invalid json', '{}', false);
} catch (e) {
  e instanceof Error; // true
  e.name;             // "ParseError"
  e.message;          // "Parse error: json parse error at byte 14: unexpected end of input"
}

try {
  evaluate('{"throw": "limit_exceeded"}', '{}', false);
} catch (e) {
  e.name;     // "Thrown"
  e.thrown;   // { type: "limit_exceeded" }
  e.operator; // "throw"
  e.node_ids; // [2]
}
```

Common error kinds:
- `ParseError`: invalid JSON in the logic, data, or options
- `InvalidOperator`: an unknown operator name (raised at evaluation, not at compile), or a multi-key object when templating is off
- `Thrown`: an explicit `throw`, or arithmetic that produced `NaN` (`{"+": ["abc", 1]}`)
- `InvalidArguments`: wrong argument shape for an operator (for example an unknown `date_diff` unit)
- `ConfigurationError`: an unknown config key or value
- `TypeMismatch`: `session.evaluateBool` / `evaluateNumber` got a result of another type

Missing data is not an error: `{"var": "missing"}` evaluates to `null` under every configuration.

---

## Performance Tips

1. **Use CompiledRule for repeated evaluation:**
   ```javascript
   // Slow: recompiles each time
   for (const user of users) {
     evaluate(logic, JSON.stringify(user), false);
   }

   // Fast: compile once
   const rule = new CompiledRule(logic, false);
   for (const user of users) {
     rule.evaluate(JSON.stringify(user));
   }
   ```

2. **Initialize once at startup (browser/bundler):**
   ```javascript
   // Application entry point
   await init();
   // Now use evaluate/CompiledRule anywhere
   ```

3. **Reuse CompiledRule instances, and free the ones you stop using:**
   ```javascript
   // Store compiled rules
   const rules = {
     isAdult: new CompiledRule('{">=": [{"var": "age"}, 18]}', false),
     isPremium: new CompiledRule('{"==": [{"var": "tier"}, "premium"]}', false),
   };

   // Rules created per request or per render should be released
   rule.free();
   ```

4. **Parse a payload once when it feeds several evaluations:** a `DataHandle` plus a `Session` skips the per-call copy and parse; on kilobyte payloads that is most of the round trip.
