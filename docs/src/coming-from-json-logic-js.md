# Coming from json-logic-js

[json-logic-js](https://github.com/jwadhams/json-logic-js) is the reference
JSONLogic implementation. datalogic-rs passes the same official JSONLogic
test suite, so **your existing rules run unchanged**. What changes is the
call surface (one function per binding) and a few configurable behaviors.
This page is the short version; see [How It Compares](comparison.md) for the
positioning.

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

datalogic-rs (browser / WASM): the WASM binding is string-in, string-out.

```javascript
import init, { evaluate } from '@goplasmatic/datalogic-wasm';
await init();
evaluate('{">": [{"var": "age"}, 18]}', '{"age": 21}', false); // "true"
```

Same rule, same result. For repeated evaluation of one rule, compile it once
(`Engine`/`CompiledRule`) instead of calling the one-shot helper in a loop.

## Custom operations

json-logic-js registers operations globally:

```javascript
jsonLogic.add_operation("double", (a) => a * 2);
```

datalogic-rs registers them per engine, and the callback works in JSON
(pre-evaluated arguments as a JSON-array string, result as a JSON string):

```javascript
import { Engine } from '@goplasmatic/datalogic-node';
const engine = new Engine({}, {
  double: (argsJson) => String(JSON.parse(argsJson)[0] * 2),
});
```

See each binding's "Custom operators" section for the exact shape.

## Behavioral differences to know

datalogic-rs's defaults are slightly stricter than json-logic-js's, and are
configurable. The two you are most likely to notice:

- **Cross-type loose equality.** By default datalogic-rs raises on
  comparisons that json-logic-js would silently resolve to `false` (for
  example an object compared to a number). For json-logic-js-classic
  behavior, build the engine with `loose_equality_errors = false`.
- **Division by zero.** datalogic-rs is configurable
  (`ReturnSaturated` by default, or `ReturnNull` / `ThrowError` /
  `ReturnInfinity`); integer division by zero always errors. Pick the
  `division_by_zero` mode that matches your expectations.

Both live on `EvaluationConfig`; see [Configuration](advanced/configuration.md).

## Extensions you gain

Beyond the JSONLogic baseline, datalogic-rs adds opt-in operators the
reference engine does not ship: datetime arithmetic, string helpers
(`length`, `starts_with`, `split`, ...), `sort`/`slice`, `try`/`throw`,
`switch`, and flagd-compatible feature-flag operators (`fractional`,
`sem_ver`). In the Rust crate these sit behind Cargo features; every
language binding enables them all. See the
[operator overview](operators/overview.md).
