//! `media` state: generated images, voice, content gallery, attachments.
//!
//! Fed by the client domain's media handlers
//! (`crates/octoscode-client/src/domains/media.rs`). Three shapes:
//!
//! - [`MediaItem`] — an artifact row (`file/attached`, gallery entries). The
//!   first card's shape, kept verbatim.
//! - [`VisualTask`] — the background visual placeholder: `visual/generating`
//!   opens it, `visual/succeeded` / `visual/failed` close it
//!   (`octos-core ui_protocol.rs:5221/5236/5252` @ pin `a6ea8505`).
//! - [`VoiceSegment`] — streamed voice framing (`voice/audio_chunk`,
//!   `:6364`). The audio BYTES are deliberately NOT retained (the payload is
//!   large and the UI plays it live); only the segment's framing is kept.
use std::collections::HashMap;
use std::sync::Mutex;

/// One media item (an image, a voice chunk reference, a gallery entry).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaItem {
    pub id: String,
    pub kind: String,
    /// A URL or a server path; the UI decides how to fetch it.
    pub reference: Option<String>,
}

/// Lifecycle of one background visual task.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualState {
    Generating,
    Succeeded,
    Failed,
}

/// A background visual task, keyed by its turn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VisualTask {
    pub turn_id: String,
    pub session_id: String,
    /// `html` | `illustrated` | `image` | `infographic`.
    pub kind: String,
    pub state: VisualState,
    /// Workspace-relative artifact filenames (`visual/succeeded`).
    pub files: Vec<String>,
    /// Failure reason (`visual/failed`), when the server gave one.
    pub reason: Option<String>,
}

/// One streamed voice utterance (chunks sharing a `segment_id`).
///
/// Framing only — never the base64 audio itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceSegment {
    pub segment_id: String,
    pub session_id: String,
    pub turn_id: String,
    /// MIME type of the audio bytes, e.g. `audio/mpeg`.
    pub mime: String,
    /// How many chunks have arrived.
    pub chunks: u32,
    /// Highest `seq` seen so far.
    pub last_seq: u32,
    /// True once a chunk with `last: true` arrived.
    pub complete: bool,
}

/// The media domain.
#[derive(Debug, Default)]
pub struct Media {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    items: HashMap<String, MediaItem>,
    visuals: HashMap<String, VisualTask>,
    voice: HashMap<String, VoiceSegment>,
    voice_exits: Vec<String>,
}

impl Media {
    // ---- artifacts (first card's shape; kept) ----------------------------

    pub fn upsert(&self, item: MediaItem) {
        self.inner.lock().unwrap().items.insert(item.id.clone(), item);
    }

    pub fn list(&self) -> Vec<MediaItem> {
        let i = self.inner.lock().unwrap();
        let mut v: Vec<MediaItem> = i.items.values().cloned().collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().items.len()
    }

    // ---- visual tasks ----------------------------------------------------

    /// `visual/generating` — open (or reopen) the placeholder for `turn_id`.
    pub fn visual_generating(&self, session_id: &str, turn_id: &str, kind: &str) {
        self.inner.lock().unwrap().visuals.insert(
            turn_id.to_owned(),
            VisualTask {
                turn_id: turn_id.to_owned(),
                session_id: session_id.to_owned(),
                kind: kind.to_owned(),
                state: VisualState::Generating,
                files: Vec::new(),
                reason: None,
            },
        );
    }

    /// `visual/succeeded` — record the delivered artifacts and close the task.
    pub fn visual_succeeded(
        &self,
        session_id: &str,
        turn_id: &str,
        kind: &str,
        files: Vec<String>,
    ) {
        let mut i = self.inner.lock().unwrap();
        let task = i.visuals.entry(turn_id.to_owned()).or_insert_with(|| VisualTask {
            turn_id: turn_id.to_owned(),
            session_id: session_id.to_owned(),
            kind: kind.to_owned(),
            state: VisualState::Generating,
            files: Vec::new(),
            reason: None,
        });
        task.kind = kind.to_owned();
        task.state = VisualState::Succeeded;
        task.files = files;
    }

    /// `visual/failed` — close the placeholder with a reason (if any).
    pub fn visual_failed(&self, turn_id: &str, reason: Option<String>) {
        let mut i = self.inner.lock().unwrap();
        if let Some(task) = i.visuals.get_mut(turn_id) {
            task.state = VisualState::Failed;
            task.reason = reason;
        }
    }

    pub fn visual(&self, turn_id: &str) -> Option<VisualTask> {
        self.inner.lock().unwrap().visuals.get(turn_id).cloned()
    }

    pub fn visual_count(&self) -> usize {
        self.inner.lock().unwrap().visuals.len()
    }

    // ---- voice -----------------------------------------------------------

    /// One `voice/audio_chunk`: accumulate the segment's framing.
    pub fn voice_chunk(
        &self,
        session_id: &str,
        turn_id: &str,
        segment_id: &str,
        seq: u32,
        mime: &str,
        last: bool,
    ) {
        let mut i = self.inner.lock().unwrap();
        let seg = i.voice.entry(segment_id.to_owned()).or_insert_with(|| VoiceSegment {
            segment_id: segment_id.to_owned(),
            session_id: session_id.to_owned(),
            turn_id: turn_id.to_owned(),
            mime: mime.to_owned(),
            chunks: 0,
            last_seq: 0,
            complete: false,
        });
        seg.mime = mime.to_owned();
        seg.chunks += 1;
        seg.last_seq = seg.last_seq.max(seq);
        if last {
            seg.complete = true;
        }
    }

    pub fn voice_segment(&self, segment_id: &str) -> Option<VoiceSegment> {
        self.inner.lock().unwrap().voice.get(segment_id).cloned()
    }

    pub fn voice_segment_count(&self) -> usize {
        self.inner.lock().unwrap().voice.len()
    }

    /// `voice/exit` — the voice turn detected an end intent for `turn_id`.
    pub fn note_voice_exit(&self, turn_id: &str) {
        let mut i = self.inner.lock().unwrap();
        if !i.voice_exits.iter().any(|t| t == turn_id) {
            i.voice_exits.push(turn_id.to_owned());
        }
    }

    pub fn voice_exits(&self) -> usize {
        self.inner.lock().unwrap().voice_exits.len()
    }
}
