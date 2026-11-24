/**
 * MV3 extension service worker.
 *
 * Flow is initialized explicitly from the extension's own packaged asset. The
 * scanner lives only in this worker's memory and is disposed when Chrome
 * suspends the worker. No source text, match value, or raw diagnostic is
 * persisted or logged.
 */

import {
  CribraError,
  createScanner,
  getCapabilities,
  getVersion,
  init,
  isInitialized,
} from "./cribra.js";
import type { Scanner } from "./cribra.js";
import {
  GuardRequestError,
  parseRequest,
  projectScanResult,
  projectTransformResult,
  toPublicTransform,
  toWireError,
} from "./protocol.js";
import type {
  CapabilitiesResponse,
  GuardRequest,
  GuardResponse,
  InitTiming,
  ScanResponse,
  TransformResponse,
} from "./protocol.js";

const WORKER_STARTED_AT = performance.now();
const ARTIFACT_URL = "wasm/cribra_bg.wasm";

let scanner: Scanner | undefined;
let scannerInitMs = 0;
let initPromise: Promise<void> | undefined;
let initTiming: InitTiming | undefined;

function hasFlow(): boolean {
  return isInitialized();
}

function hex(bytes: ArrayBuffer): string {
  let output = "";
  for (const byte of new Uint8Array(bytes)) {
    output += byte.toString(16).padStart(2, "0");
  }
  return output;
}

async function loadPinnedArtifact(): Promise<{
  readonly bytes: Uint8Array;
  readonly sha256: string;
}> {
  const url = chrome.runtime.getURL(ARTIFACT_URL);
  let response: Response;
  try {
    response = await fetch(url, { cache: "no-store", credentials: "omit" });
  } catch {
    throw new CribraError("wasm_load_failed");
  }
  if (!response.ok) {
    throw new CribraError("wasm_load_failed");
  }

  let buffer: ArrayBuffer;
  try {
    buffer = await response.arrayBuffer();
  } catch {
    throw new CribraError("wasm_load_failed");
  }
  const bytes = new Uint8Array(buffer);
  if (bytes.byteLength !== __CRIBRA_ARTIFACT_BYTES__) {
    throw new CribraError("wasm_load_failed");
  }

  const sha256 = hex(
    await crypto.subtle.digest("SHA-256", buffer),
  );
  if (sha256 !== __CRIBRA_ARTIFACT_SHA256__) {
    throw new CribraError("wasm_load_failed");
  }
  return { bytes, sha256 };
}

/**
 * Explicit, idempotent initialization. A rejected load or instantiation is
 * retryable; a failure after Flow publishes its exports stays fatal inside the
 * wrapper runtime.
 */
function initialize(): Promise<void> {
  if (hasFlow()) {
    return Promise.resolve();
  }
  if (initPromise !== undefined) {
    return initPromise;
  }

  const startedAt = performance.now();
  initPromise = (async () => {
    const artifact = await loadPinnedArtifact();
    await init({ wasm: artifact.bytes });
    const initializedAt = performance.now();
    initTiming = Object.freeze({
      workerStartedAt: WORKER_STARTED_AT,
      initializedAt,
      durationMs: initializedAt - startedAt,
      cold: true,
      wasmBytes: artifact.bytes.byteLength,
      wasmSha256: artifact.sha256,
      artifactVerified: true,
    });
  })().catch((error: unknown) => {
    initPromise = undefined;
    throw error;
  });
  return initPromise;
}

function requireScanner(): Scanner {
  if (!hasFlow()) {
    // Only reachable if this function is called outside the message path.
    throw new CribraError("not_initialized");
  }
  if (scanner === undefined || scanner.disposed) {
    const startedAt = performance.now();
    scanner = createScanner();
    scannerInitMs = performance.now() - startedAt;
  }
  return scanner;
}

function timing(): InitTiming {
  if (initTiming === undefined) {
    throw new CribraError("not_initialized");
  }
  return initTiming;
}

async function handleCapabilities(
  request: Extract<GuardRequest, { type: "cribra.capabilities" }>,
): Promise<CapabilitiesResponse> {
  await initialize();
  const active = requireScanner();
  return {
    ok: true,
    type: "cribra.capabilities",
    ...(request.requestId === undefined
      ? {}
      : { requestId: request.requestId }),
    capabilities: getCapabilities(),
    version: getVersion(),
    scanner: {
      rulesCount: active.rulesCount,
      disposed: active.disposed,
      initMs: scannerInitMs,
    },
    init: timing(),
  };
}

function handleScan(
  request: Extract<GuardRequest, { type: "cribra.scan" }>,
): ScanResponse {
  const startedAt = performance.now();
  const active = requireScanner();
  const result = active.scan(request.source, {
    ...(request.includeExplanations === undefined
      ? {}
      : { includeExplanations: request.includeExplanations }),
  });
  return {
    ok: true,
    type: "cribra.scan",
    ...(request.requestId === undefined
      ? {}
      : { requestId: request.requestId }),
    scan: projectScanResult(request.source, result),
    timing: { durationMs: performance.now() - startedAt },
  };
}

function handleTransform(
  request: Extract<GuardRequest, { type: "cribra.transform" }>,
): TransformResponse {
  const startedAt = performance.now();
  const active = requireScanner();
  const result = active.transform(
    request.source,
    toPublicTransform(request.transform),
  );
  return {
    ok: true,
    type: "cribra.transform",
    ...(request.requestId === undefined
      ? {}
      : { requestId: request.requestId }),
    transform: projectTransformResult(request.source, result),
    timing: { durationMs: performance.now() - startedAt },
  };
}

async function handle(message: unknown): Promise<GuardResponse> {
  const request = parseRequest(message);
  await initialize();
  switch (request.type) {
    case "cribra.capabilities":
      return handleCapabilities(request);
    case "cribra.scan":
      return handleScan(request);
    case "cribra.transform":
      return handleTransform(request);
  }
}

function requestIdOf(message: unknown): string | undefined {
  if (message === null || typeof message !== "object") return undefined;
  const value = (message as Record<string, unknown>).requestId;
  return typeof value === "string" && value.length <= 64 ? value : undefined;
}

function extensionOrigin(): string {
  return chrome.runtime.getURL("");
}

function isAllowedSender(sender: chrome.runtime.MessageSender): boolean {
  if (sender.id !== chrome.runtime.id) return false;
  const origin = extensionOrigin();
  return (
    (typeof sender.url === "string" && sender.url.startsWith(origin)) ||
    sender.origin === origin
  );
}

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (!isAllowedSender(sender)) {
    sendResponse(
      toWireError(
        new GuardRequestError("forbidden_sender"),
        requestIdOf(message),
      ),
    );
    return false;
  }

  handle(message).then(sendResponse, (error: unknown) => {
    sendResponse(toWireError(error, requestIdOf(message)));
  });
  return true;
});

chrome.runtime.onInstalled.addListener(() => {
  void initialize().catch(() => {
    // The failure is returned, sanitized, to the next request.
  });
});

chrome.runtime.onSuspend.addListener(() => {
  if (scanner !== undefined) {
    scanner.dispose();
    scanner = undefined;
    scannerInitMs = 0;
  }
});

// Cold-start initialization belongs to the extension worker, not to the first
// page that happens to send a message.
void initialize().catch(() => {
  // The failure is returned, sanitized, to the next request.
});
