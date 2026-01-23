import { memo } from 'react';
import type { ExecutionStep } from '../types/trace';
import { formatResultValue, isComplexValue, getValueColorClass, tokenizeJson } from '../utils/formatting';
import type { JsonToken, JsonTokenType } from '../utils/formatting';

interface DebugInfoBubbleProps {
  step: ExecutionStep;
  position?: 'top' | 'right' | 'bottom';
}

// Map token type to CSS class
function getTokenClass(type: JsonTokenType | null): string {
  if (!type) return '';
  return `json-syntax-${type}`;
}

// Render syntax-highlighted JSON
function renderHighlightedJson(value: unknown): React.ReactNode {
  const tokens = tokenizeJson(value);
  return tokens.map((token: JsonToken, index: number) => {
    const className = getTokenClass(token.type);
    if (className) {
      return <span key={index} className={className}>{token.content}</span>;
    }
    return <span key={index}>{token.content}</span>;
  });
}

export const DebugInfoBubble = memo(function DebugInfoBubble({
  step,
  position = 'top',
}: DebugInfoBubbleProps) {
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
          {hasError ? 'Error:' : 'Result:'}
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
