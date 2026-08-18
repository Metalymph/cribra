# Silens Scan

Privacy-first Rust core for detecting, querying and safely transforming
secrets and sensitive data.

Silens Scan is designed as a reusable engine rather than an application
shell. Callers provide UTF-8 text and retain control of I/O, storage and
presentation. The core performs detection, validation, deterministic
normalization, reporting and share-safe transformations without storing
matched secret values inside public findings.

The serial core is suitable for native applications and WASM/PWA
integrations. Native callers can optionally enable Rayon-backed parallel
scanning across independent inputs.

## Capabilities

-   Deterministic UTF-8 scanning
-   Literal, prefix, suffix and regex rules
-   Capture-aware regex projection
-   Deterministic and contextual validators
-   Selectable built-in detector catalog
-   Immutable per-source `ScanReport`
-   Ordered batch `ScanResults<K>`
-   Lazy query/filter API with explicit sorting
-   Aggregated `ScanSummary`
-   Optional remediation guidance per finding
-   Safe redaction
-   Semantic template generation
-   Deterministic keyed pseudonymization
-   Deterministic keyed synthetic replacements
-   Share-bundle construction
-   Optional Serde support
-   Optional Rayon parallel batch scanning
-   Unicode-aware source locations
-   Stable serial/parallel ordering
-   Reusable cached built-in scanner
-   No matched secret value stored in public findings
-   Presentation-safe rule metadata with matcher/detection mode introspection
-   Separate `SensitiveCandidate` model for ambiguous review-worthy values


## Detection pipeline

Silens Scan keeps detection evidence and ambiguous review candidates separate.

```text
caller-owned UTF-8 input
        │
        ▼
compiled matcher groups
        │
        ▼
raw matched candidates
        │
        ├─────────────── rule validator ───────────────┐
        │                                              │
        │                                  insufficient / invalid
        │                                              │
        │                                              ▼
        │                                           reject
        │
        ▼
accepted rule candidates
        │
        ▼
deterministic normalization
        │
        ▼
Finding
├── rule id
├── location
├── severity
├── confidence
└── optional remediation

separate structural review path
        │
        ▼
narrow ambiguous-shape detector
        │
        ▼
SensitiveCandidate
├── kind
├── location
└── evidence
```

A `Finding` means the configured rule pipeline has enough evidence to classify
a span. A `SensitiveCandidate` does **not** mean that a credential was detected:
it represents a structurally plausible value worth reviewing when semantic
evidence is insufficient.

This separation is intentional:

- candidate evidence is not finding `Confidence`;
- candidates have no `Severity` or `Remediation`;
- candidates never store or expose the matched source value;
- share-safe transformations operate on findings, not ambiguous candidates;
- presentation layers can explain/review candidates without weakening finding
  semantics.

### Detection modes

Public `RuleMetadata` exposes how configured rules validate matched spans:

```text
MatcherOnly
    matcher itself is authoritative

Deterministic
    candidate structure is sufficient for validation

Contextual
    surrounding source context contributes to validation
```

For example, a provider token with a distinctive documented shape can be
validated deterministically, while a generic password or API-key-shaped value
usually needs a recognizable surrounding configuration key.

### Ambiguous recovery-like values

The initial v0.2 structural candidate detector deliberately recognizes only a
narrow grouped shape:

```text
ABCD-EFGH-IJKL-MNOP
```

The current structural contract requires four groups of four uppercase ASCII
alphanumeric characters separated by hyphens and clear token boundaries.
Numeric-only values, hexadecimal-only values, lowercase/mixed-case variants,
obvious placeholders and longer/partial tokens are rejected.

This is not a recovery-code `Finding`. The same shape may also represent an
activation code, coupon, license key or application identifier. Silens Scan
therefore records only structural evidence until stronger semantic context
exists.

From `0.2.3c`, candidate emission is part of the normal per-source scan path.
`ScanReport` owns findings and ambiguous candidates as separate immutable
collections. If a structural candidate overlaps an accepted finding, the
finding wins and the redundant ambiguous candidate is discarded.


#### Candidate result semantics

The candidate channel remains separate throughout the public result model:

```text
ScanReport
├── findings()       classified detections
├── candidates()     review-only structural candidates
└── needs_review()   either channel is non-empty

ScanResults
├── findings()       flattened confirmed findings
├── candidates()     flattened ambiguous candidates
├── failed()         sources containing findings
├── review()         candidate-only sources
└── clean()          sources containing neither

ScanSummary
├── total_findings()
├── total_candidates()
├── reports_with_candidates()
├── has_candidates()
└── is_clean()       false when either channel requires review
```

Candidates are deliberately excluded from ScanQuery and from all share-safe
transformations. An isolated recovery-like candidate is therefore never
automatically redacted or rewritten.

If caller or domain knowledge establishes that the same value is sensitive, a
custom rule can classify that span as a Finding. The confirmed finding then
suppresses the overlapping ambiguous candidate.

The regression corpus freezes both sides of this boundary. It covers positive
ambiguous examples as well as non-candidates such as numeric-only groups,
hexadecimal groups, obvious placeholders, UUIDs, commit/checksum-style hashes,
package-integrity hashes, version/date strings, lowercase or mixed-case
variants, and malformed or extended grouped values.


## Install

``` toml
[dependencies]
silens-scan = "0.1"
```

Optional features are deliberately independent:

``` toml
silens-scan = { version = "0.1", features = ["serde", "parallel"] }
```

  Feature      Default   Purpose
  ------------ --------- -------------------------------------------------
  `serde`      no        Serialize and deserialize public data contracts
  `parallel`   no        Scan independent inputs concurrently with Rayon

The default build keeps the core serial and does not pull Rayon or
Serde.

## Quick start

Scanning always uses a batch-oriented API. One source is simply a
one-element batch.

``` rust
use silens_scan::Scanner;

let scanner = Scanner::default();

let results = scanner.scan([
    ("config.env", "STRIPE_SECRET_KEY=sk_live_example"),
    ("settings.toml", "log_level = \"info\""),
]);

println!("{}", results.summary());

for (source, finding) in results.findings() {
    println!(
        "{source}:{}:{} {}",
        finding.location().line(),
        finding.location().column(),
        finding.rule_id(),
    );
}
```

Caller-provided keys are preserved in the result. Each `ScanEntry<K>`
also retains the original source byte length, while `Finding` contains
metadata and coordinates rather than a copy of the matched secret.

## Detection model

The built-in scanner combines multiple matching families behind one
compiled execution pipeline:

``` text
UTF-8 source
    │
    ▼
CompiledRuleSet
    │
    ├── Aho-Corasick ── literal / prefix
    ├── regex engine ── pattern / capture
    └── specialized ─── suffix
    │
    ▼
candidate validation
    │
    ├── deterministic validators
    └── contextual validators
    │
    ▼
deterministic normalization
    │
    ▼
line / Unicode-column finalization
    │
    ▼
ScanReport → ScanResults<K>
```

A single source is scanned serially. With `parallel`, independent
sources are distributed through Rayon while the same immutable scanner
is reused.

### Built-in detection

The current built-in catalog covers detector families for:

-   GitHub
-   Stripe
-   Cloudflare
-   Slack
-   Telegram
-   JWT
-   AWS
-   Azure
-   GCP
-   password and passphrase fields
-   sensitive hashes
-   generic API keys, tokens and secrets

The canonical selectable built-in pack is exposed as
`builtins::CURRENT`.

Custom rules can be composed with the builder:

``` rust
use silens_scan::{Remediation, Rule, Scanner, Severity};

let scanner = Scanner::builder()
    .rule(
        Rule::prefix("acme.api-key", "acme_", Severity::Critical)
            .with_remediation(Remediation::RotateCredential),
    )
    .rule(
        Rule::literal("internal.marker", "PRIVATE_VALUE", Severity::High)
            .with_remediation(Remediation::RemoveSensitiveValue),
    )
    .build()?;

# Ok::<(), silens_scan::ScannerBuildError>(())
```

## Reports, queries and summaries

`ScanResults<K>` is the batch result. It exposes direct iteration as
well as a lazy query surface.

``` rust
use silens_scan::{ScanSort, Scanner, Severity};

let scanner = Scanner::default();
let results = scanner.scan([
    ("a.env", "TOKEN=example"),
    ("b.env", "MODE=production"),
]);

let high_priority = results
    .query()
    .minimum_severity(Severity::High)
    .sort(ScanSort::Location);

for (source, finding) in high_priority.iter() {
    println!("{source}: {}", finding.rule_id());
}

let summary = results.summary();
println!("{summary}");
```

Queries can filter by exact or minimum severity, exact or minimum
confidence, exact rule identifier, and convenience predicates such as
critical/high-priority/high-confidence. Sorting is explicit and happens
after lazy filtering.

`ScanQuery` intentionally operates on confirmed findings only. Ambiguous
candidates are exposed separately through `ScanReport::candidates()` and
`ScanResults::candidates()` because review evidence has no finding severity or
confidence semantics.

Candidate-only sources are available through `ScanResults::review()`. Sources
containing neither findings nor candidates are returned by
`ScanResults::clean()`.

`ScanSummary` reports:

-   sources and bytes scanned
-   reports with and without findings
-   reports containing ambiguous candidates
-   total findings and total ambiguous candidates
-   counts per severity
-   finding, candidate-review and clean-state helpers

## Remediation

A finding may carry an optional `Remediation`. Remediation is
presentation-safe guidance attached to the rule, not an automatic
mutation of the source.

``` rust
for (_, finding) in results.findings() {
    if let Some(remediation) = finding.remediation() {
        println!("{}: {}", remediation.label(), remediation.message());
    }
}
```

This allows applications to present actionable guidance such as rotating
a credential, replacing a private key, changing a password or removing a
sensitive value without embedding application-specific copy into the
scanner pipeline.

## Ambiguous sensitive values

Silens Scan separates confirmed detections from values that are only structurally suspicious.

```text
caller-owned UTF-8 input
        │
        ├── compiled detection pipeline
        │        │
        │        ▼
        │     Finding
        │
        │     enough evidence to classify
        │
        └── structural review path
                 │
                 ▼
          SensitiveCandidate
```

## Transformations

The `transform` module turns a `ScanReport` plus the original source
into share-safe output.

### Redaction

Redaction is the conservative transform. It replaces detected spans and
safely merges overlaps.

``` rust
use silens_scan::{Rule, Scanner, Severity, transform::redact};

let scanner = Scanner::builder()
    .rule(Rule::literal("credential", "SECRET", Severity::High))
    .build()?;

let source = "TOKEN=SECRET";
let results = scanner.scan([("memory", source)]);
let report = results.single_report().expect("one report");

assert_eq!(redact(source, report)?, "TOKEN=[REDACTED]");

# Ok::<(), Box<dyn std::error::Error>>(())
```

Use `redact_with` when a custom replacement marker is required.

### Semantic templates

Templates preserve the semantic rule identity while discarding the
matched value:

``` text
TOKEN=<SILENS:credential>
```

`TemplateOptions` can configure the namespace and deterministic per-rule
numbering. Template generation rejects ambiguous overlapping findings
instead of guessing which semantic placeholder should win.

### Pseudonymization

`pseudonymize` replaces findings with deterministic keyed pseudonyms.
The same input value under the same key produces the same pseudonym,
allowing correlation without retaining the original value.

``` rust
use silens_scan::transform::PseudonymizationOptions;

let options = PseudonymizationOptions::new([0x31; 32]);
```

The key is supplied by the caller and is not part of a public finding.

### Synthetic replacements

`synthesize` creates deterministic keyed synthetic values. For supported
detector families it preserves useful structural characteristics while
intentionally producing validator-invalid output, so generated material
can be used safely in examples and shareable artifacts.

``` rust
use silens_scan::transform::SynthesisOptions;

let options = SynthesisOptions::new([0x53; 32]);
```

### Share bundles

`ShareBundle` applies one explicit transformation mode to a batch while
preserving source keys and a manifest containing the transformation
mode, summary and generation time.

Available modes are represented by `ShareMode`; the manifest exposes the
non-secret `ShareModeKind`.

Use share bundles when an application needs to package transformed
sources and scan metadata together for export, support or collaboration
workflows.

## Canonical examples and golden corpus

The repository contains a canonical fixture corpus under:

``` text
examples/fixtures/
├── inputs/
└── outputs/
    ├── reports/
    ├── redacted/
    ├── templates/
    ├── pseudonymized/
    └── synthesized/
```

These are not hand-written expected outputs.
`examples/generate_fixtures.rs` runs the real scanner and transformation
APIs to regenerate them:

``` text
cargo run --example generate_fixtures --features serde
```

Golden tests independently generate the same artifacts in memory and
compare them with the committed outputs. This keeps documentation
examples tied to actual crate behavior.

The shared corpus scanner intentionally exercises custom prefix, regex
and literal rules plus remediation metadata.

## Parallel scanning

Enable `parallel` to scan independent sources concurrently:

``` toml
silens-scan = { version = "0.1", features = ["parallel"] }
```

``` rust
let results = scanner.parallel_scan([
    ("config.env", env_source),
    ("settings.toml", toml_source),
    ("service.json", json_source),
]);
```

Serial and parallel execution use the same per-source pipeline and
preserve input order. Silens Scan does not split one source into chunks
and does not create a private thread pool; Rayon uses the current pool.

Parallelism is therefore an application choice, not a semantic
difference in scanning.

## Serde

Enable `serde` when results need to cross an application boundary or be
persisted/serialized:

``` toml
silens-scan = { version = "0.1", features = ["serde"] }
```

``` rust
let json = serde_json::to_string_pretty(&results)?;
```

The canonical fixture generator uses this feature for JSON report
artifacts.

## Coordinate contract

Locations have an explicit mixed coordinate contract:

-   `start`: zero-based UTF-8 byte offset, inclusive
-   `end`: zero-based UTF-8 byte offset, exclusive
-   `line`: one-based
-   `column`: one-based Unicode scalar position

Byte offsets are suitable for exact source slicing. Line/column values
are presentation-oriented.

Silens Scan does not copy the matched source value into a public
`Finding`.

## Privacy and trust boundary

Silens Scan is a core library. It does not decide where source data
comes from or where transformed output is sent.

The intended boundary is:

``` text
application owns I/O
        │
        ▼
     &str input
        │
        ▼
   Silens Scan
        │
        ├── metadata-only findings
        └── explicit transformations
```

This makes the serial core suitable for local-first WASM/PWA and desktop
use: applications can keep scanning and transformation local to the
process/browser. Network access, remote repositories, uploads,
authentication and persistence belong to the integrating application
rather than this crate.

## Performance

Criterion benchmarks below were measured with Rust 1.97.1. They are
workload- and machine-specific engineering baselines, not portable
performance guarantees.

### Core scanning

  Workload                     Median time         Throughput
  -------------------------- ------------- ------------------
  Built-ins, 1 KiB                20.25 µs        66.81 MiB/s
  Built-ins, 64 KiB              253.90 µs       247.64 MiB/s
  Built-ins, 1 MiB                3.759 ms   **266.13 MiB/s**
  64 custom literal rules         48.09 µs     **1.27 GiB/s**
  512 custom literal rules        48.93 µs     **1.25 GiB/s**
  64 custom mixed rules          151.32 µs   **413.04 MiB/s**
  Full built-in pipeline         253.94 µs   **246.12 MiB/s**

### Match density

For a 64 KiB input:

  Density     Findings   Median time             Throughput
  --------- ---------- ------------- ----------------------
  None               0     211.97 µs       **294.85 MiB/s**
  Sparse             7      \~255 µs   **\~246--248 MiB/s**
  Dense           1496     \~2.85 ms         **\~22 MiB/s**

The dense fixture intentionally produces roughly one final finding every
43 bytes, making result production and validation a substantial part of
the workload.

### Parallel batch scanning

The benchmark scans 32 independent sources.

  Source size        Serial    Parallel   Approx. speedup
  ------------- ----------- ----------- -----------------
  4 KiB × 32       1.027 ms   244.49 µs             4.20×
  64 KiB × 32      8.161 ms    1.307 ms             6.24×
  1 MiB × 32      121.16 ms    18.24 ms             6.64×

Parallel throughput reaches approximately:

-   **563 MiB/s** for 4 KiB sources
-   **1.50 GiB/s** for 64 KiB sources
-   **1.71 GiB/s** for 1 MiB sources

### Build cost

  Ruleset                         Median build time
  ----------------------------- -------------------
  Built-in pack, cold compile              49.69 ms
  4 custom literal rules                   15.58 µs
  64 custom literal rules                  60.95 µs
  512 custom literal rules                299.24 µs

The default built-in scanner uses a shared compiled cache, so the cold
built-in compile cost is paid once per process and reused afterward.

### Performance policy

The `0.1.0` engine is performance-frozen unless a correctness issue is
found. Manual SIMD has deliberately not been introduced: current
workloads do not justify the extra architecture-specific complexity. It
can be reconsidered if real-world profiling exposes a concrete
bottleneck.

## Testing and hardening

The release test surface includes:

-   unit tests across rules, validators, reporting and transformations
-   doctests for public APIs
-   canonical end-to-end fixtures
-   golden output verification
-   serial/parallel equivalence
-   deterministic transformation checks
-   UTF-8 scalar-column checks
-   CRLF preservation
-   empty-input behavior
-   dense repeated-finding stress
-   large single-line input
-   Serde round-trip coverage

The canonical corpus is intended to remain part of the public
documentation as the crate evolves.

## Scope

Silens Scan intentionally stops at the reusable scanning core.

Application-level concerns such as file selection, remote repository
loading, authenticated workflows, uploads, UI, persistence and
subscription features belong to consumers such as the Silens Scan web
application and Silens Studio.

## Release status

`0.1.0` is the first published crates.io release.

Development toward `0.2.0` focuses on detection quality, contextual
classification, ambiguous sensitive-value review, explainability and stronger
public metadata while preserving the local-first privacy boundary and the
separation between confirmed findings and review-only candidates.

## License

MIT
