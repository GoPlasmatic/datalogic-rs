/**
 * Literal Panel Configuration
 *
 * Panel configuration for literal (non-operator) nodes: strings, numbers,
 * booleans, null and arrays.
 *
 * Literal nodes cannot hold an object (LiteralNodeData.valueType has no
 * 'object' variant; objects are structure nodes), so the literal panel does
 * not offer that type. `structurePanelConfig` is the variant for structure
 * nodes, which are seeded with valueType 'object' and a template mode.
 */

import type { PanelConfig, PanelSection, SelectOption } from './operators.types';

const LITERAL_TYPE_OPTIONS: SelectOption[] = [
  { value: 'string', label: 'String', description: 'Text value' },
  { value: 'number', label: 'Number', description: 'Numeric value' },
  { value: 'boolean', label: 'Boolean', description: 'True or false' },
  { value: 'null', label: 'Null', description: 'Null value' },
  { value: 'array', label: 'Array', description: 'List of values' },
];

const OBJECT_TYPE_OPTION: SelectOption = {
  value: 'object',
  label: 'Object',
  description: 'Key-value pairs',
};

function typeSection(options: SelectOption[]): PanelSection {
  return {
    id: 'type',
    fields: [
      {
        id: 'valueType',
        label: 'Type',
        inputType: 'select',
        required: true,
        options,
      },
    ],
  };
}

const VALUE_SECTIONS: PanelSection[] = [
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
];

const OBJECT_VALUE_SECTION: PanelSection = {
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
};

/**
 * Panel configuration for editing literal values
 */
export const literalPanelConfig: PanelConfig = {
  sections: [typeSection(LITERAL_TYPE_OPTIONS), ...VALUE_SECTIONS],
};

/**
 * Panel configuration for structure (array / object) nodes
 */
export const structurePanelConfig: PanelConfig = {
  sections: [
    typeSection([...LITERAL_TYPE_OPTIONS, OBJECT_TYPE_OPTION]),
    ...VALUE_SECTIONS,
    OBJECT_VALUE_SECTION,
  ],
};
