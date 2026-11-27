/**
 * Opaque token generation for content-script identities.
 *
 * Tokens are transient, in-memory, and never persisted. Snapshot identifiers
 * include a monotonic counter as well as random bytes, so an identifier is
 * never reused for a different snapshot even if an invalidated record is
 * evicted from the bounded store.
 */

const objectTokens = new WeakMap<object, string>();
let objectCounter = 0;
let snapshotCounter = 0;

function randomHex(bytes: number): string {
  const buffer = new Uint8Array(bytes);
  globalThis.crypto.getRandomValues(buffer);
  let hex = "";
  for (const byte of buffer) {
    hex += byte.toString(16).padStart(2, "0");
  }
  return hex;
}

/** Returns a stable opaque token for one object instance. */
export function objectToken(value: object, prefix: string): string {
  const existing = objectTokens.get(value);
  if (existing !== undefined) return existing;
  objectCounter += 1;
  const token = `${prefix}_${objectCounter}_${randomHex(12)}`;
  objectTokens.set(value, token);
  return token;
}

/** Returns a fresh snapshot identifier that can never collide by construction. */
export function newSnapshotId(): string {
  snapshotCounter += 1;
  return `snap_${snapshotCounter}_${randomHex(16)}`;
}

/** Test-only visibility into the monotonic counters. */
export function tokenCountersForTest(): {
  readonly objects: number;
  readonly snapshots: number;
} {
  return { objects: objectCounter, snapshots: snapshotCounter };
}
