// Read-only viewer.
//
// Renders a JSONLogic expression as an interactive flow diagram. No data,
// no editing — useful for documentation, rule explainers, and read-only
// dashboards.

import '@goplasmatic/datalogic-ui/styles.css';

import { DataLogicEditor, type JsonLogicValue } from '@goplasmatic/datalogic-ui';

// The annotation matters: TypeScript widens a bare literal whose array holds
// two different operator keys into a union that JsonLogicValue does not accept.
const expression: JsonLogicValue = {
  and: [
    { '>': [{ var: 'age' }, 18] },
    { '==': [{ var: 'status' }, 'active'] },
  ],
};

export default function ReadOnlyViewer() {
  return (
    <div style={{ width: '100%', height: '500px' }}>
      <DataLogicEditor value={expression} />
    </div>
  );
}
