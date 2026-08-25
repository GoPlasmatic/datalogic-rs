/**
 * Every help example in the operator registry is run through the real
 * engine (the vendored WASM build) and its documented result / error must
 * match exactly. This keeps the help panel honest: an example that drifts
 * from engine behaviour fails here instead of misleading a user.
 */

import { describe, expect, it } from 'vitest';
// The root specifier resolves to the vendored 5.3.0 build under both `tsc`
// (tsconfig `paths`) and vitest (alias to the nodejs target); the `/nodejs`
// subpath would type-check against the stale node_modules copy.
import * as wasm from '@goplasmatic/datalogic-wasm';
import { operators } from '../operators';
import type { OperatorExample } from '../operators.types';

interface EngineError {
  type?: string;
  name?: string;
  message?: string;
}

function run(example: OperatorExample): { value?: unknown; error?: EngineError } {
  const logic = JSON.stringify(example.rule);
  const data = JSON.stringify(example.data ?? null);
  try {
    return { value: JSON.parse(wasm.evaluate(logic, data, example.templating ?? false)) };
  } catch (e) {
    return { error: e as EngineError };
  }
}

describe('operator help examples match the engine', () => {
  for (const op of Object.values(operators)) {
    describe(op.name, () => {
      it('has at least one example', () => {
        expect(op.help.examples.length).toBeGreaterThan(0);
      });

      for (const example of op.help.examples) {
        it(example.title, () => {
          const outcome = run(example);

          if (example.error) {
            expect(outcome.value, 'expected an error but got a value').toBeUndefined();
            expect(outcome.error?.type).toBe(example.error.type);
            return;
          }

          if (outcome.error) {
            throw new Error(
              `unexpected engine error ${outcome.error.type}: ${outcome.error.message}`,
            );
          }

          if (example.result !== undefined) {
            expect(outcome.value).toEqual(example.result);
          } else {
            // Result-less examples are only allowed for non-deterministic
            // rules (now) and must explain themselves.
            expect(example.note, 'example without result needs a note').toBeTruthy();
          }
        });
      }
    });
  }
});
