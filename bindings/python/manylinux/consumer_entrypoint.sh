#!/bin/bash
# In-container installed-consumer qualification for the repaired manylinux wheel.
#
# Runs as root inside the pinned official PyPA manylinux image, driven by
# bindings/python/qualify_manylinux_wheel.sh. It qualifies the final
# auditwheel-repaired wheel with every CPython the image provides, using
# wheel_smoke.py's isolated, offline venvs. No Rust or Cargo is used and nothing
# is published.
set -euo pipefail

readonly WHEEL="${1:?usage: consumer_entrypoint.sh <wheel-path>}"
readonly DRIVER="/opt/python/cp313-cp313/bin/python"
readonly MANYLINUX_PLATFORM="manylinux_2_28_x86_64"

case "${WHEEL}" in
    *"-py3-none-${MANYLINUX_PLATFORM}.whl") ;;
    *)
        echo "consumer_entrypoint.sh: refusing a wheel that is not the repaired" >&2
        echo "  py3-none-${MANYLINUX_PLATFORM} wheel: ${WHEEL}" >&2
        exit 2
        ;;
esac

# Pass every manylinux CPython the image provides around the supported 3.11-3.14
# range; wheel_smoke.py qualifies each one in its own fresh venv.
interpreters=()
for minor in 11 12 13 14; do
    candidate="/opt/python/cp3${minor}-cp3${minor}/bin/python"
    if [ -x "${candidate}" ]; then
        interpreters+=("--python" "${candidate}")
    fi
done
if [ "${#interpreters[@]}" -eq 0 ]; then
    echo "consumer_entrypoint.sh: no manylinux CPython interpreters found." >&2
    exit 2
fi

exec "${DRIVER}" -B bindings/python/wheel_smoke.py "${WHEEL}" "${interpreters[@]}"
