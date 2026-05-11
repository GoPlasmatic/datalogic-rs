// Read-only viewer.
//
// Renders a JSONLogic expression as an interactive flow diagram. No data,
// no editing — useful for documentation, rule explainers, and read-only
// dashboards.

import '@xyflow/react/dist/style.css';
import '@goplasmatic/datalogic-ui/styles.css';

import { DataLogicEditor } from '@goplasmatic/datalogic-ui';

const expression = {
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
