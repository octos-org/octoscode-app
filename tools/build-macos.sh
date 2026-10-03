#!/usr/bin/env bash
# From a fresh clone to a running OctosCode on macOS, in one command (A33, decision D10f).
# docs/BUILD-macos.md has the prerequisites, the run/connect steps and troubleshooting.
#
#   tools/build-macos.sh [--package] [--octosense] [--debug] [--work <dir>]
#
# Default: the STANDALONE desktop app (crates/octoscode-desktop, binary `octoscode`), OPTIMIZED:
#   1. prepares the two renderer forks the root Cargo.toml [patch]es, in <work> (default
#      <repo>/.forks; a fork kept elsewhere is symlinked into .forks/):
#        makepad-fork             OctoSense-org/makepad@6cf03859 + patches/makepad/*      (tools/prepare-makepad-fork.sh)
#        octoscript-makepad-fork  Octoscript-Makepad@6881fb6c + patches/octoscript-makepad/* (tools/prepare-octoscript-makepad-fork.sh)
#   2. cargo build --release -p octoscode-desktop       -> target/release/octoscode (target/debug with --debug)
# --package    also tools/package-macos.sh              -> target/macos-app/OctosCode.app + OctosCode-macos-<arch>.zip
# --octosense  also the OctoSense-HOSTED variant (the module inside the OctoSense shell), in <work>/octosense-host:
#              OctoSense at the pin + patches/octosense/0001-0003 (0003 wires the module into the shell), its
#              framework sources via OctoSense's own tools/setup.py, our makepad patches on them
#              (scripts/apply-makepad-patches.sh), the octoscode crates and design/ vendored into apps/
#              (as outer/scripts/hostbuild.sh does), then
#              cargo build --release -p octosense --features app-octoscode,octoscode-module/octosense-module
#              -> <work>/octosense-host/target/release/octosense (target/debug with --debug)
# --debug      unoptimized-workspace (dev profile) builds of both instead, for development only: the
#              UI is measurably slower (A35b: the folder browser's open/navigation took 0.3-0.7 s at
#              opt-level 0 against < 0.1 s optimized). The root Cargo.toml's dev profile optimizes the
#              dependencies, so even this build stays usable.
# --release    accepted for compatibility: the optimized build is the default.
#
# Prerequisites: macOS with the Xcode Command Line Tools, git, python3, rustup (stable toolchain).
# IDEMPOTENT: every step is a no-op when its result is already there; re-run it to update after a pull.
# Network: GitHub (the four pinned repositories) and crates.io, the first time.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$HERE/.." && pwd)"
PACKAGE=0
OCTOSENSE=0
# A35b: the optimized build is the default; nobody gets the slow one by accident.
PROFILE=release
WORK=""
while [ $# -gt 0 ]; do
  case "$1" in
    --package) PACKAGE=1; shift ;;
    --octosense) OCTOSENSE=1; shift ;;
    --debug) PROFILE=debug; shift ;;
    --release) PROFILE=release; shift ;;
    --work) WORK="$2"; shift 2 ;;
    -h|--help) sed -n '2,29p' "$0"; exit 0 ;;
    *) echo "build-macos: unknown argument $1 (see --help)" >&2; exit 2 ;;
  esac
done

step() { printf '\n== %s\n' "$*"; }
die() { echo "build-macos: $*" >&2; exit 1; }

step "prerequisites"
[ "$(uname -s)" = "Darwin" ] || die "macOS only (this is $(uname -s))"
xcode-select -p >/dev/null 2>&1 || die "the Xcode Command Line Tools are missing: run  xcode-select --install"
for tool in git python3 patch rsync shasum ditto codesign; do
  command -v "$tool" >/dev/null || die "$tool not found (the Command Line Tools provide it)"
done
command -v cargo >/dev/null || die "cargo not found: install rustup (https://rustup.rs), then  rustup default stable"
echo "$(sw_vers -productName) $(sw_vers -productVersion) $(uname -m); $(rustc -V); $(git --version)"
free_gb=$(df -g "$REPO" | awk 'NR==2 {print $4}')
echo "free disk under the repo: ${free_gb} GB (the standalone build needs ~8 GB, --octosense ~25 GB more)"
[ "${free_gb:-0}" -ge 8 ] || echo "build-macos: WARNING: less than 8 GB free" >&2

WORK="${WORK:-$REPO/.forks}"
mkdir -p "$WORK" "$REPO/.forks"
WORK="$(cd "$WORK" && pwd)"

# A fork lives in <work>/<name>; .forks/<name> is the path the root Cargo.toml names.
# This script prepares only forks it owns: a .forks/<name> that is a link to anywhere
# else (a checkout shared between several clones) is never touched.
fork_path() {
  local name="$1"
  local dest="$REPO/.forks/$name"
  local want="$WORK/$name"
  if [ "$WORK" = "$REPO/.forks" ]; then
    if [ -L "$dest" ]; then
      die ".forks/$name is a link to $(readlink "$dest") (a shared checkout?); this script prepares only the forks it owns: remove the link, or pass --work <dir> for a set of your own"
    fi
  else
    if [ -L "$dest" ]; then
      [ "$(readlink "$dest")" = "$want" ] || die ".forks/$name links to $(readlink "$dest"), not $want: remove the link first"
    elif [ -e "$dest" ]; then
      die ".forks/$name is a directory, but --work puts the fork in $want: move .forks/$name away or drop --work"
    fi
    ln -sfn "$want" "$dest"
  fi
  echo "$want"
}

step "1/3 forks"
# (assignments, so a refusal inside fork_path stops the script under set -e)
MAKEPAD_FORK="$(fork_path makepad-fork)"
RENDERER_FORK="$(fork_path octoscript-makepad-fork)"
bash "$HERE/prepare-makepad-fork.sh" "$MAKEPAD_FORK"
bash "$HERE/prepare-octoscript-makepad-fork.sh" "$RENDERER_FORK"

step "2/3 the standalone app (cargo build -p octoscode-desktop, $PROFILE)"
started=$(date +%s)
if [ "$PROFILE" = release ]; then
  (cd "$REPO" && cargo build --release -p octoscode-desktop --bin octoscode)
else
  (cd "$REPO" && cargo build -p octoscode-desktop --bin octoscode)
fi
BIN="${CARGO_TARGET_DIR:-$REPO/target}/$PROFILE/octoscode"
[ -x "$BIN" ] || die "the build finished but $BIN is missing"
echo "build-macos: standalone app built in $(( $(date +%s) - started )) s: $BIN"

if [ "$PACKAGE" = 1 ]; then
  step "2b/3 OctosCode.app (tools/package-macos.sh)"
  bash "$HERE/package-macos.sh"
fi

HOST_BIN=""
if [ "$OCTOSENSE" = 1 ]; then
  step "3/3 the OctoSense-hosted variant ($PROFILE)"
  HOST="$WORK/octosense-host"
  bash "$HERE/prepare-octosense-fork.sh" "$HOST"
  # The renderer fork must sit BESIDE the host tree: patch 0003's [patch] and the vendored
  # module's octoscript-render path name ../octoscript-makepad-fork.
  RENDERER="$(dirname "$HOST")/octoscript-makepad-fork"
  [ -d "$RENDERER/crates/octoscript-makepad" ] || die "$RENDERER is missing (step 1 prepares it in <work>)"
  # OctoSense's framework checkouts (.sources: makepad + its reviewed runtime patch,
  # octoscript, octoscript-makepad), exactly as OctoSense's own README builds. setup.py
  # refuses to run over local changes, and our makepad patches below are local changes
  # to .sources/makepad: it runs once, when .sources is not prepared yet.
  if [ -d "$HOST/.sources/makepad/.git" ] && [ -d "$HOST/.sources/octoscript/.git" ] \
     && [ -d "$HOST/.sources/octoscript-makepad/.git" ]; then
    echo "build-macos: $HOST/.sources already prepared (delete it to fetch the framework sources again)"
  else
    (cd "$HOST" && python3 tools/setup.py >/dev/null)
    echo "build-macos: $HOST/.sources prepared by OctoSense's tools/setup.py"
  fi
  bash "$REPO/scripts/apply-makepad-patches.sh" "$HOST/.sources" | grep -v '^sha256 '
  # Vendor the module, client and store + design/ (outer/scripts/hostbuild.sh).
  for c in module client store; do
    rsync -a --delete --exclude target "$REPO/crates/octoscode-$c/" "$HOST/apps/octoscode/$c/"
  done
  sed -i '' 's|path = "../../.forks/octoscript-makepad-fork/|path = "../../../../octoscript-makepad-fork/|' \
    "$HOST/apps/octoscode/module/Cargo.toml"
  rsync -a --delete "$REPO/design/" "$HOST/apps/design/"
  echo "$(git -C "$REPO" rev-parse --short HEAD) $(date +%Y-%m-%dT%H:%M:%S)" > "$HOST/apps/octoscode/BUILT_FROM"
  started=$(date +%s)
  if [ "$PROFILE" = release ]; then
    (cd "$HOST" && cargo build --release -p octosense --features app-octoscode,octoscode-module/octosense-module)
  else
    (cd "$HOST" && cargo build -p octosense --features app-octoscode,octoscode-module/octosense-module)
  fi
  HOST_BIN="$HOST/target/$PROFILE/octosense"
  [ -x "$HOST_BIN" ] || die "the host build finished but $HOST_BIN is missing"
  echo "build-macos: OctoSense host built in $(( $(date +%s) - started )) s: $HOST_BIN"
fi

step "done"
cat <<EOF
Run the standalone app (a normal window; connect from its Connect card):
  $BIN
or with a server preset (the token stays in this shell's environment only):
  OCTOS_BASE_URL=http://127.0.0.1:50190 OCTOS_BEARER=<token> OCTOS_PROFILE_ID=<profile> $BIN
The instrument bridge, only when asked (per-launch token, harness/bcurl):
  $BIN --remote <port>
EOF
if [ "$PACKAGE" = 1 ]; then
  echo "The self-contained app and the zip to copy to another Mac: ${CARGO_TARGET_DIR:-$REPO/target}/macos-app/"
fi
if [ -n "$HOST_BIN" ]; then
  cat <<EOF
The OctoSense-hosted variant (OctosCode opens as a window of the OctoSense desktop):
  MAKEPAD_WM_TEST_APP=octoscode OCTOSCODE_DESIGN_DIR=$REPO/design $HOST_BIN --module octoscode
EOF
fi
