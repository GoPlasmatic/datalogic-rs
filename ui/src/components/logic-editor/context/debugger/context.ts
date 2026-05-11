import { createContext } from 'react';
import type { DebuggerContextValue } from './types';

// Create context - exported from separate file to avoid Fast Refresh issues
export const DebuggerContext = createContext<DebuggerContextValue | null>(null);
