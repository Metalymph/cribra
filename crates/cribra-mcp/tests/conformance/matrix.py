#!/usr/bin/env python3
"""Verify or regenerate the MCP conformance requirement matrix.

The matrix is keyed by (revision, leg, scenario). The canonical classification
(`required` / `not_scored`) and the original upstream `not_scored` reason are
copied verbatim from the pinned requirement sets and may not be edited here.

The script has three responsibilities:

1. Parse the two pinned requirement files with a deliberately strict reader.
   The reader understands exactly the grammar those files use and raises on
   every other structure: tabs, quoting, flow collections, anchors, aliases,
   tags, unknown keys, duplicate keys, unknown reasons, wrong indentation and
   unknown top-level sections. It never guesses.
2. Merge the canonical entries with the applicability policy below.
3. Either rewrite the generated region of CONFORMANCE.md or verify that the
   committed region matches exactly.

Usage:
    python3 matrix.py --check
    python3 matrix.py --write
    python3 matrix.py --self-test
    python3 matrix.py --print-canonical
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
from dataclasses import dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
CRATE = HERE.parents[1]
DOC = CRATE / "CONFORMANCE.md"

REVISIONS = {
    "2026-07-28": "requirements-2026-07-28.yaml",
    "2025-11-25": "requirements-2025-11-25.yaml",
}

# Byte-exact pins. A changed file must be re-pinned deliberately.
PINNED_SHA256 = {
    "2026-07-28": "ae2f4f6210fd729e2e318edd5bbfa31a43cee0bc608e48052fa26dbf1d939b57",
    "2025-11-25": "f33a304dfa2cbd999c24026a3453a64f377bba0c8aa80addadaf05862d212371",
}

# Expected entry counts per section. Upstream changes must fail loudly rather
# than silently alter the matrix.
EXPECTED_COUNTS = {
    ("2026-07-28", "server", "required"): 37,
    ("2026-07-28", "client", "required"): 32,
    ("2026-07-28", "not_scored"): 20,
    ("2025-11-25", "server", "required"): 30,
    ("2025-11-25", "client", "required"): 18,
    ("2025-11-25", "not_scored"): 10,
}

REASONS = {"extension", "added-after-release", "pending"}
LEGS = {"server", "client"}

SCENARIO_RE = re.compile(r"^[a-z0-9][a-z0-9/_-]*$")
SECTION_KEYS = {"server", "client", "not_scored"}

BEGIN = "<!-- BEGIN GENERATED MATRIX -->"
END = "<!-- END GENERATED MATRIX -->"

APPLICABILITY = {"applicable", "partial", "not-applicable"}
EXECUTION = {"native-stdio", "locally-equivalent", "official-runner", "not-executed"}
RESULT = {"pass", "fail", "not-tested", "n/a"}

MIN_JUSTIFICATION = 40


class RequirementsError(Exception):
    """The pinned requirement file is not exactly the supported grammar."""


@dataclass(frozen=True)
class Canonical:
    revision: str
    leg: str
    scenario: str
    status: str
    reason: str | None


@dataclass(frozen=True)
class Verdict:
    applicability: str
    execution: str
    result: str
    evidence: str


# --------------------------------------------------------------------------
# Strict reader
# --------------------------------------------------------------------------


def _reject(token: str, needle: str, lineno: int, filename: str) -> None:
    if needle in token:
        raise RequirementsError(
            f"{filename}:{lineno}: unsupported YAML structure {needle!r} in {token!r}"
        )


def parse_requirements(revision: str, text: str, filename: str = "<text>") -> list[Canonical]:
    """Parses one requirement file, rejecting every unsupported structure."""

    entries: list[Canonical] = []
    seen_sections: set[str] = set()
    pending: dict[str, str] | None = None
    folded_key: str | None = None
    section: str | None = None
    lines = text.split("\n")

    def flush_pending(lineno: int) -> None:
        nonlocal pending, folded_key
        if pending is None:
            return
        if set(pending) != {"scenario", "leg", "reason"}:
            raise RequirementsError(
                f"{filename}:{lineno}: not_scored entry missing keys; found {sorted(pending)}"
            )
        if pending["leg"] not in LEGS:
            raise RequirementsError(
                f"{filename}:{lineno}: unsupported not_scored leg {pending['leg']!r}"
            )
        if pending["reason"] not in REASONS:
            raise RequirementsError(
                f"{filename}:{lineno}: unsupported not_scored reason {pending['reason']!r}"
            )
        entries.append(
            Canonical(
                revision=revision,
                leg=pending["leg"],
                scenario=pending["scenario"],
                status="not_scored",
                reason=pending["reason"],
            )
        )
        pending = None
        folded_key = None

    for index, raw in enumerate(lines, start=1):
        if "\t" in raw:
            raise RequirementsError(f"{filename}:{index}: tab characters are not supported")

        line = raw.rstrip()
        if not line:
            continue

        if line.startswith("#"):
            continue

        # A folded note continuation is only legal at six spaces of
        # indentation. Any other line terminates the note and is processed
        # normally, so a following list item or section is still validated.
        if folded_key is not None:
            if raw.startswith("      "):
                continue
            folded_key = None

        if raw.endswith(":"):
            key = line[:-1]
            if key not in SECTION_KEYS:
                raise RequirementsError(f"{filename}:{index}: unknown top-level key {key!r}")
            if key in seen_sections:
                raise RequirementsError(f"{filename}:{index}: duplicate top-level key {key!r}")
            seen_sections.add(key)
            flush_pending(index)
            section = key
            continue

        if raw.startswith("  - "):
            value = line[4:]
            if value == "":
                raise RequirementsError(f"{filename}:{index}: empty list item")
            _reject(value, "[", index, filename)
            _reject(value, "{", index, filename)
            _reject(value, "&", index, filename)
            _reject(value, "*", index, filename)
            _reject(value, "!", index, filename)
            _reject(value, '"', index, filename)
            _reject(value, "'", index, filename)
            _reject(value, "|", index, filename)
            _reject(value, ">", index, filename)

            if section in ("server", "client"):
                if not SCENARIO_RE.match(value):
                    raise RequirementsError(
                        f"{filename}:{index}: malformed scenario name {value!r}"
                    )
                entries.append(
                    Canonical(revision, section, value, "required", None)  # type: ignore[arg-type]
                )
            elif section == "not_scored":
                flush_pending(index)
                if not value.startswith("scenario: "):
                    raise RequirementsError(
                        f"{filename}:{index}: not_scored item must begin with 'scenario: '"
                    )
                pending = {"scenario": value[len("scenario: ") :]}
                if not SCENARIO_RE.match(pending["scenario"]):
                    raise RequirementsError(
                        f"{filename}:{index}: malformed scenario name {pending['scenario']!r}"
                    )
            else:
                raise RequirementsError(f"{filename}:{index}: list item outside a section")
            continue

        if raw.startswith("    ") and not raw.startswith("      "):
            if section != "not_scored" or pending is None:
                raise RequirementsError(f"{filename}:{index}: unexpected mapping member {line!r}")
            key, sep, value = line.strip().partition(": ")
            if not sep:
                raise RequirementsError(f"{filename}:{index}: mapping member without a value")
            if key in pending:
                raise RequirementsError(f"{filename}:{index}: duplicate key {key!r}")
            if key not in {"leg", "reason", "note"}:
                raise RequirementsError(f"{filename}:{index}: unsupported key {key!r}")
            if key == "note":
                if value != ">-":
                    raise RequirementsError(
                        f"{filename}:{index}: only the folded scalar '>-' is supported"
                    )
                folded_key = "note"
                continue
            _reject(value, '"', index, filename)
            _reject(value, "'", index, filename)
            pending[key] = value
            continue

        raise RequirementsError(f"{filename}:{index}: unsupported line {line!r}")

    if folded_key is not None:
        folded_key = None
    flush_pending(len(lines))

    if seen_sections != SECTION_KEYS:
        raise RequirementsError(
            f"{filename}: expected sections {sorted(SECTION_KEYS)}, found {sorted(seen_sections)}"
        )

    seen: set[tuple[str, str]] = set()
    for entry in entries:
        key = (entry.leg, entry.scenario)
        if key in seen:
            raise RequirementsError(f"{filename}: duplicate entry {key}")
        seen.add(key)
    return entries


def load_canonical() -> list[Canonical]:
    all_entries: list[Canonical] = []
    for revision, filename in REVISIONS.items():
        path = HERE / filename
        raw = path.read_bytes()
        digest = hashlib.sha256(raw).hexdigest()
        if digest != PINNED_SHA256[revision]:
            raise RequirementsError(
                f"{filename}: sha256 {digest} does not match the pinned {PINNED_SHA256[revision]}"
            )
        text = raw.decode("utf-8")
        if "\r" in text:
            raise RequirementsError(f"{filename}: CRLF input is not supported")
        entries = parse_requirements(revision, text, filename)
        for key, expected in EXPECTED_COUNTS.items():
            if key[0] != revision:
                continue
            if len(key) == 3:
                actual = sum(
                    1
                    for e in entries
                    if e.status == "required" and e.leg == key[1]
                )
            else:
                actual = sum(1 for e in entries if e.status == "not_scored")
            if actual != expected:
                raise RequirementsError(
                    f"{filename}: expected {expected} entries for {key[1:]}, found {actual}"
                )
        all_entries.extend(entries)
    return all_entries


# --------------------------------------------------------------------------
# Applicability policy
# --------------------------------------------------------------------------

CLIENT_LEG = (
    "This scenario exercises an MCP client implementation. cribra-mcp is a server "
    "and ships no MCP client, so the scenario has no subject under test here."
)

TOOL_FIXTURE_ABSENT = (
    "Requires the suite's fixture tool {tool}. cribra-mcp advertises exactly "
    "cribra_scan and cribra_transform and MUST NOT advertise suite fixture tools."
)

RESOURCES_ABSENT = (
    "Requires the resources capability. cribra-mcp advertises only the tools "
    "capability, so resources/list, resources/read and resource templates have no "
    "subject under test."
)

PROMPTS_ABSENT = (
    "Requires the prompts capability. cribra-mcp advertises only the tools "
    "capability, so prompts/list and prompts/get have no subject under test."
)

HTTP_ONLY = (
    "Requires the Streamable HTTP transport: HTTP status codes, response headers, "
    "Host/Origin validation or SSE streams. cribra-mcp is a stdio-only server."
)

MRTR_ABSENT = (
    "Requires multi-round-trip input_required support plus the suite fixture tool "
    "{tool}. cribra-mcp advertises no elicitation, sampling or roots capability and "
    "returns fixed tool errors instead of input requests."
)

# (revision, scenario) -> Verdict for `required` server-leg scenarios.
# `partial` means the scenario's transport-independent checks are reproduced
# locally and the transport- or fixture-specific remainder is excluded; the
# evidence column always names the exact test.
#
# Keyed by revision as well as scenario: a legacy row must cite legacy evidence,
# never a modern-era test that merely covers the same requirement shape.
SERVER_POLICY: dict[tuple[str, str], Verdict] = {
    ("2026-07-28", "server-stateless"): Verdict(
        "partial",
        "native-stdio",
        "pass",
        "Transport-independent checks only: per-request _meta validation, "
        "server/discover shape and unsupported-version rejection. HTTP status "
        "codes, header matching (-32020) and subscriptions/listen streams are "
        "excluded. native-stdio: crates/cribra-mcp/tests/adversarial.rs"
        "::modern_version_negotiation_is_exact_over_the_wire; in-process: "
        "crates/cribra-mcp/tests/protocol_conformance.rs"
        "::modern_discovery_reports_exact_capabilities_and_caching",
    ),
    ("2026-07-28", "tools-list"): Verdict(
        "applicable",
        "native-stdio",
        "pass",
        "native-stdio: crates/cribra-mcp/tests/jsonl_stdio.rs"
        "::modern_tools_execute_scan_and_transform_without_source_disclosure "
        "(exact advertised names plus ttlMs/cacheScope); tool schema shape "
        "in-process: crates/cribra-mcp/tests/protocol_conformance.rs"
        "::modern_tools_surface_is_two_static_contracts",
    ),
    ("2026-07-28", "tools-call-simple-text"): Verdict(
        "partial",
        "native-stdio",
        "pass",
        "Fixture tool test_simple_text is absent; the equivalent requirement (a "
        "tools/call result carries a text content block) is exercised with both "
        "advertised tools. native-stdio: crates/cribra-mcp/tests/jsonl_stdio.rs"
        "::modern_tools_execute_scan_and_transform_without_source_disclosure",
    ),
    ("2026-07-28", "tools-call-error"): Verdict(
        "partial",
        "native-stdio",
        "pass",
        "Fixture tool test_error_handling is absent; the equivalent requirement "
        "(tool-level failure returns isError:true with schema-valid structured "
        "content, not a JSON-RPC error) is exercised with the advertised tools. "
        "native-stdio: crates/cribra-mcp/tests/jsonl_stdio.rs"
        "::tool_success_and_failure_results_match_advertised_schemas",
    ),
    ("2026-07-28", "caching"): Verdict(
        "partial",
        "locally-equivalent",
        "pass",
        "tools/list and server/discover carry ttlMs and cacheScope, checked "
        "in-process against the pinned schema. The scenario also probes "
        "prompts/list, resources/list, resources/templates/list and "
        "resources/read, which are excluded because those capabilities are not "
        "advertised. in-process: crates/cribra-mcp/tests/protocol_conformance.rs"
        "::modern_discovery_reports_exact_capabilities_and_caching",
    ),
    ("2025-11-25", "server-initialize"): Verdict(
        "applicable",
        "native-stdio",
        "pass",
        "Handshake, serverInfo, protocolVersion and capabilities over stdio. The "
        "scenario's optional MCP-Session-Id check is HTTP-transport-only and is not "
        "applicable to a stdio server. native-stdio: "
        "crates/cribra-mcp/tests/jsonl_stdio.rs"
        "::legacy_transcript_matches_pinned_expectations; exact shaping "
        "in-process: crates/cribra-mcp/tests/protocol_conformance.rs"
        "::legacy_lifecycle_and_shaping_are_exact",
    ),
    ("2025-11-25", "ping"): Verdict(
        "applicable",
        "native-stdio",
        "pass",
        "Legacy sessions answer ping with an empty result object. native-stdio: "
        "crates/cribra-mcp/tests/jsonl_stdio.rs"
        "::legacy_transcript_matches_pinned_expectations; exact result "
        "in-process: crates/cribra-mcp/tests/protocol_conformance.rs"
        "::legacy_ping_and_removed_modern_methods_are_exact",
    ),
    ("2025-11-25", "tools-list"): Verdict(
        "applicable",
        "native-stdio",
        "pass",
        "Two tools advertised in the legacy era with no modern-only fields, "
        "validated against the 2025-11-25 schema. native-stdio: "
        "crates/cribra-mcp/tests/jsonl_stdio.rs"
        "::legacy_tools_surface_omits_modern_fields",
    ),
    ("2025-11-25", "tools-call-simple-text"): Verdict(
        "partial",
        "native-stdio",
        "pass",
        "Fixture tool test_simple_text is absent; the equivalent requirement (a "
        "legacy tools/call result carries a non-empty text content block) is "
        "exercised with an advertised tool. native-stdio: "
        "crates/cribra-mcp/tests/jsonl_stdio.rs"
        "::legacy_tools_surface_omits_modern_fields",
    ),
    ("2025-11-25", "tools-call-error"): Verdict(
        "partial",
        "native-stdio",
        "pass",
        "Fixture tool test_error_handling is absent; the equivalent requirement "
        "(a legacy tool-level failure is an isError result with schema-valid "
        "structured content, not a JSON-RPC error) is exercised with an advertised "
        "tool. native-stdio: crates/cribra-mcp/tests/jsonl_stdio.rs"
        "::legacy_tools_surface_omits_modern_fields",
    ),
}

# Scenarios that need no per-scenario prose because a shared prerequisite
# explains them completely.
SHARED_POLICY: dict[str, str] = {
    "completion-complete": (
        "Requires completion/complete for prompt or resource-template arguments. "
        "cribra-mcp advertises only the tools capability and no completions "
        "capability."
    ),
    "server-sse-multiple-streams": HTTP_ONLY,
    "server-sse-polling": HTTP_ONLY,
    "dns-rebinding-protection": HTTP_ONLY,
    "http-header-validation": HTTP_ONLY,
    "http-custom-header-server-validation": HTTP_ONLY,
    "resources-list": RESOURCES_ABSENT,
    "resources-read-text": RESOURCES_ABSENT,
    "resources-read-binary": RESOURCES_ABSENT,
    "resources-templates-read": RESOURCES_ABSENT,
    "resources-subscribe": RESOURCES_ABSENT,
    "resources-unsubscribe": RESOURCES_ABSENT,
    "sep-2164-resource-not-found": RESOURCES_ABSENT,
    "prompts-list": PROMPTS_ABSENT,
    "prompts-get-simple": PROMPTS_ABSENT,
    "prompts-get-with-args": PROMPTS_ABSENT,
    "prompts-get-embedded-resource": PROMPTS_ABSENT,
    "prompts-get-with-image": PROMPTS_ABSENT,
    "logging-set-level": (
        "Requires the optional logging capability and logging/setLevel handling. "
        "cribra-mcp does not advertise logging and emits no log notifications."
    ),
    "tools-call-with-progress": (
        "Requires a fixture tool that emits notifications/progress for a client "
        "progressToken. cribra-mcp emits no tool-generated notifications and "
        "advertises no progress support."
    ),
    "tools-call-with-logging": (
        "Requires the optional logging capability plus the suite fixture tool "
        "test_tool_with_logging. cribra-mcp advertises no logging capability."
    ),
    "tools-call-sampling": (
        "Requires server-initiated sampling/createMessage and the suite fixture "
        "tool test_sampling. cribra-mcp advertises no sampling capability and "
        "emits no server-to-client requests."
    ),
    "tools-call-elicitation": (
        "Requires server-initiated elicitation/create and the suite fixture tool "
        "test_elicitation. cribra-mcp advertises no elicitation capability and "
        "emits no server-to-client requests."
    ),
    "elicitation-sep1034-defaults": (
        "Requires the suite fixture tool test_elicitation_sep1034_defaults and "
        "server-initiated elicitation/create. cribra-mcp advertises no elicitation "
        "capability."
    ),
    "elicitation-sep1330-enums": (
        "Requires the suite fixture tool test_elicitation_sep1330_enums and "
        "server-initiated elicitation/create. cribra-mcp advertises no elicitation "
        "capability."
    ),
    "json-schema-2020-12": (
        "Requires the suite fixture tool whose inputSchema exercises JSON Schema "
        "2020-12 keywords. cribra-mcp advertises exactly cribra_scan and "
        "cribra_transform."
    ),
    "server-session-lifecycle": (
        "Requires the Streamable HTTP session mechanism (Mcp-Session-Id headers "
        "and session termination). cribra-mcp is a stdio-only server."
    ),
}

NOT_SCORED_POLICY: dict[str, str] = {
    "tasks-lifecycle": "Extension suite (SEP-2663) requiring the tasks capability.",
    "tasks-capability-negotiation": "Extension suite (SEP-2663) requiring the tasks capability.",
    "tasks-wire-fields": "Extension suite (SEP-2663) requiring the tasks capability.",
    "tasks-request-state-removal": "Extension suite (SEP-2663) requiring the tasks capability.",
    "tasks-mrtr-input": "Extension suite (SEP-2663) requiring the tasks capability.",
    "tasks-request-headers": "Extension suite (SEP-2663) requiring the tasks capability.",
    "tasks-dispatch-and-envelope": "Extension suite (SEP-2663) requiring the tasks capability.",
    "tasks-status-notifications": "Extension suite (SEP-2663) requiring the tasks capability.",
    "tasks-required-task-error": "Extension suite (SEP-2663) requiring the tasks capability.",
    "tasks-mrtr-composition": "Extension suite (SEP-2663) requiring the tasks capability.",
}


def tool_fixture(tool: str) -> str:
    return TOOL_FIXTURE_ABSENT.format(tool=f"`{tool}`")


def mrtr_fixture(tool: str) -> str:
    return MRTR_ABSENT.format(tool=f"`{tool}`")


IMAGE_ABSENT = (
    "Requires a fixture tool returning image content. cribra-mcp advertises two "
    "tools that return text content only, so image content is not applicable to "
    "the advertised contract."
)
AUDIO_ABSENT = (
    "Requires a fixture tool returning audio content. cribra-mcp advertises two "
    "tools that return text content only, so audio content is not applicable to "
    "the advertised contract."
)
EMBEDDED_ABSENT = (
    "Requires a fixture tool returning embedded resource content. cribra-mcp "
    "advertises two tools that return text content only."
)
MIXED_ABSENT = (
    "Requires a fixture tool returning several content types in one result. "
    "cribra-mcp advertises two tools that return a single text content block."
)

INPUT_REQUIRED_TOOL = {
    "input-required-result-basic-elicitation": "test_input_required_result_elicitation",
    "input-required-result-basic-sampling": "test_input_required_result_sampling",
    "input-required-result-basic-list-roots": "test_input_required_result_list_roots",
    "input-required-result-request-state": "test_input_required_result_request_state",
    "input-required-result-multiple-input-requests": "test_input_required_result_multiple",
    "input-required-result-multi-round": "test_input_required_result_multi_round",
    "input-required-result-missing-input-response": "test_input_required_result_missing",
    "input-required-result-non-tool-request": "test_input_required_result_non_tool",
    "input-required-result-result-type": "test_input_required_result_elicitati",
    "input-required-result-unsupported-methods": "test_input_required_result_unsupported",
    "input-required-result-tampered-state": "test_input_required_result_tampered",
    "input-required-result-capability-check": "test_input_required_result_capability",
    "input-required-result-ignore-extra-params": "test_input_required_result_extra",
    "input-required-result-validate-input": "test_input_required_result_validate",
}

TOOL_FIXTURE_SCENARIOS = {
    "tools-call-image": IMAGE_ABSENT,
    "tools-call-audio": AUDIO_ABSENT,
    "tools-call-embedded-resource": EMBEDDED_ABSENT,
    "tools-call-mixed-content": MIXED_ABSENT,
}


def policy_for(entry: Canonical) -> Verdict:
    if entry.leg == "client":
        return Verdict("not-applicable", "not-executed", "n/a", CLIENT_LEG)

    scenario = entry.scenario
    policy = SERVER_POLICY.get((entry.revision, scenario))
    if policy is not None:
        return policy

    justification = SHARED_POLICY.get(scenario)
    if justification is None:
        justification = TOOL_FIXTURE_SCENARIOS.get(scenario)
    if justification is None and scenario in INPUT_REQUIRED_TOOL:
        justification = mrtr_fixture(INPUT_REQUIRED_TOOL[scenario])
    if justification is None and scenario in NOT_SCORED_POLICY:
        justification = NOT_SCORED_POLICY[scenario]
    if justification is None:
        raise RequirementsError(
            f"no applicability policy for server scenario {scenario!r} "
            f"({entry.revision}, {entry.status})"
        )

    prefix = ""
    if entry.status == "not_scored":
        prefix = (
            f"Not scored upstream ({entry.reason}), so it never counts toward a "
            "pass rate. "
        )
    return Verdict("not-applicable", "not-executed", "n/a", prefix + justification)


# --------------------------------------------------------------------------
# Matrix rendering
# --------------------------------------------------------------------------


def build_rows(entries: list[Canonical]) -> list[tuple[Canonical, Verdict]]:
    rows = []
    for entry in entries:
        verdict = policy_for(entry)
        rows.append((entry, verdict))
        if verdict.applicability not in APPLICABILITY:
            raise RequirementsError(f"bad applicability {verdict.applicability}")
        if verdict.execution not in EXECUTION:
            raise RequirementsError(f"bad execution {verdict.execution}")
        if verdict.result not in RESULT:
            raise RequirementsError(f"bad result {verdict.result}")
        if len(verdict.evidence.strip()) < MIN_JUSTIFICATION:
            raise RequirementsError(f"{entry.scenario}: justification too short")
        if verdict.applicability == "not-applicable":
            if verdict.execution != "not-executed" or verdict.result != "n/a":
                raise RequirementsError(
                    f"{entry.scenario}: not-applicable must be not-executed/n/a"
                )
        else:
            if verdict.execution == "not-executed" or verdict.result == "n/a":
                raise RequirementsError(
                    f"{entry.scenario}: applicable rows need real execution evidence"
                )
            if verdict.result == "pass":
                check_evidence(verdict.evidence)
    return rows


TEST_REF_RE = re.compile(
    r"(crates/cribra-mcp/[A-Za-z0-9_./-]+\.rs)::([A-Za-z0-9_]+)"
)


def check_evidence(evidence: str) -> None:
    """Every `pass` must point at a test file and function that exist."""
    refs = TEST_REF_RE.findall(evidence)
    if not refs:
        raise RequirementsError(
            "a passing row must cite <path>::<test_fn>; none found in: " + evidence[:80]
        )
    root = CRATE.parents[1]
    for path, name in refs:
        target = root / path
        if not target.exists():
            raise RequirementsError(f"evidence references missing file {path}")
        source = target.read_text(encoding="utf-8")
        if f"fn {name}(" not in source:
            raise RequirementsError(f"evidence references missing test fn {path}::{name}")


def render_matrix(entries: list[Canonical]) -> str:
    rows = build_rows(entries)
    lines: list[str] = []
    lines.append(BEGIN)
    lines.append("")
    lines.append("### Requirement matrix")
    lines.append("")
    lines.append(
        "Every canonical entry from both pinned requirement sets appears exactly "
        "once, keyed by `(revision, leg, scenario)`. `canonical` and `reason` are "
        "copied verbatim from upstream; `applicability`, `execution`, `result` and "
        "`evidence` are Cribra's own assessment."
    )
    lines.append("")
    lines.append(
        "- `execution = official-runner` appears in the vocabulary but is used by "
        "**zero** rows: the official runner has no stdio server mode."
    )
    lines.append(
        "- `execution = native-stdio` means a Rust integration test spoke JSON-RPC "
        "directly to the real binary over stdin/stdout."
    )
    lines.append(
        "- `execution = locally-equivalent` means a local harness or in-process "
        "protocol test reproduced the scenario's transport-independent checks "
        "against the same pinned schemas. The evidence cell names the channel of "
        "every cited test."
    )
    lines.append("")
    lines.append(
        "| revision | leg | scenario | canonical | reason | applicability | execution | result | evidence |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for entry, verdict in rows:
        reason = entry.reason or "-"
        evidence = verdict.evidence.replace("|", "\\|")
        lines.append(
            f"| {entry.revision} | {entry.leg} | {entry.scenario} | {entry.status} | "
            f"{reason} | {verdict.applicability} | {verdict.execution} | "
            f"{verdict.result} | {evidence} |"
        )
    lines.append("")
    lines.append("### Matrix summary")
    lines.append("")
    lines.extend(render_summary(rows))
    lines.append("")
    lines.append(END)
    return "\n".join(lines)


def render_summary(rows: list[tuple[Canonical, Verdict]]) -> list[str]:
    out: list[str] = []
    out.append("| revision | leg | canonical | rows | applicable | partial | not applicable | passed |")
    out.append("| --- | --- | --- | --- | --- | --- | --- | --- |")
    for revision in REVISIONS:
        for leg in ("server", "client"):
            for status in ("required", "not_scored"):
                subset = [
                    (e, v)
                    for e, v in rows
                    if e.revision == revision and e.leg == leg and e.status == status
                ]
                if not subset:
                    continue
                applicable = sum(1 for _, v in subset if v.applicability == "applicable")
                partial = sum(1 for _, v in subset if v.applicability == "partial")
                na = sum(1 for _, v in subset if v.applicability == "not-applicable")
                passed = sum(1 for _, v in subset if v.result == "pass")
                out.append(
                    f"| {revision} | {leg} | {status} | {len(subset)} | {applicable} | "
                    f"{partial} | {na} | {passed} |"
                )
    official = sum(1 for _, v in rows if v.execution == "official-runner")
    out.append("")
    out.append(f"Rows executed by the official conformance runner: **{official}**.")
    total = len(rows)
    passed = sum(1 for _, v in rows if v.result == "pass")
    failed = sum(1 for _, v in rows if v.result == "fail")
    untested = sum(1 for _, v in rows if v.result == "not-tested")
    out.append(
        f"Total canonical rows: **{total}**; passed: **{passed}**; failed: "
        f"**{failed}**; not tested: **{untested}**."
    )
    return out


# --------------------------------------------------------------------------
# Self-test: the reader must reject every unsupported structure
# --------------------------------------------------------------------------

VALID_MINIMAL = """# comment
server:
  - tools-list
client:
  - initialize
not_scored:
  - scenario: tasks-lifecycle
    leg: server
    reason: extension
    note: >-
      io.modelcontextprotocol/tasks (SEP-2663); pending against the reference
      fixture, run for visibility
"""

INVALID_SAMPLES = {
    "tab": "server:\n\t- tools-list\nclient:\nnot_scored:\n",
    "flow-sequence": "server: [tools-list]\nclient:\nnot_scored:\n",
    "quoted": 'server:\n  - "tools-list"\nclient:\nnot_scored:\n',
    "anchor": "server:\n  - &a tools-list\nclient:\nnot_scored:\n",
    "unknown-top-key": "servers:\n  - tools-list\nclient:\nnot_scored:\n",
    "duplicate-section": "server:\n  - tools-list\nserver:\n  - tools-list\nclient:\nnot_scored:\n",
    "unknown-reason": (
        "server:\nclient:\nnot_scored:\n  - scenario: x\n    leg: server\n    reason: maybe\n"
    ),
    "unknown-leg": "server:\nclient:\nnot_scored:\n  - scenario: x\n    leg: both\n    reason: extension\n",
    "missing-leg": "server:\nclient:\nnot_scored:\n  - scenario: x\n    reason: extension\n",
    "literal-block": (
        "server:\nclient:\nnot_scored:\n  - scenario: x\n    leg: server\n"
        "    reason: extension\n    note: |\n      text\n"
    ),
    "nested-list": "server:\n  - - tools-list\nclient:\nnot_scored:\n",
    "missing-section": "server:\n  - tools-list\nclient:\n",
}


def self_test() -> int:
    failures = 0
    entries = parse_requirements("2026-07-28", VALID_MINIMAL, "valid")
    expected = [
        ("server", "tools-list", "required", None),
        ("client", "initialize", "required", None),
        ("server", "tasks-lifecycle", "not_scored", "extension"),
    ]
    actual = [(e.leg, e.scenario, e.status, e.reason) for e in entries]
    if actual != expected:
        print(f"FAIL valid sample parsed as {actual}")
        failures += 1

    for name, sample in INVALID_SAMPLES.items():
        try:
            parse_requirements("2026-07-28", sample, name)
        except RequirementsError:
            continue
        print(f"FAIL invalid sample accepted: {name}")
        failures += 1

    if failures:
        print(f"self-test: {failures} failure(s)")
        return 1
    print(
        f"self-test: ok ({len(INVALID_SAMPLES)} malformed inputs rejected, "
        "valid sample parsed)"
    )
    return 0


# --------------------------------------------------------------------------
# Entry point
# --------------------------------------------------------------------------


def replace_region(document: str, region: str) -> str:
    start = document.find(BEGIN)
    if start == -1:
        raise RequirementsError(f"{DOC} has no generated region marker")
    end = document.find(END)
    if end == -1 or end < start:
        raise RequirementsError(f"{DOC} has an unterminated generated region")
    end += len(END)
    return document[:start] + region + document[end:]


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--check", action="store_true")
    group.add_argument("--write", action="store_true")
    group.add_argument("--self-test", action="store_true")
    group.add_argument("--print-canonical", action="store_true")
    args = parser.parse_args(argv)

    if args.self_test:
        return self_test()

    try:
        entries = load_canonical()
        if args.print_canonical:
            for entry in entries:
                reason = entry.reason or "-"
                print(f"{entry.revision}\t{entry.leg}\t{entry.scenario}\t{entry.status}\t{reason}")
            print(f"total={len(entries)}", file=sys.stderr)
            return 0

        region = render_matrix(entries)
        if not DOC.exists():
            raise RequirementsError(f"{DOC} does not exist")
        document = DOC.read_text(encoding="utf-8")

        if args.write:
            updated = replace_region(document, region)
            if updated != document:
                DOC.write_text(updated, encoding="utf-8")
                print(f"rewrote generated matrix in {DOC}")
            else:
                print("generated matrix already up to date")
            return 0

        if BEGIN not in document or END not in document:
            raise RequirementsError(f"{DOC} has no generated matrix region")
        if document != replace_region(document, region):
            print(
                "conformance matrix is stale: run `python3 "
                f"{Path(__file__).name} --write`",
                file=sys.stderr,
            )
            return 1
        rows = len(entries)
        applicable = sum(1 for e in entries if policy_for(e).applicability != "not-applicable")
        print(f"conformance matrix ok: {rows} canonical rows, {applicable} applicable/partial")
        return 0
    except RequirementsError as error:
        print(f"conformance matrix error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
