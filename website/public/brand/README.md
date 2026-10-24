# Cribra brand asset contract

The approved PNG artwork is installed. Do not regenerate, alter, or substitute
it. Optional SVG artwork remains absent; missing assets intentionally produce
text-only fallbacks.

| Filename | Use | Status |
| --- | --- | --- |
| `cribra-mark.svg` | Optional compact mark | Absent; unregistered |
| `cribra-lockup-light.svg` | Optional lockup for light backgrounds | Absent; logo descriptor unregistered |
| `cribra-lockup-dark.svg` | Optional lockup for dark backgrounds | Absent; logo descriptor unregistered |
| `cribra-concept.png` | Concept illustration in the homepage hero, process section, and root README | Approved; present; concept descriptor registered |
| `cribra-mask-v1.png` | Official browser favicon; Venetian half-mask artwork | Approved; present; referenced directly in BaseHead |

The SVG mark and lockups remain optional future assets. The header's text wordmark
is intentional while its logo descriptor is unregistered. No separate application
icon or dedicated social-preview image is included; social image metadata is omitted.

## Approved visual direction

The primary logo follows the top-left composition of the separately approved
reference: an elegant Venetian female mask divided vertically, with binary digits
on the left and a clean geometric right half. Sensitive-data symbols enter from
the left; clean structured output leaves to the right.

The concept illustration follows the bottom-left input → mask → output diagram
of that reference. The approved concept artwork is included as `cribra-concept.png`.

## Registering final assets

The concept descriptor already registers `cribra-concept.png` at its intrinsic
1536 × 1024 dimensions, with meaningful alt text and no separate dark variant.
The same approved PNG is displayed in both themes. The favicon is the 1254 × 1254
`cribra-mask-v1.png`, linked as `image/png` by `BaseHead.astro`, independently of
the logo/mark/concept descriptors. The following steps apply to future approved
optional artwork, not to replacing either installed PNG.

1. Add only the approved files here and preserve their original proportions.
2. In `src/data/brand.ts`, register the matching `logo`, `mark`, or `concept`
   descriptor with root-relative `light`, optional `dark`, exact intrinsic
   `width`/`height`, and meaningful `alt` text. For example, the logo descriptor
   should reference the two lockup filenames, with the approved dimensions.
3. If a concept or mark receives a dark variant, add that approved file and
   register it in the same descriptor. A single asset must work on both themes
   if no variant is supplied.
4. Rebuild and inspect both themes at mobile and desktop sizes. A build-time
   existence check omits absent images; changing assets requires another build.

`BrandLogo.astro`, `BrandMark.astro`, and `BrandConcept.astro` accept an optional
typed asset prop or use the central descriptor. They share `BrandImage.astro`,
which supplies dimensions and theme switching without image probing in the
browser. The hero currently renders `BrandConcept`, preserving its intrinsic
aspect ratio without cropping. The existing components retain text fallbacks if
their registered asset is absent at build time. `ProcessDiagram.astro` provides
the HTML/CSS process explanation below the concept image independently of artwork.

Use alt text describing meaning rather than filenames. Avoid repeating nearby
text unnecessarily when composing the final layout. Final artwork must not cause
broken requests, distorted proportions, or avoidable layout shifts.
