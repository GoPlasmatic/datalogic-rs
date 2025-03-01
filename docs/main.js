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

    // Initialize result editor as readonly
    const resultEditor = CodeMirror.fromTextArea(document.getElementById('result-editor'), {
        mode: { name: 'javascript', json: true },
        theme: 'material',
        lineNumbers: true,
        matchBrackets: true,
        readOnly: true,
        tabSize: 2,
        gutters: ["CodeMirror-linenumbers"]
    });
    
    // Add readonly class to result editor
    resultEditor.getWrapperElement().classList.add('CodeMirror-readonly');

    const resultContainer = document.querySelector('.result-container');
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
            
            // Update the result editor with properly formatted JSON
            resultEditor.setValue(JSON.stringify(result, null, 2));
            resultEditor.refresh();
            
            // Update container classes
            resultContainer.classList.remove('result-error');
            resultContainer.classList.add('result-success');
        } catch (err) {
            // Display error in result editor
            resultEditor.setValue(`Error: ${err.message}`);
            resultEditor.refresh();
            
            // Update container classes
            resultContainer.classList.remove('result-success');
            resultContainer.classList.add('result-error');
        }
    });

    sampleButton.addEventListener('click', () => {
        rulesEditor.setValue(JSON.stringify(sampleRules, null, 2));
        dataEditor.setValue(JSON.stringify(sampleData, null, 2));
    });

    // Initial sample data
    sampleButton.click();
});