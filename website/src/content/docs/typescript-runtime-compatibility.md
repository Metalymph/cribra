---
title: "TypeScript runtime compatibility"
description: "Tested Node, browser, and bundler environments for the approved, unpublished @metalymph/cribra package, with explicit claim boundaries."
order: 19
---

# TypeScript runtime compatibility

The `@metalymph/cribra` package is an **implemented and locally qualified**
binding against the Cribra WebAssembly backend. It is **not published**. The
manifest is `@metalymph/cribra@0.1.0`, not private, with
`cribraRelease.state` set to `"approved"`. The project has not published the
package, no npm registry record exists, and `@metalymph/cribra` is not yet an
available registry installation instruction.

"Implemented", "qualified from an installed package", and "published" are
separate claims. The evidence below comes from packed tarballs installed into
isolated consumers. It is not a promise of support for every version in a
declared range.

## Qualified environment matrix

| Area | Recorded qualification |
| --- | --- |
| Node ESM | Node 22.22.0, 24.22.2, and 26.13.0 for Flow and the test-only Core artifact. Default asset plus explicit bytes, `ArrayBuffer`, `file:` URL, path, and HTTP URL initialization. |
| TypeScript | 5.9.3 and 7.0.2 with `NodeNext` and `Bundler` module resolution. The public declaration closure is checked. |
| Browser | Google Chrome 156.0.8078.74, Playwright Chromium 156.0.8078.4, Playwright Firefox 157.0, and Playwright WebKit 27.2 for Flow. Chrome for the test-only Core artifact. |
| Bundlers | Vite 8.3.6 in development and production; webpack 5.112.2 in production. Both import the package by name. |
| WASM delivery | Emitted hashed URL, `application/wasm` response, and real scan, transform, and Share Bundle work in the browser. |
| Native Safari | **Unverified.** Remote automation was not enabled on the qualification host. |

The `engines.node` field declares `>=22`. That is a minimum compatibility
declaration, not evidence for odd-numbered Node releases or every 22.x, 24.x, or
26.x patch. The recorded versions are the tested versions.

## Browser claim boundaries

- **Chrome** means Google Chrome 156.0.8078.74. **Playwright Chromium** is a
  separate browser build and is listed separately.
- **Playwright Firefox 157.0** is not evidence for branded Firefox.
- **Playwright WebKit 27.2** is not native Safari. It exercises a WebKit build
  through Playwright, not Apple's Safari application or its host integration.
- **Native Safari remains unverified.** Do not describe this package as
  Safari-qualified.
- Browser storage, Workers, service workers, CSP, and UI policy remain the
  integrating application's responsibility.

## Bundler claim boundaries

Vite 8.3.6 and webpack 5.112.2 are qualified only for the recorded
configurations. Direct Rollup and esbuild consumption is **not** separately
qualified, even though those tools participate in other build pipelines. Do not
generalize Vite or webpack evidence into a universal bundler claim.

Import the package by name and let the `exports` conditions select the Node or
browser entry. Do not copy WASM glue or binary files into application source as
a substitute for resolving the installed package. Vite's dependency optimizer
worked in qualification; `optimizeDeps.exclude` is a consumer-side workaround
only if a future Vite release changes that behavior.

## Entry points and initialization

The package is ESM-only and side-effect-free at import. The `node` condition
selects `dist/index.node.js`; the `browser` condition selects
`dist/index.browser.js`; the default entry is browser-safe. Only the Node entry
imports Node built-ins.

With no argument, `init()` resolves the packaged WASM asset relative to the
package. Consumers may provide explicit bytes or a URL instead:

```ts
import { init } from "@metalymph/cribra";

await init();

// Explicit asset fallback:
await init({ wasm: new URL("./cribra_bg.wasm", import.meta.url) });
```

`init()` is idempotent and concurrency-safe. A load or instantiation failure
throws `CribraError("wasm_load_failed")` or
`CribraError("wasm_instantiate_failed")`; a retry is possible when initialization
has not become fatal. Once initialized, `createScanner()` and all scanning
operations are synchronous.

## Distribution and Core boundary

Only the Flow package may become public. The Flow build includes scanning,
batches, admitted streaming, incremental documents, the four transformations,
and Share Bundles. A test-only Core tarball exists for qualification and is
never distributed; Core rejects transformations and Share Bundles with
`CribraError("unsupported_capability")`.

The package is currently available only from a source checkout or a locally
packed tarball. See the [quick start](/docs/typescript/) for local build and
install commands. Do not infer registry availability or a successful publication
from this documentation or from the `cribraRelease` configuration; npm ownership
was verified separately and the package remains unpublished.

## Not claimed

- React Native through this WebAssembly package, or a native Node.js TypeScript
  backend;
- multi-threaded or parallel WebAssembly;
- native Safari execution;
- branded Firefox qualification;
- direct Rollup or esbuild consumption;
- universal Node-version or browser-version support;
- npm publication or staging of `@metalymph/cribra`; authenticated ownership is
  verified but a registry release has not happened.

The future native Node.js TypeScript backend remains planned and separately
gated. It is not part of this WebAssembly package. React Native has its own
native package and qualification; see the [React Native integration
guide](/docs/integration-react-native/).

## Related guides

- [Quick start](/docs/typescript/)
- [API reference](/docs/typescript-api-reference/)
- [Sessions and transformations](/docs/typescript-sessions-and-transformations/)
- [WebAssembly integration](/docs/integration-wasm/)
- [React Native integration](/docs/integration-react-native/)
- [Compatibility and distribution](/docs/compatibility-and-distribution/)
- [Language integrations](/bindings/)
