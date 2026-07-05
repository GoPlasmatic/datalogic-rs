import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import type { OperatorNodeData, CellData } from '../types';
import { signalForOperator, signalVar } from '../utils/signal';
import {
  shapeForOperator,
  glyphForOperator,
  shapeHasLed,
  pillTypeForText,
  operatorRenderKind,
  isWiredCell,
  cellDisplayText,
} from '../utils/nodeShape';
import { useDebugClassName, useNodeCollapse } from '../hooks';
import { gateNodeHeight } from '../constants';
import { useIsFlowDirection } from '../context';
import { NodeInputHandles, CollapseToggleButton, NodeDebugBubble } from './shared';
import { CellRow } from './CellRow';
import { Icon } from '../utils/icons';
import { ExpressionSyntax } from '../utils/ExpressionSyntax';

interface UnifiedOperatorNodeProps {
  id: string;
  data: OperatorNodeData;
  selected?: boolean;
}

/**
 * A single operand inside an infix chip: a type-coloured pill (or a neutral
 * expr pill for a wired sub-expression). The wire for a wired operand lands on
 * the chip's single left-edge input handle, not here, so the arrow points at
 * the chip edge rather than into its middle.
 */
function InfixOperand({ cell }: { cell: CellData }) {
  const text = cellDisplayText(cell);
  const pill = pillTypeForText(text);
  return pill === 'expr' ? (
    <span className="dl-pill dl-pill-expr">
      <ExpressionSyntax text={text} />
    </span>
  ) : (
    <span className={`dl-pill dl-pill-${pill}`}>{text}</span>
  );
}

// Boolean logic-gate silhouettes (design-system gate shapes), viewBox "0 0 88 58".
const GATE_SILHOUETTES: Record<'and' | 'or' | 'not', React.ReactNode> = {
  and: <path className="dl-gate-sil" d="M8,4 H47 A25,25 0 0 1 47,54 H8 Z" />,
  or: <path className="dl-gate-sil" d="M6,4 Q30,29 6,54 Q46,52 80,29 Q46,6 6,4 Z" />,
  not: (
    <>
      <path className="dl-gate-sil" d="M12,6 L12,52 L60,29 Z" />
      <circle className="dl-gate-sil" cx="68" cy="29" r="5" />
    </>
  ),
};

export const UnifiedOperatorNode = memo(function UnifiedOperatorNode({
  id,
  data,
  selected,
}: UnifiedOperatorNodeProps) {
  // Signal Board: colour by the value type this operator PRODUCES (its signal),
  // exposed as the --dl-sig CSS var. `color` is a CSS var reference so child
  // handles/rows inherit the same signal tint. `shape` (its role) drives the CSS
  // silhouette; `renderKind` picks the tap/infix/card layout — the one decision
  // shared with utils/layout.ts so the node's footprint matches what's drawn.
  const color = signalVar(signalForOperator(data.operator, data.category));
  const shape = shapeForOperator(data.operator, data.category);
  const renderKind = operatorRenderKind(data);
  const isFlow = useIsFlowDirection();
  const debugClassName = useDebugClassName(id);
  const toggleNodeCollapse = useNodeCollapse(id);

  // "shape = role": var / val / exists render as a compact teal DATA TAP — a
  // plug that reads from the data and points into the flow — instead of a card.
  if (renderKind === 'tap') {
    const cellText = (c: (typeof data.cells)[number]) =>
      c.label || String(c.value ?? c.placeholder ?? '');
    const pathText = data.cells[0] ? cellText(data.cells[0]) : '';
    const extraCells = data.cells.slice(1);
    return (
      <div
        className={`dl-node dl-shape-tap ${selected ? 'selected' : ''} ${debugClassName}`}
        style={{ ['--dl-sig']: color } as React.CSSProperties}
      >
        <NodeDebugBubble nodeId={id} position="top" />
        <NodeInputHandles nodeId={id} color={color} />
        <span className="dl-tap">
          <span className="dl-tap-glyph" aria-hidden="true">⌁</span>
          <span className="dl-tap-path">{pathText || '…'}</span>
          {extraCells.map((c) => (
            <span className="dl-tap-val" key={c.index}>= {cellText(c)}</span>
          ))}
        </span>
      </div>
    );
  }

  // Boolean logic-gate silhouette (AND / OR / NOT) — used when every operand is a
  // wired child. Input ports spread down the left edge; the child nodes carry the
  // logic, so the gate itself shows only its shape + name.
  if (renderKind === 'gate-shape') {
    const gate = data.operator === 'or' ? 'or' : data.operator === 'and' ? 'and' : 'not';
    const label = gate === 'and' ? 'AND' : gate === 'or' ? 'OR' : 'NOT';
    const count = data.cells.length;
    return (
      <div
        className={`dl-node dl-gate-node dl-gate-${gate} ${selected ? 'selected' : ''} ${debugClassName}`}
        // Pin the drawn height to the layout's reserved height (gateNodeHeight) so
        // the silhouette exactly fills its footprint and the ports stay spread.
        style={{ ['--dl-sig']: color, minHeight: gateNodeHeight(count) } as React.CSSProperties}
      >
        <NodeDebugBubble nodeId={id} position="top" />
        <NodeInputHandles nodeId={id} color={color} />
        {data.cells.map((cell, i) => (
          <Handle
            key={cell.index}
            type={isFlow ? 'target' : 'source'}
            position={isFlow ? Position.Left : Position.Right}
            id={`branch-${cell.index}`}
            className="dl-gate-in"
            style={
              {
                [isFlow ? 'left' : 'right']: '-4px',
                top: `${((i + 1) / (count + 1)) * 100}%`,
                background: color,
              } as React.CSSProperties
            }
          />
        ))}
        <svg className="dl-gate-svg" viewBox="0 0 88 58" preserveAspectRatio="none" aria-hidden="true">
          {GATE_SILHOUETTES[gate]}
        </svg>
        <span className="dl-gate-label">{label}</span>
      </div>
    );
  }

  // Decision diamond (if / else-if) — a standalone flowchart diamond with three
  // inputs on the left (when / then / else). An else-if chains into the else
  // input, so the diamonds read as a series down the else path.
  if (renderKind === 'decision') {
    // Inputs sit on the diamond corners: when = top, then = left (right in
    // hierarchy), else = bottom. The parent/output (NodeInputHandles) is the
    // right corner in flow, left in hierarchy — so left/right reverse together.
    const inType = isFlow ? 'target' : 'source';
    const decPort = (
      cell: (typeof data.cells)[number] | undefined,
      position: Position,
      cls: string,
      title: string,
    ) =>
      cell ? (
        <Handle
          key={cell.index}
          type={inType}
          position={position}
          id={`branch-${cell.index}`}
          className={`dl-dec-port ${cls}`}
          title={title}
        />
      ) : null;
    return (
      <div
        className={`dl-node dl-decision-node ${selected ? 'selected' : ''} ${debugClassName}`}
        style={{ ['--dl-sig']: color } as React.CSSProperties}
      >
        <NodeDebugBubble nodeId={id} position="top" />
        <NodeInputHandles nodeId={id} color={color} />
        {decPort(data.cells.find((c) => c.icon === 'diamond'), Position.Top, 'dl-dec-when', 'when')}
        {decPort(data.cells.find((c) => c.icon === 'check'), isFlow ? Position.Left : Position.Right, 'dl-dec-then', 'then')}
        {decPort(data.cells.find((c) => c.icon === 'x'), Position.Bottom, 'dl-dec-else', 'else')}
        <svg className="dl-dec-svg" viewBox="0 0 88 88" preserveAspectRatio="none" aria-hidden="true">
          <path className="dl-dec-sil" d="M44,3 L85,44 L44,85 L3,44 Z" />
        </svg>
        <span className="dl-dec-label">{data.label === 'elif' ? 'elif' : 'if'}</span>
      </div>
    );
  }

  const glyph = glyphForOperator(data.operator);
  const hasLed = shapeHasLed(shape);

  // Compare (== != < > in …) and arithmetic (+ − × ÷ …) render as compact INFIX
  // chips — "score ≥ 60", "price × 1.2". Unary operators (!, !!) render the glyph
  // as a prefix. operatorRenderKind only picks infix when at most one operand is
  // wired, so the single wire lands cleanly on the chip's left edge.
  if (renderKind === 'infix-gate' || renderKind === 'infix-arith') {
    const infixGate = renderKind === 'infix-gate';
    const unary = data.cells.length === 1;
    const wired = data.cells.find(isWiredCell);
    const parts: React.ReactNode[] = [];
    data.cells.forEach((cell, i) => {
      if (unary || i > 0) {
        parts.push(
          <span key={`op-${i}`} className="dl-inop" aria-hidden="true">
            {glyph}
          </span>,
        );
      }
      parts.push(<InfixOperand key={`c-${cell.index}`} cell={cell} />);
    });
    return (
      <div
        className={`dl-node dl-infix-node dl-shape-${shape} ${
          infixGate ? 'dl-infix-gate' : 'dl-infix-arith'
        } ${selected ? 'selected' : ''} ${debugClassName}`}
        style={{ ['--dl-sig']: color } as React.CSSProperties}
      >
        <NodeDebugBubble nodeId={id} position="top" />
        <NodeInputHandles nodeId={id} color={color} />
        {/* the (at most one) wired operand connects at the chip's left edge */}
        {wired && (
          <Handle
            type={isFlow ? 'target' : 'source'}
            position={isFlow ? Position.Left : Position.Right}
            id={`branch-${wired.index}`}
            className="dl-infix-in"
            style={{ background: color }}
          />
        )}
        <span className="dl-infix">
          {infixGate && hasLed && <span className="dl-led" aria-hidden="true" />}
          {parts}
        </span>
      </div>
    );
  }

  // Card (if / and / or / value / iterator …) — header + stacked rows.
  const showGlyph = shape === 'gate' || shape === 'arith';
  const canCollapse = data.cells.length > 1;
  const isCollapsed = canCollapse ? (data.collapsed ?? false) : false;

  return (
    <div
      className={`dl-node vertical-cell-node dl-shape-${shape} ${selected ? 'selected' : ''} ${isCollapsed ? 'collapsed' : ''} ${debugClassName}`}
      style={{ ['--dl-sig']: color } as React.CSSProperties}
    >
      <NodeDebugBubble nodeId={id} position="top" />
      <NodeInputHandles nodeId={id} color={color} />

      {/* Header: shape cue (LED / op-glyph) + monochrome category badge + label.
          Colour is spent on the value the node produces, shape encodes its role. */}
      <div className="vertical-cell-header">
        {hasLed && <span className="dl-led" aria-hidden="true" />}
        {showGlyph && <span className="dl-op-glyph">{glyph}</span>}
        <span className="vertical-cell-icon">
          <Icon name={data.icon} size={13} />
        </span>
        <span className="vertical-cell-label">{data.label}</span>
        {canCollapse && (
          <CollapseToggleButton isCollapsed={isCollapsed} onClick={toggleNodeCollapse} />
        )}
      </div>

      {/* Body: either expression text (collapsed) or cell list (expanded).
          Adding/removing arguments lives in the Properties panel sidebar, not on
          the node itself, so the card stays a clean read of the expression. */}
      {isCollapsed ? (
        <div className="vertical-cell-body collapsed-body">
          <div className="expression-text">
            <ExpressionSyntax text={data.expressionText || '...'} />
          </div>
        </div>
      ) : (
        <div className="vertical-cell-body">
          {data.cells.map((cell) => (
            <CellRow key={cell.index} cell={cell} color={color} />
          ))}
        </div>
      )}
    </div>
  );
});
