/** Loader contract. Browser-reachable modules must not import Node built-ins. */

import type { GlueInput } from "./wasm.js";

export interface WasmLoader {
  /**
   * Maps caller input (or `undefined` for the packaged asset) to something the
   * generated glue can consume. Loaders may perform I/O; they never instantiate.
   */
  load(input: GlueInput | undefined, defaultUrl: URL): Promise<GlueInput>;
}
