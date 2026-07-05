import { useContext } from 'react';
import { DirectionContext, type FlowDirection } from './DirectionContextDef';

/** The current diagram direction ('flow' by default). */
export function useDirection(): FlowDirection {
  return useContext(DirectionContext);
}

/** True when data flows left → right with the root on the right (the default). */
export function useIsFlowDirection(): boolean {
  return useContext(DirectionContext) === 'flow';
}
