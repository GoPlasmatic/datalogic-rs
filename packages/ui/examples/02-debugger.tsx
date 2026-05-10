// Debugger mode.
//
// Pass a `data` prop and the editor exposes step-through controls, an
// execution trace, and per-node evaluation results. The expression
// itself stays read-only — adopters typically combine this with a code
// editor when they want both editing and tracing (see 03-editable.tsx).

import '@xyflow/react/dist/style.css';
import '@goplasmatic/datalogic-ui/styles.css';

import { DataLogicEditor } from '@goplasmatic/datalogic-ui';

const expression = {
  if: [
    { '>=': [{ var: 'score' }, 80] },
    'pass',
    'fail',
  ],
};

const data = { score: 92 };

export default function DebuggerExample() {
  return (
    <div style={{ width: '100%', height: '600px' }}>
      <DataLogicEditor value={expression} data={data} />
    </div>
  );
}
