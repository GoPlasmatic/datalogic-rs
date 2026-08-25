// Debugger mode.
//
// Pass a `data` prop and the editor exposes step-through controls and a step
// timeline over the engine's execution trace. As you step, the current node
// shows its context and result in a bubble; nodes do not show results at
// rest. The expression itself stays read-only, so adopters typically combine
// this with a code editor when they want both editing and tracing (see
// 03-editable.tsx).

import '@goplasmatic/datalogic-ui/styles.css';

import { DataLogicEditor, type JsonLogicValue } from '@goplasmatic/datalogic-ui';

const expression: JsonLogicValue = {
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
