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

## Cloudflare Pages — manual Git configuration

No Cloudflare resources or deployments are created by this foundation.
When deploying later, configure:

| Setting | Value |
| --- | --- |
| Repository | `Metalymph/cribra` |
| Production branch | `main` |
| Root directory | `website` |
| Build command | `npm run build` |
| Build output directory | `dist` |
| `ASTRO_SITE_URL` | `https://cribra.pages.dev` |
| Node | `.nvmrc` = `22.22.0`, or explicitly set `NODE_VERSION=22.22.0` |

The build command is `astro build && pagefind --site dist`: Pages must install
dev dependencies because Astro checks and Pagefind are build tools. The npm
lockfile supplies reproducible dependency resolution. Do not use a Workers/SSR
preset, add Functions, or add a Cloudflare runtime adapter for this static site.

For a preview deployment, keep canonical metadata pointed at the production
origin; prevent public indexing using Cloudflare's preview controls. No remote
preview, hosting integration, or production deployment was qualified locally.
If the production domain changes, update the environment setting and rebuild.

### Swift-DocC Core API reference

The Core 0.7.2 reference is staged at `/api/0.7.2/swift/core/` but is not linked
from public website pages yet. Do not enable those links until the deployment
pipeline can produce and publish the reference with the site.

Generate and stage it from the repository root:

```sh
# Only needed when bindings/swift/Generated/ is absent.
just swift-apple core

just swift-docc-core
just stage-swift-docc-core
just website-build
```

`just website-build-swift-core` runs the same generation, staging, and website
build steps in order. The generated DocC tree is written to
`public/api/0.7.2/swift/core/`, which is ignored by Git. Its entry point
redirects to the generated `documentation/cribra/` module page.

The Cloudflare Pages v3 Git build image cannot produce this output: it does not
provide Xcode or `xcrun docc`, and the generated Swift package and native
artifacts are intentionally not tracked. Production publication therefore
requires one macOS-based CI pipeline that builds both Astro and Swift-DocC and
uploads the combined `dist` output. Do not mix a Git-integrated Cloudflare Pages
deployment with independent direct uploads.

References:
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
