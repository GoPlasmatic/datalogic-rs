/**
 * Elements that take text input. Editing shortcuts (Cmd/Ctrl+Z, Backspace,
 * Cmd/Ctrl+K, ...) must leave these alone so typing works normally.
 */
const TEXT_ENTRY_SELECTOR =
  'input, textarea, select, [contenteditable=""], [contenteditable="true"], [contenteditable="plaintext-only"], [role="textbox"]';

/**
 * Elements that own their keyboard input: everything above, plus buttons
 * (Space and Enter activate them) and links (Enter follows them). The
 * debugger's plain-key shortcuts (Space, arrows, Home, End) must not fire,
 * or call preventDefault, while one of these has focus.
 */
const EDITABLE_SELECTOR = `${TEXT_ENTRY_SELECTOR}, button, a[href]`;

function matches(target: EventTarget | null, selector: string): boolean {
  if (!target || typeof target !== 'object') return false;
  const el = target as Partial<HTMLElement>;
  if (el.isContentEditable) return true;
  if (typeof el.closest !== 'function') return false;
  return el.closest(selector) !== null;
}

/**
 * True when a keyboard event targeting `target` should be left to the element
 * itself rather than handled as a debugger shortcut.
 */
export function isEditableTarget(target: EventTarget | null): boolean {
  return matches(target, EDITABLE_SELECTOR);
}

/** True when `target` takes text input, so editing shortcuts must skip it. */
export function isTextEntryTarget(target: EventTarget | null): boolean {
  return matches(target, TEXT_ENTRY_SELECTOR);
}
