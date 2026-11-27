/**
 * Classic/IIFE content-script entry point.
 *
 * The script installs a synchronous document-level submit interceptor. It does
 * not instantiate WASM, load remote code, patch page-world prototypes, use
 * storage, or perform network requests. Intercepted operations remain blocked.
 *
 * The qualification hook is compiled into the separate content qualification
 * package only. The ordinary release build does not emit this content script.
 */

import { AdapterRegistry } from "./registry.js";
import { SnapshotStore } from "./snapshot.js";
import { SubmissionInterceptor } from "./interceptor.js";
import { installQualificationHook } from "./qualification.js";

declare const __CRIBRA_CONTENT_QUALIFICATION__: boolean;

function boot(): void {
  const globalObject = globalThis as unknown as {
    __cribraGuardContentBooted?: boolean;
  };
  if (globalObject.__cribraGuardContentBooted === true) return;
  globalObject.__cribraGuardContentBooted = true;

  const interceptor = new SubmissionInterceptor({
    registry: new AdapterRegistry(),
    store: new SnapshotStore(),
    document,
  });
  interceptor.install();

  if (
    typeof __CRIBRA_CONTENT_QUALIFICATION__ !== "undefined" &&
    __CRIBRA_CONTENT_QUALIFICATION__
  ) {
    installQualificationHook(interceptor);
  }
}

if (typeof document !== "undefined") {
  boot();
}
