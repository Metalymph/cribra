/** Internal scanner identity map. Native handles never escape these closures. */

import type { Capabilities, Scanner } from "./types.js";
import type { ScanEngine } from "./wasm/cribra.js";
import type { WasmModule } from "./wasm.js";

export interface ScannerInternals {
  readonly engine: ScanEngine;
  readonly wasm: WasmModule;
  readonly capabilities: Capabilities;
  isActive(): boolean;
}

const registry = new WeakMap<Scanner, ScannerInternals>();

export function registerScanner(
  scanner: Scanner,
  internals: ScannerInternals,
): void {
  registry.set(scanner, internals);
}

/** Returns the private native state for a wrapper-created scanner only. */
export function scannerInternals(scanner: unknown): ScannerInternals | undefined {
  if (scanner === null || typeof scanner !== "object") return undefined;
  return registry.get(scanner as Scanner);
}
