# Cribra v0.2 detection corpus

This directory contains corpus-level fixtures added for Cribra `0.2.7`.

The focused regression suites under `tests/` remain authoritative for individual
detector semantics. This corpus exercises broader boundaries across the complete
default scanner.

## Categories

- `positive.env` — representative deterministic and contextual detections.
- `contextual-isolated.txt` — credential-shaped values without recognized
  surrounding keys; these must not become contextual findings.
- `ambiguous.txt` — recovery-like grouped values that must remain review-only
  `SensitiveCandidate` values.
- `false-positives.txt` — UUIDs, hashes, package integrity values, build/request
  identifiers, dates, versions, placeholders and Unicode text that must remain
  clean.

All credential-looking values are synthetic and intentionally unusable.

## Policy

Corpus changes are semantic changes.

A failing corpus test must be reviewed before changing either a detector or an
expected fixture. Do not regenerate/relax expectations merely to make the suite
green.
