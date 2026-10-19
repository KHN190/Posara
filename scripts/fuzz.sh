#!/usr/bin/env bash
# Long property runs: PROPTEST_CASES (default 10000) per property.
set -euo pipefail
cd "$(dirname "$0")/.."
export PROPTEST_CASES=${PROPTEST_CASES:-10000}
cargo test -q --release -p posara-usb-midi -p posara-sync-jack -p posara-flash-log
cargo test -q --release -p posara-core --features std,gfx,sfx --test mono_mix_test --test framebuffer_test --test sample_playback_test
cargo test -q --release -p posara --test gfx_controller_test
cargo test -q --release -p posara --test gfx_raster_test --test gfx_framebuffer_test --test lang_fs_test
PROPTEST_CASES=${SFX_CASES:-2000} cargo test -q --release -p posara --test sfx_synth_output_test --test sfx_env_test --test sfx_realtime_alloc_test
cargo test -q --release -p posara-core --test native_table_test
cargo test -q --release -p posara --test lang_runner_test every_native
RUSTFLAGS="--cfg loom" cargo test -q --release -p posara --no-default-features --features sfx --test sfx_spsc_test loom
