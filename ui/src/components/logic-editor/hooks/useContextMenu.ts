import { useState, useCallback, useMemo } from 'react';
import type { NodeMouseHandler } from '@xyflow/react';
import type { LogicNode } from '../types';
import { useEditorContext } from '../context/editor';

interface ContextMenuState {
  type: 'node' | 'canvas';
  x: number;
  y: number;
  nodeId?: string;
}

export function useContextMenu(isEditMode: boolean) {
  const { focusPropertyPanel, nodes: editorNodes } = useEditorContext();
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null);

  const handleNodeContextMenu: NodeMouseHandler<LogicNode> = useCallback(
    (event, node) => {
      if (!isEditMode) return;
      event.preventDefault();
      setContextMenu({
        type: 'node',
        x: event.clientX,
        y: event.clientY,
        nodeId: node.id,
      });
    },
    [isEditMode]
  );

  const handlePaneContextMenu = useCallback(
    (event: React.MouseEvent | MouseEvent) => {
      if (!isEditMode) return;
      event.preventDefault();
      setContextMenu({
        type: 'canvas',
        x: event.clientX,
        y: event.clientY,
      });
    },
    [isEditMode]
  );

  const handleNodeDoubleClick: NodeMouseHandler<LogicNode> = useCallback(
    (_event, node) => {
      if (!isEditMode) return;

      // Determine which field to focus based on node type
      let fieldId: string | undefined;
      switch (node.data.type) {
        case 'literal':
          fieldId = 'value';
          break;
        default:
          fieldId = undefined;
      }

      focusPropertyPanel(node.id, fieldId);
    },
    [isEditMode, focusPropertyPanel]
  );

  const handleCloseContextMenu = useCallback(() => {
    setContextMenu(null);
  }, []);

  const handleEditProperties = useCallback(() => {
    if (contextMenu?.nodeId) {
      const node = editorNodes.find((n) => n.id === contextMenu.nodeId);
      if (node) {
        let fieldId: string | undefined;
        switch (node.data.type) {
          case 'literal':
            fieldId = 'value';
            break;
          default:
            fieldId = undefined;
        }
        focusPropertyPanel(contextMenu.nodeId, fieldId);
      }
    }
    handleCloseContextMenu();
  }, [contextMenu, editorNodes, focusPropertyPanel, handleCloseContextMenu]);

  const contextMenuNode = useMemo(() => {
    if (contextMenu?.type === 'node' && contextMenu.nodeId) {
      return editorNodes.find((n) => n.id === contextMenu.nodeId);
    }
    return undefined;
  }, [contextMenu, editorNodes]);

  return {
    contextMenu,
    handleNodeContextMenu,
    handlePaneContextMenu,
    handleNodeDoubleClick,
    handleCloseContextMenu,
    handleEditProperties,
    contextMenuNode,
  };
}
