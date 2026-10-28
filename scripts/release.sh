#!/usr/bin/env bash
# Cuts release X.Y.Z: bumps the version everywhere, commits, tags vX.Y.Z and pushes. The Release workflow then builds
# the app and publishes it; installed apps update themselves within a few hours (or from Settings right away).
# Usage: pnpm release 0.2.0
set -euo pipefail
cd "$(dirname "$0")/.."
version="${1:?Usage : pnpm release X.Y.Z}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "Version attendue au format X.Y.Z, pas $version"; exit 1; }
[ "$(git branch --show-current)" = main ] || { echo "Une release part de main."; exit 1; }
git diff --quiet && git diff --cached --quiet || { echo "Des changements ne sont pas commités."; exit 1; }
git fetch -q origin main
[ "$(git rev-parse HEAD)" = "$(git rev-parse origin/main)" ] || { echo "main n'est pas à jour avec origin/main."; exit 1; }

node -e '
  const fs = require("fs");
  for (const file of ["package.json", "src-tauri/tauri.conf.json"]) {
    const json = JSON.parse(fs.readFileSync(file, "utf8"));
    json.version = process.argv[1];
    fs.writeFileSync(file, JSON.stringify(json, null, 2) + "\n");
  }
' "$version"
# The [package] version is the first `version =` line of Cargo.toml.
sed -i '' "1,/^version = /s/^version = .*/version = \"$version\"/" src-tauri/Cargo.toml
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"
(cd src-tauri && cargo update -q --workspace)

git commit -qam "Gamestudio $version"
git tag "v$version"
git push -q origin main "v$version"
echo "v$version poussée. La release se construit : gh run watch --repo $(gh repo view --json nameWithOwner -q .nameWithOwner)"
