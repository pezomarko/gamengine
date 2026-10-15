#!/usr/bin/env bash
# Netcode gate (PROTOCOL.md 9, PLAN.md 11.8 Phase 2): 16 bots under turmoil at 150 ms RTT and
# 3% loss must stay under budgets.toml [net] max_bytes_per_player_s in both directions and meet
# the playability assertions. Runs in simulated time (a few seconds of wall clock).
set -euo pipefail
cd "$(dirname "$0")/.."
for m in test_room arena; do
  if [ ! -f assets/maps/built/$m.bsp ]; then
    echo "check-netcode: assets/maps/built/$m.bsp missing; build it with gm-tools map build" >&2
    exit 1
  fi
done
cargo test -q -p gm-server --locked --test netcode --test loopback -- --nocapture "$@"
# The counter-pick is a statistical claim (sixteen duelists, 120 s, one seed): its own comment
# measures the share under the bar about once in forty runs, and GitHub's two-core runners
# sit nearer the bar than the box. Three attempts: a real regression fails all three.
for attempt in 1 2 3; do
  if cargo test -q -p gm-server --locked --test counterpick -- --nocapture "$@"; then exit 0; fi
  echo "check-netcode: the counter-pick missed its bar (attempt $attempt of 3)"
done
exit 1
