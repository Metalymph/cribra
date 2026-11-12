#!/usr/bin/env bash
# Builds and runs the JSI-free host lifecycle tests against the real host
# Cribra C ABI static library.
#
# Usage:
#   bash tools/host-tests/run.sh            # plain build
#   bash tools/host-tests/run.sh asan       # AddressSanitizer
#   bash tools/host-tests/run.sh ubsan      # UndefinedBehaviorSanitizer
#   bash tools/host-tests/run.sh tsan       # ThreadSanitizer
#
# The Rust static library is not instrumented by these sanitizers; only the
# C++ test binary and the C++ worker/ownership headers are.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PACKAGE_DIR="$(cd "${SCRIPT_DIR}/../.." && pwd)"
REPO_ROOT="$(cd "${PACKAGE_DIR}/../.." && pwd)"
LIB="${REPO_ROOT}/target/release/libcribra_capi.a"
MODE="${1:-plain}"
OUT_DIR="${PACKAGE_DIR}/build/host-tests"
OUT="${OUT_DIR}/cribra-host-tests-${MODE}"

if [[ ! -f "${LIB}" ]]; then
  echo "==> Building host Cribra C ABI static library"
  cargo build --manifest-path "${REPO_ROOT}/Cargo.toml" --package cribra-capi --release
fi

FLAGS=(-std=c++20 -O1 -g -Wall -Wextra -pthread)
case "${MODE}" in
  plain | address | undefined)
    ;;
esac
case "${MODE}" in
  asan | address)
    FLAGS+=(-fsanitize=address -fno-omit-frame-pointer)
    ;;
  ubsan | undefined)
    FLAGS+=(-fsanitize=undefined -fno-sanitize-recover=all -fno-omit-frame-pointer)
    ;;
  tsan | thread)
    FLAGS+=(-fsanitize=thread)
    ;;
  plain)
    ;;
  *)
    echo "Unknown mode: ${MODE}" >&2
    echo "Use one of: plain, asan, ubsan, tsan" >&2
    exit 2
    ;;
esac

mkdir -p "${OUT_DIR}"

CXX="${CXX:-clang++}"
echo "==> ${MODE}: ${CXX} ${FLAGS[*]}"
"${CXX}" "${FLAGS[@]}" \
  -I"${PACKAGE_DIR}/cpp" \
  -I"${PACKAGE_DIR}/third_party/cribra/include" \
  "${PACKAGE_DIR}/tools/host-tests/main.cpp" \
  "${LIB}" \
  -o "${OUT}"

echo "==> Running ${OUT}"
ASAN_OPTIONS="${ASAN_OPTIONS:-abort_on_error=1}" \
UBSAN_OPTIONS="${UBSAN_OPTIONS:-print_stacktrace=1}" \
TSAN_OPTIONS="${TSAN_OPTIONS:-halt_on_error=1}" \
  "${OUT}"
