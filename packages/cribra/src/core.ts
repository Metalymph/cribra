/** Initialization state, capability/version metadata, and runtime context. */

import { CribraError } from "./errors.js";
import {
  backendFromNative,
  profileFromNative,
} from "./enums.js";
import type { WasmLoader } from "./loader.js";
import { PACKAGE_VERSION } from "./generated/package-version.js";
import type {
  Capabilities,
  InitOptions,
  VersionInfo,
} from "./types.js";
import {
  initializeGlue,
  type GlueInput,
  type WasmModule,
} from "./wasm.js";

export interface InitializedState {
  readonly wasm: WasmModule;
  readonly capabilities: Capabilities;
  readonly version: VersionInfo;
}

export interface Runtime {
  init(options?: InitOptions): Promise<void>;
  isInitialized(): boolean;
  getCapabilities(): Capabilities;
  getVersion(): VersionInfo;
  /** Internal readiness gate for scanner construction. */
  requireInitialized(): InitializedState;
}

/** A failure after the glue published its exports cannot be retried as if uninitialized. */
class FatalInitError extends Error {
  readonly error: CribraError;

  constructor(error: CribraError) {
    super(error.message);
    this.name = "FatalInitError";
    this.error = error;
  }
}

function normalizeInitInput(options: InitOptions | undefined): GlueInput | undefined {
  if (options === undefined) return undefined;
  if (options === null || typeof options !== "object" || Array.isArray(options)) {
    throw new CribraError("invalid_argument", {
      domain: "init",
      message: "Initialization options must be an object.",
    });
  }
  const input: unknown = options.wasm;
  if (input === undefined) return undefined;
  if (
    input instanceof Uint8Array ||
    input instanceof ArrayBuffer ||
    input instanceof URL ||
    typeof input === "string"
  ) {
    return input;
  }
  throw new CribraError("invalid_argument", {
    domain: "init",
    message: "Unsupported WASM input.",
  });
}

function readBoolean(value: unknown): boolean {
  if (typeof value !== "boolean") {
    throw new Error("Malformed capability manifest");
  }
  return value;
}

function readCapabilities(wasm: WasmModule): Capabilities {
  const manifest = wasm.capabilityManifest();
  try {
    return Object.freeze({
      backend: backendFromNative(manifest.backend),
      profile: profileFromNative(manifest.profile),
      singleSourceScan: readBoolean(manifest.singleSourceScan),
      batchScan: readBoolean(manifest.batchScan),
      streaming: readBoolean(manifest.streaming),
      incremental: readBoolean(manifest.incremental),
      redaction: readBoolean(manifest.redaction),
      templating: readBoolean(manifest.templating),
      pseudonymization: readBoolean(manifest.pseudonymization),
      synthesis: readBoolean(manifest.synthesis),
      shareBundle: readBoolean(manifest.shareBundle),
      ruleMetadata: readBoolean(manifest.ruleMetadata),
      explanations: readBoolean(manifest.explanations),
      parallel: readBoolean(manifest.parallel),
      utf16OffsetConversion: true,
    });
  } finally {
    manifest.free();
  }
}

function readVersion(wasm: WasmModule): VersionInfo {
  const wasmAdapter = wasm.wasmAdapterVersion();
  if (typeof wasmAdapter !== "string" || wasmAdapter.length === 0) {
    throw new Error("Malformed adapter version");
  }
  return Object.freeze({ package: PACKAGE_VERSION, wasmAdapter });
}

async function runInit(
  options: InitOptions | undefined,
  loader: WasmLoader,
  defaultUrl: URL,
): Promise<InitializedState> {
  const input = normalizeInitInput(options);

  let prepared: GlueInput;
  try {
    prepared = await loader.load(input, defaultUrl);
  } catch (error) {
    if (error instanceof CribraError) throw error;
    throw new CribraError("wasm_load_failed");
  }

  let wasm: WasmModule;
  try {
    wasm = await initializeGlue(prepared);
  } catch {
    // The generated glue assigns its internal export table only after a
    // successful finalization, so a rejected load/instantiate is retryable.
    throw new CribraError("wasm_instantiate_failed");
  }

  // From here the WASM module is active. A metadata failure is fatal rather
  // than a retryable "not initialized" state.
  try {
    return Object.freeze({
      wasm,
      capabilities: readCapabilities(wasm),
      version: readVersion(wasm),
    });
  } catch {
    throw new FatalInitError(new CribraError("wasm_instantiate_failed"));
  }
}

export function createRuntime(loader: WasmLoader, defaultUrl: URL): Runtime {
  let ready: InitializedState | undefined;
  let inflight: Promise<void> | undefined;
  let fatal: CribraError | undefined;

  function init(options?: InitOptions): Promise<void> {
    if (ready !== undefined) {
      return Promise.resolve();
    }
    if (fatal !== undefined) {
      return Promise.reject(fatal);
    }
    if (inflight !== undefined) {
      return inflight;
    }

    inflight = runInit(options, loader, defaultUrl).then(
      (state) => {
        ready = state;
        inflight = undefined;
      },
      (error: unknown) => {
        inflight = undefined;
        if (error instanceof FatalInitError) {
          fatal = error.error;
          throw error.error;
        }
        throw error;
      },
    );
    return inflight;
  }

  function requireInitialized(): InitializedState {
    if (ready !== undefined) return ready;
    if (fatal !== undefined) throw fatal;
    throw new CribraError("not_initialized");
  }

  return {
    init,
    isInitialized() {
      return ready !== undefined;
    },
    getCapabilities() {
      return requireInitialized().capabilities;
    },
    getVersion() {
      return requireInitialized().version;
    },
    requireInitialized,
  };
}
