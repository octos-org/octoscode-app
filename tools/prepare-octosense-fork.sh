#!/usr/bin/env bash
# Recreate the OctoSense fork from the pinned rev + our transport patches.
#
# The fork is a local checkout of `OctoSense-org/OctoSense` at the pinned rev
# `6e9bfd4077cf8181878ac005ad586f908084a4bf` with our patches on top, one
# commit each, in this order (`patches/octosense/`):
#   0001  the generic `OutboundCommand::Request`            (D10a)
#   0002  never drop a server reply: every event waits for room in the
#         transport's event channel                         (D10d)
#   0003  the shell hosts the octoscode module (`app-octoscode`): workspace
#         members apps/octoscode/{client,store,module} (vendored at build
#         time), the shell feature + linked_modules() push, the renderer
#         fork [patch] (../octoscript-makepad-fork), its Cargo.lock entries
#         (A33, D10f; tools/build-macos.sh --octosense builds it). The
#         repo's own workspace uses only the transport crates of this tree:
#         the extra members and [patch] of a path dependency's workspace are
#         never loaded by cargo, so 0003 changes nothing for it.
# This script is IDEMPOTENT: run twice, the second run is a no-op.
#
# - A tree already at the pin with the first k patches on top (k < all, e.g.
#   a fork prepared before 0002 existed) gets ONLY the missing patches on top;
#   nothing else in it is touched (other uncommitted work stays as it was).
# - Any other state is recreated from the pin (checkout, reset, clean). A tree
#   with uncommitted changes is then refused unless --force: they would be
#   lost.
#
# Usage:
#   tools/prepare-octosense-fork.sh [--force] [target-dir]
#       target-dir: default <repo>/.forks/octosense-fork (or $OCTO_FORKS_DIR)
#
# Exit: 0 when the tree is the pin + every patch (each one commit, in order);
#       1 on any error or refusal.
set -euo pipefail

FORCE=0
if [ "${1:-}" = "--force" ]; then
  FORCE=1
  shift
fi
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
TARGET="${1:-${OCTO_FORKS_DIR:-$REPO/.forks}/octosense-fork}"
PIN="6e9bfd4077cf8181878ac005ad586f908084a4bf"
BRANCH="feat/transport-generic-request"
PATCHES=(
  "$REPO/patches/octosense/0001-transport-generic-request.patch"
  "$REPO/patches/octosense/0002-transport-never-drop-a-reply.patch"
  "$REPO/patches/octosense/0003-shell-octoscode-module.patch"
)
TRANSPORT="apps/appcard/app/crates/octos-app-transport"
REMOTE="https://github.com/OctoSense-org/OctoSense"

command -v git >/dev/null || { echo "prepare-octosense-fork: git not found" >&2; exit 1; }
for p in "${PATCHES[@]}"; do
  [ -f "$p" ] || { echo "prepare-octosense-fork: missing patch $p" >&2; exit 1; }
done

subject() { awk '/^Subject: /{sub(/^Subject: \[PATCH\] /,"");print;exit}' "$1"; }
author() { awk '/^From: /{sub(/^From: /,"");print;exit}' "$1"; }

# Commit one patch with its own subject and author (committer = author).
apply_patch() {
  local p="$1" who name email
  if ! git apply --index --check "$p" 2>/dev/null; then
    echo "prepare-octosense-fork: $(basename "$p") does not apply to $TARGET:" >&2
    git apply --index --check "$p" 2>&1 | sed 's/^/  /' >&2 || true
    if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
      echo "  The tree has uncommitted changes (a hand-applied copy of this patch?)." >&2
      echo "  Commit or stash them, or re-run with --force to recreate the tree from the pin." >&2
    fi
    exit 1
  fi
  who="$(author "$p")"
  name="${who% <*}"
  email="${who##*<}"
  email="${email%>}"
  git apply --index "$p"
  git -c user.name="$name" -c user.email="$email" commit --quiet --author "$who" -m "$(subject "$p")"
  echo "prepare-octosense-fork: applied $(basename "$p")"
}

# k when HEAD is the pin + the first k patches (each one commit, in order,
# matched by subject); -1 otherwise.
stack_depth() {
  local k i
  for ((k = ${#PATCHES[@]}; k >= 0; k--)); do
    [ "$(git rev-parse "HEAD~$k" 2>/dev/null || true)" = "$PIN" ] || continue
    for ((i = 0; i < k; i++)); do
      [ "$(git log -1 --format=%s "HEAD~$((k - 1 - i))")" = "$(subject "${PATCHES[$i]}")" ] || continue 2
    done
    echo "$k"
    return
  done
  echo -1
}

if [ -d "$TARGET/.git" ]; then
  cd "$TARGET"
  depth="$(stack_depth)"
  transport_clean=1
  git diff --quiet HEAD -- "$TRANSPORT" || transport_clean=0
  if [ "$depth" -ge 0 ] && [ "$transport_clean" = 1 ]; then
    if [ "$depth" = "${#PATCHES[@]}" ]; then
      echo "prepare-octosense-fork: $TARGET already at $PIN + every patch (no-op)"
      exit 0
    fi
    for ((i = depth; i < ${#PATCHES[@]}; i++)); do
      apply_patch "${PATCHES[$i]}"
    done
    echo "prepare-octosense-fork: $TARGET is at $(git rev-parse --abbrev-ref HEAD) ($(git rev-parse --short HEAD)) = $PIN + every patch"
    exit 0
  fi
  if [ -n "$(git status --porcelain --untracked-files=no)" ] && [ "$FORCE" = 0 ]; then
    echo "prepare-octosense-fork: $TARGET has uncommitted changes and is not the pin + our patches;" >&2
    echo "  recreating it would discard them. Commit or move them, or re-run with --force." >&2
    exit 1
  fi
  echo "prepare-octosense-fork: recreating the existing checkout at $TARGET"
  git fetch --quiet origin "$PIN" 2>/dev/null || true
else
  echo "prepare-octosense-fork: cloning $PIN into $TARGET"
  mkdir -p "$(dirname "$TARGET")"
  git clone --quiet --no-checkout "$REMOTE" "$TARGET"
  cd "$TARGET"
  git fetch --quiet origin "$PIN"
fi

# Recreate the branch from the pinned rev and re-apply every patch, in order.
git checkout --quiet --detach "$PIN"
git checkout --quiet -B "$BRANCH"
git reset --quiet --hard "$PIN"
git clean --quiet -fd -e .sources -e target -e .cargo-ok
for p in "${PATCHES[@]}"; do
  apply_patch "$p"
done

echo "prepare-octosense-fork: $TARGET is at $BRANCH ($(git rev-parse --short HEAD)) = $PIN + every patch"
