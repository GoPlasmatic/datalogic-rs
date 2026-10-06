import { v4 as uuidv4 } from 'uuid';
import type { JsonLogicValue, LogicNode, LogicEdge } from '../../types';

// Context passed to all converters
export interface ConversionContext {
  nodes: LogicNode[];
  edges: LogicEdge[];
  parentId?: string;
  argIndex?: number;
  branchType?: 'yes' | 'no' | 'branch' | 'condition';
  /** Enable templating mode (multi-key objects compile to output-shaping templates with embedded JSONLogic). */
  templating?: boolean;
  /**
   * Id for the root node (the one converted without a parent). Every other
   * node derives its id from its parent's (see `nodeIdFor`). Omitted: a
   * fresh uuid.
   */
  rootId?: string;
}

// Common parent info for node creation
export interface ParentInfo {
  parentId?: string;
  argIndex?: number;
  branchType?: 'yes' | 'no' | 'branch' | 'condition';
}

// Converter function signature - returns the created node ID
export type ConverterFn = (
  value: JsonLogicValue,
  context: ConversionContext
) => string;

/**
 * Id for the node a converter is about to create.
 *
 * Deterministic: a child's id is its parent's id plus its argument slot, so
 * converting the same rule twice yields the same ids. The editor re-converts
 * after every edit it reports through `onChange`; with random ids that
 * remounted the canvas and dropped the selection each time. Slots are unique
 * per parent: every converter gives each child a distinct `argIndex`.
 */
export function nodeIdFor(context: Pick<ConversionContext, 'parentId' | 'argIndex' | 'rootId'>): string {
  if (context.parentId === undefined) return context.rootId ?? uuidv4();
  return `${context.parentId}.${context.argIndex ?? 0}`;
}

// Extract parent info from context
export function getParentInfo(context: ConversionContext): ParentInfo {
  return {
    parentId: context.parentId,
    argIndex: context.argIndex,
    branchType: context.branchType,
  };
}
