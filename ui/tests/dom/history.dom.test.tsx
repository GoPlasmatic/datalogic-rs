// @vitest-environment jsdom
/**
 * Undo / redo stacks (useHistoryState), driven the way EditorProvider does.
 */
import { useEffect, useRef, useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { act, renderHook } from '@testing-library/react';
import './setup';
import { useHistoryState } from '../../src/components/logic-editor/context/editor/useHistoryState';
import type { LogicNode } from '../../src/lib';

function literal(id: string, value: number): LogicNode {
  return {
    id,
    type: 'literal',
    position: { x: 0, y: 0 },
    data: { type: 'literal', value, valueType: 'number', expression: value },
  };
}

function useHarness(onNodesChange?: (nodes: LogicNode[]) => void, clearSelection = () => {}) {
  const [nodes, setNodes] = useState<LogicNode[]>([literal('a', 1)]);
  // Synced in an effect, as EditorProvider does.
  const nodesRef = useRef(nodes);
  useEffect(() => {
    nodesRef.current = nodes;
  }, [nodes]);
  const history = useHistoryState(nodesRef, setNodes, onNodesChange, clearSelection);
  /** An edit as the mutation hooks make one: snapshot, then replace. */
  const edit = (next: LogicNode[]) => {
    history.pushToUndoStack(nodes);
    setNodes(next);
  };
  return { nodes, edit, ...history };
}

describe('useHistoryState', () => {
  it('undoes and redoes edits in order', () => {
    const onNodesChange = vi.fn();
    const clearSelection = vi.fn();
    const { result } = renderHook(() => useHarness(onNodesChange, clearSelection));
    const first = result.current.nodes;

    act(() => result.current.edit([literal('a', 2)]));
    act(() => result.current.edit([literal('a', 3)]));
    expect(result.current.canUndo).toBe(true);
    expect(result.current.canRedo).toBe(false);

    act(() => result.current.undo());
    expect(result.current.nodes[0].data).toMatchObject({ value: 2 });
    act(() => result.current.undo());
    expect(result.current.nodes).toBe(first);
    expect(result.current.canUndo).toBe(false);
    expect(result.current.canRedo).toBe(true);

    act(() => result.current.redo());
    expect(result.current.nodes[0].data).toMatchObject({ value: 2 });
    expect(onNodesChange).toHaveBeenCalledTimes(3);
    expect(clearSelection).toHaveBeenCalledTimes(3);
  });

  it('drops the redo stack on a new edit', () => {
    const { result } = renderHook(() => useHarness());
    act(() => result.current.edit([literal('a', 2)]));
    act(() => result.current.undo());
    expect(result.current.canRedo).toBe(true);
    act(() => result.current.edit([literal('a', 5)]));
    expect(result.current.canRedo).toBe(false);
  });

  it('keeps snapshots by reference, sharing unchanged nodes', () => {
    const { result } = renderHook(() => useHarness());
    const before = result.current.nodes;
    act(() => result.current.edit([...before, literal('b', 9)]));
    act(() => result.current.undo());
    expect(result.current.nodes).toBe(before);
  });

  it('keeps at most 50 undo steps', () => {
    const { result } = renderHook(() => useHarness());
    for (let i = 0; i < 60; i++) act(() => result.current.edit([literal('a', 100 + i)]));
    let undos = 0;
    while (result.current.canUndo) {
      act(() => result.current.undo());
      undos++;
    }
    expect(undos).toBe(50);
  });
});
