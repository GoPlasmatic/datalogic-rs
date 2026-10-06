import type { DebugError } from './ErrorDisplay';
import { DataLogicEvaluationError } from '../logic-editor/hooks/useWasmEvaluator';

/** Pretty JSON for the "copy error" action (strings are wrapped for uniformity). */
export function errorToJson(error: Exclude<DebugError, null>): string {
  return JSON.stringify(typeof error === 'string' ? { message: error } : error, null, 2);
}

/** What an evaluation threw, as the result pane shows it. */
export function toDebugError(err: unknown): DebugError {
  if (err instanceof DataLogicEvaluationError) return err.structured;
  if (err instanceof Error) return err.message;
  return typeof err === 'string' ? err : 'Evaluation failed';
}
