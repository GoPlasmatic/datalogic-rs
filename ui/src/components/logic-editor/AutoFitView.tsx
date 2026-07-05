import { useEffect } from 'react';
import { useReactFlow } from '@xyflow/react';
import { REACT_FLOW_OPTIONS } from './constants/layout';

/**
 * Fits the diagram to the viewport on load/reload. The component is remounted
 * whenever the expression's structure (or direction) changes, so this effect
 * re-runs on every reload. Two animation frames let ReactFlow measure the
 * freshly-laid-out nodes before fitting, and a short fallback timer catches the
 * case where the layout settles a beat later (async relayout, fonts).
 */
export function AutoFitView({ nodeCount }: { nodeCount: number }) {
  const { fitView } = useReactFlow();

  useEffect(() => {
    if (nodeCount === 0) return;
    const opts = {
      padding: REACT_FLOW_OPTIONS.fitViewPadding,
      maxZoom: REACT_FLOW_OPTIONS.maxZoom,
    };
    let raf2 = 0;
    const raf1 = requestAnimationFrame(() => {
      raf2 = requestAnimationFrame(() => fitView(opts));
    });
    const timer = setTimeout(() => fitView(opts), 120);
    return () => {
      cancelAnimationFrame(raf1);
      cancelAnimationFrame(raf2);
      clearTimeout(timer);
    };
  }, [nodeCount, fitView]);

  return null;
}
