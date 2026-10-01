#!/usr/bin/env python3
"""Drive the design-flow's observe/measure/map/semantic/compile for p4 cards
(the setup/tools/pipeline_setup.py pattern, board dir swapped)."""
import json
import sys
from pathlib import Path
WS = Path('tmp/stage-b/native-ws/OctoScript-App-Design-Flow')
sys.path.insert(0, str(WS / 'flows/image-to-card'))
sys.path.insert(0, str(WS / 'flows/image-lib'))
from observe import observe, map_observations
from measure_surfaces import measure
from semantics import propose, preflight
from compile import compile_page

for name in sys.argv[1:]:
    d = Path(f'design/stage-b/phase4/cards/{name}')
    out = {}
    out['observe'] = observe(d)
    out['measure'] = measure(d)
    r = map_observations(d)
    propose(d)
    import importlib.util
    fspec = importlib.util.spec_from_file_location(
        'finalize_phase4', 'design/stage-b/phase4/tools/finalize_semantics_phase4.py')
    fmod = importlib.util.module_from_spec(fspec)
    fspec.loader.exec_module(fmod)
    fmod.finalize(d)
    try:
        p = preflight(d)
        (d / 'semantic-preflight.json').write_text(json.dumps(p, indent=2) + '\n')
        out['semantic'] = 'PASS' if p.get('pass') else json.dumps(p.get('errors', p))[:300]
    except Exception as e:
        out['semantic'] = f'RAISED {e}'
        print(name, json.dumps(out, ensure_ascii=False, default=str)[:400])
        continue  # one card's failure must not mask the others
    try:
        out['compile'] = compile_page(d, 'http://127.0.0.1:8170/ux-images/')
    except Exception as e:
        out['compile'] = f'ERROR {e}'
    print(name, json.dumps(out, ensure_ascii=False, default=str)[:400])
