#!/usr/bin/env bash
# Installs Studio in /Applications, from the latest GitHub release (default) or from this checkout (--source), then
# puts the `studio` CLI on PATH and the Claude Code skill in ~/.claude/skills. Both point inside the app, so its own
# updates carry them along. Safe while agents work: they live in Studio's tmux server, not in the app.
#
#   curl -fsSL https://raw.githubusercontent.com/nihiloproxima/gamestudio/main/scripts/install.sh | bash
#   pnpm app:install          # from a checkout: builds it locally (Node, pnpm and Rust needed)
set -euo pipefail
REPO="${STUDIO_REPO:-nihiloproxima/gamestudio}"
DEST=/Applications/Studio.app
# The app's former name: quit and removed, its data moves on Studio's first start.
LEGACY=/Applications/Gamestudio.app
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if [ "${1:-}" = "--source" ]; then
  cd "$(dirname "$0")/.."
  [ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
  pnpm install --frozen-lockfile
  pnpm tauri build --bundles app
  ditto src-tauri/target/release/bundle/macos/Studio.app "$tmp/Studio.app"
else
  command -v gh >/dev/null || { echo "Studio a besoin de gh (https://cli.github.com), connecté avec gh auth login."; exit 1; }
  gh release download --repo "$REPO" --pattern '*.app.tar.gz' --dir "$tmp/dl"
  tar -xzf "$tmp"/dl/*.app.tar.gz -C "$tmp/dl"
  ditto "$(ls -d "$tmp"/dl/*.app | head -1)" "$tmp/Studio.app"
fi

# Quit the app if it runs; the CLI shares its binary, so match the app's own process only.
quit() {
  local app="$1" exe="$2"
  if pgrep -f "$app/Contents/MacOS/$exe\$" >/dev/null; then
    osascript -e "quit app \"$(basename "$app" .app)\"" >/dev/null 2>&1 || true
    for _ in $(seq 40); do pgrep -f "$app/Contents/MacOS/$exe\$" >/dev/null || break; sleep 0.25; done
  fi
}
quit "$DEST" studio
quit "$LEGACY" gamestudio
rm -rf "$LEGACY"

rm -rf "$DEST"
ditto "$tmp/Studio.app" "$DEST"
xattr -dr com.apple.quarantine "$DEST" 2>/dev/null || true

mkdir -p "$HOME/.local/bin"
ln -sf "$DEST/Contents/MacOS/studio" "$HOME/.local/bin/studio"
if [ -d "$HOME/.claude" ]; then
  mkdir -p "$HOME/.claude/skills"
  ln -sfn "$DEST/Contents/Resources/skills/studio" "$HOME/.claude/skills/studio"
fi
case ":$PATH:" in *":$HOME/.local/bin:"*) ;; *) echo "Ajoute ~/.local/bin à ton PATH pour la commande studio." ;; esac

open "$DEST"
echo "Studio $(plutil -extract CFBundleShortVersionString raw "$DEST/Contents/Info.plist") installé dans $DEST"
