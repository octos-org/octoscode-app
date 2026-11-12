#!/usr/bin/env bash
# Check out OctoSense, the optional host of the octoscode module, at the pinned
# upstream revision `e58ea6a1765d626a6c2e1e995d2fed30a992a91d`, unpatched.
#
# OctoSense main links the octoscode module itself: its workspace depends on
# `octoscode-module` from this repository by git (the shell's `app-octoscode`
# feature) and points Octoscript-Makepad, makepad and octos at its own
# `.sources/`. tools/build-macos.sh --octosense redirects that one git
# dependency to this checkout's crates/octoscode-module with a cargo
# `--config` [patch], so no file of the OctoSense tree is edited; cargo only
# refreshes its Cargo.lock for the redirected crate.
#
# IDEMPOTENT: a checkout already at the pin is left as it is (.sources,
# target and the refreshed Cargo.lock included). Any other checkout is moved
# to the pin; local changes other than Cargo.lock are refused unless --force.
#
# Usage:
#   tools/prepare-octosense.sh [--force] [target-dir]
#       target-dir: default <repo>/.forks/octosense
set -euo pipefail

FORCE=0
if [ "${1:-}" = "--force" ]; then
  FORCE=1
  shift
fi
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
TARGET="${1:-$REPO/.forks/octosense}"
PIN="e58ea6a1765d626a6c2e1e995d2fed30a992a91d"
REMOTE="https://github.com/OctoSense-org/OctoSense"

command -v git >/dev/null || { echo "prepare-octosense: git not found" >&2; exit 1; }

if [ -d "$TARGET/.git" ]; then
  cd "$TARGET"
  if [ "$(git rev-parse HEAD)" = "$PIN" ]; then
    echo "prepare-octosense: $TARGET already at $PIN (no-op)"
    exit 0
  fi
  if [ -n "$(git status --porcelain --untracked-files=no -- . ':(exclude)Cargo.lock')" ] && [ "$FORCE" = 0 ]; then
    echo "prepare-octosense: $TARGET has local changes and is not at $PIN;" >&2
    echo "  moving it would discard them. Commit or move them, or re-run with --force." >&2
    exit 1
  fi
  echo "prepare-octosense: moving $TARGET to $PIN"
  git fetch --quiet origin "$PIN"
else
  echo "prepare-octosense: cloning $PIN into $TARGET"
  mkdir -p "$(dirname "$TARGET")"
  git clone --quiet --no-checkout "$REMOTE" "$TARGET"
  cd "$TARGET"
  git fetch --quiet origin "$PIN"
fi
git checkout --quiet --force --detach "$PIN"
echo "prepare-octosense: $TARGET is at $PIN"
