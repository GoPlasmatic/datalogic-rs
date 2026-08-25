import type {
  LogicNode,
  JsonLogicValue,
  CellData,
  OperatorNodeData,
} from '../../../types';
import type { ExpressionNode } from '../../../types/trace';
import type { ParentInfo } from '../../converters/types';
import type { TraceContext, ChildMatch } from '../types';
import { generateExpressionText, formatOperandLabel } from '../../formatting';
import { isSimpleOperand } from '../../type-helpers';
import { createBranchEdge, createArgEdge } from '../../node-factory';
import { matchOperandsToChildren, unmatchedChildren } from '../child-matching';
import { mapInlinedChildren } from '../inline-mapping';
import { traceIdToNodeId } from '../trace-ids';

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
 * Create a VerticalCellNode for switch/match expressions from trace data.
 *
 * JSON structure: {"switch": [discriminant, [[case1, result1], ...], default]}
 *
 * The engine nests the case expressions two levels deep in its tree
 * (cases array -> pair array -> case / result), so the pair wrappers are
 * flattened here and folded onto the switch node; only the case / result
 * expressions become child nodes.
 *
 * Visual layout mirrors if/else but with case/then pairs:
 *   Match   -> discriminant
 *   Case    -> case value (inline or branch)
 *   Then    -> result
 *   ...
 *   Default -> default value
 */
export function createSwitchNodeFromTrace(
  nodeId: string,
  expression: JsonLogicValue,
  children: ExpressionNode[],
  context: TraceContext,
  parentInfo: ParentInfo,
  processExpressionNode: ProcessExpressionNodeFn,
  createFallbackNode: CreateFallbackNodeFn
): void {
  const obj = expression as Record<string, unknown>;
  const operator = Object.keys(obj)[0];
  const rawArgs = obj[operator];
  const switchArgs: JsonLogicValue[] = Array.isArray(rawArgs) ? rawArgs : [rawArgs as JsonLogicValue];

  const cells: CellData[] = [];
  let cellIndex = 0;

  // Top level: [discriminant, cases wrapper, default] against the direct children
  const topMatches = matchOperandsToChildren(switchArgs, children, context.templating);
  mapInlinedChildren(unmatchedChildren(children, topMatches), nodeId, context.traceNodeMap);

  // The cases wrapper and its pair arrays record no steps of their own; fold
  // them onto the switch node and match case / result expressions against the
  // pair children.
  const casesOperand = switchArgs.length >= 2 ? switchArgs[1] : undefined;
  const casePairs: JsonLogicValue[][] = Array.isArray(casesOperand)
    ? (casesOperand as JsonLogicValue[][]).filter((pair) => Array.isArray(pair) && pair.length >= 2)
    : [];
  const wrapper = topMatches[1]?.child;
  const pairMatches: (ChildMatch | null)[] = wrapper
    ? matchOperandsToChildren(casePairs, wrapper.children ?? [], context.templating)
    : casePairs.map(() => null);
  if (wrapper) {
    context.traceNodeMap.set(traceIdToNodeId(wrapper.id), nodeId);
    mapInlinedChildren(unmatchedChildren(wrapper.children ?? [], pairMatches), nodeId, context.traceNodeMap);
  }
  const leafMatches: (ChildMatch | null)[][] = casePairs.map((pair, i) => {
    const pairNode = pairMatches[i]?.child;
    if (!pairNode) return [null, null];
    context.traceNodeMap.set(traceIdToNodeId(pairNode.id), nodeId);
    const matches = matchOperandsToChildren([pair[0], pair[1]], pairNode.children ?? [], context.templating);
    mapInlinedChildren(unmatchedChildren(pairNode.children ?? [], matches), nodeId, context.traceNodeMap);
    return matches;
  });

  // Helper to render a branch value from its trace child (or a fallback node)
  function processBranch(
    value: JsonLogicValue,
    match: ChildMatch | null,
    argIndex: number,
    branchType?: ParentInfo['branchType']
  ): string {
    if (match) {
      return processExpressionNode(match.child, context, {
        parentId: nodeId,
        argIndex,
        branchType,
      }, value);
    }
    const branchId = `${nodeId}-arg-${argIndex}`;
    createFallbackNode(branchId, value, context, {
      parentId: nodeId,
      argIndex,
      branchType,
    });
    return branchId;
  }

  // args[0] = discriminant
  if (switchArgs.length >= 1) {
    const discriminant = switchArgs[0];

    if (isSimpleOperand(discriminant)) {
      cells.push({
        type: 'inline',
        icon: 'diamond',
        rowLabel: 'Match',
        label: formatOperandLabel(discriminant),
        index: cellIndex,
      });
    } else {
      const discBranchId = processBranch(discriminant, topMatches[0], 0, 'branch');
      context.edges.push(createBranchEdge(nodeId, discBranchId, cellIndex));

      cells.push({
        type: 'branch',
        icon: 'diamond',
        rowLabel: 'Match',
        label: generateExpressionText(discriminant, 40),
        branchId: discBranchId,
        index: cellIndex,
      });
    }
    cellIndex++;
  }

  // args[1] = cases array: [[case_val, result], ...]
  for (let i = 0; i < casePairs.length; i++) {
    const pair = casePairs[i];
    const caseValue = pair[0];
    const resultValue = pair[1];

    // Case value row
    if (isSimpleOperand(caseValue)) {
      cells.push({
        type: 'inline',
        icon: 'tag',
        rowLabel: 'Case',
        label: formatOperandLabel(caseValue),
        index: cellIndex,
      });
    } else {
      const caseBranchId = processBranch(caseValue, leafMatches[i][0], cellIndex, 'branch');
      context.edges.push(createBranchEdge(nodeId, caseBranchId, cellIndex));

      cells.push({
        type: 'branch',
        icon: 'tag',
        rowLabel: 'Case',
        label: generateExpressionText(caseValue, 40),
        branchId: caseBranchId,
        index: cellIndex,
      });
    }
    cellIndex++;

    // Result value row (Then)
    if (isSimpleOperand(resultValue)) {
      cells.push({
        type: 'inline',
        icon: 'check',
        rowLabel: 'Then',
        label: formatOperandLabel(resultValue),
        index: cellIndex,
      });
    } else {
      const resultBranchId = processBranch(resultValue, leafMatches[i][1], cellIndex, 'yes');
      context.edges.push(createBranchEdge(nodeId, resultBranchId, cellIndex));

      cells.push({
        type: 'branch',
        icon: 'check',
        rowLabel: 'Then',
        label: generateExpressionText(resultValue, 40),
        branchId: resultBranchId,
        index: cellIndex,
      });
    }
    cellIndex++;
  }

  // args[2] = default (optional)
  if (switchArgs.length >= 3) {
    const defaultValue = switchArgs[2];

    if (isSimpleOperand(defaultValue)) {
      cells.push({
        type: 'inline',
        icon: 'x',
        rowLabel: 'Default',
        label: formatOperandLabel(defaultValue),
        index: cellIndex,
      });
    } else {
      const defaultBranchId = processBranch(defaultValue, topMatches[2], cellIndex, 'no');
      context.edges.push(createBranchEdge(nodeId, defaultBranchId, cellIndex));

      cells.push({
        type: 'branch',
        icon: 'x',
        rowLabel: 'Default',
        label: generateExpressionText(defaultValue, 40),
        branchId: defaultBranchId,
        index: cellIndex,
      });
    }
  }

  // Generate expression text
  const expressionText = generateExpressionText(expression);
  const label = operator === 'match' ? 'Match / Case' : 'Switch / Case';

  const switchNode: LogicNode = {
    id: nodeId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator,
      category: 'control',
      label,
      icon: 'diamond',
      cells,
      collapsed: false,
      expressionText,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
      expression,
    } as OperatorNodeData,
  };
  context.nodes.push(switchNode);

  // Add edge from parent if exists and not a branch type
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }
}
