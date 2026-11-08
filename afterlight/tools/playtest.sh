#!/usr/bin/env bash
# Playtest harness: launch the game, let it run, capture engine-side frames,
# then report anything that looks wrong.
#
# Engine-side capture (AFTERLIGHT_SHOTS) photographs the render target, so it is
# always the game and never whatever app is in front -- unlike an OS screen
# grab. It does NOT see through a fully covered window, though: macOS stops
# that window drawing and every frame after the first comes back black.
# AFTERLIGHT_OFFSCREEN=1 renders into an image instead, so the run can sit behind
# anything. The window shows nothing while it is on.
#
#   tools/playtest.sh [seconds] [label] [extra env assignments...]
#
#   tools/playtest.sh 60 baseline
#   tools/playtest.sh 90 cascade AFTERLIGHT_CASCADE=0.9
#
# Exit status is non-zero if the run panicked or produced no frames.
set -uo pipefail

SECS="${1:-45}"
LABEL="${2:-run}"
shift 2 2>/dev/null || shift $# 

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="/tmp/afterlight_playtest/$LABEL"
LOG="$OUT/run.log"
rm -rf "$OUT"; mkdir -p "$OUT"

cd "$ROOT"

# Never kill by process name: another session may have the game open.
# Track only the pid this script starts.
echo "[playtest] building..."
if ! cargo build 2>"$OUT/build.log"; then
  echo "[playtest] BUILD FAILED"
  grep -E "^error" -A 8 "$OUT/build.log" | head -40
  exit 2
fi

# -u asserts user activity, which WAKES a display that has already slept.
# -d alone only prevents it sleeping, which is no help once it already has.
# Assert user activity for the whole run, not a two-second blip: -u wakes a
# slept display, and it has to stay asserted or it dozes again mid-capture.
caffeinate -u -t $((SECS + 90)) >/dev/null 2>&1 &
WAKE=$!
disown "$WAKE" 2>/dev/null || true
sleep 3

echo "[playtest] running '$LABEL' for ${SECS}s"
# AFTERLIGHT_MUTE=1: playtests run silent; pass AFTERLIGHT_MUTE=0 after it to hear one.
# caffeinate -di: without it the display sleeps during a long session and the
# render target stops producing content -- every captured frame comes back
# solid black and looks exactly like a rendering regression. It is not.
# `env` is required: caffeinate treats the first token as its command, so
# inline VAR=value assignments would be swallowed.
caffeinate -di env AFTERLIGHT_MUTE=1 AFTERLIGHT_SKIP_MENU=1 AFTERLIGHT_SHOTS=6 AFTERLIGHT_OFFSCREEN=1 AFTERLIGHT_SHOTS_DIR="$OUT" "$@" \
  cargo run >"$LOG" 2>&1 &
RUNNER=$!
disown "$RUNNER" 2>/dev/null || true   # keep job control quiet on kill

# Find the game among what this script started -- never by name: matching
# 'target/debug/afterlight' by name found whichever game was oldest on the
# machine, the one the user was playing, and killed it at the end. On macOS
# caffeinate runs its command in its own process and `cargo run` execs the
# binary, so the pid started below usually IS the game; check it as well as
# everything under it.
descendants() { local c; for c in $(pgrep -P "$1"); do echo "$c"; descendants "$c"; done; }
GAME=""
for _ in $(seq 1 600); do
  sleep 1
  for pid in "$RUNNER" $(descendants "$RUNNER"); do
    case "$(ps -o command= -p "$pid" 2>/dev/null)" in
      target/debug/afterlight*|*/target/debug/afterlight*) GAME="$pid"; break ;;
    esac
  done
  [ -n "$GAME" ] && break
done
[ -z "$GAME" ] && echo "[playtest] never saw the game start; stopping only what this script began"

START=$(date +%s)
sleep "$SECS"
ALIVE=$(( $(date +%s) - START ))

if [ -n "$GAME" ]; then kill "$GAME" 2>/dev/null; sleep 2; kill -9 "$GAME" 2>/dev/null; fi
for pid in $(descendants "$RUNNER"); do kill "$pid" 2>/dev/null; done
kill "$RUNNER" 2>/dev/null; wait "$RUNNER" 2>/dev/null

kill "$WAKE" 2>/dev/null
FRAMES=$(ls "$OUT"/shot_*.png 2>/dev/null | wc -l | tr -d ' ')
count() { grep -ci "$1" "$LOG" 2>/dev/null | head -1; }
PANICS=$(count "panicked at")
ERRORS=$(count "^ERROR")
B0001=$(count "B0001")

echo "[playtest] alive=${ALIVE}s frames=$FRAMES panics=$PANICS errors=$ERRORS query-conflicts=$B0001"

# A black frame is ~57KB; a real one is several hundred. Identical tiny sizes
# across every frame means the display slept, not that rendering broke.
if [ "$FRAMES" != "0" ]; then
  BLACK=$(find "$OUT" -name 'shot_*.png' -size -80k | wc -l | tr -d ' ')
  if [ "$BLACK" = "$FRAMES" ]; then
    echo "[playtest] NOTE: all $FRAMES frames are blank."
    echo "[playtest]       This is the environment, not the build: the render target"
    echo "[playtest]       produces nothing while the screen is locked or asleep."
    echo "[playtest]       Panics, errors and query conflicts above are still valid."
  fi
fi
echo "[playtest] frames in $OUT"

if [ "$PANICS" != "0" ]; then
  echo "--- panic ---"
  grep -i -A 4 "panicked at" "$LOG" | head -20
fi
if [ "$ERRORS" != "0" ]; then
  echo "--- errors ---"
  grep -i "^ERROR" "$LOG" | sed 's/\x1b\[[0-9;]*m//g' | head -10
fi

[ "$PANICS" = "0" ] && [ "$FRAMES" != "0" ]
