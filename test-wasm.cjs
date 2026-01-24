#!/usr/bin/env node

/**
 * WASM Test Runner
 *
 * Runs the test suites from tests/suites/ against the WASM package
 * to verify all operators work correctly in WebAssembly.
 *
 * Usage:
 *   node test-wasm.cjs              # Run all tests
 *   node test-wasm.cjs datetime     # Run tests matching 'datetime'
 */

const { readFileSync, existsSync } = require('fs');
const { join, resolve } = require('path');
const { execSync } = require('child_process');

const SUITES_DIR = join(__dirname, 'tests', 'suites');
const WASM_PKG_DIR = join(__dirname, 'wasm', 'pkg', 'nodejs');

// Import WASM package - need to manually load since package.json has type: module
let evaluate, CompiledRule;
try {
    // Temporarily patch the environment to load CommonJS
    const wasmPath = join(WASM_PKG_DIR, 'datalogic_wasm.js');
    const wasmBgPath = join(WASM_PKG_DIR, 'datalogic_wasm_bg.wasm');

    if (!existsSync(wasmPath)) {
        throw new Error(`WASM file not found: ${wasmPath}`);
    }

    // Load the WASM module by reading and eval-ing it with proper context
    const wasmCode = readFileSync(wasmPath, 'utf-8');
    const wasmBytes = readFileSync(wasmBgPath);

    // Create a CommonJS-compatible module context
    const moduleExports = {};
    const moduleContext = {
        exports: moduleExports,
        module: { exports: moduleExports },
        require: require,
        __dirname: WASM_PKG_DIR,
        Buffer: Buffer,
        WebAssembly: WebAssembly,
        TextDecoder: TextDecoder,
        TextEncoder: TextEncoder,
        console: console,
        Error: Error,
        Symbol: Symbol,
        FinalizationRegistry: FinalizationRegistry,
        DataView: DataView,
        Uint8Array: Uint8Array,
        Array: Array
    };

    // Create a function that executes the module code
    const moduleFunction = new Function(
        'exports', 'module', 'require', '__dirname', 'Buffer',
        'WebAssembly', 'TextDecoder', 'TextEncoder', 'console', 'Error',
        'Symbol', 'FinalizationRegistry', 'DataView', 'Uint8Array', 'Array',
        wasmCode
    );

    moduleFunction(
        moduleContext.exports,
        moduleContext.module,
        moduleContext.require,
        moduleContext.__dirname,
        moduleContext.Buffer,
        moduleContext.WebAssembly,
        moduleContext.TextDecoder,
        moduleContext.TextEncoder,
        moduleContext.console,
        moduleContext.Error,
        moduleContext.Symbol,
        moduleContext.FinalizationRegistry,
        moduleContext.DataView,
        moduleContext.Uint8Array,
        moduleContext.Array
    );

    evaluate = moduleContext.exports.evaluate;
    CompiledRule = moduleContext.exports.CompiledRule;

    if (!evaluate) {
        throw new Error('evaluate function not found in WASM exports');
    }
} catch (err) {
    console.error('Failed to load WASM package. Run `cd wasm && ./build.sh` first.');
    console.error(err.message);
    console.error(err.stack);
    process.exit(1);
}

// Colors for terminal output
const colors = {
    red: '\x1b[31m',
    green: '\x1b[32m',
    yellow: '\x1b[33m',
    blue: '\x1b[34m',
    reset: '\x1b[0m',
    dim: '\x1b[2m'
};

function loadTestSuite(filePath) {
    const content = readFileSync(filePath, 'utf-8');
    const tests = JSON.parse(content);
    // Filter out comments (strings)
    return tests.filter(item => typeof item === 'object');
}

function deepEqual(a, b) {
    if (a === b) return true;
    if (a === null || b === null) return a === b;
    if (typeof a !== typeof b) return false;
    if (typeof a !== 'object') return a === b;

    if (Array.isArray(a) !== Array.isArray(b)) return false;

    if (Array.isArray(a)) {
        if (a.length !== b.length) return false;
        return a.every((item, i) => deepEqual(item, b[i]));
    }

    const keysA = Object.keys(a);
    const keysB = Object.keys(b);
    if (keysA.length !== keysB.length) return false;

    return keysA.every(key => deepEqual(a[key], b[key]));
}

function runTest(test, suiteName) {
    const { description, rule, data, result, error, preserve_structure } = test;

    try {
        const ruleJson = JSON.stringify(rule);
        const dataJson = JSON.stringify(data ?? null);

        const output = evaluate(ruleJson, dataJson, preserve_structure ?? false);
        const actual = JSON.parse(output);

        if (error) {
            // Expected an error but got a result
            return {
                passed: false,
                description,
                expected: `Error: ${JSON.stringify(error)}`,
                actual: output,
                suiteName
            };
        }

        if (deepEqual(actual, result)) {
            return { passed: true, description, suiteName };
        } else {
            return {
                passed: false,
                description,
                expected: JSON.stringify(result),
                actual: output,
                suiteName
            };
        }
    } catch (err) {
        const errMessage = err?.message || String(err);

        if (error) {
            // Expected an error and got one - pass (matching Rust test behavior)
            return { passed: true, description, suiteName };
        }

        return {
            passed: false,
            description,
            expected: JSON.stringify(result),
            actual: `Error: ${errMessage}`,
            suiteName,
            isError: true
        };
    }
}

function runAllTests(filter = null) {
    // Load index
    const indexPath = join(SUITES_DIR, 'index.json');
    if (!existsSync(indexPath)) {
        console.error(`Index file not found: ${indexPath}`);
        process.exit(1);
    }

    const suiteFiles = JSON.parse(readFileSync(indexPath, 'utf-8'));

    let totalPassed = 0;
    let totalFailed = 0;
    const failures = [];
    const errorSuites = new Set();
    const passedSuites = [];
    const failedSuites = [];

    for (const suiteFile of suiteFiles) {
        // Apply filter if provided
        if (filter && !suiteFile.toLowerCase().includes(filter.toLowerCase())) {
            continue;
        }

        const suitePath = join(SUITES_DIR, suiteFile);
        if (!existsSync(suitePath)) {
            console.warn(`${colors.yellow}⚠ Suite not found: ${suiteFile}${colors.reset}`);
            continue;
        }

        const tests = loadTestSuite(suitePath);
        let suitePassed = 0;
        let suiteFailed = 0;

        for (const test of tests) {
            const result = runTest(test, suiteFile);

            if (result.passed) {
                suitePassed++;
                totalPassed++;
            } else {
                suiteFailed++;
                totalFailed++;
                failures.push(result);
                if (result.isError) {
                    errorSuites.add(suiteFile);
                }
            }
        }

        const status = suiteFailed === 0
            ? `${colors.green}✓${colors.reset}`
            : `${colors.red}✗${colors.reset}`;

        if (suiteFailed > 0) {
            console.log(`${status} ${suiteFile}: ${suitePassed}/${suitePassed + suiteFailed} passed`);
            failedSuites.push(suiteFile);
        } else {
            console.log(`${status} ${suiteFile}: ${suitePassed}/${suitePassed + suiteFailed} passed`);
            passedSuites.push(suiteFile);
        }
    }

    // Summary
    console.log('\n' + '='.repeat(60));
    console.log(`${colors.blue}Summary${colors.reset}`);
    console.log('='.repeat(60));
    console.log(`Total: ${totalPassed + totalFailed} tests`);
    console.log(`${colors.green}Passed: ${totalPassed}${colors.reset}`);
    console.log(`${colors.red}Failed: ${totalFailed}${colors.reset}`);

    if (failures.length > 0) {
        console.log('\n' + '='.repeat(60));
        console.log(`${colors.red}Failed Tests${colors.reset}`);
        console.log('='.repeat(60));

        for (const failure of failures) {
            console.log(`\n${colors.red}✗ ${failure.suiteName}${colors.reset}`);
            console.log(`  ${failure.description}`);
            console.log(`  ${colors.dim}Expected:${colors.reset} ${failure.expected}`);
            console.log(`  ${colors.dim}Actual:${colors.reset}   ${failure.actual}`);
        }
    }

    if (errorSuites.size > 0) {
        console.log('\n' + '='.repeat(60));
        console.log(`${colors.yellow}Suites with Errors (potential WASM issues)${colors.reset}`);
        console.log('='.repeat(60));
        for (const suite of errorSuites) {
            console.log(`  - ${suite}`);
        }
    }

    return totalFailed === 0;
}

// Main
const filter = process.argv[2] || null;
console.log(`${colors.blue}WASM Test Runner${colors.reset}`);
console.log('='.repeat(60));
if (filter) {
    console.log(`Filter: ${filter}`);
    console.log('');
}

const success = runAllTests(filter);
process.exit(success ? 0 : 1);
