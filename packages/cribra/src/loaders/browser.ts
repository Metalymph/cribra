/** Browser/default loader: no Node built-in imports. */

import type { WasmLoader } from "../loader.js";

export const browserLoader: WasmLoader = {
  async load(input, defaultUrl) {
    // URL and string inputs are handed to the generated glue so it can use
    // instantiateStreaming and its MIME-type fallback.
    return input ?? defaultUrl;
  },
};
