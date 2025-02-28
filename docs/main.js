import init, { JsJsonLogic } from './pkg/datalogic_rs.js';

let logic;

async function initWasm() {
    await init();
    logic = new JsJsonLogic();
}

document.addEventListener('DOMContentLoaded', async () => {
    // Initialize WASM
    await initWasm();

    // Get DOM elements
    const rulesEditor = CodeMirror.fromTextArea(document.getElementById('rules-editor'), {
        mode: { name: 'javascript', json: true },
        theme: 'material',
        lineNumbers: true,
        matchBrackets: true,
        autoCloseBrackets: true,
        tabSize: 2,
        gutters: ["CodeMirror-linenumbers"],
        lint: true
    });

    const dataEditor = CodeMirror.fromTextArea(document.getElementById('data-editor'), {
        mode: { name: 'javascript', json: true },
        theme: 'material',
        lineNumbers: true,
        matchBrackets: true,
        autoCloseBrackets: true,
        tabSize: 2,
        gutters: ["CodeMirror-linenumbers"],
        lint: true
    });

    const resultArea = document.getElementById('result-area');
    const evaluateButton = document.getElementById('evaluate-button');
    const sampleButton = document.getElementById('sample-button');

    // Initialize Material Design components
    const textFields = document.querySelectorAll('.mdc-text-field');
    textFields.forEach(textField => new mdc.textField.MDCTextField(textField));

    // Sample data
    const sampleRules = {
        "some": [
            {"var": "items"},
            {
                ">=": [{"var": "qty"}, 1]
            }
        ]
    };

    const sampleData = {
        "items": [
            {"qty": 1, "id": "first"},
            {"qty": 2, "id": "second"}
        ]
    };

    // Event handlers
    evaluateButton.addEventListener('click', async () => {
        try {
            const rules = JSON.parse(rulesEditor.getValue());
            let data = null;
            const dataValue = dataEditor.getValue().trim();
            if (dataValue) {
                data = JSON.parse(dataValue);
            }
            
            const result = await logic.apply(rules, data);
            
            resultArea.classList.remove('error');
            resultArea.classList.add('success');
            resultArea.textContent = JSON.stringify(result, null, 2);
        } catch (err) {
            resultArea.classList.remove('success');
            resultArea.classList.add('error');
            resultArea.textContent = `Error: ${err.message}`;
        }
    });

    sampleButton.addEventListener('click', () => {
        rulesEditor.value = JSON.stringify(sampleRules, null, 2);
        dataEditor.value = JSON.stringify(sampleData, null, 2);
        
        // Notify Material text fields of value change
        textFields.forEach(textField => textField.layout());
    });

    // Initial sample data
    sampleButton.click();
});