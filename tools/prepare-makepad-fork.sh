#!/usr/bin/env bash
# Recreate the makepad fork this repository's workspace builds against (the
# standalone OctosCode app, crates/octoscode-desktop, and the test suite).
#
# The fork is `OctoSense-org/makepad` at the rev OctoSense pins
# (`6cf03859`, its native-runtime.lock.json / runtime-patches.lock.json, and
# the `rev` in this repo's root Cargo.toml) with ONE commit on top: every
# `patches/makepad/*.patch`, applied by scripts/apply-makepad-patches.sh (the
# same files every other tree that builds the app gets, decision D10b):
#   layouter-ellipsis-pen.patch   the truncation ellipsis starts where the text ends (D10e)
#   macos-notifications.patch     the notification API (cfg(makepad_notifications), D10b)
#   packaged-file-resource.patch  a packaged build still reads file_resource(<absolute path>)
#   remote-browser-guard.patch    the --remote bridge's per-launch token (D10c)
# The root Cargo.toml's [patch."https://github.com/OctoSense-org/makepad.git"]
# points at this checkout.
#
# OctoSense's own runtime patch (tools/runtime-patches/makepad-settings.patch,
# which its tools/setup.py applies to .sources/makepad) is NOT part of this
# fork: the module neither calls nor needs any API it adds (this workspace has
# always built the module against the unpatched rev), our patches apply to the
# clean rev, and what it changes is Android/GLES (IME sync, JNI, program
# cache, density), accessibility preferences only the Android host feeds, and
# two widget helpers the module does not use (ViewRef::scroll_pos, the
# TextInputStateQuery event). The OctoSense-hosted build keeps it (setup.py).
#
# The checkout is shallow (the one pinned commit, like OctoSense's setup.py).
# This script is IDEMPOTENT: run twice, the second run is a no-op. The commit
# body records every patch's name and sha256 prefix, so an added or edited
# patch is noticed and the fork is recreated from the pin.
#
# Usage:
#   tools/prepare-makepad-fork.sh [--force] [target-dir]
#       target-dir: default <repo>/.forks/makepad-fork (or $OCTO_FORKS_DIR/makepad-fork)
#       --force: recreate even when the checkout has uncommitted changes (they are lost)
#   MAKEPAD_GIT_CACHE=<a clone holding the pinned commit> fetches from there first.
#
# Exit: 0 when the tree is the pin + the current patch set (one commit, clean);
#       1 on any error or refusal.
set -euo pipefail

FORCE=0
if [ "${1:-}" = "--force" ]; then
  FORCE=1
  shift
fi
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
TARGET="${1:-${OCTO_FORKS_DIR:-$REPO/.forks}/makepad-fork}"
PIN="6cf03859630761f5cb99ce7fcdfd8c30475d9ab8"
BRANCH="octoscode/makepad-patches"
REMOTE="https://github.com/OctoSense-org/makepad.git"
SUBJECT="octoscode: makepad patches (patches/makepad, scripts/apply-makepad-patches.sh)"

command -v git >/dev/null || { echo "prepare-makepad-fork: git not found" >&2; exit 1; }
command -v patch >/dev/null || { echo "prepare-makepad-fork: patch(1) not found" >&2; exit 1; }
shopt -s nullglob
PATCHES=("$REPO"/patches/makepad/*.patch)
[ ${#PATCHES[@]} -gt 0 ] || { echo "prepare-makepad-fork: no patches/makepad/*.patch" >&2; exit 1; }

# One line per patch: its name and the first 16 hex of its sha256.
stamp() {
  local p
  for p in "${PATCHES[@]}"; do
    echo "$(basename "$p") $(shasum -a 256 "$p" | cut -c1-16)"
  done
}

if [ -d "$TARGET/.git" ]; then
  cd "$TARGET"
  if [ "$(git rev-parse -q --verify HEAD~1 2>/dev/null || true)" = "$PIN" ] \
     && [ "$(git log -1 --format=%s)" = "$SUBJECT" ] \
     && [ "$(git log -1 --format=%b | sed '/^$/d')" = "$(stamp)" ] \
     && [ -z "$(git status --porcelain --untracked-files=no)" ]; then
    echo "prepare-makepad-fork: $TARGET already at ${PIN:0:8} + the current patch set (no-op)"
    exit 0
  fi
  if [ -n "$(git status --porcelain --untracked-files=no)" ] && [ "$FORCE" = 0 ]; then
    echo "prepare-makepad-fork: $TARGET has uncommitted changes and is not the pin + the current patches;" >&2
    echo "  recreating it would discard them. Commit or move them, or re-run with --force." >&2
    exit 1
  fi
  echo "prepare-makepad-fork: recreating $TARGET from ${PIN:0:8} (the patch set changed or the tree moved)"
else
  if [ -e "$TARGET" ] && [ -n "$(ls -A "$TARGET" 2>/dev/null)" ]; then
    echo "prepare-makepad-fork: $TARGET exists, is not empty and is not a git checkout" >&2
    exit 1
  fi
  echo "prepare-makepad-fork: fetching OctoSense-org/makepad@${PIN:0:8} into $TARGET (shallow)"
  mkdir -p "$TARGET"
  cd "$TARGET"
  git init --quiet
  git remote add origin "$REMOTE"
fi

if ! git cat-file -e "$PIN^{commit}" 2>/dev/null; then
  if [ -n "${MAKEPAD_GIT_CACHE:-}" ]; then
    git fetch --quiet --no-tags --depth=1 "$MAKEPAD_GIT_CACHE" "$PIN" 2>/dev/null || true
  fi
  git cat-file -e "$PIN^{commit}" 2>/dev/null || git fetch --quiet --no-tags --depth=1 origin "$PIN"
fi

git checkout --quiet --force --detach "$PIN"
git checkout --quiet -B "$BRANCH"
git reset --quiet --hard "$PIN"
git clean --quiet -fd -e target
if ! out="$("$REPO/scripts/apply-makepad-patches.sh" "$TARGET" 2>&1)"; then
  echo "$out" >&2
  echo "prepare-makepad-fork: a patch did not apply to ${PIN:0:8}" >&2
  exit 1
fi
echo "$out" | grep -v '^sha256 ' || true
# Every patch must now be applied (it reverse-applies cleanly).
"$REPO/scripts/apply-makepad-patches.sh" --check "$TARGET" >/dev/null
git add -A
git -c user.name=octos -c user.email=octos@local commit --quiet -m "$SUBJECT" -m "$(stamp)"
echo "prepare-makepad-fork: $TARGET is at $BRANCH ($(git rev-parse --short HEAD)) = ${PIN:0:8} + ${#PATCHES[@]} patches"
