/**
 * History State Hook
 *
 * Manages undo/redo history stacks for the editor.
 */

import { useCallback, useRef, useState, type Dispatch, type RefObject, type SetStateAction } from 'react';
import type { LogicNode } from '../../types';

const MAX_HISTORY_SIZE = 50;

export function useHistoryState(
  nodesRef: RefObject<LogicNode[]>,
  setInternalNodes: Dispatch<SetStateAction<LogicNode[]>>,
  onNodesChange: ((nodes: LogicNode[]) => void) | undefined,
  clearSelection: () => void
) {
  // Stacks live in refs because they hold deep clones, change synchronously
  // alongside reducer-style updates, and are read by callbacks. The two
  // `can*` booleans below mirror their `length > 0` state so consumers can
  // depend on them in render without reading the ref from the memo body.
  const undoStackRef = useRef<LogicNode[][]>([]);
  const redoStackRef = useRef<LogicNode[][]>([]);
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);

  const pushToUndoStack = useCallback((nodes: LogicNode[]) => {
    undoStackRef.current = [
      ...undoStackRef.current.slice(-MAX_HISTORY_SIZE + 1),
      JSON.parse(JSON.stringify(nodes)),
    ];
    redoStackRef.current = [];
    setCanUndo(true);
    setCanRedo(false);
  }, []);

  const undo = useCallback(() => {
    if (undoStackRef.current.length === 0) return;

    const previousState = undoStackRef.current.pop()!;
    redoStackRef.current.push(JSON.parse(JSON.stringify(nodesRef.current)));
    setCanUndo(undoStackRef.current.length > 0);
    setCanRedo(true);

    setInternalNodes(previousState);
    onNodesChange?.(previousState);
    clearSelection();
  }, [nodesRef, setInternalNodes, onNodesChange, clearSelection]);

  const redo = useCallback(() => {
    if (redoStackRef.current.length === 0) return;

    const nextState = redoStackRef.current.pop()!;
    undoStackRef.current.push(JSON.parse(JSON.stringify(nodesRef.current)));
    setCanUndo(true);
    setCanRedo(redoStackRef.current.length > 0);

    setInternalNodes(nextState);
    onNodesChange?.(nextState);
    clearSelection();
  }, [nodesRef, setInternalNodes, onNodesChange, clearSelection]);

  return { pushToUndoStack, undo, redo, canUndo, canRedo };
}
