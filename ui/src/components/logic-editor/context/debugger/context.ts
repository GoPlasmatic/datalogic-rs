import { createContext } from 'react';
import type { DebuggerContextValue } from './types';
import type { DebugNodeStore } from './node-store';

// Create context - exported from separate file to avoid Fast Refresh issues
export const DebuggerContext = createContext<DebuggerContextValue | null>(null);

/**
 * Per-node debug state, for node components. The store object never
 * changes, so reading it does not re-render a node; nodes subscribe to
 * their own state through `useNodeDebugState`.
 */
export const DebugNodeStoreContext = createContext<DebugNodeStore | null>(null);
