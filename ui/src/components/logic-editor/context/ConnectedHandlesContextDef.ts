import { createContext } from 'react';

// Map from nodeId to Set of connected handle IDs
export type ConnectedHandlesMap = Map<string, Set<string>>;

export const ConnectedHandlesContext = createContext<ConnectedHandlesMap>(new Map());
