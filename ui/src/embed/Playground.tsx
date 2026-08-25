import { useState, useCallback, useEffect } from 'react';
import { DataLogicEditor, type JsonLogicValue } from '../components/logic-editor';
import { useWasmEvaluator, DataLogicEvaluationError } from '../components/logic-editor/hooks';
import { ErrorDisplay, type DebugError } from '../components/debug-panel/ErrorDisplay';
import { EMBED_SAMPLE_EXPRESSIONS as SAMPLE_EXPRESSIONS } from '../constants/embed-sample-expressions';
import { JsonHighlight } from './JsonHighlight';
import { JsonEditor } from './JsonEditor';
import { detectTheme, type PlaygroundProps } from './utils';

export type { PlaygroundProps };

export function Playground({ editable = false, templating: initialTemplating = false }: PlaygroundProps) {

  const [logicText, setLogicText] = useState<string>('');
  const [expression, setExpression] = useState<JsonLogicValue | null>(null);
  const [logicError, setLogicError] = useState<string | null>(null);

  const [dataText, setDataText] = useState<string>('{}');
  const [data, setData] = useState<unknown>({});
  const [dataError, setDataError] = useState<string | null>(null);

  const [result, setResult] = useState<unknown>(undefined);
  const [resultError, setResultError] = useState<DebugError>(null);

  const [templating, setTemplating] = useState<boolean>(initialTemplating);
  const [selectedExample, setSelectedExample] = useState<string>('');

  // Detect theme
  const theme = detectTheme();

  const { ready: wasmReady, loading: wasmLoading, error: wasmError, evaluate } = useWasmEvaluator({ templating });

  // Handle logic text changes
  const handleLogicChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const text = e.target.value;
    setLogicText(text);

    if (!text.trim()) {
      setExpression(null);
      setLogicError(null);
      return;
    }

    try {
      const parsed = JSON.parse(text);
      setExpression(parsed);
      setLogicError(null);
    } catch (err) {
      setLogicError(err instanceof Error ? err.message : 'Invalid JSON');
    }
  }, []);

  // Handle data text changes: any JSON value is a valid root context
  const handleDataChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const text = e.target.value;
    setDataText(text);

    if (!text.trim()) {
      setData({});
      setDataError(null);
      return;
    }

    try {
      setData(JSON.parse(text));
      setDataError(null);
    } catch (err) {
      setDataError(err instanceof Error ? err.message : 'Invalid JSON');
    }
  }, []);

  // Canvas edits flow back into the Logic text and the result
  const handleExpressionChange = useCallback((newExpr: JsonLogicValue | null) => {
    setExpression(newExpr);
    setLogicText(newExpr !== null ? JSON.stringify(newExpr, null, 2) : '');
    setLogicError(null);
  }, []);

  // Load sample expression (switching templating to match the sample)
  const loadSample = useCallback((name: string) => {
    const sample = SAMPLE_EXPRESSIONS[name];
    if (sample) {
      setSelectedExample(name);
      setExpression(sample.logic);
      setLogicText(JSON.stringify(sample.logic, null, 2));
      setLogicError(null);
      setData(sample.data);
      setDataText(JSON.stringify(sample.data, null, 2));
      setDataError(null);
      setTemplating(sample.templating ?? false);
    }
  }, []);

  // Load first sample on mount
  useEffect(() => {
    const firstName = Object.keys(SAMPLE_EXPRESSIONS)[0];
    // eslint-disable-next-line react-hooks/set-state-in-effect -- Initialization on mount is intentional
    loadSample(firstName);
  }, [loadSample]);

  // Evaluate expression when inputs change
  /* eslint-disable react-hooks/set-state-in-effect -- Derived state computation from expression/data changes */
  useEffect(() => {
    if (!wasmReady || !expression || logicError || dataError) {
      setResult(undefined);
      setResultError(null);
      return;
    }

    try {
      const evalResult = evaluate(expression, data);
      setResult(evalResult);
      setResultError(null);
    } catch (err) {
      setResult(undefined);
      if (err instanceof DataLogicEvaluationError) {
        setResultError(err.structured);
      } else {
        setResultError(err instanceof Error ? err.message : typeof err === 'string' ? err : 'Evaluation failed');
      }
    }
  }, [wasmReady, expression, data, logicError, dataError, evaluate]);
  /* eslint-enable react-hooks/set-state-in-effect */

  return (
    <div className="datalogic-playground" data-theme={theme}>
      {/* Header */}
      <div className="playground-header">
        <span className="playground-title">JSONLogic Playground</span>
        <div className="playground-controls">
          <label className="playground-templating-toggle" title="Compile multi-key objects as output templates with embedded JSONLogic">
            <input
              type="checkbox"
              checked={templating}
              onChange={(e) => setTemplating(e.target.checked)}
            />
            <span>Templating</span>
          </label>
          <select
            className="playground-examples"
            value={selectedExample}
            onChange={(e) => loadSample(e.target.value)}
          >
            <option value="" disabled>
              Load Example...
            </option>
            {Object.keys(SAMPLE_EXPRESSIONS).map((name) => (
              <option key={name} value={name}>
                {name}
              </option>
            ))}
          </select>
        </div>
      </div>

      {/* Row 1: Logic, Data, Result columns */}
      <div className="playground-input-row">
        <div className="playground-column">
          <div className="playground-column-header">Logic</div>
          <JsonEditor
            value={logicText}
            onChange={handleLogicChange}
            hasError={!!logicError}
            placeholder="Enter JSONLogic expression..."
            className="playground-json-editor"
          />
          {logicError && <div className="playground-error">{logicError}</div>}
        </div>

        <div className="playground-column">
          <div className="playground-column-header">Data</div>
          <JsonEditor
            value={dataText}
            onChange={handleDataChange}
            hasError={!!dataError}
            placeholder="Enter JSON data (object, array or scalar)..."
            className="playground-json-editor"
          />
          {dataError && <div className="playground-error">{dataError}</div>}
        </div>

        <div className="playground-column">
          <div className="playground-column-header">Result</div>
          <div className={`playground-result ${resultError || wasmError ? 'has-error' : ''}`}>
            {wasmError ? (
              <span className="playground-result-error">Failed to load WASM: {wasmError}</span>
            ) : resultError ? (
              <div className="playground-result-error embed-error">
                <ErrorDisplay error={resultError} />
              </div>
            ) : wasmLoading ? (
              <span className="json-highlight json-null">Loading WASM...</span>
            ) : (
              <JsonHighlight value={result} placeholder="" />
            )}
          </div>
        </div>
      </div>

      {/* Row 2: Diagram */}
      <div className="playground-diagram-row">
        <DataLogicEditor
          value={expression}
          onChange={editable ? handleExpressionChange : undefined}
          data={data}
          theme={theme}
          editable={editable}
          templating={templating}
          onTemplatingChange={setTemplating}
        />
      </div>
    </div>
  );
}
