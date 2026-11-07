# TypeScript Package Release Runbook

Status: **release infrastructure implemented in W3.2C-2. No npm record, tag,
or publication exists. The first publication is blocked until npm ownership is
verified and a separate explicit release authorization is given.**

This runbook covers the independently versioned npm package built from
`packages/cribra`. It does not cover the Core crate, `cribra-wasm`, the CLI, or
any other adapter. Only the Flow package may become public; the Core tarball is
a test-only qualification artifact and is never published.

## 1. Current identity state

| Field | Current value |
| --- | --- |
| `package.json` `name` | `cribra-typescript` (provisional) |
| `package.json` `version` | `0.1.0` |
| `private` | `true` |
| Approved release name | `@metalymph/cribra` |
| `cribraRelease.state` | `pending-ownership` |
| Tag namespace | `typescript-v*` |
| Dist-tag | `latest` |
| Registry | `https://registry.npmjs.org/` |

Every script derives the package specifier and tarball names from
`package.json`. Renaming the package must not require editing qualification,
packaging, type-test, or release code.

Release mode fails closed unless all of the following hold:

- `cribraRelease.state === "approved"`;
- `name === cribraRelease.approvedName`;
- `name` is not the provisional `cribra-typescript`;
- `private` is absent;
- `version` is valid SemVer;
- `type === "module"` and `engines.node === ">=22"`;
- `publishConfig.access === "public"`;
- `publishConfig.provenance === true`;
- `publishConfig.registry === "https://registry.npmjs.org/"`;
- `publishConfig.tag === "latest"`.

Any failure is intentional. Do not weaken the gate to make a release run.

## 2. Verify npm ownership before renaming

The rename is blocked until ownership of the approved scope is proven **while
authenticated**. An unauthenticated `npm view` 404 for `@metalymph/cribra` is not
proof that the scope is available, and it is not proof that the maintainer can
publish into it.

Run, on a trusted workstation:

```sh
npm login
npm whoami
npm org ls metalymph
npm access list packages @metalymph --json
npm view @metalymph/cribra --json
```

Interpretation:

- `npm whoami` must return the maintainer account, not `ENEEDAUTH`.
- `@metalymph` works as a personal scope only if the npm username is
  `metalymph`.
- Otherwise `@metalymph` must be an npm organization, and the authenticated
  account must have publish rights in it.
- `npm org ls metalymph` must succeed and list the account. A 404 while
  unauthenticated is not authoritative.
- `npm view @metalymph/cribra` must either return 404 or show a record the
  maintainer controls. An existing record owned by someone else blocks the
  rename.

Record the command output in the release authorization. Do not infer ownership
from the GitHub account name: the GitHub `Metalymph` user and an npm scope are
different systems.

### 2.1 Apply the approved identity

Only after ownership is proven and the rename is reviewed:

1. Set `package.json` `name` to `@metalymph/cribra`.
2. Remove `"private": true`.
3. Set `cribraRelease.state` to `"approved"`.
4. Update the identity and install examples in `packages/cribra/README.md`,
   `docs/TYPESCRIPT_API.md`, and this runbook. The README ships in the tarball,
   so stale provisional instructions are a release defect.
5. Regenerate both lockfiles so their root names match:
   ```sh
   npm install --package-lock-only --ignore-scripts --no-audit --no-fund
   npm --prefix packages/cribra/qualification install \
     --package-lock-only --ignore-scripts --no-audit --no-fund
   ```
6. Run the full checks in section 8.
7. Confirm `npm run package:check:release` passes and
   `npm run release:stage-check ...` still refuses until authorization inputs
   are supplied.

If the approved name must change, update `cribraRelease.approvedName` and the
documentation first; do not bypass the comparison.

## 3. First-release bootstrap for a nonexistent package

npm Trusted Publishing can only be configured for a package that already
exists. A brand-new package therefore needs one bootstrap record before OIDC
staging can work.

### 3.1 Placeholder publication

After explicit authorization, publish a placeholder under a non-`latest`
dist-tag:

```sh
# Use a throwaway tarball whose package.json has the approved name and
# version 0.0.0-stage. Never publish the real qualified artifact as the
# placeholder.
npm publish <placeholder>.tgz --access public --tag stage --ignore-scripts
```

Rules:

- The placeholder version is `0.0.0-stage`.
- It is published only with `--tag stage`.
- It must never receive the `latest` dist-tag.
- The placeholder is a public, persistent registry record. It remains listed,
  is not a release, and should be documented in the release notes.
- Do not publish the real `0.1.0` artifact manually. It must come from the
  protected staging workflow so that provenance and digest verification apply.

### 3.2 Configure Trusted Publishing

Immediately before the authorized release, configure the npm Trusted Publisher
for the repository and workflow:

- publisher: GitHub `Metalymph/cribra`;
- workflow: `.github/workflows/typescript-release.yml`;
- environment: `npm-release`.

npm expires an unused Trusted Publisher configuration after two days. Configure
it only when the release is ready to run, not weeks in advance.

No long-lived npm token is stored in GitHub. The staging job uses OIDC
(`id-token: write`) and exchanges it for a short-lived credential.

## 4. Readiness workflow

`.github/workflows/typescript-release.yml` is manual only.

### 4.1 Trigger readiness

1. Confirm the release commit is on `main` and fully qualified.
2. Create the tag from that exact commit, for example:
   ```sh
   git tag typescript-v0.1.0 <qualified-commit>
   git push origin typescript-v0.1.0
   ```
   Tag creation and pushing are separate release operations. They are not part
   of the implementation workstream.
3. Dispatch the workflow from the tag ref with:
   - `mode: readiness`
   - `tag: typescript-v0.1.0`
   - `confirmation` empty

The workflow:

1. runs the reusable `.github/workflows/typescript.yml` qualification matrix
   (`workflow_call`);
2. verifies the dispatched ref is exactly `refs/tags/<tag>`;
3. checks out the tag with full depth;
4. downloads the qualified Flow/Core tarballs and consumer evidence;
5. runs `release-readiness.mjs --skip-qualification --require-matrix
   --require-releaseable`;
6. uploads `typescript-release-<tag>` containing:
   - `release-manifest.json`;
   - `SHA256SUMS`;
   - `flow/<package>-<version>.tgz`.

Readiness performs no registry call and has `contents: read` only. It does not
receive `id-token: write`.

### 4.2 Readiness gates

Readiness fails unless:

- the tag matches `typescript-v<semver>` and the manifest version;
- the tag resolves to HEAD and the workflow ref/sha match;
- the checkout is clean;
- the identity is approved and non-provisional;
- repeat packing is byte-for-byte identical;
- the qualified tarball and WASM digests still match the pack evidence;
- the Flow and Core WASM digests differ;
- the Core tarball is private and excluded;
- the complete consumer matrix is present:
  - Node 22, 24, and 26 for Flow and Core;
  - TypeScript 5.9.3 and 7.0.2;
  - Chrome, Chromium, Playwright Firefox, and Playwright WebKit for Flow;
  - Chrome for Core;
  - Vite and webpack.

Native Safari is recorded as `unverified` and must never be presented as
qualified.

Local equivalent, without matrix or releaseability requirements:

```sh
cd packages/cribra
npm run qualify:pack
npm run release:readiness -- --local --skip-qualification --allow-provisional
npm run release:verify -- \
  --release-root ../../target/typescript-release \
  --provisional
```

The local manifest is `releaseable: false` while the identity is provisional.

## 5. Staging workflow

Staging is a separate manual dispatch after readiness review. It is protected by
the GitHub environment `npm-release`.

### 5.1 Environment protection

Configure `npm-release` with:

- required reviewers (at least one maintainer other than the dispatcher when
  more than one maintainer is active);
- deployment branches/tags restricted to `typescript-v*`;
- an environment variable `CRIBRA_RELEASE_AUTHORIZED=1`;
- no bypass actors unless a documented emergency policy exists.

The workflow reads `CRIBRA_RELEASE_AUTHORIZED` from the environment, not from
the dispatch form. Removing the variable, or setting it to anything other than
`1`, makes staging fail.

The job receives `contents: read` and `id-token: write`. It never receives a
persistent npm token or `NODE_AUTH_TOKEN`.

### 5.2 Trigger staging

Dispatch the workflow from `refs/tags/<tag>` with:

- `mode: stage`
- `tag: typescript-v0.1.0`
- `confirmation: stage typescript-v0.1.0` (exact string)

Staging fails closed unless:

- the protected `npm-release` environment defines
  `CRIBRA_RELEASE_AUTHORIZED=1`;
- `--authorized` is passed;
- the confirmation is exactly `stage <tag>`;
- the identity is approved;
- the manifest is `releaseable: true` and its tag matches;
- the workflow ref and SHA match the qualified tag and commit;
- the tarball matches the manifest filename, size, SHA-256, WASM digest, and
  file records;
- `npm view <name>@<version>` proves the version does not already exist.

The workflow then runs:

```sh
npm stage publish <tarball> --access public --provenance --tag latest --ignore-scripts
```

It stops after staging. It never approves or promotes the staged version.

### 5.3 Human approval

`npm stage publish` source-inspects as pass-through; it does not prove that the
registry received the same bytes. Before approving with 2FA:

1. Download the staged tarball with `npm stage download`.
2. Compute SHA-256.
3. Compare it with `release-manifest.json` and `SHA256SUMS` from
   `typescript-release-<tag>`.
4. Confirm the package name, version, dist-tag, and provenance attestation.
5. Confirm the staged tarball contains `dist/wasm/cribra_bg.wasm` and no Core
   or test-only files.
6. Approve the staged version with 2FA.

A mismatch is a release stop. Do not approve, and do not retry with a different
artifact under the same version.

## 6. Digest and provenance verification

The release manifest records at least:

- package name, version, tag, dist-tag, access, and registry;
- source repository, directory, commit, ref, tag, and cleanliness;
- Node, npm, Rust, Cargo, `wasm-bindgen`, TypeScript, platform, arch, and OS;
- adapter crate/version and `wasm-bindgen` version;
- Flow tarball filename, size, SHA-256, WASM SHA-256, npm integrity, and file
  records;
- qualification command, evidence records, matrix, repeat-pack evidence, and
  pack timestamp;
- the test-only Core tarball digest with `private: true` and
  `excludedFromRelease: true`;
- the native-Safari warning.

`npm stage publish --provenance` produces a signed attestation linking the
tarball to the workflow, repository, commit, and OIDC identity. Verify it on the
registry record after staging.

## 7. Rollback and unpublish limits

- Published npm versions are immutable. A published version cannot be
  overwritten.
- The supported rollback is a new patch release plus `npm deprecate` on the
  affected version with a migration message.
- Unpublishing may be possible only within npm's short window and can be
  restricted by policy or dependents. Do not plan a release around unpublish.
- Dist-tags can be moved, but never move `latest` to the `0.0.0-stage`
  placeholder.
- If staging is wrong but not yet approved, reject the staged version instead of
  approving it.
- If provenance or digest verification fails after approval, deprecate the
  version, publish a fixed patch, and document the incident. Do not attempt to
  rewrite history.

## 8. Verification checklist

Before readiness:

```sh
cd packages/cribra
npm ci
npm run check
npm run test:types
npm run build
npm test
npm run package:check
npm run package:check:release      # expected to fail while provisional
npm run qualify:pack
npm run qualify:all
```

Before staging:

- [ ] npm ownership proven while authenticated.
- [ ] Rename applied and `cribraRelease.state` is `approved`.
- [ ] `private` removed and `publishConfig` unchanged.
- [ ] Both lockfiles regenerated.
- [ ] `package:check` and `package:check:release` pass.
- [ ] Readiness run uploaded a `releaseable: true` manifest.
- [ ] Tarball SHA-256 and WASM SHA-256 match the manifest.
- [ ] Repeat pack was byte-for-byte identical.
- [ ] Full matrix evidence is present.
- [ ] `npm-release` environment reviewers and tag restrictions are configured.
- [ ] `npm-release` defines `CRIBRA_RELEASE_AUTHORIZED=1`.
- [ ] Trusted Publisher is configured and less than two days old.
- [ ] Explicit release authorization is recorded outside the repository.

After staging:

- [ ] `npm stage download` SHA-256 matches the manifest.
- [ ] Staged dist-tag is `latest` for the real release and never for
      `0.0.0-stage`.
- [ ] Provenance attestation points to the expected repository, workflow,
      commit, and environment.
- [ ] Human 2FA approval is complete.
- [ ] Registry metadata and the package contents match the reviewed artifact.
- [ ] Release notes state that Core is not published and native Safari is
      unverified.

## 9. Explicit non-actions

The W3.2C-2 implementation does **not**:

- create any npm record, including `0.0.0-stage`;
- configure a Trusted Publisher;
- stage or publish a package;
- create or push a Git tag;
- create a GitHub Release;
- add a long-lived npm token;
- publish the Core tarball;
- claim native Safari support.

Those remain separately authorized operational steps.
