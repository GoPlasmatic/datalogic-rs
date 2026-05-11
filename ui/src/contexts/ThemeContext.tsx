/* eslint-disable react-refresh/only-export-components */
import { createContext, useState, useEffect, useCallback, type ReactNode } from 'react';

export type ThemePreference = 'light' | 'dark' | 'system';
export type ResolvedTheme = 'light' | 'dark';

export interface ThemeContextType {
  /** What the user picked. `'system'` means follow OS preference. */
  themePreference: ThemePreference;
  /** What's actually applied to the document (`'light'` or `'dark'`). */
  resolvedTheme: ResolvedTheme;
  /** Set the user preference. Pass `'system'` to follow OS. */
  setThemePreference: (preference: ThemePreference) => void;

  // Backwards-compatible API (kept for consumers that don't know about 3-way).
  theme: ResolvedTheme;
  toggleTheme: () => void;
  setTheme: (theme: ResolvedTheme) => void;
}

export const ThemeContext = createContext<ThemeContextType | undefined>(undefined);

const STORAGE_KEY = 'theme';

function readStoredPreference(): ThemePreference {
  if (typeof window === 'undefined') return 'system';
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored === 'light' || stored === 'dark' || stored === 'system') return stored;
  return 'system';
}

function getSystemTheme(): ResolvedTheme {
  if (typeof window === 'undefined') return 'light';
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [themePreference, setThemePreferenceState] = useState<ThemePreference>(readStoredPreference);
  const [systemTheme, setSystemTheme] = useState<ResolvedTheme>(getSystemTheme);

  // Track system theme changes (only matters while in 'system' mode, but we keep the
  // listener attached regardless so the user can toggle in and out without setup cost).
  useEffect(() => {
    if (typeof window === 'undefined') return;
    const mq = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = (e: MediaQueryListEvent) => setSystemTheme(e.matches ? 'dark' : 'light');
    mq.addEventListener('change', handler);
    return () => mq.removeEventListener('change', handler);
  }, []);

  const resolvedTheme: ResolvedTheme = themePreference === 'system' ? systemTheme : themePreference;

  useEffect(() => {
    document.documentElement.setAttribute('data-theme', resolvedTheme);
    localStorage.setItem(STORAGE_KEY, themePreference);
  }, [resolvedTheme, themePreference]);

  const setThemePreference = useCallback((pref: ThemePreference) => {
    setThemePreferenceState(pref);
  }, []);

  const toggleTheme = useCallback(() => {
    setThemePreferenceState((prev) => {
      const current: ResolvedTheme = prev === 'system' ? getSystemTheme() : prev;
      return current === 'light' ? 'dark' : 'light';
    });
  }, []);

  const setTheme = useCallback((theme: ResolvedTheme) => {
    setThemePreferenceState(theme);
  }, []);

  return (
    <ThemeContext.Provider
      value={{
        themePreference,
        resolvedTheme,
        setThemePreference,
        theme: resolvedTheme,
        toggleTheme,
        setTheme,
      }}
    >
      {children}
    </ThemeContext.Provider>
  );
}
