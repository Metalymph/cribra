# CI/CD

Cribra treats `main` as the release-quality branch.

## Continuous integration

Pull requests and pushes to `main` are validated by four independent surfaces.

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
