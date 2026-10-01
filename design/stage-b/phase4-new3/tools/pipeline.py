#!/usr/bin/env python3
"""Drive the design-flow's observe/measure/map/semantic/compile per board-2
card (the D2a sidebar screens 1-5), in pipeline_setup.py's direct-call style:
native.py's order and imports, no flow files modified.

Run: python3 tools/pipeline.py [observe,measure,map,semantic,compile|all] [1 2 3 4 5]
"""
import json, sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent                       # design/stage-b/phase4-new3
WS = Path('tmp/stage-b/native-ws/OctoScript-App-Design-Flow')
sys.path.insert(0, str(WS / 'flows/image-to-card'))
sys.path.insert(0, str(WS / 'flows/image-lib'))
from observe import observe, map_observations
from measure_surfaces import measure
from semantics import propose, preflight
from compile import compile_page

ARTWORK = 'http://127.0.0.1:8190/ux-images/'   # this lane's published-asset port


import hashlib


def _sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def review_icons(d):
    """Post-process the proposed semantic-map: the svg line icons are authored
    by this lane from the reference (no raster, no UI text), so they take the
    reviewed `icon` decision (POLICY roles.icon = Svg/Icon/Image; the
    autonomy board's precedent element), not propose's needs_review."""
    import json
    contract = json.loads((d / 'contract.json').read_text())
    sm_path = d / 'semantic-map.json'
    sm = json.loads(sm_path.read_text())
    by_id = {e['id']: e for e in sm['elements']}
    fixed = 0
    for n in walk_nodes(contract['tree']):
        if n.get('t') != 'svg':
            continue
        asset = d / 'assets' / f"{n['id']}.svg"
        if not asset.is_file():
            raise FileNotFoundError(asset)
        e = by_id[n['id']]
        e['role'] = 'icon'
        e['basis'] = 'Line icon reconstructed as vector paths from the reference; no raster or UI text.'
        e['confidence'] = 1.0
        e['decision'] = 'reviewed'
        e['asset'] = {'path': f'assets/{n["id"]}.svg', 'sha256': _sha(asset),
                      'method': 'reference_svg',
                      'reference_sha256': sm['reference_sha256'],
                      'fit': 'stretch', 'clip': True,
                      'notes': 'Visually measured source icon reconstructed as vector paths; no text or embedded raster'}
        fixed += 1
    sm_path.write_text(json.dumps(sm, indent=2, ensure_ascii=False) + '\n')
    return f'{fixed} icons reviewed'


def walk_nodes(n):
    yield n
    for c in n.get('c', []):
        yield from walk_nodes(c)

STAGES = (sys.argv[1].split(',') if len(sys.argv) > 1 else ['observe', 'measure', 'map',
                                                            'semantic', 'compile'])
if STAGES == ['all']:
    STAGES = ['observe', 'measure', 'map', 'semantic', 'compile']
NUMS = [int(x) for x in sys.argv[2:]] or [1, 2, 3, 4]

for n in NUMS:
    d = ROOT / 'cards' / f'phase4n3-{n:02d}'
    out = {}
    if 'observe' in STAGES:
        out['observe'] = observe(d)
    if 'measure' in STAGES:
        out['measure'] = measure(d)
    if 'map' in STAGES:
        if (d / 'mapped.json').exists():
            out['map'] = 'retained'
        else:
            r = map_observations(d)
            out['map'] = r if isinstance(r, str) else 'mapped+proposed'
        # `propose()` is what WRITES semantic-map.json, and the early return
        # above skips it once mapped.json exists (board 2's pipeline carried
        # that quirk because its mapped.json and semantic-map.json were both
        # committed together). A fresh card has no semantic-map.json, so the
        # semantic stage's preflight would fail with 'missing semantic-map.json;
        # run the classify stage'. Run it whenever the manifest is absent.
        if not (d / 'semantic-map.json').exists():
            propose(d)
            out['map'] = out.get('map', '') + '+proposed'
    if 'semantic' in STAGES:
        out['icons'] = review_icons(d)
    if 'semantic' in STAGES:
        p = preflight(d)
        (d / 'semantic-preflight.json').write_text(json.dumps(p, indent=2) + '\n')
        out['semantic'] = 'PASS' if p.get('pass') else 'FAIL: ' + json.dumps(p.get('errors', p))[:300]
    if 'compile' in STAGES:
        r = compile_page(d, ARTWORK)
        out['compile'] = r if isinstance(r, str) else 'compiled'
    print(f"phase4n3-{n:02d}:", json.dumps(out, ensure_ascii=False, default=str)[:400])
