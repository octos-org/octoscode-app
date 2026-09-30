#!/usr/bin/env bash
# Recreate the OctoSense fork from the pinned rev + our transport patch.
#
# The fork is a local checkout of `OctoSense-org/OctoSense` at the pinned rev
# `6e9bfd4077cf8181878ac005ad586f908084a4bf` with one commit on top: the
# generic `OutboundCommand::Request` (see `patches/octosense/0001-*.patch`).
# This script is IDEMPOTENT: run twice, the second run is a no-op.
#
# Usage:
#   tools/prepare-octosense-fork.sh [target-dir]     # default ../octosense-fork (or $OCTO_FORKS_DIR)
#
# Exit: 0 when the tree equals the fork branch (patch applied, clean);
#       1 on any error.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
TARGET="${1:-${OCTO_FORKS_DIR:-..}/octosense-fork}"
PIN="6e9bfd4077cf8181878ac005ad586f908084a4bf"
BRANCH="feat/transport-generic-request"
PATCH="$REPO/patches/octosense/0001-transport-generic-request.patch"
REMOTE="https://github.com/OctoSense-org/OctoSense"

command -v git >/dev/null || { echo "prepare-octosense-fork: git not found" >&2; exit 1; }
[ -f "$PATCH" ] || { echo "prepare-octosense-fork: missing patch $PATCH" >&2; exit 1; }

# Already prepared with the patch applied and nothing else pending?
if [ -d "$TARGET/.git" ]; then
  cur_pin="$(git -C "$TARGET" rev-parse HEAD~1 2>/dev/null || true)"
  dirty="$(git -C "$TARGET" status --porcelain 2>/dev/null | wc -l | tr -d ' ')"
  if [ "$cur_pin" = "$PIN" ] && [ "$dirty" = "0" ] \
     && git -C "$TARGET" log --oneline -1 --format=%s | grep -q "generic JSON-RPC Request"; then
    echo "prepare-octosense-fork: $TARGET already at $PIN + patch (no-op)"
    exit 0
  fi
  echo "prepare-octosense-fork: reusing existing checkout at $TARGET"
  cd "$TARGET"
  git fetch --quiet origin "$PIN" 2>/dev/null || true
else
  echo "prepare-octosense-fork: cloning $PIN into $TARGET"
  mkdir -p "$(dirname "$TARGET")"
  git clone --quiet --no-checkout "$REMOTE" "$TARGET"
  cd "$TARGET"
  git fetch --quiet origin "$PIN"
fi

# Recreate the branch from the pinned rev and re-apply the patch.
git checkout --quiet --detach "$PIN"
git checkout --quiet -B "$BRANCH"
git reset --quiet --hard "$PIN"
git clean --quiet -fd -e .sources -e target -e .cargo-ok

git apply --index "$PATCH"
SUBJECT="$(awk '/^Subject: /{sub(/^Subject: \[PATCH\] /,"");print;exit}' "$PATCH")"
AUTHOR="$(awk '/^From: /{sub(/^From: /,"");print;exit}' "$PATCH")"
git -c user.name=octos -c user.email=octos@local commit --quiet \
  --author "$AUTHOR" -m "$SUBJECT"

echo "prepare-octosense-fork: $TARGET is at $BRANCH ($(git rev-parse --short HEAD)) = $PIN + patch"
