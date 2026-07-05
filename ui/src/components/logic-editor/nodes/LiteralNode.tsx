import { memo } from 'react';
import type { LiteralNodeData } from '../types';
import { signalForLiteral, signalVar } from '../utils/signal';
import { formatValue } from '../utils/formatting';
import { useDebugClassName } from '../hooks';
import { NodeInputHandles, NodeDebugBubble } from './shared';

interface LiteralNodeProps {
  id: string;
  data: LiteralNodeData;
  selected?: boolean;
}

/** Short type tag shown on the folded-corner constant tag (Signal Board). */
const LITERAL_PTAG: Record<LiteralNodeData['valueType'], string> = {
  number: 'num',
  string: 'str',
  boolean: 'bool',
  array: 'arr',
  null: 'null',
};

export const LiteralNode = memo(function LiteralNode({
  id,
  data,
  selected,
}: LiteralNodeProps) {
  const color = signalVar(signalForLiteral(data.valueType));
  const debugClassName = useDebugClassName(id);

  // "shape = role": a bare constant renders as a folded-corner tag, tinted by
  // its value type (the signal it carries), value in mono.
  return (
    <div
      className={`dl-node literal-node dl-shape-literal ${selected ? 'selected' : ''} ${debugClassName}`}
      style={{ ['--dl-sig']: color } as React.CSSProperties}
    >
      <NodeDebugBubble nodeId={id} position="top" />
      <NodeInputHandles nodeId={id} color={color} />

      <span className="dl-lit">
        <span className="dl-lit-ptag">{LITERAL_PTAG[data.valueType]}</span>
        <span className="literal-value">{formatValue(data.value)}</span>
      </span>
    </div>
  );
});
