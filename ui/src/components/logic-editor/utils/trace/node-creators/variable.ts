import type {
  LogicNode,
  JsonLogicValue,
  OperatorNodeData,
  CellData,
} from '../../../types';
import type { ExpressionNode } from '../../../types/trace';
import type { ParentInfo } from '../../converters/types';
import type { TraceContext } from '../types';
import { TRUNCATION_LIMITS } from '../../../constants';
import { getOperator } from '../../../config/operators';
import { getCategoryIcon } from '../../../config/categories';
import { type IconName } from '../../icons';
import { generateExpressionText, generateArgSummary } from '../../formatting';
import { isSimpleOperand } from '../../type-helpers';
import { createArgEdge, createBranchEdge, buildVariableCells } from '../../node-factory';
import {
  parseVarOperand,
  parseValOperand,
  parseExistsOperand,
  type VariableOperator,
} from '../../converters/variable-cells';
import { matchOperandsToChildren, unmatchedChildren } from '../child-matching';
import { mapInlinedChildren } from '../inline-mapping';

// Forward declaration for processExpressionNode and createFallbackNode
type ProcessExpressionNodeFn = (
  exprNode: ExpressionNode,
  context: TraceContext,
  parentInfo: ParentInfo,
  originalExpression?: JsonLogicValue
) => string;

type CreateFallbackNodeFn = (
  nodeId: string,
  value: JsonLogicValue,
  context: TraceContext,
  parentInfo: ParentInfo
) => void;

/**
 * Whether a var / val / exists expression has a static path (renderable as
 * editable cells). Computed paths are wired as ordinary operator arguments by
 * the caller instead.
 */
export function isStaticVariableExpression(operator: VariableOperator, operands: JsonLogicValue): boolean {
  if (operator === 'var') return parseVarOperand(operands).isStatic;
  if (operator === 'val') return parseValOperand(operands).isStatic;
  return parseExistsOperand(operands).isStatic;
}

/**
 * Create a variable node from trace data. Mirrors the static variable
 * converter: the path renders as editable cells, and a complex default value
 * (an expression) becomes a wired child, matched to its trace child.
 */
export function createVariableNodeFromTrace(
  nodeId: string,
  expression: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo,
  processExpressionNode?: ProcessExpressionNodeFn,
  createFallbackNode?: CreateFallbackNodeFn
): void {
  const obj = expression as Record<string, unknown>;
  const operator = Object.keys(obj)[0] as VariableOperator;
  const operands = obj[operator] as JsonLogicValue;

  const op = getOperator(operator);
  const category = op?.category ?? 'variable';
  const icon: IconName = getCategoryIcon(category) as IconName;

  // Parse operands to extract path, default value, scope, and path components
  let path: string | number = '';
  let defaultValue: JsonLogicValue | undefined;
  let scopeJump: number | undefined;
  let pathComponents: JsonLogicValue[] | undefined;

  if (operator === 'var') {
    const parsed = parseVarOperand(operands);
    path = parsed.path;
    defaultValue = parsed.defaultValue;
  } else if (operator === 'val') {
    const parsed = parseValOperand(operands);
    scopeJump = parsed.scope;
    pathComponents = parsed.components;
  } else {
    const parsed = parseExistsOperand(operands);
    if (Array.isArray(parsed.path)) {
      pathComponents = parsed.path;
    } else {
      path = parsed.path;
    }
  }

  // A complex default is a branch child; a simple one is an inline cell
  const hasComplexDefault = operator === 'var' && defaultValue !== undefined && !isSimpleOperand(defaultValue);
  const cells: CellData[] = buildVariableCells({
    operator,
    path,
    defaultValue: hasComplexDefault ? undefined : defaultValue,
    scopeJump,
    pathComponents,
  });

  if (hasComplexDefault && defaultValue !== undefined && processExpressionNode && createFallbackNode) {
    const matches = matchOperandsToChildren([defaultValue], children, context.templating);
    let branchId: string;
    if (matches[0]) {
      branchId = processExpressionNode(matches[0].child, context, {
        parentId: nodeId,
        argIndex: 1,
        branchType: 'branch',
      }, defaultValue);
    } else {
      branchId = `${nodeId}-arg-1`;
      createFallbackNode(branchId, defaultValue, context, {
        parentId: nodeId,
        argIndex: 1,
        branchType: 'branch',
      });
    }
    mapInlinedChildren(unmatchedChildren(children, matches), nodeId, context.traceNodeMap);

    const summary = generateArgSummary(defaultValue);
    summary.label = generateExpressionText(defaultValue, TRUNCATION_LIMITS.expressionText);
    cells.push({
      type: 'branch',
      rowLabel: 'Default',
      icon: 'hash',
      branchId,
      index: 1,
      summary,
    });
    // The branch edge uses the cell index so it matches CellHandles / edge-builder.
    context.edges.push(createBranchEdge(nodeId, branchId, 1));
  } else if (children.length > 0) {
    // Simple default (or no child creator): any trace children fold into this node
    mapInlinedChildren(children, nodeId, context.traceNodeMap);
  }

  const expressionText = generateExpressionText(expression);

  const node: LogicNode = {
    id: nodeId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator,
      category,
      label: op?.label ?? operator,
      icon,
      cells,
      collapsed: false,
      expressionText,
      expression,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
    } as OperatorNodeData,
  };
  context.nodes.push(node);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }
}
