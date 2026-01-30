/**
 * Properties Panel Components
 *
 * The right-side panel for viewing and editing node properties in edit mode.
 */

export { PropertiesPanel } from './PropertiesPanel';
export { HelpSection } from './HelpSection';
export { ArgumentsSection } from './ArgumentsSection';
export {
  type ArgumentInfo,
  supportsVariableArgs,
  hasArguments,
  getOperatorName,
  isSimpleLiteral,
  getLiteralType,
  formatNodeValue,
  extractArguments,
} from './utils/argument-parser';

// Import CSS
import './properties-panel.css';
