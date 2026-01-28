/**
 * Edge Components
 *
 * Custom edge components for the visual editor.
 */

import { EditableEdge } from './EditableEdge';

export { EditableEdge } from './EditableEdge';
export { EdgeOperatorPicker } from './EdgeOperatorPicker';

// Edge types for ReactFlow
export const edgeTypes = {
  editable: EditableEdge,
};

// Import CSS
import './edges.css';
