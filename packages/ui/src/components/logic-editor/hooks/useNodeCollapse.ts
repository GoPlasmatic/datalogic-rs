import { useCallback } from 'react';
import { useReactFlow } from '@xyflow/react';
import type { LogicNode, OperatorNodeData } from '../types';

type CollapsibleNodeData = OperatorNodeData;

/**
 * Hook that provides a toggle callback for collapsing/expanding nodes.
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
