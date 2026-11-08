#!/usr/bin/env bash
# Frame-time benchmark: run one scenario muted and offscreen for a fixed time
# with vsync off, and print its PERF SUMMARY (see src/perf.rs).
#
#   tools/perf_bench.sh <label> <seconds> [extra env assignments...]
#
#   tools/perf_bench.sh starter-idle 40 AFTERLIGHT_MOVETEST=1
#   tools/perf_bench.sh capital-idle 40 AFTERLIGHT_MOVETEST=1 AFTERLIGHT_DESIGN=designs/factions/eternal_hegemony/citadel.json
#
# Logs go to $AFTERLIGHT_PERF_DIR (default /tmp/afterlight_perf)/<label>.log.
# Only the pid this script starts is ever killed.
set -uo pipefail
LABEL="$1"; SECS="$2"; shift 2
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${AFTERLIGHT_PERF_DIR:-/tmp/afterlight_perf}"
mkdir -p "$OUT"
LOG="$OUT/$LABEL.log"
cd "$ROOT"
# Other sessions build on this machine too, and a rustc running beside the
# game halves its frame rate. Wait (up to 15 min) for the CPU to be ours.
for _ in $(seq 1 180); do pgrep -x rustc >/dev/null || break; sleep 5; done
pgrep -x rustc >/dev/null && echo "[perf] WARNING: another build is still running; numbers will be noisy"
cargo build 2>"$OUT/$LABEL.build.log" || { echo "[perf] BUILD FAILED"; grep -E "^error" -A8 "$OUT/$LABEL.build.log" | head -30; exit 2; }
pgrep -x rustc >/dev/null && echo "[perf] WARNING: a build started alongside; numbers will be noisy"
caffeinate -i env AFTERLIGHT_MUTE=1 AFTERLIGHT_SKIP_MENU=1 AFTERLIGHT_OFFSCREEN=1 AFTERLIGHT_PERF=1 AFTERLIGHT_PERF_QUIT="$SECS" "$@" \
  cargo run >"$LOG" 2>&1 &
PID=$!
descendants() { local c; for c in $(pgrep -P "$1"); do echo "$c"; descendants "$c"; done; }
# PERF_SAMPLE_AT=<s> (set for this script, not the game): at s seconds into
# the run, profile the game with macOS `sample` for PERF_SAMPLE_SECS (8) and
# rank its functions with tools/perf_sample_top.py. No rebuild needed.
SAMPLE_AT="${PERF_SAMPLE_AT:-}"
SAMPLED=""
STARTED=$(date +%s)
# The game exits itself at SECS; give it a margin for boot, then stop it.
for _ in $(seq 1 $((SECS + 90))); do
  kill -0 "$PID" 2>/dev/null || break
  if [ -n "$SAMPLE_AT" ] && [ -z "$SAMPLED" ] && grep -q "PERF t=$SAMPLE_AT " "$LOG" 2>/dev/null; then
    for p in "$PID" $(descendants "$PID"); do
      case "$(ps -o command= -p "$p" 2>/dev/null)" in
        *target/debug/afterlight*) sample "$p" "${PERF_SAMPLE_SECS:-8}" -mayDie -file "$OUT/$LABEL.sample.txt" >/dev/null 2>&1; SAMPLED=1; break ;;
      esac
    done
  fi
  sleep 1
done
if kill -0 "$PID" 2>/dev/null; then
  echo "[perf] had to stop the run"
  for p in $(descendants "$PID"); do kill "$p" 2>/dev/null; done
  kill "$PID" 2>/dev/null
fi
grep -q "panicked" "$LOG" && { echo "[perf] PANIC"; grep -A5 "panicked" "$LOG" | head -12; }
python3 -I "$ROOT/tools/perf_summary.py" "$LOG"
[ -f "$OUT/$LABEL.sample.txt" ] && python3 -I "$ROOT/tools/perf_sample_top.py" "$OUT/$LABEL.sample.txt"
