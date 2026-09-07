#!/bin/sh
# Prepare a pinned LOCAL test stack without modifying a user's installed tools.
# Separate fetch/build phases let the caller deny networking during compilation.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
PHASE=${1:-all}
case "$PHASE" in fetch|build|all) ;; *) echo 'usage: prepare-local.sh [fetch|build|all]' >&2; exit 2;; esac
OUT=${COMMONS_OUT_DIR:-"$ROOT/out"}
OUT=$(python3 -c 'import pathlib,sys; root=pathlib.Path(sys.argv[1]).resolve(); out=pathlib.Path(sys.argv[2]).resolve(); assert out != root and out.is_relative_to(root), "COMMONS_OUT_DIR must stay inside this checkout"; print(out)' "$ROOT" "$OUT")
mkdir -p "$OUT"
umask 077
mkdir -p "$OUT/home" "$OUT/tmp" "$OUT/cargo" "$OUT/host" "$OUT/guest" "$OUT/artifacts"
export TMPDIR="$OUT/tmp/" CARGO_HOME="${CARGO_HOME:-$OUT/cargo}"
export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1
export CARGO_NET_GIT_FETCH_WITH_CLI=true CARGO_INCREMENTAL=0 CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
export RISC0_DEV_MODE=0 RISC0_PROVER=ipc RISC0_EXECUTOR=ipc
PIN=47eba256479f6f785acbd138834340703cd03401
LEZ="$OUT/lez"
if [ "$PHASE" = fetch ] || [ "$PHASE" = all ]; then
    python3 "$ROOT/scripts/fetch-proof-deps.py" "$OUT/proof-deps"
    if [ ! -d "$LEZ/.git" ]; then
        mkdir -p "$LEZ"
        git -c core.hooksPath=/dev/null -C "$LEZ" init -q
        git -C "$LEZ" remote add origin https://github.com/logos-blockchain/logos-execution-zone.git
        git -c core.hooksPath=/dev/null -C "$LEZ" fetch --depth 1 origin "$PIN"
        git -c core.hooksPath=/dev/null -C "$LEZ" checkout --detach -q FETCH_HEAD
    fi
    test "$(git -C "$LEZ" rev-parse HEAD)" = "$PIN"
    for manifest in "$LEZ/Cargo.toml" "$ROOT/testnet/Cargo.toml" "$ROOT/integration/Cargo.toml" "$ROOT/cli/Cargo.toml"; do
        cargo +1.94.0 fetch --locked --manifest-path "$manifest"
    done
fi
if [ "$PHASE" = fetch ]; then printf '%s\n' 'Fetch complete. Build phase can now run with outbound networking denied.'; exit 0; fi
PATHS="$OUT/proof-deps/paths.json"
test -f "$PATHS" || { echo 'Missing verified dependency paths; run fetch phase first' >&2; exit 1; }
test "$(git -C "$LEZ" rev-parse HEAD)" = "$PIN"
get_path() { python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))[sys.argv[2]])' "$PATHS" "$1"; }
LBC_ROOT_DIR="$(get_path lbc_root)"
export LBC_ROOT_DIR
RAPIDSNARK_LIB_DIR="$(get_path rapidsnark_lib)"
export RAPIDSNARK_LIB_DIR
RISC0_SERVER_PATH="$(get_path r0vm)"
export RISC0_SERVER_PATH
GUEST_RUSTC=$(get_path guest_rustc)
export CARGO_NET_OFFLINE=true CARGO_TARGET_DIR="$OUT/host" CARGO_PROFILE_DEV_DEBUG=0
case "$(uname -s)" in
 Darwin) export DYLD_LIBRARY_PATH="$RAPIDSNARK_LIB_DIR${DYLD_LIBRARY_PATH:+:$DYLD_LIBRARY_PATH}" ;;
 Linux) export LD_LIBRARY_PATH="$RAPIDSNARK_LIB_DIR${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" ;;
esac
cargo +1.94.0 build --locked --offline --manifest-path "$LEZ/Cargo.toml" --features standalone -p sequencer_service
cargo +1.94.0 build --locked --offline --manifest-path "$ROOT/integration/Cargo.toml"
cargo +1.94.0 build --locked --offline --manifest-path "$ROOT/cli/Cargo.toml"
(
    cd "$ROOT/testnet"
    RUSTC="$GUEST_RUSTC" CARGO_TARGET_DIR="$OUT/guest" cargo +1.94.0 build --locked --offline --release \
        --target riscv32im-risc0-zkvm-elf -p commons-logos-testnet-guests
)
for family in allowlist threshold; do
    cargo +1.94.0 run --locked --offline --quiet --manifest-path "$ROOT/testnet/Cargo.toml" \
        -p commons-logos-testnet-sdk --bin pack_v024 -- \
        "$OUT/guest/riscv32im-risc0-zkvm-elf/release/commons_${family}_v024" "$OUT/artifacts/commons_$family"
done
printf '%s\n' 'Pinned local stack compiled. No node was started and no transaction was submitted.'
