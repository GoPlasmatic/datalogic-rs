import { useCallback } from 'react';
import { useReactFlow } from '@xyflow/react';
import type { LogicNode, OperatorNodeData, VerticalCellNodeData } from '../types';

type CollapsibleNodeData = OperatorNodeData | VerticalCellNodeData;

/**
 * Hook that provides a toggle callback for collapsing/expanding nodes.
 * Works with both OperatorNode and VerticalCellNode.
 */
export function useNodeCollapse(nodeId: string): (e: React.MouseEvent) => void {
  const { setNodes } = useReactFlow();

  return useCallback(
    (e: React.MouseEvent) => {
      e.stopPropagation();
      setNodes((nodes) =>
        nodes.map((node) => {
          if (node.id === nodeId) {
            const nodeData = node.data as CollapsibleNodeData;
            return {
              ...node,
              data: {
                ...nodeData,
                collapsed: !nodeData.collapsed,
              },
            } as LogicNode;
          }
          return node;
        })
      );
    },
    [nodeId, setNodes]
  );
}
