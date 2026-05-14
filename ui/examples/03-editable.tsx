// Editable mode (controlled).
//
// Pair `editable` with `value` + `onChange` for full visual editing —
// node selection, properties panel, context menus, and undo/redo are all
// driven from the toolbar. Add `data` to combine editing with live
// debugging.

import { useState } from 'react';

import '@xyflow/react/dist/style.css';
import '@goplasmatic/datalogic-ui/styles.css';

import { DataLogicEditor, type JsonLogicValue } from '@goplasmatic/datalogic-ui';

const initial: JsonLogicValue = {
  '>': [{ var: 'cart.total' }, 100],
};

export default function EditableExample() {
  const [expression, setExpression] = useState<JsonLogicValue | null>(initial);

  return (
    <div style={{ width: '100%', height: '600px' }}>
      <DataLogicEditor
        value={expression}
        onChange={setExpression}
        editable
      />
      <pre>{JSON.stringify(expression, null, 2)}</pre>
    </div>
  );
}
