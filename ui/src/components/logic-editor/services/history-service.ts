/**
 * History Service
 *
 * Provides undo/redo functionality for the editor.
 * Extracted from EditorContext for better modularity and testability.
 */

import { useRef, useState, useCallback, useMemo } from 'react';
import type { LogicNode } from '../types';

const MAX_HISTORY_SIZE = 50;

export interface HistoryService {
  /** Push current state to undo stack */
  pushToUndoStack: (nodes: LogicNode[]) => void;
  /** Undo the last action */
  undo: () => LogicNode[] | null;
  /** Redo the last undone action */
  redo: () => LogicNode[] | null;
  /** Whether undo is available */
  canUndo: boolean;
  /** Whether redo is available */
  canRedo: boolean;
  /** Clear all history */
  clearHistory: () => void;
}

/**
 * Hook to create a history service for undo/redo functionality
 *
 * @param currentNodesRef - Ref to the current nodes array (for capturing current state)
 * @returns HistoryService object
 */
export function useHistoryService(
  currentNodesRef: React.RefObject<LogicNode[]>
): HistoryService {
  const undoStackRef = useRef<LogicNode[][]>([]);
  const redoStackRef = useRef<LogicNode[][]>([]);
  const [historyVersion, setHistoryVersion] = useState(0);

  const pushToUndoStack = useCallback((nodes: LogicNode[]) => {
    undoStackRef.current = [
      ...undoStackRef.current.slice(-MAX_HISTORY_SIZE + 1),
      JSON.parse(JSON.stringify(nodes)),
    ];
    redoStackRef.current = [];
    setHistoryVersion((v) => v + 1);
  }, []);

  const undo = useCallback((): LogicNode[] | null => {
    if (undoStackRef.current.length === 0) return null;

    const previousState = undoStackRef.current.pop()!;
    if (currentNodesRef.current) {
      redoStackRef.current.push(JSON.parse(JSON.stringify(currentNodesRef.current)));
    }
    setHistoryVersion((v) => v + 1);

    return previousState;
  }, [currentNodesRef]);

  const redo = useCallback((): LogicNode[] | null => {
    if (redoStackRef.current.length === 0) return null;

    const nextState = redoStackRef.current.pop()!;
    if (currentNodesRef.current) {
      undoStackRef.current.push(JSON.parse(JSON.stringify(currentNodesRef.current)));
    }
    setHistoryVersion((v) => v + 1);

    return nextState;
  }, [currentNodesRef]);

  const clearHistory = useCallback(() => {
    undoStackRef.current = [];
    redoStackRef.current = [];
    setHistoryVersion((v) => v + 1);
  }, []);

  const canUndo = useMemo(
    () => undoStackRef.current.length > 0,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [historyVersion]
  );

  const canRedo = useMemo(
    () => redoStackRef.current.length > 0,
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [historyVersion]
  );

  return {
    pushToUndoStack,
    undo,
    redo,
    canUndo,
    canRedo,
    clearHistory,
  };
}
