import { createContext, useContext, useEffect } from 'react';

/**
 * The root element (`.logic-editor`) of the enclosing editor instance, or
 * null before it mounts. Keyboard shortcuts listen here rather than on
 * `window`, so they fire only while focus is inside this editor: a page
 * with several editors, or with its own shortcuts, keeps its keys.
 */
export const EditorRootContext = createContext<HTMLElement | null>(null);

/**
 * Listen for `keydown` on the enclosing editor's root element while
 * `enabled`. Events reach the root only from focused elements inside it.
 */
export function useEditorKeydown(
  handler: (event: KeyboardEvent) => void,
  enabled = true,
): void {
  const root = useContext(EditorRootContext);
  useEffect(() => {
    if (!root || !enabled) return;
    root.addEventListener('keydown', handler);
    return () => root.removeEventListener('keydown', handler);
  }, [root, handler, enabled]);
}
