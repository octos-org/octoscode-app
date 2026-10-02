#!/usr/bin/env python3
"""A28 — the diff review's word-mark / syntax / bound fixture (SYNTHETIC).

No live server ever answered `diff/preview/get` with a preview (r30a recorded
only the request and a preview-less refusal), so this writes two replies in
the octos-core `DiffPreviewGetResult` shape (`ui_protocol.rs`
`DiffPreviewGetResult` / `DiffPreviewFile` / `DiffPreviewHunk` /
`DiffPreviewLine`), one frame per line, the same `{dir, method, body}` frame
shape every recording in this directory uses:

1. `words` (preview ...0f1, "Retry the steer queue with backoff") — design
   board 4 frame 1 (design/stage-a/phase4-new4/README.md, row 23):
   - `crates/octos-cli/src/steer_queue.rs` (Rust grammar): two EQUAL 1:1
     change blocks (old 44 -> new 44, old 46 -> new 46) whose pairs share
     >= 25 % of their words, and one UNEQUAL block (+48, +49 with no
     removed line) that keeps the line tint only;
   - `config/octos.conf` (no grammar: plain text, still word-marked);
   - `docs/steer.md`: a 1:1 pair sharing < 25 % of its words (no marks).
2. `large` (preview ...0f2, "Bump octos to 0.24.1") — frame 1b: 520 + 2 lines
   (> 400) so NOTHING is decorated: `Cargo.lock` (no grammar) and
   `crates/octos-cli/Cargo.toml` (TOML grammar), each with a version bump
   pair that WOULD be word-marked below the bound.

The session id is the placeholder `<SESSION>`; the replay server and the
tests put the opened Session's id there. Regenerate with:

    python3 tools/fixtures/a28_diff_words_fixture.py
"""
import json
import os

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(ROOT, "crates", "octoscode-client", "tests", "fixtures", "a28-diff-words-synthetic.jsonl")
WORDS_ID = "01920000-0000-7000-8000-0000000000f1"
LARGE_ID = "01920000-0000-7000-8000-0000000000f2"


def ctx(content, old, new):
    return {"kind": "context", "content": content, "old_line": old, "new_line": new}


def rem(content, old):
    return {"kind": "removed", "content": content, "old_line": old}


def add(content, new):
    return {"kind": "added", "content": content, "new_line": new}


def words_preview():
    steer = [
        ctx("    fn redeliver(&mut self, conn: &Conn) -> Result<()> {", 42, 42),
        ctx("        let pending = self.drain_pending();", 43, 43),
        rem("        let delay = Duration::from_millis(500);", 44),
        add("        let delay = self.backoff.next(attempt);", 44),
        ctx("        for msg in pending.iter() {", 45, 45),
        rem("            conn.send(msg)?;", 46),
        add("            conn.send_with_retry(msg, &self.backoff)?;", 46),
        ctx("        }", 47, 47),
        add("        self.backoff.reset();", 48),
        add("        tracing::debug!(\"redelivered {} messages\", pending.len());", 49),
        ctx("        Ok(())", 48, 50),
        ctx("    }", 49, 51),
    ]
    conf = [rem("steer.retry.delay_ms = 500", 3), add("steer.retry.delay_ms = 250", 3)]
    doc = [
        ctx("## Retries", 11, 11),
        rem("Retries happen immediately.", 12),
        add("Backoff doubles up to 30 s.", 12),
    ]
    return {
        "status": "ready",
        "source": "pending_store",
        "preview": {
            "session_id": "<SESSION>",
            "preview_id": WORDS_ID,
            "title": "Retry the steer queue with backoff",
            "files": [
                {"path": "crates/octos-cli/src/steer_queue.rs", "status": "modified",
                 "hunks": [{"header": "@@ -42,8 +42,10 @@ impl SteerQueue {", "lines": steer}]},
                {"path": "config/octos.conf", "status": "modified",
                 "hunks": [{"header": "@@ -3,1 +3,1 @@", "lines": conf}]},
                {"path": "docs/steer.md", "status": "modified",
                 "hunks": [{"header": "@@ -11,2 +11,2 @@", "lines": doc}]},
            ],
        },
    }


def large_preview():
    lines = [
        ctx('source = "registry+https://github.com/rust-lang/crates.io-index"', 1240, 1240),
        rem('version = "0.24.0"', 1241),
        add('version = "0.24.1"', 1241),
    ]
    old = new = 1242
    n = 0
    while len(lines) < 520:
        n += 1
        block = [
            "",
            "[[package]]",
            f'name = "octos-dep-{n:03}"',
            f'version = "0.{n % 9}.{n % 13}"',
            'source = "registry+https://github.com/rust-lang/crates.io-index"',
            f'checksum = "{(n * 2654435761) % (1 << 64):016x}{(n * 40503) % (1 << 48):012x}a28d1ff5"',
        ]
        for content in block:
            lines.append(ctx(content, old, new))
            old += 1
            new += 1
    lines = lines[:520]
    toml = [
        ctx("[package]", 1, 1),
        ctx('name = "octos-cli"', 2, 2),
        rem('version = "0.24.0"', 3),
        add('version = "0.24.1"', 3),
    ]
    count = lambda ls, k: sum(1 for l in ls if l["kind"] in k)
    header = f"@@ -1240,{count(lines, ('context', 'removed'))} +1240,{count(lines, ('context', 'added'))} @@"
    return {
        "status": "ready",
        "source": "pending_store",
        "preview": {
            "session_id": "<SESSION>",
            "preview_id": LARGE_ID,
            "title": "Bump octos to 0.24.1",
            "files": [
                {"path": "Cargo.lock", "status": "modified", "hunks": [{"header": header, "lines": lines}]},
                {"path": "crates/octos-cli/Cargo.toml", "status": "modified",
                 "hunks": [{"header": "@@ -1,3 +1,3 @@", "lines": toml}]},
            ],
        },
    }


def main():
    frames = [
        {"dir": "in", "method": "res:diff/preview/get", "fixture": "words", "body": words_preview()},
        {"dir": "in", "method": "res:diff/preview/get", "fixture": "large", "body": large_preview()},
    ]
    with open(OUT, "w") as f:
        for fr in frames:
            f.write(json.dumps(fr, ensure_ascii=False, separators=(",", ":")) + "\n")
    total = sum(len(h["lines"]) for fr in frames[1:] for fl in fr["body"]["preview"]["files"] for h in fl["hunks"])
    print(f"wrote {os.path.relpath(OUT, ROOT)}: words + large ({total} lines)")


if __name__ == "__main__":
    main()
