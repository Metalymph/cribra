#!/bin/sh
set -eu

header="${1:-include/cribra.h}"
library="${2:-}"

if [ -z "$library" ]; then
    case "$(uname -s)" in
        Darwin)
            library="target/debug/libcribra_capi.dylib"
            ;;
        Linux)
            library="target/debug/libcribra_capi.so"
            ;;
        *)
            echo "unsupported platform for Unix symbol check" >&2
            exit 2
            ;;
    esac
fi

if [ ! -f "$header" ]; then
    echo "missing generated header: $header" >&2
    exit 1
fi

if [ ! -f "$library" ]; then
    echo "missing native dynamic library: $library" >&2
    exit 1
fi

mkdir -p target/c-smoke

expected="target/c-smoke/capi-symbols.expected"
actual="target/c-smoke/capi-symbols.actual"

# Extract every native ABI function declared by the generated C header.
#
# Use grep rather than sed word-boundary syntax so this behaves consistently on
# both BSD/macOS and GNU/Linux.
grep -Eo 'cribra_[A-Za-z0-9_]+[[:space:]]*\(' "$header" \
    | sed -E 's/[[:space:]]*\($//' \
    | sort -u > "$expected"

case "$(uname -s)" in
    Darwin)
        nm -gU "$library" \
            | awk '{print $NF}' \
            | grep -E '^_cribra_[A-Za-z0-9_]+$' \
            | sed 's/^_//' \
            | sort -u > "$actual"
        ;;
    Linux)
        nm -D --defined-only "$library" \
            | awk '{print $NF}' \
            | grep -E '^cribra_[A-Za-z0-9_]+$' \
            | sort -u > "$actual"
        ;;
esac

if [ ! -s "$expected" ]; then
    echo "no cribra_* functions found in generated header" >&2
    exit 1
fi

if [ ! -s "$actual" ]; then
    echo "no cribra_* functions exported by native dynamic library" >&2
    exit 1
fi

if ! diff -u "$expected" "$actual"; then
    echo "native C ABI symbol set differs from generated header" >&2
    exit 1
fi

count="$(wc -l < "$actual" | tr -d '[:space:]')"
echo "cribra-capi symbols: ok (${count} exports)"
