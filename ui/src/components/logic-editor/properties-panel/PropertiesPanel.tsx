/**
 * PropertiesPanel Component
 *
 * The main properties panel that displays context-aware properties
 * for the currently selected node.
 */

import { memo, useEffect, useState, useRef, useImperativeHandle, forwardRef, useCallback, useMemo } from 'react';
import { X, ChevronDown, ChevronRight, Trash2, Search } from 'lucide-react';
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
import { getOperatorsGroupedByCategory } from '../config/operators';
import { categories } from '../config/categories';
import type { OperatorCategory } from '../config/operators.types';

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
    createNode,
    hasNodes,
    propertyPanelFocusRef,
  } = useEditorContext();

  const isCanvasEmpty = !hasNodes();

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

  // Don't render if not in edit mode
  if (!isEditMode) {
    return null;
  }

  // Check if selected node is root (can't be deleted)
  const isRoot = selectedNode ? isRootNode(selectedNode) : false;

  const handleDelete = () => {
    if (selectedNode && !isRoot) {
      deleteNode(selectedNode.id);
    }
  };

  return (
    <div className="properties-panel" style={{ width }}>
      {selectedNode ? (
        <SelectedNodePanel
          ref={panelRendererRef}
          node={selectedNode}
          values={panelValues}
          onChange={updatePanelValue}
          onDeselect={() => selectNode(null)}
          onDelete={handleDelete}
          canDelete={!isRoot}
        />
      ) : (
        <EmptyStatePanel onAddNode={createNode} isCanvasEmpty={isCanvasEmpty} />
      )}
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

      {/* Arguments Section - for operator nodes (shown first) */}
      {(node.data.type === 'operator' || node.data.type === 'verticalCell') && (
        <ArgumentsSection node={node} />
      )}

      {/* Properties Section - only for literals and variable operators */}
      {panelConfig && (node.data.type === 'literal' || node.data.type === 'variable') && (
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
                seeAlso={operatorConfig.help.seeAlso}
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

interface EmptyStatePanelProps {
  onAddNode: (type: 'variable' | 'operator' | 'literal' | 'condition', operatorName?: string) => void;
  isCanvasEmpty: boolean;
}

// Category display order
const CATEGORY_ORDER: OperatorCategory[] = [
  'variable',
  'comparison',
  'logical',
  'arithmetic',
  'control',
  'string',
  'array',
  'datetime',
  'validation',
  'error',
  'utility',
];

const EmptyStatePanel = memo(function EmptyStatePanel({
  onAddNode,
  isCanvasEmpty,
}: EmptyStatePanelProps) {
  const [searchQuery, setSearchQuery] = useState('');
  const [expandedCategories, setExpandedCategories] = useState<Set<OperatorCategory>>(
    new Set(['variable', 'comparison', 'arithmetic', 'control'])
  );

  // Get operators grouped by category
  const operatorsByCategory = useMemo(() => getOperatorsGroupedByCategory(), []);

  // Filter operators based on search query
  const filteredOperatorsByCategory = useMemo(() => {
    if (!searchQuery.trim()) return operatorsByCategory;

    const lowerQuery = searchQuery.toLowerCase();
    const filtered = new Map<OperatorCategory, typeof operatorsByCategory extends Map<OperatorCategory, infer V> ? V : never>();

    for (const [category, ops] of operatorsByCategory) {
      const matchingOps = ops.filter(
        (op) =>
          op.name.toLowerCase().includes(lowerQuery) ||
          op.label.toLowerCase().includes(lowerQuery) ||
          op.description.toLowerCase().includes(lowerQuery)
      );
      if (matchingOps.length > 0) {
        filtered.set(category, matchingOps);
      }
    }

    return filtered;
  }, [operatorsByCategory, searchQuery]);

  const toggleCategory = useCallback((category: OperatorCategory) => {
    setExpandedCategories((prev) => {
      const next = new Set(prev);
      if (next.has(category)) {
        next.delete(category);
      } else {
        next.add(category);
      }
      return next;
    });
  }, []);

  const handleOperatorClick = useCallback(
    (operatorName: string, category: OperatorCategory) => {
      if (category === 'variable') {
        onAddNode('variable');
      } else {
        onAddNode('operator', operatorName);
      }
    },
    [onAddNode]
  );

  // When canvas has nodes but nothing selected - just show hint
  if (!isCanvasEmpty) {
    return (
      <div className="properties-panel-content">
        <div className="properties-panel-empty">
          <p className="properties-panel-empty-title">No node selected</p>
          <p className="properties-panel-empty-hint">
            Click a node to view and edit its properties.
          </p>
        </div>
      </div>
    );
  }

  // When canvas is empty - show full operator list
  return (
    <div className="properties-panel-content">
      <div className="properties-panel-header">
        <h3 className="properties-panel-title">Start with an Operator</h3>
      </div>

      {/* Search Input */}
      <div className="properties-panel-search">
        <Search size={14} className="properties-panel-search-icon" />
        <input
          type="text"
          className="properties-panel-search-input"
          placeholder="Search operators..."
          value={searchQuery}
          onChange={(e) => setSearchQuery(e.target.value)}
        />
      </div>

      {/* Operator Categories */}
      <div className="properties-panel-operators-list">
        {CATEGORY_ORDER.map((category) => {
          const ops = filteredOperatorsByCategory.get(category);
          if (!ops || ops.length === 0) return null;

          const categoryMeta = categories[category];
          const isExpanded = expandedCategories.has(category) || searchQuery.trim() !== '';

          return (
            <div key={category} className="properties-panel-category">
              <button
                className="properties-panel-category-header"
                onClick={() => toggleCategory(category)}
                type="button"
              >
                {isExpanded ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
                <span
                  className="properties-panel-category-dot"
                  style={{ backgroundColor: categoryMeta.color }}
                />
                <span className="properties-panel-category-label">
                  {categoryMeta.label}
                </span>
                <span className="properties-panel-category-count">{ops.length}</span>
              </button>

              {isExpanded && (
                <div className="properties-panel-category-items">
                  {ops.map((op) => (
                    <button
                      key={op.name}
                      className="properties-panel-operator-item"
                      onClick={() => handleOperatorClick(op.name, category)}
                      type="button"
                      title={op.description}
                    >
                      <span className="properties-panel-operator-name">{op.label}</span>
                      <span className="properties-panel-operator-symbol">{op.name}</span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
});
