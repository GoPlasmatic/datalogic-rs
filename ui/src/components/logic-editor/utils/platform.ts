interface NavigatorWithUAData extends Navigator {
  userAgentData?: { platform?: string };
}

/**
 * True on macOS and iOS, where shortcuts use Cmd instead of Ctrl.
 *
 * Reads `navigator.userAgentData.platform` where the browser has it and
 * falls back to the user agent string. Replaces the deprecated
 * `navigator.platform`.
 */
export function isApplePlatform(): boolean {
  if (typeof navigator === 'undefined') return false;
  const platform = (navigator as NavigatorWithUAData).userAgentData?.platform || navigator.userAgent;
  return /mac|iphone|ipad|ipod/i.test(platform);
}

/** The platform's shortcut modifier: Cmd on Apple platforms, Ctrl elsewhere. */
export function hasShortcutModifier(event: KeyboardEvent): boolean {
  return isApplePlatform() ? event.metaKey : event.ctrlKey;
}
