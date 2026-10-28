#!/usr/bin/env bash
# Regenerates the README screenshots from the browser demo (fictional games, no personal data), at 2x.
# Usage: scripts/screenshots.sh   (needs Google Chrome; CHROME=/path/to/chrome to use another one)
set -euo pipefail
cd "$(dirname "$0")/.."
CHROME="${CHROME:-/Applications/Google Chrome.app/Contents/MacOS/Google Chrome}"
OUT=docs/screenshots
PORT=1430
mkdir -p "$OUT"
profile="$(mktemp -d)"
pnpm exec vite --port "$PORT" --strictPort >/dev/null 2>&1 &
vite=$!
trap 'kill "$vite" 2>/dev/null; rm -rf "$profile"' EXIT
for _ in $(seq 60); do curl -sf "http://localhost:$PORT" >/dev/null && break; sleep 0.5; done

# shot <file> <query>: one screen of the demo, opened through its browser-mode deep link (state/store.ts openFromUrl).
shot() {
  "$CHROME" --headless=new --hide-scrollbars --window-size=1440,900 --force-device-scale-factor=2 \
    --virtual-time-budget=6000 --user-data-dir="$profile" --screenshot="$OUT/$1.png" \
    "http://localhost:$PORT/?$2" >/dev/null 2>&1
  echo "$OUT/$1.png"
}

shot overview "view=overview"
shot roadmap "view=roadmap"
shot tasks "view=tasks&item=FRO-1"
shot backlog "view=backlog"
shot agents "view=agents&session=fro-1-claude"
shot assets "view=assets"
shot palette "view=tasks&palette=1"
