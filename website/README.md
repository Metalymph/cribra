# Cribra website

Static, English-language product website and introductory documentation for
Cribra. It lives independently of the Rust workspace and does not build or load
Cribra/WASM. No backend, tracking, authentication, or Cloudflare adapter is used.

## Theme provenance

Imported from **Metalymph/astro-tone**, `main`, commit
`39a774b6e981ded677b8e443fd306e48682b6f81` on 2026-10-23:
https://github.com/Metalymph/astro-tone/tree/39a774b6e981ded677b8e443fd306e48682b6f81

This is the author's fork, not a fresh import from the original upstream.
Tone's original source is credited to its contributors and hanityx. `LICENSE`
retains the MIT notice and the bundled Pretendard and Source Code Pro font
licenses. No demo photography, original theme marks, or blog sample posts ship.

Reused foundations: `BaseLayout`, `BaseHead`, `Header`, `HeaderLink`, `Footer`,
`ThemeToggle`, `SearchPalette`, lazy search scripts, local fonts, CSS tokens,
prose/code styles, and Expressive Code configuration. `DocLayout` adapts the
article presentation without dates, related posts, or comments. Blog-only RSS,
reading-time, and toc-rail dependencies were removed; npm and its lockfile remain.

## Local development

Requires **Node >=22.22.0** and npm. `.nvmrc` preserves the theme's Node 22.22.0
selection. Do not rely on a hosting provider's default being recent enough.

From this directory:

```sh
npm ci
npm run dev
npm run check
npm run lint
npm run lint:css
ASTRO_SITE_URL=https://cribra.pages.dev npm run build
npm run preview -- --host 127.0.0.1
```

Open http://127.0.0.1:4321 for the preview. Full-text Pagefind search is generated
by `npm run build`, not `npm run dev`. The command palette searches page titles
and descriptions in both modes; its link opens full-text search. The search page
accepts `?q=...` for an initial query. Cmd/Ctrl+K opens the palette.

Copy `.env.example` to `.env.local` if you want the canonical production origin
in ordinary local builds. Environment variables override the env file. A build
without `ASTRO_SITE_URL` remains usable locally, omits absolute canonical/social
URLs and the sitemap, and emits noindex plus a disallowing robots.txt. Do not
publish that local-only output. Relative paths and origin-dependent metadata are
never inferred from an invented domain.

From the repository root, the independent recipes are:

```sh
just website-dev
just website-check
ASTRO_SITE_URL=https://cribra.pages.dev just website-build
```

Existing Rust recipes are unchanged; website recipes are not dependencies of the
Rust gates. Generated `.astro/`, `dist/`, and `node_modules/` are ignored here.

## Routes and content

- `/` — product homepage
- `/docs/` — documentation landing page and topic links
- `/docs/getting-started/` — install, first scan, profiles, integration entry points
- `/docs/core-concepts/` — boundaries, detection/validation, reports/review
- `/docs/transformations/` — transformations and Share Bundles
- `/docs/scanning-modes/` — whole-source, streaming, incremental
- `/bindings/` — evidence-backed integration matrix
- `/examples/` — existing runnable repository examples and static excerpts
- `/search/` — Pagefind; excluded from search indexing and sitemap
- `/404.html` — static error page; excluded from search indexing and sitemap

The four Markdown guides use a typed Astro Content Collection. Detailed technical
and qualification contracts remain in the repository docs. `src/data/bindings.ts`
is the single website source for integration availability, qualification,
publication, and capability information. The homepage derives its integration
list from it. Review this dated snapshot whenever the underlying contracts change.

Current source and `docs/COMPATIBILITY.md` take precedence over stale statements:
the website uses Core 0.7.2, does not assume CLI registry installation, and does
not describe current C/WASM session APIs as universally absent. Published WASM
and checkout dependency contracts remain distinct. Do not turn generated bindings,
local consumers, or CI diagnostic artifacts into registry-publication claims.

## Branding

The website uses two approved Cribra brand assets:

- `public/brand/cribra-concept.png` — Venetian half-mask concept artwork,
  displayed on the homepage and in the repository README.
- `public/brand/cribra-mask-v1.png` — official browser favicon.

The concept artwork illustrates sensitive-data input, the Cribra engine, and
structured output. The homepage also provides an accessible textual process
explanation independent of the illustration.

The concept PNG is registered in `src/data/brand.ts` and rendered through
`BrandConcept` in the hero and “How Cribra works” section. Its intrinsic dimensions
preserve its aspect ratio; the same approved PNG is used in both themes. The
favicon PNG is referenced directly by `BaseHead`.

Optional SVG mark and light/dark lockup files are currently absent; the `logo`
and `mark` descriptors are unregistered. The header's text wordmark and missing-asset
fallbacks remain intentional. See `public/brand/README.md` for the asset contract.

No separate application icon or dedicated social-preview image is included.

## CI generation and Cloudflare publication

### GitHub Actions pipeline

`.github/workflows/website-swift-docc.yml` builds the complete website on a
GitHub-hosted `xcode-27` macOS ARM64 runner. It validates pull requests and
pushes to `main` that touch `website/**`, `bindings/swift/**`,
`crates/cribra-uniffi/**`, `src/**`, `Cargo.toml`, `Cargo.lock`, the `justfile`,
or the workflow itself. `workflow_dispatch` starts it manually.

The pipeline reuses the repository's generation, staging, and build recipes:

```sh
npm ci --prefix website
just swift-apple core
just swift-docc-core
just stage-swift-docc-core
just website-check
just website-build
node website/scripts/validate-swift-docc-build.mjs
```

It requires:

| Tool | Requirement |
| --- | --- |
| Runner / Xcode | GitHub-hosted `xcode-27`, macOS ARM64, Xcode 27 |
| Swift | Swift 6.4 with `xcrun docc` |
| Rust | 1.97.0 with `aarch64-apple-darwin`, `aarch64-apple-ios`, `aarch64-apple-ios-sim` |
| Node | `.nvmrc` = `22.22.0` |
| Python | Python 3 from the runner, used by the DocC generator |
| `just` | Installed with Homebrew |

`npm ci` uses the committed lockfile. No build secrets are required.

After validation, the pipeline creates
`$RUNNER_TEMP/cribra-website-swift-docc-<commit-sha>.tar.gz` containing the
complete `website/dist` tree. The archive is stored outside `website/dist` so it
cannot include itself. It is checked with `gzip -t`, its file count is compared
with `website/dist`, and an extraction round trip verifies the Astro homepage,
the DocC module page, and a DocC file whose name is not valid on Windows
(`builderrorcode/!=(_:_:).json`).

Only the archive is uploaded, as the
`cribra-website-swift-docc-<commit-sha>` artifact for 14 days. Generated DocC
filenames and directory structure are preserved unchanged; archiving prevents
`actions/upload-artifact` from rejecting Windows-incompatible names. The
disabled deployment job downloads the archive, checks it, and extracts it so the
archive's `dist/` root becomes `website/dist` before Wrangler runs.

The validator checks the homepage, the versioned DocC entry point and module
page, required symbol pages, DocC assets, Pagefind output, and every root-relative
`href`/`src` reference in the built HTML. It also writes a manifest with file
counts, byte size, and required-file sizes to the workflow step summary.
Validation failures fail the job.

### Local generation and validation

Run from the repository root:

```sh
npm ci --prefix website
just swift-apple core
just swift-docc-core
just stage-swift-docc-core
just website-check
ASTRO_SITE_URL=https://cribra.pages.dev just website-build
node website/scripts/validate-swift-docc-build.mjs
```

`just website-build-swift-core` still performs the DocC generation, staging, and
website build subset. It does not build the Swift Apple artifacts or run the
combined-output validator.

### Validation versus publication

Pull-request and manual runs only validate and upload an artifact. Builds from
`main` are publication candidates, but the workflow does not deploy. The
deployment job is intentionally hard-disabled with `if: ${{ false }}` and no
Cloudflare credentials are used by the build job.

The Core 0.7.2 reference is staged at `/api/0.7.2/swift/core/` but is not linked
from public website pages. Do not enable those links until publication is
qualified; that belongs to W2.9.3.

### Cloudflare Pages prerequisites

The existing Cloudflare Pages project is assumed to use Git integration with
these settings. Verify each value before activation:

| Setting | Value |
| --- | --- |
| Repository | `Metalymph/cribra` |
| Production branch | `main` |
| Root directory | `website` |
| Build command | `npm run build` |
| Build output directory | `dist` |
| `ASTRO_SITE_URL` | `https://cribra.pages.dev` |
| Node | `.nvmrc` = `22.22.0`, or explicitly set `NODE_VERSION=22.22.0` |
| Pages project | `cribra` (assumed from `https://cribra.pages.dev`) |

The Git build command is `astro build && pagefind --site dist`: Pages must
install dev dependencies because Astro checks and Pagefind are build tools. The
npm lockfile supplies reproducible dependency resolution. Do not use a
Workers/SSR preset, add Functions, or add a Cloudflare runtime adapter for this
static site.

The Git build image does not provide Xcode, `xcrun docc`, or the Rust Apple
toolchain, so it cannot produce the combined Astro and Swift-DocC output. Do not
convert the project to Direct Upload: Cloudflare does not support converting a
Git-integrated project to Direct Upload.

Before enabling deployment, also verify the account ID, current Git integration
and branch-control settings, current production deployment, and any custom
domain in the Cloudflare dashboard. Then complete this activation procedure:

1. Create an API token with **Account → Cloudflare Pages → Edit**.
2. Add `CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` as GitHub environment
   secrets for the protected `production` environment.
3. In the Pages project, open **Build → Branch control** and disable automatic
   production deployments. Disable automatic preview deployments as well, so
   Wrangler is the only deployment mechanism.
4. Require reviewers on the `production` GitHub environment.
5. Confirm the pinned Wrangler version in the workflow
   (`wranglerVersion: "4.153.0"`) and review the deployment command.
6. Replace the deployment job's `if: ${{ false }}` with a reviewed main-only
   condition. Keep the job dependent on the validated `build` job, and use the
   protected `production` environment as the approval gate.
7. Run the workflow manually or on a `main` merge, inspect the downloaded
   archive and validation manifest, then confirm the deployment.

The prepared command deploys the extracted tree:

```sh
wrangler pages deploy website/dist --project-name=cribra --branch=main
```

Do not re-enable automatic Git deployments after activation. That would create
competing production deployments.

### Rollback and future versions

To roll back, open the Pages project's **Deployments** list, choose a previously
successful production deployment, and select **Rollback to this deployment**.
Cloudflare retains prior production deployments. GitHub artifacts are retained
for 14 days; rebuild an older commit if a longer window is required.

API references are versioned by path (`/api/<version>/swift/core/`). The
`swift-docc-core` and `stage-swift-docc-core` recipes and the output validator
currently hardcode `0.7.2`. A new version requires a deliberate update to those
paths. Keep older version directories in place; do not replace or prune them.
Add public navigation links only after the new version is published.

References:
- https://developers.cloudflare.com/pages/get-started/direct-upload/
- https://developers.cloudflare.com/pages/configuration/git-integration/
- https://developers.cloudflare.com/pages/configuration/rollbacks/
- https://developers.cloudflare.com/pages/framework-guides/deploy-an-astro-site/
- https://developers.cloudflare.com/pages/configuration/build-image/

## Review checks

Run the checks and build above, then inspect the production preview in light and
dark themes at mobile/tablet/desktop sizes. Check keyboard navigation, palette
open/close/empty results, Pagefind results, horizontal matrix scrolling, code
blocks, metadata, sitemap/robots, links and fragment targets, and asset fallbacks.
Use `git diff --check` and inspect the complete untracked-file manifest as well.

Keep these checks distinct from published-package, provider, hosted, and Rust
qualification. The website adds no Rust API or release qualification claim.
