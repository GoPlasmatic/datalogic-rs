/**
 * Elements that own their keyboard input. The debugger's global shortcuts
 * (Space, arrows, Home, End) must not fire, or call preventDefault, while one
 * of these has focus: text fields, selects, buttons (Space activates them),
 * contenteditable regions and ARIA text boxes.
 */
const EDITABLE_SELECTOR =
  'input, textarea, select, button, [contenteditable=""], [contenteditable="true"], [contenteditable="plaintext-only"], [role="textbox"]';

/**
 * True when a keyboard event targeting `target` should be left to the element
 * itself rather than handled as a debugger shortcut.
 */
export function isEditableTarget(target: EventTarget | null): boolean {
  if (!target || typeof target !== 'object') return false;
  const el = target as Partial<HTMLElement>;
  if (el.isContentEditable) return true;
  if (typeof el.closest !== 'function') return false;
  return el.closest(EDITABLE_SELECTOR) !== null;
}
