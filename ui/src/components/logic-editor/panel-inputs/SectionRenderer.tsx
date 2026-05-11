import { memo, useState, useCallback, forwardRef, useImperativeHandle, useRef } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import type { PanelSection } from '../config/operators.types';
import { FieldRenderer, type FieldRendererRef } from './FieldRenderer';
import { evaluateConditions } from './visibility';

interface SectionRendererProps {
  section: PanelSection;
  values: Record<string, unknown>;
  onChange: (fieldId: string, value: unknown) => void;
  disabled?: boolean;
}

export interface SectionRendererRef {
  focusField: (fieldId: string) => void;
  focusFirstField: () => void;
}

/**
 * Renders a panel section with its title and fields.
 * Handles:
 * - Section-level visibility (showWhen)
 * - Field-level visibility (showWhen)
 * - Collapsible sections (defaultCollapsed)
 */
export const SectionRenderer = memo(forwardRef<SectionRendererRef, SectionRendererProps>(function SectionRenderer({
  section,
  values,
  onChange,
  disabled = false,
}, ref) {
  const [isCollapsed, setIsCollapsed] = useState(section.defaultCollapsed ?? false);
  const fieldRefs = useRef<Map<string, FieldRendererRef>>(new Map());

  const setFieldRef = useCallback((fieldId: string, fieldRef: FieldRendererRef | null) => {
    if (fieldRef) {
      fieldRefs.current.set(fieldId, fieldRef);
    } else {
      fieldRefs.current.delete(fieldId);
    }
  }, []);

  // Expose focus methods
  useImperativeHandle(ref, () => ({
    focusField: (fieldId: string) => {
      // Expand section if collapsed
      setIsCollapsed(false);
      // Focus the field after a short delay to allow render
      setTimeout(() => {
        fieldRefs.current.get(fieldId)?.focus();
      }, 50);
    },
    focusFirstField: () => {
      // Expand section if collapsed
      setIsCollapsed(false);
      // Focus the first field
      setTimeout(() => {
        const firstFieldId = section.fields[0]?.id;
        if (firstFieldId) {
          fieldRefs.current.get(firstFieldId)?.focus();
        }
      }, 50);
    },
  }), [section.fields]);

  const handleToggle = useCallback(() => {
    setIsCollapsed((prev) => !prev);
  }, []);

  // Check section-level visibility
  const isSectionVisible = evaluateConditions(section.showWhen, values);
  if (!isSectionVisible) {
    return null;
  }

  // Filter visible fields
  const visibleFields = section.fields.filter((field) =>
    evaluateConditions(field.showWhen, values)
  );

  // Don't render empty sections
  if (visibleFields.length === 0) {
    return null;
  }

  const hasTitle = section.title !== undefined;
  const isCollapsible = hasTitle && section.defaultCollapsed !== undefined;

  return (
    <div className="panel-section">
      {hasTitle && (
        <div
          className={`panel-section-header ${isCollapsible ? 'collapsible' : ''}`}
          onClick={isCollapsible ? handleToggle : undefined}
          role={isCollapsible ? 'button' : undefined}
          tabIndex={isCollapsible ? 0 : undefined}
          onKeyDown={
            isCollapsible
              ? (e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    handleToggle();
                  }
                }
              : undefined
          }
        >
          {isCollapsible && (
            <span className="panel-section-toggle">
              {isCollapsed ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
            </span>
          )}
          <span className="panel-section-title">{section.title}</span>
        </div>
      )}

      {!isCollapsed && (
        <div className="panel-section-content">
          {visibleFields.map((field) => (
            <FieldRenderer
              key={field.id}
              ref={(r) => setFieldRef(field.id, r)}
              field={field}
              value={values[field.id]}
              onChange={(value) => onChange(field.id, value)}
              disabled={disabled}
            />
          ))}
        </div>
      )}
    </div>
  );
}));
