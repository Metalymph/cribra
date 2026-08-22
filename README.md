# Cribra

Embeddable, local-first Rust engine for detecting, reviewing, and safely transforming secrets and sensitive data.

Cribra is designed as a reusable engine rather than an application
shell. Callers provide UTF-8 text and retain control of I/O, storage and
presentation. The core performs detection, validation, deterministic
normalization, reporting and share-safe transformations without storing
matched secret values inside public findings.

The serial core is suitable for native applications and WASM/PWA
integrations. Native callers can optionally enable Rayon-backed parallel
scanning across independent inputs.

> **Project rename:** the reusable engine was published as `silens-scan`
> through the `0.1.x` line. Starting with `0.2.0`, the engine is named
> **Cribra** and the Rust package/crate is `cribra`. **Silens Scan** remains
> the Silens web product powered by Cribra.

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

Cribra keeps detection evidence and ambiguous review candidates separate.

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
activation code, coupon, license key or application identifier. Cribra
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
cribra = "0.2"
```

Optional features are deliberately independent:

``` toml
cribra = { version = "0.2", features = ["serde", "parallel"] }
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
use cribra::Scanner;

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

## Custom rules and domain knowledge

Custom rules let the integrating application declare sensitive values that
Cribra cannot infer from generic structure alone.

They use the same public rule model as the scanner core:

```text
application/domain knowledge
        │
        ▼
      Rule
        │
        ├── literal
        ├── prefix
        ├── suffix
        └── pattern
        │
        ▼
  ScannerBuilder
        │
        ▼
      Finding
```

A custom rule is matcher-authoritative. Its public metadata therefore reports
`DetectionMode::MatcherOnly`; private built-in validators are not exposed to
custom rules.

``` rust
use cribra::{Remediation, Rule, Scanner, Severity};

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

# Ok::<(), cribra::ScannerBuildError>(())
```

Built-ins and custom rules can be composed in the same scanner:

``` rust
use cribra::{Rule, Scanner, Severity, builtins};

let scanner = Scanner::builder()
    .builtins(builtins::CURRENT)
    .rule(Rule::literal(
        "acme.recovery-code",
        "ABCD-EFGH-IJKL-MNOP",
        Severity::Critical,
    ))
    .build()?;

# Ok::<(), cribra::ScannerBuildError>(())
```

Rule identifiers are scanner-wide identities. Duplicate IDs are rejected,
including collisions between a custom rule and a built-in rule. Distinct IDs
may intentionally use identical matchers.

Custom regular-expression rules use the full regex match as the finding span.
Capture projection remains an internal built-in capability rather than a public
custom-rule feature.

Patterns that can produce a zero-length match are rejected at construction
time. This prevents empty findings from anchors, optional expressions and other
zero-width patterns from entering the scan pipeline.

For example:

``` rust
use cribra::{Rule, RuleError, Severity};

let error = Rule::pattern("bad", r".*", Severity::High)
    .expect_err("zero-length-capable patterns are rejected");

assert!(matches!(error, RuleError::PatternMatchesEmpty));
```

Custom rules are also the explicit escape hatch for ambiguous values. A value
such as:

``` text
ABCD-EFGH-IJKL-MNOP
```

may only be a `SensitiveCandidate` when scanned generically. If an application
knows that this exact value or shape is a real credential in its domain, a
custom rule can classify it as a normal `Finding`. The confirmed finding then
suppresses the overlapping ambiguous candidate.

This keeps the scanner honest:

```text
built-in knowledge
        +
structural ambiguity
        +
application/domain knowledge
        │
        ▼
      Scanner
```

## Reports, queries and summaries

`ScanResults<K>` is the batch result. It exposes direct iteration as
well as a lazy query surface.

``` rust
use cribra::{ScanSort, Scanner, Severity};

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

Cribra separates confirmed detections from values that are only
structurally suspicious.

```text
ScanReport
├── findings()
│   └── classified detections
├── candidates()
│   └── review-only structural candidates
└── needs_review()
    └── true when either channel is non-empty

ScanResults
├── findings()
├── candidates()
├── failed()   -> sources containing findings
├── review()   -> candidate-only sources
└── clean()    -> sources containing neither
```

A `SensitiveCandidate` is deliberately not a `Finding`:

- it has no `Severity`;
- it has no finding `Confidence`;
- it has no `Remediation`;
- it never stores the matched source value;
- it does not participate in `ScanQuery`;
- it is never transformed automatically.

The current structural candidate detector recognizes a narrow recovery-like
grouped form such as:

```text
ABCD-EFGH-IJKL-MNOP
```

This does not mean the value is a recovery credential. The same structure can
represent an activation code, license key, coupon or application identifier.
The scanner therefore emits only structural review evidence unless a normal
rule provides enough evidence to classify the span.

If a confirmed finding overlaps a structural candidate, the finding wins and
the ambiguous candidate is suppressed.

## Explainability

Cribra exposes typed explanation facts without introducing a second
classification authority or presentation copy into the core.

```text
caller-owned UTF-8 input
        │
        ├──────────────── rule-backed detection ────────────────┐
        │                                                       │
        │                                                       ▼
        │                                                compiled matcher
        │                                                       │
        │                                                       ▼
        │                                                   validator
        │                                                       │
        │                                    ┌──────────────────┴──────────────────┐
        │                                    │                                     │
        │                              insufficient                           accepted
        │                                    │                                     │
        │                                    ▼                                     ▼
        │                                  reject                               Finding
        │                                                                          │
        │                                                                          │
        │                                                        Scanner rule metadata
        │                                                                          │
        │                                                                          ▼
        │                                                   Explanation::Classified
        │                                                   ├── MatcherOnly
        │                                                   ├── Deterministic
        │                                                   └── Contextual
        │
        └──────────────── structural review path ───────────────┐
                                                                │
                                                                ▼
                                                     ambiguous-shape detector
                                                                │
                                                                ▼
                                                       SensitiveCandidate
                                                                │
                                                                │
                                                        CandidateEvidence
                                                                │
                                                                ▼
                                                    Explanation::Ambiguous
```

`Explanation` is deliberately presentation-agnostic:

```rust
use cribra::{CandidateEvidence, DetectionMode, Explanation};

let classified = Explanation::classified(DetectionMode::Contextual);
let ambiguous = Explanation::ambiguous(CandidateEvidence::Structural);

assert!(classified.is_classified());
assert!(ambiguous.is_ambiguous());
```

For a confirmed `Finding`, explanation is resolved against the `Scanner` that
owns the compiled rule metadata:

```rust
let explanation = finding.explanation(&scanner);
```

The finding does not duplicate detection mode or other rule authority.
Resolution returns `None` when the supplied scanner cannot resolve the finding
unambiguously, including when compatible rule identities imply conflicting
explanations.

For a `SensitiveCandidate`, explanation is derived directly from its existing
candidate evidence:

```rust
let explanation = candidate.explanation();
```

This distinction preserves the result model:

```text
Finding
    │
    │ stable rule identity
    ▼
Scanner / RuleMetadata
    │
    ▼
DetectionMode
    │
    ▼
Explanation::Classified

SensitiveCandidate
    │
    ▼
CandidateEvidence
    │
    ▼
Explanation::Ambiguous
```

Explanation facts are not embedded as duplicate state in `Finding`,
`SensitiveCandidate`, `ScanReport` or `ScanResults`. With the `serde` feature,
`Explanation` itself is serializable when an application explicitly needs the
derived explanation contract.

The core intentionally does not provide human-facing explanation strings.
Applications such as a WASM/PWA interface, desktop application, service,
middleware or custom integration can map the typed facts to their own copy and
presentation without changing scanner semantics.

### Evidence semantics

The public evidence vocabulary has two independent axes:

| Result | Classification authority | Explanation fact | Severity | Finding confidence | Remediation |
| --- | --- | --- | --- | --- | --- |
| `Finding` | configured rule pipeline | `Classified(DetectionMode)` | yes | yes | optional |
| `SensitiveCandidate` | structural review detector | `Ambiguous(CandidateEvidence)` | no | no | no |

`DetectionMode` describes **how a rule-backed detection is validated**.
`CandidateEvidence` describes **why an unclassified value is worth review**.
Neither should be inferred from the other.

This is also why ambiguous candidates remain outside `ScanQuery` and automatic
share-safe transformations: review evidence is not silently promoted into a
classified secret.

## Transformations

The `transform` module turns a `ScanReport` plus the original source
into share-safe output.

### Redaction

Redaction is the conservative transform. It replaces detected spans and
safely merges overlaps.

``` rust
use cribra::{Rule, Scanner, Severity, transform::redact};

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
TOKEN=<CRIBRA:credential>
```

`TemplateOptions` can configure the namespace and deterministic per-rule
numbering. Template generation rejects ambiguous overlapping findings
instead of guessing which semantic placeholder should win.

### Pseudonymization

`pseudonymize` replaces findings with deterministic keyed pseudonyms.
The same input value under the same key produces the same pseudonym,
allowing correlation without retaining the original value.

``` rust
use cribra::transform::PseudonymizationOptions;

let options = PseudonymizationOptions::new([0x31; 32]);
```

The key is supplied by the caller and is not part of a public finding.

### Synthetic replacements

`synthesize` creates deterministic keyed synthetic values. For supported
detector families it preserves useful structural characteristics while
intentionally producing validator-invalid output, so generated material
can be used safely in examples and shareable artifacts.

``` rust
use cribra::transform::SynthesisOptions;

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
cribra = { version = "0.2", features = ["parallel"] }
```

``` rust
let results = scanner.parallel_scan([
    ("config.env", env_source),
    ("settings.toml", toml_source),
    ("service.json", json_source),
]);
```

Serial and parallel execution use the same per-source pipeline and
preserve input order. Cribra does not split one source into chunks
and does not create a private thread pool; Rayon uses the current pool.

Parallelism is therefore an application choice, not a semantic
difference in scanning.

## Serde

Enable `serde` when results need to cross an application boundary or be
persisted/serialized:

``` toml
cribra = { version = "0.2", features = ["serde"] }
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

Cribra does not copy the matched source value into a public
`Finding`.

## Privacy and trust boundary

Cribra is a core library. It does not decide where source data
comes from or where transformed output is sent.

The intended boundary is:

``` text
application owns I/O
        │
        ▼
     &str input
        │
        ▼
   Cribra
        │
        ├── metadata-only findings
        └── explicit transformations
```

This makes the serial core suitable for local-first WASM/PWA and desktop
use: applications can keep scanning and transformation local to the
process/browser. Network access, remote repositories, uploads,
authentication and persistence belong to the integrating application
rather than this crate.

Cribra itself performs no network access. The core accepts
caller-provided UTF-8 text and returns local structured results.

Network access, repository loading, uploads, authentication, persistence,
filesystem traversal and presentation belong exclusively to the integrating
application.

## Performance

Criterion results are machine- and workload-specific engineering baselines, not
portable performance guarantees. The final v0.2 regression baseline was
measured with Rust 1.97.1.

| Representative workload | Median time | Throughput |
| --- | ---: | ---: |
| Built-ins, 64 KiB | 535.27 µs | 117.47 MiB/s |
| Built-ins, 1 MiB | 8.0754 ms | 123.88 MiB/s |
| No findings, 64 KiB | 212.41 µs | 294.24 MiB/s |
| Realistic v0.2 mixed source, 64 KiB | 268.29 µs | 233.39 MiB/s |
| 64 custom literal rules | 75.227 µs | 830.82 MiB/s |
| 512 custom literal rules | 74.567 µs | 839.61 MiB/s |

v0.2 performs substantially more semantic work than v0.1, including expanded
contextual classification and a separate ambiguous-candidate channel. The final
performance pass uses conservative contextual prefiltering and a shared
contextual gate so clean-path cost does not scale linearly with the contextual
regex portfolio. Prefilters only eliminate impossible work; rule matching and
validation remain authoritative.

See [`docs/PERFORMANCE.md`](docs/PERFORMANCE.md) for benchmark policy,
diagnostic suites, the complete v0.2 baseline and interpretation.

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

### Detection regression corpus

The v0.2 regression corpus freezes detector behavior beyond isolated unit tests.
It covers representative positive detections, contextual values without semantic
keys, ambiguous review-only values, known false-positive families, adversarial
opaque identifiers, malformed provider-shaped values, punctuation boundaries,
common ENV/JSON/YAML/TOML/plain-text representations, LF/CRLF equivalence,
Unicode coordinates, exact-span collisions, partial overlaps and
finding/candidate interaction.

Corpus failures are reviewed as semantic changes. Expectations are not relaxed
or regenerated merely to make the suite pass.


## Scope

Cribra intentionally stops at the reusable scanning core.

Application-level concerns such as file selection, remote repository
loading, authenticated workflows, uploads, UI, persistence and
subscription features belong to consumers such as the Silens Scan web
application and Silens Studio desktop app.

## Release status

`0.1.0` is the first published crates.io release.

Development toward `0.2.0` focuses on detection quality, contextual
classification, ambiguous sensitive-value review, explainability and stronger
public metadata while preserving the local-first privacy boundary and the
separation between confirmed findings and review-only candidates.

## Development

Cribra ships both a `justfile` and an equivalent `Makefile`. `just` is the
preferred local interface; Make is provided for contributors, CI-like
environments, and automation that already standardizes on it.

| Command | Purpose |
| --- | --- |
| `just gate` | Normal local quality gate |
| `just release-gate` | MSRV + security + package + publication dry run |
| `just test-all` | Test all native features |
| `just wasm` | Check the default browser/WASM contract |
| `just wasm-serde` | Check browser/WASM with Serde |
| `just audit` | Run RustSec with `cargo audit` |
| `just bench-all` | Run maintained benchmark suites |
| `just clean` | Remove Cargo build artifacts |

Equivalent targets are available as `make gate`, `make release-gate`,
`make wasm`, and so on.

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for the complete development,
validation, and pull-request workflow.

## License

MIT
