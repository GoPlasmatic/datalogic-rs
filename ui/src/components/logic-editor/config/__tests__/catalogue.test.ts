/**
 * Registry checks against the engine's operator catalogue.
 *
 * `docs/src/operators/operators.json` is generated from the engine's
 * operator table (see `crates/datalogic-rs/tests/operators_json_test.rs`)
 * and lists every operator in every feature family, whichever features a
 * build compiled in. Unlike `registry.test.ts`, these checks need no
 * vendored WASM build.
 */

import { describe, expect, it } from 'vitest';
import { operators } from '../operators';
import type { AritySpec, OperatorCategory } from '../operators.types';
import catalogueJson from '../../../../../../docs/src/operators/operators.json?raw';

interface CatalogueRow {
  name: string;
  aliases: string[];
  family: string;
  feature: string | null;
  min_args: number;
  max_args: number | null;
}

const catalogue: CatalogueRow[] = JSON.parse(catalogueJson);

/** The argument counts an arity spec admits, `max` null when unbounded. */
function admits(arity: AritySpec): { min: number; max: number | null } {
  const nominal: Partial<Record<AritySpec['type'], number>> = {
    nullary: 0,
    unary: 1,
    binary: 2,
    ternary: 3,
  };
  const fixed = nominal[arity.type];
  return {
    min: arity.min ?? fixed ?? 0,
    max: arity.max ?? fixed ?? null,
  };
}

describe('operator registry vs the engine catalogue', () => {
  it('covers every engine operator and alias, and nothing else', () => {
    const engine = catalogue.flatMap((row) => [row.name, ...row.aliases]).sort();
    expect(Object.keys(operators).sort()).toEqual(engine);
  });

  it('never offers an argument count the engine does not read', () => {
    // Only typed operators declare their counts in the table; the others
    // check their own arguments and report 0..unbounded.
    for (const row of catalogue.filter((r) => r.max_args !== null)) {
      for (const name of [row.name, ...row.aliases]) {
        const ui = admits(operators[name].arity);
        expect(ui.min, `${name} min`).toBeGreaterThanOrEqual(row.min_args);
        expect(ui.max, `${name} max`).not.toBeNull();
        expect(ui.max!, `${name} max`).toBeLessThanOrEqual(row.max_args!);
      }
    }
  });

  it('files each single-category family under its category', () => {
    const categoryOf: Record<string, OperatorCategory> = {
      DateTime: 'datetime',
      ExtObject: 'object',
      Tensor: 'tensor',
      Flagd: 'flagd',
    };
    for (const row of catalogue) {
      const category = categoryOf[row.family];
      if (category) expect(operators[row.name].category, row.name).toBe(category);
    }
  });
});
