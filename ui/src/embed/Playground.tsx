import { useState, useCallback, useEffect } from 'react';
import { DataLogicEditor, type JsonLogicValue } from '../components/logic-editor';
import { useWasmEvaluator } from '../components/logic-editor/hooks';
import { EMBED_SAMPLE_EXPRESSIONS as SAMPLE_EXPRESSIONS } from '../constants/embed-sample-expressions';
import { JsonHighlight } from './JsonHighlight';
import { JsonEditor } from './JsonEditor';
import { detectTheme, type PlaygroundProps } from './utils';

export type { PlaygroundProps };

export function Playground({ editable = false }: PlaygroundProps) {

  const [logicText, setLogicText] = useState<string>('');
  const [expression, setExpression] = useState<JsonLogicValue | null>(null);
  const [logicError, setLogicError] = useState<string | null>(null);

  const [dataText, setDataText] = useState<string>('{}');
  const [data, setData] = useState<object>({});
  const [dataError, setDataError] = useState<string | null>(null);

  const [result, setResult] = useState<unknown>(undefined);
  const [resultError, setResultError] = useState<string | null>(null);

  const [selectedExample, setSelectedExample] = useState<string>('');

  // Detect theme
  const theme = detectTheme();

  const { ready: wasmReady, loading: wasmLoading, evaluate } = useWasmEvaluator({});

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

  // Handle data text changes
  const handleDataChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const text = e.target.value;
    setDataText(text);

    if (!text.trim()) {
      setData({});
      setDataError(null);
      return;
    }

    try {
      const parsed = JSON.parse(text);
      if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
        setDataError('Data must be a JSON object');
        return;
      }
      setData(parsed);
      setDataError(null);
    } catch (err) {
      setDataError(err instanceof Error ? err.message : 'Invalid JSON');
    }
  }, []);

  // Load sample expression
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
      setResultError(err instanceof Error ? err.message : 'Evaluation failed');
    }
  }, [wasmReady, expression, data, logicError, dataError, evaluate]);
  /* eslint-enable react-hooks/set-state-in-effect */

  return (
    <div className="datalogic-playground" data-theme={theme}>
      {/* Header */}
      <div className="playground-header">
        <span className="playground-title">JSONLogic Playground</span>
        <div className="playground-controls">
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
            placeholder="Enter JSON data..."
            className="playground-json-editor"
          />
          {dataError && <div className="playground-error">{dataError}</div>}
        </div>

        <div className="playground-column">
          <div className="playground-column-header">Result</div>
          <div className={`playground-result ${resultError ? 'has-error' : ''}`}>
            {resultError ? (
              <span className="playground-result-error">{resultError}</span>
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
          data={data}
          theme={theme}
          editable={editable}
        />
      </div>
    </div>
  );
}
