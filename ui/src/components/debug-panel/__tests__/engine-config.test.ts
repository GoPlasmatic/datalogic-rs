/**
 * Engine config edits must keep the stored config minimal: it is what the
 * toolbar summary and the shared URL are built from, so an override that
 * merely restates the active preset is noise.
 */

import { describe, expect, it } from 'vitest';
import { withOverride, withCoercion, resolveEvaluationConfig } from '../engine-config';
import { customOperatorNamesKey } from '../../logic-editor/hooks';

describe('withOverride', () => {
  it('keeps an override that differs from the preset', () => {
    expect(withOverride({}, 'division_by_zero', 'throw_error')).toEqual({
      division_by_zero: 'throw_error',
    });
  });

  it('drops an override that restates the preset', () => {
    expect(withOverride({}, 'division_by_zero', 'return_saturated')).toEqual({});
  });

  it('re-prunes every knob when the preset changes', () => {
    // 'throw_error' is an override under `default` but the base under `strict`
    const withDiv = withOverride({}, 'division_by_zero', 'throw_error');
    expect(withDiv.division_by_zero).toBe('throw_error');
    expect(withOverride(withDiv, 'preset', 'strict')).toEqual({ preset: 'strict' });
  });

  it('keeps an override that is still meaningful under the new preset', () => {
    const withNan = withOverride({}, 'arithmetic_nan_handling', 'ignore_value');
    expect(withOverride(withNan, 'preset', 'strict')).toEqual({
      preset: 'strict',
      arithmetic_nan_handling: 'ignore_value',
    });
  });

  it('drops coercion flags the new preset already implies', () => {
    const flagged = withCoercion({}, 'reject_non_numeric', true);
    expect(flagged.numeric_coercion).toEqual({ reject_non_numeric: true });
    expect(withOverride(flagged, 'preset', 'strict')).toEqual({ preset: 'strict' });
  });

  it('leaves the effective values unchanged by the pruning', () => {
    const pruned = withOverride(withOverride({}, 'division_by_zero', 'throw_error'), 'preset', 'strict');
    expect(resolveEvaluationConfig(pruned).division_by_zero).toBe('throw_error');
  });
});

describe('customOperatorNamesKey', () => {
  it('is stable across object identities with the same names', () => {
    const a = customOperatorNamesKey({ double: () => 0, half: () => 0 });
    const b = customOperatorNamesKey({ half: () => 1, double: () => 1 });
    expect(a).toBe(b);
  });

  it('changes when a name is added or removed', () => {
    const one = customOperatorNamesKey({ double: () => 0 });
    expect(customOperatorNamesKey({ double: () => 0, half: () => 0 })).not.toBe(one);
    expect(customOperatorNamesKey(undefined)).not.toBe(one);
  });
});
