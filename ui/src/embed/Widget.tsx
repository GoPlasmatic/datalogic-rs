import { useState, useCallback, useEffect } from 'react';
import { DataLogicEditor, type JsonLogicValue } from '../components/logic-editor';
import { useWasmEvaluator } from '../components/logic-editor/hooks';
import { JsonHighlight } from './JsonHighlight';
import { JsonEditor } from './JsonEditor';
import { detectTheme, type WidgetProps } from './utils';

export type { WidgetProps };

export function Widget({ logic: initialLogic, data: initialData = {}, height = '500px', theme = 'auto', editable = false }: WidgetProps) {
  // Detect theme from mdBook or system
  const resolvedTheme = theme === 'auto' ? detectTheme() : theme;

  // State for editable inputs
  const [logicText, setLogicText] = useState<string>(JSON.stringify(initialLogic, null, 2));
  const [logic, setLogic] = useState<JsonLogicValue>(initialLogic);
  const [logicError, setLogicError] = useState<string | null>(null);

  const [dataText, setDataText] = useState<string>(JSON.stringify(initialData, null, 2));
  const [data, setData] = useState<object>(initialData);
  const [dataError, setDataError] = useState<string | null>(null);

  const [result, setResult] = useState<unknown>(undefined);
  const [resultError, setResultError] = useState<string | null>(null);

  const { ready: wasmReady, evaluate } = useWasmEvaluator({});

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
      setResultError(err instanceof Error ? err.message : 'Evaluation failed');
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
            placeholder="Enter JSON data..."
            className="widget-json-editor"
          />
          {dataError && <div className="widget-error">{dataError}</div>}
        </div>

        <div className="widget-column">
          <div className="widget-column-header">Result</div>
          <div className={`widget-result ${resultError ? 'has-error' : ''}`}>
            {resultError ? (
              <span className="widget-result-error">{resultError}</span>
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
          data={data}
          theme={resolvedTheme}
          className="datalogic-widget-editor"
          editable={editable}
        />
      </div>
    </div>
  );
}
