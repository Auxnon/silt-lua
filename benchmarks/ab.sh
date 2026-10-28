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
#          COUNT=1        count user-space instructions instead of timing: build the
#                         `silt` CLI on both sides and run each bench script through
#                         `perf stat`. Counts barely move with machine load, so this
#                         still works while other jobs run. FILTER is a regex here.
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

# Each side builds into its own target dir. Cargo hashes a path package by its
# place in the workspace, so the base worktree and the working tree compile to
# the same unit; in one shared dir the second build looks fresh and reuses the
# first side's binary.
bench_bin() { # build the bench in $1 into target dir $2, copy it to $3
    (cd "$1" && CARGO_TARGET_DIR="$2" cargo bench -q --bench interpreter \
        --features "$FEATURES" --no-run)
    cp "$(ls -t "$2"/release/deps/interpreter-* | grep -v '\.d$' | head -1)" "$3"
}

silt_bin() { # build the CLI in $1 into target dir $2, copy it to $3
    (cd "$1" && CARGO_TARGET_DIR="$2" cargo build -q --release -p silt-cli \
        --no-default-features --features "$FEATURES")
    cp "$2/release/silt" "$3"
}

echo "Building A ($BASE)..."
WT="$OUT/base"
git worktree remove --force "$WT" 2>/dev/null || true
git worktree add --detach --quiet "$WT" "$BASE"
trap 'git worktree remove --force "$WT" 2>/dev/null || true' EXIT
if [[ "${COUNT:-}" == 1 ]]; then BUILD=silt_bin A="$OUT/silt-A" B="$OUT/silt-B"
else BUILD=bench_bin A="$OUT/A" B="$OUT/B"; fi
"$BUILD" "$WT" "$OUT/target-base" "$A"
echo "Building B (working tree)..."
"$BUILD" "$ROOT" "$ROOT/target" "$B"
if cmp -s "$A" "$B"; then
    echo "note: A and B are the same binary (no code change between them)"
fi

PIN=()
[[ -n "$CORE" ]] && command -v taskset >/dev/null && PIN=(taskset -c "$CORE")

if [[ "${COUNT:-}" == 1 ]]; then
    # both sides run the working tree's scripts, so an edited bench can't skew A
    W="$OUT/scripts"
    rm -rf "$W" && mkdir -p "$W"
    python3 -I - "$ROOT/benches/interpreter.rs" "$W" "$FILTER" <<'PY'
import re, sys
src, out, filt = open(sys.argv[1]).read(), sys.argv[2], sys.argv[3]
scripts = {m[1].lower(): m[2] for m in re.finditer(r'const (\w+): &str = r#"(.*?)"#;', src, re.S)}
for m in re.finditer(r'\(\s*"(\w+)",\s*"(\w+)",\s*r#"(.*?)"#,\s*\)', src, re.S):
    scripts[f"vec_{m[1]}_{m[2]}"] = m[3]
for name, body in scripts.items():
    # the userdata benches need types the CLI doesn't register
    if name.startswith("ud_") or (filt and not re.search(filt, name)):
        continue
    open(f"{out}/{name}.lua", "w").write(body)
PY
    printf '\n%-26s %14s %14s %8s\n' bench 'A instr' 'B instr' 'B vs A'
    for f in "$W"/*.lua; do
        for side in A B; do
            best=""
            for _ in $(seq "$ROUNDS"); do
                "${PIN[@]}" perf stat -x, -e instructions:u -o "$OUT/stat" \
                    "$OUT/silt-$side" "$f" >/dev/null 2>&1
                n=$(grep -m1 instructions "$OUT/stat" | cut -d, -f1)
                [[ -z "$best" || "$n" -lt "$best" ]] && best=$n
            done
            declare "n_$side=$best"
        done
        python3 -I -c 'import sys; a, b = int(sys.argv[2]), int(sys.argv[3])
print(f"{sys.argv[1]:26} {a:14,} {b:14,} {(b / a - 1) * 100:+7.1f}%")' \
            "$(basename "$f" .lua)" "$n_A" "$n_B"
    done
    exit 0
fi

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
