# Examples

Minimal copy-paste starting points for `@goplasmatic/datalogic-ui`. Each
file is a single React component that shows one mode of `DataLogicEditor`
in roughly 30–40 lines. The full `src/App.tsx` playground in this
package is a more comprehensive demo; these examples are the small,
focused shapes you can lift straight into your own app.

| Example                       | Mode                  | What it shows                                                |
|-------------------------------|-----------------------|--------------------------------------------------------------|
| `01-readonly-viewer.tsx`      | Read-only             | Render an expression as a flow diagram — no `data`, no edits |
| `02-debugger.tsx`             | Debugger              | Add a `data` prop to enable step-through trace inspection    |
| `03-editable.tsx`             | Editable + onChange   | Full visual editing with controlled state and persistence    |

## Running

These files aren't wired into a build target — they're reference
snippets. Drop one into a Vite/Next/CRA project that has the peer
dependencies installed:

```bash
npm install @goplasmatic/datalogic-ui @xyflow/react react react-dom
```

Then import the example component as your page/route. The required CSS
imports (React Flow base + the component's own styles) are at the top of
each example.

## Where to look next

- Full props reference and customization options: see the
  [package README](../README.md) and the
  [docs site](https://goplasmatic.github.io/datalogic-rs/react-ui/installation.html).
- Live playground in your browser:
  [goplasmatic.github.io/datalogic-rs/playground/](https://goplasmatic.github.io/datalogic-rs/playground/).
