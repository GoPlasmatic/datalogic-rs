/**
 * Services Index
 *
 * Re-exports all editor services for convenient imports.
 */

export { useHistoryService, type HistoryService } from './history-service';
export { useSelectionService, type SelectionService } from './selection-service';
export { useClipboardService, type ClipboardService, type PasteResult } from './clipboard-service';
export {
  addArgument,
  removeArgument,
  wrapInOperator,
  duplicateNodeTree,
  createArgumentNode,
  getDefaultValueForCategory,
  type AddArgumentResult,
} from './node-mutation-service';
