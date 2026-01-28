/**
 * Editor Context Definition
 *
 * Defines the React context for the visual editor.
 */

import { createContext } from 'react';
import type { EditorContextValue } from './types';

export const EditorContext = createContext<EditorContextValue | null>(null);
