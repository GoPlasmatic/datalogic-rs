/**
 * Services Index
 *
 * Re-exports all editor services for convenient imports.
 */

export {
  addArgument,
  removeArgument,
  canEditArguments,
  wrapInOperator,
  duplicateNodeTree,
  updateInlineOperand,
  inlineOperandForCell,
  type AddArgumentResult,
} from './node-mutation-service';
