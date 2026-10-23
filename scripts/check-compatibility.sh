#!/usr/bin/env bash
set -uo pipefail
cd "$(dirname "$0")/.."
failed=0
python3 scripts/check-contract.py || failed=1
cargo test -p posara-core --features std,gfx,sfx,synth,fs,midi,net,imu --test native_table_test || failed=1
cargo test -p posara --test lang_compile_carts_test --test lang_fs_test --test lang_runner_test || failed=1
cargo test -p posara --no-default-features --features web,test --test lang_compile_carts_test --test lang_fs_test --test lang_runner_test || failed=1
exit "$failed"
