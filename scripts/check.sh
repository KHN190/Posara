#!/usr/bin/env bash
# --fast (default): tests, web, feature matrix, portability, wasm. --slow: also fuzz.sh.
set -uo pipefail
cd "$(dirname "$0")/.."
mode=${1:---fast}
case $mode in --fast|--slow) ;; *) echo "usage: $0 [--fast|--slow]"; exit 2 ;; esac
fail=0
step() { echo "== $1"; shift; "$@" || fail=1; }
step "workspace tests" cargo test -q --workspace --exclude posara-web
step "web tests" cargo test -q -p posara --no-default-features --features web,test
step "feature matrix" bash scripts/check-features.sh
step "portability" bash scripts/check-portable.sh
step "wasm" bash scripts/check-wasm.sh
[ "$mode" = --slow ] && step "fuzz" bash scripts/fuzz.sh
[ $fail = 0 ] && echo "check $mode: all green" || { echo "check $mode: FAILED"; exit 1; }
