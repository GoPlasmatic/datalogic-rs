import { useState, useCallback } from 'react';
import { ChevronDown, Copy, Check } from 'lucide-react';
import type { JsonLogicValue, StructuredError } from '../logic-editor/types';
import { JsonEditor, JsonDisplay } from './JsonHighlighter';
import { Tooltip } from '../Tooltip';
import './DebugPanel.css';

/** Error shape accepted by the debug panel: a plain string for parse-level
 * problems, or a `StructuredError` for runtime errors out of the engine. */
export type DebugError = StructuredError | string | null;

function ErrorDisplay({ error }: { error: Exclude<DebugError, null> }) {
  if (typeof error === 'string') {
    return (
      <>
        <span className="error-icon">!</span>
        {error}
      </>
    );
  }
  return (
    <>
      <span className="error-icon">!</span>
      <span className="error-type-pill" data-kind={error.type}>{error.type}</span>
      <span className="error-message">{error.message}</span>
      {error.operator && (
        <span className="error-operator-chip">op: {error.operator}</span>
      )}
    </>
  );
}

interface DebugPanelProps {
  logic: JsonLogicValue | null;
  logicText: string;
  onLogicChange: (text: string) => void;
  logicError: DebugError;
  data: unknown;
  dataText: string;
  onDataChange: (text: string) => void;
  dataError: DebugError;
  result: unknown;
  resultError: DebugError;
  wasmReady: boolean;
  wasmLoading: boolean;
  accordion?: boolean;
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
  accordion = false,
}: DebugPanelProps) {
  const [expandedSection, setExpandedSection] = useState<string>('logic');
  const [resultCopied, setResultCopied] = useState(false);

  const toggleSection = useCallback((section: string) => {
    setExpandedSection(prev => prev === section ? '' : section);
  }, []);

  const handleCopyResult = useCallback(async () => {
    if (resultError !== null || result === undefined) return;
    try {
      await navigator.clipboard.writeText(JSON.stringify(result, null, 2));
      setResultCopied(true);
      setTimeout(() => setResultCopied(false), 1500);
    } catch (err) {
      console.error('Failed to copy result:', err);
    }
  }, [result, resultError]);

  const isExpanded = (section: string) => !accordion || expandedSection === section;
  const sectionClass = (section: string) => {
    if (!accordion) return '';
    return expandedSection === section ? 'expanded' : 'collapsed';
  };

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
      <div className={`debug-section logic-section ${sectionClass('logic')}`}>
        <button
          className="debug-section-header"
          onClick={accordion ? () => toggleSection('logic') : undefined}
          type="button"
        >
          <div className="debug-section-header-left">
            {accordion && (
              <ChevronDown
                size={14}
                className={`debug-section-chevron ${!isExpanded('logic') ? 'collapsed' : ''}`}
              />
            )}
            <h3>Logic</h3>
          </div>
          <div className="debug-section-header-right" onClick={e => e.stopPropagation()}>
            <Tooltip label="Pretty-print this JSON" side="left">
              <button
                className="format-btn"
                onClick={handleFormatLogic}
                disabled={logic === null}
              >
                Format
              </button>
            </Tooltip>
          </div>
        </button>
        {isExpanded('logic') && (
          <div className="debug-section-content">
            <JsonEditor
              value={logicText}
              onChange={onLogicChange}
              placeholder="Enter JSONLogic expression..."
              hasError={!!logicError}
            />
            {logicError && (
              <div className="debug-error">
                <ErrorDisplay error={logicError} />
              </div>
            )}
          </div>
        )}
      </div>

      {/* Data Input Section */}
      <div className={`debug-section data-section ${sectionClass('data')}`}>
        <button
          className="debug-section-header"
          onClick={accordion ? () => toggleSection('data') : undefined}
          type="button"
        >
          <div className="debug-section-header-left">
            {accordion && (
              <ChevronDown
                size={14}
                className={`debug-section-chevron ${!isExpanded('data') ? 'collapsed' : ''}`}
              />
            )}
            <h3>Data</h3>
          </div>
          <div className="debug-section-header-right" onClick={e => e.stopPropagation()}>
            <Tooltip label="Pretty-print this JSON" side="left">
              <button
                className="format-btn"
                onClick={handleFormatData}
                disabled={!!dataError}
              >
                Format
              </button>
            </Tooltip>
          </div>
        </button>
        {isExpanded('data') && (
          <div className="debug-section-content">
            <JsonEditor
              value={dataText}
              onChange={onDataChange}
              placeholder="Enter data object (JSON)..."
              hasError={!!dataError}
            />
            {dataError && (
              <div className="debug-error">
                <ErrorDisplay error={dataError} />
              </div>
            )}
          </div>
        )}
      </div>

      {/* Result Section */}
      <div className={`debug-section result-section ${sectionClass('result')}`}>
        <button
          className="debug-section-header"
          onClick={accordion ? () => toggleSection('result') : undefined}
          type="button"
        >
          <div className="debug-section-header-left">
            {accordion && (
              <ChevronDown
                size={14}
                className={`debug-section-chevron ${!isExpanded('result') ? 'collapsed' : ''}`}
              />
            )}
            <h3>Result</h3>
          </div>
          <div className="debug-section-header-right" onClick={e => e.stopPropagation()}>
            {wasmLoading && <span className="wasm-status loading">Loading</span>}
            {wasmReady && (
              <Tooltip label={resultCopied ? 'Copied' : 'Copy result'} side="left">
                <button
                  type="button"
                  className={`debug-header-action ${resultCopied ? 'copied' : ''}`}
                  onClick={handleCopyResult}
                  disabled={resultError !== null || result === undefined}
                >
                  {resultCopied ? <Check size={13} /> : <Copy size={13} />}
                </button>
              </Tooltip>
            )}
          </div>
        </button>
        {isExpanded('result') && (
          <div className="debug-section-content">
            {resultError ? (
              <div className="debug-result error">
                <ErrorDisplay error={resultError} />
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
