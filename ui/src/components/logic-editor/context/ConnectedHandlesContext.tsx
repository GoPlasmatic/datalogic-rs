import { useMemo, type ReactNode } from 'react';
import type { LogicEdge } from '../types';
import { ConnectedHandlesContext, type ConnectedHandlesMap } from './ConnectedHandlesContextDef';

interface ConnectedHandlesProviderProps {
  edges: LogicEdge[];
  children: ReactNode;
}

export function ConnectedHandlesProvider({
  edges,
  children,
}: ConnectedHandlesProviderProps) {
  const connectedHandles = useMemo(() => {
    const map: ConnectedHandlesMap = new Map();

    for (const edge of edges) {
      // Track source handles
      const sourceHandleId = edge.sourceHandle || 'default';
      if (!map.has(edge.source)) {
        map.set(edge.source, new Set());
      }
      map.get(edge.source)!.add(sourceHandleId);

      // Track target handles
      const targetHandleId = edge.targetHandle || 'default';
      if (!map.has(edge.target)) {
        map.set(edge.target, new Set());
      }
      map.get(edge.target)!.add(targetHandleId);
    }

    return map;
  }, [edges]);

  return (
    <ConnectedHandlesContext.Provider value={connectedHandles}>
      {children}
    </ConnectedHandlesContext.Provider>
  );
}
