import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useRef,
  useState,
  type ReactNode,
} from 'react';
import { createPortal } from 'react-dom';
import './Tooltip.css';

type Side = 'top' | 'bottom' | 'left' | 'right';

interface TooltipProps {
  label: string;
  shortcut?: string;
  side?: Side;
  /** Delay before the tooltip appears, in ms. Default 300ms. */
  delay?: number;
  children: ReactNode;
}

interface Position {
  top: number;
  left: number;
  side: Side;
}

// Distance between trigger and tooltip in pixels.
const OFFSET = 8;

/**
 * Lightweight tooltip primitive.
 *
 * Wraps its children in a layout-neutral `<span>` that catches hover/focus
 * events, then renders the tooltip itself into a portal at `document.body`
 * with `position: fixed` — so it's immune to `overflow: hidden` or stacking-
 * context clipping from any ancestor (e.g. `.logic-editor`'s rounded clip).
 *
 * Hides on coarse-pointer / no-hover devices via CSS.
 */
export function Tooltip({
  label,
  shortcut,
  side = 'bottom',
  delay = 300,
  children,
}: TooltipProps) {
  const [visible, setVisible] = useState(false);
  const [position, setPosition] = useState<Position | null>(null);
  const triggerRef = useRef<HTMLSpanElement | null>(null);
  const tooltipRef = useRef<HTMLDivElement | null>(null);
  const showTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const computePosition = useCallback((): Position | null => {
    const trigger = triggerRef.current;
    const tooltip = tooltipRef.current;
    if (!trigger) return null;
    const triggerRect = trigger.getBoundingClientRect();
    const tooltipRect = tooltip?.getBoundingClientRect();
    const w = tooltipRect?.width ?? 0;
    const h = tooltipRect?.height ?? 0;

    const compute = (s: Side): { top: number; left: number } => {
      switch (s) {
        case 'top':
          return {
            top: triggerRect.top - h - OFFSET,
            left: triggerRect.left + triggerRect.width / 2 - w / 2,
          };
        case 'bottom':
          return {
            top: triggerRect.bottom + OFFSET,
            left: triggerRect.left + triggerRect.width / 2 - w / 2,
          };
        case 'left':
          return {
            top: triggerRect.top + triggerRect.height / 2 - h / 2,
            left: triggerRect.left - w - OFFSET,
          };
        case 'right':
          return {
            top: triggerRect.top + triggerRect.height / 2 - h / 2,
            left: triggerRect.right + OFFSET,
          };
      }
    };

    let chosen: Side = side;
    let { top, left } = compute(side);

    // Flip if we'd run off the viewport edge in the requested direction.
    const margin = 4;
    if (side === 'bottom' && top + h > window.innerHeight - margin) {
      chosen = 'top';
      ({ top, left } = compute('top'));
    } else if (side === 'top' && top < margin) {
      chosen = 'bottom';
      ({ top, left } = compute('bottom'));
    } else if (side === 'right' && left + w > window.innerWidth - margin) {
      chosen = 'left';
      ({ top, left } = compute('left'));
    } else if (side === 'left' && left < margin) {
      chosen = 'right';
      ({ top, left } = compute('right'));
    }

    // Clamp to viewport so the tooltip is never partially off-screen.
    left = Math.max(margin, Math.min(left, window.innerWidth - w - margin));
    top = Math.max(margin, Math.min(top, window.innerHeight - h - margin));

    return { top, left, side: chosen };
  }, [side]);

  const show = useCallback(() => {
    if (showTimerRef.current) clearTimeout(showTimerRef.current);
    showTimerRef.current = setTimeout(() => {
      setVisible(true);
    }, delay);
  }, [delay]);

  const hide = useCallback(() => {
    if (showTimerRef.current) {
      clearTimeout(showTimerRef.current);
      showTimerRef.current = null;
    }
    setVisible(false);
    setPosition(null);
  }, []);

  useEffect(() => {
    return () => {
      if (showTimerRef.current) clearTimeout(showTimerRef.current);
    };
  }, []);

  /* eslint-disable react-hooks/set-state-in-effect --
     Standard measure-then-position pattern: we need the rendered tooltip's
     dimensions before we can place it next to the trigger, so the position
     state has to be set after the first paint. */
  useLayoutEffect(() => {
    if (!visible) return;
    setPosition(computePosition());

    const handleReposition = () => setPosition(computePosition());
    window.addEventListener('scroll', handleReposition, true);
    window.addEventListener('resize', handleReposition);
    return () => {
      window.removeEventListener('scroll', handleReposition, true);
      window.removeEventListener('resize', handleReposition);
    };
  }, [visible, computePosition]);
  /* eslint-enable react-hooks/set-state-in-effect */

  return (
    <>
      <span
        ref={triggerRef}
        className="dl-tooltip-wrap"
        onMouseEnter={show}
        onMouseLeave={hide}
        onFocus={show}
        onBlur={hide}
        aria-label={label}
      >
        {children}
      </span>
      {visible && typeof document !== 'undefined' &&
        createPortal(
          <div
            ref={tooltipRef}
            role="tooltip"
            className="dl-tooltip"
            data-side={position?.side ?? side}
            style={{
              position: 'fixed',
              top: position?.top ?? -9999,
              left: position?.left ?? -9999,
              opacity: position ? 1 : 0,
            }}
          >
            <span className="dl-tooltip-label">{label}</span>
            {shortcut ? <span className="dl-tooltip-shortcut">{shortcut}</span> : null}
          </div>,
          document.body,
        )}
    </>
  );
}
