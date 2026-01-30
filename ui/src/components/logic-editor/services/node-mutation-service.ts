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
export { addArgument, removeArgument, type AddArgumentResult } from './argument-service';
export { wrapInOperator, duplicateNodeTree } from './node-transform-service';
export { cloneNodesWithIdMapping, getDescendants, updateParentChildReference } from '../utils/node-cloning';
