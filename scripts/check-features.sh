#!/usr/bin/env bash
# Two-axis feature matrix: capability compiles with zero backend (platform-free),
# each backend compiles, two backends at once is rejected (compile_error).
set -uo pipefail
cd "$(dirname "$0")/.."
fail=0
ok() { echo "  ok   $*"; }
bad() { echo "  FAIL $*"; fail=1; }

check() {
  local desc="$1"; shift
  if cargo check -p posara --no-default-features "$@" >/dev/null 2>&1; then ok "$desc"; else bad "$desc"; fi
}
check "gfx capability (zero backend)"     --features gfx
check "sfx capability (zero backend)"     --features sfx
check "gfx+sfx+compiler (zero backend)"   --features gfx,sfx,compiler
check "net capability (zero backend)"     --features net,compiler
check "web backend"                       --features web
check "desktop backend"                   --features desktop

if cargo build --target wasm32-unknown-unknown -p posara-web >/dev/null 2>&1; then
  ok "posara-web wasm32 build"; else bad "posara-web wasm32 build"; fi

CORE_ALL=gfx,sfx,synth,fs,midi,net,imu
for f in gfx sfx synth fs midi net imu $CORE_ALL; do
  if cargo check -p posara-core --no-default-features --features "$f" --target thumbv8m.main-none-eabihf >/dev/null 2>&1; then
    ok "posara-core [$f] thumbv8m no_std"; else bad "posara-core [$f] thumbv8m no_std"; fi
done
for f in "" "$CORE_ALL" "std,$CORE_ALL"; do
  if cargo test -p posara-core --no-default-features ${f:+--features "$f"} >/dev/null 2>&1; then
    ok "posara-core tests [${f:-none}]"; else bad "posara-core tests [${f:-none}]"; fi
done
for c in posara-core posara-flash-log posara-sync-jack posara-usb-midi; do
  if cargo check -p "$c" --target thumbv8m.main-none-eabihf >/dev/null 2>&1; then
    ok "$c thumbv8m no_std"; else bad "$c thumbv8m no_std"; fi
done

if cargo check -p posara --no-default-features --features gfx-desktop,gfx-web >/dev/null 2>&1; then
  bad "two gfx backends rejected (compiled — guard missing!)"
else
  ok "two gfx backends rejected (compile_error)"
fi

[ $fail -eq 0 ] && echo "matrix: all green" || echo "matrix: FAILURES above"
exit $fail
