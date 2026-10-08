# goplasmatic/datalogic

[![Packagist](https://img.shields.io/packagist/v/goplasmatic/datalogic)](https://packagist.org/packages/goplasmatic/datalogic)
[![CI](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/GoPlasmatic/datalogic-rs/actions/workflows/ci.yml)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)

Part of [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs): one engine, every runtime.

PHP bindings for [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs),
the JSONLogic rules engine with one Rust core and official bindings for
Rust, Node.js, the browser (WASM), Python, Go, Java, .NET, and PHP.
Compile a rule once and evaluate it many times, natively in PHP. Same
rules, same semantics: every binding runs the same core and passes the
same 2,128-case conformance battery (66 suites).

For the cross-runtime overview and the API-tier model every binding
implements, see the
[repo README](https://github.com/GoPlasmatic/datalogic-rs#readme).

> **New in v5.** This package is new: there is no v4 PHP artifact. If
> you are coming from the v4 Rust crate or the v4
> `@goplasmatic/datalogic` WASM package, see
> [MIGRATION.md](https://github.com/GoPlasmatic/datalogic-rs/blob/main/MIGRATION.md)
> for the engine's v4 → v5 changes.

## Install

```bash
composer require goplasmatic/datalogic
```

Composer records the constraint `^5.8`. The package requires PHP 8.4+
with `ext-ffi` enabled. The binding is a PHP FFI wrapper over the
engine's C ABI; the Composer package ships the native library under
`lib/<os>-<arch>/` for every supported platform, and the FFI loader
picks the one for the host at runtime. You need no Rust toolchain.

| Platform | Architectures   |
|----------|-----------------|
| Linux    | x86_64, aarch64 |
| macOS    | x86_64, arm64   |
| Windows  | x86_64, arm64   |

## Quick start

```php
use Goplasmatic\Datalogic\Engine;

$engine = new Engine();
echo $engine->apply('{"+":[1,2]}', '{}');  // "3"
```

Rules, data, and results cross the boundary as JSON strings; use
`json_encode` / `json_decode` at the edges.

## Compile once, evaluate many

Compile the rule once when you'll evaluate it against many data inputs:

```php
$engine = new Engine();
$rule = $engine->compile('{"var":"x"}');
foreach ([1, 2, 3] as $x) {
    echo $rule->evaluate(json_encode(['x' => $x])), "\n";
}
```

`Engine` and compiled `Rule` objects carry no per-call state: build and
compile once per process and reuse them across requests. Sessions
(below) hold a mutable arena, so give each evaluation loop its own.

### Compile modes and checked compiles

`compile` reads a rule in the engine's own mode. `compileTemplate` and
`compileStrict` choose templating for one compile, so one engine (with
one set of custom operators) can compile both conditions and output
templates. `compileMode($rule, $mode)` takes the mode as an `int`:
`Native::MODE_ENGINE`, `Native::MODE_STRICT` or `Native::MODE_TEMPLATE`
(`Goplasmatic\Datalogic\Internal\Native`).

```php
$shape = $engine->compileTemplate('{"user": {"var": "name"}, "n": 1}');
echo $shape->evaluate('{"name": "ana"}');  // {"user":"ana","n":1}
```

`check($rule, $mode = Native::MODE_ENGINE)` reports every problem the
engine can see before the rule runs, as a JSON array of `{code,
severity, message, pointer, operator}`, where `pointer` is an RFC 6901
JSON Pointer into the rule. Errors are what will fail (an unknown
operator, with a suggestion one edit away; an argument count the
operator rejects; a timezone that does not exist); warnings run but are
probably mistakes. A bad rule does not make `check` throw.
`compileChecked($rule)` compiles only a rule with no error and otherwise
throws `ParseException` with `$errorType === "CompileError"`, whose
`$diagnosticsJson` holds the same array:

```php
use Goplasmatic\Datalogic\Exception\ParseException;

echo $engine->check('{"if": [{">": [{"var": "age"}, 17]}, "adult", {"vr": "x"}]}');
// [{"code":"UnknownOperator","severity":"error","message":"unknown operator `vr`; did you mean `var`?",
//   "pointer":"/if/2","operator":"vr"}]

try {
    $rule = $engine->compileChecked($ruleJson);
} catch (ParseException $e) {
    echo $e->errorType;        // "CompileError"
    echo $e->diagnosticsJson;  // every diagnostic, errors and warnings
}
```

### What a rule reads

`$rule->facts()` returns, as JSON, the data paths the rule reads (each
as its segments), the operators it calls, and whether its result
depends only on its data:

```php
$rule = $engine->compile(
    '{"and": [{">=": [{"var": "user.age"}, 18]}, {"in": [{"var": "user.plan"}, ["pro", "team"]]}]}'
);
echo $rule->facts();
// {"reads":[["user","age"],["user","plan"]],"computed_reads":false,"reads_complete":true,
//  "reads_data":true,"operators":[">=","and","in","val"],"custom_operators":[],
//  "deterministic":true}
```

`reads_complete` is `false` when the rule builds a path at runtime or
calls a custom operator, and `deterministic` is `false` for `now` and
for any custom operator. The facts describe the compiled rule, so a
branch the optimizer removed is neither read nor listed.

## Sessions (hot loops)

A `Session` reuses one arena across evaluations and resets it at the
start of every call, so peak memory stays bounded:

```php
$session = $engine->openSession();
foreach ($inputs as $data) {
    $result = $session->evaluate($rule, $data);
}
```

PHP's destructor releases native handles when the wrapper object goes
out of scope; every wrapper type also exposes an explicit
`close()` for early release.

## Data handles (parse once, evaluate many)

When one payload feeds many evaluations, parse it once into a
`DataHandle` and pass the handle to `Rule::evaluate` or
`Session::evaluate` in place of the JSON string, which skips the
per-call JSON parse:

```php
use Goplasmatic\Datalogic\DataHandle;

$data = new DataHandle('{"user":{"age":42,"plan":"pro"}}');
$rule->evaluate($data);              // same result as the string overload
$session->evaluate($rule, $data);    // hot path: zero parse work per call
```

Handles are immutable and engine-independent: one handle can feed
rules compiled by different engines, any number of times (evaluation
never consumes it). The binding releases the native memory when PHP
collects the object, or when you call `close()`;
`allocatedBytes()` reports the handle's resident size.

## Typed evaluations

Sessions can return native PHP scalars instead of JSON strings, which
suits predicates (feature flags, routing) where decoding JSON per call
is pure overhead. All four take a compiled `Rule` and a
`DataHandle`:

```php
$session->evaluateBool($rule, $data);    // bool  (strict: JSON true/false only)
$session->evaluateInt($rule, $data);     // int   (exact integers only)
$session->evaluateFloat($rule, $data);   // float (any JSON number)
$session->evaluateTruthy($rule, $data);  // bool  (JSONLogic truthiness, never mismatches)
```

The strict variants throw `EvaluateException` with
`$errorType === "TypeMismatch"` when the result is of any other type;
`evaluateTruthy` collapses any result through the engine's configured
truthiness rules (the same coercion `if`/`and`/`or` apply).

`$engine->truthy($valueJson)` applies the same truthiness to a value
you already hold, so a host check agrees with the engine. The argument
is JSON text: `$engine->truthy('[]')` and `$engine->truthy('{}')` are
`false` under the default rules, and a configured `truthy_evaluator`
applies.

## Batch evaluation

Evaluate one rule against many payloads (`evaluateBatch`) or many
rules against one payload (`evaluateMany`, the rule-set / feature-flag
shape) in a single native call. Results come back in input order; a
failed item puts a `BatchItemError` in its slot instead of aborting
the other N-1; item failures never throw:

```php
use Goplasmatic\Datalogic\BatchItemError;

$results = $session->evaluateBatch($rule, $handles);   // list<DataHandle> in
$results = $session->evaluateMany($rules, $data);      // list<Rule> in

foreach ($results as $i => $r) {
    if ($r instanceof BatchItemError) {
        error_log("item {$i} failed: {$r->tag}: {$r->message}");
        continue;
    }
    // $r is the item's JSON-string result
}
```

`BatchItemError` exposes `$status` (the raw C-ABI status code), `$tag`
(stable engine tag, for example `"Thrown"`, `"TypeError"`,
`"InvalidOperator"`), `$message`, and `$operator` (innermost failing
operator, when known). An item whose error the binding cannot decode
reads as tag `"InternalError"` with the raw item JSON as its message.

## API surface

The binding mirrors the Rust engine's
[API tier model](https://github.com/GoPlasmatic/datalogic-rs#one-api-shape-every-binding).
Rules and results cross the boundary as JSON strings, except where a
tier says otherwise: data can be a `DataHandle`, typed evaluations
return native PHP scalars, and batch calls return a list.

| Tier            | Entry point                                                    | Use when                                              |
|-----------------|----------------------------------------------------------------|-------------------------------------------------------|
| One-shot        | `$engine->apply($rule, $data)`                                 | Ad-hoc evaluation, one rule + one data shape          |
| Engine + config | `new Engine($templating)` / `Engine::builder()…->build()`      | Templating mode, custom operators, evaluation config  |
| Compile once    | `$engine->compile($rule)` → `$rule->evaluate($data)`           | Same rule evaluated against many data inputs          |
| Parse once      | `new DataHandle($json)` → pass instead of a JSON string        | Same payload evaluated by many rules/calls            |
| Session         | `$engine->openSession()` → `$session->evaluate($rule, $data)`  | Hot loops: amortise arena reset across iterations     |
| Typed           | `$session->evaluateBool/Int/Float/Truthy($rule, $handle)`      | Predicates: native scalars, no JSON decode per call   |
| Batch           | `$session->evaluateBatch($rule, $handles)` / `evaluateMany($rules, $handle)` | Many evaluations per FFI crossing, per-item errors |
| Traced          | `$engine->openTracedSession()` → `$session->evaluate($rule, $data)` | Step-by-step debugging; feeds the React debugger |
| Metered         | `$session->evaluateMetered($rule, $data, $budget)` → `['value' => …, 'ops' => …]` | What an evaluation costs, under an optional cap |
| Checked         | `$engine->check($rule, $mode)` / `$engine->compileChecked($rule)` | Reject a bad rule before it runs                |
| Introspection   | `$engine->operators()` / `$rule->facts()` / `$engine->truthy($json)` | Tooling: operator catalogue, what a rule reads |

`Rule::evaluate` and `Session::evaluate` accept either a JSON string or
a `DataHandle`.

## Custom operators

Register PHP-implemented operators through the builder. Each callback
receives the operator's pre-evaluated arguments as a JSON-array string
and returns a JSON-value string. If the callback throws, the evaluation
fails with error type `"Custom"` and a message that names the operator
and carries yours.

```php
$engine = Engine::builder()
    ->addOperator('double', function (string $argsJson): string {
        $args = json_decode($argsJson, true);
        return (string) ((int) $args[0] * 2);
    })
    ->build();
echo $engine->apply('{"double":[21]}', '{}');  // "42"
```

**Built-ins win**: registering a name that one of the engine's built-in
operators answers to (`+`, `if`, `var`, an alias such as `?:`) has no
effect at evaluation time; the built-in runs. Call
`withStrictOperatorNames(true)` before `addOperator` to make such a
registration throw `EvaluateException` (`$errorType ===
"ConfigurationError"`) instead:

```php
Engine::builder()
    ->withStrictOperatorNames(true)
    ->addOperator('length', fn (string $args): string => '0');  // throws: `length` is a built-in
```

A name from an operator family the engine leaves out (see
[Operator families](#operator-families)) is free for a custom operator.

## Engine configuration

`Engine::builder()->setConfigJson($json)` sets the evaluation semantics
from a JSON object string: an optional `preset` plus per-field
overrides. An unknown key or value makes `setConfigJson` throw
`EvaluateException` (error type `ConfigurationError`), so a typo fails
before the engine exists:

```php
$lenient = Engine::builder()
    ->setConfigJson('{"division_by_zero":"return_null"}')
    ->build();
echo $lenient->apply('{"/":[1.5,0]}', '{}');  // "null"

$strict = Engine::builder()
    ->setConfigJson('{"preset":"strict"}')
    ->build();
$strict->apply('{"+":["",1]}', '{}');         // throws: strict rejects non-numeric coercion
```

| Key | Values |
|-----|--------|
| `preset` | `"default"`, `"safe_arithmetic"`, `"strict"` |
| `arithmetic_nan_handling` | `"throw_error"`, `"ignore_value"`, `"coerce_to_zero"`, `"return_null"` |
| `division_by_zero` | `"return_saturated"`, `"throw_error"`, `"return_null"`, `"return_infinity"` |
| `loose_equality_errors` | `bool` |
| `missing_var` | `"null"` (default: a missing variable reads as `null`), `"error"` (raises `VariableNotFound`) |
| `truthy_evaluator` | `"javascript"`, `"python"`, `"strict_boolean"` |
| `numeric_coercion` | object of bools: `empty_string_to_zero`, `null_to_zero`, `bool_to_number`, `reject_non_numeric` |
| `max_recursion_depth` | integer >= 1 |
| `ops_budget` | integer >= 1, or `null` for unbounded (caps the work one evaluation may do; crossing it raises `BudgetExceeded`) |

The `preset` applies first; the remaining keys override individual
fields on top of it. Every binding shares this JSON schema and parses it
with the same core code, so a config that works here works in the
Python, Node, and WASM bindings too. The Rust crate's
[`EvaluationConfig`](https://docs.rs/datalogic-rs/latest/datalogic_rs/struct.EvaluationConfig.html)
documents the full semantics of each knob.

With `"missing_var": "error"`, a `var` / `val` read that finds nothing
throws `EvaluateException` with `$errorType === "VariableNotFound"`
naming the path, so a typo in a path fails instead of evaluating as
`null`. A default (`{"var": ["x", 0]}`), a present `null`, `missing`,
`missing_some` and `exists` are not misses, and `try` in the rule
catches the error.

The builder also takes `withTemplating(bool)`,
`withTemplateKeyEscape(string)` (a one-character prefix that marks a
template key as an output field: with `'$'`, `{"$type": ...}` emits the
key `type`), `withStrictOperatorNames(bool)` (see
[Custom operators](#custom-operators)) and `withFamilies(string ...)`
(below).

### Metering: what a rule costs

`$session->evaluateMetered($rule, $dataJson, $budget = 0)` returns
`['value' => string, 'ops' => int]`: the result JSON and the operations
the evaluation charged. A `$budget` of `0` uses the engine's
`ops_budget` (unbounded if it has none); any other value caps that one
call.

```php
$m = $engine->openSession()->evaluateMetered($rule, '{"xs": [1, 2, 3]}');
echo $m['value'], ' cost ', $m['ops'], " operations\n";
```

One operation is one node the engine dispatches, one item an iterator
walks, or what an operator charges for the data it moves. Crossing the
budget throws `EvaluateException` with `$errorType === "BudgetExceeded"`
before the work is done, and a `try` in the rule cannot catch it.

### Operator families

`$engine->operators()` returns the operator catalogue as a JSON array,
one object per built-in operator the engine evaluates (`name`,
`aliases`, `family`, `feature`, `min_args`, `max_args`,
`reads_context`, `effect`, `cost`, `scoped_arg`), in the schema of
[`operators.json`](https://github.com/GoPlasmatic/datalogic-rs/blob/main/docs/src/operators/operators.json).

`withFamilies(...)` keeps an engine to the JSONLogic core plus the
families you name: `ExtString`, `ExtArray`, `ExtObject`, `ExtControl`,
`ExtMath`, `ErrorHandling`, `DateTime`, `Tensor`, `Flagd`. By default an
engine has every family. A name from a family left out compiles as an
unknown operator, which `check` reports and evaluation rejects with
`$errorType === "InvalidOperator"`:

```php
$strings = Engine::builder()->withFamilies('ExtString')->build();
echo $strings->apply('{"length": "abc"}', 'null');  // 3
$strings->apply('{"now": []}', 'null');             // throws: InvalidOperator
```

An unknown family name throws `EvaluateException` with
`$errorType === "ConfigurationError"`. With strict operator names on,
call `withFamilies` before `addOperator`.

## Error handling

Engine errors are thrown as subclasses of
`Goplasmatic\Datalogic\Exception\DatalogicException` (a
`RuntimeException`):

| Exception           | When                                                      |
|---------------------|-----------------------------------------------------------|
| `ParseException`    | Malformed rule or data JSON (`"ParseError"`), or a rule `compileChecked` refused (`"CompileError"`) |
| `EvaluateException` | Everything else: a failure while evaluating, an unknown operator (`"InvalidOperator"`, also raised at compile for a multi-key object outside templating), a rejected config or builder option (`"ConfigurationError"`), a typed-result mismatch (`"TypeMismatch"`), a rule from a different engine (`"InvalidArgument"`) |

Misuse of the PHP API throws the standard exceptions: a
`RuntimeException` for a closed handle or a builder that already
built, and an `InvalidArgumentException` for a bad argument.

The structured fields ride on the base class as public readonly
properties: `$errorType` is the stable engine tag (for example
`"Thrown"`, `"TypeError"`, `"VariableNotFound"`, `"BudgetExceeded"`, or
the binding-level `"TypeMismatch"`), `$operatorName` the innermost
failing operator (custom operators included), `$pathJson` the
root-to-leaf error path as a JSON array, and `$nodeIdsJson` the
compiled-node ids from leaf to root; each is `null` when not
applicable. `$diagnosticsJson` is set only for `"CompileError"`.
Arithmetic NaN surfaces as `$errorType === "Thrown"` with a message
carrying `{"type":"NaN"}`; there is no `"NaN"` tag.

```php
use Goplasmatic\Datalogic\Exception\EvaluateException;

try {
    $engine->apply('{"+":["x",1]}', '{}');  // arithmetic on a non-numeric string throws {"type":"NaN"}
} catch (EvaluateException $e) {
    echo $e->errorType;     // "Thrown"
    echo $e->operatorName;  // "+"
    echo $e->pathJson;      // JSON-array path through the compiled tree
}
```

The binding targets the engine's C ABI **v2.1**: every fallible native
call returns a status code plus an owned error handle. The first time
the binding loads the library (at server start when you preload it) it
checks `datalogic_abi_version() == 2` and `datalogic_abi_minor() >= 1`,
and throws a `RuntimeException` naming both versions for a stale
library.

## Threading

| Type         | Pattern                                         |
|--------------|-------------------------------------------------|
| `Engine`     | Build once per process; reuse across requests   |
| `Rule`       | Compile once per process; reuse across requests |
| `DataHandle` | Immutable; share freely across engines/rules    |
| `Session`    | One per evaluation loop; do not share           |

PHP is single-threaded per request, so `Engine`, `Rule`, `Session`, and
`TracedSession` are all safe in that model.

Custom operators use PHP FFI's auto-coercion of PHP callables to C
function pointers. The engine keeps the callables, and every `Rule`,
`Session` and `TracedSession` keeps its engine, so custom operators
keep working after `$engine->close()` for as long as something
compiled or opened from it lives.

Each native handle has one owner. Since 5.8, passing a handle a wrapper
already owns to another wrapper's constructor (`new Rule($rule->handle())`)
throws `InvalidArgumentException` instead of freeing the handle twice
later.

## Tracing

```php
$session = $engine->openTracedSession();
$run = $session->evaluate('{"+":[{"var":"x"},1]}', '{"x":41}');
echo $run->result;             // 42
echo count($run->steps);       // number of execution steps
echo $run->pointers['3'];      // /+/1: node 3 is the literal 1
```

Same trace envelope as every other binding; the
[React debugger](https://github.com/GoPlasmatic/datalogic-rs/tree/main/ui)
reads it as is. `TracedRun` exposes `$result`, `$expressionTree`,
`$steps`, `$error`, `$structuredError` and `$pointers` (plus
`isSuccess()`); runtime failures surface inside the run rather than as
exceptions. `$pointers` maps each node id to the RFC 6901 JSON Pointer
of the rule value that node was compiled from, so a debugger can place
each step in the rule as written; it is empty when the rule does not
compile. `$session->evaluate($rule, $data, $mode)` compiles the rule in
a mode (`Native::MODE_TEMPLATE`, ...), so a template traces as one.
Tracing disables the optimizer so every operator appears in the trace:
use it for debugging, not hot paths.

## Performance

<!-- canonical-bench v5.1 -->
Geomean across 51 operator benchmark suites (Apple M2 Pro, median of 3 runs; pairwise shared-suite ratios per the [methodology](https://github.com/GoPlasmatic/datalogic-rs/blob/main/tools/benchmark/BENCHMARK.md)): the native Rust core evaluates at **10.3 ns/op**, 7.0× faster than json-logic-engine (compiled, the fastest JS engine), 28.1× faster than jsonlogic-rs (the closest Rust alternative), and 83.6× faster than the json-logic-js reference implementation. The WASM build under Node measures 900.5 ns geomean (88× native); on Node servers, prefer `@goplasmatic/datalogic-node`.

The PHP FFI boundary adds a small per-call marshalling cost on top of
the core numbers.

## Preloading (opcache.preload + FFI)

By default the binding calls `FFI::cdef` on first use, which works on
the CLI with no extra configuration. Production FPM/web SAPIs should use
[FFI preloading](https://www.php.net/manual/en/ffi.configuration.php)
instead: PHP's default `ffi.enable=preload` forbids runtime `FFI::cdef`
outside the CLI, and preloading also moves all header parsing to server
start. The package ships a ready-made preload script:

```ini
; php.ini
opcache.preload=/path/to/vendor/goplasmatic/datalogic/preload.php
opcache.preload_user=www-data
ffi.enable=preload
```

`preload.php` resolves the native library exactly like the runtime
loader does (see [Building from source](#building-from-source)),
rewrites the `FFI_LIB` line of the bundled header
(`src/datalogic-ffi.h`) to that absolute path, and registers the
persistent FFI scope `"datalogic"` via `FFI::load`. At request time the
binding finds the scope through `FFI::scope("datalogic")` and skips
`FFI::cdef` entirely; when no scope is preloaded it falls back to
`FFI::cdef`. To pin a specific library build, set the
`DATALOGIC_NATIVE_LIB` environment variable before the server starts.

If your application already has a preload script, `require` the
package's `preload.php` from it (it is idempotent), or call
`\Goplasmatic\Datalogic\Internal\Native::preload()` yourself.
If you manage your own headers, you can copy `src/datalogic-ffi.h`,
hard-code `FFI_LIB` to your library path, and `FFI::load` it
yourself; the committed default (`libdatalogic_c.so`) resolves via
the OS loader path. The header is also the single source of the cdef
declarations (Native.php reads it with the `#define` lines stripped),
so the two load paths cannot drift apart.

## Building from source

The binding lives in
[`bindings/php/`](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/php).
The FFI loader searches for the cdylib in order: the
`DATALOGIC_NATIVE_LIB` env var, the package's `lib/<os>-<arch>/` layout,
the in-tree C ABI target dir, then the OS's default loader paths. A
fresh clone needs the C ABI built once:

```bash
git clone https://github.com/GoPlasmatic/datalogic-rs
cd datalogic-rs/bindings/c && cargo build --release
cd ../php
composer install
vendor/bin/phpunit
```

## Learn more

- [datalogic-rs repository](https://github.com/GoPlasmatic/datalogic-rs#readme)
- [Rust crate deep-dive](https://github.com/GoPlasmatic/datalogic-rs/tree/main/crates/datalogic-rs#readme)
- [PHP docs chapter](https://goplasmatic.github.io/datalogic-rs/php.html)
- [Online playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [JSONLogic specification](https://jsonlogic.com)
- [C ABI internals](https://github.com/GoPlasmatic/datalogic-rs/tree/main/bindings/c#readme)

## License

Apache-2.0. See the
[main repository](https://github.com/GoPlasmatic/datalogic-rs) for
source and contribution guidelines.
