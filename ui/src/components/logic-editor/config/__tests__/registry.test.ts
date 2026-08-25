/**
 * Registry completeness and consistency checks.
 *
 * The registry must cover exactly the engine's builtin operator set, which
 * includes the aliases `var` (of val), `?:` (of if) and `match` (of switch).
 * Each alias has its own registry entry so the picker, help panel and
 * arity validation work for either spelling; the alias entries say so in
 * their notes and point at the canonical operator via seeAlso.
 */

import { describe, expect, it } from 'vitest';
// The root specifier resolves to the vendored 5.3.0 build under both `tsc`
// (tsconfig `paths`) and vitest (alias to the nodejs target); the `/nodejs`
// subpath would type-check against the stale node_modules copy.
import * as wasm from '@goplasmatic/datalogic-wasm';
import { operators, getOperator, isOperator } from '../operators';
import { categories } from '../categories';
import {
  DOCS_BASE_URL,
  getDocsUrlForOperator,
  getOperatorDocsAnchor,
  mdBookAnchor,
} from '../docs';
import { formatArity } from '../arity';
import { literalPanelConfig, structurePanelConfig } from '../literalPanel';
import type { OperatorCategory } from '../operators.types';

const ALIASES: Record<string, string> = {
  var: 'val',
  '?:': 'if',
  match: 'switch',
};

describe('operator registry', () => {
  it('matches the engine builtin operator names exactly', () => {
    const engine = [...wasm.builtinOperatorNames()].sort();
    const ui = Object.keys(operators).sort();
    expect(ui).toEqual(engine);
  });

  it('keys agree with operator names', () => {
    for (const [key, op] of Object.entries(operators)) {
      expect(op.name).toBe(key);
    }
  });

  it('every alias has its own entry pointing at the canonical operator', () => {
    for (const [alias, canonical] of Object.entries(ALIASES)) {
      expect(isOperator(alias)).toBe(true);
      expect(isOperator(canonical)).toBe(true);
      expect(getOperator(alias)?.help.seeAlso).toContain(canonical);
    }
  });

  it('every operator category has metadata', () => {
    for (const op of Object.values(operators)) {
      expect(categories[op.category], `category for ${op.name}`).toBeDefined();
    }
  });

  it('every category is used by at least one operator', () => {
    const used = new Set(Object.values(operators).map((op) => op.category));
    for (const name of Object.keys(categories) as OperatorCategory[]) {
      expect(used.has(name), `unused category ${name}`).toBe(true);
    }
  });

  it('flagd operators are registered under the flagd category', () => {
    expect(getOperator('fractional')?.category).toBe('flagd');
    expect(getOperator('sem_ver')?.category).toBe('flagd');
  });

  it('seeAlso references resolve', () => {
    for (const op of Object.values(operators)) {
      for (const ref of op.help.seeAlso ?? []) {
        expect(isOperator(ref), `${op.name} seeAlso ${ref}`).toBe(true);
      }
    }
  });

  it('example error types are engine error names', () => {
    const known = new Set(['Thrown', 'InvalidArguments', 'InvalidOperator', 'ParseError']);
    for (const op of Object.values(operators)) {
      for (const ex of op.help.examples) {
        if (ex.error) expect(known.has(ex.error.type), `${op.name}: ${ex.title}`).toBe(true);
      }
    }
  });
});

describe('docs links', () => {
  it('normalises headings the way mdBook does', () => {
    expect(mdBookAnchor('starts_with')).toBe('starts_with');
    expect(mdBookAnchor('+ (Add)')).toBe('-add');
    expect(mdBookAnchor('- (Subtract)')).toBe('--subtract');
    expect(mdBookAnchor('!! (Double Not / Boolean Cast)')).toBe('-double-not--boolean-cast');
    expect(mdBookAnchor('switch / match')).toBe('switch--match');
    expect(mdBookAnchor('?? (Null Coalesce)')).toBe('-null-coalesce');
  });

  it('anchors symbolic operators through their doc headings', () => {
    expect(getOperatorDocsAnchor('+')).toBe('-add');
    expect(getOperatorDocsAnchor('>=')).toBe('-greater-than-or-equal');
    expect(getOperatorDocsAnchor('?:')).toBe('-ternary');
    expect(getOperatorDocsAnchor('match')).toBe('switch--match');
    expect(getOperatorDocsAnchor('keys')).toBe('keys');
    expect(getOperatorDocsAnchor('sem_ver')).toBe('sem_ver');
  });

  it('builds a page + anchor URL for every operator', () => {
    const pattern = new RegExp(`^${DOCS_BASE_URL.replace(/[.*+?^${}()|[\]\\/]/g, '\\$&')}[a-z-]+\\.html#[a-z0-9_-]+$`);
    for (const op of Object.values(operators)) {
      const url = getDocsUrlForOperator(op);
      expect(url, op.name).toMatch(pattern);
    }
    expect(getDocsUrlForOperator(operators.fractional)).toBe(`${DOCS_BASE_URL}flagd.html#fractional`);
    expect(getDocsUrlForOperator(operators.type)).toBe(`${DOCS_BASE_URL}control-flow.html#type`);
    expect(getDocsUrlForOperator(operators['!='])).toBe(`${DOCS_BASE_URL}comparison.html#-not-equals`);
  });
});

describe('formatArity', () => {
  it('honours explicit min/max for every arity type', () => {
    expect(formatArity(operators.throw.arity)).toBe('Args: 0-1');
    expect(formatArity({ type: 'unary', min: 0, max: 1 })).toBe('Args: 0-1');
    expect(formatArity({ type: 'unary' })).toBe('Args: 1');
    expect(formatArity({ type: 'binary', min: 2, max: 2 })).toBe('Args: 2');
    expect(formatArity({ type: 'range', min: 1, max: 4 })).toBe('Args: 1-4');
    expect(formatArity({ type: 'nary', min: 1 })).toBe('Args: 1+');
    expect(formatArity({ type: 'nary', min: 0 })).toBe('Args: 0+');
    expect(formatArity({ type: 'variadic' })).toBe('Args: 2+');
    expect(formatArity({ type: 'chainable', min: 2 })).toBe('Args: 2+ (chainable)');
    expect(formatArity({ type: 'special', min: 2 })).toBe('Args: 2+');
    expect(formatArity({ type: 'special' })).toBe('Args: special');
    expect(formatArity({ type: 'nullary', min: 0, max: 0 })).toBe('Args: 0');
  });

  it('reflects the engine arities fixed in this pass', () => {
    expect(formatArity(operators['??'].arity)).toBe('Args: 1+');
    expect(formatArity(operators.sort.arity)).toBe('Args: 1-3');
    expect(formatArity(operators.slice.arity)).toBe('Args: 1-4');
    expect(formatArity(operators.reduce.arity)).toBe('Args: 2-3');
    expect(formatArity(operators.parse_date.arity)).toBe('Args: 2-3');
    expect(formatArity(operators.format_date.arity)).toBe('Args: 2-3');
  });
});

describe('literal panel', () => {
  function typeOptions(config: typeof literalPanelConfig): unknown[] {
    const field = config.sections[0].fields.find((f) => f.id === 'valueType');
    return (field?.options ?? []).map((o) => o.value);
  }

  it('offers only value types a literal node can hold', () => {
    expect(typeOptions(literalPanelConfig)).toEqual(['string', 'number', 'boolean', 'null', 'array']);
    expect(literalPanelConfig.sections.some((s) => s.id === 'objectValue')).toBe(false);
  });

  it('keeps the object option for structure nodes', () => {
    expect(typeOptions(structurePanelConfig)).toContain('object');
    expect(structurePanelConfig.sections.some((s) => s.id === 'objectValue')).toBe(true);
  });
});
