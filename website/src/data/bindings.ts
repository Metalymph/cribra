/** Repository evidence snapshot, not a live package-registry lookup. */
export const evidenceDate = '2026-11-07';
export const repository = 'https://github.com/Metalymph/cribra';

/** A labeled destination that is safe to render as a link. */
export interface BindingLink {
  label: string;
  href: string;
}

/** One independently evidenced integration surface. */
export interface Binding {
  id: string;
  name: string;
  /** Primary integration guide on this website. */
  guide: BindingLink;
  /** Additional existing guide pages for the same integration. */
  additionalGuides?: BindingLink[];
  /** Generated or hand-written API reference, when one exists. */
  apiReference?: BindingLink;
  /** Published package, release, or repository distribution entry. */
  package?: BindingLink;
  /** Discovery/registry listing, never compatibility-build evidence. */
  registry?: BindingLink;
  implementation: string;
  capabilities: string;
  qualification: string;
  limits: string;
  distribution: string;
  sources: BindingLink[];
}

const source = (label: string, path: string) => ({
  label,
  href: `${repository}/blob/main/${path}`,
});
const compatibility = source('Compatibility audit', 'docs/COMPATIBILITY.md');
const engineCi = 'https://github.com/Metalymph/cribra/actions/runs/39411373054';

/** Shared by the detailed matrix and homepage integration list. */
export const bindings: readonly Binding[] = [
  {
    id: 'rust',
    name: 'Rust',
    guide: { label: 'Rust integration guide', href: '/docs/integration-rust/' },
    apiReference: { label: 'Rust API overview', href: '/docs/api-reference/' },
    package: { label: 'cribra 0.7.2 release', href: `${repository}/releases/tag/v0.7.2` },
    implementation: 'Available · cribra 0.7.2 · MSRV 1.97',
    capabilities:
      'Core: scanning, reports, batch, admitted streaming and incremental documents. Flow (default): four transforms and Share Bundles. Fuse: Flow plus Serde and parallel independent-source scans. These are Cargo feature combinations, not features named core/flow/fuse.',
    qualification:
      'Recorded engine CI: Linux full gate, native Linux ARM64, RISC-V64 under QEMU, and macOS/Windows tests. Local minimal/default/all-features public API contracts and three independent Cargo consumers passed.',
    limits:
      'Recorded environments are evidence, not a promise for every platform. Whole-source execution is the semantic reference.',
    distribution:
      'Core 0.7.2 GitHub release and crates.io publication independently verified by the repository audit.',
    sources: [
      compatibility,
      { label: 'Engine CI receipt', href: engineCi },
      source('API entry points', 'src/lib.rs'),
    ],
  },
  {
    id: 'c',
    name: 'C ABI',
    guide: { label: 'C ABI integration guide', href: '/docs/integration-c/' },
    implementation: 'Available · cribra-capi 0.3.0 · ABI protocol 0.3.0',
    capabilities:
      'Scanner configuration, metadata, findings/review, batch, admitted byte streaming, incremental snapshots, four transformations and in-memory Share Bundles. AUTO batches may use parallel scanning only with the adapter parallel feature.',
    qualification:
      'Native C ABI gates recorded on Ubuntu, macOS and Windows; real C/C++ consumers and header checks. Additional macOS ARM64 checks verified 57 exports, header synchronization, and an independent C11 consumer using a copied header and dynamic library (ABI 0.3.0).',
    limits:
      'Explicit ownership and destruction. Borrowed views must not outlive their owner. Existing C evidence does not qualify every FFI language.',
    distribution:
      'Source-built static/dynamic libraries and checked-in header. Cargo publish = false; no registry package asserted.',
    sources: [
      compatibility,
      source('C ABI contract', 'docs/INTEROP.md'),
      source('Public header', 'include/cribra.h'),
    ],
  },
  {
    id: 'wasm',
    name: 'WebAssembly',
    guide: { label: 'WebAssembly integration guide', href: '/docs/integration-wasm/' },
    additionalGuides: [{ label: 'WebAssembly examples', href: '/examples/#webassembly' }],
    package: {
      label: 'cribra-wasm 0.4.7 release',
      href: `${repository}/releases/tag/cribra-wasm-v0.4.7`,
    },
    implementation: 'Available · cribra-wasm 0.4.7 · typed JS/TypeScript exports',
    capabilities:
      'Checkout Core/Flow: configuration, metadata, ordered sequential batch scans, admitted streaming and incremental snapshots. Flow adds transformations and Share Bundles. No Fuse or Rayon parallel API.',
    qualification:
      'Existing production browser and Node/Bun/Deno examples. Newer sessions, configuration, batches and Share Bundles checked locally on macOS ARM64 with Node 26.12.0/Bun 1.4.3 against Core oracles. On 2026-10-25, Node 26.12.1 Core/Flow runtime qualification and the Binaryen -Oz production release gate passed, including JS/TypeScript contract, Rust parity and package validation.',
    limits:
      'Earlier WASM CI predates source session APIs. New session browser execution and Deno qualification are not established by those local checks; Workers/CSP remain host responsibilities.',
    distribution:
      'Independent 0.4.7 release and registry entry recorded. Published 0.4.7 depends on Core ^0.6; the checkout uses Core ^0.7. Checkout additions do not establish published session APIs or an npm package.',
    sources: [
      compatibility,
      source('WASM guide', 'crates/cribra-wasm/README.md'),
      {
        label: 'Earlier WASM CI',
        href: 'https://github.com/Metalymph/cribra/actions/runs/39411373136',
      },
    ],
  },
  {
    id: 'typescript',
    name: 'TypeScript',
    guide: { label: 'TypeScript quick start', href: '/docs/typescript/' },
    additionalGuides: [
      {
        label: 'TypeScript sessions and transformations',
        href: '/docs/typescript-sessions-and-transformations/',
      },
      {
        label: 'TypeScript runtime compatibility',
        href: '/docs/typescript-runtime-compatibility/',
      },
    ],
    apiReference: {
      label: 'TypeScript API reference',
      href: '/docs/typescript-api-reference/',
    },
    implementation:
      'Implemented · cribra-typescript 0.1.0 · WebAssembly backend · provisional and unpublished',
    capabilities:
      'Scanning, ordered batch scans, admitted byte streaming, incremental documents, structured errors and UTF-8 offset conversion. Flow adds the four scan-coupled transformations and in-memory Share Bundles. No native TypeScript backend, React Native binding or multi-threaded WASM API.',
    qualification:
      'W3.2C-1 installed-tarball qualification: Node 22.22.0, 24.22.2 and 26.13.0 for Flow and the test-only Core artifact; TypeScript 5.9.3 and 7.0.2 with NodeNext and Bundler; Google Chrome 156.0.8078.74, Playwright Chromium 156.0.8078.4, Playwright Firefox 157.0 and Playwright WebKit 27.2 for Flow; Chrome for Core; Vite 8.3.6 development/production and webpack 5.112.2 production. Public documentation examples run from the installed Flow tarball under Node.',
    limits:
      'Native Safari is unverified. Playwright WebKit is not Safari, Playwright Firefox is not branded Firefox, and Chrome is distinct from Playwright Chromium. The engines.node >=22 declaration is not evidence for untested or odd-numbered releases. Direct Rollup/esbuild consumption, React Native and multi-threaded WASM are not qualified.',
    distribution:
      'Working package cribra-typescript@0.1.0 is private and not published; no npm registry record exists. The intended name @metalymph/cribra is unavailable and pending ownership verification. Install only from a source checkout or a locally packed Flow tarball; the Core tarball is test-only.',
    sources: [
      compatibility,
      source('TypeScript API contract', 'docs/TYPESCRIPT_API.md'),
      source('TypeScript release runbook', 'docs/TYPESCRIPT_RELEASE.md'),
      source('Package README', 'packages/cribra/README.md'),
      source('TypeScript quick start', 'website/src/content/docs/typescript.md'),
    ],
  },
  {
    id: 'cli',
    name: 'CLI',
    guide: {
      label: 'CLI guide',
      href: '/docs/getting-started/#use-the-cli-from-this-repository',
    },
    additionalGuides: [{ label: 'CLI examples', href: '/examples/#cli' }],
    implementation: 'Available · cribra-cli 0.4.4 · executable cribra',
    capabilities:
      'Explicit UTF-8 file/stdin scanning, filters, sorting, metadata reports, four transforms and explicit-file bundle export. No directory traversal, streaming session or incremental-document command; parallel execution is library-only.',
    qualification:
      'Repository CLI regression and command-parity contracts. Bundle export is supported on macOS/Linux.',
    limits:
      'A successful scan returns exit code 0 even when findings or review candidates exist. Broader prebuilt-binary platform qualification is not established here.',
    distribution:
      'Build from source with cargo build -p cribra-cli. Registry publication was not established by the compatibility audit; Core releases do not publish the CLI.',
    sources: [
      compatibility,
      source('CLI capability contract', 'docs/CLI_PARITY.md'),
      source('CLI source', 'crates/cribra-cli/README.md'),
    ],
  },
  {
    id: 'swift',
    name: 'Swift',
    guide: { label: 'Swift integration guide', href: '/docs/integration-swift/' },
    apiReference: {
      label: 'Swift Core API reference (0.7.2)',
      href: '/api/0.7.2/swift/core/documentation/cribra/',
    },
    package: {
      label: 'Published cribra-swift package',
      href: 'https://github.com/Metalymph/cribra-swift',
    },
    registry: {
      label: 'Swift Package Index listing',
      href: 'https://swiftpackageindex.com/Metalymph/cribra-swift',
    },
    implementation: 'Available · Core 0.7.2 · published Fuse SwiftPM package 0.7.2',
    capabilities:
      'Core: configuration, reports, batch, admitted streaming and incremental snapshots. Flow adds transformations/Share Bundles. Fuse adds Core-owned parallel scans and is the published official remote package. Core and Flow remain source-build profiles. Select one matching Cribra module/package per process.',
    qualification:
      'macOS ARM64, Swift 6.4 and independent Core/Flow/Fuse SwiftPM package consumers passed; recorded Apple CI passed all three packages. On 2026-10-29 a clean-room remote consumer resolved cribra-swift 0.7.2, downloaded and checksum-verified the XCFramework, and ran on macOS ARM64. The same consumer compiled and linked for arm64-apple-ios16.0-simulator.',
    limits:
      'No device or Simulator runtime execution claim. macOS 13/iOS 16 are declared minimums, not runtime coverage of each OS. Intel slices absent. No signing or notarization qualification.',
    distribution:
      'Published official Fuse SwiftPM package: Metalymph/cribra-swift 0.7.2 (tag fa37a32), with a checksum-verified XCFramework release asset. Core and Flow remain source-build alternatives. The package is listed on the Swift Package Index for discovery; the listing is not Cribra compatibility-build evidence.',
    sources: [
      compatibility,
      source('Swift consumer guide', 'bindings/swift/README.md'),
      {
        label: 'Published Swift package',
        href: 'https://github.com/Metalymph/cribra-swift',
      },
      {
        label: 'Swift 0.7.2 release',
        href: 'https://github.com/Metalymph/cribra-swift/releases/tag/0.7.2',
      },
      {
        label: 'Apple CI receipt',
        href: 'https://github.com/Metalymph/cribra/actions/runs/39411373180',
      },
    ],
  },
  {
    id: 'python',
    name: 'Python',
    guide: { label: 'Python integration guide', href: '/docs/integration-python/' },
    implementation: 'Available · module cribra_uniffi · Core/Flow/Fuse consumers',
    capabilities:
      'Core scanning/configuration, batch, admitted streaming and incremental snapshots. Flow adds transformations/Share Bundles; Fuse adds parallel scans. Packaged wheel contains Fuse only.',
    qualification:
      'Installed Fuse wheel on macOS ARM64 CPython 3.11.17, 3.12.15, 3.13.7 and 3.14.8 passed isolated consumer tests. Recorded CI includes Linux x86_64, repaired manylinux_2_28_x86_64 and native Windows x86_64 installed-wheel gates.',
    limits:
      'macOS 13 deployment metadata is not runtime qualification on macOS 13. No musl, Linux/Windows ARM64 or Intel macOS qualification established.',
    distribution:
      'Local/CI cribra 0.7.2+fuse wheels. No PyPI publication established; use the documented local build/offline installation.',
    sources: [
      compatibility,
      source('Python consumer guide', 'bindings/python/README.md'),
      { label: 'Engine CI receipt', href: engineCi },
    ],
  },
  {
    id: 'kotlin',
    name: 'Kotlin/JVM',
    guide: { label: 'Kotlin/JVM integration guide', href: '/docs/integration-kotlin/' },
    implementation: 'Available · Core 0.7.2 · adapter 0.1.0 · JDK 17',
    capabilities:
      'Core/Flow/Fuse with scanning, configuration, batch, admitted streams and incremental snapshots; Flow transformations/Share Bundles; Fuse parallel scanning. One profile JAR and matching native library per classpath.',
    qualification:
      'Recorded Kotlin 2.4.21/JDK 17 bundle consumers on macOS ARM64, Linux x86_64 and Windows x86_64. Later configuration/session conformance and Core/Flow/Fuse isolated bundle consumers passed locally on macOS ARM64 (three positive consumers and nine expected rejections).',
    limits:
      'No Android, Kotlin Multiplatform, other architecture/JDK or external Gradle application qualification inferred. Later local conformance does not update earlier CI evidence.',
    distribution:
      'Local/CI profile JARs and host ZIPs; CI artifacts have retention limits. No Maven Central or other package publication established.',
    sources: [
      compatibility,
      source('Kotlin consumer guide', 'bindings/kotlin/README.md'),
      {
        label: 'Bundle CI receipt',
        href: 'https://github.com/Metalymph/cribra/actions/runs/39411373200',
      },
    ],
  },
];

/** Official workstreams that are planned or in progress; none is a published package unless the matrix above says so. */
export const plannedBindings = [
  {
    name: 'Native Rust MCP server',
    detail:
      'Planned ultra-lightweight stdio server over Cribra Core, with no Node.js/Python runtime, database or external service and no arbitrary filesystem access by default.',
  },
  {
    name: 'Kotlin/JVM Fuse distribution',
    detail:
      'Maven Central is the intended registry. Local/CI artifacts exist; no Maven Central publication is established.',
  },
  {
    name: 'Python Fuse distribution',
    detail:
      'PyPI is the intended registry. Local/CI artifacts exist; no PyPI publication is established.',
  },
  {
    name: 'TypeScript native backend',
    detail:
      'Future UniFFI-backed backend for Node.js, separate from the implemented WebAssembly package. React Native requirements remain separately gated; UniFFI does not by itself provide a working React Native binding.',
  },
] as const;

/** Candidates without a scheduled commitment or published package. */
export const demandDrivenBindings = ['C#/.NET', 'Go', 'PHP', 'Ruby', 'Elixir'] as const;

export const contributingHref = `${repository}/blob/main/CONTRIBUTING.md`;
