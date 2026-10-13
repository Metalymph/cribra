# Owned incremental documents — unreleased v0.6.0

D1–D4 are implemented and locally qualified; D5 is documentation and
release-readiness work. This API is not present in published core v0.5.0 and
is not exposed by current CLI, C or WASM adapters.

The additive Rust API owns a mutable UTF-8 document and its current complete
`ScanReport`. Scanner compilation is shared through immutable configuration.
Every successful replacement edit has exactly the whole-source `Scanner::scan`
report semantics, including validation confidence, coordinates, duplicates,
priority resolution, overlapping findings and structural suppression.

```rust
use cribra::{IncrementalLimits, Rule, Scanner, Severity};

let scanner = Scanner::builder()
    .rule(Rule::literal("credential", "SECRET", Severity::High))
    .build()?;
let limits = IncrementalLimits::default()
    .with_max_candidates(Some(10_000))
    .with_max_retained_cache_records(Some(20_000))
    .with_max_materialized_results(Some(10_000))
    .with_max_transaction_storage_bytes(Some(8 * 1024 * 1024));
let mut document = scanner.incremental_session_with_limits("SEC".into(), limits)?;
document.apply_edit(0, 3..3, "RET")?;
assert_eq!(document.revision(), 1);
assert_eq!(document.report().len(), 1);
# Ok::<(), Box<dyn std::error::Error>>(())
```

`incremental_session(String)` leaves optional limits unlimited. The session can
outlive its scanner. `source()`, `revision()` and `report()` borrow the last
committed snapshot; Rust prevents those borrows from surviving a mutable edit.
A cloned report belongs to the caller and is outside session accounting.

An edit replaces a half-open byte range using the expected revision. Insertions,
deletions and replacements share this operation. Range endpoints must be UTF-8
scalar boundaries. Every successful operation, including a no-op, increments the
revision once. There is no feed/EOF/finalize stage for this mutable-document API.

## Exactness and work

Literal start ownership and structural token boundaries are selectively rescanned
on the complete haystack. Cached contextual validators are reused only when their
exact source dependencies remain unchanged. Prefix, Suffix and all regex rules,
including per-rule prefilters and projected captures, retain authoritative
whole-rule fallback. No matcher or validator is compiled again during edits.

Global normalization, coordinate construction and structural suppression remain
shared with ordinary scanning. The source is copied transactionally and caches
are traversed; local matching does not imply sublinear total update cost.
Built-in Core, Personal, Financial and Combined portfolios are supported through
fallback. This does **not** change their streaming compatibility classification.

## Four independent optional limits

| Limit | Meaning |
| --- | --- |
| `max_candidates` | New raw rule emissions per construction/edit, including rejected candidates and duplicate emissions. Reuse/revalidation and structural detection do not consume this counter. |
| `max_retained_cache_records` | Peak simultaneously retained old plus staged rule and unsuppressed structural records. Rejected and normalization-losing records count. |
| `max_materialized_results` | Findings constructed after normalization plus structural result copies before suppression, per operation. Suppression refunds no tickets. |
| `max_transaction_storage_bytes` | Conservative peak logical payload of controlled session/transaction buffers, including old and staged snapshots. |

All checks precede their controlled ownership acquisition. Raw emissions are
intercepted at shared emission sites, including fallback and prefiltered regex
paths; there is no intermediate unbounded raw candidate vector.

Cache ownership counts separate physical record storage. The old and staged
caches are separate allocations and both count. Shared compiled rule metadata
and automata do not become additional cache records. Structural span reservations
transfer to structural cache identities without charging the same identity twice;
the temporary span payload and destination payload both count while they coexist.

For example, retaining one literal record can require two cache slots during a
no-op edit. Limits reset their per-operation counters only when that operation
ends; cache accounting begins with the still-owned current snapshot. They are
not lifetime quotas. A suppressed structural observation still occupies a cache
slot and consumed a result-materialization ticket. A normalization loser occupies
a cache slot, but is not a constructed public finding.

The incremental result limit is intentionally different from streaming
`max_retained_results`: incremental caches already retain pre-normalization
records, whereas the streaming limit charges accepted records before EOF
normalization. The old report contributes transaction bytes, not new operation
materialization tickets.

## Logical payload accounting

A snapshot includes:

- document byte length;
- per-rule heap array of `Vec<CachedCandidate>` headers;
- all cached rule records and unsuppressed structural records;
- final report finding and structural arrays.

The session's literal-length array counts once. Updates also account for new
source storage, ownership ranges, structural staging spans, accepted-candidate
projection, finding construction and unsuppressed structural result clones.
All arithmetic uses checked counts and `size_of` for the compiled target.
Each controlled contiguous allocation must also fit Rust's representable size.

Reservations remain active until buffers drop or ownership transfers. Truncating
or clearing a vector does not demonstrate released capacity. Box conversion may
relocate a vector, so admission conservatively reserves a destination payload
while the source reservation is still live; exact allocator behavior is not
assumed. This can reject a transaction whose allocator would reuse storage.

The budget is **not a total heap-memory cap**. It excludes:

- Vec/String spare capacity and allocator overhead or reallocation machinery;
- fixed stack/state metadata;
- shared compiled regex/Aho-Corasick automata and rule strings;
- regex-library search caches and capture workspace;
- authoritative validator allocations, such as lowercase/context strings,
  decoded credential buffers, base64 re-encoding and parsing scratch;
- caller-owned replacement input and report clones.

These opaque/existing workspaces can depend on candidate size. Incremental
sessions own the full document; unlike bounded streaming, retention is not
bounded by the ruleset. No source-length policy, custom allocator or allocator
failure recovery is introduced.

## Failure and transaction contract

Input errors are checked in this order: activity, revision, range, UTF-8
boundaries, revision increment and representable replacement length. Stale
revision, invalid range/boundaries and edit arithmetic errors leave the session
active and unchanged.

After preflight, a resource-accounting or execution-invariant failure is terminal.
The first failure stops execution, staged ownership is dropped, and the old
source, revision, caches and complete report remain available. `is_active()` is
false; further edits return `NotActive`. There is no partially committed report,
limit-reset method or recovery/reset operation. Constructing another session is
explicit. Constructor failure returns no session and drops its owned input.

Errors and the session's redacted `Debug` contain no document text. Panic or
allocator exhaustion recovery is outside this contract.

See [performance methodology](PERFORMANCE.md#incremental-d4-methodology) and
[D4 qualification evidence](0.6-d4-qualification.md).
