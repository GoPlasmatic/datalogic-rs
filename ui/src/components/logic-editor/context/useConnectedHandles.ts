import { useContext } from 'react';
import { ConnectedHandlesContext } from './ConnectedHandlesContextDef';

// Hook to check if a specific handle on a node is connected
export function useIsHandleConnected(nodeId: string, handleId: string): boolean {
  const connectedHandles = useContext(ConnectedHandlesContext);
  const nodeHandles = connectedHandles.get(nodeId);
  return nodeHandles?.has(handleId) ?? false;
}

// Hook to get all connected handles for a node
export function useConnectedHandles(nodeId: string): Set<string> {
  const connectedHandles = useContext(ConnectedHandlesContext);
  return connectedHandles.get(nodeId) ?? new Set();
}
