/**
 * PropertiesPanel Component
 *
 * The main properties panel that displays context-aware properties
 * for the currently selected node.
 */

import { memo, useEffect, useState, useRef, useImperativeHandle, forwardRef, useCallback } from 'react';
import { X, ChevronDown, ChevronRight, Trash2 } from 'lucide-react';
import { useEditorContext } from '../context/editor';
import { PanelRenderer, type PanelRendererRef } from '../panel-inputs';
import { HelpSection } from './HelpSection';
import { ArgumentsSection } from './ArgumentsSection';
import { isRootNode } from '../utils/node-deletion';
import {
  getPanelConfigForNode,
  getOperatorConfigForNode,
  getInitialValuesFromNode,
  getNodeDisplayLabel,
  getNodeCategory,
} from './utils';

interface PropertiesPanelProps {
  /** Width of the panel in pixels */
  width?: number;
}

export const PropertiesPanel = memo(function PropertiesPanel({
  width = 280,
}: PropertiesPanelProps) {
  const {
    selectedNode,
    isEditMode,
    panelValues,
    updatePanelValue,
    resetPanelValues,
    selectNode,
    applyPanelChanges,
    deleteNode,
    propertyPanelFocusRef,
  } = useEditorContext();

  // Timer ref for debounced auto-apply
  const applyTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Ref for PanelRenderer focus
  const panelRendererRef = useRef<PanelRendererRef>(null);

  // Pending focus field (set when focus is requested before panel is ready)
  const pendingFocusRef = useRef<string | undefined>(undefined);

  // Focus field handler
  const focusField = useCallback((fieldId?: string) => {
    if (panelRendererRef.current) {
      panelRendererRef.current.focusField(fieldId);
      pendingFocusRef.current = undefined;
    } else {
      // Store for later when panel is ready
      pendingFocusRef.current = fieldId;
    }
  }, []);

  // Register focus handler with editor context
  useEffect(() => {
    if (propertyPanelFocusRef) {
      propertyPanelFocusRef.current = { focusField };
    }
    return () => {
      if (propertyPanelFocusRef) {
        propertyPanelFocusRef.current = null;
      }
    };
  }, [propertyPanelFocusRef, focusField]);

  // Initialize panel values when selection changes
  useEffect(() => {
    if (selectedNode) {
      const initialValues = getInitialValuesFromNode(selectedNode.data);
      resetPanelValues(initialValues);
    }
  }, [selectedNode, resetPanelValues]);

  // Handle pending focus after selection change
  useEffect(() => {
    if (selectedNode && pendingFocusRef.current !== undefined) {
      // Delay to allow panel to render
      const timer = setTimeout(() => {
        if (panelRendererRef.current && pendingFocusRef.current !== undefined) {
          panelRendererRef.current.focusField(pendingFocusRef.current);
          pendingFocusRef.current = undefined;
        }
      }, 50);
      return () => clearTimeout(timer);
    }
  }, [selectedNode]);

  // Auto-apply panel changes with debounce
  useEffect(() => {
    // Clear any pending timer
    if (applyTimerRef.current) {
      clearTimeout(applyTimerRef.current);
    }

    // Only apply if we have values and a selected node
    if (selectedNode && Object.keys(panelValues).length > 0) {
      applyTimerRef.current = setTimeout(() => {
        applyPanelChanges();
        applyTimerRef.current = null;
      }, 500);
    }

    return () => {
      if (applyTimerRef.current) {
        clearTimeout(applyTimerRef.current);
      }
    };
  }, [panelValues, selectedNode, applyPanelChanges]);

  // Don't render if not in edit mode or no node is selected
  if (!isEditMode || !selectedNode) {
    return null;
  }

  // Check if selected node is root (can't be deleted)
  const isRoot = isRootNode(selectedNode);

  const handleDelete = () => {
    if (!isRoot) {
      deleteNode(selectedNode.id);
    }
  };

  return (
    <div className="properties-panel" style={{ width }}>
      <SelectedNodePanel
        ref={panelRendererRef}
        node={selectedNode}
        values={panelValues}
        onChange={updatePanelValue}
        onDeselect={() => selectNode(null)}
        onDelete={handleDelete}
        canDelete={!isRoot}
      />
    </div>
  );
});

interface SelectedNodePanelProps {
  node: NonNullable<ReturnType<typeof useEditorContext>['selectedNode']>;
  values: Record<string, unknown>;
  onChange: (fieldId: string, value: unknown) => void;
  onDeselect: () => void;
  onDelete: () => void;
  canDelete: boolean;
}

const SelectedNodePanel = memo(forwardRef<PanelRendererRef, SelectedNodePanelProps>(function SelectedNodePanel({
  node,
  values,
  onChange,
  onDeselect,
  onDelete,
  canDelete,
}, ref) {
  const [helpExpanded, setHelpExpanded] = useState(false);
  const panelRendererRef = useRef<PanelRendererRef>(null);
  const panelConfig = getPanelConfigForNode(node.data);
  const operatorConfig = getOperatorConfigForNode(node.data);
  const label = getNodeDisplayLabel(node.data);
  const category = getNodeCategory(node.data);

  // Forward ref to PanelRenderer
  useImperativeHandle(ref, () => ({
    focusField: (fieldId?: string) => {
      panelRendererRef.current?.focusField(fieldId);
    },
  }), []);

  return (
    <div className="properties-panel-content">
      {/* Header */}
      <div className="properties-panel-header">
        <div className="properties-panel-header-info">
          <h3 className="properties-panel-title">{label}</h3>
          {category && (
            <span className="properties-panel-category">{category}</span>
          )}
        </div>
        <button
          className="properties-panel-close"
          onClick={onDeselect}
          title="Deselect"
          type="button"
        >
          <X size={16} />
        </button>
      </div>

      {/* Arguments Section - for operator nodes */}
      {node.data.type === 'operator' && (
        <ArgumentsSection node={node} />
      )}

      {/* Properties Section - only for literals */}
      {panelConfig && node.data.type === 'literal' && (
        <div className="properties-panel-section">
          <div className="properties-panel-section-header">
            <span>Properties</span>
          </div>
          <PanelRenderer
            ref={panelRendererRef}
            config={panelConfig}
            values={values}
            onChange={onChange}
          />
        </div>
      )}

      {/* Help Section - collapsible, collapsed by default */}
      {operatorConfig && (
        <div className="properties-panel-section properties-panel-section--collapsible">
          <button
            className="properties-panel-section-toggle"
            onClick={() => setHelpExpanded(!helpExpanded)}
            type="button"
          >
            {helpExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
            <span>Help</span>
          </button>
          {helpExpanded && (
            <div className="properties-panel-section-content">
              <HelpSection
                help={operatorConfig.help}
                arity={operatorConfig.arity}
              />
            </div>
          )}
        </div>
      )}

      {/* Delete Node */}
      <div className="properties-panel-footer">
        <button
          className="properties-panel-delete"
          type="button"
          title={canDelete ? 'Delete this node and its children' : 'Cannot delete root node'}
          onClick={onDelete}
          disabled={!canDelete}
        >
          <Trash2 size={14} />
          Delete Node
        </button>
      </div>
    </div>
  );
}));
