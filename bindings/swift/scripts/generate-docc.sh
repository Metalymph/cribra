#!/usr/bin/env bash
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
cd "$root"

profile="${1:-all}"
shift || true

hosting_base_path=""

while (( $# > 0 )); do
  case "$1" in
    --hosting-base-path)
      if (( $# < 2 )) || [[ -z "$2" ]]; then
        echo "FAIL: --hosting-base-path requires a value" >&2
        exit 2
      fi
      hosting_base_path="$2"
      shift 2
      ;;
    *)
      echo "Usage: $0 <core|flow|fuse|all> [--hosting-base-path <path>]" >&2
      exit 2
      ;;
  esac
done

case "$profile" in
  core|flow|fuse)
    profiles=("$profile")
    ;;
  all)
    profiles=(core flow fuse)
    ;;
  *)
    echo "Usage: $0 <core|flow|fuse|all> [--hosting-base-path <path>]" >&2
    exit 2
    ;;
esac

if [[ -n "$hosting_base_path" ]]; then
  if [[ "$profile" == "all" ]]; then
    echo "FAIL: --hosting-base-path requires one profile" >&2
    exit 2
  fi
  if [[ "$hosting_base_path" != /* || "$hosting_base_path" == */ ]]; then
    echo "FAIL: hosting base path must start with '/' and have no trailing slash" >&2
    exit 2
  fi
fi

sdk="$(xcrun --sdk macosx --show-sdk-path)"

for current in "${profiles[@]}"; do
  case "$current" in
    core) package="bindings/swift" ;;
    flow) package="bindings/swift/Profiles/Flow" ;;
    fuse) package="bindings/swift/Profiles/Fuse" ;;
  esac

  output="target/swift-docc/$current"
  symbols="$output/symbol-graphs"
  archive="$output/Cribra.doccarchive"

  echo "=== Swift-DocC: $current ==="

  swift build --package-path "$package" --configuration debug

  build_dir="$(swift build --package-path "$package" --show-bin-path)"

  mkdir -p "$symbols"
  rm -f "$symbols"/*.symbols.json
  rm -rf "$archive"

  swift symbolgraph-extract \
    -module-name Cribra \
    -minimum-access-level public \
    -target arm64-apple-macosx13.0 \
    -sdk "$sdk" \
    -I "$build_dir" \
    -I "$build_dir/include" \
    -output-dir "$symbols"

  python3 - "$symbols" "$current" <<'PY'
import json
import pathlib
import sys

directory = pathlib.Path(sys.argv[1])
profile = sys.argv[2]

files = sorted(directory.glob("*.symbols.json"))
if not files:
    raise SystemExit(f"FAIL: {profile}: no symbol graphs")

symbols = []
for path in files:
    symbols.extend(json.loads(path.read_text()).get("symbols", []))

names = {s.get("names", {}).get("title", "") for s in symbols}

required = {
    "Scanner",
    "ScanReport",
    "Finding",
    "StreamSession",
    "IncrementalSession",
}

missing = required - names
if missing:
    raise SystemExit(
        f"FAIL: {profile}: missing API symbols: {sorted(missing)}"
    )

print(f"PASS: {profile}: {len(symbols)} public symbols")
PY

  hosting_args=()
  if [[ -n "$hosting_base_path" ]]; then
    hosting_args=(
      --transform-for-static-hosting
      --hosting-base-path "$hosting_base_path"
    )
  fi

  xcrun docc convert \
    bindings/swift/Documentation/Cribra.docc \
    --additional-symbol-graph-dir "$symbols" \
    --output-dir "$archive" \
    --fallback-display-name "Cribra $current" \
    --fallback-bundle-identifier "dev.metalymph.cribra.$current" \
    --fallback-default-module-kind "Library" \
    --experimental-documentation-coverage \
    --analyze \
    ${hosting_args[@]+"${hosting_args[@]}"}

  test -d "$archive"

  echo "PASS: $archive"
done
