#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CHANGELOG_FILE="$ROOT_DIR/specs/changelog.md"
CARGO_FILE="$ROOT_DIR/Cargo.toml"
MODE=sync
FORCE=0

for arg in "$@"; do
  case "$arg" in
    --sync) MODE=sync ;;
    --check) MODE=check ;;
    --check-new) MODE=check-new ;;
    --force) FORCE=1 ;;
    *) echo "Unknown argument: $arg"; exit 1 ;;
  esac
done

CHANGELOG_VERSION=$(sed -nE 's/^## v?([0-9]+\.[0-9]+\.[0-9]+)$/\1/p' "$CHANGELOG_FILE" | head -n 1)
CARGO_VERSION=$(awk -F '=' '/^\[package\]/{in_pkg=1} /^\[/ && !/^\[package\]/{in_pkg=0} in_pkg && $1 ~ /^version[[:space:]]*$/ {gsub(/["[:space:]]/, "", $2); print $2; exit}' "$CARGO_FILE")

if [[ -z "$CHANGELOG_VERSION" || -z "$CARGO_VERSION" ]]; then
  echo "Could not read a stable version from changelog.md and Cargo.toml." >&2
  exit 1
fi

if [[ "$MODE" == check ]]; then
  [[ "$CHANGELOG_VERSION" == "$CARGO_VERSION" ]] || { echo "Version mismatch: changelog $CHANGELOG_VERSION, Cargo.toml $CARGO_VERSION" >&2; exit 1; }
  exit 0
fi

if [[ "$MODE" == check-new ]]; then
  [[ "$CHANGELOG_VERSION" != "$CARGO_VERSION" ]] && [[ "$(printf '%s\n' "$CARGO_VERSION" "$CHANGELOG_VERSION" | sort -V | tail -n1)" == "$CHANGELOG_VERSION" ]] || {
    echo "Changelog must declare a version newer than Cargo.toml ($CARGO_VERSION); found $CHANGELOG_VERSION." >&2
    exit 1
  }
  exit 0
fi

if [[ "$FORCE" -eq 0 ]]; then
  CURRENT_BRANCH=$(git -C "$ROOT_DIR" branch --show-current 2>/dev/null || true)
  [[ "$CURRENT_BRANCH" == develop ]] || exit 0
fi

[[ "$CHANGELOG_VERSION" == "$CARGO_VERSION" ]] && exit 0

awk -v new_ver="$CHANGELOG_VERSION" '
  /^\[package\]/ { in_pkg = 1 }
  /^\[/ && !/^\[package\]/ { in_pkg = 0 }
  in_pkg && /^version[[:space:]]*=/ && !replaced {
    sub(/version[[:space:]]*=[[:space:]]*"[^"]+"/, "version = \"" new_ver "\"")
    replaced = 1
  }
  { print }
' "$CARGO_FILE" > "$CARGO_FILE.tmp"
mv "$CARGO_FILE.tmp" "$CARGO_FILE"
(cd "$ROOT_DIR" && cargo check --quiet)
git -C "$ROOT_DIR" add "$CARGO_FILE" "$ROOT_DIR/Cargo.lock"
echo "Synchronized release version to $CHANGELOG_VERSION."
