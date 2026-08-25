import type { JsonLogicValue, LogicNode, OperatorNodeData, CellData, LogicEdge } from '../../types';
import type { ConversionContext, ConverterFn } from './types';
import { getParentInfo } from './types';
import { generateExpressionText } from '../formatting';
import { createArgEdge } from '../node-factory';
import { v4 as uuidv4 } from 'uuid';

type BranchType = 'yes' | 'no' | 'branch' | 'condition' | undefined;

/** Operators rendered as a decision diamond chain. */
export function isIfOperator(operator: string): boolean {
  return operator === 'if' || operator === '?:';
}

/** The three inputs of a decision diamond. */
export type DecisionSlot = 'when' | 'then' | 'else';

/**
 * Which diamond input a cell belongs to, from its row label (the converter and
 * trace builder write 'when'/'then'/'else'; older in-editor nodes used
 * 'If'/'Then'/'Else'). Undefined for cells that are not part of a diamond.
 */
export function decisionSlotOf(cell: CellData): DecisionSlot | undefined {
  const label = (cell.rowLabel ?? '').toLowerCase();
  if (label === 'when' || label === 'if') return 'when';
  if (label === 'then') return 'then';
  if (label === 'else') return 'else';
  return undefined;
}

/**
 * True when the cells carry the diamond layout: a when and a then input and
 * at most one else input, nothing else. A generic `if` node (single operand,
 * shorthand form, or built from an invalid rule) does not.
 */
export function isDecisionCells(cells: CellData[]): boolean {
  if (cells.length < 2 || cells.length > 3) return false;
  const slots = cells.map(decisionSlotOf);
  if (slots.some((s) => s === undefined)) return false;
  return (
    slots.filter((s) => s === 'when').length === 1 &&
    slots.filter((s) => s === 'then').length === 1 &&
    slots.filter((s) => s === 'else').length <= 1
  );
}

/** Look up the cell for a diamond input. */
export function decisionCell(cells: CellData[], slot: DecisionSlot): CellData | undefined {
  return cells.find((c) => decisionSlotOf(c) === slot);
}

const DECISION_ICONS: Record<DecisionSlot, CellData['icon']> = {
  when: 'diamond',
  then: 'check',
  else: 'x',
};

const DECISION_INDEX: Record<DecisionSlot, number> = { when: 0, then: 1, else: 2 };

/** Build a branch cell for a diamond input. */
export function makeDecisionBranchCell(slot: DecisionSlot, branchId: string, label?: string): CellData {
  return {
    type: 'branch',
    icon: DECISION_ICONS[slot],
    rowLabel: slot,
    label,
    branchId,
    index: DECISION_INDEX[slot],
  };
}

/** Build an inline placeholder cell for a diamond input (value is stored on the expression). */
export function makeDecisionInlineCell(slot: DecisionSlot, label: string): CellData {
  return {
    type: 'inline',
    icon: DECISION_ICONS[slot],
    rowLabel: slot,
    label,
    index: DECISION_INDEX[slot],
  };
}

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
 * Convert if/then/else into a CHAIN of standalone decision-diamond nodes, one
 * diamond per condition, rather than a single block. Each diamond has three
 * inputs (when / then / else); a trailing else-if becomes the next diamond wired
 * into the else input, so the diamonds read as a series down the else path.
 *
 * The caller guarantees at least two arguments (condition + then); shorter or
 * non-array forms are converted as generic operator nodes so they serialize
 * back unchanged.
 */
export function convertIfElse(
  operator: string,
  ifArgs: JsonLogicValue[],
  context: ConversionContext,
  convertValue: ConverterFn
): string {
  const parentInfo = getParentInfo(context);
  return buildDiamond(
    operator,
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
  operator: string,
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
  // The diamond's own operands: its condition, its then value and, for an
  // else-if chain, the remaining chain nested as the else value. The
  // serializer flattens the chain back into the flat argument list.
  const ownArgs: JsonLogicValue[] = [condition, thenValue];

  // when: the condition (input 0)
  const condId = convertValue(condition, {
    nodes: context.nodes,
    edges: context.edges,
    parentId: diamondId,
    argIndex: 0,
    branchType: 'condition',
    templating: context.templating,
  });
  context.edges.push(diamondEdge(diamondId, condId, 0));
  cells.push(makeDecisionBranchCell('when', condId, generateExpressionText(condition, 40)));

  // then: the value when the condition holds (input 1)
  const thenId = convertValue(thenValue, {
    nodes: context.nodes,
    edges: context.edges,
    parentId: diamondId,
    argIndex: 1,
    branchType: 'yes',
    templating: context.templating,
  });
  context.edges.push(diamondEdge(diamondId, thenId, 1));
  cells.push(makeDecisionBranchCell('then', thenId, generateExpressionText(thenValue, 40)));

  // else: a final value, or the next diamond in the chain (else-if)
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
    cells.push(makeDecisionBranchCell('else', elseId, generateExpressionText(rest[0], 40)));
    ownArgs.push(rest[0]);
  } else if (rest.length >= 2) {
    const nextId = buildDiamond(operator, rest, context, convertValue, diamondId, 2, 'no', true);
    context.edges.push(diamondEdge(diamondId, nextId, 2));
    const nested = { [operator]: rest };
    cells.push(makeDecisionBranchCell('else', nextId, generateExpressionText(nested, 40)));
    ownArgs.push(nested);
  }

  const node: LogicNode = {
    id: diamondId,
    type: 'operator',
    position: { x: 0, y: 0 },
    data: {
      type: 'operator',
      operator,
      category: 'control',
      label: isElif ? 'elif' : 'if',
      icon: 'diamond',
      cells,
      collapsed: false,
      expressionText: generateExpressionText({ [operator]: args }),
      parentId,
      argIndex,
      branchType,
      expression: { [operator]: ownArgs },
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
