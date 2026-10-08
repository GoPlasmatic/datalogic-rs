# Coming from json-logic-js

[json-logic-js](https://github.com/jwadhams/json-logic-js) is the reference
JSONLogic implementation. datalogic-rs passes the same official JSONLogic
test suite, so **your existing rules run unchanged**. What changes is the
call surface and a few defaults, listed below. See
[How It Compares](comparison.md) for the wider comparison.

## The one-liner

json-logic-js:

```javascript
import jsonLogic from 'json-logic-js';
jsonLogic.apply({ ">": [{ var: "age" }, 18] }, { age: 21 }); // true
```

datalogic-rs (Node, native binding):

```javascript
import { apply } from '@goplasmatic/datalogic-node';
apply({ ">": [{ var: "age" }, 18] }, { age: 21 }); // true
```

datalogic-rs (browser / WASM): the WASM binding takes and returns JSON text.

```javascript
import init, { Engine } from '@goplasmatic/datalogic-wasm';
await init();
const engine = new Engine();
engine.evalStr('{">": [{"var": "age"}, 18]}', '{"age": 21}'); // "true"
```

All three return the same result. To evaluate one rule many times, compile
it once with `engine.compile(rule)` and call `evaluate` on the returned
`Rule`, instead of calling a one-shot in a loop.

## Custom operations

json-logic-js registers operations globally:

```javascript
jsonLogic.add_operation("double", (a) => a * 2);
```

datalogic-rs registers them per engine, and the callback works in JSON
(evaluated arguments as a JSON-array string, result as a JSON string):

```javascript
import { Engine } from '@goplasmatic/datalogic-node';
const engine = new Engine({}, {
  double: (argsJson) => String(JSON.parse(argsJson)[0] * 2),
});
```

Each binding's chapter has the exact callback shape. Name a custom
operator after a built-in and it never runs; `strictOperatorNames: true`
in the engine options turns that into a `ConfigurationError`.

## Behavioral differences to know

datalogic-rs's defaults are stricter than json-logic-js's in a few places,
and most are configurable. The ones you are most likely to notice:

- **Cross-type loose equality.** By default `==` raises an error on a
  comparison that json-logic-js resolves to `false`, such as an object
  compared to a number. For the json-logic-js behavior, set
  `loose_equality_errors` to `false`.
- **Division by zero.** A fractional dividend over zero returns
  `±f64::MAX` by default (`ReturnSaturated`; `ReturnNull`, `ThrowError` and
  `ReturnInfinity` are the other `division_by_zero` modes). An integer
  divided by an integer zero raises `Thrown {"type": "NaN"}` in every mode.
- **Empty objects.** `{}` is falsy, like `[]`; json-logic-js treats it
  as truthy. In Rust, a `TruthyEvaluator::Custom` closure can restore the
  json-logic-js rule.
- **Large integers.** Integers keep their `i64` value, so two integers above
  2^53 that JavaScript rounds to one number compare as different. This
  applies to rules and data passed as JSON text; a JS number has already
  been rounded before the Node binding sees it.

These settings live on `EvaluationConfig`, or the `config` engine option in
the bindings; see [Configuration](advanced/configuration.md).

## Extensions you gain

Beyond the JSONLogic baseline, datalogic-rs adds operators the reference
engine does not ship: datetime arithmetic, string helpers (`length`,
`starts_with`, `split`, ...), `sort` / `slice` / `group_by` / `distinct`,
`keys` / `values` / `entries`, `try` / `throw`, `switch`, and
flagd-compatible feature-flag operators (`fractional`, `sem_ver`). In the
Rust crate these sit behind Cargo features; every language binding enables
them all. See the [operator overview](operators/overview.md).

You also gain checks json-logic-js does not have: `engine.check(rule)`
lists unknown operators (with a suggestion) and wrong argument counts
before the rule runs ([Rule Analysis](advanced/rule-analysis.md)), and the
`missing_var: "error"` config setting makes a read of a missing data path
fail instead of returning `null`.
