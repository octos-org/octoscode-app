#!/usr/bin/env bash
# The scripted "another Mac" check (A33, decision D10f): a fresh clone builds BOTH variants from scratch.
#
#   tools/check-fresh-clone-macos.sh <empty-work-dir> [--source <repo or URL>] [--branch <name>] [-- <build-macos.sh args>]
#
# In <empty-work-dir> it makes a fresh user HOME (`user-home/`) with an EMPTY cargo home, keeps only the system PATH
# plus rustup's proxies (the stable toolchain is a prerequisite), clones <source> (default: this repository) at
# <branch> (default: the current branch) into user-home/src/octoscode-app, and runs
#   tools/build-macos.sh --package --octosense      (or the args after --)
# there, logging to <empty-work-dir>/fresh-clone.log with the time of every step and the disk use.
# Exit: build-macos.sh's. Nothing outside <empty-work-dir> is written (crates and repositories are downloaded).
set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
[ $# -ge 1 ] || { sed -n '2,12p' "$0"; exit 2; }
OM="$1"; shift
SOURCE="$REPO"
BRANCH="$(git -C "$REPO" rev-parse --abbrev-ref HEAD)"
BUILD_ARGS=(--package --octosense)
while [ $# -gt 0 ]; do
  case "$1" in
    --source) SOURCE="$2"; shift 2 ;;
    --branch) BRANCH="$2"; shift 2 ;;
    --) shift; BUILD_ARGS=("$@"); break ;;
    *) echo "check-fresh-clone: unknown argument $1" >&2; exit 2 ;;
  esac
done
mkdir -p "$OM"
OM="$(cd "$OM" && pwd)"
[ -z "$(ls -A "$OM" 2>/dev/null)" ] || { echo "check-fresh-clone: $OM must be empty" >&2; exit 1; }
REAL_HOME="$HOME"
RUSTUP_BIN="$(dirname "$(command -v rustup || command -v cargo)")"
mkdir -p "$OM/user-home/src" "$OM/user-home/Downloads"
exec > >(tee "$OM/fresh-clone.log") 2>&1

export HOME="$OM/user-home"
export CARGO_HOME="$HOME/.cargo"
export RUSTUP_HOME="${RUSTUP_HOME:-$REAL_HOME/.rustup}"
export PATH="$RUSTUP_BIN:/usr/bin:/bin:/usr/sbin:/sbin"
unset CARGO_TARGET_DIR RUSTFLAGS CARGO_INCREMENTAL OCTOSCODE_DESIGN_DIR OCTO_FORKS_DIR MAKEPAD MAKEPAD_PACKAGE_DIR
echo "fresh-clone: HOME=<work>/user-home, CARGO_HOME=<work>/user-home/.cargo (empty), PATH=rustup + system; $(date)"
echo "fresh-clone: $(command -v python3); jobs ${CARGO_BUILD_JOBS:-all cores}"
t0=$(date +%s)
git clone --quiet --branch "$BRANCH" "$SOURCE" "$HOME/src/octoscode-app" || exit 1
cd "$HOME/src/octoscode-app" || exit 1
echo "fresh-clone: cloned $(git log --oneline -1 | cut -c1-80) in $(( $(date +%s) - t0 )) s"
t1=$(date +%s)
tools/build-macos.sh "${BUILD_ARGS[@]}"
rc=$?
t2=$(date +%s)
echo "fresh-clone: build-macos.sh ${BUILD_ARGS[*]} exit $rc after $(( t2 - t1 )) s (clone to done: $(( t2 - t0 )) s)"
for d in "$HOME/src/octoscode-app" "$HOME/src/octoscode-app/target" "$HOME/src/octoscode-app/.forks/makepad-fork" \
         "$HOME/src/octoscode-app/.forks/octosense-fork" "$HOME/src/octoscode-app/.forks/octoscript-makepad-fork" \
         "$HOME/src/octoscode-app/.forks/octosense-host" "$CARGO_HOME" "$OM"; do
  [ -e "$d" ] && printf 'fresh-clone: disk %8s  %s\n' "$(du -sh "$d" | cut -f1)" "${d#$OM/}"
done
exit $rc
