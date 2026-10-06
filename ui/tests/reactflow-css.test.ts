/**
 * The bundled React Flow base CSS must match the installed @xyflow/react,
 * scoped to the editor. Regenerate with `node scripts/reactflow-css.mjs`.
 */
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

const uiRoot = resolve(import.meta.dirname, '..');
const css = readFileSync(resolve(uiRoot, 'src/components/logic-editor/styles/reactflow-base.css'), 'utf8');

describe('reactflow-base.css', () => {
  it('is generated from the installed @xyflow/react', () => {
    expect(() =>
      execFileSync(process.execPath, ['scripts/reactflow-css.mjs', '--check'], { cwd: uiRoot, stdio: 'pipe' }),
    ).not.toThrow();
  });

  it('scopes every rule to the editor', () => {
    const body = css.replace(/\/\*[\s\S]*?\*\//g, '').replace(/@keyframes[^{]*\{[^{}]*(\{[^{}]*\}[^{}]*)*\}/g, '');
    const selectors = [...body.matchAll(/([^{}]+)\{[^{}]*\}/g)].flatMap((m) => m[1].split(',').map((s) => s.trim()));
    expect(selectors.length).toBeGreaterThan(50);
    for (const selector of selectors) expect(selector.startsWith(':where(.logic-editor) ')).toBe(true);
  });

  it('keeps upstream touch-action on the pane', () => {
    expect(css).toMatch(/:where\(\.logic-editor\) \.react-flow__pane \{[^}]*touch-action: none;/);
  });
});
