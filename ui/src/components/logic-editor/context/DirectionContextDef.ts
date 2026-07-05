import { createContext } from 'react';

/**
 * Diagram direction:
 *  - 'flow'      — data flows left → right, root/result on the RIGHT (Signal Board default).
 *  - 'hierarchy' — root operator on the LEFT, operands nesting to the right (JSON order).
 *
 * The two are exact mirrors: dagre rankDir, edge orientation, and handle sides
 * all flip together. Node handle components read this to place their handles.
 */
export type FlowDirection = 'flow' | 'hierarchy';

export const DirectionContext = createContext<FlowDirection>('flow');
