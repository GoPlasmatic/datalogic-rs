/**
 * Node Mutation Service
 *
 * Barrel file that re-exports all node mutation functions.
 * The actual implementations are split across:
 * - node-creation-service.ts (node creation helpers)
 * - argument-service.ts (add/remove arguments)
 * - node-transform-service.ts (wrap/duplicate operations)
 */

export { getDefaultValueForCategory, createArgumentNode } from './node-creation-service';
export { addArgument, removeArgument, canEditArguments, type AddArgumentResult } from './argument-service';
export { wrapInOperator, duplicateNodeTree } from './node-transform-service';
export { updateInlineOperand, inlineOperandForCell } from './inline-edit-service';
export {
  cloneNodesWithIdMapping,
  getDescendants,
  updateParentChildReference,
  replaceChildReference,
} from '../utils/node-cloning';
