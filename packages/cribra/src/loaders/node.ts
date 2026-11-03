/** Node-only loader. This module is unreachable from the browser entry. */

import { readFile } from "node:fs/promises";

import { CribraError } from "../errors.js";
import type { GlueInput } from "../wasm.js";
import type { WasmLoader } from "../loader.js";

function isFileUrl(value: URL): boolean {
  return value.protocol === "file:";
}

function isHttpUrl(value: URL): boolean {
  return value.protocol === "http:" || value.protocol === "https:";
}

async function fetchBytes(url: URL): Promise<ArrayBuffer> {
  let response: Response;
  try {
    response = await fetch(url);
  } catch {
    throw new CribraError("wasm_load_failed");
  }
  if (!response.ok) {
    throw new CribraError("wasm_load_failed");
  }
  try {
    return await response.arrayBuffer();
  } catch {
    throw new CribraError("wasm_load_failed");
  }
}

async function readBytes(url: URL): Promise<Uint8Array> {
  try {
    return await readFile(url);
  } catch {
    throw new CribraError("wasm_load_failed");
  }
}

function classifyString(value: string): "file" | "http" | "path" {
  if (value.startsWith("file:")) return "file";
  if (value.startsWith("http:") || value.startsWith("https:")) return "http";
  return "path";
}

export const nodeLoader: WasmLoader = {
  async load(input: GlueInput | undefined, defaultUrl: URL): Promise<GlueInput> {
    if (input === undefined) {
      return readBytes(defaultUrl);
    }
    if (input instanceof Uint8Array || input instanceof ArrayBuffer) {
      return input;
    }
    if (input instanceof URL) {
      if (isFileUrl(input)) return readBytes(input);
      if (isHttpUrl(input)) return fetchBytes(input);
      throw new CribraError("wasm_load_failed");
    }
    if (typeof input === "string") {
      const kind = classifyString(input);
      if (kind === "http") return fetchBytes(new URL(input));
      if (kind === "file") return readBytes(new URL(input));
      return readBytes(new URL(input, defaultUrl));
    }
    throw new CribraError("invalid_argument", {
      domain: "init",
      message: "Unsupported WASM input.",
    });
  },
};
