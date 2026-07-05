import type {
  LogicNode,
  JsonLogicValue,
  CellData,
  OperatorNodeData,
} from '../../../types';
import type { ExpressionNode } from '../../../types/trace';
import type { ParentInfo } from '../../converters/types';
import type { TraceContext, NodeType } from '../types';
import { generateExpressionText } from '../../formatting';
import { createBranchEdge, createArgEdge } from '../../node-factory';
import { findMatchingChild, getNextUnusedChild } from '../child-matching';
import { determineNodeType } from '../node-type';

type BranchType = 'yes' | 'no' | 'branch' | 'condition';

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
 * Create a CHAIN of decision-diamond nodes for an if/then/else from trace data —
 * one diamond per condition (when / then / else), the else input chaining into
 * the next diamond for each else-if. The first diamond keeps the trace node id so
 * its debug mapping is preserved.
 */
export function createIfElseNodeFromTrace(
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
  const ifArgs = obj[operator] as JsonLogicValue[];
  const usedChildIndices = new Set<number>();

  // Resolve one arg to a child node via trace matching (exact, then positional,
  // then a fallback node), returning its id.
  const processArg = (
    value: JsonLogicValue,
    parentId: string,
    argIndex: number,
    branchType: BranchType,
  ): string => {
    const match = findMatchingChild(value, children, usedChildIndices);
    if (match) {
      usedChildIndices.add(match.index);
      return processExpressionNode(match.child, context, { parentId, argIndex, branchType });
    }
    const nodeType: NodeType = determineNodeType(value, context.templating);
    const nextUnused = nodeType !== 'literal' ? getNextUnusedChild(children, usedChildIndices) : null;
    if (nextUnused) {
      usedChildIndices.add(nextUnused.index);
      return processExpressionNode(nextUnused.child, context, { parentId, argIndex, branchType }, value);
    }
    const fallbackId = `${parentId}-arg${argIndex}`;
    createFallbackNode(fallbackId, value, context, { parentId, argIndex, branchType });
    return fallbackId;
  };

  // Split into (condition, then) pairs plus an optional final else.
  const pairs: { condition: JsonLogicValue; thenValue: JsonLogicValue }[] = [];
  for (let i = 0; i + 1 < ifArgs.length; i += 2) {
    pairs.push({ condition: ifArgs[i], thenValue: ifArgs[i + 1] });
  }
  const hasFinalElse = ifArgs.length % 2 === 1;
  const elseValue = hasFinalElse ? ifArgs[ifArgs.length - 1] : undefined;

  // First diamond keeps the trace node id (debug mapping); the rest get fresh ids.
  const diamondIds = pairs.map((_, k) => (k === 0 ? nodeId : `${nodeId}-elif-${k}`));

  // Process condition + then for each pair in evaluation order, then the else, so
  // the positional trace-child matching consumes children in the original order.
  const condIds: string[] = [];
  const thenIds: string[] = [];
  for (let k = 0; k < pairs.length; k++) {
    condIds.push(processArg(pairs[k].condition, diamondIds[k], 0, 'condition'));
    thenIds.push(processArg(pairs[k].thenValue, diamondIds[k], 1, 'yes'));
  }
  const elseBranchId =
    elseValue !== undefined
      ? processArg(elseValue, diamondIds[diamondIds.length - 1], 2, 'no')
      : undefined;

  // Build one diamond node per condition, chaining the else input to the next.
  for (let k = 0; k < pairs.length; k++) {
    const dId = diamondIds[k];
    const cells: CellData[] = [];

    cells.push({
      type: 'branch',
      icon: 'diamond',
      rowLabel: 'when',
      label: generateExpressionText(pairs[k].condition, 40),
      branchId: condIds[k],
      index: 0,
    });
    context.edges.push(createBranchEdge(dId, condIds[k], 0));

    cells.push({
      type: 'branch',
      icon: 'check',
      rowLabel: 'then',
      label: generateExpressionText(pairs[k].thenValue, 40),
      branchId: thenIds[k],
      index: 1,
    });
    context.edges.push(createBranchEdge(dId, thenIds[k], 1));

    const elseTarget = k < pairs.length - 1 ? diamondIds[k + 1] : elseBranchId;
    if (elseTarget) {
      const restArgs = ifArgs.slice((k + 1) * 2);
      cells.push({
        type: 'branch',
        icon: 'x',
        rowLabel: 'else',
        label:
          k < pairs.length - 1
            ? generateExpressionText({ if: restArgs }, 40)
            : generateExpressionText(elseValue as JsonLogicValue, 40),
        branchId: elseTarget,
        index: 2,
      });
      context.edges.push(createBranchEdge(dId, elseTarget, 2));
    }

    const isElif = k > 0;
    const node: LogicNode = {
      id: dId,
      type: 'operator',
      position: { x: 0, y: 0 },
      data: {
        type: 'operator',
        operator: 'if',
        category: 'control',
        label: isElif ? 'elif' : 'if',
        icon: 'diamond',
        cells,
        collapsed: false,
        expressionText: generateExpressionText({ if: ifArgs.slice(k * 2) }),
        parentId: isElif ? diamondIds[k - 1] : parentInfo.parentId,
        argIndex: isElif ? 2 : parentInfo.argIndex,
        branchType: isElif ? 'no' : parentInfo.branchType,
        expression: { if: ifArgs.slice(k * 2) },
      } as OperatorNodeData,
    };
    context.nodes.push(node);
  }

  // The head diamond wires to the parent (unless it's itself a branch child).
  if (parentInfo.parentId && !parentInfo.branchType) {
    context.edges.push(createArgEdge(parentInfo.parentId, nodeId, parentInfo.argIndex ?? 0));
  }
}
