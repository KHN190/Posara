#!/usr/bin/env bash
# Long property runs: PROPTEST_CASES (default 10000) per property.
set -euo pipefail
cd "$(dirname "$0")/.."
export PROPTEST_CASES=${PROPTEST_CASES:-10000}
cargo test -q --release -p posara-usb-midi -p posara-sync-jack -p posara-flash-log
cargo test -q --release -p posara-core --test mono_mix_test
cargo test -q --release -p posara --test gfx_controller_test
cargo test -q --release -p posara --test gfx_raster_test --test gfx_framebuffer_test --test lang_fs_test
PROPTEST_CASES=${SFX_CASES:-2000} cargo test -q --release -p posara --test sfx_synth_output_test --test sfx_realtime_alloc_test
