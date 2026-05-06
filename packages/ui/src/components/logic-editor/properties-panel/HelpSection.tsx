/**
 * HelpSection Component
 *
 * Displays operator help information including summary, arity, return type,
 * and collapsible examples.
 */

import { memo, useState, useCallback } from 'react';
import { ChevronDown, ChevronRight, Copy, Check } from 'lucide-react';
import type { OperatorHelp, AritySpec, OperatorExample } from '../config/operators.types';

interface HelpSectionProps {
  help: OperatorHelp;
  arity: AritySpec;
}

/**
 * Formats arity for display
 */
function formatArity(arity: AritySpec): string {
  switch (arity.type) {
    case 'nullary':
      return 'Args: 0';
    case 'unary':
      return 'Args: 1';
    case 'binary':
      return 'Args: 2';
    case 'ternary':
      return 'Args: 3';
    case 'nary':
      return `Args: ${arity.min ?? 1}+`;
    case 'variadic':
      return `Args: ${arity.min ?? 2}+`;
    case 'chainable':
      return `Args: ${arity.min ?? 2}+ (chainable)`;
    case 'range':
      if (arity.min !== undefined && arity.max !== undefined) {
        return arity.min === arity.max
          ? `Args: ${arity.min}`
          : `Args: ${arity.min}-${arity.max}`;
      }
      return 'Args: varies';
    case 'special':
      return 'Args: special';
    default:
      return 'Args: varies';
  }
}

export const HelpSection = memo(function HelpSection({
  help,
  arity,
}: HelpSectionProps) {
  const [showExamples, setShowExamples] = useState(false);

  const toggleExamples = useCallback(() => {
    setShowExamples((prev) => !prev);
  }, []);

  return (
    <div className="help-section">
      {/* Summary */}
      <p className="help-summary">{help.summary}</p>

      {/* Badges */}
      <div className="help-badges">
        <span className="help-badge help-badge-arity">{formatArity(arity)}</span>
        <span className="help-badge help-badge-return">
          Returns: <code>{help.returnType}</code>
        </span>
      </div>

      {/* Examples toggle */}
      {help.examples.length > 0 && (
        <button
          className="help-examples-toggle"
          onClick={toggleExamples}
          type="button"
        >
          {showExamples ? <ChevronDown size={14} /> : <ChevronRight size={14} />}
          <span>{showExamples ? 'Hide' : 'Show'} Examples ({help.examples.length})</span>
        </button>
      )}

      {/* Examples */}
      {showExamples && (
        <div className="help-examples">
          {help.examples.map((example, index) => (
            <ExampleCard key={index} example={example} />
          ))}
        </div>
      )}

      {/* Notes */}
      {help.notes && help.notes.length > 0 && (
        <div className="help-notes">
          <div className="help-notes-title">Notes</div>
          <ul className="help-notes-list">
            {help.notes.map((note, index) => (
              <li key={index}>{note}</li>
            ))}
          </ul>
        </div>
      )}

    </div>
  );
});

interface ExampleCardProps {
  example: OperatorExample;
}

const ExampleCard = memo(function ExampleCard({ example }: ExampleCardProps) {
  const [copied, setCopied] = useState(false);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(JSON.stringify(example.rule, null, 2));
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Clipboard API not available
    }
  }, [example.rule]);

  return (
    <div className="help-example">
      <div className="help-example-header">
        <span className="help-example-title">{example.title}</span>
        <button
          className="help-example-copy"
          onClick={handleCopy}
          title="Copy rule"
          type="button"
        >
          {copied ? <Check size={12} /> : <Copy size={12} />}
        </button>
      </div>
      <pre className="help-example-code">
        <code>{JSON.stringify(example.rule, null, 2)}</code>
      </pre>
      {example.data !== undefined && (
        <div className="help-example-data">
          <span className="help-example-label">Data:</span>
          <code>{JSON.stringify(example.data)}</code>
        </div>
      )}
      {example.result !== undefined && (
        <div className="help-example-result">
          <span className="help-example-label">→</span>
          <code>{JSON.stringify(example.result)}</code>
        </div>
      )}
      {example.error && (
        <div className="help-example-error">
          <span className="help-example-label">Error:</span>
          <code>{example.error.type}</code>
        </div>
      )}
      {example.note && (
        <div className="help-example-note">{example.note}</div>
      )}
    </div>
  );
});
