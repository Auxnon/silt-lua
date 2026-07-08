#!/usr/bin/env bash
# Compare silt-lua against PUC-Lua (the reference C implementation) on the benchmark scripts.
#
# This is a WALL-CLOCK, process-level comparison (it includes interpreter startup). For
# fine-grained, in-process numbers use `cargo bench` (Criterion) instead — see PLAN.md §8.
#
# Usage:   benchmarks/compare.sh
# Requires: a `lua` (or `lua5.4`/`luajit`) on PATH for the reference side. If none is found,
#           only the silt-lua column is printed.
#
# This script is NOT run by CI. It is a manual tool.
set -euo pipefail

cd "$(dirname "$0")/.."

REPS="${REPS:-3}"   # take the best of N runs to reduce noise

echo "Building silt (release)…"
cargo build --release --bin silt >/dev/null 2>&1
SILT="./target/release/silt"

# Pick a reference Lua if available.
REF=""
for cand in lua lua5.4 lua5.3 luajit; do
    if command -v "$cand" >/dev/null 2>&1; then REF="$cand"; break; fi
done
if [ -z "$REF" ]; then
    echo "WARNING: no reference 'lua' found on PATH — printing silt-only timings."
fi

# best-of-N wall clock (seconds) for a command, using the shell's built-in time via /usr/bin/time
best_time() {
    local best="" t
    for _ in $(seq 1 "$REPS"); do
        # %e = elapsed wall seconds (GNU/BSD /usr/bin/time both support -p style differently;
        # fall back to bash `time` if /usr/bin/time is unavailable)
        if command -v /usr/bin/time >/dev/null 2>&1; then
            t=$({ /usr/bin/time -p "$@" >/dev/null; } 2>&1 | awk '/^real/{print $2}')
        else
            t=$({ time "$@" >/dev/null; } 2>&1 | awk '/real/{print $2}')
        fi
        if [ -z "$best" ] || awk "BEGIN{exit !($t < $best)}"; then best="$t"; fi
    done
    echo "$best"
}

printf "%-22s %12s %12s %10s\n" "benchmark" "silt (s)" "${REF:-lua} (s)" "ratio"
printf -- "------------------------------------------------------------\n"
for f in benchmarks/*.lua; do
    name=$(basename "$f" .lua)
    s=$(best_time "$SILT" "$f")
    if [ -n "$REF" ]; then
        r=$(best_time "$REF" "$f")
        ratio=$(awk "BEGIN{ if ($r>0) printf \"%.2fx\", $s/$r; else print \"-\" }")
        printf "%-22s %12s %12s %10s\n" "$name" "$s" "$r" "$ratio"
    else
        printf "%-22s %12s %12s %10s\n" "$name" "$s" "-" "-"
    fi
done
