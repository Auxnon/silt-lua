#!/usr/bin/env bash
# A/B the Criterion benches: a base revision (A) against the working tree (B).
#
# The dispatch loop is one large function, so an edit anywhere can move unrelated
# benchmarks by 5-20% through code generation, and back-to-back Criterion runs
# drift by a few percent on their own. This builds both bench binaries once, then
# alternates them (A B A B ...) and keeps each benchmark's best run, so drift hits
# both sides alike.
#
# Usage:   benchmarks/ab.sh [BASE_REV] [CRITERION_FILTER]
#          benchmarks/ab.sh HEAD 'vec_|native'
# Env:     ROUNDS=3       alternating rounds (each runs A and B once)
#          FEATURES=vector
#          CORE=5         pin to one core with taskset (empty to skip)
#          PERF=1         also print `perf stat` counters (instructions, branch and
#                         i-cache misses) for one run of each side; needs `perf` and
#                         kernel.perf_event_paranoid <= 1
#
# Not run by CI. A manual tool, like compare.sh.
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$PWD"
BASE="${1:-HEAD}"
FILTER="${2:-}"
ROUNDS="${ROUNDS:-3}"
FEATURES="${FEATURES:-vector}"
CORE="${CORE-5}"
OUT="$ROOT/target/ab"
mkdir -p "$OUT"

bench_bin() { # build the bench in $1, print the binary's path
    (cd "$1" && CARGO_TARGET_DIR="$ROOT/target" cargo bench --bench interpreter \
        --features "$FEATURES" --no-run 2>&1) |
        grep -oE 'target/release/deps/interpreter-[0-9a-f]+' | tail -1
}

echo "Building A ($BASE)..."
WT="$OUT/base"
git worktree remove --force "$WT" 2>/dev/null || true
git worktree add --detach --quiet "$WT" "$BASE"
trap 'git worktree remove --force "$WT" 2>/dev/null || true' EXIT
cp "$ROOT/$(bench_bin "$WT")" "$OUT/A"
echo "Building B (working tree)..."
cp "$ROOT/$(bench_bin "$ROOT")" "$OUT/B"

PIN=()
[[ -n "$CORE" ]] && command -v taskset >/dev/null && PIN=(taskset -c "$CORE")

RES="$OUT/results.txt"
: >"$RES"
for r in $(seq "$ROUNDS"); do
    for side in A B; do
        echo "round $r/$ROUNDS: $side"
        "${PIN[@]}" "$OUT/$side" --bench $FILTER --warm-up-time 1 --measurement-time 3 \
            --noplot --discard-baseline 2>/dev/null |
            python3 -I -c '
import re, sys
unit = {"ns": 1e-6, "µs": 1e-3, "us": 1e-3, "ms": 1.0, "s": 1e3}
text = sys.stdin.read()
for m in re.finditer(r"^(\S+)\s*\n?\s*time:\s*\[\S+ \S+ (\S+) (\S+)", text, re.M):
    print(sys.argv[1], m.group(1), float(m.group(2)) * unit[m.group(3)])
' "$side" >>"$RES"
    done
done

python3 -I - "$RES" <<'EOF'
import collections, sys
runs = collections.defaultdict(lambda: collections.defaultdict(list))
for line in open(sys.argv[1]):
    side, name, ms = line.split()
    runs[name][side].append(float(ms))
print(f"\n{'bench':26} {'A ms':>9} {'B ms':>9} {'B vs A':>8}  {'spread A/B':>11}")
for name, s in runs.items():
    a, b = min(s["A"]), min(s["B"])
    spread = lambda xs: (max(xs) - min(xs)) / min(xs) * 100
    print(f"{name:26} {a:9.2f} {b:9.2f} {(b / a - 1) * 100:+7.1f}%  {spread(s['A']):4.1f}/{spread(s['B']):4.1f}%")
EOF

if [[ "${PERF:-}" == 1 ]]; then
    for side in A B; do
        echo -e "\nperf stat, $side:"
        "${PIN[@]}" perf stat -e instructions,cycles,branch-misses,L1-icache-load-misses \
            "$OUT/$side" --bench $FILTER --warm-up-time 1 --measurement-time 3 \
            --noplot --discard-baseline 2>&1 >/dev/null | grep -E 'instructions|cycles|misses'
    done
fi
