// The cross-binding scenarios in bindings/scenarios/api.json, through the
// Node API. Every binding runs the same file (see bindings/BINDINGS.md).

import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { Engine } from '../index.js';

const scenarios = JSON.parse(
  readFileSync(new URL('../../scenarios/api.json', import.meta.url)),
).filter((c) => typeof c === 'object');

function engineFor(c) {
  const o = c.engine ?? {};
  return new Engine({
    templating: o.templating ?? false,
    templateKeyEscape: o.template_key_escape ?? undefined,
    config: o.config ?? undefined,
    families: o.families ?? undefined,
  });
}

function run(c) {
  try {
    const engine = engineFor(c);
    switch (c.call) {
      case 'check':
        return engine.check(c.rule, c.mode).map((d) => [d.code, d.pointer]);
      case 'truthy':
        return engine.truthy(c.value);
      case 'facts':
        return engine.compile(c.rule).facts();
      case 'metered':
        return JSON.parse(engine.compile(c.rule).evaluateMetered(c.data, c.budget).result);
      case 'trace': {
        const run = JSON.parse(engine.evaluateWithTrace(JSON.stringify(c.rule), JSON.stringify(c.data)));
        return { result: run.result, pointers: [...new Set(Object.values(run.pointers ?? {}))].sort() };
      }
      default: {
        const compile = {
          evaluate: 'compile',
          compile_template: 'compileTemplate',
          compile_strict: 'compileStrict',
          compile_checked: 'compileChecked',
        }[c.call];
        return engine[compile](c.rule).evaluate(c.data);
      }
    }
  } catch (e) {
    return ['error', e.errorType];
  }
}

for (const c of scenarios) {
  test(`${c.call}: ${c.description}`, () => {
    const got = run(c);
    if ('error' in c) {
      assert.deepEqual(got, ['error', c.error]);
    } else if ('diagnostics' in c) {
      assert.deepEqual(got, c.diagnostics);
    } else if ('trace' in c) {
      assert.deepEqual(got, c.trace);
    } else if ('facts' in c) {
      for (const [k, v] of Object.entries(c.facts)) assert.deepEqual(got[k], v, k);
    } else {
      assert.deepEqual(got, c.result);
    }
  });
}
