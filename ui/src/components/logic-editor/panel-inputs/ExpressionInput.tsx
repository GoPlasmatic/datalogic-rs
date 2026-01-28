import { memo } from 'react';
import { GitBranch } from 'lucide-react';

interface ExpressionInputProps {
  id: string;
  value: unknown;
  onChange: (value: unknown) => void;
  disabled?: boolean;
  required?: boolean;
  placeholder?: string;
}

/**
 * Input for JSONLogic expressions.
 * Currently displays a summary of the expression with an indicator that it branches to another node.
 * Full expression editing will be handled by the canvas connection system.
 */
export const ExpressionInput = memo(function ExpressionInput({
  id,
  value,
  disabled = false,
  placeholder = 'Expression',
}: ExpressionInputProps) {
  const getExpressionSummary = (expr: unknown): string => {
    if (expr === null) return 'null';
    if (expr === undefined) return placeholder;
    if (typeof expr === 'string') return `"${expr}"`;
    if (typeof expr === 'number' || typeof expr === 'boolean') return String(expr);
    if (Array.isArray(expr)) return `[${expr.length} items]`;
    if (typeof expr === 'object') {
      const keys = Object.keys(expr);
      if (keys.length === 1) {
        const op = keys[0];
        return `{${op}: ...}`;
      }
      return `{${keys.length} keys}`;
    }
    return String(expr);
  };

  const hasExpression = value !== undefined && value !== null;

  return (
    <div id={id} className={`panel-input-expression ${disabled ? 'disabled' : ''}`}>
      <div className="panel-input-expression-content">
        {hasExpression ? (
          <>
            <GitBranch size={14} className="panel-input-expression-icon" />
            <span className="panel-input-expression-summary">
              {getExpressionSummary(value)}
            </span>
          </>
        ) : (
          <span className="panel-input-expression-empty">{placeholder}</span>
        )}
      </div>
      <div className="panel-input-expression-hint">
        Connect on canvas
      </div>
    </div>
  );
});
