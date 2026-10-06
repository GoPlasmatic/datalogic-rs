/**
 * Test helper: evaluate through a shared `Engine`, the binding's supported
 * API. The free `evaluate` and `evaluateWithTrace` functions the tests
 * used before are deprecated and go away in 6.0. Test-only: nothing in the
 * app or the library imports this.
 */
import { Engine } from '@goplasmatic/datalogic-wasm';

const engines = new Map<boolean, Engine>();

function engineFor(templating: boolean): Engine {
  let engine = engines.get(templating);
  if (!engine) {
    engine = new Engine({ templating });
    engines.set(templating, engine);
  }
  return engine;
}

/** `Engine#evalStr` on a cached engine with the given templating mode. */
export function evalStr(logic: string, data: string, templating = false): string {
  return engineFor(templating).evalStr(logic, data);
}

/** `Engine#evaluateWithTrace` on a cached engine with the given templating mode. */
export function evaluateWithTraceStr(logic: string, data: string, templating = false): string {
  return engineFor(templating).evaluateWithTrace(logic, data);
}
