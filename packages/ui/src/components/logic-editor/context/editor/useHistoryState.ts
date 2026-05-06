/**
 * History State Hook
 *
 * Manages undo/redo history stacks for the editor.
 */

import { useCallback, useMemo, useRef, useState, type Dispatch, type RefObject, type SetStateAction } from 'react';
import type { LogicNode } from '../../types';

const MAX_HISTORY_SIZE = 50;

export function useHistoryState(
  nodesRef: RefObject<LogicNode[]>,
  setInternalNodes: Dispatch<SetStateAction<LogicNode[]>>,
  onNodesChange: ((nodes: LogicNode[]) => void) | undefined,
  clearSelection: () => void
) {
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

  const undo = useCallback(() => {
    if (undoStackRef.current.length === 0) return;

    const previousState = undoStackRef.current.pop()!;
    redoStackRef.current.push(JSON.parse(JSON.stringify(nodesRef.current)));
    setHistoryVersion((v) => v + 1);

    setInternalNodes(previousState);
    onNodesChange?.(previousState);
    clearSelection();
  }, [nodesRef, setInternalNodes, onNodesChange, clearSelection]);

  const redo = useCallback(() => {
    if (redoStackRef.current.length === 0) return;

    const nextState = redoStackRef.current.pop()!;
    undoStackRef.current.push(JSON.parse(JSON.stringify(nodesRef.current)));
    setHistoryVersion((v) => v + 1);

    setInternalNodes(nextState);
    onNodesChange?.(nextState);
    clearSelection();
  }, [nodesRef, setInternalNodes, onNodesChange, clearSelection]);

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

  return { pushToUndoStack, undo, redo, canUndo, canRedo };
}
