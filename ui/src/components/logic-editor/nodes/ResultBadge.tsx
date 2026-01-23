import { memo, useState } from 'react';
import type { EvaluationResult } from '../hooks/useDebugEvaluation';
import { formatResultValue, isComplexValue } from '../utils/formatting';

interface ResultBadgeProps {
  result: EvaluationResult | undefined;
  compact?: boolean; // For header display
}

export const ResultBadge = memo(function ResultBadge({ result, compact = false }: ResultBadgeProps) {
  const [showPopover, setShowPopover] = useState(false);

  if (!result) {
    return null;
  }

  const { value, error, type } = result;

  if (error) {
    return (
      <div
        className={`result-badge error ${compact ? 'compact' : ''}`}
        onMouseEnter={() => setShowPopover(true)}
        onMouseLeave={() => setShowPopover(false)}
      >
        <span className="result-icon">!</span>
        {!compact && <span className="result-text">Error</span>}
        {showPopover && (
          <div className="result-popover error">
            <pre>{error}</pre>
          </div>
        )}
      </div>
    );
  }

  const isTruthy = type === 'boolean' ? value === true : Boolean(value);
  const displayValue = formatResultValue(value);
  const isComplex = isComplexValue(value);

  return (
    <div
      className={`result-badge ${type} ${isTruthy ? 'truthy' : 'falsy'} ${compact ? 'compact' : ''}`}
      onMouseEnter={() => isComplex && setShowPopover(true)}
      onMouseLeave={() => setShowPopover(false)}
    >
      <span className="result-text">{displayValue}</span>
      {showPopover && isComplex && (
        <div className="result-popover">
          <pre>{JSON.stringify(value, null, 2)}</pre>
        </div>
      )}
    </div>
  );
});
