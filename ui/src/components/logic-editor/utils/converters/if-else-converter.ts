import type { JsonLogicValue, LogicNode, OperatorNodeData, CellData, LogicEdge } from '../../types';
import type { ConversionContext, ConverterFn } from './types';
import { getParentInfo } from './types';
import { generateExpressionText } from '../formatting';
import { createArgEdge } from '../node-factory';
import { v4 as uuidv4 } from 'uuid';

type BranchType = 'yes' | 'no' | 'branch' | 'condition' | undefined;

// Edge from a diamond to one of its inputs. Rendered edges are rebuilt
// child->parent from the cells; this parent->child form is for the dagre layout.
function diamondEdge(nodeId: string, childId: string, cellIndex: number): LogicEdge {
  return {
    id: `${nodeId}-b${cellIndex}-${childId}`,
    source: nodeId,
    target: childId,
    sourceHandle: `branch-${cellIndex}`,
    targetHandle: 'left',
  };
}

/**
 * Convert if/then/else into a CHAIN of standalone decision-diamond nodes — one
 * diamond per condition, rather than a single block. Each diamond has three
 * inputs (when / then / else); a trailing else-if becomes the next diamond wired
 * into the else input, so the diamonds read as a series down the else path.
 */
export function convertIfElse(
  ifArgs: JsonLogicValue[],
  context: ConversionContext,
  convertValue: ConverterFn
): string {
  const parentInfo = getParentInfo(context);

  // A lone value (no condition) — just render it.
  if (ifArgs.length === 1) {
    return convertValue(ifArgs[0], {
      nodes: context.nodes,
      edges: context.edges,
      parentId: parentInfo.parentId,
      argIndex: parentInfo.argIndex,
      branchType: parentInfo.branchType,
      templating: context.templating,
    });
  }

  return buildDiamond(
    ifArgs,
    context,
    convertValue,
    parentInfo.parentId,
    parentInfo.argIndex,
    parentInfo.branchType,
    false
  );
}

function buildDiamond(
  args: JsonLogicValue[],
  context: ConversionContext,
  convertValue: ConverterFn,
  parentId: string | undefined,
  argIndex: number | undefined,
  branchType: BranchType,
  isElif: boolean
): string {
  const diamondId = uuidv4();
  const condition = args[0];
  const thenValue = args[1];
  const rest = args.slice(2);
  const cells: CellData[] = [];

  // when — the condition (input 0)
  const condId = convertValue(condition, {
    nodes: context.nodes,
    edges: context.edges,
    parentId: diamondId,
    argIndex: 0,
    branchType: 'condition',
    templating: context.templating,
  });
  context.edges.push(diamondEdge(diamondId, condId, 0));
  cells.push({
    type: 'branch',
    icon: 'diamond',
    rowLabel: 'when',
    label: generateExpressionText(condition, 40),
    branchId: condId,
    index: 0,
  });

  // then — the value when the condition holds (input 1)
  const thenId = convertValue(thenValue, {
    nodes: context.nodes,
    edges: context.edges,
    parentId: diamondId,
    argIndex: 1,
    branchType: 'yes',
    templating: context.templating,
  });
  context.edges.push(diamondEdge(diamondId, thenId, 1));
  cells.push({
    type: 'branch',
    icon: 'check',
    rowLabel: 'then',
    label: generateExpressionText(thenValue, 40),
    branchId: thenId,
    index: 1,
  });

  // else — a final value, or the next diamond in the chain (else-if)
  if (rest.length === 1) {
    const elseId = convertValue(rest[0], {
      nodes: context.nodes,
      edges: context.edges,
      parentId: diamondId,
      argIndex: 2,
      branchType: 'no',
      templating: context.templating,
    });
    context.edges.push(diamondEdge(diamondId, elseId, 2));
    cells.push({
      type: 'branch',
      icon: 'x',
      rowLabel: 'else',
      label: generateExpressionText(rest[0], 40),
      branchId: elseId,
      index: 2,
    });
  } else if (rest.length >= 2) {
    const nextId = buildDiamond(rest, context, convertValue, diamondId, 2, 'no', true);
    context.edges.push(diamondEdge(diamondId, nextId, 2));
    cells.push({
      type: 'branch',
      icon: 'x',
      rowLabel: 'else',
      label: generateExpressionText({ if: rest }, 40),
      branchId: nextId,
      index: 2,
    });
  }

  const node: LogicNode = {
    id: diamondId,
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
      expressionText: generateExpressionText({ if: args }),
      parentId,
      argIndex,
      branchType,
      expression: { if: args },
    } as OperatorNodeData,
  };
  context.nodes.push(node);

  // Top-level diamond wires to its parent; nested (else-if) diamonds are wired by
  // their caller via the else input, so skip the extra arg edge for those.
  if (parentId && !branchType) {
    context.edges.push(createArgEdge(parentId, diamondId, argIndex ?? 0));
  }

  return diamondId;
}
