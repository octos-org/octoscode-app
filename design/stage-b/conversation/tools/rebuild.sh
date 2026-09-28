#!/bin/bash
# Re-derive stage-B conversation cards end to end (idempotent).
set -e
cd "$(dirname "$0")/../../../.."   # repo root: p0-harness
PROJ="$PWD/design/stage-b/conversation"
CLONE="$PWD/tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
WS="$PWD/tmp/stage-b/native-ws"
PY=/Users/yuechen/miniconda3/bin/python3

# 1) Bundle the monospace face the kit ships into the clone's kit-host resources.
#    `self:resources/ux/*` resolves against the host crate's own tree at BUILD time,
#    so the face must be present before beauty-host is (re)built. makepad already
#    ships LiberationMono-Regular.ttf; we copy it rather than add a new dependency.
mkdir -p "$WS/octoscript-makepad/apps/kit-host/resources/ux"
cp -n "$WS/makepad/widgets/resources/LiberationMono-Regular.ttf" \
      "$WS/octoscript-makepad/apps/kit-host/resources/ux/LiberationMono-Regular.ttf"

# 2) Build beauty-host from the clone (it bakes the resource set above).
TARGET="$PWD/tmp/beauty-clone-target"
BEAUTY="$TARGET/release/beauty-host"
if [ ! -x "$BEAUTY" ]; then
  (
    cd "$WS/octoscript-makepad"
    export CARGO_HOME=/Users/yuechen/home/oa.noindex/.shared-cargo-home
    export CARGO_TARGET_DIR="$TARGET"
    export CARGO_BUILD_JOBS=2
    export PATH="$CARGO_HOME/bin:/Users/yuechen/.cargo/bin:/usr/bin:/bin:/usr/sbin:/sbin"
    export GIT_CONFIG_GLOBAL=/dev/null
    RUSTFLAGS='' CARGO_PROFILE_RELEASE_LTO=false cargo build --release -p kit-host --bin beauty-host
  )
fi

$PY design/stage-b/conversation/tools/author_v2.py
for n in 01 02 03 04 05 06 07 08 09 10 11 12; do
  D="$PROJ/cards/conversation-$n"
  rm -f "$D/observations.json" "$D/annotations.json" "$D/mapped.json" "$D/semantic-map.json" \
        "$D/conversion-brief.md" "$D/mapping.json" "$D/page.card" "$D/page.data.json" \
        "$D/semantic-preflight.json" "$D/semantic-audit.json" "$D/semantic-repair.json"
done
bash "$CLONE/tools/image-to-appcard-flow.sh" run --project "$PROJ" --manifest "$PROJ/image-to-appcard-flow.json" --stages observe,measure,map
$PY design/stage-b/conversation/tools/fix_metrics.py
$PY design/stage-b/conversation/tools/finalize_semantics.py
bash "$CLONE/tools/image-to-appcard-flow.sh" run --project "$PROJ" --manifest "$PROJ/image-to-appcard-flow.json" --stages semantic,compile
echo "REBUILD_OK"
