import type { DebugError } from './ErrorDisplay';

/** Pretty JSON for the "copy error" action (strings are wrapped for uniformity). */
export function errorToJson(error: Exclude<DebugError, null>): string {
  return JSON.stringify(typeof error === 'string' ? { message: error } : error, null, 2);
}
