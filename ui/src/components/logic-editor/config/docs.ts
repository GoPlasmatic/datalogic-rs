/**
 * Operator documentation links
 *
 * Every operator has a `## <heading>` section on one of the mdBook pages
 * under https://goplasmatic.github.io/datalogic-rs/operators/. The page
 * comes from the operator's category (see categories.ts `docsPage`) and the
 * anchor from the heading text, normalised the way mdBook does it: lowercase,
 * whitespace to '-', everything that is not alphanumeric, '-' or '_' dropped.
 *
 * For symbolic operators the heading is "<symbol> (<Name>)", so the symbol
 * disappears and the anchor becomes e.g. "#-add". Those headings are listed
 * here; word-named operators are their own heading.
 */

import type { Operator, OperatorCategory } from './operators.types';
import { getCategoryDocsPage } from './categories';

export const DOCS_BASE_URL = 'https://goplasmatic.github.io/datalogic-rs/operators/';

/** Doc headings that differ from the operator name (mdBook input text). */
const DOC_HEADINGS: Record<string, string> = {
  '+': '+ (Add)',
  '-': '- (Subtract)',
  '*': '* (Multiply)',
  '/': '/ (Divide)',
  '%': '% (Modulo)',
  '==': '== (Equals)',
  '===': '=== (Strict Equals)',
  '!=': '!= (Not Equals)',
  '!==': '!== (Strict Not Equals)',
  '>': '> (Greater Than)',
  '>=': '>= (Greater Than or Equal)',
  '<': '< (Less Than)',
  '<=': '<= (Less Than or Equal)',
  '!': '! (Not)',
  '!!': '!! (Double Not / Boolean Cast)',
  '?:': '?: (Ternary)',
  '??': '?? (Null Coalesce)',
  switch: 'switch / match',
  match: 'switch / match',
};

/** mdBook's heading-id normalisation. */
export function mdBookAnchor(heading: string): string {
  let out = '';
  for (const ch of heading.trim().toLowerCase()) {
    if (/[\p{L}\p{N}_-]/u.test(ch)) out += ch;
    else if (/\s/.test(ch)) out += '-';
  }
  return out;
}

/** Anchor for an operator's section on its docs page. */
export function getOperatorDocsAnchor(name: string): string {
  return mdBookAnchor(DOC_HEADINGS[name] ?? name);
}

/** Full URL of an operator's documentation section. */
export function getOperatorDocsUrl(name: string, category: OperatorCategory): string {
  return `${DOCS_BASE_URL}${getCategoryDocsPage(category)}.html#${getOperatorDocsAnchor(name)}`;
}

/** Convenience overload for a full operator config. */
export function getDocsUrlForOperator(op: Operator): string {
  return getOperatorDocsUrl(op.name, op.category);
}
