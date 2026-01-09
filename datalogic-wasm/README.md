# datalogic-wasm

WebAssembly bindings for [datalogic-rs](https://github.com/GoPlasmatic/datalogic-rs), a high-performance JSONLogic implementation in Rust.

## Building

### Prerequisites

- [Rust](https://rustup.rs/) with `wasm32-unknown-unknown` target
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

```bash
# Install wasm-pack
cargo install wasm-pack

# Add WASM target
rustup target add wasm32-unknown-unknown
```

### Build for Web

```bash
cd datalogic-wasm
wasm-pack build --target web
```

This creates a `pkg/` directory with:
- `datalogic_wasm.js` - JavaScript bindings
- `datalogic_wasm_bg.wasm` - WebAssembly binary
- `package.json` - npm package metadata

### Build for Node.js

```bash
wasm-pack build --target nodejs
```

### Build for Bundlers (webpack, etc.)

```bash
wasm-pack build --target bundler
```

## Usage

### In Browser (ES Modules)

```html
<script type="module">
import init, { evaluate, CompiledRule } from './pkg/datalogic_wasm.js';

async function run() {
    await init();

    // Simple evaluation
    const result = evaluate('{"==": [1, 1]}', '{}');
    console.log(result); // "true"

    // With data
    const result2 = evaluate('{"var": "x"}', '{"x": 42}');
    console.log(result2); // "42"

    // Compiled rule for repeated evaluation
    const rule = new CompiledRule('{"+": [{"var": "a"}, {"var": "b"}]}');
    console.log(rule.evaluate('{"a": 1, "b": 2}')); // "3"
    console.log(rule.evaluate('{"a": 10, "b": 20}')); // "30"
}

run();
</script>
```

### In Node.js

```javascript
const { evaluate, CompiledRule } = require('./pkg/datalogic_wasm.js');

// Simple evaluation
const result = evaluate('{"==": [1, 1]}', '{}');
console.log(result); // "true"

// Compiled rule
const rule = new CompiledRule('{"+": [{"var": "a"}, {"var": "b"}]}');
console.log(rule.evaluate('{"a": 1, "b": 2}')); // "3"
```

## API

### `evaluate(logic: string, data: string): string`

Evaluate a JSONLogic expression against data.

- `logic` - JSON string containing the JSONLogic expression
- `data` - JSON string containing the data to evaluate against
- Returns: JSON string result
- Throws: Error string on invalid JSON or evaluation error

### `CompiledRule`

A compiled JSONLogic rule for repeated evaluation.

#### `new CompiledRule(logic: string)`

Create a new compiled rule.

- `logic` - JSON string containing the JSONLogic expression
- Throws: Error string on invalid JSON or compilation error

#### `evaluate(data: string): string`

Evaluate the compiled rule against data.

- `data` - JSON string containing the data
- Returns: JSON string result
- Throws: Error string on invalid JSON or evaluation error

## Running Tests

```bash
# Run tests in headless Chrome
wasm-pack test --headless --chrome

# Run tests in headless Firefox
wasm-pack test --headless --firefox
```

## License

Apache-2.0
