import type { JsonLogicValue, LogicNode, LogicEdge } from '../../types';

// Context passed to all converters
export interface ConversionContext {
  nodes: LogicNode[];
  edges: LogicEdge[];
  parentId?: string;
  argIndex?: number;
  branchType?: 'yes' | 'no' | 'branch';
  /** Enable structure preserve mode for JSON templates with embedded JSONLogic */
  preserveStructure?: boolean;
}

// Common parent info for node creation
export interface ParentInfo {
  parentId?: string;
  argIndex?: number;
  branchType?: 'yes' | 'no' | 'branch';
}

// Converter function signature - returns the created node ID
export type ConverterFn = (
  value: JsonLogicValue,
  context: ConversionContext
) => string;

// Extract parent info from context
export function getParentInfo(context: ConversionContext): ParentInfo {
  return {
    parentId: context.parentId,
    argIndex: context.argIndex,
    branchType: context.branchType,
  };
}
