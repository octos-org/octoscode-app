#!/bin/bash
# Re-derive stage-B conversation cards end to end (idempotent).
set -e
cd "$(dirname "$0")/../../../.."   # repo root: p0-harness
PROJ="$PWD/design/stage-b/conversation"
CLONE="$PWD/tmp/stage-b/native-ws/OctoScript-App-Design-Flow"
PY=/Users/yuechen/miniconda3/bin/python3

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
