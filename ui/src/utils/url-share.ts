import { encode as msgpackEncode, decode as msgpackDecode } from '@msgpack/msgpack';
import { deflateSync, inflateSync } from 'fflate';
import type { DataLogicEvaluationConfig } from '../components/logic-editor/types';

interface ShareableState {
  l: unknown;  // logic (short key for smaller payload)
  d: unknown;  // data
  t?: boolean; // templating (optional)
  c?: DataLogicEvaluationConfig; // engine config (optional, only when non-default)
}

export interface SharedState {
  logic: unknown;
  data: unknown;
  templating?: boolean;
  config?: DataLogicEvaluationConfig;
}

export interface ShareOptions {
  templating?: boolean;
  config?: DataLogicEvaluationConfig | null;
}

// Encode in 32 KB chunks: spreading a large Uint8Array into
// String.fromCharCode(...bytes) overflows the call stack (RangeError) once
// the compressed payload passes roughly 120 KB in V8.
const CHUNK_SIZE = 0x8000;

// Base64URL encoding (URL-safe, no padding)
function toBase64Url(bytes: Uint8Array): string {
  let binary = '';
  for (let i = 0; i < bytes.length; i += CHUNK_SIZE) {
    binary += String.fromCharCode.apply(
      null,
      bytes.subarray(i, i + CHUNK_SIZE) as unknown as number[],
    );
  }
  return btoa(binary)
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}

function fromBase64Url(str: string): Uint8Array {
  // Restore standard base64
  let base64 = str.replace(/-/g, '+').replace(/_/g, '/');
  // Add padding if needed
  while (base64.length % 4) base64 += '=';
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) {
    bytes[i] = binary.charCodeAt(i);
  }
  return bytes;
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return !!value && typeof value === 'object' && !Array.isArray(value);
}

/**
 * Encode logic + data (+ templating flag and engine config) into a compact
 * URL-safe string. `templating` may be passed positionally for backward
 * compatibility, or inside an options object together with `config`.
 */
export function encodeShareableState(
  logic: unknown,
  data: unknown,
  options?: boolean | ShareOptions,
): string {
  const opts: ShareOptions = typeof options === 'boolean' ? { templating: options } : options ?? {};
  const state: ShareableState = { l: logic, d: data };
  if (opts.templating) state.t = true;
  if (opts.config && Object.keys(opts.config).length > 0) state.c = opts.config;

  // Pipeline: Object -> MessagePack -> Deflate -> Base64URL
  const packed = msgpackEncode(state);
  const compressed = deflateSync(packed, { level: 9 });
  return toBase64Url(compressed);
}

export function decodeShareableState(encoded: string): SharedState | null {
  try {
    // Pipeline: Base64URL -> Inflate -> MessagePack -> Object
    const compressed = fromBase64Url(encoded);
    const packed = inflateSync(compressed);
    const state = msgpackDecode(packed) as ShareableState;
    if (!isPlainObject(state) || !('l' in state)) return null;
    const result: SharedState = { logic: state.l, data: state.d };
    if (state.t === true) result.templating = true;
    if (isPlainObject(state.c)) result.config = state.c as DataLogicEvaluationConfig;
    return result;
  } catch {
    return null;
  }
}

export function generateShareableUrl(
  logic: unknown,
  data: unknown,
  options?: boolean | ShareOptions,
): string {
  const encoded = encodeShareableState(logic, data, options);
  const url = new URL(window.location.href);
  url.searchParams.set('s', encoded);
  return url.toString();
}

export function parseShareableUrl(): SharedState | null {
  const params = new URLSearchParams(window.location.search);
  const encoded = params.get('s');
  if (!encoded) return null;
  return decodeShareableState(encoded);
}
