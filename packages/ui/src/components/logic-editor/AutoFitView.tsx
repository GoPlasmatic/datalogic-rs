import { useEffect } from 'react';
import { useReactFlow } from '@xyflow/react';
import { REACT_FLOW_OPTIONS } from './constants/layout';

export function AutoFitView({ nodeCount }: { nodeCount: number }) {
  const { fitView } = useReactFlow();

  useEffect(() => {
    const timer = setTimeout(() => {
      fitView({
        padding: REACT_FLOW_OPTIONS.fitViewPadding,
        maxZoom: REACT_FLOW_OPTIONS.maxZoom,
      });
    }, 50);
    return () => clearTimeout(timer);
  }, [nodeCount, fitView]);

  return null;
}
