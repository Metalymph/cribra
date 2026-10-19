#!/bin/bash
# In-container half of the manylinux_2_28_x86_64 Fuse wheel qualification.
#
# Runs as root inside the pinned official PyPA manylinux image, driven by
# bindings/python/build_manylinux_wheel.sh. It installs the pinned Rust
# toolchain and auditwheel, builds the wheel through the normal builder, and
# lets auditwheel decide the platform tag. Nothing is published.
set -euo pipefail

readonly MANYLINUX_PLATFORM="manylinux_2_28_x86_64"
readonly RUST_TOOLCHAIN="1.97.0"
readonly AUDITWHEEL_VERSION="6.8.2"
readonly PYTHON="/opt/python/cp313-cp313/bin/python"
readonly OUT="target/uniffi-python-wheel/fuse/${MANYLINUX_PLATFORM}"

# The image ships no Rust toolchain; install the pinned one the builder expects.
if ! command -v rustup >/dev/null 2>&1; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | \
        sh -s -- -y --no-modify-path --profile minimal \
        --default-toolchain "${RUST_TOOLCHAIN}"
fi
# shellcheck source=/dev/null
. "${HOME}/.cargo/env"

"${PYTHON}" -m pip install --disable-pip-version-check --no-cache-dir \
    --requirement bindings/python/build-requirements.txt
"${PYTHON}" -m pip install --disable-pip-version-check --no-cache-dir \
    "auditwheel==${AUDITWHEEL_VERSION}"

# CRIBRA_MANYLINUX_PLATFORM opts build_wheel.py into its isolated output
# directory and auditwheel-gated tagging. The default Linux and macOS paths stay
# untouched, and an unqualified wheel is never relabelled by hand.
export CRIBRA_MANYLINUX_PLATFORM="${MANYLINUX_PLATFORM}"
"${PYTHON}" -B bindings/python/build_wheel.py

echo "manylinux_2_28_x86_64 wheel written under ${OUT}/dist:"
ls -l "${OUT}/dist"
