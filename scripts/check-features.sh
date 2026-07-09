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
check "web backend"                       --features web
check "desktop backend"                   --features desktop

if cargo build --target wasm32-unknown-unknown -p posara-web >/dev/null 2>&1; then
  ok "posara-web wasm32 build"; else bad "posara-web wasm32 build"; fi

if cargo check -p posara --no-default-features --features gfx-desktop,gfx-web >/dev/null 2>&1; then
  bad "two gfx backends rejected (compiled — guard missing!)"
else
  ok "two gfx backends rejected (compile_error)"
fi

[ $fail -eq 0 ] && echo "matrix: all green" || echo "matrix: FAILURES above"
exit $fail
