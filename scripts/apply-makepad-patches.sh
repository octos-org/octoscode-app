#!/usr/bin/env bash
# Apply our makepad runtime patches (patches/makepad/*.patch) to makepad source
# trees — the SAME files to every tree that builds the app (decision D10b,
# docs/decisions/D10b-makepad-macos-notifications.md).
#
# Each <root> is a directory holding `makepad/` (an OctoSense `.sources`, or
# the APK tree's cargo-makepad checkout `.mk`) or a makepad checkout itself.
# The patches are unified diffs against makepad rev 6cf03859 (the rev
# OctoSense pins, native-runtime.lock.json / runtime-patches.lock.json).
#
# IDEMPOTENT: a patch already applied (it reverse-applies cleanly) is skipped;
# a patch that applies neither way stops the script before touching that
# tree. With --check nothing is written: it reports, per tree, applied /
# applicable / conflicting and exits 1 if any patch would not apply.
# Afterwards it prints the sha256 of every file the patches touch, so the
# trees can be compared (tests/a25_makepad_patch.rs does it in the suite).
#
# Usage:
#   scripts/apply-makepad-patches.sh [--check] <root> [<root>...]
# e.g. (the integrator's three trees + the APK's cargo-makepad; N = the
# oa.noindex work root):
#   scripts/apply-makepad-patches.sh $N/host-<name>/.sources \
#       $N/octosense-fork/.sources $N/apk-build/.sources $N/apk-build/.mk
#
# Back to clean upstream: `patch -R -p1 -d <tree>/makepad < patches/makepad/<p>.patch`
# (or `git -C <tree>/makepad checkout -- . && git clean -fd platform/src`).
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
CHECK=0
if [ "${1:-}" = "--check" ]; then CHECK=1; shift; fi
[ $# -ge 1 ] || { echo "usage: $0 [--check] <root> [<root>...]" >&2; exit 2; }
command -v patch >/dev/null || { echo "apply-makepad-patches: patch(1) not found" >&2; exit 1; }
shopt -s nullglob
PATCHES=("$REPO"/patches/makepad/*.patch)
[ ${#PATCHES[@]} -gt 0 ] || { echo "apply-makepad-patches: no patches/makepad/*.patch" >&2; exit 1; }

tree_of() {
  if [ -d "$1/makepad/platform/src" ]; then echo "$1/makepad"
  elif [ -d "$1/platform/src" ]; then echo "$1"
  else return 1; fi
}

touched() { sed -n 's|^+++ b/\([^[:space:]]*\).*|\1|p' "$1"; }

status=0
for root in "$@"; do
  tree="$(tree_of "$root")" || { echo "apply-makepad-patches: no makepad tree under $root" >&2; exit 1; }
  for p in "${PATCHES[@]}"; do
    name="$(basename "$p")"
    if patch -R -p1 -f -s --dry-run -d "$tree" < "$p" >/dev/null 2>&1; then
      echo "applied     $name  $tree"
    elif patch -p1 -f -s --dry-run -d "$tree" < "$p" >/dev/null 2>&1; then
      if [ "$CHECK" = 1 ]; then
        echo "applicable  $name  $tree"
      else
        patch -p1 -f -s -d "$tree" < "$p"
        echo "applied     $name  $tree  (now)"
      fi
    else
      echo "CONFLICT    $name  $tree  — the tree is not the pinned rev the patch was made against" >&2
      status=1
      [ "$CHECK" = 1 ] || exit 1
    fi
  done
done

# The fingerprint of every touched file, per tree (identical trees print the
# same column).
for p in "${PATCHES[@]}"; do
  for f in $(touched "$p"); do
    line="$f"
    for root in "$@"; do
      tree="$(tree_of "$root")"
      if [ -f "$tree/$f" ]; then
        line="$line $(shasum -a 256 "$tree/$f" | cut -c1-12)"
      else
        line="$line ------------"
      fi
    done
    echo "sha256 $line"
  done
done
exit $status
