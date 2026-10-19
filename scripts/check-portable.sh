#!/usr/bin/env bash
# Portable-core oracle: the capability layer names zero raw platform APIs. Time/
# fs/thread live behind Clock / Storage / Presenter seams. Exempt desktop sinks:
# png.rs (image save), record.rs (WAV), midi (desktop-only midir), runner driver.
set -uo pipefail
cd "$(dirname "$0")/.."
PAT='std::time::Instant|std::time::SystemTime|std::thread|std::fs::'

# shipped code only: a file stops at its `#[cfg(test…)] mod`.
shipped() {
  awk -v pat="$PAT" '
    FNR == 1 { skip = 0; prev = "" }
    prev ~ /#\[cfg\((all\()?test[,)]/ && $0 ~ /^[[:space:]]*mod / { skip = 1 }
    !skip && $0 ~ pat { print FILENAME ":" FNR ":" $0 }
    { prev = $0 }
  ' "$@"
}

hits=$(shipped crates/posara/src/host.rs crates/posara/src/backend/clock.rs $(find crates/posara/src/plugins crates/posara-core/src -name '*.rs') \
  | grep -vE 'plugins/gfx/png.rs|plugins/sfx/record.rs|plugins/midi/')

if [ -n "$hits" ]; then
  echo "portable oracle: LEAK"
  echo "$hits"
  exit 1
fi
echo "portable oracle: clean"
