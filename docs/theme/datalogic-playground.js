/**
 * DataLogic Playground - Interactive JSONLogic evaluation
 * Provides inline "Try It" widgets and a full playground page
 */

// WASM module state
let wasmReady = false;
let wasmModule = null;
let initPromise = null;

// CDN URL for the published npm package
const WASM_CDN_URL = 'https://unpkg.com/@goplasmatic/datalogic@4.0.6/web/datalogic_wasm.js';

// Initialize WASM module
async function initWasm() {
    if (initPromise) return initPromise;

    initPromise = (async () => {
        try {
            // Dynamic import of WASM JS module from CDN
            const module = await import(WASM_CDN_URL);

            // Initialize WASM - the default export initializes the module
            await module.default();

            wasmModule = module;
            wasmReady = true;
            console.log('DataLogic WASM initialized successfully from CDN');
            return true;
        } catch (error) {
            console.error('Failed to initialize DataLogic WASM:', error);
            wasmReady = false;
            return false;
        }
    })();

    return initPromise;
}

// Evaluate JSONLogic expression
function evaluateLogic(logic, data) {
    if (!wasmReady || !wasmModule) {
        throw new Error('WASM module not initialized');
    }
    return wasmModule.evaluate(logic, data);
}

// Format JSON for display
function formatJson(str) {
    try {
        const obj = JSON.parse(str);
        return JSON.stringify(obj, null, 2);
    } catch {
        return str;
    }
}

// Validate JSON string
function isValidJson(str) {
    try {
        JSON.parse(str);
        return true;
    } catch {
        return false;
    }
}

// JSON syntax highlighting
function highlightJson(str) {
    // Escape HTML entities
    const escaped = str
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;');

    // Apply syntax highlighting
    return escaped
        // Strings (including keys in quotes)
        .replace(/"([^"\\]|\\.)*"/g, (match) => {
            // Check if this is a key (followed by :)
            return `<span class="json-string">${match}</span>`;
        })
        // Numbers
        .replace(/\b(-?\d+\.?\d*([eE][+-]?\d+)?)\b/g, '<span class="json-number">$1</span>')
        // Booleans
        .replace(/\b(true|false)\b/g, '<span class="json-boolean">$1</span>')
        // Null
        .replace(/\bnull\b/g, '<span class="json-null">null</span>')
        // Brackets and braces
        .replace(/([{}\[\]])/g, '<span class="json-bracket">$1</span>')
        // Highlight keys (strings followed by :)
        .replace(/<span class="json-string">("([^"\\]|\\.)*")<\/span>(\s*:)/g,
            '<span class="json-key">$1</span>$3');
}

// Sync scroll between textarea and highlight layer
function syncScroll(textarea, highlight) {
    highlight.scrollTop = textarea.scrollTop;
    highlight.scrollLeft = textarea.scrollLeft;
}

// Create an inline "Try It" widget
function createWidget(container) {
    const logic = container.dataset.logic || '{}';
    const data = container.dataset.data || '{}';
    const originalLogic = logic;
    const originalData = data;

    const formattedLogic = formatJson(logic);
    const formattedData = formatJson(data);

    container.innerHTML = `
        <div class="playground-widget-inner">
            <div class="playground-header">
                <span class="playground-title">Try It</span>
                <div class="playground-actions">
                    <button class="playground-btn playground-reset" title="Reset to original">Reset</button>
                    <button class="playground-btn playground-run" title="Run (Ctrl+Enter)">Run</button>
                </div>
            </div>
            <div class="playground-body">
                <div class="playground-editors">
                    <div class="playground-editor-group">
                        <label>Logic</label>
                        <div class="playground-editor-container">
                            <div class="playground-highlight" aria-hidden="true">${highlightJson(formattedLogic)}</div>
                            <textarea class="playground-logic" spellcheck="false">${formattedLogic}</textarea>
                        </div>
                    </div>
                    <div class="playground-editor-group">
                        <label>Data</label>
                        <div class="playground-editor-container">
                            <div class="playground-highlight" aria-hidden="true">${highlightJson(formattedData)}</div>
                            <textarea class="playground-data" spellcheck="false">${formattedData}</textarea>
                        </div>
                    </div>
                </div>
                <div class="playground-output">
                    <label>Result</label>
                    <div class="playground-result"></div>
                </div>
            </div>
        </div>
    `;

    const logicInput = container.querySelector('.playground-logic');
    const dataInput = container.querySelector('.playground-data');
    const logicHighlight = container.querySelector('.playground-editor-group:first-child .playground-highlight');
    const dataHighlight = container.querySelector('.playground-editor-group:last-child .playground-highlight');
    const resultDiv = container.querySelector('.playground-result');
    const runBtn = container.querySelector('.playground-run');
    const resetBtn = container.querySelector('.playground-reset');

    // Update highlighting on input
    function updateLogicHighlight() {
        logicHighlight.innerHTML = highlightJson(logicInput.value);
    }
    function updateDataHighlight() {
        dataHighlight.innerHTML = highlightJson(dataInput.value);
    }

    // Run evaluation
    function run() {
        const logicStr = logicInput.value.trim();
        const dataStr = dataInput.value.trim();

        // Validate JSON
        if (!isValidJson(logicStr)) {
            resultDiv.className = 'playground-result error';
            resultDiv.textContent = 'Invalid JSON in Logic field';
            return;
        }
        if (!isValidJson(dataStr)) {
            resultDiv.className = 'playground-result error';
            resultDiv.textContent = 'Invalid JSON in Data field';
            return;
        }

        try {
            const result = evaluateLogic(logicStr, dataStr);
            resultDiv.className = 'playground-result success';
            resultDiv.textContent = formatJson(result);
        } catch (error) {
            resultDiv.className = 'playground-result error';
            resultDiv.textContent = 'Error: ' + error.message;
        }
    }

    // Reset to original values
    function reset() {
        logicInput.value = formatJson(originalLogic);
        dataInput.value = formatJson(originalData);
        updateLogicHighlight();
        updateDataHighlight();
        resultDiv.className = 'playground-result';
        resultDiv.textContent = '';
    }

    // Event listeners
    runBtn.addEventListener('click', run);
    resetBtn.addEventListener('click', reset);

    // Input event for highlighting
    logicInput.addEventListener('input', updateLogicHighlight);
    dataInput.addEventListener('input', updateDataHighlight);

    // Scroll sync
    logicInput.addEventListener('scroll', () => syncScroll(logicInput, logicHighlight));
    dataInput.addEventListener('scroll', () => syncScroll(dataInput, dataHighlight));

    // Keyboard shortcut: Ctrl/Cmd + Enter to run
    // Also prevent arrow keys from triggering mdBook page navigation
    function handleKeydown(e) {
        // Stop propagation for all keys to prevent mdBook navigation
        e.stopPropagation();

        if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
            e.preventDefault();
            run();
        }
    }
    logicInput.addEventListener('keydown', handleKeydown);
    dataInput.addEventListener('keydown', handleKeydown);

    // Auto-run on initial load
    if (wasmReady) {
        run();
    }
}

// Initialize all playground widgets on the page
function initPlaygroundWidgets() {
    const widgets = document.querySelectorAll('.playground-widget');
    widgets.forEach(widget => {
        if (!widget.classList.contains('initialized')) {
            createWidget(widget);
            widget.classList.add('initialized');
        }
    });
}

// Create the full playground page
function initFullPlayground() {
    const container = document.getElementById('full-playground');
    if (!container) return;

    // Example templates
    const examples = {
        'Simple Comparison': {
            logic: '{ "==": [1, 1] }',
            data: '{}'
        },
        'Variable Access': {
            logic: '{ "var": "user.name" }',
            data: '{ "user": { "name": "Alice", "age": 30 } }'
        },
        'Conditional': {
            logic: '{ "if": [{ ">=": [{ "var": "age" }, 18] }, "adult", "minor"] }',
            data: '{ "age": 21 }'
        },
        'Array Filter': {
            logic: '{ "filter": [{ "var": "numbers" }, { ">": [{ "var": "" }, 5] }] }',
            data: '{ "numbers": [1, 3, 5, 7, 9, 11] }'
        },
        'Array Map': {
            logic: '{ "map": [{ "var": "items" }, { "*": [{ "var": "" }, 2] }] }',
            data: '{ "items": [1, 2, 3, 4, 5] }'
        },
        'String Concatenation': {
            logic: '{ "cat": ["Hello, ", { "var": "name" }, "!"] }',
            data: '{ "name": "World" }'
        },
        'Arithmetic': {
            logic: '{ "+": [{ "var": "a" }, { "var": "b" }, { "var": "c" }] }',
            data: '{ "a": 10, "b": 20, "c": 30 }'
        },
        'Nested Logic': {
            logic: '{ "and": [{ ">": [{ "var": "score" }, 50] }, { "==": [{ "var": "status" }, "active"] }] }',
            data: '{ "score": 75, "status": "active" }'
        },
        'Reduce': {
            logic: '{ "reduce": [{ "var": "numbers" }, { "+": [{ "var": "accumulator" }, { "var": "current" }] }, 0] }',
            data: '{ "numbers": [1, 2, 3, 4, 5] }'
        },
        'Feature Flag': {
            logic: '{ "and": [{ "==": [{ "var": "user.plan" }, "premium"] }, { ">=": [{ "var": "user.age" }, 18] }] }',
            data: '{ "user": { "plan": "premium", "age": 25 } }'
        }
    };

    const firstExample = Object.keys(examples)[0];
    const formattedLogic = formatJson(examples[firstExample].logic);
    const formattedData = formatJson(examples[firstExample].data);

    container.innerHTML = `
        <div class="full-playground-container">
            <div class="full-playground-header">
                <span class="full-playground-title">JSONLogic Playground</span>
                <div class="full-playground-controls">
                    <select class="playground-examples">
                        <option value="">Load Example...</option>
                        ${Object.keys(examples).map(name => `<option value="${name}">${name}</option>`).join('')}
                    </select>
                    <button class="playground-btn playground-reset" title="Format JSON">Format</button>
                    <button class="playground-btn playground-reset playground-clear" title="Clear all">Clear</button>
                    <button class="playground-btn playground-run" title="Run (Ctrl+Enter)">Run</button>
                </div>
            </div>
            <div class="full-playground-body">
                <div class="full-playground-editors">
                    <div class="full-playground-editor-group">
                        <label>Logic (JSONLogic Expression)</label>
                        <div class="full-playground-editor-container">
                            <div class="full-playground-highlight" aria-hidden="true">${highlightJson(formattedLogic)}</div>
                            <textarea class="full-playground-logic" spellcheck="false" placeholder='{"==": [1, 1]}'>${formattedLogic}</textarea>
                        </div>
                    </div>
                    <div class="full-playground-editor-group">
                        <label>Data (JSON Object)</label>
                        <div class="full-playground-editor-container">
                            <div class="full-playground-highlight" aria-hidden="true">${highlightJson(formattedData)}</div>
                            <textarea class="full-playground-data" spellcheck="false" placeholder='{}'>${formattedData}</textarea>
                        </div>
                    </div>
                </div>
                <div class="full-playground-output">
                    <label>Result</label>
                    <div class="full-playground-result"></div>
                </div>
            </div>
        </div>
    `;

    const logicInput = container.querySelector('.full-playground-logic');
    const dataInput = container.querySelector('.full-playground-data');
    const logicHighlight = container.querySelector('.full-playground-editor-group:first-child .full-playground-highlight');
    const dataHighlight = container.querySelector('.full-playground-editor-group:nth-child(2) .full-playground-highlight');
    const resultDiv = container.querySelector('.full-playground-result');
    const runBtn = container.querySelector('.playground-run');
    const formatBtn = container.querySelector('.playground-reset:not(.playground-clear)');
    const clearBtn = container.querySelector('.playground-clear');
    const examplesSelect = container.querySelector('.playground-examples');

    // Update highlighting on input
    function updateLogicHighlight() {
        logicHighlight.innerHTML = highlightJson(logicInput.value);
    }
    function updateDataHighlight() {
        dataHighlight.innerHTML = highlightJson(dataInput.value);
    }

    // Run evaluation
    function run() {
        const logicStr = logicInput.value.trim();
        const dataStr = dataInput.value.trim() || '{}';

        if (!logicStr) {
            resultDiv.className = 'full-playground-result error';
            resultDiv.textContent = 'Please enter a JSONLogic expression';
            return;
        }

        if (!isValidJson(logicStr)) {
            resultDiv.className = 'full-playground-result error';
            resultDiv.textContent = 'Invalid JSON in Logic field';
            return;
        }
        if (!isValidJson(dataStr)) {
            resultDiv.className = 'full-playground-result error';
            resultDiv.textContent = 'Invalid JSON in Data field';
            return;
        }

        try {
            const result = evaluateLogic(logicStr, dataStr);
            resultDiv.className = 'full-playground-result success';
            resultDiv.textContent = formatJson(result);
        } catch (error) {
            resultDiv.className = 'full-playground-result error';
            resultDiv.textContent = 'Error: ' + error.message;
        }
    }

    // Format JSON in editors
    function format() {
        try {
            logicInput.value = formatJson(logicInput.value);
            updateLogicHighlight();
        } catch {}
        try {
            dataInput.value = formatJson(dataInput.value);
            updateDataHighlight();
        } catch {}
    }

    // Clear all
    function clear() {
        logicInput.value = '';
        dataInput.value = '{}';
        updateLogicHighlight();
        updateDataHighlight();
        resultDiv.className = 'full-playground-result';
        resultDiv.textContent = '';
        examplesSelect.value = '';
    }

    // Load example
    function loadExample() {
        const name = examplesSelect.value;
        if (name && examples[name]) {
            logicInput.value = formatJson(examples[name].logic);
            dataInput.value = formatJson(examples[name].data);
            updateLogicHighlight();
            updateDataHighlight();
            run();
        }
    }

    // Event listeners
    runBtn.addEventListener('click', run);
    formatBtn.addEventListener('click', format);
    clearBtn.addEventListener('click', clear);
    examplesSelect.addEventListener('change', loadExample);

    // Input event for highlighting
    logicInput.addEventListener('input', updateLogicHighlight);
    dataInput.addEventListener('input', updateDataHighlight);

    // Scroll sync
    logicInput.addEventListener('scroll', () => syncScroll(logicInput, logicHighlight));
    dataInput.addEventListener('scroll', () => syncScroll(dataInput, dataHighlight));

    // Keyboard shortcut
    // Also prevent arrow keys from triggering mdBook page navigation
    function handleKeydown(e) {
        // Stop propagation for all keys to prevent mdBook navigation
        e.stopPropagation();

        if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
            e.preventDefault();
            run();
        }
    }
    logicInput.addEventListener('keydown', handleKeydown);
    dataInput.addEventListener('keydown', handleKeydown);

    // Auto-run on initial load
    if (wasmReady) {
        run();
    }
}

// Initialize on page load
document.addEventListener('DOMContentLoaded', async () => {
    // Show loading state for widgets
    document.querySelectorAll('.playground-widget').forEach(widget => {
        widget.innerHTML = '<div class="playground-loading">Loading playground...</div>';
    });

    const fullPlayground = document.getElementById('full-playground');
    if (fullPlayground) {
        fullPlayground.innerHTML = '<div class="playground-loading">Loading playground...</div>';
    }

    // Initialize WASM
    const success = await initWasm();

    if (success) {
        // Initialize widgets
        initPlaygroundWidgets();
        initFullPlayground();
    } else {
        // Show error state
        document.querySelectorAll('.playground-widget').forEach(widget => {
            widget.innerHTML = '<div class="playground-error">Failed to load playground. Please refresh the page.</div>';
        });
        if (fullPlayground) {
            fullPlayground.innerHTML = '<div class="playground-error">Failed to load playground. Please refresh the page.</div>';
        }
    }
});

// Re-initialize widgets when page content changes (for mdBook's navigation)
if (typeof window !== 'undefined') {
    // MutationObserver to detect page changes
    const observer = new MutationObserver((mutations) => {
        if (wasmReady) {
            initPlaygroundWidgets();
            initFullPlayground();
        }
    });

    // Start observing when DOM is ready
    document.addEventListener('DOMContentLoaded', () => {
        const content = document.getElementById('content');
        if (content) {
            observer.observe(content, { childList: true, subtree: true });
        }
    });
}
