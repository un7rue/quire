#!/bin/sh
# Sets Quire's version and tags it. Cargo.toml holds the one version; tauri.conf.json,
# the sign-in screen and `quire-server --version` all read it from there.
#   dev/version.sh            prints the current version
#   dev/version.sh 0.2.0      sets it, updates Cargo.lock, commits and tags v0.2.0
# Versions are MAJOR.MINOR.PATCH: PATCH for fixes, MINOR for new features, MAJOR when saved
# templates, reports or the server's data folder need converting. Write the RELEASES.md
# entry ("## 0.2.0 — 5 November 2026") before running it.
set -e
cd "$(dirname "$0")/.."
cur=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
[ -z "$1" ] && { echo "$cur"; exit 0; }
new=$1
echo "$new" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$' || { echo "version must look like 1.2.3" >&2; exit 2; }
grep -q "^## $new " RELEASES.md || { echo "RELEASES.md has no \"## $new — …\" entry yet" >&2; exit 2; }
git diff --quiet && git diff --cached --quiet || { echo "commit or stash your changes first" >&2; exit 2; }
if [ "$new" != "$cur" ]; then
  sed -i.bak "1,/^version = /s/^version = \".*\"/version = \"$new\"/" Cargo.toml && rm Cargo.toml.bak
  cargo update -p quire --offline -q 2>/dev/null || cargo update -p quire -q
  git commit -qam "Quire $new"
fi
git tag -a "v$new" -m "Quire $new"
echo "Quire $cur → $new, tagged v$new. Push with: git push && git push origin v$new"
