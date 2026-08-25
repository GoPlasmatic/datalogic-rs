// The vendored 5.3.0 engine (nodejs target); imported relatively so tsc sees
// its typings (builtinOperatorNames, Engine.evaluateWithTrace) rather than the
// older package in node_modules.
import * as wasm from '../../../../../../vendor/datalogic/nodejs/datalogic_wasm.js';
import type { JsonLogicValue, LogicNode } from '../../../types';
import type { TracedResult } from '../../../types/trace';
import { jsonLogicToNodes } from '../../jsonlogic-to-nodes';
import { traceToNodes } from '../trace-to-nodes';
import type { TraceConversionResult } from '../types';

export { wasm };

/** Run the real engine and parse the trace envelope. */
export function runTrace(rule: JsonLogicValue, data: unknown = {}, templating = false): TracedResult {
  return JSON.parse(wasm.evaluateWithTrace(JSON.stringify(rule), JSON.stringify(data), templating)) as TracedResult;
}

export interface Analysis {
  trace: TracedResult;
  result: TraceConversionResult;
  staticResult: ReturnType<typeof jsonLogicToNodes>;
  nodeIds: Set<string>;
  /** Visual node id -> number of steps that resolve to it */
  stepsPerNode: Map<string, number>;
  /** Steps whose trace id resolves to no visual node */
  unmappedSteps: number[];
  /** Operator / structure nodes whose id was synthesized (no trace id) */
  syntheticNodes: LogicNode[];
  /** Operator / structure nodes with a trace id but no step */
  stepless: LogicNode[];
  duplicateEdgeIds: string[];
}

const SYNTHETIC_ID = /-(arg|expr)-?\d+$/;
const isVisualOperator = (n: LogicNode) => n.type === 'operator' || n.type === 'structure';

/** Trace + static conversion of a rule with the bookkeeping the assertions need. */
export function analyze(rule: JsonLogicValue, data: unknown = {}, templating = false): Analysis {
  const trace = runTrace(rule, data, templating);
  const result = traceToNodes(trace, { templating, originalValue: rule });
  const staticResult = jsonLogicToNodes(rule, { templating });
  const nodeIds = new Set(result.nodes.map((n) => n.id));

  const stepsPerNode = new Map<string, number>();
  const unmappedSteps: number[] = [];
  for (const step of trace.steps) {
    const visualId = result.traceNodeMap.get(`trace-${step.node_id}`);
    if (!visualId || !nodeIds.has(visualId)) {
      unmappedSteps.push(step.node_id);
      continue;
    }
    stepsPerNode.set(visualId, (stepsPerNode.get(visualId) ?? 0) + 1);
  }

  const syntheticNodes = result.nodes.filter((n) => isVisualOperator(n) && SYNTHETIC_ID.test(n.id));
  const stepless = result.nodes.filter(
    (n) => isVisualOperator(n) && /^trace-\d+$/.test(n.id) && !stepsPerNode.has(n.id)
  );

  const seen = new Set<string>();
  const duplicateEdgeIds: string[] = [];
  for (const edge of result.edges) {
    if (seen.has(edge.id)) duplicateEdgeIds.push(edge.id);
    seen.add(edge.id);
  }

  return { trace, result, staticResult, nodeIds, stepsPerNode, unmappedSteps, syntheticNodes, stepless, duplicateEdgeIds };
}

export const countByType = (nodes: LogicNode[], type: string) => nodes.filter((n) => n.type === type).length;

export const findOperatorNode = (nodes: LogicNode[], operator: string) =>
  nodes.find((n) => n.type === 'operator' && (n.data as { operator?: string }).operator === operator);

/** Every operator name used anywhere inside a rule. */
export function collectOperators(value: unknown, into = new Set<string>()): Set<string> {
  if (Array.isArray(value)) {
    value.forEach((v) => collectOperators(v, into));
  } else if (value !== null && typeof value === 'object') {
    const keys = Object.keys(value);
    if (keys.length === 1) into.add(keys[0]);
    keys.forEach((k) => collectOperators((value as Record<string, unknown>)[k], into));
  }
  return into;
}
