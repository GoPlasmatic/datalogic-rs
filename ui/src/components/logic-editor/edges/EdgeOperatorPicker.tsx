/**
 * Edge Operator Picker Component
 *
 * A popup that appears when clicking the [+] button on an edge,
 * allowing users to select an operator to insert.
 */

import { memo, useState, useCallback, useEffect, useMemo, useRef } from 'react';
import { Search, X, Variable, Hash } from 'lucide-react';
import { useEditorContext } from '../context/editor';
import { edgeInsertDiscardsSubtree } from '../context/editor/useNodeEdgeInsert';
import {
  getOperatorsGroupedByCategory,
  searchOperators,
} from '../config/operators';
import { categories } from '../config/categories';
import type { Operator, OperatorCategory } from '../config/operators.types';

interface EdgeInfo {
  edgeId: string;
  sourceId: string;
  targetId: string;
}

interface EdgeOperatorPickerProps {
  edgeInfo: EdgeInfo;
  onClose: () => void;
}

export const EdgeOperatorPicker = memo(function EdgeOperatorPicker({
  edgeInfo,
  onClose,
}: EdgeOperatorPickerProps) {
  const { insertNodeOnEdge, nodes } = useEditorContext();
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedCategory, setSelectedCategory] = useState<OperatorCategory | null>(null);
  const pickerRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  // Focus search input on mount
  useEffect(() => {
    inputRef.current?.focus();
  }, []);

  // Close on click outside
  useEffect(() => {
    const handleClickOutside = (e: MouseEvent) => {
      if (pickerRef.current && !pickerRef.current.contains(e.target as Node)) {
        onClose();
      }
    };

    const handleEscape = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleEscape);
    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [onClose]);

  // A var / literal cannot hold the edge's existing child, so quick-add
  // replaces that subtree rather than wrapping it the way an operator does.
  // Say so on the buttons instead of dropping the work silently.
  const quickAddReplaces = useMemo(
    () => edgeInsertDiscardsSubtree(nodes, edgeInfo.sourceId, edgeInfo.targetId),
    [nodes, edgeInfo.sourceId, edgeInfo.targetId]
  );

  // Get operators to display
  const operatorsToShow = searchQuery
    ? searchOperators(searchQuery)
    : selectedCategory
      ? Array.from(getOperatorsGroupedByCategory().get(selectedCategory) || [])
      : [];

  // Get all categories with operators
  const categoriesWithOperators = Array.from(getOperatorsGroupedByCategory().entries())
    .filter(([, ops]) => ops.length > 0)
    .map(([cat]) => cat);

  const handleOperatorSelect = useCallback(
    (operator: Operator) => {
      insertNodeOnEdge(
        edgeInfo.sourceId,
        edgeInfo.targetId,
        operator.name
      );
      onClose();
    },
    [edgeInfo, insertNodeOnEdge, onClose]
  );

  const handleQuickAdd = useCallback(
    (type: 'variable' | 'literal') => {
      // For variable/literal, we use a pseudo-operator name
      insertNodeOnEdge(
        edgeInfo.sourceId,
        edgeInfo.targetId,
        type === 'variable' ? '__variable__' : '__literal__'
      );
      onClose();
    },
    [edgeInfo, insertNodeOnEdge, onClose]
  );

  return (
    <div
      ref={pickerRef}
      className="dl-edge-operator-picker"
      onClick={(e) => e.stopPropagation()}
    >
      {/* Header */}
      <div className="dl-edge-picker-header">
        <span className="dl-edge-picker-title">Insert Node</span>
        <button
          type="button"
          className="dl-edge-picker-close"
          onClick={onClose}
          title="Close"
        >
          <X size={14} />
        </button>
      </div>

      {/* Search */}
      <div className="dl-edge-picker-search">
        <Search size={14} className="dl-edge-picker-search-icon" />
        <input
          ref={inputRef}
          type="text"
          className="dl-edge-picker-search-input"
          placeholder="Search operators..."
          value={searchQuery}
          onChange={(e) => {
            setSearchQuery(e.target.value);
            setSelectedCategory(null);
          }}
        />
      </div>

      {/* Quick Add */}
      <div className="dl-edge-picker-quick">
        <button
          type="button"
          className="dl-edge-picker-quick-btn"
          onClick={() => handleQuickAdd('variable')}
          title={quickAddReplaces ? 'Replace this branch with a variable' : 'Insert variable'}
        >
          <Variable size={14} />
          <span>{quickAddReplaces ? 'Replace with Variable' : 'Variable'}</span>
        </button>
        <button
          type="button"
          className="dl-edge-picker-quick-btn"
          onClick={() => handleQuickAdd('literal')}
          title={quickAddReplaces ? 'Replace this branch with a literal' : 'Insert literal'}
        >
          <Hash size={14} />
          <span>{quickAddReplaces ? 'Replace with Literal' : 'Literal'}</span>
        </button>
      </div>
      {quickAddReplaces && (
        <div className="dl-edge-picker-note">
          Replaces the branch below this edge. Operators wrap it instead.
        </div>
      )}

      {/* Categories or Search Results */}
      <div className="dl-edge-picker-content">
        {!searchQuery && !selectedCategory ? (
          // Show categories
          <div className="dl-edge-picker-categories">
            {categoriesWithOperators.map((cat) => {
              const catMeta = categories[cat];
              return (
                <button
                  key={cat}
                  type="button"
                  className="dl-edge-picker-category"
                  onClick={() => setSelectedCategory(cat)}
                  style={{ borderLeftColor: catMeta?.color || '#888' }}
                >
                  <span className="dl-edge-picker-category-name">
                    {catMeta?.label || cat}
                  </span>
                  <span className="dl-edge-picker-category-count">
                    {getOperatorsGroupedByCategory().get(cat)?.length || 0}
                  </span>
                </button>
              );
            })}
          </div>
        ) : (
          // Show operators
          <div className="dl-edge-picker-operators">
            {selectedCategory && !searchQuery && (
              <button
                type="button"
                className="dl-edge-picker-back"
                onClick={() => setSelectedCategory(null)}
              >
                ← Back to categories
              </button>
            )}
            {operatorsToShow.length > 0 ? (
              operatorsToShow.map((op) => (
                <button
                  key={op.name}
                  type="button"
                  className="dl-edge-picker-operator"
                  onClick={() => handleOperatorSelect(op)}
                  title={op.help.summary}
                >
                  <span className="dl-edge-picker-operator-name">{op.name}</span>
                  <span className="dl-edge-picker-operator-label">{op.label}</span>
                </button>
              ))
            ) : (
              <div className="dl-edge-picker-empty">
                {searchQuery ? 'No operators found' : 'No operators in this category'}
              </div>
            )}
          </div>
        )}
      </div>
    </div>
  );
});

export default EdgeOperatorPicker;
