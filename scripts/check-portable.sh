#!/usr/bin/env bash
# Portable-core oracle: the capability layer names zero raw platform APIs. Time/
# fs/thread live behind Clock / Storage / Presenter seams. Exempt desktop sinks:
# png.rs (image save), record.rs (WAV), midi (desktop-only midir), runner driver.
set -uo pipefail
cd "$(dirname "$0")/.."
PAT='std::time::Instant|std::time::SystemTime|std::thread|std::fs::'

hits=$(grep -rnE "$PAT" src/host.rs src/plugins src/backend/clock.rs --include=*.rs 2>/dev/null \
  | grep -vE 'plugins/gfx/png.rs|plugins/sfx/record.rs|plugins/midi/')

if [ -n "$hits" ]; then
  echo "portable oracle: LEAK"
  echo "$hits"
  exit 1
fi
echo "portable oracle: clean"
