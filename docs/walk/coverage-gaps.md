# Coverage gaps — smoke-only rows (#33a review item 3)

These rows PASS, but only through the generic smoke checks (the shell mounts, a
control is present, the module is Live). Their own behaviour — what the row's
`case` describes — has **no specific check yet**. They are coverage gaps, NOT
defects: the native surfaces are healthy, the walk-runner just doesn't exercise
the row's specific behaviour. Phase-4 planning input: each gap needs a
behaviour-specific check (assert what the case says the user would see).

Pass split for the whole run: **specific=60, smoke=54** (114 scripted, 30
distinct checks; docs/walk/results.csv `depth` column, introduced in 06b259c).

## keyboard (3 smoke-only rows)

Generic checks that carried these rows: Escape is delivered inertly (app stays live, no crash) x3; keyboard focus order: sidebar controls remain after keys x1

- row 106: starts a first workspace with the keyboard and focuses the useful field when add
- row 142: collapses and expands from the keyboard without losing the plan
- row 172: keyboard and semantics guarantees from the a11y batch hold

## palette (1 smoke-only rows)

Generic checks that carried these rows: the command palette overlay is mounted with search and list x1; the palette hint row shows the key hints x1

- row 68: opens, navigates, and executes the command palette by keyboard only

## peer (20 smoke-only rows)

Generic checks that carried these rows: the fleet roster renders its rows and slot dots x20; the sidebar hosts the GOALS/LOOPS/FLEET sections x20

- row 120: no seat panel when peer/control is unadvertised (no-method)
- row 121: no seat panel when external_driver_v1 is unadvertised (no-feature)
- row 122: each of the four commands emits exactly ONE peer/control frame
- row 123: an accepted receipt renders its slug and duplicate:false
- row 125: a refused RECEIPT renders the typed peer_control_refused refusal
- row 126: a driver_fence_stale typed error drops the seat, keeps the §6 label
- row 127: commands are keyboard-reachable and still emit exactly one frame
- row 128: Fleet opens from the navigation footer and lists the mock's 2 lanes
- row 129: Start stays disabled until a model is chosen and a brief is typed
- row 130: Start acquires then emits exactly ONE peer/dispatch and adopts a Fleet row
- row 135: a stale lease refuses peer/dispatch with the bounded §6 label
- row 136: a refused peer/dispatch renders the bounded label, never server copy
- row 137: the protocol console renders ONLY behind Fleet's Advanced disclosure
- row 138: an unknown lane is unconstructible: the picker is the ONLY lane source
- row 151: reports a start acknowledgement timeout as unknown, then settles on release
- row 183: settings preserves its geometry while model management loads at ${viewport.width
- row 184: skip link bypasses navigation and multiline commands retain valid accessibility 
- row 185: empty session search offers a direct way back and replies use the chat column
- row 72: Alt+D focuses Fleet's capability notice; suppressed inside inputs
- row 73: Alt+P toggles the peer dock fold, expanding and collapsing symmetrically

## recovery (20 smoke-only rows)

Generic checks that carried these rows: the module reaches conn: Live x27

- row 102: native monitors create, list, pause, resume and delete through typed receipts
- row 115: staging renders idle, then a peer turn flips the row to live
- row 117: a peer turn terminal lands the row as done while its sibling stays unlanded
- row 119: removes exactly the done row, keeps the live row, and announces the count
- row 13: an external-held session restores uploaded media and reasoning for an explicit r
- row 14: Resume chat and an owned seat release control before sending once
- row 162: restores the origin, tab credential, session, and workspace after refresh
- row 174: restores a pending structured question across a reload
- row 186: resume lists unverified candidates, refuses bare IDs, and requires confirmation 
- row 187: canceling a held historical open cannot install a late transcript or steal a new
- row 188: resuming an already retained busy historical Session never reopens or resets its
- row 189: reasoning visibility is retained per Session and does not change captured model 
- row 20: keeps the workspace restore receipt through canonical refresh without rewinding 
- row 204: keeps per-Session transcripts distinct and restores the pre-reload Session
- row 207: a slow model-management import can be canceled and never opens after cancellatio
- row 209: a failed local-command chunk restores input and never sends command text to the 
- row 210: a late clipboard command result does not enter a different conversation
- row 58: sidebar native buttons keep Enter behavior and search Escape restores its trigge
- row 66: restores and resolves a parked question on its owning Session with exact ids
- row 98: goal pause, resume and stop preserve the current objective and budget, and newer

## settings (10 smoke-only rows)

Generic checks that carried these rows: the settings drawer mounts with its close control x10; the drawer header reads Session settings with its sections x5; connection actions live in settings (Live status visible) x1

- row 10: binds a cross-profile launch to the resolved profile before open
- row 164: separates the Session runtime from the Profile default model
- row 165: keeps server connection actions in General settings
- row 176: preserves workspace launch decisions without exposing profile ids in connect
- row 181: skill writes hold the profile lock through deferred replies and deduplicate conf
- row 212: manual light preserves readable conversation and settings colors on a dark OS
- row 4: desktop notifications require Settings opt-in and stay silent while reading
- row 86: canceling a provider confirmation preserves Settings at ${viewport.width}px
- row 87: manages a provider through the DSH-style Models settings flow
- row 9: accepts a profile-routable web identity but closes a scoped mismatch

