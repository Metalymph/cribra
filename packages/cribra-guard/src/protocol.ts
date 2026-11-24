/**
 * Minimal extension message protocol.
 *
 * The wire surface deliberately carries no matched values, no source echoes,
 * and no raw diagnostics. Findings keep Cribra's UTF-8 byte spans and add the
 * UTF-16 text range so callers cannot silently treat byte offsets as
 * JavaScript string indices.
 */

import {
  byteSpanToTextRange,
  CribraError,
} from "./cribra.js";
import type {
  Candidate,
  Capabilities,
  Finding,
  ScanResult,
  TextRange,
  TransformRequest,
  TransformResult,
  VersionInfo,
} from "./cribra.js";

/** Rejects oversized pages before the scanner can allocate for them. */
export const MAX_SOURCE_LENGTH = 1_000_000;
export const MAX_REQUEST_ID_LENGTH = 64;
export const MAX_REPLACEMENT_LENGTH = 1_024;
export const MAX_NAMESPACE_LENGTH = 128;

export type GuardMessageType =
  | "cribra.capabilities"
  | "cribra.scan"
  | "cribra.transform";

export type GuardTransformRequest =
  | {
      readonly kind: "redact";
      readonly replacement?: string;
    }
  | {
      readonly kind: "template";
      readonly namespace?: string;
      readonly numbered?: boolean;
    };

export interface CapabilitiesRequest {
  readonly type: "cribra.capabilities";
  readonly requestId?: string;
}

export interface ScanRequest {
  readonly type: "cribra.scan";
  readonly requestId?: string;
  readonly source: string;
  readonly includeExplanations?: boolean;
}

export interface TransformMessageRequest {
  readonly type: "cribra.transform";
  readonly requestId?: string;
  readonly source: string;
  readonly transform: GuardTransformRequest;
}

export type GuardRequest =
  | CapabilitiesRequest
  | ScanRequest
  | TransformMessageRequest;

export type GuardProtocolErrorCode =
  | "invalid_message"
  | "message_too_large"
  | "unsupported_transform"
  | "forbidden_sender";

export interface WireError {
  readonly code: string;
  readonly domain?: string;
  readonly message: string;
  readonly ruleId?: string;
}

export interface WireFinding extends Finding {
  /** Half-open UTF-16 code-unit range for `String.prototype.slice`. */
  readonly textRange: TextRange;
}

export interface WireCandidate extends Candidate {
  readonly textRange: TextRange;
}

export interface WireScanResult extends Omit<ScanResult, "findings" | "candidates"> {
  readonly findings: readonly WireFinding[];
  readonly candidates: readonly WireCandidate[];
}

export interface InitTiming {
  /** `performance.now()` in the worker when the module began evaluating. */
  readonly workerStartedAt: number;
  /** `performance.now()` when Flow became usable. */
  readonly initializedAt: number;
  /** Wall time from worker module start to successful initialization. */
  readonly durationMs: number;
  /** True while this worker had not initialized Flow before this response. */
  readonly cold: boolean;
  readonly wasmBytes: number;
  readonly wasmSha256: string;
  /** The runtime digest matched the artifact pinned into this build. */
  readonly artifactVerified: boolean;
}

export interface OperationTiming {
  readonly durationMs: number;
}

export interface CapabilitiesResponse {
  readonly ok: true;
  readonly type: "cribra.capabilities";
  readonly requestId?: string;
  readonly capabilities: Capabilities;
  readonly version: VersionInfo;
  readonly scanner: {
    readonly rulesCount: number;
    readonly disposed: boolean;
    /** Time spent in the synchronous `createScanner` call in this worker. */
    readonly initMs: number;
  };
  readonly init: InitTiming;
}

export interface ScanResponse {
  readonly ok: true;
  readonly type: "cribra.scan";
  readonly requestId?: string;
  readonly scan: WireScanResult;
  readonly timing: OperationTiming;
}

export interface TransformResponse {
  readonly ok: true;
  readonly type: "cribra.transform";
  readonly requestId?: string;
  readonly transform: WireScanResult & { readonly output: string };
  readonly timing: OperationTiming;
}

export interface ErrorResponse {
  readonly ok: false;
  readonly type: GuardMessageType | "cribra.error";
  readonly requestId?: string;
  readonly error: WireError;
}

export type GuardResponse =
  | CapabilitiesResponse
  | ScanResponse
  | TransformResponse
  | ErrorResponse;

export class GuardRequestError extends Error {
  readonly code: GuardProtocolErrorCode;

  constructor(code: GuardProtocolErrorCode) {
    super(code);
    this.name = "GuardRequestError";
    this.code = code;
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function readRequestId(value: unknown): string | undefined {
  if (value === undefined) return undefined;
  if (typeof value !== "string" || value.length > MAX_REQUEST_ID_LENGTH) {
    throw new GuardRequestError("invalid_message");
  }
  return value;
}

function readSource(value: unknown): string {
  if (typeof value !== "string") {
    throw new GuardRequestError("invalid_message");
  }
  if (value.length > MAX_SOURCE_LENGTH) {
    throw new GuardRequestError("message_too_large");
  }
  return value;
}

function readOptionalBoolean(
  record: Record<string, unknown>,
  field: string,
): boolean | undefined {
  const value = record[field];
  if (value === undefined) return undefined;
  if (typeof value !== "boolean") {
    throw new GuardRequestError("invalid_message");
  }
  return value;
}

function readOptionalBoundedString(
  record: Record<string, unknown>,
  field: string,
  maximum: number,
): string | undefined {
  const value = record[field];
  if (value === undefined) return undefined;
  if (typeof value !== "string" || value.length > maximum) {
    throw new GuardRequestError("invalid_message");
  }
  return value;
}

function readTransform(value: unknown): GuardTransformRequest {
  if (!isRecord(value)) {
    throw new GuardRequestError("invalid_message");
  }
  switch (value.kind) {
    case "redact": {
      const replacement = readOptionalBoundedString(
        value,
        "replacement",
        MAX_REPLACEMENT_LENGTH,
      );
      return {
        kind: "redact",
        ...(replacement === undefined ? {} : { replacement }),
      };
    }
    case "template": {
      const namespace = readOptionalBoundedString(
        value,
        "namespace",
        MAX_NAMESPACE_LENGTH,
      );
      const numbered = readOptionalBoolean(value, "numbered");
      return {
        kind: "template",
        ...(namespace === undefined ? {} : { namespace }),
        ...(numbered === undefined ? {} : { numbered }),
      };
    }
    default:
      throw new GuardRequestError("unsupported_transform");
  }
}

/** Validates one untrusted `chrome.runtime` message. */
export function parseRequest(message: unknown): GuardRequest {
  if (!isRecord(message)) {
    throw new GuardRequestError("invalid_message");
  }
  const requestId = readRequestId(message.requestId);
  switch (message.type) {
    case "cribra.capabilities":
      return {
        type: "cribra.capabilities",
        ...(requestId === undefined ? {} : { requestId }),
      };
    case "cribra.scan": {
      const includeExplanations = readOptionalBoolean(
        message,
        "includeExplanations",
      );
      return {
        type: "cribra.scan",
        source: readSource(message.source),
        ...(requestId === undefined ? {} : { requestId }),
        ...(includeExplanations === undefined ? {} : { includeExplanations }),
      };
    }
    case "cribra.transform":
      return {
        type: "cribra.transform",
        source: readSource(message.source),
        transform: readTransform(message.transform),
        ...(requestId === undefined ? {} : { requestId }),
      };
    default:
      throw new GuardRequestError("invalid_message");
  }
}

function textRangeFor(source: string, span: Finding["span"]): TextRange {
  return byteSpanToTextRange(source, span);
}

export function projectScanResult(
  source: string,
  result: ScanResult,
): WireScanResult {
  return Object.freeze({
    ...result,
    findings: Object.freeze(
      result.findings.map((finding) =>
        Object.freeze({
          ...finding,
          textRange: textRangeFor(source, finding.span),
        }),
      ),
    ),
    candidates: Object.freeze(
      result.candidates.map((candidate) =>
        Object.freeze({
          ...candidate,
          textRange: textRangeFor(source, candidate.span),
        }),
      ),
    ),
  });
}

export function projectTransformResult(
  source: string,
  result: TransformResult,
): WireScanResult & { readonly output: string } {
  return Object.freeze({
    ...projectScanResult(source, result.scan),
    output: result.output,
  });
}

/** Converts the extension-facing transform to the public Flow request. */
export function toPublicTransform(
  request: GuardTransformRequest,
): TransformRequest {
  return request;
}

/**
 * Produces a fixed, source-free diagnostic. Unknown failures never expose
 * their message or stack; Cribra's own messages are already caller-free.
 */
export function toWireError(error: unknown, requestId?: string): ErrorResponse {
  if (error instanceof CribraError) {
    return {
      ok: false,
      type: "cribra.error",
      ...(requestId === undefined ? {} : { requestId }),
      error: {
        code: error.code,
        domain: error.domain,
        message: error.message,
        ...(error.ruleId === undefined ? {} : { ruleId: error.ruleId }),
      },
    };
  }
  if (error instanceof GuardRequestError) {
    return {
      ok: false,
      type: "cribra.error",
      ...(requestId === undefined ? {} : { requestId }),
      error: {
        code: error.code,
        domain: "extension",
        message: messageForGuardError(error.code),
      },
    };
  }
  return {
    ok: false,
    type: "cribra.error",
    ...(requestId === undefined ? {} : { requestId }),
    error: {
      code: "internal_error",
      domain: "extension",
      message: "The extension could not complete the request.",
    },
  };
}

function messageForGuardError(code: GuardProtocolErrorCode): string {
  switch (code) {
    case "invalid_message":
      return "The extension request is malformed.";
    case "message_too_large":
      return "The extension request exceeds the supported source size.";
    case "unsupported_transform":
      return "The extension does not support this transform.";
    case "forbidden_sender":
      return "The sender is not allowed to use the Cribra host.";
  }
}
