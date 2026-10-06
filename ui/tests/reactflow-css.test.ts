/**
 * The bundled React Flow base CSS must match the installed @xyflow/react,
 * scoped to the editor. Regenerate with `node scripts/reactflow-css.mjs`.
 */
import { execFileSync } from 'node:child_process';
import { readdirSync, readFileSync } from 'node:fs';
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

  it('leaves no editor rule that targets every React Flow on the page', () => {
    const dir = resolve(uiRoot, 'src/components/logic-editor');
    const files = readdirSync(dir, { recursive: true, encoding: 'utf8' }).filter((f) => f.endsWith('.css'));
    const leaks: string[] = [];
    for (const file of files) {
      const text = readFileSync(resolve(dir, file), 'utf8').replace(/\/\*[\s\S]*?\*\//g, '');
      for (const m of text.matchAll(/([^{}]+)\{/g)) {
        for (const selector of m[1].split(',').map((s) => s.trim())) {
          if (selector.startsWith('.react-flow')) leaks.push(`${file}: ${selector}`);
        }
      }
    }
    expect(leaks).toEqual([]);
  });
});
