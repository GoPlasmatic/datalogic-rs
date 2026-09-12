import type { StructuredError } from '../logic-editor/types';
import { JsonDisplay } from './JsonHighlighter';

/** Error shape accepted by the debug panel: a plain string for parse-level
 * problems, or a `StructuredError` for runtime errors out of the engine. */
export type DebugError = StructuredError | string | null;

interface DetailChip {
  label: string;
  value: string;
}

function collectDetailChips(error: StructuredError): DetailChip[] {
  const chips: DetailChip[] = [];
  if (error.variable !== undefined) chips.push({ label: 'variable', value: String(error.variable) });
  if (error.level !== undefined) chips.push({ label: 'level', value: String(error.level) });
  if (error.index !== undefined) chips.push({ label: 'index', value: String(error.index) });
  if (error.length !== undefined) chips.push({ label: 'length', value: String(error.length) });
  if (error.budget !== undefined) chips.push({ label: 'budget', value: error.budget.toLocaleString() });
  if (error.spent !== undefined) chips.push({ label: 'spent', value: error.spent.toLocaleString() });
  if (error.stage !== undefined) chips.push({ label: 'stage', value: String(error.stage) });
  return chips;
}

interface ErrorDisplayProps {
  error: Exclude<DebugError, null>;
  /**
   * Compact mode keeps the single-line header (type pill, message, operator
   * chip) and drops the breadcrumb, detail chips and thrown payload. Used by
   * inline error strips under the JSON editors and by the embed widgets.
   */
  compact?: boolean;
}

/**
 * Renders a debug-panel error. Strings render as-is; structured engine
 * errors get a type pill, the message, the failing operator, the
 * `node_ids` breadcrumb (compile-time node ids, root to leaf), any variant
 * extras (variable, index/length, stage, budget/spent) and the thrown
 * payload as JSON.
 */
export function ErrorDisplay({ error, compact = false }: ErrorDisplayProps) {
  if (typeof error === 'string') {
    return (
      <>
        <span className="error-icon">!</span>
        <span className="error-message">{error}</span>
      </>
    );
  }

  const chips = collectDetailChips(error);
  const hasBreadcrumb = !!error.node_ids && error.node_ids.length > 0;
  const hasThrown = error.thrown !== undefined;
  const showDetails = !compact && (hasBreadcrumb || chips.length > 0 || hasThrown);

  return (
    <div className="error-display" data-kind={error.type}>
      <div className="error-display-head">
        <span className="error-icon">!</span>
        <span className="error-type-pill" data-kind={error.type}>{error.type}</span>
        <span className="error-message">{error.message}</span>
        {error.operator && (
          <span className="error-operator-chip" title="Failing operator">op: {error.operator}</span>
        )}
      </div>
      {showDetails && (
        <div className="error-display-details">
          {hasBreadcrumb && (
            <div className="error-detail-row" title="Compile-time node ids from the root to the failing node">
              <span className="error-detail-label">path</span>
              <span className="error-breadcrumb">
                {error.node_ids!.map((id, i) => (
                  <span key={`${id}-${i}`} className="error-breadcrumb-step">
                    {i > 0 && <span className="error-breadcrumb-sep" aria-hidden="true">&rsaquo;</span>}
                    <span className="error-breadcrumb-id">#{id}</span>
                  </span>
                ))}
              </span>
            </div>
          )}
          {chips.length > 0 && (
            <div className="error-detail-row">
              {chips.map((chip) => (
                <span key={chip.label} className="error-detail-chip">
                  <span className="error-detail-label">{chip.label}</span>
                  <span className="error-detail-value">{chip.value}</span>
                </span>
              ))}
            </div>
          )}
          {hasThrown && (
            <div className="error-detail-row error-thrown">
              <span className="error-detail-label">thrown</span>
              <JsonDisplay value={error.thrown} className="error-thrown-json" />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
