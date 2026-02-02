import { memo, useMemo } from 'react';
import { Handle, Position } from '@xyflow/react';
import type { StructureNodeData } from '../types';
import { useDebugClassName, useNodeCollapse } from '../hooks';
import { NodeInputHandles, CollapseToggleButton, NodeDebugBubble } from './shared';
import { Icon } from '../utils/icons';
import { ExpressionSyntax } from '../utils/ExpressionSyntax';

// Color for structure nodes (gray like literal, but slightly different)
const STRUCTURE_COLOR = '#64748B';

// Height of each line in the JSON body (in pixels)
const LINE_HEIGHT = 18;
// Header height
const HEADER_HEIGHT = 32;
// Body padding
const BODY_PADDING = 8;

interface StructureNodeProps {
  id: string;
  data: StructureNodeData;
  selected?: boolean;
}

export const StructureNode = memo(function StructureNode({
  id,
  data,
  selected,
}: StructureNodeProps) {
  const debugClassName = useDebugClassName(id);
  const toggleNodeCollapse = useNodeCollapse(id);

  const isCollapsed = data.collapsed ?? false;
  const expressionElements = useMemo(
    () => data.elements.filter(e => e.type === 'expression'),
    [data.elements]
  );
  const hasExpressions = expressionElements.length > 0;

  // Calculate line numbers for each expression based on offset in formatted JSON
  const expressionLineNumbers = useMemo(() => {
    return expressionElements.map(element => {
      const textBefore = data.formattedJson.slice(0, element.startOffset);
      const lineNumber = textBefore.split('\n').length;
      return lineNumber;
    });
  }, [expressionElements, data.formattedJson]);

  return (
    <div
      className={`dl-node structure-node ${selected ? 'selected' : ''} ${isCollapsed ? 'collapsed' : ''} ${debugClassName}`}
      style={{
        borderColor: STRUCTURE_COLOR,
        backgroundColor: `${STRUCTURE_COLOR}10`,
      }}
    >
      <NodeDebugBubble nodeId={id} position="top" />
      <NodeInputHandles nodeId={id} color={STRUCTURE_COLOR} />

      {/* Header with icon, label, and collapse toggle */}
      <div className="structure-node-header" style={{ backgroundColor: STRUCTURE_COLOR }}>
        <span className="structure-node-icon">
          <Icon name={data.isArray ? 'list' : 'braces'} size={14} />
        </span>
        <span className="structure-node-label">
          {data.isArray ? 'Array' : 'Object'}
        </span>
        {hasExpressions && (
          <CollapseToggleButton isCollapsed={isCollapsed} onClick={toggleNodeCollapse} />
        )}
      </div>

      {/* Body: either expression text (collapsed) or formatted JSON (expanded) */}
      {isCollapsed ? (
        <div className="structure-node-body collapsed-body">
          <div className="expression-text">
            <ExpressionSyntax text={data.expressionText || '...'} />
          </div>
        </div>
      ) : (
        <div className="structure-node-body">
          <FormattedJson
            json={data.formattedJson}
            elements={data.elements}
          />
        </div>
      )}

      {/* Render handles at node level, positioned based on line numbers */}
      {!isCollapsed && expressionElements.map((_element, idx) => {
        const lineNumber = expressionLineNumbers[idx];
        // Calculate vertical position: header + padding + (lineNumber - 1) * lineHeight + half line height
        const topPosition = HEADER_HEIGHT + BODY_PADDING + (lineNumber - 1) * LINE_HEIGHT + LINE_HEIGHT / 2;

        return (
          <Handle
            key={`branch-${idx}`}
            type="source"
            position={Position.Right}
            id={`branch-${idx}`}
            className="structure-branch-handle"
            style={{
              background: '#3B82F6',
              top: `${topPosition}px`,
              right: '-4px',
            }}
          />
        );
      })}
    </div>
  );
});

// Formatted JSON with expression markers (visual only, no handles)
interface FormattedJsonProps {
  json: string;
  elements: StructureNodeData['elements'];
}

function FormattedJson({ json, elements }: FormattedJsonProps) {
  // Get expression elements sorted by offset
  const expressionElements = elements
    .filter(e => e.type === 'expression')
    .sort((a, b) => a.startOffset - b.startOffset);

  // If no expressions, just render syntax-highlighted JSON
  if (expressionElements.length === 0) {
    return <JsonSyntax text={json} />;
  }

  // Split JSON at expression positions and render with markers
  const parts: React.ReactNode[] = [];
  let lastEnd = 0;
  let markerIndex = 0;

  for (const element of expressionElements) {
    // Add text before this expression
    if (element.startOffset > lastEnd) {
      const textBefore = json.slice(lastEnd, element.startOffset);
      parts.push(
        <JsonSyntax key={`text-${lastEnd}`} text={textBefore} />
      );
    }

    // Add expression marker (visual indicator, no handle)
    parts.push(
      <span key={`marker-${markerIndex}`} className="structure-expression-marker">
        {element.key || `[${markerIndex}]`}
      </span>
    );

    lastEnd = element.endOffset;
    markerIndex++;
  }

  // Add remaining text after last expression
  if (lastEnd < json.length) {
    parts.push(
      <JsonSyntax key={`text-${lastEnd}`} text={json.slice(lastEnd)} />
    );
  }

  return <>{parts}</>;
}

// Simple JSON syntax highlighter
function JsonSyntax({ text }: { text: string }) {
  const highlighted = highlightJson(text);
  return <>{highlighted}</>;
}

// Tokenize and highlight JSON
function highlightJson(text: string): React.ReactNode[] {
  const result: React.ReactNode[] = [];
  let i = 0;

  while (i < text.length) {
    const char = text[i];

    // Whitespace
    if (/\s/.test(char)) {
      let whitespace = '';
      while (i < text.length && /\s/.test(text[i])) {
        whitespace += text[i];
        i++;
      }
      result.push(whitespace);
      continue;
    }

    // String (key or value)
    if (char === '"') {
      let str = '"';
      i++;
      while (i < text.length && text[i] !== '"') {
        if (text[i] === '\\' && i + 1 < text.length) {
          str += text[i] + text[i + 1];
          i += 2;
        } else {
          str += text[i];
          i++;
        }
      }
      if (i < text.length) {
        str += '"';
        i++;
      }

      // Check if it's a key (followed by colon)
      let j = i;
      while (j < text.length && /\s/.test(text[j])) j++;
      const isKey = text[j] === ':';

      result.push(
        <span key={`str-${i}`} className={isKey ? 'json-syntax-key' : 'json-syntax-string'}>
          {str}
        </span>
      );
      continue;
    }

    // Number
    if (/[-\d]/.test(char)) {
      let num = '';
      while (i < text.length && /[-\d.eE+]/.test(text[i])) {
        num += text[i];
        i++;
      }
      result.push(
        <span key={`num-${i}`} className="json-syntax-number">
          {num}
        </span>
      );
      continue;
    }

    // Boolean/null
    if (text.slice(i, i + 4) === 'true') {
      result.push(
        <span key={`bool-${i}`} className="json-syntax-boolean-true">
          true
        </span>
      );
      i += 4;
      continue;
    }
    if (text.slice(i, i + 5) === 'false') {
      result.push(
        <span key={`bool-${i}`} className="json-syntax-boolean-false">
          false
        </span>
      );
      i += 5;
      continue;
    }
    if (text.slice(i, i + 4) === 'null') {
      result.push(
        <span key={`null-${i}`} className="json-syntax-null">
          null
        </span>
      );
      i += 4;
      continue;
    }

    // Punctuation
    if (/[{}[\]:,]/.test(char)) {
      result.push(
        <span key={`punct-${i}`} className="json-syntax-punctuation">
          {char}
        </span>
      );
      i++;
      continue;
    }

    // Other characters (shouldn't happen in valid JSON)
    result.push(char);
    i++;
  }

  return result;
}
