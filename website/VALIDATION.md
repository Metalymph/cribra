# W1 implementation and validation record

Original W1 validation: 2026-10-23. Documentation alignment and pre-PR audit:
2026-10-24, on branch feature/website-foundation. Historical, user-reported manual,
and current audit results below are separate evidence sets.

## Implemented result

A fully static Astro Tone website with the requested seven-section homepage,
four introductory Content Collection guides, an evidence-backed interoperability
matrix, runnable-repository example links, full-text Pagefind search, and the
existing title/description command palette. Typography, local fonts, spacing,
code presentation, and theme switching come from Tone. See README.md for theme
revision, reused components, the complete route list, and deployment instructions.

The matrix has separate implementation, qualification, distribution/publication,
and capability columns. Its single data source is src/data/bindings.ts, also used
by the homepage. Source snapshots are not presented as a live package-registry
query. Core 0.7.2, CLI source installation, source/published WASM differences, and
local/CI native-binding artifacts follow current source and compatibility docs.

## Validation performed

**Original W1 automated validation — 2026-10-23.** The following table records
the initial implementation, before approved PNG branding and the process diagram
were added; it is not a fresh qualification of the current checkout.

Host: macOS ARM64; Node v26.12.1; npm 11.20.0. No additional application or browser
testing dependency was installed.

| Check | Result |
| --- | --- |
| npm install --package-lock-only --ignore-scripts, then npm ci | Passed; lockfile retained and synchronized after removing three blog-only dependencies |
| npm run check | Passed: 38 files, zero errors, warnings, or hints |
| npm run lint | Passed |
| npm run lint:css | Passed |
| ASTRO_SITE_URL=https://cribra.pages.dev npm run build | Passed: 10 static HTML pages; sitemap generated |
| Pagefind 1.5.2 generation | Passed: English index, 8 content pages; search/404 excluded |
| Generated HTML parser inspection | Passed: 201 relative links/asset references, including fragment targets; zero missing references |
| Canonical and Open Graph URLs | All HTML pages use the supplied origin; canonical and og:url agree |
| Robots / sitemap | Production sitemap reference present; search and 404 omitted from sitemap |
| No-origin Astro build to a separate temporary directory | Passed: no canonical, og:url, or sitemap; noindex metadata and Disallow robots output |
| Removed-demo inspection | No demo routes, sample artwork, original theme icons, fabricated domain, blog feed, comments, or analytics references in built HTML |
| Browser console | No warnings/errors observed during the final homepage inspection |
| Justfile syntax and preservation | just --list passed; comparison against HEAD proves all prior content unchanged after subtracting the 12 added lines |
| Whitespace | git diff --check passed; all new source files checked separately with git diff --no-index --check |
| Rust/workspace isolation | Original W1 added only justfile recipes outside website/; the current branch also includes the root README artwork addition. Current scope is verified below |

The initial ?q= initialization was corrected to use PagefindUI.triggerSearch
rather than querying an asynchronously mounted input. Browser testing also caught
and corrected Tone's stale no-results translation token to SEARCH_TERM.


**Subsequent manual validation — 2026-10-24 (reported by the user).** These
results are preserved as supplied; they are not attributed to new browser tests
in this audit.

| Check | Reported result |
| --- | --- |
| Astro check | 0 errors, 0 warnings, 0 hints |
| ESLint / Stylelint | Passed |
| Prettier check on Markdown guides | Passed |
| Static production build with ASTRO_SITE_URL=https://cribra.pages.dev | Passed |
| Sitemap / robots.txt | Sitemap index and sitemap generated; robots.txt references the correct sitemap |
| Homepage metadata | Canonical and Open Graph URL use the production origin; robots is index, follow |
| Pagefind | 8 indexed pages |
| Internal link target existence | 130 links, all existing; fragments, external links, and accessibility were not checked |
| Generated-file ignores | dist/, node_modules/, and .astro/ ignored |
| Referenced Rust examples | Correspond to existing repository source files |

**Final documentation audit — 2026-10-24 (executed for this task).** Host:
macOS ARM64, Node v26.12.1, npm 11.20.0. No dependency installation, upgrade, or
new browser testing was performed.

| Check | Current result |
| --- | --- |
| npm --prefix website run check | Passed: 39 files; 0 errors, 0 warnings, 0 hints |
| npm --prefix website run lint | Passed; exit 0 |
| npm --prefix website run lint:css | Passed; exit 0 |
| ASTRO_SITE_URL=https://cribra.pages.dev npm --prefix website run build | Passed: 10 static HTML pages; Pagefind 1.5.2 indexed 8 pages / 789 words |
| Generated canonical / Open Graph URLs | All 10 HTML pages match their expected production-origin URL; homepage is https://cribra.pages.dev/ |
| Homepage robots / favicon / social preview | index, follow; image/png favicon at /brand/cribra-mask-v1.png; no dedicated social image metadata |
| Sitemap index / sitemap / robots.txt | Index points to https://cribra.pages.dev/sitemap-0.xml; sitemap has 8 production-origin URLs, excluding search and 404; robots.txt points to https://cribra.pages.dev/sitemap-index.xml |
| git diff --check | Passed; new website source files checked separately for whitespace |
| Actual Git manifest / ignores | 2 modified root files and 70 new website source files; generated output and dependencies excluded |
| Repository scope | Only README.md and justfile are modified tracked files; no Rust implementation, Cargo manifests/lockfile, bindings, adapters, or unrelated changes |
| Documentation-only audit edits | Only website/VALIDATION.md, website/README.md, and website/public/brand/README.md changed during this task; artwork and all other source hashes unchanged |

No new browser, accessibility, URL-fragment, or external-link tests were run in
this documentation audit. The earlier 201-reference automated inspection and the
later 130-link manual target-existence check have different scopes and must not
be combined into a current comprehensive link/accessibility qualification.

## Browser inspection

Original W1 browser inspection (2026-10-23), executed against the static
production preview in the desktop app's embedded
browser. Widths checked: 320, 375, 768, and 1280 pixels. These are responsive
viewport checks, not physical mobile-device or cross-browser qualification.

- Homepage inspected visually in light and dark themes.
- Theme selection persisted through navigation and reload.
- Mobile/tablet/desktop documents and examples stayed within viewport width;
  long code used the existing Expressive Code presentation.
- At 375 pixels the compatibility region was 341 pixels wide, containing a
  1216-pixel table; keyboard ArrowRight moved its horizontal scroll offset.
- Keyboard skip link transferred focus to main-content; active Docs navigation
  remained correct on nested guide routes.
- Palette opened from navigation; matching incremental search found Scanning
  modes; unknown terms showed No matching pages; Escape dismissed the dialog.
- Pagefind ?q=streaming returned Scanning modes and other relevant pages;
  redaction returned Examples and Explicit transformations; an unknown query
  displayed its correct no-results message.
- Before the approved PNG assets were installed, absent artwork produced no
  image elements or broken requests; wordmark and processing-text fallbacks
  remained present. This is historical fallback evidence, not current asset status.

## Branding integration

The approved concept image, public/brand/cribra-concept.png (1536 × 1024), is
registered in src/data/brand.ts and displayed by BrandConcept through the shared
BrandImage component. It appears in the homepage hero, the process section, and
the root README. The same PNG supports both themes, retaining its intrinsic
aspect ratio and meaningful alternative text. ProcessDiagram supplies the separate
HTML/CSS process explanation below the illustration.

The approved public/brand/cribra-mask-v1.png (1254 × 1254) is the official browser
favicon, referenced directly as image/png by BaseHead. No temporary favicon is
part of the current source manifest. Optional SVG mark and light/dark lockup
files remain absent; logo and mark descriptors remain unregistered. The header's
text wordmark and missing-asset fallbacks are intentional.

No dedicated application icon or social-preview image is included. Social image
metadata is omitted. The asset contract is documented in public/brand/README.md;
the existing theme LICENSE retains historical upstream asset notices, including
its original favicon filenames. Those notices are attribution, not current site
asset references, and are preserved unchanged.

## Cloudflare Pages preparation

Repository Metalymph/cribra; production branch main; root website; build command
npm run build; output dist; ASTRO_SITE_URL=https://cribra.pages.dev. The inherited
.nvmrc selects Node 22.22.0 (minimum >=22.22.0); NODE_VERSION can explicitly match
it in Pages. The local checks above ran on Node 26.12.1, not on that minimum.

No Cloudflare resource was created, and no deployment, publication, push, commit,
or pull request was performed. The local static preview is not hosting evidence.
Approved PNG artwork is installed; optional SVG artwork is not a release blocker.
Hosting configuration and preview indexing controls remain documented for eventual
manual deployment. No staging, commit, push, PR creation, or deployment is part of
this documentation audit.

## Exact source-file manifest

Derived on 2026-10-24 from git diff --name-only HEAD and
git ls-files --others --exclude-standard website. The index contains no staged
changes. Total: 72 source files (2 modified roots and 70 new website files).

Modified existing files:

- README.md — approved concept artwork, descriptive alt text, and width
- justfile — three independent website recipes

All website files below are new to the Cribra repository, including imported
theme sources and the two approved PNG assets. Generated dist/, .astro/, and
node_modules/ files are ignored and excluded.

- website/.env.example
- website/.gitignore
- website/.nvmrc
- website/.prettierrc
- website/.stylelintrc.json
- website/LICENSE
- website/README.md
- website/VALIDATION.md
- website/astro-theme-config.ts
- website/astro.config.mjs
- website/eslint.config.mjs
- website/package-lock.json
- website/package.json
- website/public/brand/README.md
- website/public/brand/cribra-concept.png
- website/public/brand/cribra-mask-v1.png
- website/src/assets/fonts/pretendard-variable.woff2
- website/src/assets/fonts/source-code-pro-latin-400-normal.woff2
- website/src/components/BaseHead.astro
- website/src/components/BindingMatrix.astro
- website/src/components/BrandConcept.astro
- website/src/components/BrandImage.astro
- website/src/components/BrandLogo.astro
- website/src/components/BrandMark.astro
- website/src/components/Footer.astro
- website/src/components/Header.astro
- website/src/components/HeaderLink.astro
- website/src/components/ProcessDiagram.astro
- website/src/components/SearchPalette.astro
- website/src/components/ThemeToggle.astro
- website/src/config/expressive-code.ts
- website/src/consts.ts
- website/src/content.config.ts
- website/src/content/docs/core-concepts.md
- website/src/content/docs/getting-started.md
- website/src/content/docs/scanning-modes.md
- website/src/content/docs/transformations.md
- website/src/data/bindings.ts
- website/src/data/brand.ts
- website/src/layouts/BaseLayout.astro
- website/src/layouts/DocLayout.astro
- website/src/pages/404.astro
- website/src/pages/bindings/index.astro
- website/src/pages/docs/[...slug].astro
- website/src/pages/docs/index.astro
- website/src/pages/examples/index.astro
- website/src/pages/index.astro
- website/src/pages/robots.txt.js
- website/src/pages/search.astro
- website/src/scripts/image-loading.ts
- website/src/scripts/mount.ts
- website/src/scripts/search-page.ts
- website/src/scripts/search-palette-loader.ts
- website/src/scripts/search-palette.ts
- website/src/scripts/skip-link.ts
- website/src/styles/base.css
- website/src/styles/code.css
- website/src/styles/elements.css
- website/src/styles/global.css
- website/src/styles/layouts/post.css
- website/src/styles/pages/home.css
- website/src/styles/pages/not-found.css
- website/src/styles/pages/search.css
- website/src/styles/prose.css
- website/src/styles/site.css
- website/src/styles/tokens.css
- website/src/styles/utilities.css
- website/src/utils/paths.ts
- website/src/utils/serialize-script-json.ts
- website/tsconfig.json
