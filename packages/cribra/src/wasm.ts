/** Internal single import of the generated wasm-bindgen glue. */

import initGlue from "./wasm/cribra.js";
import * as glue from "./wasm/cribra.js";

export type WasmModule = typeof glue;
export type GlueInput = Uint8Array | ArrayBuffer | URL | string;

let loaded: WasmModule | undefined;

/**
 * Instantiates the generated glue exactly once per wrapper module instance.
 *
 * The glue only publishes its internal export table after a successful
 * instantiation, so a rejected load/instantiate may be retried safely.
 */
export async function initializeGlue(input: GlueInput): Promise<WasmModule> {
  if (loaded === undefined) {
    await initGlue({ module_or_path: input });
    loaded = glue;
  }
  return loaded;
}

export function isGlueLoaded(): boolean {
  return loaded !== undefined;
}
