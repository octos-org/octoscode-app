//! F6 — media notification handlers, one fixture per method, straight into the
//! store (no transport, no socket).
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::domains::media::VisualState;
use octoscode_store::Store;

use octoscode_client::domains;
use octoscode_client::Registry;

fn notification(method: &str, params: serde_json::Value) -> UiNotification {
    UiNotification::from_method_and_params(method, params)
        .expect("the test's params decode for this method")
}

fn registered_registry(store: &Arc<Store>) -> Registry {
    let mut reg = Registry::new();
    domains::media::register(&mut reg, store.clone());
    reg
}

const TURN: &str = "00000000-0000-7000-8000-0000000000f6";

#[test]
fn all_six_media_notifications_are_wired() {
    let store = Arc::new(Store::new());
    let reg = registered_registry(&store);
    for m in [
        methods::VISUAL_GENERATING,
        methods::VISUAL_SUCCEEDED,
        methods::VISUAL_FAILED,
        methods::VOICE_AUDIO_CHUNK,
        methods::VOICE_EXIT,
        methods::FILE_ATTACHED,
    ] {
        assert!(reg.handles(m), "expected a handler for {m}");
    }
}

#[test]
fn visual_generating_opens_a_placeholder() {
    let store = Arc::new(Store::new());
    let mut reg = registered_registry(&store);
    let n = notification(
        methods::VISUAL_GENERATING,
        serde_json::json!({"session_id": "s1", "turn_id": TURN, "kind": "image"}),
    );
    assert!(reg.dispatch(&n));
    let task = store.domains.media.visual(TURN).expect("a visual task");
    assert_eq!(task.state, VisualState::Generating);
    assert_eq!(task.kind, "image");
    assert_eq!(task.session_id, "s1");
    assert_eq!(store.seen_count(methods::VISUAL_GENERATING), 1);
}

#[test]
fn visual_succeeded_closes_the_task_and_records_artifacts() {
    let store = Arc::new(Store::new());
    let mut reg = registered_registry(&store);
    assert!(reg.dispatch(&notification(
        methods::VISUAL_GENERATING,
        serde_json::json!({"session_id": "s1", "turn_id": TURN, "kind": "illustrated"}),
    )));
    assert!(reg.dispatch(&notification(
        methods::VISUAL_SUCCEEDED,
        serde_json::json!({
            "session_id": "s1",
            "turn_id": TURN,
            "kind": "illustrated",
            "files": ["art/a.png", "art/b.png"]
        }),
    )));
    let task = store.domains.media.visual(TURN).unwrap();
    assert_eq!(task.state, VisualState::Succeeded);
    assert_eq!(task.files, vec!["art/a.png".to_owned(), "art/b.png".to_owned()]);
    // Both delivered artifacts are media items too.
    assert_eq!(store.domains.media.count(), 2);
}

#[test]
fn visual_failed_marks_the_task_failed_with_its_reason() {
    let store = Arc::new(Store::new());
    let mut reg = registered_registry(&store);
    assert!(reg.dispatch(&notification(
        methods::VISUAL_GENERATING,
        serde_json::json!({"session_id": "s1", "turn_id": TURN, "kind": "html"}),
    )));
    assert!(reg.dispatch(&notification(
        methods::VISUAL_FAILED,
        serde_json::json!({"session_id": "s1", "turn_id": TURN, "reason": "timeout"}),
    )));
    let task = store.domains.media.visual(TURN).unwrap();
    assert_eq!(task.state, VisualState::Failed);
    assert_eq!(task.reason.as_deref(), Some("timeout"));
}

#[test]
fn voice_audio_chunks_accumulate_one_segment() {
    let store = Arc::new(Store::new());
    let mut reg = registered_registry(&store);
    // Three frames, seq out of order on the third, last on the third.
    for (seq, last) in [(0u32, false), (1, false), (2, true)] {
        assert!(reg.dispatch(&notification(
            methods::VOICE_AUDIO_CHUNK,
            serde_json::json!({
                "session_id": "s1",
                "turn_id": TURN,
                "segment_id": "seg1",
                "seq": seq,
                "mime": "audio/mpeg",
                "audio_b64": "AAAA",
                "last": last
            }),
        )));
    }
    let seg = store.domains.media.voice_segment("seg1").expect("a segment");
    assert_eq!(seg.chunks, 3);
    assert_eq!(seg.last_seq, 2);
    assert!(seg.complete);
    assert_eq!(seg.mime, "audio/mpeg");
    assert_eq!(store.domains.media.voice_segment_count(), 1);
    assert_eq!(store.seen_count(methods::VOICE_AUDIO_CHUNK), 3);
}

#[test]
fn voice_exit_is_recorded_once_per_turn() {
    let store = Arc::new(Store::new());
    let mut reg = registered_registry(&store);
    let exit = || {
        notification(
            methods::VOICE_EXIT,
            serde_json::json!({"session_id": "s1", "turn_id": TURN}),
        )
    };
    assert!(reg.dispatch(&exit()));
    assert!(reg.dispatch(&exit()));
    assert_eq!(store.domains.media.voice_exits(), 1);
    assert_eq!(store.seen_count(methods::VOICE_EXIT), 2);
}

#[test]
fn file_attached_becomes_a_media_item() {
    let store = Arc::new(Store::new());
    let mut reg = registered_registry(&store);
    assert!(reg.dispatch(&notification(
        methods::FILE_ATTACHED,
        serde_json::json!({
            "session_id": "s1",
            "turn_id": TURN,
            "path": "out/report.pdf",
            "mime": "application/pdf"
        }),
    )));
    let items = store.domains.media.list();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "out/report.pdf");
    assert_eq!(items[0].kind, "application/pdf");
    assert_eq!(store.seen_count(methods::FILE_ATTACHED), 1);
}

#[test]
fn file_attached_without_a_mime_falls_back_to_a_generic_kind() {
    let store = Arc::new(Store::new());
    let mut reg = registered_registry(&store);
    assert!(reg.dispatch(&notification(
        methods::FILE_ATTACHED,
        serde_json::json!({"session_id": "s1", "turn_id": TURN, "path": "out/x"}),
    )));
    assert_eq!(store.domains.media.list()[0].kind, "file");
}
