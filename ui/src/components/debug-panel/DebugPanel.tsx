import { useCallback } from 'react';
import type { JsonLogicValue } from '../logic-editor/types';
import { JsonEditor, JsonDisplay } from './JsonHighlighter';
import './DebugPanel.css';

interface DebugPanelProps {
  logic: JsonLogicValue | null;
  logicText: string;
  onLogicChange: (text: string) => void;
  logicError: string | null;
  data: unknown;
  dataText: string;
  onDataChange: (text: string) => void;
  dataError: string | null;
  result: unknown;
  resultError: string | null;
  wasmReady: boolean;
  wasmLoading: boolean;
}

export function DebugPanel({
  logic,
  logicText,
  onLogicChange,
  logicError,
  dataText,
  onDataChange,
  dataError,
  result,
  resultError,
  wasmReady,
  wasmLoading,
}: DebugPanelProps) {
  const handleFormatLogic = useCallback(() => {
    if (logic !== null) {
      onLogicChange(JSON.stringify(logic, null, 2));
    }
  }, [logic, onLogicChange]);

  const handleFormatData = useCallback(() => {
    try {
      const parsed = JSON.parse(dataText);
      onDataChange(JSON.stringify(parsed, null, 2));
    } catch {
      // Ignore format errors
    }
  }, [dataText, onDataChange]);

  return (
    <div className="debug-panel">
      {/* Logic Input Section - 50% */}
      <div className="debug-section logic-section">
        <div className="debug-section-header">
          <h3>Logic</h3>
          <button
            className="format-btn"
            onClick={handleFormatLogic}
            disabled={logic === null}
          >
            Format
          </button>
        </div>
        <div className="debug-section-content">
          <JsonEditor
            value={logicText}
            onChange={onLogicChange}
            placeholder="Enter JSONLogic expression..."
            hasError={!!logicError}
          />
          {logicError && (
            <div className="debug-error">
              <span className="error-icon">!</span>
              {logicError}
            </div>
          )}
        </div>
      </div>

      {/* Data Input Section - 25% */}
      <div className="debug-section data-section">
        <div className="debug-section-header">
          <h3>Data</h3>
          <button
            className="format-btn"
            onClick={handleFormatData}
            disabled={!!dataError}
          >
            Format
          </button>
        </div>
        <div className="debug-section-content">
          <JsonEditor
            value={dataText}
            onChange={onDataChange}
            placeholder="Enter data object (JSON)..."
            hasError={!!dataError}
          />
          {dataError && (
            <div className="debug-error">
              <span className="error-icon">!</span>
              {dataError}
            </div>
          )}
        </div>
      </div>

      {/* Result Section - 25% */}
      <div className="debug-section result-section">
        <div className="debug-section-header">
          <h3>Result</h3>
          {wasmLoading && <span className="wasm-status loading">Loading WASM...</span>}
          {wasmReady && <span className="wasm-status ready">WASM Ready</span>}
        </div>
        <div className="debug-section-content">
          {resultError ? (
            <div className="debug-result error">
              <span className="error-icon">!</span>
              {resultError}
            </div>
          ) : (
            <JsonDisplay value={result} />
          )}
        </div>
      </div>
    </div>
  );
}

export default DebugPanel;
