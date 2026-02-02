# @goplasmatic/datalogic

High-performance [JSONLogic](https://jsonlogic.com/) engine for JavaScript/TypeScript, powered by WebAssembly.

This package provides WebAssembly bindings for [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs), a Rust implementation of JSONLogic that supports all standard operators plus extended functionality.

## Installation

```bash
npm install @goplasmatic/datalogic
```

## Quick Start

```javascript
import init, { evaluate, CompiledRule } from '@goplasmatic/datalogic';

// Initialize the WASM module (required for web/ES modules)
await init();

// Simple evaluation
const result = evaluate('{"==": [1, 1]}', '{}', false);
console.log(result); // "true"

// With data
const result2 = evaluate('{"var": "user.age"}', '{"user": {"age": 25}}', false);
console.log(result2); // "25"

// Compiled rule for repeated evaluation (better performance)
const rule = new CompiledRule('{"+": [{"var": "a"}, {"var": "b"}]}', false);
console.log(rule.evaluate('{"a": 1, "b": 2}')); // "3"
console.log(rule.evaluate('{"a": 10, "b": 20}')); // "30"
```

## Usage by Environment

### Browser (ES Modules)

```html
<script type="module">
import init, { evaluate, CompiledRule } from '@goplasmatic/datalogic';

async function run() {
    // Initialize WASM module
    await init();

    // Now you can use evaluate and CompiledRule
    const result = evaluate('{"and": [true, {"var": "active"}]}', '{"active": true}', false);
    console.log(result); // "true"
}

run();
</script>
```

### Node.js

```javascript
// ESM
import { evaluate, CompiledRule } from '@goplasmatic/datalogic';

// No init() needed for Node.js
const result = evaluate('{"==": [1, 1]}', '{}', false);
console.log(result); // "true"

// Compiled rule
const rule = new CompiledRule('{"if": [{"var": "premium"}, "VIP", "Standard"]}', false);
console.log(rule.evaluate('{"premium": true}')); // "\"VIP\""
console.log(rule.evaluate('{"premium": false}')); // "\"Standard\""
```

### Bundlers (Webpack, Vite, etc.)

```javascript
import init, { evaluate, CompiledRule } from '@goplasmatic/datalogic';

// For bundlers, you may need to initialize
await init();

const result = evaluate('{">=": [{"var": "score"}, 80]}', '{"score": 85}', false);
console.log(result); // "true"
```

### Explicit Target Imports

If you need to import a specific target build:

```javascript
// Web target (ES modules with init)
import init, { evaluate } from '@goplasmatic/datalogic/web';

// Bundler target
import init, { evaluate } from '@goplasmatic/datalogic/bundler';

// Node.js target
import { evaluate } from '@goplasmatic/datalogic/nodejs';
```

## API Reference

### `evaluate(logic: string, data: string, preserve_structure: boolean): string`

Evaluate a JSONLogic expression against data.

**Parameters:**
- `logic` - JSON string containing the JSONLogic expression
- `data` - JSON string containing the data to evaluate against
- `preserve_structure` - If `true`, preserves object structure for JSON templates with embedded JSONLogic (templating mode)

**Returns:** JSON string result

**Throws:** Error string on invalid JSON or evaluation error

```javascript
evaluate('{"==": [{"var": "x"}, 5]}', '{"x": 5}', false); // "true"
evaluate('{"+": [1, 2, 3]}', '{}', false); // "6"
evaluate('{"map": [[1,2,3], {"+": [{"var": ""}, 1]}]}', '{}', false); // "[2,3,4]"

// With preserve_structure for templating
evaluate('{"name": {"var": "user"}, "active": true}', '{"user": "Alice"}', true);
// '{"name":"Alice","active":true}'
```

### `evaluate_with_trace(logic: string, data: string, preserve_structure: boolean): string`

Evaluate with execution trace for debugging. Returns detailed step-by-step information about how the expression was evaluated.

**Parameters:**
- `logic` - JSON string containing the JSONLogic expression
- `data` - JSON string containing the data to evaluate against
- `preserve_structure` - If `true`, preserves object structure for JSON templates with embedded JSONLogic (templating mode)

**Returns:** JSON string containing `TracedResult` with:
- `result` - The evaluation result
- `expression_tree` - Tree structure of the expression with node IDs
- `steps` - Array of execution steps with context and intermediate results

```javascript
const trace = evaluate_with_trace('{"and": [true, {"var": "x"}]}', '{"x": true}', false);
console.log(JSON.parse(trace));
// {
//   "result": true,
//   "expression_tree": { "id": 0, "expression": "{\"and\": [...]}", ... },
//   "steps": [...]
// }
```

### `CompiledRule`

A compiled JSONLogic rule for repeated evaluation. Pre-compiling rules provides better performance when evaluating the same logic against different data.

#### `new CompiledRule(logic: string, preserve_structure: boolean)`

Create a new compiled rule.

**Parameters:**
- `logic` - JSON string containing the JSONLogic expression
- `preserve_structure` - If `true`, preserves object structure for JSON templates with embedded JSONLogic (templating mode)

```javascript
const rule = new CompiledRule('{">=": [{"var": "age"}, 18]}', false);
```

#### `evaluate(data: string): string`

Evaluate the compiled rule against data.

```javascript
rule.evaluate('{"age": 21}'); // "true"
rule.evaluate('{"age": 16}'); // "false"
```

## Supported Operators

This library supports 59 built-in operators covering all standard JSONLogic plus extended functionality:

**Logical:** `and`, `or`, `!`, `!!`

**Comparison:** `==`, `===`, `!=`, `!==`, `<`, `<=`, `>`, `>=`

**Arithmetic:** `+`, `-`, `*`, `/`, `%`, `min`, `max`, `abs`, `ceil`, `floor`

**Control Flow:** `if`, `?:`, `??` (coalesce)

**Array:** `map`, `filter`, `reduce`, `all`, `some`, `none`, `merge`, `in`, `sort`, `slice`

**String:** `cat`, `substr`, `starts_with`, `ends_with`, `upper`, `lower`, `trim`, `split`, `length`

**Data Access:** `var`, `val`, `exists`, `missing`, `missing_some`

**Date/Time:** `now`, `datetime`, `timestamp`, `parse_date`, `format_date`, `date_diff`

**Error Handling:** `try`, `throw`

**Type Operations:** `type`

**Special:** `preserve` (structure preservation for templating)

For the complete list and documentation, see the [main repository](https://github.com/GoPlasmatic/datalogic-rs).

## Performance

This WASM-based implementation provides near-native performance:

- **Compiled rules** are significantly faster for repeated evaluations
- **Zero-copy** where possible between JS and WASM
- **Small bundle size** (~50KB gzipped)

## Building from Source

### Prerequisites

- [Rust](https://rustup.rs/) with `wasm32-unknown-unknown` target
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

```bash
# Install wasm-pack
cargo install wasm-pack

# Add WASM target
rustup target add wasm32-unknown-unknown
```

### Build

```bash
cd wasm
./build.sh
```

This creates a `pkg/` directory with builds for all targets (web, bundler, nodejs).

### Running Tests

```bash
# Run tests in headless Chrome
wasm-pack test --headless --chrome

# Run tests in headless Firefox
wasm-pack test --headless --firefox
```

## License

Apache-2.0

## Documentation

For complete documentation including advanced usage, framework integration, and API details, see the [full documentation](https://goplasmatic.github.io/datalogic-rs/javascript/installation.html).

## Links

- [GitHub Repository](https://github.com/GoPlasmatic/datalogic-rs)
- [Full Documentation](https://goplasmatic.github.io/datalogic-rs/)
- [Online Playground](https://goplasmatic.github.io/datalogic-rs/playground/)
- [JSONLogic Specification](https://jsonlogic.com/)
