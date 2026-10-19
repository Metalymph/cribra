#!/bin/sh
# Build and auditwheel-qualify the manylinux_2_28_x86_64 Fuse wheel.
#
# Requires Docker. Runs the official PyPA manylinux image (pinned by digest)
# under linux/amd64 and executes bindings/python/manylinux/entrypoint.sh inside
# it. On a non-x86_64 host the image is emulated, so the build is slow.
#
# The wheel and the auditwheel report land in
# target/uniffi-python-wheel/fuse/manylinux_2_28_x86_64/ and are never published.
set -eu

# Official PyPA manylinux_2_28 x86_64 image, pinned by manifest digest
# (immutable). Tag at pin time: quay.io/pypa/manylinux_2_28_x86_64:latest
readonly MANYLINUX_IMAGE="quay.io/pypa/manylinux_2_28_x86_64@sha256:1e7b8460ad8fb3fb0fbe1cd818845159ac896b4fd17452d56e892c291f3b97a8"

command -v docker >/dev/null 2>&1 || {
    echo "build_manylinux_wheel.sh: docker is required." >&2
    exit 2
}

root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)

# The container runs as root; the build tree stays under the ignored target/
# directory, in its own manylinux_2_28_x86_64 platform directory, and is never
# published.
exec docker run --rm \
    --platform linux/amd64 \
    --volume "${root}:/work" \
    --workdir /work \
    "${MANYLINUX_IMAGE}" \
    /bin/bash /work/bindings/python/manylinux/entrypoint.sh
