//! `visual/*`, `voice/*`, `content/*`, `file/attached`, `smart_home/*` — media.
//!
//! Requests: `content/list`, `content/delete`, `content/bulk_delete`,
//! `smart_home/*` (stubs — not this card's methods).
//! This card's notification surface: `visual/generating`, `visual/succeeded`,
//! `visual/failed`, `voice/audio_chunk`, `voice/exit`, `file/attached`
//! (`octos-core crates/octos-core/src/ui_protocol.rs:5221/5236/5252/6364/5269/6335`
//! @ pin `a6ea8505`).
//!
//! Each handler writes the store's `media` domain
//! (`crates/octoscode-store/src/domains/media.rs`) and then records the arrival
//! via `store.note_seen`, so a diagnostic tile (or a test) can see it.
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octos_core::ui_protocol::methods;
use octoscode_store::domains::media::MediaItem;
use octoscode_store::Store;

use crate::registry::{NotificationHandler, Registry};

/// `visual/generating` — the background visual task opened a placeholder.
pub struct VisualGeneratingHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for VisualGeneratingHandler {
    const METHOD: &'static str = methods::VISUAL_GENERATING;
    fn handle(&self, notification: &UiNotification) {
        let UiNotification::VisualGenerating(event) = notification else {
            return;
        };
        self.store.domains.media.visual_generating(
            &event.session_id.0,
            &event.turn_id.0.to_string(),
            &event.kind,
        );
        self.store.note_seen(Self::METHOD);
    }
}

/// `visual/succeeded` — the artifacts landed; the client clears the placeholder.
pub struct VisualSucceededHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for VisualSucceededHandler {
    const METHOD: &'static str = methods::VISUAL_SUCCEEDED;
    fn handle(&self, notification: &UiNotification) {
        let UiNotification::VisualSucceeded(event) = notification else {
            return;
        };
        let turn_id = event.turn_id.0.to_string();
        self.store.domains.media.visual_succeeded(
            &event.session_id.0,
            &turn_id,
            &event.kind,
            event.files.clone(),
        );
        // Each delivered artifact is also a media item (the same paths the
        // accompanying `file/attached` events carry).
        for file in &event.files {
            self.store.domains.media.upsert(MediaItem {
                id: file.clone(),
                kind: event.kind.clone(),
                reference: Some(file.clone()),
            });
        }
        self.store.note_seen(Self::METHOD);
    }
}

/// `visual/failed` — the visual task failed or timed out.
pub struct VisualFailedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for VisualFailedHandler {
    const METHOD: &'static str = methods::VISUAL_FAILED;
    fn handle(&self, notification: &UiNotification) {
        let UiNotification::VisualFailed(event) = notification else {
            return;
        };
        self.store
            .domains
            .media
            .visual_failed(&event.turn_id.0.to_string(), event.reason.clone());
        self.store.note_seen(Self::METHOD);
    }
}

/// `voice/audio_chunk` — one streamed voice-reply frame.
pub struct VoiceAudioChunkHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for VoiceAudioChunkHandler {
    const METHOD: &'static str = methods::VOICE_AUDIO_CHUNK;
    fn handle(&self, notification: &UiNotification) {
        let UiNotification::VoiceAudioChunk(event) = notification else {
            return;
        };
        // Framing only: the base64 payload is played live and never retained.
        self.store.domains.media.voice_chunk(
            &event.session_id.0,
            &event.turn_id.0.to_string(),
            &event.segment_id,
            event.seq,
            &event.mime,
            event.last,
        );
        self.store.note_seen(Self::METHOD);
    }
}

/// `voice/exit` — the voice turn detected an end / goodbye / mute intent.
pub struct VoiceExitHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for VoiceExitHandler {
    const METHOD: &'static str = methods::VOICE_EXIT;
    fn handle(&self, notification: &UiNotification) {
        let UiNotification::VoiceExit(event) = notification else {
            return;
        };
        self.store
            .domains
            .media
            .note_voice_exit(&event.turn_id.0.to_string());
        self.store.note_seen(Self::METHOD);
    }
}

/// `file/attached` — an artifact the tool produced (path or URL).
pub struct FileAttachedHandler {
    pub store: Arc<Store>,
}
impl NotificationHandler for FileAttachedHandler {
    const METHOD: &'static str = methods::FILE_ATTACHED;
    fn handle(&self, notification: &UiNotification) {
        let UiNotification::FileAttached(event) = notification else {
            return;
        };
        self.store.domains.media.upsert(MediaItem {
            id: event.path.clone(),
            // The MIME hint when the tool gave one; `file` otherwise.
            kind: event.mime.clone().unwrap_or_else(|| "file".to_owned()),
            reference: Some(event.path.clone()),
        });
        self.store.note_seen(Self::METHOD);
    }
}

pub fn register(reg: &mut Registry, store: Arc<Store>) {
    reg.register(VisualGeneratingHandler { store: store.clone() });
    reg.register(VisualSucceededHandler { store: store.clone() });
    reg.register(VisualFailedHandler { store: store.clone() });
    reg.register(VoiceAudioChunkHandler { store: store.clone() });
    reg.register(VoiceExitHandler { store: store.clone() });
    reg.register(FileAttachedHandler { store });
}
