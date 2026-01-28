import { memo, forwardRef, useImperativeHandle } from 'react';
import type { PanelField, SelectOption } from '../config/operators.types';
import { TextInput } from './TextInput';
import { TextAreaInput } from './TextAreaInput';
import { NumberInput } from './NumberInput';
import { BooleanInput } from './BooleanInput';
import { SelectInput } from './SelectInput';
import { PathInput } from './PathInput';
import { PathArrayInput } from './PathArrayInput';
import { ExpressionInput } from './ExpressionInput';
import { JsonInput } from './JsonInput';

interface FieldRendererProps {
  field: PanelField;
  value: unknown;
  onChange: (value: unknown) => void;
  disabled?: boolean;
}

export interface FieldRendererRef {
  focus: () => void;
}

/**
 * Renders the appropriate input component based on the field's inputType.
 * This is the bridge between the declarative PanelField config and the actual input components.
 */
export const FieldRenderer = memo(forwardRef<FieldRendererRef, FieldRendererProps>(function FieldRenderer({
  field,
  value,
  onChange,
  disabled = false,
}, ref) {
  const fieldId = `panel-field-${field.id}`;
  const isDisabled = disabled;

  // Expose focus method
  useImperativeHandle(ref, () => ({
    focus: () => {
      // Try to find and focus the input element
      const input = document.getElementById(fieldId) as HTMLInputElement | HTMLTextAreaElement | HTMLSelectElement | null;
      input?.focus();
      if (input && 'select' in input) {
        input.select();
      }
    },
  }), [fieldId]);

  const renderInput = () => {
    switch (field.inputType) {
      case 'text':
        return (
          <TextInput
            id={fieldId}
            value={(value as string) ?? ''}
            onChange={onChange}
            placeholder={field.placeholder}
            disabled={isDisabled}
            required={field.required}
          />
        );

      case 'textarea':
        return (
          <TextAreaInput
            id={fieldId}
            value={(value as string) ?? ''}
            onChange={onChange}
            placeholder={field.placeholder}
            disabled={isDisabled}
            required={field.required}
          />
        );

      case 'number':
        return (
          <NumberInput
            id={fieldId}
            value={value as number | undefined}
            onChange={onChange}
            placeholder={field.placeholder}
            disabled={isDisabled}
            required={field.required}
            min={field.min}
            max={field.max}
          />
        );

      case 'boolean':
        return (
          <BooleanInput
            id={fieldId}
            value={(value as boolean) ?? (field.defaultValue as boolean) ?? false}
            onChange={onChange}
            disabled={isDisabled}
          />
        );

      case 'select': {
        const selectValue = (value ?? field.defaultValue ?? field.options?.[0]?.value ?? '') as string | number | boolean;
        return (
          <SelectInput
            id={fieldId}
            value={selectValue}
            onChange={onChange}
            options={field.options as SelectOption[]}
            disabled={isDisabled}
            required={field.required}
          />
        );
      }

      case 'path':
        return (
          <PathInput
            id={fieldId}
            value={(value as string) ?? ''}
            onChange={onChange}
            placeholder={field.placeholder}
            disabled={isDisabled}
            required={field.required}
          />
        );

      case 'pathArray':
        return (
          <PathArrayInput
            id={fieldId}
            value={(value as string[]) ?? []}
            onChange={onChange}
            disabled={isDisabled}
            required={field.required}
          />
        );

      case 'expression':
        return (
          <ExpressionInput
            id={fieldId}
            value={value}
            onChange={onChange}
            disabled={isDisabled}
            required={field.required}
            placeholder={field.placeholder}
          />
        );

      case 'json':
        return (
          <JsonInput
            id={fieldId}
            value={value}
            onChange={onChange}
            disabled={isDisabled}
            required={field.required}
          />
        );

      default:
        return (
          <div className="panel-input-unsupported">
            Unsupported input type: {field.inputType}
          </div>
        );
    }
  };

  return (
    <div className="panel-field">
      <label htmlFor={fieldId} className="panel-field-label">
        {field.label}
        {field.required && <span className="panel-field-required">*</span>}
      </label>
      {renderInput()}
      {field.helpText && (
        <span className="panel-field-help">{field.helpText}</span>
      )}
    </div>
  );
}));
