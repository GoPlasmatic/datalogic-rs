import { memo, forwardRef, useImperativeHandle, useRef, useCallback } from 'react';
import type { PanelConfig, ContextVariable } from '../config/operators.types';
import { SectionRenderer, type SectionRendererRef } from './SectionRenderer';
import './panel-inputs.css';

interface PanelRendererProps {
  config: PanelConfig;
  values: Record<string, unknown>;
  onChange: (fieldId: string, value: unknown) => void;
  disabled?: boolean;
}

export interface PanelRendererRef {
  focusField: (fieldId?: string) => void;
}

/**
 * Renders a complete panel configuration.
 * This is the main entry point for rendering operator/literal panels.
 *
 * Features:
 * - Renders all sections from the config
 * - Displays context variables hint for iterator operators
 * - Displays chainable hint for comparison operators
 */
export const PanelRenderer = memo(forwardRef<PanelRendererRef, PanelRendererProps>(function PanelRenderer({
  config,
  values,
  onChange,
  disabled = false,
}, ref) {
  // Store refs to all sections
  const sectionRefs = useRef<Map<string, SectionRendererRef>>(new Map());

  const setSectionRef = useCallback((sectionId: string, sectionRef: SectionRendererRef | null) => {
    if (sectionRef) {
      sectionRefs.current.set(sectionId, sectionRef);
    } else {
      sectionRefs.current.delete(sectionId);
    }
  }, []);

  // Expose focusField method
  useImperativeHandle(ref, () => ({
    focusField: (fieldId?: string) => {
      // If no fieldId, focus the first field in the first section
      if (!fieldId) {
        const firstSection = config.sections[0];
        if (firstSection) {
          const sectionRef = sectionRefs.current.get(firstSection.id);
          sectionRef?.focusFirstField();
        }
        return;
      }

      // Find which section contains this field and focus it
      for (const section of config.sections) {
        const hasField = section.fields.some((f) => f.id === fieldId);
        if (hasField) {
          const sectionRef = sectionRefs.current.get(section.id);
          sectionRef?.focusField(fieldId);
          return;
        }
      }
    },
  }), [config.sections]);

  return (
    <div className="panel-renderer">
      {/* Render all sections */}
      {config.sections.map((section) => (
        <SectionRenderer
          key={section.id}
          ref={(r) => setSectionRef(section.id, r)}
          section={section}
          values={values}
          onChange={onChange}
          disabled={disabled}
        />
      ))}

      {/* Chainable hint for comparison operators */}
      {config.chainable && <ChainableHint />}

      {/* Context variables for iterator operators */}
      {config.contextVariables && config.contextVariables.length > 0 && (
        <ContextVariablesHint variables={config.contextVariables} />
      )}
    </div>
  );
}));

/**
 * Hint displayed for chainable comparison operators
 */
const ChainableHint = memo(function ChainableHint() {
  return (
    <div className="panel-hint panel-hint-chainable">
      <div className="panel-hint-icon">⛓</div>
      <div className="panel-hint-content">
        <div className="panel-hint-title">Chainable Comparison</div>
        <div className="panel-hint-description">
          Add more values to create chained comparisons like <code>a &lt; b &lt; c</code>
        </div>
      </div>
    </div>
  );
});

interface ContextVariablesHintProps {
  variables: ContextVariable[];
}

/**
 * Hint displayed for iterator operators showing available context variables
 */
const ContextVariablesHint = memo(function ContextVariablesHint({
  variables,
}: ContextVariablesHintProps) {
  return (
    <div className="panel-hint panel-hint-context">
      <div className="panel-hint-icon">💡</div>
      <div className="panel-hint-content">
        <div className="panel-hint-title">Inside expression, use:</div>
        <ul className="panel-context-list">
          {variables.map((variable) => (
            <li key={variable.name || '_current'} className="panel-context-item">
              <code className="panel-context-example">{variable.example}</code>
              <span className="panel-context-label">{variable.label}</span>
              <span className="panel-context-description">{variable.description}</span>
            </li>
          ))}
        </ul>
      </div>
    </div>
  );
});
