//! Store tests — one per domain, plus the transcript's folding behaviour.
//!
//! These exercise the store directly (no transport, no socket), which is what
//! makes the fan-out shape verifiable: each domain's state is its own struct
//! with its own lock, so a test never has to set up the others.
use octoscode_store::domains::approval::PendingApproval;
use octoscode_store::domains::autonomy::AutonomyEntity;
use octoscode_store::domains::media::MediaItem;
use octoscode_store::domains::peer::Peer;
use octoscode_store::domains::task::Task;
use octoscode_store::domains::tool::RuntimeTool;
use octoscode_store::{EntryKind, Session, Store};

fn session(id: &str) -> Session {
    Session {
        id: id.to_owned(),
        title: Some(format!("Session {id}")),
        message_count: 0,
        updated_at: None,
        last_prompt: None,
        active_turn: false,
    }
}

// ---------------------------------------------------------------- connection

#[test]
fn connection_state_records_display_and_liveness() {
    let store = Store::new();
    store.set_connection("Reconnecting { attempt: 2 }".into(), false);
    assert_eq!(store.connection(), "Reconnecting { attempt: 2 }");
    assert!(!store.is_live());

    store.set_connection("Live".into(), true);
    assert_eq!(store.connection(), "Live");
    assert!(store.is_live());
}

// --------------------------------------------------------------- capabilities

#[test]
fn config_capabilities_round_trip_and_lookup_is_case_insensitive() {
    let store = Store::new();
    assert_eq!(store.capabilities(), Vec::<String>::new());

    store.set_capabilities(vec!["auxiliary.rest_to_ws.v1".into(), "Session.Hydrate.V1".into()]);
    assert_eq!(store.capabilities().len(), 2);
    assert!(store.domains.config.has_capability("auxiliary.rest_to_ws.v1"));
    assert!(store.domains.config.has_capability("  SESSION.HYDRATE.V1 "));
    assert!(!store.domains.config.has_capability("nope"));
    assert_eq!(store.domains.config.count(), 2);
}

// ------------------------------------------------------------------- sessions

#[test]
fn session_list_and_count_and_active() {
    let store = Store::new();
    store.set_sessions(vec![session("a"), session("b")]);
    assert_eq!(store.session_count(), 2);
    assert_eq!(store.sessions()[0].title.as_deref(), Some("Session a"));

    store.set_active(Some("b".into()));
    assert_eq!(store.active_session().as_deref(), Some("b"));
    assert_eq!(store.summary(), "conn:    sessions: 2");
}

#[test]
fn list_merge_keeps_locally_known_sessions_the_reply_omits() {
    let store = Store::new();
    store.set_sessions(vec![session("a"), session("b")]);
    store.set_active(Some("b".into()));

    // The gate's catalog reply can lag the tab (#34b): a reply naming only
    // "a" must not drop the locally-known "b" (the #39a row-2 live defect —
    // the sidebar emptied and the running turn became unreachable).
    store.set_sessions(vec![session("a")]);
    let listed = store.sessions();
    let ids: Vec<&str> = listed.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"a") && ids.contains(&"b"), "{ids:?}");
    assert_eq!(store.session_count(), 2);
    // The active id survives a lagging reply (it cannot dangle: the merged
    // list keeps every locally-known row).
    assert_eq!(store.active_session().as_deref(), Some("b"));

    // An id that survives stays active (unchanged contract).
    store.set_active(Some("a".into()));
    store.set_sessions(vec![session("a"), session("c")]);
    assert_eq!(store.active_session().as_deref(), Some("a"));
}

// -------------------------------------------------------------------- turns

#[test]
fn turn_lifecycle_tracks_in_flight() {
    let store = Store::new();
    store.domains.turn.started("t1");
    store.domains.turn.started("t2");
    assert!(store.domains.turn.is_in_flight("t1"));
    assert_eq!(store.domains.turn.in_flight_count(), 2);

    store.domains.turn.ended("t1");
    assert!(!store.domains.turn.is_in_flight("t1"));
    assert_eq!(store.domains.turn.in_flight_count(), 1);
    assert_eq!(store.domains.turn.seen(), vec!["t1", "t2"]);
}

// ------------------------------------------------------------------ timeline

#[test]
fn deltas_fold_into_one_assistant_entry() {
    let store = Store::new();
    let tl = &store.domains.session.timeline;
    for text in ["Hel", "lo", ", ", "world"] {
        tl.append_delta("s", Some("t1"), EntryKind::ASSISTANT_TEXT, text);
    }
    // Four deltas -> ONE entry (the card's "fold into one assistant entry").
    assert_eq!(tl.len("s"), 1);
    assert_eq!(store.live_text("s"), "Hello, world");
    assert_eq!(tl.entries("s")[0].kind, EntryKind::ASSISTANT_TEXT);
}

#[test]
fn a_closed_turn_starts_a_new_entry_for_the_next_delta() {
    let store = Store::new();
    let tl = &store.domains.session.timeline;
    tl.append_delta("s", Some("t1"), EntryKind::ASSISTANT_TEXT, "first");
    tl.close_turn("s", "t1");
    tl.append_delta("s", Some("t1"), EntryKind::ASSISTANT_TEXT, "second");

    assert_eq!(tl.len("s"), 2, "a closed entry must not absorb later text");
    assert_eq!(tl.entries("s")[0].text, "first");
    assert_eq!(tl.entries("s")[1].text, "second");
}

#[test]
fn a_delta_does_not_cross_turns_or_sessions() {
    let store = Store::new();
    let tl = &store.domains.session.timeline;
    tl.append_delta("s1", Some("t1"), EntryKind::ASSISTANT_TEXT, "a");
    tl.append_delta("s2", Some("t1"), EntryKind::ASSISTANT_TEXT, "b");
    tl.append_delta("s1", Some("t2"), EntryKind::ASSISTANT_TEXT, "c");

    assert_eq!(store.live_text("s1"), "ac");
    assert_eq!(store.live_text("s2"), "b");
}

/// Card #21i — a late delta must not overwrite the canonical receipt.
///
/// The real server interleaves: `assistant_persisted` lands mid-stream, and more
/// `assistant_delta`s follow it. On the live `trace.jsonl` (turn
/// `01a0ed1d-9d4d-7343-9aff-c5d5e6d20ccc`) the receipt arrived at frame 94 of
/// 105 and the 11 deltas after it joined to EXACTLY the 45-char tail
/// `` ` and is formatted into the `{}` placeholder.`` — which is what the screen
/// showed, because `finalize_assistant` closed the entry and the later deltas
/// opened a SECOND entry that `timeline_rows` renders (`.next_back()`).
///
/// The web is explicit that the receipt wins ("Receipt finality wins over
/// delivery order", `apps/web/src/features/timeline/model.ts:655-663`), so the
/// persisted body is the segment's canonical text and later deltas are dropped.
/// This test is the store-level half; `f21d_lowering.rs` proves the row that
/// renders reads it.
#[test]
fn a_delta_after_the_receipt_keeps_the_canonical_persisted_body() {
    let store = Store::new();
    let tl = &store.domains.session.timeline;

    // The exact live order + strings (turn 01a0ed1d…, live-gate trace.jsonl).
    const PRE: &str = "`main.rs` prints a single line, `5` — `main` calls `println!(\"{}\", add(2, 3))`, and the helper `add(a: i32, b: i32) -> i32 { a + b }` returns the sum of its arguments, so `2 + 3` evaluates to `5`";
    const PERSISTED: &str = "`main.rs` prints a single line, `5` — `main` calls `println!(\"{}\", add(2, 3))`, and the helper `add(a: i32, b: i32) -> i32 { a + b }` returns the sum of its arguments, so `2 + 3` evaluates to `5` and is formatted into the `{}` placeholder.";
    const LATE: &str = "` and is formatted into the `{}` placeholder.";

    tl.append_delta("s1", Some("t1"), EntryKind::ASSISTANT_TEXT, PRE);
    tl.finalize_assistant("s1", "t1", PERSISTED);
    // Deltas that arrive AFTER the receipt (the real interleaving).
    tl.append_delta("s1", Some("t1"), EntryKind::ASSISTANT_TEXT, LATE);

    assert_eq!(
        store.live_text("s1"),
        PERSISTED,
        "the canonical persisted body must win over later deltas"
    );
    assert_eq!(
        tl.entries("s1").len(),
        1,
        "a late delta must fold into the finalized entry, not open a second one"
    );

    // A delta after the turn ALSO closed must still be dropped: the web drops
    // every delta once the turn has a terminal (`timeline/model.ts:487-491`), so
    // the receipt keeps the row even past the terminal.
    tl.close_turn("s1", "t1");
    tl.append_delta("s1", Some("t1"), EntryKind::ASSISTANT_TEXT, LATE);
    assert_eq!(
        store.live_text("s1"),
        PERSISTED,
        "a post-terminal delta must not resurrect a tail row"
    );
    assert_eq!(tl.entries("s1").len(), 1, "still exactly one assistant entry");
}

#[test]
fn entry_kind_tags_are_lane_extensible_and_declared_kinds_are_complete() {
    // The twelve parity-matrix kinds are declared once, here:
    assert_eq!(EntryKind::DECLARED.len(), 12);
    assert!(EntryKind::DECLARED.contains(&EntryKind::ASSISTANT_TEXT));
    assert!(EntryKind::DECLARED.contains(&EntryKind::ATTACHMENT));

    // A domain declares its OWN kind in its own file — no shared enum edit:
    const MY_KIND: EntryKind = EntryKind::new("my.domain.thing");
    let store = Store::new();
    store
        .domains
        .session
        .timeline
        .append("s", None, MY_KIND, "hello".into());
    let got = store.domains.session.timeline.of_kind("s", MY_KIND);
    assert_eq!(got.len(), 1);
    assert_eq!(got[0].kind.tag(), "my.domain.thing");
}

#[test]
fn appending_with_a_payload_keeps_structured_detail() {
    let store = Store::new();
    store.domains.session.timeline.append_data(
        "s",
        Some("t1".to_owned()),
        EntryKind::TOOL_CALL,
        "bash".into(),
        serde_json::json!({"name": "bash", "args": {"cmd": "ls"}}),
    );
    let entries = store.domains.session.timeline.of_kind("s", EntryKind::TOOL_CALL);
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].data["args"]["cmd"], "ls");

    // Entries get monotonic ids.
    let id2 = store
        .domains
        .session
        .timeline
        .append("s", None, EntryKind::SYSTEM_NOTICE, "x".into());
    assert!(id2 > entries[0].id);
}

// -------------------------------------------------------------- other domains

#[test]
fn tool_inventory_round_trips() {
    let store = Store::new();
    assert_eq!(store.domains.tool.count(), 0);
    store.domains.tool.set(vec![RuntimeTool {
        name: "bash".into(),
        category: Some("exec".into()),
        status: Some("available".into()),
        policy: None,
        aliases: vec!["shell".into()],
    }]);
    assert_eq!(store.domains.tool.count(), 1);
    assert_eq!(store.domains.tool.list()[0].aliases, vec!["shell".to_owned()]);
}

#[test]
fn approval_decide_marks_the_right_row() {
    let store = Store::new();
    store.domains.approval.push(PendingApproval {
        id: "a1".into(),
        target: Some("rm -rf".into()),
        decided: false,
        auto_resolved: false,
        cancelled: false,
    });
    assert!(store.domains.approval.decide("a1"));
    assert!(store.domains.approval.pending()[0].decided);
    assert!(!store.domains.approval.decide("nope"), "an unknown id is not an error");
}

#[test]
fn stub_domains_hold_their_shape() {
    let store = Store::new();

    store.domains.task.upsert(Task {
        id: "task1".into(),
        title: Some("build".into()),
        state: Some("running".into()),
    });
    assert_eq!(store.domains.task.count(), 1);

    store.domains.autonomy.upsert(AutonomyEntity {
        id: "loop1".into(),
        kind: "loop".into(),
        detail: None,
    });
    store.domains.autonomy.upsert(AutonomyEntity {
        id: "mon1".into(),
        kind: "monitor".into(),
        detail: None,
    });
    assert_eq!(store.domains.autonomy.of_kind("loop").len(), 1);
    assert_eq!(store.domains.autonomy.count(), 2);

    store.domains.peer.stage("Edison".into());
    store.domains.peer.stage("Tesla".into());
    store.domains.peer.close("Tesla");
    assert_eq!(store.domains.peer.open_count(), 1);
    assert!(store.domains.peer.list().iter().any(|p: &Peer| p.name == "Tesla" && p.closed));

    store.domains.profile.set_current("octoscode".into());
    store.domains.profile.set_providers(vec!["anthropic".into()]);
    assert_eq!(store.domains.profile.current().as_deref(), Some("octoscode"));
    assert_eq!(store.domains.profile.providers().len(), 1);

    store.domains.media.upsert(MediaItem {
        id: "img1".into(),
        kind: "image".into(),
        reference: Some("/api/files/img1".into()),
    });
    assert_eq!(store.domains.media.count(), 1);

    store.domains.review.note_started("r1".into());
    assert_eq!(store.domains.review.last_started().as_deref(), Some("r1"));

    assert_eq!(autonomy_entity_count(&store), 2);
}

fn autonomy_entity_count(store: &Store) -> usize {
    store.domains.autonomy.count()
}

// -------------------------------------------------------------- diagnostics

#[test]
fn diagnostics_counts_by_method() {
    let store = Store::new();
    store.note_seen("message/delta");
    store.note_seen("message/delta");
    store.note_seen("turn/started");

    assert_eq!(store.seen_count("message/delta"), 2);
    assert_eq!(store.seen_count("never/seen"), 0);
    assert_eq!(store.diagnostics.total(), 3);
    assert_eq!(store.diagnostics.methods(), vec!["message/delta", "turn/started"]);
}
