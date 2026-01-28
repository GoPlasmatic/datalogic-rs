/**
 * Literal Panel Configuration
 *
 * Panel configuration for literal (non-operator) nodes like strings, numbers,
 * booleans, null, arrays, and objects.
 */

import type { PanelConfig } from './operators.types';

/**
 * Panel configuration for editing literal values
 */
export const literalPanelConfig: PanelConfig = {
  sections: [
    {
      id: 'type',
      fields: [
        {
          id: 'valueType',
          label: 'Type',
          inputType: 'select',
          required: true,
          options: [
            { value: 'string', label: 'String', description: 'Text value' },
            { value: 'number', label: 'Number', description: 'Numeric value' },
            { value: 'boolean', label: 'Boolean', description: 'True or false' },
            { value: 'null', label: 'Null', description: 'Null value' },
            { value: 'array', label: 'Array', description: 'List of values' },
            { value: 'object', label: 'Object', description: 'Key-value pairs' },
          ],
        },
      ],
    },
    {
      id: 'stringValue',
      showWhen: [{ field: 'valueType', operator: 'equals', value: 'string' }],
      fields: [
        {
          id: 'value',
          label: 'Value',
          inputType: 'textarea',
          placeholder: 'Enter text...',
        },
      ],
    },
    {
      id: 'numberValue',
      showWhen: [{ field: 'valueType', operator: 'equals', value: 'number' }],
      fields: [
        {
          id: 'value',
          label: 'Value',
          inputType: 'number',
          placeholder: '0',
        },
      ],
    },
    {
      id: 'booleanValue',
      showWhen: [{ field: 'valueType', operator: 'equals', value: 'boolean' }],
      fields: [
        {
          id: 'value',
          label: 'Value',
          inputType: 'boolean',
          defaultValue: false,
        },
      ],
    },
    {
      id: 'arrayValue',
      showWhen: [{ field: 'valueType', operator: 'equals', value: 'array' }],
      fields: [
        {
          id: 'elements',
          label: 'Elements',
          inputType: 'expression',
          repeatable: true,
          helpText: 'Array elements (can be literals or expressions)',
        },
      ],
    },
    {
      id: 'objectValue',
      showWhen: [{ field: 'valueType', operator: 'equals', value: 'object' }],
      fields: [
        {
          id: 'mode',
          label: 'Mode',
          inputType: 'select',
          defaultValue: 'pure',
          options: [
            {
              value: 'pure',
              label: 'Pure',
              description: 'All values are literals',
            },
            {
              value: 'template',
              label: 'Template',
              description: 'Values can contain expressions',
            },
          ],
        },
      ],
    },
  ],
};
