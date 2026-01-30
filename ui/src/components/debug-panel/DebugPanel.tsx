import { useState, useCallback } from 'react';
import { ChevronDown } from 'lucide-react';
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
  const [expandedSection, setExpandedSection] = useState<string>('logic');

  const toggleSection = useCallback((section: string) => {
    setExpandedSection(prev => prev === section ? '' : section);
  }, []);

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
      {/* Logic Input Section */}
      <div className={`debug-section logic-section ${expandedSection !== 'logic' ? 'collapsed' : 'expanded'}`}>
        <button
          className="debug-section-header"
          onClick={() => toggleSection('logic')}
          type="button"
        >
          <div className="debug-section-header-left">
            <ChevronDown
              size={14}
              className={`debug-section-chevron ${expandedSection !== 'logic' ? 'collapsed' : ''}`}
            />
            <h3>Logic</h3>
          </div>
          <div className="debug-section-header-right" onClick={e => e.stopPropagation()}>
            <button
              className="format-btn"
              onClick={handleFormatLogic}
              disabled={logic === null}
            >
              Format
            </button>
          </div>
        </button>
        {expandedSection === 'logic' && (
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
        )}
      </div>

      {/* Data Input Section */}
      <div className={`debug-section data-section ${expandedSection !== 'data' ? 'collapsed' : 'expanded'}`}>
        <button
          className="debug-section-header"
          onClick={() => toggleSection('data')}
          type="button"
        >
          <div className="debug-section-header-left">
            <ChevronDown
              size={14}
              className={`debug-section-chevron ${expandedSection !== 'data' ? 'collapsed' : ''}`}
            />
            <h3>Data</h3>
          </div>
          <div className="debug-section-header-right" onClick={e => e.stopPropagation()}>
            <button
              className="format-btn"
              onClick={handleFormatData}
              disabled={!!dataError}
            >
              Format
            </button>
          </div>
        </button>
        {expandedSection === 'data' && (
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
        )}
      </div>

      {/* Result Section */}
      <div className={`debug-section result-section ${expandedSection !== 'result' ? 'collapsed' : 'expanded'}`}>
        <button
          className="debug-section-header"
          onClick={() => toggleSection('result')}
          type="button"
        >
          <div className="debug-section-header-left">
            <ChevronDown
              size={14}
              className={`debug-section-chevron ${expandedSection !== 'result' ? 'collapsed' : ''}`}
            />
            <h3>Result</h3>
          </div>
          <div className="debug-section-header-right" onClick={e => e.stopPropagation()}>
            {wasmLoading && <span className="wasm-status loading">Loading WASM...</span>}
            {wasmReady && <span className="wasm-status ready">WASM Ready</span>}
          </div>
        </button>
        {expandedSection === 'result' && (
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
        )}
      </div>
    </div>
  );
}

export default DebugPanel;
