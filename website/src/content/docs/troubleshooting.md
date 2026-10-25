---
title: Troubleshooting
description: Diagnose scanning admission, UTF-8 edits, resource failures and adapter mismatches.
order: 23
---

# Troubleshooting

Use the reported error and the relevant integration contract rather than retrying blindly. Cribra processes caller-provided input; it does not discover files, configure application logging, or select a destination for output.

## A scanner cannot start streaming

Streaming admission is configuration-dependent. The default scanner and complete built-in portfolios are not automatically streamable. Unsupported rules or validator combinations are rejected before processing; there is no implicit whole-source fallback. Use a supported custom ruleset or [whole-source scanning](/docs/whole-source-scanning/) and consult the [streaming guide](/docs/streaming-scanning/) and [resource limits](/docs/resource-limits-and-errors/).

## A chunked scan fails at EOF

`push` accepts byte fragments that may split UTF-8 scalars; `finish` checks genuine EOF. An incomplete trailing scalar or terminal execution error does not yield a partial report. Treat the failed streaming session as terminal and start a new session with corrected input. See [streaming scanning](/docs/streaming-scanning/).

## An incremental edit is rejected

Check the expected revision, half-open byte range and UTF-8 scalar boundaries. Invalid or stale edits are recoverable; retry against the current committed revision. Resource and internal-invariant failures are terminal for future edits, although the previous committed snapshot remains available. See [incremental scanning](/docs/incremental-scanning/) and [resource limits and errors](/docs/resource-limits-and-errors/).

## A limit fails despite a small final report

Budgets count intermediate work, not only final visible findings. Streaming can count rejected raw candidate emissions; incremental limits can account for coexisting committed and staged state. These limits are not process-wide heap caps. Inspect [resource limits and errors](/docs/resource-limits-and-errors/) before increasing a budget.

## A finding is absent but a review candidate appears

`SensitiveCandidate` represents review-only evidence, not a classified `Finding`. Review candidates are not automatically promoted or transformed. Check [findings and reports](/docs/findings-reports/) and [ownership and transformations](/docs/ownership-and-transformations/) before applying application policy.

## An adapter example does not match an installed package

Source implementation, local qualification, CI qualification and package publication are separate states. In particular, current WASM source APIs can differ from the published WASM package, and generated UniFFI artifacts are not evidence of remote package publication. Use the matching binding, native library and build profile, then consult [compatibility and distribution](/docs/compatibility-and-distribution/) and the relevant [integration guide](/docs/).

## A scan succeeds but the output is not safe to share

Detection is not exhaustive and transformed output may retain indirect identifiers or unclassified material. The host application controls logging, storage, retention and sharing. Review the [security model](/docs/security-model/) and [explicit transformations](/docs/transformations/).
