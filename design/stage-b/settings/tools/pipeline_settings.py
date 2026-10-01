#!/usr/bin/env python3
"""Drive the design-flow's own observe/measure/map/semantic/compile per settings card.

pipeline_setup.py's direct-call pattern (native.py's order and imports, no flow
files modified), pointed at this board's seven settings cards."""
import json, sys
from pathlib import Path
WS = Path('tmp/stage-b/native-ws/OctoScript-App-Design-Flow')
sys.path.insert(0, str(WS / 'flows/image-lib'))
from observe import observe, map_observations
from measure_surfaces import measure
from semantics import propose, preflight
from compile import compile_page

STAGES = sys.argv[1].split(',') if len(sys.argv) > 1 else ['observe','measure','map','semantic','compile']
for k in range(6, 13):
    d = Path(f'design/stage-b/settings/cards/settings-{k:02d}')
    out = {}
    if 'observe' in STAGES:  out['observe'] = observe(d)
    if 'measure' in STAGES:  out['measure'] = measure(d)
    if 'map' in STAGES:
        if (d / 'mapped.json').exists():
            out['map'] = 'retained'
        else:
            r = map_observations(d); propose(d)
            out['map'] = r if isinstance(r, str) else 'mapped+proposed'
    if 'semantic' in STAGES:
        p = preflight(d)
        (d / 'semantic-preflight.json').write_text(json.dumps(p, indent=2) + '\n')
        out['semantic'] = 'PASS' if p.get('pass') else 'FAIL: ' + json.dumps(p.get('errors', p))[:200]
    if 'compile' in STAGES:
        r = compile_page(d, 'http://127.0.0.1:8170/ux-images/')
        out['compile'] = r if isinstance(r, str) else 'compiled'
    print(f"settings-{k:02d}:", json.dumps(out, ensure_ascii=False, default=str)[:300])
