# Cribra Guard — MV3 Flow host qualification

Private WEBEXT.1A workspace. It is not published, not a Guard UI, and not a
general browser interception layer.

This package proves one architecture: a Chromium Manifest V3 extension service
worker can explicitly initialize the existing Cribra Flow WebAssembly backend,
scan synthetic sources, redact/template them, and recover safely across worker
lifecycle events.

Every detected finding or candidate still requires user review. This milestone
does not claim universal pre-send interception.

## What is here

| Path | Purpose |
| --- | --- |
| `manifest.json` | MV3 manifest, module service worker, empty permissions, WASM-capable extension CSP |
| `src/service-worker.ts` | Explicit local asset load, pinned-hash verification, Flow init, scanner lifecycle, message handling |
| `src/protocol.ts` | Strict message validation, UTF-8 span plus UTF-16 text-range projection, sanitized errors |
| `src/qualification.ts` | Qualification-only extension page bridge for real `chrome.runtime.sendMessage` calls |
| `scripts/build.mjs` | Rebuilds the Flow artifact from Rust source, verifies `artifact.lock.json`, bundles the extension (release-safe by default; `--with-qualification` adds the test page) |
| `scripts/prepare.mjs` | Refreshes the ignored wrapper version/declaration surface before `tsc` |
| `scripts/qualify.mjs` | Real Chromium MV3 qualification, lifecycle tests, failure injection, memory and network evidence |
| `artifact.lock.json` | Pinned production WASM and glue hashes plus toolchain versions |

There is no content script, page adapter, approval UI, offscreen document,
telemetry, or persistence.

## Artifact provenance

The extension never reads the WASM or glue from `packages/cribra/src/wasm` or
`packages/cribra/dist`. Both are generated locations and can hold different
builds. `scripts/build.mjs` always builds the reviewed Rust source into
`.build/flow`, applies `wasm-opt -Oz`, and verifies the result against
`artifact.lock.json`.

- Source crate: `cribra-wasm` 0.4.7, Core `cribra` 0.7.2
- Features: `transform` (the Flow profile)
- `wasm-bindgen`: 0.2.126, exactly as pinned by `Cargo.lock`
- `wasm-opt`: 133, `-Oz`
- Production WASM: 1,422,511 bytes
  - SHA-256 `a7e11b118d1ab52b23db41d9ea9826fb564907b25e33cfb1ea5c8833e911f9c3`
- Raw wasm-bindgen WASM before `-Oz`: 1,820,743 bytes
  - SHA-256 `b0edc6aa008f8a2cb4bb27f8939445d67cf1951c636b0e5e500eb8e55d506e7f`
- Generated glue: 94,659 bytes
  - SHA-256 `2583d2a30c286f725d46283c8cbdb6245d97a52b492f458c5eefdbfcb01467cb`

The service worker fetches only
`chrome-extension://<id>/wasm/cribra_bg.wasm`, verifies its byte length and
SHA-256 against constants injected at build time, and passes the bytes to
`init({ wasm })`. A mismatched or missing asset fails closed.

See `PROVENANCE.md` for the complete record and reproducibility note.

## Build

Prerequisites:

- Node.js 22 or newer (the qualification run used Node.js 26.13.1)
- Rust toolchain with the `wasm32-unknown-unknown` target
- `wasm-bindgen-cli` 0.2.126
- Binaryen `wasm-opt` 133
- Playwright's Chromium build: `npx playwright-core install chromium`

The qualification harness uses Playwright `channel: "chromium"` in headless new
mode. The headless shell does not load extensions, so do not install with
`--only-shell`.

```sh
cd packages/cribra-guard
npm install

# Release-safe extension: no qualification page or bridge.
npm run build

# Qualification build: adds the test-only page and bridge.
npm run build:qualification
npm run check
npm run qualify
```

`npm run qa` runs the qualification build, type-check, and qualification
together. The ordinary `npm run build` output is the package shape intended for
future release packaging; it contains no test harness.

CI: `.github/workflows/cribra-guard.yml` runs the same procedure on Ubuntu with
pinned Rust 1.100.0, wasm-bindgen 0.2.126, Binaryen 133, Playwright 1.64.1 and
Node.js 24, then verifies that the release build excludes the test harness.

The build also regenerates the wrapper's ignored
`src/generated/package-version.ts` and copies the matching generated glue
declarations into the ignored `packages/cribra/src/wasm` directory. This lets
`tsc` check the reviewed wrapper source without a prior `@metalymph/cribra`
package build. No tracked Cribra source is changed.

A local bindgen install under `packages/cribra-guard/.tools` is used when
present; otherwise `wasm-bindgen` must be on `PATH`. The build refuses a
version that does not match `Cargo.lock`.

To intentionally accept a new artifact, rebuild and update the lock:

```sh
node scripts/build.mjs --write-lock
```

The lock is verified on every ordinary `npm run build`. The build does not
require a published npm package.

## Load the unpacked extension

1. Open `chrome://extensions`.
2. Enable Developer mode.
3. Choose **Load unpacked** and select `packages/cribra-guard/dist`.
4. The extension has no UI. Open the service worker entry to inspect it.

`npm run build` produces a release-safe package with no qualification harness.
To load the harness manually, run `npm run build:qualification` first. The page
at `chrome-extension://<id>/qualification.html` exists only for automated tests.
It is not registered in the manifest and is not a Guard interface.

## Message protocol

All requests are sent with `chrome.runtime.sendMessage` from an extension page.
The worker rejects messages whose sender is not the same extension origin.

```ts
{ type: "cribra.capabilities", requestId?: string }
{ type: "cribra.scan", requestId?: string, source: string, includeExplanations?: boolean }
{ type: "cribra.transform", requestId?: string, source: string,
  transform: { kind: "redact", replacement?: string }
           | { kind: "template", namespace?: string, numbered?: boolean } }
```

Successful scan findings carry both:

- `span`: half-open UTF-8 byte offsets from Cribra
- `textRange`: half-open UTF-16 code-unit offsets for `String.prototype.slice`

Consumers must never use a byte span as a JavaScript string index.

Responses never contain source text, matched values, or raw diagnostics.
Failures use fixed messages such as
`The Cribra WebAssembly module could not be initialized.` with a structured
`code` and `domain`. Unknown failures become a fixed `internal_error`.

Source input is capped at 1,000,000 UTF-16 code units for this qualification
host. Oversized requests are rejected before scanning.

## Qualification

`npm run qualify` launches Chrome for Testing 156 through Playwright, loads the
unpacked qualification extension in a persistent Chromium profile, and drives
the real MV3 service worker through `chrome.runtime.sendMessage`. It requires
`npm run build:qualification` first; `npm run qa` performs both.

The report is written to `.build/qualification/report.json`. The current run
passes all 72 checks.

Coverage:

- Manifest V3, module service worker, empty permissions, extension CSP with
  `'wasm-unsafe-eval'`
- Flow capability manifest, pinned artifact hash, scanner construction
- Synthetic scan metadata, UTF-8 byte spans versus UTF-16 text ranges
- Unicode source with emoji, accented Latin, and CJK characters
- Redaction, templating, and clean rescans of transformed output
- Unpaired-surrogate offset failure, malformed messages, oversized source,
  unsupported transforms, and sanitized errors
- 16 concurrent scans and transforms with correlated request IDs
- 200 warm scans, 50 warm transforms, 1,500 scans for heap-retention evidence
- Forced `ServiceWorker.stopWorker`, offline cold reinitialization, in-flight
  termination, and page disconnect recovery
- Corrupt, unsupported, tampered, and missing WASM failure injection
- No storage API, no IndexedDB records, no cache entries, and no scanned value in
  the extension profile storage
- No remote HTTP(S) request, no remote code, no `eval`, no `new Function`, no
  `importScripts`

Failure injection results:

| Variant | Result |
| --- | --- |
| Corrupt bytes with matching build hash | `wasm_instantiate_failed` |
| Valid empty module with missing exports | `wasm_instantiate_failed` |
| Tampered packaged WASM | `wasm_load_failed` |
| Missing packaged WASM | `wasm_load_failed` |

Latest measurements (Chrome for Testing 156.0.8078.4, macOS arm64):

| Measurement | Result |
| --- | --- |
| WASM | 1,422,511 bytes; 512,990 bytes gzip |
| Release extension (unpacked) | 1,564,025 bytes |
| Release extension (`zip -9 -r`) | 540,347 bytes |
| Qualification extension (unpacked) | 1,565,019 bytes |
| Qualification extension (`zip -9 -r`) | 541,188 bytes |
| Launch to first response | 1,368 ms (includes Playwright launch) |
| Worker Flow init | 39.4 ms |
| Scanner construction | 210.4 ms |
| First scan | 5.2 ms (worker-measured) |
| Warm scan p50 / p95 | 0.0 ms / 0.1 ms |
| Warm scan round-trip mean | 0.193 ms |
| Warm transform p50 / p95 | 0.0 ms / 0.1 ms |
| Warm transform round-trip mean | 0.190 ms |
| WASM linear memory (declared initial) | 23 pages, 1,507,328 bytes |
| V8 worker heap after 300 scans | 936,532 bytes |
| V8 worker heap after 1,500 scans | 950,820 bytes |
| Heap retention delta over 1,200 scans | 14,288 bytes |
| Forced stop | 101.5 ms |
| Offline cold reinit | 6.9 ms |

Release figures come from a separate `npm run build`; qualification figures
include the 854-byte test page and bridge. The report records both.

The heap figures come from CDP `Runtime.getHeapUsage` after a forced GC and
cover the worker's V8 heap only. The module declares 23 initial linear-memory
pages (1.44 MiB); that memory and browser engine overhead are separate. A rough
working set is therefore the JS heap plus roughly 1.5 MiB of WASM memory at
instantiation, growing as the scanner allocates.

Worker-measured durations are short because `performance.now()` is coarsened in
this context and the synthetic source is small. The report is authoritative for
the exact run.

## Known limitations

- Every finding or candidate requires user review. The host does not claim
  universal pre-send interception.
- A source containing an unpaired UTF-16 surrogate and a finding fails with
  `invalid_unicode_string` during offset projection rather than returning a
  partial result.
- Playwright attaches to the service worker, so natural idle suspension is not
  observed. Restart is exercised with `ServiceWorker.stopWorker`; this is
  termination plus cold reinitialization, not an idle-timeout measurement.
- Runtime network capture covers page requests. Worker remote-code/network
  evidence is the static bundle audit plus the offline cold reinitialization.
- `chrome.runtime.onSuspend` disposal is best effort. The next request also
  checks `scanner.disposed` and recreates the scanner.
- The qualification page and message bridge are emitted only by
  `npm run build:qualification`; the ordinary release build omits them.
- No content script, page adapter, approval UI, offscreen document, telemetry,
  backend, or publication is included.
