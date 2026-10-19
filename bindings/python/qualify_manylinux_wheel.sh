#!/bin/sh
# Qualify the auditwheel-repaired manylinux_2_28_x86_64 Fuse wheel.
#
# Requires Docker. Runs the official PyPA manylinux image (pinned by digest) under
# linux/amd64 and executes bindings/python/manylinux/consumer_entrypoint.sh inside
# it, so wheel_smoke.py runs the installed-consumer tests against the final
# repaired wheel with the image's own CPython 3.11-3.14. Without an argument the
# single wheel under
# target/uniffi-python-wheel/fuse/manylinux_2_28_x86_64/dist/ is qualified.
#
# Build that wheel first with bindings/python/build_manylinux_wheel.sh (3.1). This
# step never rebuilds and never publishes.
set -eu

# Same pinned official PyPA manylinux image as build_manylinux_wheel.sh.
readonly MANYLINUX_IMAGE="quay.io/pypa/manylinux_2_28_x86_64@sha256:1e7b8460ad8fb3fb0fbe1cd818845159ac896b4fd17452d56e892c291f3b97a8"

command -v docker >/dev/null 2>&1 || {
    echo "qualify_manylinux_wheel.sh: docker is required." >&2
    exit 2
}

root=$(CDPATH='' cd -- "$(dirname -- "$0")/../.." && pwd)
wheel=${1:-}
if [ -n "${wheel}" ]; then
    case "${wheel}" in
        /*) ;;
        *) wheel="$(pwd)/${wheel}" ;;
    esac
else
    set -- "${root}"/target/uniffi-python-wheel/fuse/manylinux_2_28_x86_64/dist/cribra-*-py3-none-manylinux_2_28_x86_64.whl
    if [ ! -f "$1" ] || [ "$#" -ne 1 ]; then
        echo "qualify_manylinux_wheel.sh: expected exactly one repaired wheel under" >&2
        echo "  target/uniffi-python-wheel/fuse/manylinux_2_28_x86_64/dist/" >&2
        echo "build it first with 'just uniffi-python-manylinux-wheel'." >&2
        exit 2
    fi
    wheel=$1
fi

case "${wheel}" in
    "${root}"/*) relative=${wheel#"${root}"/} ;;
    *)
        echo "qualify_manylinux_wheel.sh: wheel must be inside the repository: ${wheel}" >&2
        exit 2
        ;;
esac
[ -f "${root}/${relative}" ] || {
    echo "qualify_manylinux_wheel.sh: wheel not found: ${wheel}" >&2
    exit 2
}

# The container runs as root and only reads the wheel; nothing is written and
# nothing is published.
exec docker run --rm \
    --platform linux/amd64 \
    --volume "${root}:/work" \
    --workdir /work \
    "${MANYLINUX_IMAGE}" \
    /bin/bash /work/bindings/python/manylinux/consumer_entrypoint.sh "/work/${relative}"
