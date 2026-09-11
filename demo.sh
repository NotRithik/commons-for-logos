#!/bin/sh
# Standalone Commons demo. Build prerequisites with scripts/prepare-local.sh.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
PYTHON=${PYTHON:-python3}
if ! command -v "$PYTHON" >/dev/null 2>&1; then
    printf '%s\n' 'Python 3.12 or newer is required.' >&2
    exit 127
fi
export RISC0_DEV_MODE=0 RISC0_PROVER=ipc RISC0_EXECUTOR=ipc
exec "$PYTHON" "$ROOT/scripts/demo-local.py" "$@"
