#!/usr/bin/env python3
"""A36 — build the FAITHFUL memory fixture for board 5
(`crates/octoscode-client/tests/fixtures/a36-memory-proposal-synthetic.jsonl`).

SYNTHETIC: no server answers memory for the Session's profile today. octos
a6ea8505 (and main dde76555) resolve every `memory/*` call to the signed-in
identity's profile — `admin` for the server token — so on a private serve
`memory/overview` returns admin's empty memory and `memory/search`,
`memory/load` and `memory/ingest` are refused with `-32603
runtime_unavailable` (the RECORDED replies are
`a36-memory-a6ea8505.jsonl`, A36's probe of a private serve). The Memory
dialog is built for the Session's profile (operator decision D2 A); this
fixture is what a server with the upstream change
(`docs/proposals/memory-profile-scope.md`) answers:

* every reply in octos-cli's own shape at a6ea8505:
  - `memory/overview` -> `{overview: MemoryOverviewResponse + the RPC-layer
    truncation fields}` (`api/memory_panel.rs` `MemoryOverviewResponse`,
    `ui_protocol_transport.rs` `apply_memory_overview_budgets`);
  - `memory/search` -> `{hits: [octos_memory::Hit]}` (`recall.rs` `Hit`:
    `{id, kind, source, title, abstract, score, timestamp, trust}`);
  - `memory/load` -> `{record: octos_memory::Record, page?, page_truncated}`
    (`record.rs` `Record`; `page` only for `bank:` knowledge records);
  - `memory/entity` -> `{name, content, content_truncated,
    content_total_bytes}`;
  - `memory/ingest` -> `UpsertReport` + `embedded`;
* PLUS the proposal's echo: a top-level `profile_id` naming the profile that
  answered (here `dsflash`, the Session's).

The content is board 5's (frames 2-9): the long-term memory, today's note,
three entity pages, two staging notes, the three "steer queue" hits and the
records they open. Timestamps are fixed here; `replay_serve --scenario
memory` re-dates them relative to the moment it answers ("Updated 2h ago").

Run from the repo root: python3 tools/fixtures/a36_memory_fixture.py
"""
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "crates/octoscode-client/tests/fixtures/a36-memory-proposal-synthetic.jsonl"
PROFILE = "dsflash"

LONG_TERM = """## Preferences
- Prefers Rust 2024 edition and cargo nextest.
- Reviews diffs before any commit; never force-pushes.

## Projects
- `octos`: steer queue redelivery on reconnect.
- `octoscode-app`: the native client on Makepad.

## People
- Sam reviews the octos PRs.
"""

TODAY = "Fixed the steer queue redelivery; the backoff now resets after a successful send.\n"

RECENT = [
    {"date": "2026-10-02", "content": "Reviewed PR #2566 (session fork); asked for a test on the fork name.\n"},
    {"date": "2026-09-30", "content": "Moved the native client to the board-3 dialog kit.\n"},
]

ENTITIES = [
    ("dsflash", "DeepSeek V4 Flash profile for daily work."),
    ("octos-core", "Rust crate with the UI protocol and its methods."),
    ("steer-queue", "Redelivers queued steering after a reconnect."),
]

STEER_QUEUE_PAGE = """# steer-queue

Redelivers queued steering messages after a reconnect.

## Facts
- Drains a snapshot first so the order stays stable.
- Backoff starts at 250 ms and resets after a successful send.

## Related
- octos-core · `ui_protocol.rs`
"""

PAGES = {
    "steer-queue": STEER_QUEUE_PAGE,
    "octos-core": "# octos-core\n\nRust crate with the UI protocol and its methods.\n\n## Facts\n"
                  "- Method names and their params live in `ui_protocol.rs`.\n"
                  "- Every memory method is gated by `auxiliary.rest_to_ws.v1`.\n",
    "dsflash": "# dsflash\n\nDeepSeek V4 Flash profile for daily work.\n\n## Facts\n"
               "- Spend caps apply to every turn.\n",
}

DOC_ID = "doc:octoscode:7f3a9c2e5b1d4a60"
EPISODE_ID = "episode:9f2c41d07e3b"

HITS = [
    {"id": "bank:steer-queue", "kind": "knowledge", "source": "bank", "title": "steer-queue",
     "abstract": "Redelivers queued steering after a reconnect; drains a snapshot first so order stays stable.",
     "score": 0.91, "timestamp": "2026-10-02T16:20:00Z", "trust": "trusted"},
    {"id": EPISODE_ID, "kind": "episode", "source": "episodes", "title": "Fix steer queue drop on reconnect",
     "abstract": "Added a backoff to redeliver; queued steering is resent from a snapshot.",
     "score": 0.74, "timestamp": "2026-10-01T18:05:00Z", "trust": "untrusted"},
    {"id": DOC_ID, "kind": "document", "source": "octoscode", "title": "Backoff for redelivery",
     "abstract": "Start at 250 ms, double up to 8 s, reset after a successful send.",
     "score": 0.52, "timestamp": "2026-09-30T09:12:00Z", "trust": "untrusted"},
]


def record(hit, body=None, visits=0):
    r = {"schema_version": 1, "id": hit["id"], "kind": hit["kind"], "source": hit["source"],
         "timestamp": hit["timestamp"], "title": hit["title"], "abstract": hit["abstract"],
         "trust": hit["trust"], "visits": visits, "promoted": False, "updated_at": hit["timestamp"]}
    if body:
        r["body"] = body
    if visits:
        r["last_visit"] = "2026-10-03T08:00:00Z"
    return r


def frame(direction, method, body, at):
    return {"at_ms": at, "body": body, "dir": direction, "method": method, "wall_ms": 1791061740000 + at}


def main():
    overview = {
        "ok": True,
        "long_term": LONG_TERM,
        "long_term_updated_at": "2026-10-03T10:00:00Z",
        "long_term_truncated": False,
        "long_term_total_bytes": len(LONG_TERM.encode()),
        "today": TODAY,
        "today_truncated": False,
        "today_total_bytes": len(TODAY.encode()),
        "recent": [dict(n, content_truncated=False, content_total_bytes=len(n["content"].encode())) for n in RECENT],
        "entities": [{"name": n, "summary": s} for n, s in ENTITIES],
        "entities_truncated": False,
        "staging_notes": 2,
        "staging_truncated": False,
        "refresh_enabled": True,
    }
    doc_body = ("Start at 250 ms, double up to 8 s, reset after a successful send.\n"
                "Applies to every redeliver attempt after a reconnect.")
    frames = [
        frame("out", "memory/overview", {"profile_id": PROFILE}, 0),
        frame("in", "memory/overview", {"profile_id": PROFILE, "overview": overview}, 40),
        frame("out", "memory/search", {"profile_id": PROFILE, "query": "steer queue", "limit": 20}, 1000),
        frame("in", "memory/search", {"profile_id": PROFILE, "hits": HITS}, 1060),
        frame("out", "memory/load", {"profile_id": PROFILE, "id": DOC_ID}, 2000),
        frame("in", "memory/load", {"profile_id": PROFILE, "record": record(HITS[2], doc_body, 3),
                                    "page_truncated": False}, 2040),
        frame("out", "memory/load", {"profile_id": PROFILE, "id": "bank:steer-queue"}, 2500),
        frame("in", "memory/load", {"profile_id": PROFILE, "record": record(HITS[0], None, 1),
                                    "page": STEER_QUEUE_PAGE, "page_truncated": False}, 2540),
        frame("out", "memory/load", {"profile_id": PROFILE, "id": EPISODE_ID}, 2800),
        frame("in", "memory/load", {"profile_id": PROFILE, "record": record(HITS[1], None, 2),
                                    "page_truncated": False}, 2840),
    ]
    at = 3000
    for name, _ in ENTITIES:
        content = PAGES[name]
        frames.append(frame("out", "memory/entity", {"profile_id": PROFILE, "name": name}, at))
        frames.append(frame("in", "memory/entity", {"profile_id": PROFILE, "name": name, "content": content,
                                                    "content_truncated": False,
                                                    "content_total_bytes": len(content.encode())}, at + 30))
        at += 100
    frames += [
        frame("out", "memory/ingest", {"profile_id": PROFILE, "records": [{
            "id": "doc:octoscode:0123456789abcdef", "kind": "document", "source": "octoscode",
            "timestamp": "2026-10-03T12:00:00Z", "title": "Backoff for redelivery",
            "abstract": "Start at 250 ms, double up to 8 s, reset after a successful send."}]}, 4000),
        frame("in", "memory/ingest", {"profile_id": PROFILE, "inserted": 1, "updated": 0, "unchanged": 0,
                                      "vectors_stored": 0, "embedded": 0}, 4050),
    ]
    OUT.write_text("".join(json.dumps(f, ensure_ascii=False, sort_keys=True) + "\n" for f in frames))
    print(f"wrote {len(frames)} frames -> {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
