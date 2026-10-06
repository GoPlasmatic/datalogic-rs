import { memo, useEffect, useMemo, useRef, useState } from 'react';
import { createPortal } from 'react-dom';
import { X } from 'lucide-react';
import { useDebuggerContext } from '../context';
import { formatResultValue } from '../utils/formatting';
import { getValueColorClass } from '../utils/type-helpers';
import { formatTraceFailure, traceFailureType } from '../utils/trace/trace-failure';
import { stepNodeId } from '../context/debugger/node-store';
import type { NodeSummary } from '../context/debugger/types';
import type { ExecutionStep } from '../types/trace';

interface StepTimelineProps {
  /** Element the panel is anchored to; the panel mounts in its `.logic-editor-body` (or `.logic-editor`) ancestor */
  anchor: HTMLElement | null;
  onClose: () => void;
}

/**
 * Step list for the trace: one row per execution step (index, node label and
 * expression, iteration, result or error). Click a row to jump to that step.
 * Mounted into the editor body so it overlays the canvas, although the
 * transport that opens it lives in the toolbar.
 */
export function StepTimeline({ anchor, onClose }: StepTimelineProps) {
  const { state, traceNodeMap, nodeSummaries, goToStep, traceError } = useDebuggerContext();
  const { steps, currentStepIndex } = state;
  const [errorsOnly, setErrorsOnly] = useState(false);
  const listRef = useRef<HTMLDivElement>(null);

  // The transport lives in the toolbar, a sibling of the body: resolve the
  // body through the editor root.
  const host = useMemo(() => {
    if (!anchor) return null;
    const root = anchor.closest<HTMLElement>('.logic-editor');
    return (
      anchor.closest<HTMLElement>('.logic-editor-body') ??
      root?.querySelector<HTMLElement>('.logic-editor-body') ??
      root
    );
  }, [anchor]);

  const errorCount = useMemo(() => steps.filter((s) => s.error).length, [steps]);

  const rows = useMemo(
    () =>
      steps
        .map((step, index) => ({ step, index }))
        .filter(({ step }) => !errorsOnly || !!step.error),
    [steps, errorsOnly]
  );

  // Keep the current step in view while stepping / playing
  useEffect(() => {
    if (currentStepIndex < 0 || !listRef.current) return;
    const row = listRef.current.querySelector<HTMLElement>(`[data-step-index="${currentStepIndex}"]`);
    row?.scrollIntoView({ block: 'nearest' });
  }, [currentStepIndex]);

  if (!host) return null;

  return createPortal(
    <div className="dl-debugger-timeline" role="region" aria-label="Execution steps">
      <div className="dl-debugger-timeline-header">
        <span className="dl-debugger-timeline-title">Steps</span>
        <span className="dl-debugger-timeline-count">{steps.length}</span>
        <span className="dl-debugger-timeline-spacer" />
        {errorCount > 0 && (
          <label className="dl-debugger-timeline-filter">
            <input
              type="checkbox"
              checked={errorsOnly}
              onChange={(e) => setErrorsOnly(e.target.checked)}
            />
            <span>Errors only ({errorCount})</span>
          </label>
        )}
        <button
          type="button"
          className="dl-debugger-timeline-close"
          onClick={onClose}
          aria-label="Close step list"
        >
          <X size={14} />
        </button>
      </div>

      {traceError && (
        <div className="dl-debugger-timeline-failure">
          <span className="dl-debugger-timeline-failure-kind">
            {traceFailureType(traceError) ?? 'Error'}
          </span>
          <span className="dl-debugger-timeline-failure-message">{formatTraceFailure(traceError)}</span>
        </div>
      )}

      <div className="dl-debugger-timeline-list" ref={listRef}>
        {rows.map(({ step, index }) => {
          const visualId = stepNodeId(step, traceNodeMap);
          return (
            <TimelineRow
              key={step.step_id ?? index}
              step={step}
              index={index}
              visualId={visualId}
              summary={nodeSummaries.get(visualId)}
              isCurrent={index === currentStepIndex}
              isDone={currentStepIndex >= 0 && index < currentStepIndex}
              onSelect={goToStep}
            />
          );
        })}
        {rows.length === 0 && (
          <div className="dl-debugger-timeline-empty">No steps to show</div>
        )}
      </div>
    </div>,
    host
  );
}

interface TimelineRowProps {
  step: ExecutionStep;
  index: number;
  visualId: string;
  summary: NodeSummary | undefined;
  isCurrent: boolean;
  isDone: boolean;
  onSelect: (index: number) => void;
}

/**
 * One step. Memoized: a step changes the props of at most two rows (the old
 * and the new current), so the rest are skipped instead of re-rendering
 * the whole list on every step of playback.
 */
const TimelineRow = memo(function TimelineRow({
  step,
  index,
  visualId,
  summary,
  isCurrent,
  isDone,
  onSelect,
}: TimelineRowProps) {
  const hasError = !!step.error;
  const hasIteration = step.iteration_index !== undefined && step.iteration_total !== undefined;
  const className = [
    'dl-debugger-timeline-row',
    isCurrent && 'is-current',
    isDone && 'is-done',
    hasError && 'is-error',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <button
      type="button"
      className={className}
      data-step-index={index}
      onClick={() => onSelect(index)}
      aria-current={isCurrent ? 'step' : undefined}
      title={summary?.detail || undefined}
    >
      <span className="dl-debugger-timeline-index">{index + 1}</span>
      <span className="dl-debugger-timeline-node">
        <span className="dl-debugger-timeline-label">{summary?.label ?? visualId}</span>
        {summary?.detail && (
          <span className="dl-debugger-timeline-detail">{summary.detail}</span>
        )}
      </span>
      {hasIteration && (
        <span className="dl-debugger-timeline-iter">
          {(step.iteration_index ?? 0) + 1}/{step.iteration_total}
        </span>
      )}
      {hasError ? (
        <span className="dl-debugger-timeline-result is-error" title={step.error ?? undefined}>
          error
        </span>
      ) : (
        <span className={`dl-debugger-timeline-result ${getValueColorClass(step.result)}`}>
          {formatResultValue(step.result)}
        </span>
      )}
    </button>
  );
});
