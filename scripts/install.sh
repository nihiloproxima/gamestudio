#!/usr/bin/env bash
# Installs Gamestudio in /Applications, from the latest GitHub release (default) or from this checkout (--source),
# then puts the `studio` CLI on PATH and the Claude Code skill in ~/.claude/skills. Both point inside the app, so its
# own updates carry them along. Safe while agents work: they live in Gamestudio's tmux server, not in the app.
#
#   curl -fsSL https://raw.githubusercontent.com/nihiloproxima/gamestudio/main/scripts/install.sh | bash
#   pnpm app:install          # from a checkout: builds it locally (Node, pnpm and Rust needed)
set -euo pipefail
REPO="${GAMESTUDIO_REPO:-nihiloproxima/gamestudio}"
DEST=/Applications/Gamestudio.app
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if [ "${1:-}" = "--source" ]; then
  cd "$(dirname "$0")/.."
  [ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
  pnpm install --frozen-lockfile
  pnpm tauri build --bundles app
  ditto src-tauri/target/release/bundle/macos/Gamestudio.app "$tmp/Gamestudio.app"
else
  command -v gh >/dev/null || { echo "Gamestudio a besoin de gh (https://cli.github.com), connecté avec gh auth login."; exit 1; }
  gh release download --repo "$REPO" --pattern '*.app.tar.gz' --dir "$tmp"
  tar -xzf "$tmp"/*.app.tar.gz -C "$tmp"
fi

# Quit the app if it runs; the CLI shares its binary, so match the app's own process only.
if pgrep -f "$DEST/Contents/MacOS/gamestudio\$" >/dev/null; then
  osascript -e 'quit app "Gamestudio"' >/dev/null 2>&1 || true
  for _ in $(seq 40); do pgrep -f "$DEST/Contents/MacOS/gamestudio\$" >/dev/null || break; sleep 0.25; done
fi

rm -rf "$DEST"
ditto "$tmp/Gamestudio.app" "$DEST"
xattr -dr com.apple.quarantine "$DEST" 2>/dev/null || true

mkdir -p "$HOME/.local/bin"
ln -sf "$DEST/Contents/MacOS/gamestudio" "$HOME/.local/bin/studio"
if [ -d "$HOME/.claude" ]; then
  mkdir -p "$HOME/.claude/skills"
  ln -sfn "$DEST/Contents/Resources/skills/studio" "$HOME/.claude/skills/studio"
fi
case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *) echo "Ajoute ~/.local/bin à ton PATH pour la commande studio." ;; esac

open "$DEST"
echo "Gamestudio $(plutil -extract CFBundleShortVersionString raw "$DEST/Contents/Info.plist") installée dans $DEST"
