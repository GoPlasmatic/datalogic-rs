import { useState, useCallback, useEffect } from 'react';
import { DataLogicEditor, type JsonLogicValue } from '../components/logic-editor';
import { useWasmEvaluator, DataLogicEvaluationError } from '../components/logic-editor/hooks';
import { ErrorDisplay, type DebugError } from '../components/debug-panel/ErrorDisplay';
import { JsonHighlight } from './JsonHighlight';
import { JsonEditor } from './JsonEditor';
import { detectTheme, type WidgetProps } from './utils';

export type { WidgetProps };

export function Widget({
  logic: initialLogic,
  data: initialData = {},
  height = '500px',
  theme = 'auto',
  editable = false,
  templating: initialTemplating = false,
}: WidgetProps) {
  // Detect theme from mdBook or system
  const resolvedTheme = theme === 'auto' ? detectTheme() : theme;

  // State for editable inputs
  const [logicText, setLogicText] = useState<string>(JSON.stringify(initialLogic, null, 2));
  const [logic, setLogic] = useState<JsonLogicValue>(initialLogic);
  const [logicError, setLogicError] = useState<string | null>(null);

  const [dataText, setDataText] = useState<string>(JSON.stringify(initialData, null, 2));
  const [data, setData] = useState<unknown>(initialData);
  const [dataError, setDataError] = useState<string | null>(null);

  const [result, setResult] = useState<unknown>(undefined);
  const [resultError, setResultError] = useState<DebugError>(null);

  const [templating, setTemplating] = useState<boolean>(initialTemplating);

  const { ready: wasmReady, evaluate } = useWasmEvaluator({ templating });

  // Handle logic text changes
  const handleLogicChange = useCallback((e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const text = e.target.value;
    setLogicText(text);

    if (!text.trim()) {
      setLogic({});
      setLogicError(null);
      return;
    }

    try {
      const parsed = JSON.parse(text);
      setLogic(parsed);
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
    const next = newExpr ?? {};
    setLogic(next);
    setLogicText(JSON.stringify(next, null, 2));
    setLogicError(null);
  }, []);

  // Evaluate expression when inputs change
  /* eslint-disable react-hooks/set-state-in-effect -- Derived state computation from expression/data changes */
  useEffect(() => {
    if (!wasmReady || logicError || dataError) {
      setResult(undefined);
      setResultError(null);
      return;
    }

    try {
      const evalResult = evaluate(logic, data);
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
  }, [wasmReady, logic, data, logicError, dataError, evaluate]);
  /* eslint-enable react-hooks/set-state-in-effect */

  return (
    <div className="datalogic-widget" style={{ height }} data-theme={resolvedTheme}>
      {/* Row 1: Logic, Data, Result columns */}
      <div className="widget-input-row">
        <div className="widget-column">
          <div className="widget-column-header">Logic</div>
          <JsonEditor
            value={logicText}
            onChange={handleLogicChange}
            hasError={!!logicError}
            placeholder="Enter JSONLogic expression..."
            className="widget-json-editor"
          />
          {logicError && <div className="widget-error">{logicError}</div>}
        </div>

        <div className="widget-column">
          <div className="widget-column-header">Data</div>
          <JsonEditor
            value={dataText}
            onChange={handleDataChange}
            hasError={!!dataError}
            placeholder="Enter JSON data (object, array or scalar)..."
            className="widget-json-editor"
          />
          {dataError && <div className="widget-error">{dataError}</div>}
        </div>

        <div className="widget-column">
          <div className="widget-column-header">Result</div>
          <div className={`widget-result ${resultError ? 'has-error' : ''}`}>
            {resultError ? (
              <div className="widget-result-error embed-error">
                <ErrorDisplay error={resultError} />
              </div>
            ) : (
              <JsonHighlight value={result} placeholder="" />
            )}
          </div>
        </div>
      </div>

      {/* Row 2: Diagram */}
      <div className="widget-diagram-row">
        <DataLogicEditor
          value={logic}
          onChange={editable ? handleExpressionChange : undefined}
          data={data}
          theme={resolvedTheme}
          className="datalogic-widget-editor"
          editable={editable}
          templating={templating}
          onTemplatingChange={setTemplating}
        />
      </div>
    </div>
  );
}
