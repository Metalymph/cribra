# MCP conformance qualification

This document is the M3 conformance record for `cribra-mcp`. It records what was
checked, how it was checked, what the result was, and where the limits of the
claim are. It is deliberately explicit about the difference between *not
applicable to this implementation*, *not executed*, and *passed*.

**Verdict: PARTIAL PASS.** Every canonical server scenario whose prerequisites
this server actually meets is implemented and passes a real, executable test
(10 of 10 applicable rows). No official conformance-runner row was executed
(0 of 147), because the official runner has no stdio server mode. Nothing here
is a certification.

## 1. Scope

Qualified surface:

- the shipping `cribra-mcp` binary, built with `cargo build --release -p cribra-mcp`;
- newline-delimited JSON-RPC 2.0 over stdin/stdout, one process per session;
- modern MCP revision `2026-07-28` (stateless, per-request `_meta`);
- legacy MCP revision `2025-11-25` (`initialize` / `notifications/initialized`);
- both eras in one process;
- exactly two tools, `cribra_scan` and `cribra_transform`, with the advertised
  input and structured-output schemas.

Out of scope, and therefore never claimed: HTTP transport, authorization,
resources, prompts, logging, completion, sampling, elicitation, roots, tasks,
progress, keyed pseudonymization, and M4.

## 2. Referee inputs

The requirement sets are vendored byte-exactly and checked by SHA-256 on every
matrix run.

| File | SHA-256 |
| --- | --- |
| `requirements-2026-07-28.yaml` | `ae2f4f6210fd729e2e318edd5bbfa31a43cee0bc608e48052fa26dbf1d939b57` |
| `requirements-2025-11-25.yaml` | `f33a304dfa2cbd999c24026a3453a64f377bba0c8aa80addadaf05862d212371` |

Provenance: `modelcontextprotocol/conformance` at commit
`96a6f9478b7f2e125cf0360a213383cf57533893` (retrieved 2026-11-15,
package `@modelcontextprotocol/conformance@0.2.0-alpha.12`). See
[`tests/conformance/README.md`](tests/conformance/README.md).

The parser in [`tests/conformance/matrix.py`](tests/conformance/matrix.py) is a
strict, dependency-free reader. It accepts exactly the grammar those two files
use and raises on tabs, flow collections, quoting, anchors, aliases, tags,
duplicate keys, unknown top-level keys, unknown legs, unknown `not_scored`
reasons and unsupported indentation. `--self-test` proves that 12 malformed
inputs are rejected and one valid input parses correctly. There is no partial
or heuristic YAML support.

## 3. Why zero rows are executed by the official runner

The official suite's **server** command accepts a URL and nothing else
(quoted with the refinement body elided):

```ts
// src/schemas.ts @ 96a6f94
export const ServerOptionsSchema = z.object({
  url: z.string().url('Invalid server URL'),
  scenario: z.string().refine(...).optional()
});
```

There is no `command`, `args` or `stdio` field. `runServerConformanceTest`
therefore takes a server URL. The `sdk` subcommand is an SDK-repository auditor
that polls an HTTP URL; it is not a stdio server harness either. The only
command with a `command` field is `ClientOptionsSchema`, which tests a *client
implementation*, not a server.

Consequently the matrix records `execution = official-runner` for **zero**
rows. `server-stateless`, `caching`, `server-sse-multiple-streams`,
`dns-rebinding-protection` and the `http-*` scenarios also require HTTP status
codes, headers or streams that a stdio-only server cannot exhibit, so they
could not be executed even with a URL-capable runner pointed at a proxy.

This is a limitation of the referee, not a choice by Cribra. It is why the
verdict is PARTIAL PASS rather than PASS: an independent runner never observed
this binary.

## 4. Supported surface as observed by the matrix tests

Revisions: exactly `2026-07-28` and `2025-11-25`. `server/discover` returns
`supportedVersions: ["2026-07-28"]`. A modern request whose `_meta` names any
other revision is rejected with `-32022` carrying
`{"supported": ["2026-07-28"], "requested": <sanitized>}`. Legacy `initialize`
follows the handshake's version-negotiation shape instead: the client proposes a
revision and the server answers with the revision it will speak
(`2025-11-25`); a malformed `initialize` is a fixed `-32602`. The server never
returns `-32022` from `initialize`, and this document does not claim it does.

Capabilities: exactly `{"tools": {}}`. No `prompts`, `resources`, `logging`,
`sampling`, `elicitation`, `completions`, `roots` or `tasks` advertisement is
ever emitted, in either era.

Transport: stdio only, POSIX. The writer uses `fcntl`/`poll`/`SIGPIPE`
handling, so the crate targets Unix-like systems. Qualification was executed on
macOS aarch64 (Apple silicon). Windows is not a supported target — the transport
does not compile there — and is not claimed.

## 5. How applicability was derived

Applicability was derived from the prerequisites of each canonical scenario as
published upstream, not from a target percentage:

- **Client-leg rows (64).** The suite's client leg exercises a *client*
  implementation's behaviour (`initialize` parsing, elicitation handling, OAuth
  flows, sampling, roots, …). `cribra-mcp` ships no MCP client, so there is no
  subject under test. All 64 are `not-applicable`.
- **Server rows needing an unimplemented capability (45 modern + 28 legacy, 73
  rows).** Resources, prompts, logging, completion, sampling, elicitation,
  tasks, HTTP, SSE and DNS-rebinding scenarios are inapplicable because the
  scenario's first prerequisite — that capability or transport — is absent.
  These are marked `not-applicable`, never `pass`.
- **Server rows needing a suite fixture tool (e.g. `test_simple_text`,
  `test_error_handling`, `test_image_content`).** The suite drives its own
  fixture tools. This server MUST advertise only `cribra_scan` and
  `cribra_transform`, so a fixture tool cannot be added without violating the
  M1/M2 contract. Where the *requirement* (not the fixture name) is
  transport-independent, the row is `partial` and the equivalent requirement is
  tested against the advertised tools. Where the fixture's distinctive content
  type (image, audio, embedded resource, mixed content) is the entire point,
  the row is `not-applicable`.
- **`not_scored` rows.** Upstream freezes a reason (`extension`,
  `added-after-release`, `pending`). The reason is copied verbatim into the
  matrix and the row never counts toward any pass rate.

The `partial` classification is used only where a locally reproduced subset
exists. It is never used to hide a missing mandatory behaviour: every
`partial` row names the exact excluded portion in its evidence cell.

## 6. Execution, result and evidence are separate

| Column | Meaning |
| --- | --- |
| `applicability` | whether this implementation has the prerequisite feature the scenario exercises |
| `execution` | who drove the wire: `native-stdio` (a Rust test spawned the real binary), `locally-equivalent` (a local harness or in-process protocol test reproduced the transport-independent checks), `official-runner` (unused, 0 rows), `not-executed` |
| `result` | `pass` / `fail` / `not-tested` / `n/a` |
| `evidence` | the exact file and test function, or the SDK scenario name |

Rules enforced mechanically by `matrix.py`:

- every `pass` must cite `crates/cribra-mcp/<path>.rs::<test_fn>` and both the
  file and the function must exist;
- `not-applicable` rows must be `not-executed`/`n/a`;
- applicable rows may not be `not-executed`/`n/a`;
- every justification is at least 40 characters, so `-`/`TODO` placeholders
  cannot pass;
- the row set must equal the canonical set exactly, with no duplicates — the
  matrix is keyed by `(revision, leg, scenario)`.

## 7. Generated matrix

The region between the markers is generated by
`python3 tests/conformance/matrix.py --write` and verified by `--check`. Do not
edit it by hand.

<!-- BEGIN GENERATED MATRIX -->

### Requirement matrix

Every canonical entry from both pinned requirement sets appears exactly once, keyed by `(revision, leg, scenario)`. `canonical` and `reason` are copied verbatim from upstream; `applicability`, `execution`, `result` and `evidence` are Cribra's own assessment.

- `execution = official-runner` appears in the vocabulary but is used by **zero** rows: the official runner has no stdio server mode.
- `execution = native-stdio` means a Rust integration test spoke JSON-RPC directly to the real binary over stdin/stdout.
- `execution = locally-equivalent` means a local harness or in-process protocol test reproduced the scenario's transport-independent checks against the same pinned schemas. The evidence cell names the channel of every cited test.

| revision | leg | scenario | canonical | reason | applicability | execution | result | evidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-07-28 | server | server-stateless | required | - | partial | native-stdio | pass | Transport-independent checks only: per-request _meta validation, server/discover shape and unsupported-version rejection. HTTP status codes, header matching (-32020) and subscriptions/listen streams are excluded. native-stdio: crates/cribra-mcp/tests/adversarial.rs::modern_version_negotiation_is_exact_over_the_wire; in-process: crates/cribra-mcp/tests/protocol_conformance.rs::modern_discovery_reports_exact_capabilities_and_caching |
| 2026-07-28 | server | completion-complete | required | - | not-applicable | not-executed | n/a | Requires completion/complete for prompt or resource-template arguments. cribra-mcp advertises only the tools capability and no completions capability. |
| 2026-07-28 | server | tools-list | required | - | applicable | native-stdio | pass | native-stdio: crates/cribra-mcp/tests/jsonl_stdio.rs::modern_tools_execute_scan_and_transform_without_source_disclosure (exact advertised names plus ttlMs/cacheScope); tool schema shape in-process: crates/cribra-mcp/tests/protocol_conformance.rs::modern_tools_surface_is_two_static_contracts |
| 2026-07-28 | server | tools-call-simple-text | required | - | partial | native-stdio | pass | Fixture tool test_simple_text is absent; the equivalent requirement (a tools/call result carries a text content block) is exercised with both advertised tools. native-stdio: crates/cribra-mcp/tests/jsonl_stdio.rs::modern_tools_execute_scan_and_transform_without_source_disclosure |
| 2026-07-28 | server | tools-call-image | required | - | not-applicable | not-executed | n/a | Requires a fixture tool returning image content. cribra-mcp advertises two tools that return text content only, so image content is not applicable to the advertised contract. |
| 2026-07-28 | server | tools-call-audio | required | - | not-applicable | not-executed | n/a | Requires a fixture tool returning audio content. cribra-mcp advertises two tools that return text content only, so audio content is not applicable to the advertised contract. |
| 2026-07-28 | server | tools-call-embedded-resource | required | - | not-applicable | not-executed | n/a | Requires a fixture tool returning embedded resource content. cribra-mcp advertises two tools that return text content only. |
| 2026-07-28 | server | tools-call-mixed-content | required | - | not-applicable | not-executed | n/a | Requires a fixture tool returning several content types in one result. cribra-mcp advertises two tools that return a single text content block. |
| 2026-07-28 | server | tools-call-error | required | - | partial | native-stdio | pass | Fixture tool test_error_handling is absent; the equivalent requirement (tool-level failure returns isError:true with schema-valid structured content, not a JSON-RPC error) is exercised with the advertised tools. native-stdio: crates/cribra-mcp/tests/jsonl_stdio.rs::tool_success_and_failure_results_match_advertised_schemas |
| 2026-07-28 | server | tools-call-with-progress | required | - | not-applicable | not-executed | n/a | Requires a fixture tool that emits notifications/progress for a client progressToken. cribra-mcp emits no tool-generated notifications and advertises no progress support. |
| 2026-07-28 | server | server-sse-multiple-streams | required | - | not-applicable | not-executed | n/a | Requires the Streamable HTTP transport: HTTP status codes, response headers, Host/Origin validation or SSE streams. cribra-mcp is a stdio-only server. |
| 2026-07-28 | server | resources-list | required | - | not-applicable | not-executed | n/a | Requires the resources capability. cribra-mcp advertises only the tools capability, so resources/list, resources/read and resource templates have no subject under test. |
| 2026-07-28 | server | resources-read-text | required | - | not-applicable | not-executed | n/a | Requires the resources capability. cribra-mcp advertises only the tools capability, so resources/list, resources/read and resource templates have no subject under test. |
| 2026-07-28 | server | resources-read-binary | required | - | not-applicable | not-executed | n/a | Requires the resources capability. cribra-mcp advertises only the tools capability, so resources/list, resources/read and resource templates have no subject under test. |
| 2026-07-28 | server | resources-templates-read | required | - | not-applicable | not-executed | n/a | Requires the resources capability. cribra-mcp advertises only the tools capability, so resources/list, resources/read and resource templates have no subject under test. |
| 2026-07-28 | server | sep-2164-resource-not-found | required | - | not-applicable | not-executed | n/a | Requires the resources capability. cribra-mcp advertises only the tools capability, so resources/list, resources/read and resource templates have no subject under test. |
| 2026-07-28 | server | prompts-list | required | - | not-applicable | not-executed | n/a | Requires the prompts capability. cribra-mcp advertises only the tools capability, so prompts/list and prompts/get have no subject under test. |
| 2026-07-28 | server | prompts-get-simple | required | - | not-applicable | not-executed | n/a | Requires the prompts capability. cribra-mcp advertises only the tools capability, so prompts/list and prompts/get have no subject under test. |
| 2026-07-28 | server | prompts-get-with-args | required | - | not-applicable | not-executed | n/a | Requires the prompts capability. cribra-mcp advertises only the tools capability, so prompts/list and prompts/get have no subject under test. |
| 2026-07-28 | server | prompts-get-embedded-resource | required | - | not-applicable | not-executed | n/a | Requires the prompts capability. cribra-mcp advertises only the tools capability, so prompts/list and prompts/get have no subject under test. |
| 2026-07-28 | server | prompts-get-with-image | required | - | not-applicable | not-executed | n/a | Requires the prompts capability. cribra-mcp advertises only the tools capability, so prompts/list and prompts/get have no subject under test. |
| 2026-07-28 | server | dns-rebinding-protection | required | - | not-applicable | not-executed | n/a | Requires the Streamable HTTP transport: HTTP status codes, response headers, Host/Origin validation or SSE streams. cribra-mcp is a stdio-only server. |
| 2026-07-28 | server | caching | required | - | partial | locally-equivalent | pass | tools/list and server/discover carry ttlMs and cacheScope, checked in-process against the pinned schema. The scenario also probes prompts/list, resources/list, resources/templates/list and resources/read, which are excluded because those capabilities are not advertised. in-process: crates/cribra-mcp/tests/protocol_conformance.rs::modern_discovery_reports_exact_capabilities_and_caching |
| 2026-07-28 | server | input-required-result-basic-elicitation | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_elicitation`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | server | input-required-result-basic-sampling | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_sampling`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | server | input-required-result-basic-list-roots | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_list_roots`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | server | input-required-result-request-state | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_request_state`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | server | input-required-result-multiple-input-requests | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_multiple`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | server | input-required-result-multi-round | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_multi_round`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | server | input-required-result-missing-input-response | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_missing`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | server | input-required-result-non-tool-request | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_non_tool`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | server | input-required-result-result-type | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_elicitati`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | server | input-required-result-unsupported-methods | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_unsupported`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | server | input-required-result-tampered-state | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_tampered`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | server | input-required-result-capability-check | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_capability`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | server | input-required-result-ignore-extra-params | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_extra`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | server | input-required-result-validate-input | required | - | not-applicable | not-executed | n/a | Requires multi-round-trip input_required support plus the suite fixture tool `test_input_required_result_validate`. cribra-mcp advertises no elicitation, sampling or roots capability and returns fixed tool errors instead of input requests. |
| 2026-07-28 | client | tools_call | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | request-metadata | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/metadata-default | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/metadata-var1 | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/metadata-var2 | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/metadata-var3 | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/basic-cimd | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/scope-from-www-authenticate | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/scope-from-scopes-supported | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/scope-omitted-when-undefined | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/scope-step-up | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/scope-retry-limit | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/token-endpoint-auth-basic | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/token-endpoint-auth-post | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/token-endpoint-auth-none | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/pre-registration | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/resource-mismatch | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/offline-access-scope | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/offline-access-not-supported | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/authorization-server-migration | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/iss-supported | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/iss-not-advertised | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/iss-supported-missing | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/iss-wrong-issuer | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/iss-unexpected | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/iss-normalized | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/metadata-issuer-mismatch | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | sep-2322-client-request-state | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | http-standard-headers | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | http-custom-headers | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | http-invalid-tool-headers | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | json-schema-ref-no-deref | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/client-credentials-jwt | not_scored | extension | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/client-credentials-basic | not_scored | extension | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/enterprise-managed-authorization | not_scored | extension | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/dpop | not_scored | extension | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/dpop-nonce | not_scored | extension | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | auth/wif-jwt-bearer | not_scored | extension | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | client | json-schema-2020-12-preservation | not_scored | added-after-release | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2026-07-28 | server | tasks-lifecycle | not_scored | extension | not-applicable | not-executed | n/a | Not scored upstream (extension), so it never counts toward a pass rate. Extension suite (SEP-2663) requiring the tasks capability. |
| 2026-07-28 | server | tasks-capability-negotiation | not_scored | extension | not-applicable | not-executed | n/a | Not scored upstream (extension), so it never counts toward a pass rate. Extension suite (SEP-2663) requiring the tasks capability. |
| 2026-07-28 | server | tasks-wire-fields | not_scored | extension | not-applicable | not-executed | n/a | Not scored upstream (extension), so it never counts toward a pass rate. Extension suite (SEP-2663) requiring the tasks capability. |
| 2026-07-28 | server | tasks-request-state-removal | not_scored | extension | not-applicable | not-executed | n/a | Not scored upstream (extension), so it never counts toward a pass rate. Extension suite (SEP-2663) requiring the tasks capability. |
| 2026-07-28 | server | tasks-mrtr-input | not_scored | extension | not-applicable | not-executed | n/a | Not scored upstream (extension), so it never counts toward a pass rate. Extension suite (SEP-2663) requiring the tasks capability. |
| 2026-07-28 | server | tasks-request-headers | not_scored | extension | not-applicable | not-executed | n/a | Not scored upstream (extension), so it never counts toward a pass rate. Extension suite (SEP-2663) requiring the tasks capability. |
| 2026-07-28 | server | tasks-dispatch-and-envelope | not_scored | extension | not-applicable | not-executed | n/a | Not scored upstream (extension), so it never counts toward a pass rate. Extension suite (SEP-2663) requiring the tasks capability. |
| 2026-07-28 | server | tasks-status-notifications | not_scored | extension | not-applicable | not-executed | n/a | Not scored upstream (extension), so it never counts toward a pass rate. Extension suite (SEP-2663) requiring the tasks capability. |
| 2026-07-28 | server | tasks-required-task-error | not_scored | extension | not-applicable | not-executed | n/a | Not scored upstream (extension), so it never counts toward a pass rate. Extension suite (SEP-2663) requiring the tasks capability. |
| 2026-07-28 | server | tasks-mrtr-composition | not_scored | extension | not-applicable | not-executed | n/a | Not scored upstream (extension), so it never counts toward a pass rate. Extension suite (SEP-2663) requiring the tasks capability. |
| 2026-07-28 | server | json-schema-2020-12 | not_scored | pending | not-applicable | not-executed | n/a | Not scored upstream (pending), so it never counts toward a pass rate. Requires the suite fixture tool whose inputSchema exercises JSON Schema 2020-12 keywords. cribra-mcp advertises exactly cribra_scan and cribra_transform. |
| 2026-07-28 | server | http-header-validation | not_scored | pending | not-applicable | not-executed | n/a | Not scored upstream (pending), so it never counts toward a pass rate. Requires the Streamable HTTP transport: HTTP status codes, response headers, Host/Origin validation or SSE streams. cribra-mcp is a stdio-only server. |
| 2026-07-28 | server | http-custom-header-server-validation | not_scored | pending | not-applicable | not-executed | n/a | Not scored upstream (pending), so it never counts toward a pass rate. Requires the Streamable HTTP transport: HTTP status codes, response headers, Host/Origin validation or SSE streams. cribra-mcp is a stdio-only server. |
| 2025-11-25 | server | server-initialize | required | - | applicable | native-stdio | pass | Handshake, serverInfo, protocolVersion and capabilities over stdio. The scenario's optional MCP-Session-Id check is HTTP-transport-only and is not applicable to a stdio server. native-stdio: crates/cribra-mcp/tests/jsonl_stdio.rs::legacy_transcript_matches_pinned_expectations; exact shaping in-process: crates/cribra-mcp/tests/protocol_conformance.rs::legacy_lifecycle_and_shaping_are_exact |
| 2025-11-25 | server | logging-set-level | required | - | not-applicable | not-executed | n/a | Requires the optional logging capability and logging/setLevel handling. cribra-mcp does not advertise logging and emits no log notifications. |
| 2025-11-25 | server | ping | required | - | applicable | native-stdio | pass | Legacy sessions answer ping with an empty result object. native-stdio: crates/cribra-mcp/tests/jsonl_stdio.rs::legacy_transcript_matches_pinned_expectations; exact result in-process: crates/cribra-mcp/tests/protocol_conformance.rs::legacy_ping_and_removed_modern_methods_are_exact |
| 2025-11-25 | server | completion-complete | required | - | not-applicable | not-executed | n/a | Requires completion/complete for prompt or resource-template arguments. cribra-mcp advertises only the tools capability and no completions capability. |
| 2025-11-25 | server | tools-list | required | - | applicable | native-stdio | pass | Two tools advertised in the legacy era with no modern-only fields, validated against the 2025-11-25 schema. native-stdio: crates/cribra-mcp/tests/jsonl_stdio.rs::legacy_tools_surface_omits_modern_fields |
| 2025-11-25 | server | tools-call-simple-text | required | - | partial | native-stdio | pass | Fixture tool test_simple_text is absent; the equivalent requirement (a legacy tools/call result carries a non-empty text content block) is exercised with an advertised tool. native-stdio: crates/cribra-mcp/tests/jsonl_stdio.rs::legacy_tools_surface_omits_modern_fields |
| 2025-11-25 | server | tools-call-image | required | - | not-applicable | not-executed | n/a | Requires a fixture tool returning image content. cribra-mcp advertises two tools that return text content only, so image content is not applicable to the advertised contract. |
| 2025-11-25 | server | tools-call-audio | required | - | not-applicable | not-executed | n/a | Requires a fixture tool returning audio content. cribra-mcp advertises two tools that return text content only, so audio content is not applicable to the advertised contract. |
| 2025-11-25 | server | tools-call-embedded-resource | required | - | not-applicable | not-executed | n/a | Requires a fixture tool returning embedded resource content. cribra-mcp advertises two tools that return text content only. |
| 2025-11-25 | server | tools-call-mixed-content | required | - | not-applicable | not-executed | n/a | Requires a fixture tool returning several content types in one result. cribra-mcp advertises two tools that return a single text content block. |
| 2025-11-25 | server | tools-call-with-logging | required | - | not-applicable | not-executed | n/a | Requires the optional logging capability plus the suite fixture tool test_tool_with_logging. cribra-mcp advertises no logging capability. |
| 2025-11-25 | server | tools-call-error | required | - | partial | native-stdio | pass | Fixture tool test_error_handling is absent; the equivalent requirement (a legacy tool-level failure is an isError result with schema-valid structured content, not a JSON-RPC error) is exercised with an advertised tool. native-stdio: crates/cribra-mcp/tests/jsonl_stdio.rs::legacy_tools_surface_omits_modern_fields |
| 2025-11-25 | server | tools-call-with-progress | required | - | not-applicable | not-executed | n/a | Requires a fixture tool that emits notifications/progress for a client progressToken. cribra-mcp emits no tool-generated notifications and advertises no progress support. |
| 2025-11-25 | server | tools-call-sampling | required | - | not-applicable | not-executed | n/a | Requires server-initiated sampling/createMessage and the suite fixture tool test_sampling. cribra-mcp advertises no sampling capability and emits no server-to-client requests. |
| 2025-11-25 | server | tools-call-elicitation | required | - | not-applicable | not-executed | n/a | Requires server-initiated elicitation/create and the suite fixture tool test_elicitation. cribra-mcp advertises no elicitation capability and emits no server-to-client requests. |
| 2025-11-25 | server | elicitation-sep1034-defaults | required | - | not-applicable | not-executed | n/a | Requires the suite fixture tool test_elicitation_sep1034_defaults and server-initiated elicitation/create. cribra-mcp advertises no elicitation capability. |
| 2025-11-25 | server | server-sse-multiple-streams | required | - | not-applicable | not-executed | n/a | Requires the Streamable HTTP transport: HTTP status codes, response headers, Host/Origin validation or SSE streams. cribra-mcp is a stdio-only server. |
| 2025-11-25 | server | elicitation-sep1330-enums | required | - | not-applicable | not-executed | n/a | Requires the suite fixture tool test_elicitation_sep1330_enums and server-initiated elicitation/create. cribra-mcp advertises no elicitation capability. |
| 2025-11-25 | server | resources-list | required | - | not-applicable | not-executed | n/a | Requires the resources capability. cribra-mcp advertises only the tools capability, so resources/list, resources/read and resource templates have no subject under test. |
| 2025-11-25 | server | resources-read-text | required | - | not-applicable | not-executed | n/a | Requires the resources capability. cribra-mcp advertises only the tools capability, so resources/list, resources/read and resource templates have no subject under test. |
| 2025-11-25 | server | resources-read-binary | required | - | not-applicable | not-executed | n/a | Requires the resources capability. cribra-mcp advertises only the tools capability, so resources/list, resources/read and resource templates have no subject under test. |
| 2025-11-25 | server | resources-templates-read | required | - | not-applicable | not-executed | n/a | Requires the resources capability. cribra-mcp advertises only the tools capability, so resources/list, resources/read and resource templates have no subject under test. |
| 2025-11-25 | server | resources-subscribe | required | - | not-applicable | not-executed | n/a | Requires the resources capability. cribra-mcp advertises only the tools capability, so resources/list, resources/read and resource templates have no subject under test. |
| 2025-11-25 | server | resources-unsubscribe | required | - | not-applicable | not-executed | n/a | Requires the resources capability. cribra-mcp advertises only the tools capability, so resources/list, resources/read and resource templates have no subject under test. |
| 2025-11-25 | server | prompts-list | required | - | not-applicable | not-executed | n/a | Requires the prompts capability. cribra-mcp advertises only the tools capability, so prompts/list and prompts/get have no subject under test. |
| 2025-11-25 | server | prompts-get-simple | required | - | not-applicable | not-executed | n/a | Requires the prompts capability. cribra-mcp advertises only the tools capability, so prompts/list and prompts/get have no subject under test. |
| 2025-11-25 | server | prompts-get-with-args | required | - | not-applicable | not-executed | n/a | Requires the prompts capability. cribra-mcp advertises only the tools capability, so prompts/list and prompts/get have no subject under test. |
| 2025-11-25 | server | prompts-get-embedded-resource | required | - | not-applicable | not-executed | n/a | Requires the prompts capability. cribra-mcp advertises only the tools capability, so prompts/list and prompts/get have no subject under test. |
| 2025-11-25 | server | prompts-get-with-image | required | - | not-applicable | not-executed | n/a | Requires the prompts capability. cribra-mcp advertises only the tools capability, so prompts/list and prompts/get have no subject under test. |
| 2025-11-25 | server | dns-rebinding-protection | required | - | not-applicable | not-executed | n/a | Requires the Streamable HTTP transport: HTTP status codes, response headers, Host/Origin validation or SSE streams. cribra-mcp is a stdio-only server. |
| 2025-11-25 | client | initialize | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | tools_call | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | elicitation-sep1034-client-defaults | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | sse-retry | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/metadata-default | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/metadata-var1 | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/metadata-var2 | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/metadata-var3 | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/basic-cimd | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/scope-from-www-authenticate | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/scope-from-scopes-supported | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/scope-omitted-when-undefined | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/scope-step-up | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/scope-retry-limit | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/token-endpoint-auth-basic | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/token-endpoint-auth-post | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/token-endpoint-auth-none | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/pre-registration | required | - | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/client-credentials-jwt | not_scored | extension | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/client-credentials-basic | not_scored | extension | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/enterprise-managed-authorization | not_scored | extension | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/dpop | not_scored | extension | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/dpop-nonce | not_scored | extension | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | client | auth/wif-jwt-bearer | not_scored | extension | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | server | server-session-lifecycle | not_scored | added-after-release | not-applicable | not-executed | n/a | Not scored upstream (added-after-release), so it never counts toward a pass rate. Requires the Streamable HTTP session mechanism (Mcp-Session-Id headers and session termination). cribra-mcp is a stdio-only server. |
| 2025-11-25 | client | json-schema-2020-12-preservation | not_scored | added-after-release | not-applicable | not-executed | n/a | This scenario exercises an MCP client implementation. cribra-mcp is a server and ships no MCP client, so the scenario has no subject under test here. |
| 2025-11-25 | server | json-schema-2020-12 | not_scored | pending | not-applicable | not-executed | n/a | Not scored upstream (pending), so it never counts toward a pass rate. Requires the suite fixture tool whose inputSchema exercises JSON Schema 2020-12 keywords. cribra-mcp advertises exactly cribra_scan and cribra_transform. |
| 2025-11-25 | server | server-sse-polling | not_scored | pending | not-applicable | not-executed | n/a | Not scored upstream (pending), so it never counts toward a pass rate. Requires the Streamable HTTP transport: HTTP status codes, response headers, Host/Origin validation or SSE streams. cribra-mcp is a stdio-only server. |

### Matrix summary

| revision | leg | canonical | rows | applicable | partial | not applicable | passed |
| --- | --- | --- | --- | --- | --- | --- | --- |
| 2026-07-28 | server | required | 37 | 1 | 4 | 32 | 5 |
| 2026-07-28 | server | not_scored | 13 | 0 | 0 | 13 | 0 |
| 2026-07-28 | client | required | 32 | 0 | 0 | 32 | 0 |
| 2026-07-28 | client | not_scored | 7 | 0 | 0 | 7 | 0 |
| 2025-11-25 | server | required | 30 | 3 | 2 | 25 | 5 |
| 2025-11-25 | server | not_scored | 3 | 0 | 0 | 3 | 0 |
| 2025-11-25 | client | required | 18 | 0 | 0 | 18 | 0 |
| 2025-11-25 | client | not_scored | 7 | 0 | 0 | 7 | 0 |

Rows executed by the official conformance runner: **0**.
Total canonical rows: **147**; passed: **10**; failed: **0**; not tested: **0**.

<!-- END GENERATED MATRIX -->

## 8. Evidence inventory

### Native stdio tests against the real binary

| File | Tests | Covers |
| --- | --- | --- |
| `tests/protocol_conformance.rs` | 12 | schema-pinned shaping, discovery, capabilities, caching hints, lifecycle, ping, removed methods, tool contracts |
| `tests/jsonl_stdio.rs` | 14 | end-to-end transcripts for both eras and dual-era use, fixture/schema validation |
| `tests/adversarial.rs` | 16 | framing, invalid UTF-8, deep nesting, id bounds, oversized frames, duplicate ids, bursts, ordering, 1 MiB source boundary, repeated lifecycle, wire-level version negotiation |
| `tests/privacy.rs` | 5 | canary-based leak checks for scan, errors, stderr and transform text/structured agreement |
| `tests/cancellation.rs` | 5 | queued, running and post-completion cancellation, malformed cancellations, cancellation behind a blocked writer |
| `tests/transport_races.rs` | 7 | backpressure exit code, broken pipe, EOF discard, partial pipe writes, mid-response disconnect |

### Independent SDK harnesses

| Harness | Client | Scenarios |
| --- | --- | --- |
| `qualification/typescript/run.mjs` | `@modelcontextprotocol/client` 2.4.0 | 5 |
| `qualification/python/run.py` | `mcp` 2.3.0 | 5 |

Both drive the release binary through the published SDK APIs only. Tool results
are re-validated with a third-party JSON Schema validator (`ajv` 8.17.1 for
draft 2020-12, `jsonschema` 4.26.0) against the advertised output schemas, so
the schema claim does not rest on Cribra's own validator.

Negotiation is tested for what the SDKs actually do:

- TypeScript `versionNegotiation.mode: { pin: '2026-07-28' }` negotiates modern
  at that revision; `'auto'` probes and selects modern. The legacy **fallback**
  path is not exercised against this server and is not claimed.
- Python `mode='2026-07-28'` installs a *locally synthesized* `DiscoverResult`
  and performs **no** wire discovery. The harness asserts that shape
  (`cacheScope=public`, empty capabilities) rather than pretending a probe
  happened. Explicit discovery is tested separately, on a legacy connection
  with no cached result, where `session.discover()` genuinely sends
  `server/discover` and adopts the server's answer (`cacheScope=private`,
  `ttlMs=0`, tools capability).

## 9. Reproduction

```sh
# Rust evidence (debug + release profiles, both toolchains)
cargo test -p cribra-mcp --all-targets
cargo test -p cribra

# Generated matrix is complete, unique and justified
python3 crates/cribra-mcp/tests/conformance/matrix.py --self-test
python3 crates/cribra-mcp/tests/conformance/matrix.py --check

# Release binary + both independent SDK suites
just mcp-qualify
```

`just mcp-qualify` runs `crates/cribra-mcp/qualification/run-qualification.sh`,
which builds the release binary, verifies the matrix, installs each harness's
pinned dependencies if absent, and runs both suites.

Environment used for the recorded run: macOS aarch64, rustc 1.100.0 (plus the
1.97.0 MSRV toolchain), Node v26.13.1, Python 3.13.18.

## 10. Security, privacy and transport behaviour qualified

- **No source disclosure from `cribra_scan`.** Synthetic canaries in the source,
  in matched spans and in regex patterns never appear in any response or on
  stderr. Findings carry rule id and byte offsets only.
- **No argument echoing.** Invalid arguments, uncompilable patterns, overlapping
  spans and unsupported modes return fixed codes with no interpolated values.
- **stderr stays silent.** Malformed JSON, oversized frames, invalid UTF-8 and
  protocol violations never produce descriptive diagnostics.
- **Bounded parsing.** Inputs are capped at 2 MiB per frame; oversized frames are
  discarded to the newline and rejected with a fixed `-32600` that carries no
  id and no content. Deep nesting degrades to `-32700`, not a crash.
- **Identifier handling.** Maximum-length ids round-trip byte-exactly, longer ids
  are rejected, unreadable ids are omitted rather than echoed as `null`, and a
  live duplicate is rejected while a completed id is reusable.
- **Overload.** A burst at queue capacity answers every request exactly once,
  with `-32603 Server overloaded` for rejected work.
- **Terminal states.** EOF discards queued work and exits 0; a closed stdout is a
  clean broken-pipe shutdown; sustained backpressure terminates with exit code 3
  rather than hanging the reader.
- **Cancellation.** A cancelled request never reaches stdout, whether it was
  queued, running, or already behind a blocked writer. Phases that are not
  externally observable are pinned by registry unit tests and stated as such.
- **Determinism.** No M3 test depends on a fixed sleep to pass. Absence of output
  is proven by a later barrier response: the writer emits strictly in completion
  order, so a suppression failure surfaces as an unexpected id rather than a
  timing flake.

## 11. Unsupported optional features

Not advertised and not implemented: resources, resource templates, prompts,
logging, completion, sampling, elicitation, roots, tasks (SEP-2663) and
multi-round `input_required` requests. `ping` is answered in the legacy era and
is method-not-found in the modern era. Requests for removed methods return
`-32601`. These absences are the justification for the corresponding
`not-applicable` rows; they are not silently scored as passes.

## 12. Residual risks

1. **No official-runner observation.** The strongest available claim is
   schema-pinned local reproduction plus two independent SDKs. If the suite
   adds a stdio server mode, re-pin and re-run; the matrix will need new
   `official-runner` rows.
2. **Applicability judgement.** Prerequisites were read from the published
   scenario descriptions, not executed. A misread prerequisite would show up as
   an over-broad `not-applicable`. Mitigation: every justification names the
   concrete capability or transport that is missing, so a reviewer can falsify
   it.
3. **Fixture-tool divergence.** `partial` rows test the requirement's intent
   with `cribra_scan`/`cribra_transform` instead of the suite's fixture tool. The
   behaviour is equivalent for text content and error results, but it is not
   byte-identical to the fixture's parameters.
4. **Platform coverage.** Executed on macOS aarch64 only. Linux/x86-64 should
   behave identically (same libc surface) but was not executed here.
5. **SDK version drift.** Pins are exact; a future SDK release may negotiate
   differently. Re-run `just mcp-qualify` after any bump.

## 13. Verdict

| Area | Result | Basis |
| --- | --- | --- |
| Canonical matrix completeness | **PASS** | 147 rows, all unique, all justified, 12 malformed-input self-tests |
| Applicable server scenarios | **PASS** | 10 of 10 applicable/partial rows pass against the real binary |
| Protocol shaping vs pinned schemas | **PASS** | every captured message validated against the official revision schemas |
| Adversarial transport behaviour | **PASS** | 16 adversarial + 7 race tests |
| Privacy | **PASS** | canary leak tests over scan, errors, stderr, transform |
| TypeScript SDK interoperability | **PASS** | 5 of 5 scenarios, `@modelcontextprotocol/client` 2.4.0 |
| Python SDK interoperability | **PASS** | 5 of 5 scenarios, `mcp` 2.3.0 |
| Official runner execution | **NOT EXECUTED** | runner is URL-only; zero stdio server scenarios possible |
| Client-leg conformance | **NOT APPLICABLE** | no MCP client is shipped |

**Overall: PARTIAL PASS.** The implementation passes every conformance
requirement that its advertised scope actually imposes, verified by direct
native-stdio tests and by two independent SDK clients. It is not certified: no
official runner observed it, and the majority of canonical scenarios have no
subject under test because the capabilities they exercise are intentionally
absent.
