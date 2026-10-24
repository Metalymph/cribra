/** Repository evidence snapshot, not a live package-registry lookup. */
export const evidenceDate = '2026-10-23';
export const repository = 'https://github.com/Metalymph/cribra';

/** One independently evidenced integration surface. */
export interface Binding {
  id: string;
  name: string;
  implementation: string;
  capabilities: string;
  qualification: string;
  limits: string;
  distribution: string;
  sources: { label: string; href: string }[];
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
    implementation: 'Available · cribra 0.7.2 · MSRV 1.97',
    capabilities:
      'Core: scanning, reports, batch, admitted streaming and incremental documents. Flow (default): four transforms and Share Bundles. Fuse: Flow plus Serde and parallel independent-source scans. These are Cargo feature combinations, not features named core/flow/fuse.',
    qualification:
      'Recorded engine CI: Linux full gate, native Linux ARM64, RISC-V64 under QEMU, and macOS/Windows tests.',
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
    implementation: 'Available · cribra-capi 0.3.0 · ABI protocol 0.3.0',
    capabilities:
      'Scanner configuration, metadata, findings/review, batch, admitted byte streaming, incremental snapshots, four transformations and in-memory Share Bundles. AUTO batches may use parallel scanning only with the adapter parallel feature.',
    qualification:
      'Native C ABI gates recorded on Ubuntu, macOS and Windows; real C/C++ consumers and header checks. Later metadata checks were local macOS ARM64 source checks.',
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
    implementation: 'Available · cribra-wasm 0.4.7 · typed JS/TypeScript exports',
    capabilities:
      'Checkout Core/Flow: configuration, metadata, ordered sequential batch scans, admitted streaming and incremental snapshots. Flow adds transformations and Share Bundles. No Fuse or Rayon parallel API.',
    qualification:
      'Existing production browser and Node/Bun/Deno examples. Newer sessions, configuration, batches and Share Bundles checked locally on macOS ARM64 with Node 26.12.0/Bun 1.4.3 against Core oracles.',
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
    id: 'cli',
    name: 'CLI',
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
    implementation: 'Available · Core 0.7.2 · UniFFI adapter 0.1.0',
    capabilities:
      'Core: configuration, reports, batch, admitted streaming and incremental snapshots. Flow adds transformations/Share Bundles. Fuse adds Core-owned parallel scans. Select one matching Cribra module/package per process.',
    qualification:
      'macOS ARM64, Swift 6.4 and external package consumers; recorded Apple CI passed all three packages. ARM64 device/Simulator archives built; Simulator compiled/linked.',
    limits:
      'No device or Simulator execution claim. macOS 13/iOS 16 are declared minimums, not runtime coverage of each OS. Intel slices absent.',
    distribution:
      'Generated local SwiftPM packages and XCFrameworks. No remote SwiftPM publication or signed release established.',
    sources: [
      compatibility,
      source('Swift consumer guide', 'bindings/swift/README.md'),
      {
        label: 'Apple CI receipt',
        href: 'https://github.com/Metalymph/cribra/actions/runs/39411373180',
      },
    ],
  },
  {
    id: 'python',
    name: 'Python',
    implementation: 'Available · module cribra_uniffi · Core/Flow/Fuse consumers',
    capabilities:
      'Core scanning/configuration, batch, admitted streaming and incremental snapshots. Flow adds transformations/Share Bundles; Fuse adds parallel scans. Packaged wheel contains Fuse only.',
    qualification:
      'Installed Fuse wheel on macOS ARM64 CPython 3.11–3.14. Recorded CI includes Linux x86_64, repaired manylinux_2_28_x86_64 and native Windows x86_64 installed-wheel gates.',
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
    implementation: 'Available · Core 0.7.2 · adapter 0.1.0 · JDK 17',
    capabilities:
      'Core/Flow/Fuse with scanning, configuration, batch, admitted streams and incremental snapshots; Flow transformations/Share Bundles; Fuse parallel scanning. One profile JAR and matching native library per classpath.',
    qualification:
      'Recorded Kotlin 2.4.21/JDK 17 bundle consumers on macOS ARM64, Linux x86_64 and Windows x86_64. Later configuration/session conformance ran locally on macOS ARM64.',
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

/** Roadmap integrations; none is presented as implemented or qualified. */
export const plannedBindings = [
  'Higher-level TypeScript wrapper over existing typed WASM exports',
  'TypeScript / React Native via UniFFI',
  'C# / .NET via C ABI',
  'Go via cgo',
  'PHP via FFI',
] as const;
