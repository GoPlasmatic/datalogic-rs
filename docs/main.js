import { JsonLogicEvaluator } from './jsonlogic.js';

let evaluator;

async function initWasm() {
    evaluator = new JsonLogicEvaluator();
    await evaluator.initialize();
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
    const sampleRules = evaluator.getSampleRules();
    const sampleData = evaluator.getSampleData();

    // Event handlers
    evaluateButton.addEventListener('click', async () => {
        try {
            const rules = JSON.parse(rulesEditor.getValue());
            let data = null;
            const dataValue = dataEditor.getValue().trim();
            if (dataValue) {
                data = JSON.parse(dataValue);
            }
            
            const result = await evaluator.evaluate(rules, data);
            
            if (result.success) {
                // Update the result editor with properly formatted JSON
                resultEditor.setValue(JSON.stringify(result.result, null, 2));
                resultEditor.refresh();
                
                // Update container classes
                resultContainer.classList.remove('result-error');
                resultContainer.classList.add('result-success');
            } else {
                // Display error in result editor
                resultEditor.setValue(`Error: ${result.error}`);
                resultEditor.refresh();
                
                // Update container classes
                resultContainer.classList.remove('result-success');
                resultContainer.classList.add('result-error');
            }
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