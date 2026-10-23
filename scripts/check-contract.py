#!/usr/bin/env python3
"""Check the native snapshots and existing cart output oracles."""
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    baseline = json.loads((ROOT / "crates/posara/tests/contracts/baseline.json").read_text())
    changed = []
    for name, expected in baseline["fixtures"].items():
        path = ROOT / name
        actual = hashlib.sha256(path.read_bytes()).hexdigest() if path.is_file() else None
        if actual != expected:
            changed.append(name)
    if changed:
        print("Contract oracle changed:\n" + "\n".join(changed), file=sys.stderr)
        return 1
    print("Native snapshots and cart output oracles unchanged.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
