#!/usr/bin/env bash
# Reproduce the M3 interoperability qualification for cribra-mcp.
#
# This builds the release binary once, then runs the two independent SDK
# harnesses against it. It does not run the Rust test suite and does not claim
# official conformance-suite execution: see ../CONFORMANCE.md for what each
# command does and does not establish.
#
# Usage: ./run-qualification.sh
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../../.." && pwd)"
bin="$repo/target/release/cribra-mcp"

printf '== building release server ==\n'
(cd "$repo" && cargo build --release -p cribra-mcp)

if [[ ! -x "$bin" ]]; then
  printf 'error: release binary missing at %s\n' "$bin" >&2
  exit 1
fi

printf '\n== verifying generated requirement matrix ==\n'
python3 "$repo/crates/cribra-mcp/tests/conformance/matrix.py" --self-test
python3 "$repo/crates/cribra-mcp/tests/conformance/matrix.py" --check

printf '\n== TypeScript SDK (@modelcontextprotocol/client 2.4.0) ==\n'
ts_dir="$here/typescript"
if [[ ! -d "$ts_dir/node_modules" ]]; then
  (cd "$ts_dir" && npm ci --no-audit --no-fund)
fi
(cd "$ts_dir" && CRIBRA_MCP_BIN="$bin" node run.mjs)

printf '\n== Python SDK (mcp 2.3.0) ==\n'
py_dir="$here/python"
if [[ ! -x "$py_dir/.venv/bin/python" ]]; then
  python3 -m venv "$py_dir/.venv"
  "$py_dir/.venv/bin/pip" install --quiet --upgrade pip
  "$py_dir/.venv/bin/pip" install --quiet -r "$py_dir/requirements.txt"
fi
CRIBRA_MCP_BIN="$bin" "$py_dir/.venv/bin/python" "$py_dir/run.py"

printf '\nAll SDK qualification scenarios passed against %s\n' "$bin"
