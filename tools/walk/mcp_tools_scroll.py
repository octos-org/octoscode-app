#!/usr/bin/env python3
"""Regression: the separate MCP and Tools panels must own wheel scrolling.

Run against an OWNED --remote app with a scrollable restored conversation and
an open inventory dialog: mcp_tools_scroll.py <port> <report.json>.
Only widget geometry is recorded; no chat text, tokens or server writes.
"""
import json
import pathlib
import sys
import time
import urllib.request

import bridgeauth  # noqa: F401 -- per-launch remote authentication


def run(port):
    base = f"http://127.0.0.1:{port}"

    def get(path):
        return urllib.request.urlopen(base + path, timeout=15).read()

    def snap():
        return json.loads(get('/snap'))['s']

    def item(name):
        return next(w for w in snap() if w['i'] == name and w['r'][2] > 0 and w['r'][3] > 0)

    def click(name):
        x, y, w, h = item(name)['r']
        get(f'/click?x={x+w/2}&y={y+h/2}&wait=1')
        time.sleep(.5)

    def chat():
        rows = snap()
        first = next(i for i, w in enumerate(rows) if w['i'] == 'timeline_list')
        last = next(i for i in range(first + 1, len(rows)) if rows[i]['i'] == 'composer_dock')
        return [(w['i'], w['ty'], w['r']) for w in rows[first:last]]

    def tools():
        return [(w['i'], w['r']) for w in snap() if w['i'].startswith('b3_inv_tool_')]

    checks = []
    click('b3_close')
    click('set_cap_tools')
    # Give the live inventory read time to finish.
    for _ in range(60):
        if tools():
            break
        time.sleep(.25)
    assert tools(), 'A scrollable tool list is required'
    baseline = chat()
    assert len(baseline) > 1, 'A populated conversation is required'
    first_tools = tools()
    x, y, w, h = item('b3_scroll')['r']
    for label, dy in [('tools down', 240), ('tools up', -240), ('tools top boundary', -240)]:
        get(f'/m?k=scroll&x={x+w/2}&y={y+70}&dy={dy}&wait=1')
        time.sleep(.3)
        same = chat() == baseline
        checks.append({'check': label + ': chat stays fixed', 'pass': same})
        if label == 'tools down':
            checks.append({'check': 'tools list scrolls', 'pass': tools() != first_tools})
    click('b3_close')
    click('set_cap_mcp')
    time.sleep(.5)
    x, y, w, h = item('b3_scroll')['r']
    get(f'/m?k=scroll&x={x+w/2}&y={y+50}&dy=-240&wait=1')
    time.sleep(.3)
    checks.append({'check': 'MCP panel: chat stays fixed', 'pass': chat() == baseline})
    # The modal backdrop also blocks scroll, even outside its card.
    get('/m?k=scroll&x=1150&y=350&dy=-240&wait=1')
    time.sleep(.3)
    checks.append({'check': 'modal backdrop: chat stays fixed', 'pass': chat() == baseline})
    click('b3_close')
    # This walk opens inventory from Settings; close the lower modal as well.
    if any(w['i'] == 'settings_close' and w['r'][2] > 0 for w in snap()):
        get('/m?k=scroll&x=1150&y=350&dy=-240&wait=1')
        time.sleep(.3)
        checks.append({'check': 'Settings backdrop: chat stays fixed', 'pass': chat() == baseline})
        click('settings_close')
    x, y, w, h = item('timeline_list')['r']
    get(f'/m?k=scroll&x={x+w/2}&y={y+h/2}&dy=-240&wait=1')
    time.sleep(.3)
    checks.append({'check': 'chat scrolls after the dialog closes', 'pass': chat() != baseline})
    return checks


if __name__ == '__main__':
    checks = run(int(sys.argv[1]))
    pathlib.Path(sys.argv[2]).write_text(json.dumps(checks, indent=2) + '\n')
    print(json.dumps(checks, indent=2))
    raise SystemExit(0 if all(c['pass'] for c in checks) else 1)
