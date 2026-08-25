/**
 * HelpSection Component
 *
 * Displays operator help information including summary, arity, return type,
 * a link to the operator's documentation page, and collapsible examples.
 */

import { memo, useState, useCallback, useMemo } from 'react';
import { ChevronDown, ChevronRight, Copy, Check, ExternalLink } from 'lucide-react';
import type {
  Operator,
  OperatorHelp,
  AritySpec,
  OperatorExample,
} from '../config/operators.types';
import { operators as operatorRegistry } from '../config/operators';
import { getCategory } from '../config/categories';
import { getDocsUrlForOperator } from '../config/docs';
import { formatArity } from '../config/arity';
import { Icon } from '../utils/icons';

interface HelpSectionProps {
  help: OperatorHelp;
  arity: AritySpec;
  /**
   * The operator being described. Optional for backward compatibility: when
   * omitted, the operator is recovered from the registry by matching the
   * `help` object identity (configs are static, so this is exact).
   */
  operator?: Operator;
}

function findOperatorByHelp(help: OperatorHelp): Operator | undefined {
  return Object.values(operatorRegistry).find((op) => op.help === help);
}

export const HelpSection = memo(function HelpSection({
  help,
  arity,
  operator,
}: HelpSectionProps) {
  const [showExamples, setShowExamples] = useState(false);

  const toggleExamples = useCallback(() => {
    setShowExamples((prev) => !prev);
  }, []);

  const op = useMemo(() => operator ?? findOperatorByHelp(help), [operator, help]);
  const category = op ? getCategory(op.category) : undefined;
  const iconName = op?.ui?.icon ?? category?.icon;
  const docsUrl = op ? getDocsUrlForOperator(op) : undefined;

  return (
    <div className="help-section">
      {/* Header: icon, label, operator name */}
      {op && (
        <div className="help-header">
          {iconName && (
            <Icon name={iconName} size={14} style={{ color: category?.color }} />
          )}
          <span>{op.label}</span>
          <code className="help-operator-name">{op.name}</code>
        </div>
      )}

      {/* Summary */}
      <p className="help-summary">{help.summary}</p>

      {/* Badges */}
      <div className="help-badges">
        <span className="help-badge help-badge-arity">{formatArity(arity)}</span>
        <span className="help-badge help-badge-return">
          Returns: <code>{help.returnType}</code>
        </span>
        {docsUrl && (
          <a
            className="help-badge help-badge-docs"
            href={docsUrl}
            target="_blank"
            rel="noreferrer"
            title="Open the operator documentation"
          >
            Docs <ExternalLink size={11} />
          </a>
        )}
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
      {example.templating && (
        <div className="help-example-data">
          <span className="help-example-label">Mode:</span>
          <code>templating</code>
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
