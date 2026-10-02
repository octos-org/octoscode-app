#!/usr/bin/env python3
"""A10 — build the FAITHFUL external-driver fixture
(`crates/octoscode-client/tests/fixtures/a10-fleet-driver-synthetic.jsonl`).

No recording exists for `peer/dispatch`, `peer/control`, `session/driver/*`
or `external_driver_v1` (no live server here advertises them). This builds
one from:

* the RECORDED r6 `session/open` reply (`r6-peer-a6ea8505.jsonl`), with the
  external-driver methods + `external_driver_v1` added exactly as the web's
  e2e fixture server advertises them on a `peer-control-*` workspace
  (`apps/web/scripts/mock-ui-server.mjs:292-312`);
* the web's protocol shapes for every reply: `session/driver/get` with an
  operations page (`external-driver.ts:156-197`,
  `external-driver-operations.ts:316-388`, mock `peerControlDriverResponse`),
  `session/driver/acquire` (`parseDriverAcquireResult`, mock
  `peerControlAcquireResponse`), the `peer/dispatch` receipt
  (`decodePeerDispatchReceipt`, mock `peerDispatchAcceptedReceipt`), the
  `peer/control` receipt (`decodePeerControlReceipt`, mock
  `peerControlAcceptedReceipt`), the typed refusal error (mock
  `replyError(-32602, "driver operation refused: peer/dispatch",
  {kind})`), and the mock's two lanes (`PEER_CONTROL_LANES`);
* the RECORDED notification shapes (`turn/started` from c24, `approval/
  requested` + `approval/decided` + `user_question/requested` from r23),
  re-pointed at the adopted peer session `dsflash:main#peer-<slug>`.

Fields the replay echoes from the request (the operation id, the requested
lane, the acquiring driver id, the control target) are written with the
values a first Start would carry; `replay_serve --scenario fleet` and
`tests/a10_fleet.rs` echo them per request, exactly like the web's mock.

usage: python3 tools/fixtures/a10_fleet_fixture.py
"""
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent.parent
FIX = ROOT / "crates" / "octoscode-client" / "tests" / "fixtures"
SESSION = "dsflash:main"
PROFILE = "dsflash"
SLUG = "fleet-review"
PEER = f"{SESSION}#peer-{SLUG}"
ADOPTED_TURN = "00000000-0000-4000-8000-0000000000d1"
APPROVAL = "01a0eb92-9444-7101-aa6f-10065886f57e"
DRIVER = "octoscode-native:00000000-0000-4000-8000-0000000000aa"
OP = "00000000-0000-4000-8000-0000000000f1"
PRIOR_OP = "00000000-0000-4000-8000-0000000000e1"
DRIVER_METHODS = [
    "session/driver/get",
    "session/driver/acquire",
    "session/driver/renew",
    "session/driver/release",
    "peer/control",
    "peer/dispatch",
]


def recorded(name, method):
    for line in open(FIX / name):
        v = json.loads(line)
        if v["dir"] == "in" and v["method"] == method and v["body"] is not None:
            if method != "session/open" or "active_profile_id" in v["body"]:
                return v["body"]
    raise SystemExit(f"{name}: no {method}")


def main():
    opened = recorded("r6-peer-a6ea8505.jsonl", "session/open")
    caps = opened["capabilities"]
    caps["supported_features"] = caps["supported_features"] + ["external_driver_v1"]
    caps["supported_methods"] = caps["supported_methods"] + [m for m in DRIVER_METHODS if m not in caps["supported_methods"]]
    started = dict(recorded("c24-autonomy-a6ea8505.jsonl", "turn/started"))
    started.update({"session_id": PEER, "turn_id": ADOPTED_TURN})
    requested = dict(recorded("r23-conversation-a6ea8505.jsonl", "approval/requested"))
    requested.update({
        "session_id": PEER, "turn_id": ADOPTED_TURN, "approval_id": APPROVAL,
        "tool_name": "shell", "title": "Peer requests to run checks",
        "body": "A staged peer wants to run the repository checks.",
        "approval_kind": "command", "risk": "medium",
    })
    decided = dict(recorded("r23-conversation-a6ea8505.jsonl", "approval/decided"))
    decided.update({"session_id": PEER, "turn_id": ADOPTED_TURN, "approval_id": APPROVAL})
    question = dict(recorded("r23-conversation-a6ea8505.jsonl", "user_question/requested"))
    question.update({"session_id": PEER, "turn_id": ADOPTED_TURN})
    binding = {
        "driver_id": DRIVER, "epoch": 7, "revision": 42,
        "lease_expires_at_ms": 1770000000000, "workspace_root": "<WORKSPACE>",
    }
    prior = {
        "operation_id": PRIOR_OP, "kind": "peer_dispatch", "lifecycle": "started",
        "created_at_ms": 1769999000000, "started_at_ms": 1769999001000,
        "acceptance": {
            "model": "glm-4.6", "model_lane": "lane-review", "workspace_root": "<WORKSPACE>",
            "scoped_goal": {"goal_id": "goal_01"},
            "adopted_turn_id": "00000000-0000-4000-8000-0000000000d0",
            "adopted_session_id": f"{SESSION}#peer-lint-sweep", "slug": "lint-sweep",
            "accepted_at_ms": 1769999000000, "payload_digest": "synthetic-dispatch-digest-prior",
        },
    }
    lanes = [
        {"key": "lane-primary", "provider": "openai", "model": "gpt-5.4", "api_key_env": "OPENAI_API_KEY",
         "base_url": None, "description": "Primary dispatch lane", "default_context_window": 400000,
         "max_output_tokens": 128000, "api_type": "responses"},
        {"key": "lane-review", "provider": "anthropic", "model": "claude-opus-4-1", "api_key_env": "ANTHROPIC_API_KEY",
         "base_url": None, "description": "Secondary review lane", "default_context_window": 200000,
         "max_output_tokens": 64000, "api_type": "messages"},
    ]
    frames = [
        ("meta", "fixture:provenance", {"note": "SYNTHETIC — built by tools/fixtures/a10_fleet_fixture.py from the r6/r23/c24 recordings and the web's external-driver protocol (mock-ui-server.mjs peer-control fixture); no live recording exists for these methods"}),
        ("out", "session/open", {"profile_id": PROFILE, "session_id": SESSION}),
        ("in", "session/open", opened),
        ("out", "session/driver/get", {"session_id": SESSION, "operations": {"limit": 50}}),
        ("in", "session/driver/get", {
            "mode": "external", "recovery": "none", "binding": binding,
            "operations": {"items": [prior], "snapshot": f"synthetic-snapshot-{SESSION}",
                           "observed_revision": "42", "complete": True, "next_cursor": None},
        }),
        ("out", "profile/sub_providers/list", {"profile_id": PROFILE}),
        ("in", "profile/sub_providers/list", {"profile_id": PROFILE, "sub_providers": lanes}),
        ("out", "session/driver/acquire", {"session_id": SESSION, "driver_id": DRIVER, "expected_revision": 42, "lease_seconds": 120}),
        ("in", "session/driver/acquire", {"control_token": "synthetic-control-token", "pending_work": ["synthetic-pending-op"], "recovery": "none", "binding": binding}),
        ("out", "peer/prepare", {"brief": "Review the reconnect diff", "title": "Review the reconnect diff", "session_id": SESSION, "profile_id": PROFILE}),
        ("in", "peer/prepare", {
            "slug": SLUG, "topic": f"peer-{SLUG}", "profile_id": PROFILE, "cwd": "<WORKSPACE>",
            "brief_path": f"<HOME>/.octos/profiles/{PROFILE}/data/peers/{SLUG}/brief.md",
            "peers": [{"slug": SLUG, "topic": f"peer-{SLUG}", "profile_id": PROFILE, "cwd": "<WORKSPACE>",
                       "brief_path": f"<HOME>/.octos/profiles/{PROFILE}/data/peers/{SLUG}/brief.md"}],
        }),
        ("out", "peer/dispatch", {"session_id": SESSION, "driver_id": DRIVER, "epoch": 7, "control_token": "synthetic-control-token",
                                  "operation_id": OP, "model": "lane-primary",
                                  "dispatch": {"kind": "new_brief", "brief": "Review the reconnect diff", "title": SLUG}}),
        ("in", "peer/dispatch", {
            "operation_id": OP, "state": "accepted", "model": "gpt-5.4", "model_lane": "lane-primary",
            "workspace_root": "<WORKSPACE>", "scoped_goal": None, "adopted_turn_id": ADOPTED_TURN,
            "adopted_session_id": PEER, "slug": SLUG, "duplicate": False,
            "accepted_at_ms": 1770000000000, "payload_digest": "synthetic-payload-digest",
        }),
        ("in", "err:peer/dispatch", {"code": -32602, "message": "driver operation refused: peer/dispatch", "data": {"kind": "driver_model_unavailable"}}),
        ("out", "session/open", {"session_id": PEER, "profile_id": PROFILE, "cwd": "<WORKSPACE>"}),
        ("in", "turn/started", started),
        ("in", "approval/requested", requested),
        ("out", "peer/control", {"session_id": SESSION, "driver_id": DRIVER, "epoch": 7, "control_token": "synthetic-control-token",
                                 "operation_id": "00000000-0000-4000-8000-0000000000c9", "target_operation_id": OP,
                                 "expected_turn_id": ADOPTED_TURN,
                                 "command": {"kind": "approval_respond", "approval_id": APPROVAL, "decision": "approve"}}),
        ("in", "peer/control", {
            "operation_id": "00000000-0000-4000-8000-0000000000c9", "state": "accepted", "target_operation_id": OP,
            "expected_turn_id": ADOPTED_TURN, "target_session_id": PEER, "slug": SLUG,
            "accepted_at_ms": 1770000000000, "payload_digest": "synthetic-payload-digest", "duplicate": False,
        }),
        ("in", "approval/decided", decided),
        ("in", "user_question/requested", question),
        ("in", "turn/error", {"session_id": PEER, "turn_id": ADOPTED_TURN, "code": "interrupted", "message": "Turn interrupted"}),
    ]
    out = FIX / "a10-fleet-driver-synthetic.jsonl"
    with open(out, "w") as fh:
        for i, (d, m, body) in enumerate(frames):
            fh.write(json.dumps({"at_ms": i, "body": body, "dir": d, "method": m}, sort_keys=True) + "\n")
    print(f"wrote {out.relative_to(ROOT)} ({len(frames)} frames)")


if __name__ == "__main__":
    main()
