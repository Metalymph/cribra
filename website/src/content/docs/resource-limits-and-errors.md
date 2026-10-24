---
title: Resource Limits & Errors
description: Streaming admission, incremental budgets, failure semantics, and memory caveats.
order: 21
---

# Resource Limits & Errors

## Streaming admission

Streaming is **opt-in and configuration-dependent**. `Scanner::stream()` does not make every scanner streamable: unsupported matcher/validator combinations are rejected at admission, with **no silent whole-source fallback**. For example, the complete built-in portfolio and per-rule regex prefilters are not automatically admissible. Check `docs/streaming.md` before selecting rules for chunked execution.

`push(&[u8])` accepts ordered fragments, including fragments that split UTF-8 scalars. Genuine EOF with incomplete UTF-8 fails. Only a successful consuming `finish()` produces a complete report. Execution failures are terminal; subsequent `push`/`finish` operations return inactive-session errors. If a push encounters an execution failure before a later transport-validation failure, the first execution failure takes precedence.

### Streaming limits

| Limit | Counts or bounds |
| --- | --- |
| `max_candidates` | Cumulative raw rule emissions, including validator-rejected emissions. |
| `max_retained_results` | Accepted rule and structural review records retained **before** final normalization/suppression. |
| `max_stream_storage_bytes` | Checked **logical executor payload** admitted before execution allocation. |

All three are optional and default to no configured limit. None caps total source length or all heap allocations; caller buffers, output records, compiled automata, allocator overhead and other workspace may lie outside the logical budget.

## Incremental transactions

Incremental sessions retain a full mutable UTF-8 document. Edits use checked half-open byte ranges and an expected revision. Valid edits stage the new source, caches and report, then publish **only after complete success**. Invalid edits and arithmetic overflows are recoverable. Resource or internal-invariant failures leave the previous committed snapshot readable but make the session terminal; no partial staged report is exposed.

### Incremental limits

| Limit | Counts or bounds |
| --- | --- |
| `max_candidates` | New raw rule emissions per construction/edit; rejected and duplicate emissions count. |
| `max_retained_cache_records` | Peak old **plus** staged rule and structural cache records. |
| `max_materialized_results` | Results constructed before final suppression, per operation. |
| `max_transaction_storage_bytes` | Conservative peak logical session/transaction payload, including coexisting snapshots. |

Limits are independent and optional. Streaming's retained-result limit and incremental's materialized-result limit measure **different stages**; do not compare their values as equivalent. Incremental logical accounting excludes allocator spare capacity, compiled state, regex/validator workspace and caller-owned clones. Local rescanning does **not** imply sublinear end-to-end edit cost.

## Error handling policy

1. Distinguish configuration/admission errors from errors during source execution.
2. Treat a streaming execution failure as terminal; create a fresh session if retrying.
3. For incremental edits, check whether the error is recoverable or terminal before attempting further changes.
4. Never publish a partially constructed report or treat budget admission as an operating-system memory cap.
5. Propagate errors without appending original sensitive source to diagnostics.

**Source of truth:** `docs/streaming.md`, `docs/incremental.md`, `docs/ARCHITECTURE.md`.

## Related guides

- [Streaming scanning](/docs/streaming-scanning/)
- [Incremental scanning](/docs/incremental-scanning/)
- [Troubleshooting](/docs/troubleshooting/)
