import { encode as msgpackEncode, decode as msgpackDecode } from '@msgpack/msgpack';
import { deflateSync, inflateSync } from 'fflate';

interface ShareableState {
  l: unknown;  // logic (short key for smaller payload)
  d: unknown;  // data
  p?: boolean; // preserveStructure (optional)
}

// Base64URL encoding (URL-safe, no padding)
function toBase64Url(bytes: Uint8Array): string {
  const binary = String.fromCharCode(...bytes);
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

export function encodeShareableState(logic: unknown, data: unknown, preserveStructure?: boolean): string {
  const state: ShareableState = { l: logic, d: data };
  if (preserveStructure) state.p = true;

  // Pipeline: Object → MessagePack → Deflate → Base64URL
  const packed = msgpackEncode(state);
  const compressed = deflateSync(packed, { level: 9 });
  return toBase64Url(compressed);
}

export function decodeShareableState(encoded: string): { logic: unknown; data: unknown; preserveStructure?: boolean } | null {
  try {
    // Pipeline: Base64URL → Inflate → MessagePack → Object
    const compressed = fromBase64Url(encoded);
    const packed = inflateSync(compressed);
    const state = msgpackDecode(packed) as ShareableState;
    return { logic: state.l, data: state.d, preserveStructure: state.p };
  } catch {
    return null;
  }
}

export function generateShareableUrl(logic: unknown, data: unknown, preserveStructure?: boolean): string {
  const encoded = encodeShareableState(logic, data, preserveStructure);
  const url = new URL(window.location.href);
  url.searchParams.set('s', encoded);
  return url.toString();
}

export function parseShareableUrl(): { logic: unknown; data: unknown; preserveStructure?: boolean } | null {
  const params = new URLSearchParams(window.location.search);
  const encoded = params.get('s');
  if (!encoded) return null;
  return decodeShareableState(encoded);
}
