#!/bin/bash
# Board 2 (setup) stage-B rebuild: same renderer patches + beauty-host build as board 1,
# then the image-to-appcard flow for the six setup screens. Idempotent.
set -e
cd "$(dirname "$0")/../../../.."   # repo root
PROJ="$PWD/design/stage-b/setup"
CLONE="$PWD/tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
WS="$PWD/tmp/stage-b/native-ws"
PATCHES="$PWD/design/stage-b/conversation/tools"   # renderer patches are shared with board 1
PY="${PY:-python3}"

mkdir -p "$WS/octoscript-makepad/apps/kit-host/resources/ux"
[ -f "$WS/octoscript-makepad/apps/kit-host/resources/ux/LiberationMono-Regular.ttf" ] || \
  cp "$WS/makepad/widgets/resources/LiberationMono-Regular.ttf" \
     "$WS/octoscript-makepad/apps/kit-host/resources/ux/LiberationMono-Regular.ttf"

TOP="$PATCHES/renderer-18e.patch"
if ! git -C "$WS/octoscript-makepad" apply --reverse --check "$TOP" >/dev/null 2>&1; then
  git -C "$WS/octoscript-makepad" checkout -- \
    crates/octoscript-makepad/src/design.rs apps/kit-host/src/beauty.rs \
    crates/octoscript-node/src/node.rs crates/octoscript-render/src/eval.rs
  for PATCH in renderer-inline-code.patch renderer-responsive.patch renderer-16c.patch renderer-16d.patch renderer-16e.patch renderer-18e.patch; do
    git -C "$WS/octoscript-makepad" apply "$PATCHES/$PATCH"
    echo "applied $PATCH"
  done
  rm -f "$PWD/tmp/beauty-clone-target/release/beauty-host"
fi

TARGET="$PWD/tmp/beauty-clone-target"
BEAUTY="$TARGET/release/beauty-host"
if [ ! -x "$BEAUTY" ]; then
  (
    cd "$WS/octoscript-makepad"
    export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
    export CARGO_TARGET_DIR="$TARGET"
    export CARGO_BUILD_JOBS=2
    export PATH="$CARGO_HOME/bin:$HOME/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    export GIT_CONFIG_GLOBAL=/dev/null
    RUSTFLAGS='' CARGO_PROFILE_RELEASE_LTO=false cargo build --release -p kit-host --bin beauty-host
  )
fi
echo "BUILD_OK $BEAUTY"
