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
# `cp -n` exits 1 when it skips an existing file, which `set -e` would treat as
# fatal — copy only when absent instead.
[ -f "$WS/octoscript-makepad/apps/kit-host/resources/ux/LiberationMono-Regular.ttf" ] || \
  cp "$WS/makepad/widgets/resources/LiberationMono-Regular.ttf" \
     "$WS/octoscript-makepad/apps/kit-host/resources/ux/LiberationMono-Regular.ttf"

# 2) Apply the renderer patches in order (idempotent).
#    `design.rs`'s widget match had no arm emitting makepad's `Markdown`, so a
#    prose node carrying inline `code` could not reach the widget's inline-code
#    draw hook (`widgets/src/markdown.rs:118-176`); card #16b then made the
#    measured backend honour the fill/fit flags. Both patches touch the same
#    regions of `design.rs`, so a PER-PATCH reverse-check is unreliable: once
#    both are applied, reverse-checking only the first fails because the second
#    moved its context. Detect the fully-applied state from the TOP patch, and
#    otherwise rebuild the renderer from pristine and apply both in order —
#    deterministic and order-correct (the clone is a build artifact).
TOP="$PWD/design/stage-b/conversation/tools/renderer-18e.patch"
if ! git -C "$WS/octoscript-makepad" apply --reverse --check "$TOP" >/dev/null 2>&1; then
  git -C "$WS/octoscript-makepad" checkout -- \
    crates/octoscript-makepad/src/design.rs apps/kit-host/src/beauty.rs \
    crates/octoscript-node/src/node.rs crates/octoscript-render/src/eval.rs
  for PATCH in renderer-inline-code.patch renderer-responsive.patch renderer-16c.patch renderer-16d.patch renderer-16e.patch renderer-18e.patch; do
    git -C "$WS/octoscript-makepad" apply "$PWD/design/stage-b/conversation/tools/$PATCH"
    echo "applied $PATCH"
  done
  # The patched renderer changes the host, so force a rebuild of that binary.
  rm -f "$PWD/tmp/beauty-clone-target/release/beauty-host"
fi

# 3) Build beauty-host from the clone (it bakes the resource set above).
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
# `measure` records a surface fill as one whole-interior median, which a large
# differently-coloured child contaminates (card #11e: tool_3's white card went
# grey because its console box covers 61% of it). Restore the authored fill there.
$PY design/stage-b/conversation/tools/fix_surfaces.py
$PY design/stage-b/conversation/tools/finalize_semantics.py
bash "$CLONE/tools/image-to-appcard-flow.sh" run --project "$PROJ" --manifest "$PROJ/image-to-appcard-flow.json" --stages semantic,compile
echo "REBUILD_OK"
