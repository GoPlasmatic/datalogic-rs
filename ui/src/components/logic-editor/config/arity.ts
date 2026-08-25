/**
 * Arity presentation
 *
 * Renders an AritySpec as the "Args: ..." badge text. Explicit `min` / `max`
 * always win over the nominal arity type, so `throw` (unary, 0-1) reads
 * "Args: 0-1" and `??` (nary, 1+) reads "Args: 1+".
 */

import type { AritySpec } from './operators.types';

const NOMINAL_COUNT: Partial<Record<AritySpec['type'], number>> = {
  nullary: 0,
  unary: 1,
  binary: 2,
  ternary: 3,
};

const NOMINAL_MIN: Partial<Record<AritySpec['type'], number>> = {
  nary: 1,
  variadic: 2,
  chainable: 2,
};

export function formatArity(arity: AritySpec): string {
  const { type, min, max } = arity;
  const suffix = type === 'chainable' ? ' (chainable)' : '';

  if (min !== undefined && max !== undefined) {
    return min === max ? `Args: ${min}${suffix}` : `Args: ${min}-${max}${suffix}`;
  }
  if (max !== undefined) {
    return `Args: ${min ?? 0}-${max}${suffix}`;
  }

  const fixed = NOMINAL_COUNT[type];
  if (fixed !== undefined) {
    return `Args: ${min ?? fixed}`;
  }

  const nominalMin = NOMINAL_MIN[type];
  if (nominalMin !== undefined) {
    return `Args: ${min ?? nominalMin}+${suffix}`;
  }

  if (min !== undefined) {
    return `Args: ${min}+`;
  }
  return type === 'special' ? 'Args: special' : 'Args: varies';
}
