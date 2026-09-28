//! F6 — store-level tests for the `peer` and `media` domains.
//!
//! The client's `f6_media.rs` / `f6_peer.rs` prove the notification → registry
//! → store path; these exercise the domain state directly (no transport),
//! which is what makes the fan-out shape verifiable per the store recipe.
use octoscode_store::domains::media::VisualState;
use octoscode_store::domains::peer::Peer;
use octoscode_store::Store;

// ------------------------------------------------------------------- peer

#[test]
fn a_staged_peer_carries_its_routing_facts() {
    let store = Store::new();
    store.domains.peer.observe_staged(Peer {
        name: "edison".into(),
        closed: false,
        topic: Some("peer-edison".into()),
        profile_id: Some("octoscode".into()),
        origin_session_id: Some("octoscode:main".into()),
        brief_path: Some("/tmp/peers/edison/brief.md".into()),
        cwd: Some("/tmp/peers/edison".into()),
        worktree_branch: Some("peer/edison".into()),
    });
    let row = store.domains.peer.get("edison").expect("the staged peer");
    assert_eq!(row.topic.as_deref(), Some("peer-edison"));
    assert_eq!(row.origin_session_id.as_deref(), Some("octoscode:main"));
    assert_eq!(row.worktree_branch.as_deref(), Some("peer/edison"));
    assert_eq!(store.domains.peer.open_count(), 1);
}

#[test]
fn replayed_staged_does_not_reopen_a_closed_peer() {
    let store = Store::new();
    store.domains.peer.observe_staged(Peer::named("edison"));
    store.domains.peer.mark_closed("edison");
    assert_eq!(store.domains.peer.open_count(), 0);
    // The durable event replays after the close.
    store.domains.peer.observe_staged(Peer::named("edison"));
    assert!(store.domains.peer.get("edison").unwrap().closed);
    assert_eq!(store.domains.peer.open_count(), 0);
}

#[test]
fn closing_an_unseen_slug_still_records_it_closed() {
    let store = Store::new();
    store.domains.peer.mark_closed("ghost");
    let row = store.domains.peer.get("ghost").expect("recorded closed");
    assert!(row.closed);
    assert_eq!(store.domains.peer.open_count(), 0);
}

#[test]
fn the_roster_indexes_by_slug_not_by_order() {
    let store = Store::new();
    store.domains.peer.observe_staged(Peer::named("tesla"));
    store.domains.peer.observe_staged(Peer::named("edison"));
    store.domains.peer.mark_closed("tesla");
    let roster = store.domains.peer.list();
    assert_eq!(roster.iter().map(|p| p.name.as_str()).collect::<Vec<_>>(), vec!["edison", "tesla"]);
    assert_eq!(store.domains.peer.open_count(), 1);
}

// ------------------------------------------------------------------ media

#[test]
fn a_visual_task_walks_generating_to_succeeded() {
    let store = Store::new();
    store.domains.media.visual_generating("s1", "t1", "image");
    assert_eq!(store.domains.media.visual("t1").unwrap().state, VisualState::Generating);
    store.domains.media.visual_succeeded("s1", "t1", "image", vec!["a.png".into()]);
    let task = store.domains.media.visual("t1").unwrap();
    assert_eq!(task.state, VisualState::Succeeded);
    assert_eq!(task.files, vec!["a.png".to_owned()]);
}

#[test]
fn failing_an_unknown_visual_task_is_a_no_op_not_a_panic() {
    let store = Store::new();
    store.domains.media.visual_failed("never-seen", Some("timeout".into()));
    assert!(store.domains.media.visual("never-seen").is_none());
    assert_eq!(store.domains.media.visual_count(), 0);
}

#[test]
fn voice_chunks_fold_into_one_segment_with_framing() {
    let store = Store::new();
    for (seq, last) in [(0u32, false), (1, true)] {
        store.domains.media.voice_chunk("s1", "t1", "seg", seq, "audio/mpeg", last);
    }
    let seg = store.domains.media.voice_segment("seg").unwrap();
    assert_eq!(seg.chunks, 2);
    assert_eq!(seg.last_seq, 1);
    assert!(seg.complete);
    assert_eq!(store.domains.media.voice_segment_count(), 1);
}

#[test]
fn voice_exits_are_deduped_per_turn() {
    let store = Store::new();
    store.domains.media.note_voice_exit("t1");
    store.domains.media.note_voice_exit("t1");
    store.domains.media.note_voice_exit("t2");
    assert_eq!(store.domains.media.voice_exits(), 2);
}
