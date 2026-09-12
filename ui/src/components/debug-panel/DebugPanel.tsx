import { useState, useCallback } from 'react';
import { ChevronDown, Copy, Check, Gauge, Settings2 } from 'lucide-react';
import type { JsonLogicValue } from '../logic-editor/types';
import { JsonEditor, JsonDisplay } from './JsonHighlighter';
import { ErrorDisplay, type DebugError } from './ErrorDisplay';
import { errorToJson } from './error-utils';
import { Tooltip } from '../Tooltip';
import './DebugPanel.css';

export type { DebugError } from './ErrorDisplay';

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
  /**
   * Operations the engine charged for the last evaluation, or `null` when
   * nothing has run yet (or the WASM build predates metering). Shown next
   * to the result so the cost of a rule is visible while editing it, not
   * only when it trips the budget.
   */
  resultOps?: number | null;
  resultError: DebugError;
  wasmReady: boolean;
  wasmLoading: boolean;
  accordion?: boolean;
  /**
   * Summary of the active non-default engine settings (see
   * `summarizeEvaluationConfig`). Shown as a badge in the Result header so
   * a surprising result or error can be traced back to the configuration.
   */
  configSummary?: string | null;
  /** Opens the engine settings editor (badge becomes a button when provided). */
  onOpenEngineSettings?: () => void;
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
  resultOps = null,
  resultError,
  wasmReady,
  wasmLoading,
  accordion = false,
  configSummary,
  onOpenEngineSettings,
}: DebugPanelProps) {
  const [expandedSection, setExpandedSection] = useState<string>('logic');
  const [resultCopied, setResultCopied] = useState(false);

  const toggleSection = useCallback((section: string) => {
    setExpandedSection(prev => prev === section ? '' : section);
  }, []);

  const hasResultError = resultError !== null;
  const canCopy = hasResultError || result !== undefined;

  // Copies the result JSON, or the full structured error JSON when the
  // evaluation failed (type, message, operator, node_ids, thrown, ...).
  const handleCopyResult = useCallback(async () => {
    let text: string;
    if (resultError !== null) {
      text = errorToJson(resultError);
    } else if (result !== undefined) {
      text = JSON.stringify(result, null, 2);
    } else {
      return;
    }
    try {
      await navigator.clipboard.writeText(text);
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

  const opsBadge = resultOps === null || resultError !== null ? null : (
    <Tooltip
      label={
        `${resultOps.toLocaleString()} operations charged. One per node the engine dispatched, ` +
        `one per item an iterator walked, plus what operators charge per element. ` +
        `Literals and constant-folded subtrees cost nothing.`
      }
      side="left"
    >
      <span className="engine-ops-badge">
        <Gauge size={11} />
        <span className="engine-ops-badge-text">{resultOps.toLocaleString()} ops</span>
      </span>
    </Tooltip>
  );

  const configBadge = configSummary ? (
    <Tooltip label={`Engine settings: ${configSummary}`} side="left">
      {onOpenEngineSettings ? (
        <button
          type="button"
          className="engine-config-badge engine-config-badge--button"
          onClick={onOpenEngineSettings}
        >
          <Settings2 size={11} />
          <span className="engine-config-badge-text">{configSummary}</span>
        </button>
      ) : (
        <span className="engine-config-badge">
          <Settings2 size={11} />
          <span className="engine-config-badge-text">{configSummary}</span>
        </span>
      )}
    </Tooltip>
  ) : null;

  return (
    <div className="debug-panel">
      {/* Logic Input Section */}
      <div className={`debug-section logic-section ${sectionClass('logic')}`}>
        <div className="debug-section-header">
          {accordion ? (
            <button
              type="button"
              className="debug-section-header-left debug-section-toggle"
              onClick={() => toggleSection('logic')}
              aria-expanded={isExpanded('logic')}
            >
              <ChevronDown
                size={14}
                className={`debug-section-chevron ${!isExpanded('logic') ? 'collapsed' : ''}`}
              />
              <h3>Logic</h3>
            </button>
          ) : (
            <div className="debug-section-header-left">
              <h3>Logic</h3>
            </div>
          )}
          <div className="debug-section-header-right">
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
        </div>
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
                <ErrorDisplay error={logicError} compact />
              </div>
            )}
          </div>
        )}
      </div>

      {/* Data Input Section */}
      <div className={`debug-section data-section ${sectionClass('data')}`}>
        <div className="debug-section-header">
          {accordion ? (
            <button
              type="button"
              className="debug-section-header-left debug-section-toggle"
              onClick={() => toggleSection('data')}
              aria-expanded={isExpanded('data')}
            >
              <ChevronDown
                size={14}
                className={`debug-section-chevron ${!isExpanded('data') ? 'collapsed' : ''}`}
              />
              <h3>Data</h3>
            </button>
          ) : (
            <div className="debug-section-header-left">
              <h3>Data</h3>
            </div>
          )}
          <div className="debug-section-header-right">
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
        </div>
        {isExpanded('data') && (
          <div className="debug-section-content">
            <JsonEditor
              value={dataText}
              onChange={onDataChange}
              placeholder="Enter data (any JSON: object, array or scalar)..."
              hasError={!!dataError}
            />
            {dataError && (
              <div className="debug-error">
                <ErrorDisplay error={dataError} compact />
              </div>
            )}
          </div>
        )}
      </div>

      {/* Result Section */}
      <div
        className={`debug-section result-section ${sectionClass('result')} ${
          resultError ? 'has-error' : result === true ? 'has-true' : result === false ? 'has-false' : ''
        }`}
      >
        <div className="debug-section-header">
          {accordion ? (
            <button
              type="button"
              className="debug-section-header-left debug-section-toggle"
              onClick={() => toggleSection('result')}
              aria-expanded={isExpanded('result')}
            >
              <ChevronDown
                size={14}
                className={`debug-section-chevron ${!isExpanded('result') ? 'collapsed' : ''}`}
              />
              <h3>Result</h3>
            </button>
          ) : (
            <div className="debug-section-header-left">
              <h3>Result</h3>
            </div>
          )}
          <div className="debug-section-header-right">
            {opsBadge}
            {configBadge}
            {wasmLoading && <span className="wasm-status loading">Loading</span>}
            {wasmReady && (
              <Tooltip
                label={resultCopied ? 'Copied' : hasResultError ? 'Copy error JSON' : 'Copy result'}
                side="left"
              >
                <button
                  type="button"
                  className={`debug-header-action ${resultCopied ? 'copied' : ''}`}
                  onClick={handleCopyResult}
                  disabled={!canCopy}
                  aria-label={hasResultError ? 'Copy error JSON' : 'Copy result'}
                >
                  {resultCopied ? <Check size={13} /> : <Copy size={13} />}
                </button>
              </Tooltip>
            )}
          </div>
        </div>
        {isExpanded('result') && (
          <div className="debug-section-content">
            {resultError ? (
              <div className="debug-result error">
                <ErrorDisplay error={resultError} />
                {configSummary && (
                  <div className="debug-result-config-note">
                    Evaluated with engine settings: {configSummary}
                  </div>
                )}
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
