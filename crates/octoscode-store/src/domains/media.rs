//! `media` state: generated images, voice, content gallery, attachments.
//!
//! Stub for the fan-out lane (`visual/*`, `voice/*`, `content/*`,
//! `file/attached`, `smart_home/*`).
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

/// The media domain.
#[derive(Debug, Default)]
pub struct Media {
    inner: Mutex<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    items: HashMap<String, MediaItem>,
}

impl Media {
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
}
