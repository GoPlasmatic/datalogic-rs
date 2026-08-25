import { memo } from 'react';
import type { ExecutionStep } from '../types/trace';
import { formatResultValue, isComplexValue } from '../utils/formatting';
import { getValueColorClass } from '../utils/type-helpers';
import { formatTraceFailure, traceFailureType, type TraceFailure } from '../utils/trace/trace-failure';
import { tokenizeValue, type JsonToken, type JsonTokenType } from '../../../utils/json-tokenizer';

interface DebugInfoBubbleProps {
  /** The execution step to show (context, result / error, iteration) */
  step?: ExecutionStep | null;
  /**
   * Trace-level failure to show instead of a step, used for the failing node
   * at rest (before stepping) when the engine reported an error breadcrumb.
   */
  failure?: TraceFailure | null;
  position?: 'top' | 'right' | 'bottom';
}

// Map token type to CSS class
function getTokenClass(type: JsonTokenType): string {
  if (type === 'whitespace' || type === 'unknown') return '';
  // Map boolean to handle true/false distinction
  if (type === 'boolean') return 'json-syntax-boolean-true';
  return `json-syntax-${type}`;
}

// Render syntax-highlighted JSON
function renderHighlightedJson(value: unknown): React.ReactNode {
  const tokens = tokenizeValue(value);
  return tokens.map((token: JsonToken, index: number) => {
    const className = getTokenClass(token.type);
    if (className) {
      return <span key={index} className={className}>{token.value}</span>;
    }
    return <span key={index}>{token.value}</span>;
  });
}

export const DebugInfoBubble = memo(function DebugInfoBubble({
  step,
  failure,
  position = 'top',
}: DebugInfoBubbleProps) {
  // Failure-only bubble: the engine's error for the failed node, no step data
  if (!step) {
    if (!failure) return null;
    const kind = traceFailureType(failure);
    return (
      <div className={`debug-info-bubble debug-info-${position} error`}>
        <div className="debug-info-section">
          <span className="debug-info-label">
            {'⚠ '}{kind ? `Error (${kind}):` : 'Error:'}
          </span>
          <pre className="debug-info-value debug-info-error">{formatTraceFailure(failure)}</pre>
        </div>
      </div>
    );
  }

  const hasError = !!step.error;
  const hasIteration = step.iteration_index !== undefined && step.iteration_total !== undefined;
  const result = step.result;
  const displayResult = formatResultValue(result);
  const isComplex = isComplexValue(result);
  const valueColorClass = getValueColorClass(result);

  return (
    <div className={`debug-info-bubble debug-info-${position} ${hasError ? 'error' : ''}`}>
      {/* Iteration info */}
      {hasIteration && (
        <div className="debug-info-iteration">
          Iteration {(step.iteration_index ?? 0) + 1} of {step.iteration_total}
        </div>
      )}

      {/* Context section */}
      <div className="debug-info-section">
        <span className="debug-info-label">Context:</span>
        <pre className="debug-info-value">
          {renderHighlightedJson(step.context)}
        </pre>
      </div>

      {/* Result section */}
      <div className="debug-info-section">
        <span className="debug-info-label">
          {hasError ? '⚠ Error:' : 'Result:'}
        </span>
        {hasError ? (
          <pre className="debug-info-value debug-info-error">{step.error}</pre>
        ) : (
          <div className="debug-info-result">
            {isComplex ? (
              <pre className="debug-info-value">
                {renderHighlightedJson(result)}
              </pre>
            ) : (
              <span className={`debug-info-simple-value ${valueColorClass}`}>
                {displayResult}
              </span>
            )}
          </div>
        )}
      </div>
    </div>
  );
});
