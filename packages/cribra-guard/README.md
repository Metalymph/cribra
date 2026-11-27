# Cribra Guard — MV3 Flow host and content-layer qualification

Private WEBEXT.1A/1B workspace. It is not published, not a Guard UI, and not a
general browser interception layer.

This package proves two architectures:

1. a Chromium Manifest V3 extension service worker can explicitly initialize the
   existing Cribra Flow WebAssembly backend, scan synthetic sources,
   redact/template them, and recover safely across worker lifecycle events;
2. a qualification-only classic content script can synchronously intercept
   explicitly recognized submission paths, capture exact editor snapshots, and
   perform controlled, read-back-verified editor replacement without claiming
   universal interception or protection.

Every detected finding or candidate still requires user review. The content
layer is a fixture-qualified adapter proof, not an authorization controller or
approval UI.

## What is here

| Path | Purpose |
| --- | --- |
| `manifest.json` | Production MV3 manifest, module service worker, empty permissions, WASM-capable extension CSP. No content scripts. |
| `manifest.qualification.json` | Qualification-only manifest: one content script at `document_start`, access only to `http://127.0.0.1/*` |
| `src/service-worker.ts` | Explicit local asset load, pinned-hash verification, Flow init, scanner lifecycle, message handling |
| `src/protocol.ts` | Strict message validation, UTF-8 span plus UTF-16 text-range projection, sanitized errors |
| `src/guard/` | Pure deterministic Guard policy and submission state machine (WEBEXT.1B.1) |
| `src/content/` | Typed page-adapter contract, fixture adapters, snapshot store, editor read/write, synchronous interceptor, qualification hook |
| `src/qualification.ts` | Qualification-only extension page bridge for real `chrome.runtime.sendMessage` calls |
| `scripts/build.mjs` | Rebuilds the Flow artifact, verifies `artifact.lock.json`, and emits either the release/WASM qualification package or the separate content qualification package |
| `scripts/prepare.mjs` | Refreshes the ignored wrapper version/declaration surface before `tsc` |
| `scripts/qualify.mjs` | Real Chromium MV3 WASM-host qualification, lifecycle tests, failure injection, memory and network evidence |
| `scripts/qualify-content.mjs` | Real Chromium extension-context content qualification against local fixtures and endpoint receipts |
| `scripts/test-guard.mjs` | Deterministic pure-Guard unit tests |
| `scripts/test-content.mjs` | Deterministic content-layer unit tests with a tiny in-memory DOM |
| `scripts/packaging.test.mjs` | Release/qualification packaging-boundary regression tests |
| `artifact.lock.json` | Per-host pinned production WASM and glue hashes plus toolchain versions |

The production package contains no content script, page adapter, approval UI,
offscreen document, telemetry, or persistence. The content script, fixtures,
and isolated-world test hook are emitted only into `.build/content-dist/`.

## Artifact provenance

The extension never reads the WASM or glue from `packages/cribra/src/wasm` or
`packages/cribra/dist`. Both are generated locations and can hold different
builds. `scripts/build.mjs` always builds the reviewed Rust source into
`.build/flow`, applies `wasm-opt -Oz --strip-producers`, and verifies the
result against `artifact.lock.json`. The build sets `--remap-path-prefix` for
the Cargo registry, workspace, and rust-src locations so host paths do not leak
into the artifact.

- Source crate: `cribra-wasm` 0.4.7, Core `cribra` 0.7.2
- Features: `transform` (the Flow profile)
- `wasm-bindgen`: 0.2.126, exactly as pinned by `Cargo.lock`
- `wasm-opt`: 133, `-Oz --strip-producers`
- Registry and toolchain source paths are remapped; the build pins one
  production digest per build host because rustc's crate disambiguators and
  symbol ordering are host-specific.
- Production WASM (`darwin-arm64`): 1,420,144 bytes
  - SHA-256 `b3706880cbe172f1ad471bbbd1b3eb54ff38af1eb0b7d77c0130e327f1d7e990`
- Production WASM (`linux-x64`, CI): 1,420,144 bytes
  - SHA-256 `a5bd3f0b498fedc62f5af1b91e44693ac2417b07eddfbba81cdede3356fccd4a`
- Raw wasm-bindgen WASM before `-Oz` (`darwin-arm64`): 1,818,487 bytes
  - SHA-256 `3563d087ec2288358ade5fae58c002832727361a8e6002b4c0af8c435facfa06`
- Generated glue: 94,659 bytes
  - SHA-256 `2583d2a30c286f725d46283c8cbdb6245d97a52b492f458c5eefdbfcb01467cb`

`artifact.lock.json` verifies the digest for the current host. An unknown host
fails closed until `--write-lock` records its digest. The Linux CI digest is the
canonical release-build digest; local macOS builds are qualified against their
own pinned entry.

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

# Release-safe extension: no qualification page, bridge, or content script.
npm run build

# WASM-host qualification build: adds the test-only page and bridge to dist/.
npm run build:qualification

# Separate content qualification package in .build/content-dist/.
# Reuses and re-verifies the existing .build/flow artifact when present;
# otherwise it performs a normal verified build.
npm run build:content-qualification

npm run check
npm run test:lock
npm run test:packaging
npm run test:guard
npm run test:content
npm run qualify
npm run qualify:content
```

`npm run qa` runs both qualification builds, strict type-checks both source and
test surfaces, the lock and packaging regressions, the pure Guard tests, the
content-layer unit tests, the existing Chromium MV3 WASM qualification, and the
new Chromium extension-context content qualification. The ordinary
`npm run build` output is the package shape intended for future release
packaging; it contains no test harness and no content script.

A normal build requires a matching `artifact.lock.json` entry for the current
host. Missing locks, unknown hosts, and digest mismatches fail before any
output is written. Test-only harness builds that skip or override the artifact
must pass `--skip-build --allow-unverified`; there is no implicit bypass. A
content qualification build reuses `.build/flow` only when it can still verify
the pinned production digest and glue against the lock.

CI: `.github/workflows/cribra-guard.yml` runs the same procedure on Ubuntu with
pinned Rust 1.100.0, wasm-bindgen 0.2.126, Binaryen 133, Playwright 1.64.1 and
Node.js 24, then verifies that the release build excludes the test harness. The
Linux build verifies the `linux-x64` lock entry and is the canonical release
artifact.

The build also regenerates the wrapper's ignored
`src/generated/package-version.ts` and copies the matching generated glue
declarations into the ignored `packages/cribra/src/wasm` directory. This lets
`tsc` check the reviewed wrapper source without a prior `@metalymph/cribra`
package build. No tracked Cribra source is changed.

A local bindgen install under `packages/cribra-guard/.tools` is used when
present; otherwise `wasm-bindgen` must be on `PATH`. The build refuses a
version that does not match `Cargo.lock`.

To intentionally accept a new artifact, rebuild and update the current host's
lock entry:

```sh
node scripts/build.mjs --write-lock
```

Other hosts' entries are preserved. The lock is verified on every ordinary
`npm run build`, and an unknown host fails closed. The build does not require a
published npm package.

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

## Content layer (qualification only)

The content layer lives under `src/content/`. It does not import the Guard state
machine, the scanner, WASM, storage, or any messaging controller. It is built
only into `.build/content-dist/`.

### Adapter contract

Each page adapter owns exactly one explicitly recognized submission shape:

| Type | Meaning |
| --- | --- |
| `SupportedCoverage` | The form and designated editor are recognized and writable |
| `UnsupportedCoverage` | The form/editor is outside this adapter's scope |
| `DegradedCoverage` | The editor is recognized but not safely writable |
| `InvalidatedCoverage` | A previously recognized path lost its editor, form, or adapter identity |

Adapters expose identity/version, coverage detection, submitter support, exact
source read, controlled replacement, and focus. Capture results are a separate
source-free discriminated union: `captured`, `duplicate`, `invalidated`,
`degraded`, `not_cancellable`, and `unsupported_ignored`.

### Supported and unsupported paths

Supported fixture paths:

- A marked native form with exactly one designated `textarea` or ordinary
  `text`/`search` input. Click, keyboard, and `requestSubmit()` are intercepted
  synchronously; multiple submit buttons, validation, repeated attempts, editor
  replacement, and form replacement are qualified.
- One marked plain-text `contenteditable`. The logical source is exact
  `textContent`; replacement uses `textContent` only, never `innerHTML`.

Explicitly unsupported or bypassing paths:

- Unmarked forms and arbitrary fields are untouched.
- Password and hidden designated editors are rejected.
- Rich-text editors, canvas editors, closed shadow roots, framework editor
  models, and multiple eligible editors are unsupported.
- `form.submit()` bypasses the submit event.
- Direct `fetch`/XHR, `sendBeacon`, autosave, and other page network channels are
  outside DOM interception.
- Non-cancellable events are reported, not claimed as blocked.
- A page `window` capture handler runs before document capture.
- A previously recognized form that loses its editor, marker, or writability
  stays blocked while it can still be intercepted; if the form itself is
  replaced, a coverage-loss report is emitted instead of claiming protection.

### Snapshot invariants

- Equality is exact JavaScript string equality, never normalized text.
- Snapshot identifiers are opaque, counter-plus-random, and never reused.
- A snapshot binds to the adapter, editor instance, form instance, document,
  and exact source.
- Observable `input`/`change` events and pending `MutationObserver` records
  invalidate associated snapshots before final verification.
- Invalidation is sticky: A-to-B-to-A does not revive the A snapshot, and an
  invalidated identifier cannot be reused.
- Editor and form replacement invalidate associated snapshots.
- Pending captures have an explicit `release` path and are bounded; the store
  evicts oldest invalid/released records first.
- Source is limited to 1,000,000 UTF-16 code units per snapshot, matching the
  scanner protocol limit, and 4,000,000 aggregate retained code units across
  valid snapshots. An oversized intercepted submission stays blocked with no
  snapshot and no pending editor. Eviction, release, invalidation, suspension,
  and disposal clear retained source references.
- Source is transient in memory only. It is never logged, stored, or returned by
  the qualification hook.
- Silent programmatic changes are detected through explicit read-back where
  observable. A silent transient change that returns to the original text
  before read-back is not observable and is documented as a limitation.

### Browser lifecycle

- A non-persisted `pagehide` disposes the interceptor. A new document receives
  a fresh content script on the next navigation.
- A persisted `pagehide` (Back/Forward Cache) invalidates every snapshot, clears
  retained source, and keeps the submit listener installed. A persisted
  `pageshow` keeps automatic interception active; a fresh capture is required.
- Explicit `dispose` or non-persisted teardown leaves `installed()` false. The
  implementation does not claim protection while inactive.
- The qualification exercises synthetic persisted `pagehide`/`pageshow` events
  and real back navigation. Whether Chromium actually restores a particular page
  from bfcache is browser/profile-controlled; if it does not restore, a fresh
  content script is injected. Both paths are qualified as protected.

### Controlled replacement

- Native controls use the prototype `value` setter, then dispatch `input` and
  `change`.
- Contenteditable uses `textContent` only.
- Success requires exact read-back equality. Detached, read-only, disabled,
  normalized, partial, or mismatched writes are failures.
- Unicode, surrogate pairs, multiline textarea content, and empty text are
  qualified.
- A successful replacement always invalidates the previous source snapshot. The
  future controller must establish a new snapshot and perform transformation
  revalidation. This milestone never submits after writing.

### Coverage accuracy

Qualification covers only the designated editor text and the tested submission
path. It does not claim protection for other form fields, entire serialized HTTP
request bodies, hidden or password fields, attachments, or page-controlled
autosave/network channels. The sensitive fixture demonstrates that browser form
serialization can include more fields than the exact editor snapshot.

### Trust boundary and packaging isolation

- The page DOM, fixture markers (`data-cribra-adapter`, `data-cribra-editor`),
  page events, and page messages are never proof of consent, browser identity,
  or authorization.
- Capture results are source-free diagnostics. The future controller must
  authenticate extension senders, derive tab/frame/adapter identity from trusted
  browser APIs, and bind authorization to exact snapshots.
- The qualification manifest grants access only to `http://127.0.0.1/*`, uses no
  `scripting`, `debugger`, `webRequestBlocking`, `storage`, or `tabs`
  permission, and is never emitted into the production package.
- The isolated-world qualification hook is installed only in
  `.build/content-dist/`. It is unreachable from the page and returns only
  metadata, booleans, and replacement status.
- Production `dist/` has empty permissions, empty host permissions, no content
  scripts, no web-accessible resources, and no content script file. The
  production WASM and glue digests remain pinned and unchanged.

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
| WASM | 1,420,144 bytes; 512,561 bytes gzip |
| Release extension (unpacked) | 1,561,769 bytes |
| Release extension (`zip -9 -r`) | 540,046 bytes |
| Qualification extension (unpacked) | 1,562,763 bytes |
| Qualification extension (`zip -9 -r`) | 540,888 bytes |
| Launch to first response | 3,093 ms (includes Playwright launch) |
| Worker Flow init | 11.3 ms |
| Scanner construction | 212.0 ms |
| First scan | 8.9 ms (worker-measured) |
| Warm scan p50 / p95 | 0.0 ms / 0.1 ms |
| Warm scan round-trip mean | 0.180 ms |
| Warm transform p50 / p95 | 0.0 ms / 0.1 ms |
| Warm transform round-trip mean | 0.194 ms |
| WASM linear memory (declared initial) | 23 pages, 1,507,328 bytes |
| V8 worker heap after 300 scans | 936,532 bytes |
| V8 worker heap after 1,500 scans | 945,328 bytes |
| Heap retention delta over 1,200 scans | 8,796 bytes |
| Forced stop | 101.8 ms |
| Offline cold reinit | 7.3 ms |

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

### Content qualification

`npm run qualify:content` launches the separate `.build/content-dist/` unpacked
extension in Chromium and serves local fixtures from `http://127.0.0.1`. It
reaches the isolated-world hook through CDP and proves interception with
fixture-side handler counters and local endpoint receipts.

The current run passes all 153 checks. Additional deterministic gates pass
content unit tests **55/55** and packaging regressions **7/7**. Coverage
includes:

- classic IIFE content script, `document_start`, main frame only,
  localhost-only manifest, no `scripting`/`debugger`/`webRequestBlocking`/
  `storage`/`tabs` permissions;
- native textarea and text-input click, keyboard, and `requestSubmit()` paths;
- duplicate suppression, explicit release, and recapture with new identifiers;
- multiple submit buttons, validation, and repeated attempts;
- a sensitive fixture where only the designated editor is snapshotted while the
  browser form serialization still contains password, hidden, and unrelated
  fields;
- contenteditable exact `textContent`, Unicode/surrogates, multiline, empty
  text, input/change notifications, and controlled replacement;
- editor and form replacement, invalidation, coverage-loss reporting, and
  unmarked replacement forms remaining unsupported;
- A-to-B-to-A sticky invalidation, pending `MutationObserver` flush, silent
  programmatic read-back detection, and the documented silent transient limit;
- listener installation/cleanup, navigation teardown, synthetic persisted
  pagehide/pageshow, real back navigation, multiple independent tabs, and
  bounded snapshot storage;
- oversized source remaining blocked with no snapshot or pending editor;
- `form.submit()`, direct fetch, autosave, earlier window handlers, and
  non-cancellable events as explicit limitations;
- no source text in console output, capture results, metadata, server receipts,
  or extension profile storage.

The report is written to `.build/content-qualification/report.json`.

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
- The production package contains no content script or page adapter. The
  content layer is a qualification-only fixture proof; consent, approval UI,
  authenticated controller messaging, transformation orchestration, and
  authorized resubmission belong to later milestones.
- Content interception cannot stop direct fetch/XHR, `form.submit()`,
  non-cancellable events, earlier window capture handlers, autosave, or
  unsupported editors. It does not claim universal protection.
- DOM observation cannot detect every transient or silent mutation. A silent
  change that returns to the original text before read-back is not observable.
- Snapshot coverage is the exact designated editor text, not the entire browser
  form serialization, attachments, hidden or password fields, or
  page-controlled network channels.
- Source retention is bounded to 1,000,000 UTF-16 code units per snapshot and
  4,000,000 aggregate code units. Oversized intercepted content stays blocked;
  it is never silently submitted.
- Real Back/Forward Cache eligibility is browser/profile-controlled. Persisted
  pagehide/pageshow and real back navigation are qualified, but the test does
  not force a bfcache restore. If the page is restored without bfcache, a fresh
  content script is injected.
- Adapter detection and read failures are fail-closed: a cancelable submit is
  cancelled and reported as invalidated rather than risk a silent bypass. A
  faulty adapter can therefore over-block a form, but it cannot fail open.
- No approval UI, offscreen document, telemetry, backend, or publication is
  included.
