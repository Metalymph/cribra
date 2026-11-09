# CI/CD

Cribra treats `main` as the release-quality branch.

## Continuous integration

Pull requests and pushes to `main` are validated by five independent surfaces.

### CI

The primary Linux gate runs:

```text
cargo fmt --all -- --check
cargo check --all-features
cargo test
cargo test --features serde
cargo test --all-features
cargo test --doc
cargo clippy --all-targets --all-features -- -D warnings
```

macOS and Windows additionally run cross-platform `cargo check` and test gates.

### MSRV

`Cargo.toml` declares Rust 1.97 as the minimum supported Rust version.
The MSRV workflow tests that contract explicitly rather than relying on the
stable toolchain.

### WASM

The supported browser-oriented WASM contract is checked for
`wasm32-unknown-unknown` with:

- default features;
- `serde`.

The optional `parallel` feature is not part of the supported browser WASM
contract.

### TypeScript

`.github/workflows/typescript.yml` is dedicated to the TypeScript package
artifact and is scoped to `packages/cribra/**`, `crates/cribra-wasm/**`,
`Cargo.toml`, `Cargo.lock`, and the workflow itself. It is also callable via
`workflow_call` by the release workflow. The package is
`@metalymph/cribra@0.1.0` (not private, not published); its manifest is the
single source of the package name, version, and tarball names.

One producer job builds the Flow and test-only Core `npm pack` tarballs and
uploads them with `actions/upload-artifact`. Independent consumer jobs download
that artifact and never rebuild it:

- Node 22, 24, and 26 ESM consumers;
- TypeScript 5.9.3 and 7.0.2 with `NodeNext` and `Bundler` resolution;
- Chrome, Chromium, Playwright Firefox, and Playwright WebKit;
- Vite development/production and webpack production, importing the package by
  name.

The workflow has `contents: read` permissions, no registry credentials, and no
publish step. Native Safari is probed only on manual dispatch on macOS; it
reports blocked when remote automation is not already enabled and never
modifies system settings.

### TypeScript release

`.github/workflows/typescript-release.yml` is manual only and has two paths.
Ordinary pushes, pull requests, and unreviewed tag creation never reach staging.

1. **Readiness** reuses `typescript.yml` through `workflow_call`, then verifies
   the dispatched tag and commit, byte-for-byte repeat-pack reproducibility, the
   complete consumer matrix, and the approved package identity. It writes
   `release-manifest.json` and `SHA256SUMS` and uploads
   `typescript-release-<tag>`. It has `contents: read` and no OIDC permission.
2. **Stage** requires the protected GitHub environment `npm-release` and
   `id-token: write`. It fails unless the identity is approved, the tag/commit
   match the qualified manifest, the tarball digest and file records match, the
   version is not already published, the protected environment defines
   `CRIBRA_RELEASE_AUTHORIZED=1`, and the confirmation is exactly
   `stage <tag>`. It runs
   `npm stage publish` and stops. A maintainer must download the staged
   tarball, compare its SHA-256 with the manifest, verify provenance, and approve
   with 2FA. The workflow never approves or promotes the version.

The manifest is `@metalymph/cribra@0.1.0` with `cribraRelease.state: "approved"`,
so release-mode checks pass. Staging is still impossible without the separately
protected authorization, and no npm record exists yet. The npm CLI is pinned to
`12.2.2`, above the required `11.15` minimum for staged publishing.

Trusted Publishing is configured only for the approved package and only
immediately before an authorized release; npm expires an unused configuration
after two days. A brand-new package needs a `0.0.0-stage` placeholder published
only under the `stage` dist-tag before OIDC staging can work. The placeholder is
never `latest` and is not a release.

See [the TypeScript release runbook](TYPESCRIPT_RELEASE.md) for the verified
ownership, bootstrap, digest verification, approval, and rollback limits. Rollback means a
new patch plus `npm deprecate`; published versions cannot be overwritten.

### Security

RustSec's `audit-check` action audits `Cargo.lock`:

- when dependency manifests change;
- on pushes to `main` that modify dependencies;
- every Monday;
- on explicit manual dispatch.

Scheduled audits may create GitHub issues for newly discovered advisories.

Dependabot checks Cargo and GitHub Actions dependencies weekly.

## Release process

Releases are tag-driven.

A tag such as:

```text
v0.5.0
```

must exactly match the version in `Cargo.toml`.

The release workflow then:

1. validates the tag/version match;
2. runs formatting, checks, tests, rustdoc and clippy;
3. runs `cargo package`;
4. runs `cargo publish --dry-run`;
5. authenticates to crates.io using Trusted Publishing/OIDC;
6. publishes the crate;
7. creates the matching GitHub Release.

No release is published from an ordinary push to `main`.

## crates.io Trusted Publishing bootstrap

Trusted Publishing requires the crate to exist on crates.io before the trusted
publisher can be configured.

For the first `cribra` release:

1. publish the initial Cribra crate manually;
2. configure `Metalymph/cribra` as a trusted GitHub publisher on crates.io;
3. configure the GitHub `release` environment if desired;
4. use tag-driven releases from that point onward.

No long-lived crates.io API token is required by the automated release
workflow.

## Recommended branch protection

For `main`:

- require pull requests before merge;
- require the CI, MSRV, WASM and Security status checks;
- require branches to be up to date before merge;
- require conversation resolution;
- block force pushes;
- block branch deletion.

While Cribra has a single maintainer, review approval can remain optional.
When external contributors become active, require at least one approving
review.

## Local equivalent

Before pushing a substantial change, run:

```text
cargo fmt --all
cargo fmt --all -- --check
cargo check --all-features
cargo test
cargo test --features serde
cargo test --all-features
cargo test --doc
cargo clippy --all-targets --all-features -- -D warnings
cargo package --allow-dirty
```

Security audits can be run locally with:

```text
cargo install cargo-audit
cargo audit
```

## Core 0.5.0 integration and publication boundary

The existing `.github/workflows/release.yml` listens only to pushed tags matching
`v*.*.*`; its validator additionally requires exact equality with core version.
For core 0.5.0 the tag is `v0.5.0`. A branch push or merge to `main` does not
publish. Jobs run in order: `validate`, core-only `publish` in environment
`release` using OIDC, then `github-release`. Adapter publication is not included.
The WASM adapter has its own `cribra-wasm-v*` tag workflow and independent version.

The active repository ruleset `PR on main` (21052746), inspected on 2026-10-12,
requires a pull request, resolved review threads, an up-to-date branch and these
checks: `Linux full gate`, `Test macos-latest`, `Test windows-latest`,
`wasm32-unknown-unknown`, `RustSec cargo audit`, and `Rust 1.97`. It blocks branch
deletion and force pushes and has no bypass actors. Approval count is zero.
The classic branch-protection endpoint returns 404; the ruleset is authoritative.
Native C ABI checks are also release qualification gates on all three platforms.
The `release` environment currently has no additional approval or branch policy.

Integrate by PR into `main`, qualify its exact tree with the existing workflows,
and verify the resulting main commit. Do not tag an unqualified feature revision
or rely on CI from an older main commit. After explicit publication authorization,
create `v0.5.0` at the qualified main commit and push that tag to origin. Observe
the existing release workflow through publication and GitHub Release creation;
do not duplicate either operation manually. The workflow does not itself enforce
main ancestry or await Windows CI, so those are mandatory pre-tag checks.

See [the qualification ledger](0.5-qualification.md) for this release's exact
evidence and [release preparation](0.5-release.md) for the integration boundary.
