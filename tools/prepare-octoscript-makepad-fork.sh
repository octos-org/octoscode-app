#!/usr/bin/env bash
# Recreate the Octoscript-Makepad fork from the pinned rev + our renderer patch
# series (card #21b step 1).
#
# The fork is a local checkout of `OctoSense-org/Octoscript-Makepad` at the rev
# the OctoSense workspace pins (`6881fb6`, see the root `Cargo.toml`) with one
# commit on top: the flow/fill lowering the #16/#18 components are authored
# against (`patches/octoscript-makepad/000{1..6}-*.patch`). Without it a filled
# node lowers to a zero extent and a flowing child is pinned at its measured
# `abs_pos`.
#
# This script is IDEMPOTENT: run twice, the second run is a no-op.
#
# Usage:
#   tools/prepare-octoscript-makepad-fork.sh [target-dir]   # default ~/home/oa.noindex/octoscript-makepad-fork
#
# Exit: 0 when the tree equals the fork branch (patches applied, clean); 1 on error.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
TARGET="${1:-/Users/yuechen/home/oa.noindex/octoscript-makepad-fork}"
PIN="6881fb6c3c3220e407633b0ba5211c3d42c7e625"
BRANCH="feat/renderer-flow-fill"
REMOTE="https://github.com/OctoSense-org/Octoscript-Makepad.git"
PATCHES=(
  "$REPO/patches/octoscript-makepad/0001-responsive.patch"
  "$REPO/patches/octoscript-makepad/0002-inline-code.patch"
  "$REPO/patches/octoscript-makepad/0003-16c.patch"
  "$REPO/patches/octoscript-makepad/0004-16d.patch"
  "$REPO/patches/octoscript-makepad/0005-16e.patch"
  "$REPO/patches/octoscript-makepad/0006-18e-padright.patch"
  # Card #21b: `to_makepad_ui_in_slot` — lower a component parent-relative when
  # it is mounted into a slot rather than at the window origin.
  "$REPO/patches/octoscript-makepad/0007-in-slot.patch"
)

command -v git >/dev/null || { echo "prepare-octoscript-makepad-fork: git not found" >&2; exit 1; }
for p in "${PATCHES[@]}"; do
  [ -f "$p" ] || { echo "prepare-octoscript-makepad-fork: missing patch $p" >&2; exit 1; }
done

if [ -d "$TARGET/.git" ]; then
  cur_pin="$(git -C "$TARGET" rev-parse HEAD~1 2>/dev/null || true)"
  dirty="$(git -C "$TARGET" status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$cur_pin" = "$PIN" ] && [ "$dirty" = "0" ] \
     && git -C "$TARGET" log --oneline -1 --format=%s | grep -q "flow/fill lowering"; then
    echo "prepare-octoscript-makepad-fork: $TARGET already at $PIN + patches (no-op)"
    exit 0
  fi
  echo "prepare-octoscript-makepad-fork: reusing existing checkout at $TARGET"
  cd "$TARGET"
  git fetch --quiet origin "$PIN" 2>/dev/null || true
else
  echo "prepare-octoscript-makepad-fork: cloning $PIN into $TARGET"
  mkdir -p "$(dirname "$TARGET")"
  git clone --quiet --no-checkout "$REMOTE" "$TARGET"
  cd "$TARGET"
  git fetch --quiet origin "$PIN"
fi

git checkout --quiet --detach "$PIN"
git checkout --quiet -B "$BRANCH"
git reset --quiet --hard "$PIN"
git clean --quiet -fd -e .sources -e target -e .cargo-ok

for p in "${PATCHES[@]}"; do git apply --index "$p"; done
git -c user.name=octos -c user.email=octos@local commit --quiet -m \
  "feat(renderer): flow/fill lowering (responsive, inline-code, 16c/16d/16e, 18e padright)"

echo "prepare-octoscript-makepad-fork: $TARGET is at $BRANCH ($(git rev-parse --short HEAD)) = $PIN + $((${#PATCHES[@]})) patches"
