#!/usr/bin/env bash
# Builds Gamestudio and (re)installs it as /Applications/Gamestudio.app, with the `studio` CLI on PATH and the
# `studio` skill linked into ~/.claude/skills.
# Safe while agents work: they live in Gamestudio's tmux server, not in the app, and the new app reattaches to them.
# Usage: pnpm app:install   (after a git pull to update)
set -euo pipefail
cd "$(dirname "$0")/.."
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

APP=src-tauri/target/release/bundle/macos/Gamestudio.app
DEST=/Applications/Gamestudio.app

pnpm install --frozen-lockfile
pnpm tauri build --bundles app

# The app and its CLI share the binary; quitting the app only detaches its terminal views.
if pgrep -x gamestudio >/dev/null; then
  pkill -x gamestudio || true
  for _ in $(seq 40); do pgrep -x gamestudio >/dev/null || break; sleep 0.25; done
fi

rm -rf "$DEST"
ditto "$APP" "$DEST"
mkdir -p "$HOME/.local/bin" "$HOME/.claude/skills"
ln -sf "$DEST/Contents/MacOS/gamestudio" "$HOME/.local/bin/studio"
ln -sfn "$PWD/skills/studio" "$HOME/.claude/skills/studio"
open "$DEST"
echo "Gamestudio $(git describe --always --dirty 2>/dev/null || echo dev) installée dans $DEST"
