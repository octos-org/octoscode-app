#!/usr/bin/env bash
# Check out Octoscript-Makepad at the rev this workspace pins, unpatched, for
# the root Cargo.toml's [patch."https://github.com/OctoSense-org/Octoscript-Makepad.git"].
#
# The renderer needs no patch of ours: Octoscript-Makepad main has the
# measured-design lowering the components are authored against (#77). The
# checkout exists because makepad-widgets also takes `octoscript-node` from
# Octoscript-Makepad, at an older rev; redirecting every Octoscript-Makepad
# crate to this one checkout keeps ONE octoscript-node, as makepad's own
# workspace and OctoSense's `.sources` do.
#
# This script is IDEMPOTENT: run twice, the second run is a no-op.
#
# Usage:
#   tools/prepare-octoscript-makepad.sh [target-dir]   # default <repo>/.forks/octoscript-makepad (or $OCTO_FORKS_DIR)
#
# Exit: 0 when the checkout is at the pin and clean; 1 on error.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
TARGET="${1:-${OCTO_FORKS_DIR:-$REPO/.forks}/octoscript-makepad}"
PIN="aa80f72c509c767faf04822b939d44b2f34fbc81"
REMOTE="https://github.com/OctoSense-org/Octoscript-Makepad.git"

command -v git >/dev/null || { echo "prepare-octoscript-makepad: git not found" >&2; exit 1; }

if [ -e "$TARGET/.git" ]; then
  if [ "$(git -C "$TARGET" rev-parse HEAD 2>/dev/null)" = "$PIN" ] \
     && [ -z "$(git -C "$TARGET" status --porcelain 2>/dev/null)" ]; then
    echo "prepare-octoscript-makepad: $TARGET already at $PIN (no-op)"
    exit 0
  fi
  [ -z "$(git -C "$TARGET" status --porcelain)" ] \
    || { echo "prepare-octoscript-makepad: $TARGET has local changes; move them away first" >&2; exit 1; }
  echo "prepare-octoscript-makepad: moving $TARGET to $PIN"
  git -C "$TARGET" fetch --quiet origin "$PIN" 2>/dev/null || true
else
  echo "prepare-octoscript-makepad: cloning $PIN into $TARGET"
  mkdir -p "$(dirname "$TARGET")"
  git clone --quiet --no-checkout "$REMOTE" "$TARGET"
  git -C "$TARGET" fetch --quiet origin "$PIN"
fi
git -C "$TARGET" checkout --quiet --detach "$PIN"

echo "prepare-octoscript-makepad: $TARGET is at $PIN"
