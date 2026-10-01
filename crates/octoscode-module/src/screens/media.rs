//! P4d4 — media: the per-Session attachment draft store, ported field-by-field
//! from the web oracle
//! (`apps/web/src/features/media/attachment-drafts.ts:1-347`).
//!
//! The rules that matter, and that this file keeps:
//! * **Selection never uploads.** Files live only in this bounded, in-memory
//!   draft; nothing is read, decoded or sent until the EXPLICIT upload action
//!   (attachment-drafts.ts:135-140, 54-59).
//! * **Dedup on the (name, size, mime) triple** — re-adding an existing draft is
//!   an idempotent no-op (attachment-drafts.ts:142-150).
//! * **Selection is atomic**: an invalid or oversized batch leaves the existing
//!   draft intact (attachment-drafts.ts:129-131).
//! * **Same-tick upload clicks cannot duplicate work** (attachment-drafts.ts:167-168).
//! * **The late-receipt guard**: after every await the upload re-checks that its
//!   entry is still current, so removing an in-flight entry IGNORES a receipt
//!   that lands later (attachment-drafts.ts:196-215, `current()`).
//! * **Receipt validation**: a receipt must name this Profile's uploaded handle
//!   and match the draft's mime and byte count (attachment-drafts.ts:203-208).
//! * **Consumed only at an ACCEPTED local enqueue boundary** — a rejected queue
//!   keeps the complete draft (attachment-drafts.ts:264-289, `submitTurn`).
//! * **Authority is checked before the media is handed over**; a mismatched
//!   scope is an error, never a cross-Session attachment
//!   (attachment-drafts.ts:308-318, `#validatedMedia`).
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use octoscode_store::Store;

pub const MAX_TURN_IMAGES: usize = 4;
pub const MAX_IMAGE_BYTES: u64 = 20 * 1024 * 1024;
pub const IMAGE_ACCEPT: &str = ".png,.jpg,.jpeg,.gif,.webp";

/// The opaque authenticated runtime identity; never a credential
/// (`AttachmentScope`, attachment-drafts.ts:11-15).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentScope {
    pub authority_key: String,
    pub session_id: String,
    pub profile_id: String,
}

impl AttachmentScope {
    pub fn is_complete(&self) -> bool {
        [&self.authority_key, &self.session_id, &self.profile_id]
            .iter()
            .all(|v| !v.trim().is_empty())
    }

    /// The scope identity check (`#validatedMedia`, attachment-drafts.ts:308-313).
    pub fn matches(&self, other: &AttachmentScope) -> bool {
        self.authority_key == other.authority_key
            && self.session_id == other.session_id
            && self.profile_id == other.profile_id
    }
}

/// A draft's status (`AttachmentDraft.status`, attachment-drafts.ts:24).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DraftStatus {
    Selected,
    Uploading,
    Ready,
    Error,
}

impl DraftStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Selected => "selected",
            Self::Uploading => "uploading",
            Self::Ready => "ready",
            Self::Error => "error",
        }
    }
}

/// One attachment draft (`AttachmentDraft`, attachment-drafts.ts:19-26).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentDraft {
    pub id: String,
    pub name: String,
    pub bytes: u64,
    pub mime: String,
    pub status: DraftStatus,
    pub error: Option<String>,
}

/// The uploaded media this draft holds — the `FileRef` shape
/// (`TurnMedia`, attachment-drafts.ts / octos-core `ui_protocol.rs:3916`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TurnMedia {
    pub path: String,
    pub mime: String,
    pub size_bytes: u64,
}

impl TurnMedia {
    pub fn to_value(&self) -> Value {
        serde_json::json!({
            "path": self.path,
            "mime": self.mime,
            "size_bytes": self.size_bytes,
        })
    }
}

/// The extension-derived mime + matching-type check (`imageMime`,
/// attachment-drafts.ts:38-52). The type is derived from the EXTENSION, and a
/// declared browser type that disagrees is a refusal, not a silent override.
pub fn image_mime(name: &str, declared: Option<&str>) -> Result<String, String> {
    let lower = name.to_lowercase();
    let ext = [".png", ".jpg", ".jpeg", ".gif", ".webp"]
        .into_iter()
        .find(|e| lower.ends_with(e))
        .map(|e| e.trim_start_matches('.').to_owned());
    let mime = match ext.as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg".to_owned(),
        Some(e) => format!("image/{e}"),
        None => {
            return Err("Choose PNG, JPEG, GIF or WebP images with matching file types.".to_owned())
        }
    };
    if let Some(declared) = declared.filter(|d| !d.is_empty()) {
        if !declared.eq_ignore_ascii_case(&mime) {
            return Err("Choose PNG, JPEG, GIF or WebP images with matching file types.".to_owned());
        }
    }
    Ok(mime)
}

/// The size gate (`imageMime`, attachment-drafts.ts:49-51).
pub fn check_size(bytes: u64) -> Result<(), String> {
    if bytes == 0 || bytes > MAX_IMAGE_BYTES {
        return Err("Each image must be nonempty and at most 20 MiB.".to_owned());
    }
    Ok(())
}

/// `uploadedHandleForProfile` (packages/client/src/media.ts:38-72): the handle
/// must be `up/<base64url of an absolute path>/<opaque>`, the decoded path must
/// start with the profile, and no component may be empty, `.`, `..` or a
/// control character.
pub fn uploaded_handle_for_profile(value: &str, profile_id: &str) -> bool {
    if value.len() > 16384 {
        return false;
    }
    let parts: Vec<&str> = value.split('/').collect();
    if parts.len() != 3 || parts[0] != "up" || parts[2].is_empty() {
        return false;
    }
    let b64 = parts[1];
    if b64.is_empty()
        || !b64
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return false;
    }
    // base64url -> bytes -> utf-8, the strict path check.
    let Some(decoded) = base64url_decode(b64) else { return false };
    let Some(text) = String::from_utf8(decoded).ok() else { return false };
    let normalized = text.replace('\\', "/");
    let path: Vec<&str> = normalized.split('/').collect();
    path.len() >= 2
        && path[0] == profile_id
        && path.iter().all(|c| {
            !c.is_empty() && *c != "." && *c != ".." && !c.chars().any(|ch| ch.is_control())
        })
}

/// Strict base64url decode (no padding tolerance: the web's `atob` is strict).
fn base64url_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buf: u32 = 0;
    let mut bits = 0u32;
    for c in input.chars() {
        let v = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '-' => 62,
            '_' => 63,
            _ => return None,
        };
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    if bits > 0 && (buf & ((1 << bits) - 1)) != 0 {
        return None; // trailing bits must be zero, like a strict decode
    }
    Some(out)
}

/// A file the person selected, held ONLY in the local draft.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalFile {
    pub name: String,
    pub bytes: u64,
    pub mime: String,
    /// The file's bytes, read only at the explicit upload action.
    pub content: Arc<Vec<u8>>,
}

#[derive(Debug, Clone)]
struct Entry {
    draft: AttachmentDraft,
    file: Option<LocalFile>,
    media: Option<TurnMedia>,
}

/// A bounded, in-memory draft store per persistent Session record
/// (`AttachmentDraftStore`, attachment-drafts.ts:61-94).
#[derive(Debug)]
pub struct AttachmentDraftStore {
    scope: AttachmentScope,
    /// The owning authority's negotiated TURN_START gate; **defaults closed**
    /// (attachment-drafts.ts:80).
    upload_available: bool,
    inner: Mutex<Inner>,
    next_id: AtomicU64,
    /// The per-entry abort the cancel arm sets, standing in for the web's
    /// `AbortController` (attachment-drafts.ts:69, 196).
    cancelled: Mutex<HashMap<String, Arc<AtomicBool>>>,
    disposed: AtomicBool,
}

#[derive(Debug, Default)]
struct Inner {
    entries: HashMap<String, Entry>,
    /// Insertion order, so the picker keeps selection order.
    order: Vec<String>,
    uploading: usize,
    /// Entries whose upload is already in flight: a second `begin_upload` for
    /// the same id is refused, so a same-tick repeat click cannot duplicate
    /// work (attachment-drafts.ts:182-186).
    claimed: HashSet<String>,
}

impl AttachmentDraftStore {
    /// Construct with a scope; a blank authority/session/profile is refused
    /// (attachment-drafts.ts:87-90).
    pub fn new(scope: AttachmentScope, upload_available: bool) -> Result<Self, String> {
        if !scope.is_complete() {
            return Err(
                "A confirmed attachment authority, Profile and Session are required.".to_owned(),
            );
        }
        Ok(Self {
            scope,
            upload_available,
            inner: Mutex::new(Inner::default()),
            next_id: AtomicU64::new(1),
            cancelled: Mutex::new(HashMap::new()),
            disposed: AtomicBool::new(false),
        })
    }

    pub fn scope(&self) -> &AttachmentScope {
        &self.scope
    }

    pub fn upload_available(&self) -> bool {
        self.upload_available
    }

    /// The rows, in selection order.
    pub fn entries(&self) -> Vec<AttachmentDraft> {
        let i = self.inner.lock().unwrap();
        i.order
            .iter()
            .filter_map(|id| i.entries.get(id).map(|e| e.draft.clone()))
            .collect()
    }

    pub fn uploading(&self) -> bool {
        self.inner.lock().unwrap().uploading > 0
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn disposed(&self) -> bool {
        self.disposed.load(Ordering::SeqCst)
    }

    /// `#assertCurrent` (attachment-drafts.ts:117-123): a retired authority
    /// invalidates the store and then refuses.
    fn assert_current(&self) -> Result<(), String> {
        if self.disposed() {
            return Err(
                "This attachment draft no longer belongs to a current Session authority."
                    .to_owned(),
            );
        }
        Ok(())
    }

    /// Atomic selection (attachment-drafts.ts:125-163): a rejected batch leaves
    /// the existing draft INTACT, and a duplicate (name,size,mime) is a no-op.
    pub fn select_files(&self, files: Vec<LocalFile>) -> Result<usize, String> {
        self.assert_current()?;
        if !self.upload_available {
            return Err("Image uploads are unavailable on this server.".to_owned());
        }
        let existing = self.entries();
        if files.len() + existing.len() > MAX_TURN_IMAGES {
            return Err("Attach at most four images per turn.".to_owned());
        }
        // Validate the WHOLE batch before mutating anything.
        let mut validated = Vec::new();
        for f in &files {
            let mime = image_mime(&f.name, Some(&f.mime))?;
            check_size(f.bytes)?;
            validated.push((f, mime));
        }
        let mut seen: Vec<String> = existing
            .iter()
            .map(|d| dedup_key(&d.name, d.bytes, &d.mime))
            .collect();
        let mut added = 0usize;
        let mut i = self.inner.lock().unwrap();
        for (file, mime) in validated {
            let key = dedup_key(&file.name, file.bytes, &mime);
            if seen.contains(&key) {
                continue; // idempotent re-add: no new draft, no new id
            }
            seen.push(key);
            added += 1;
            let id = self.next_id.fetch_add(1, Ordering::SeqCst).to_string();
            i.order.push(id.clone());
            i.entries.insert(
                id.clone(),
                Entry {
                    draft: AttachmentDraft {
                        id,
                        name: file.name.clone(),
                        bytes: file.bytes,
                        mime,
                        status: DraftStatus::Selected,
                        error: None,
                    },
                    file: Some(file.clone()),
                    media: None,
                },
            );
        }
        Ok(added)
    }

    /// The ids eligible for upload: only `selected` or `error`
    /// (`uploadSelected`, attachment-drafts.ts:165-180). Same-tick repeated
    /// clicks find the same set, and `#upload` refuses an entry already in
    /// flight, so work is never duplicated.
    pub fn upload_candidates(&self) -> Vec<String> {
        let i = self.inner.lock().unwrap();
        i.order
            .iter()
            .filter(|id| {
                i.entries.get(*id).map(|e| {
                    matches!(e.draft.status, DraftStatus::Selected | DraftStatus::Error)
                }) == Some(true)
            })
            .cloned()
            .collect()
    }

    /// Begin one upload: mark it `uploading` and hand back its abort token.
    /// Returns `Err` when the entry is not uploadable or is ALREADY in
    /// flight (attachment-drafts.ts:182-186) — that is the same-tick dedup.
    pub fn begin_upload(&self, id: &str) -> Result<(LocalFile, String, Arc<AtomicBool>), String> {
        self.assert_current()?;
        let abort = Arc::new(AtomicBool::new(false));
        {
            let mut i = self.inner.lock().unwrap();
            if i.claimed.contains(id) {
                return Err("This attachment is already uploading.".to_owned());
            }
            let Some(entry) = i.entries.get_mut(id) else {
                return Err("The attachment is no longer selected.".to_owned());
            };
            if !matches!(entry.draft.status, DraftStatus::Selected | DraftStatus::Error) {
                return Err("The attachment is no longer selected.".to_owned());
            }
            entry.draft.status = DraftStatus::Uploading;
            entry.draft.error = None;
            let file = entry.file.clone().ok_or_else(|| {
                "The attachment is no longer selected.".to_owned()
            })?;
            let mime = entry.draft.mime.clone();
            i.uploading += 1;
            i.claimed.insert(id.to_owned());
            self.cancelled.lock().unwrap().insert(id.to_owned(), abort.clone());
            return Ok((file, mime, abort));
        }
    }

    /// Validate a receipt and fold it in (`#upload`, attachment-drafts.ts:196-215).
    /// Returns `Ok(true)` when the receipt was APPLIED, `Ok(false)` when it was
    /// IGNORED because the entry is no longer current (removed/cancelled) —
    /// the late-receipt guard.
    pub fn complete_upload(&self, id: &str, media: &TurnMedia) -> Result<bool, String> {
        if self.disposed() {
            return Ok(false);
        }
        // Read the abort signal WITHOUT consuming it: the web's `current()`
        // re-checks it after every await (attachment-drafts.ts:196-215).
        if let Some(abort) = self.cancelled.lock().unwrap().get(id) {
            if abort.load(Ordering::SeqCst) {
                self.settle(id);
                return Ok(false); // cancelled: IGNORE the late receipt
            }
        }
        self.cancelled.lock().unwrap().remove(id);
        let mut i = self.inner.lock().unwrap();
        Self::settle_locked(&mut i, id);
        let Some(entry) = i.entries.get_mut(id) else {
            return Ok(false); // removed mid-flight: IGNORE the late receipt
        };
        // Receipt validation (attachment-drafts.ts:203-208).
        if !uploaded_handle_for_profile(&media.path, &self.scope.profile_id)
            || media.mime != entry.draft.mime
            || media.size_bytes != entry.draft.bytes
        {
            entry.draft.status = DraftStatus::Error;
            entry.draft.error = Some("Invalid attachment receipt".to_owned());
            return Err("Invalid attachment receipt".to_owned());
        }
        entry.media = Some(media.clone());
        entry.file = None; // the local bytes are released
        entry.draft.status = DraftStatus::Ready;
        entry.draft.error = None;
        Ok(true)
    }

    /// Drop the in-flight bookkeeping for `id` (the claim + the counter).
    fn settle(&self, id: &str) {
        let mut i = self.inner.lock().unwrap();
        Self::settle_locked(&mut i, id);
    }

    fn settle_locked(i: &mut Inner, id: &str) {
        i.claimed.remove(id);
        i.uploading = i.uploading.saturating_sub(1);
    }

    /// An upload that failed: mark the entry `error` so it can be retried
    /// (`#upload`'s catch arm, attachment-drafts.ts:216-222).
    pub fn fail_upload(&self, id: &str, reason: &str) {
        self.cancelled.lock().unwrap().remove(id);
        let mut i = self.inner.lock().unwrap();
        Self::settle_locked(&mut i, id);
        if let Some(entry) = i.entries.get_mut(id) {
            entry.draft.status = DraftStatus::Error;
            entry.draft.error = Some(reason.to_owned());
        }
    }

    /// Removes the local draft reference only; NEVER deletes a server file
    /// (`remove`, attachment-drafts.ts:255-260).
    pub fn remove(&self, id: &str) {
        if let Some(abort) = self.cancelled.lock().unwrap().remove(id) {
            abort.store(true, Ordering::SeqCst); // cancel an in-flight upload
        }
        let mut i = self.inner.lock().unwrap();
        i.claimed.remove(id);
        i.entries.remove(id);
        i.order.retain(|x| x != id);
    }

    /// Synchronous local admission: a REJECTED queue keeps the complete draft
    /// (`submitTurn`, attachment-drafts.ts:281-289).
    pub fn submit_turn(
        &self,
        expected: &AttachmentScope,
        admit: impl FnOnce(&[TurnMedia]) -> bool,
    ) -> Result<bool, String> {
        let media = self.validated_media(expected)?;
        if !admit(&media) {
            return Ok(false); // draft intact
        }
        self.clear_entries();
        Ok(true)
    }

    /// Take the media at an ACCEPTED enqueue boundary
    /// (`takeForTurn`, attachment-drafts.ts:264-272). The caller must attach
    /// the returned batch to that immutable turn.
    pub fn take_for_turn(&self, expected: &AttachmentScope) -> Result<Vec<TurnMedia>, String> {
        let media = self.validated_media(expected)?;
        self.clear_entries();
        Ok(media)
    }

    /// `#validatedMedia` (attachment-drafts.ts:308-318).
    fn validated_media(&self, expected: &AttachmentScope) -> Result<Vec<TurnMedia>, String> {
        self.assert_current()?;
        if !self.scope.matches(expected) {
            return Err("Attachments belong to another Session authority.".to_owned());
        }
        let i = self.inner.lock().unwrap();
        if i.order
            .iter()
            .filter_map(|id| i.entries.get(id))
            .any(|e| e.draft.status != DraftStatus::Ready || e.media.is_none())
        {
            return Err("Upload or remove every selected image before sending this turn.".to_owned());
        }
        Ok(i.order
            .iter()
            .filter_map(|id| i.entries.get(id).and_then(|e| e.media.clone()))
            .collect())
    }

    /// `restoreUploaded` (attachment-drafts.ts:293-306): re-adopt media that is
    /// already uploaded, without uploading again. Refuses when the draft is
    /// not empty, and refuses media from another Profile.
    pub fn restore_uploaded(&self, media: Vec<TurnMedia>) -> Result<bool, String> {
        self.assert_current()?;
        if self.len() > 0 {
            return Ok(false);
        }
        if media
            .iter()
            .any(|m| !uploaded_handle_for_profile(&m.path, &self.scope.profile_id))
        {
            return Err("Attachments belong to another Profile.".to_owned());
        }
        let mut i = self.inner.lock().unwrap();
        for m in media {
            let id = self.next_id.fetch_add(1, Ordering::SeqCst).to_string();
            // The web takes the name from the handle's path component
            // (`item.path.split("/")[2]!`).
            let name = m.path.split('/').nth(2).unwrap_or(&m.path).to_owned();
            i.order.push(id.clone());
            i.entries.insert(
                id.clone(),
                Entry {
                    draft: AttachmentDraft {
                        id,
                        name,
                        bytes: m.size_bytes,
                        mime: m.mime.clone(),
                        status: DraftStatus::Ready,
                        error: None,
                    },
                    file: None,
                    media: Some(m),
                },
            );
        }
        Ok(true)
    }

    /// Auth retirement: abort work and release every retained local file
    /// (`invalidate`, attachment-drafts.ts:323-331).
    pub fn invalidate(&self) {
        if self.disposed.swap(true, Ordering::SeqCst) {
            return;
        }
        for abort in self.cancelled.lock().unwrap().values() {
            abort.store(true, Ordering::SeqCst);
        }
        self.cancelled.lock().unwrap().clear();
        self.clear_entries();
    }

    fn clear_entries(&self) {
        let mut i = self.inner.lock().unwrap();
        i.entries.clear();
        i.order.clear();
        i.uploading = 0;
    }
}

/// The dedup identity: the (name, size, mime) triple as a JSON array
/// (attachment-drafts.ts:142-150).
fn dedup_key(name: &str, bytes: u64, mime: &str) -> String {
    serde_json::json!([name, bytes, mime]).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope() -> AttachmentScope {
        AttachmentScope {
            authority_key: "auth-1".into(),
            session_id: "dsflash:main".into(),
            profile_id: "dsflash".into(),
        }
    }

    fn store() -> AttachmentDraftStore {
        AttachmentDraftStore::new(scope(), true).expect("scope")
    }

    fn file(name: &str, bytes: u64) -> LocalFile {
        LocalFile {
            name: name.to_owned(),
            bytes,
            mime: "image/png".into(),
            content: Arc::new(vec![0u8; bytes as usize]),
        }
    }

    fn handle_for(name: &str) -> String {
        // up/<base64url("/dsflash/uploads/x.png")>/<opaque>
        let path = format!("dsflash/uploads/{name}");
        format!("up/{}/opaque", b64url(path.as_bytes()))
    }

    fn b64url(bytes: &[u8]) -> String {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            out.push(T[(n >> 18) as usize & 63] as char);
            out.push(T[(n >> 12) as usize & 63] as char);
            if chunk.len() > 1 {
                out.push(T[(n >> 6) as usize & 63] as char);
            }
            if chunk.len() > 2 {
                out.push(T[n as usize & 63] as char);
            }
        }
        out
    }

    // ---- attachment-drafts.test.ts "deduplicates same-tick uploads and waits
    // ---- for all selected image receipts"
    #[test]
    fn a_repeated_select_is_deduplicated_on_the_triple() {
        let s = store();
        assert_eq!(s.select_files(vec![file("a.png", 10)]).unwrap(), 1);
        // the SAME (name,size,mime) is an idempotent no-op
        assert_eq!(s.select_files(vec![file("a.png", 10)]).unwrap(), 0);
        assert_eq!(s.len(), 1, "no duplicate draft, no new id");
        // a different size IS a new draft
        assert_eq!(s.select_files(vec![file("a.png", 11)]).unwrap(), 1);
        assert_eq!(s.len(), 2);
    }

    #[test]
    fn selection_is_atomic_and_bounded() {
        let s = store();
        // an invalid file in the batch leaves the existing draft INTACT
        assert!(s.select_files(vec![file("ok.png", 10), file("bad.txt", 10)]).is_err());
        assert_eq!(s.len(), 0, "a rejected batch adds nothing");
        // at most four per turn
        let five: Vec<LocalFile> = (1..=5).map(|i| file(&format!("{i}.png"), 10)).collect();
        assert_eq!(
            s.select_files(five).unwrap_err(),
            "Attach at most four images per turn."
        );
        assert_eq!(s.len(), 0);
        let four: Vec<LocalFile> = (1..=4).map(|i| file(&format!("{i}.png"), 10)).collect();
        assert_eq!(s.select_files(four).unwrap(), 4);
    }

    #[test]
    fn mime_and_size_rules_match_the_web() {
        assert_eq!(image_mime("a.PNG", Some("image/png")).unwrap(), "image/png");
        assert_eq!(image_mime("a.jpg", Some("image/jpeg")).unwrap(), "image/jpeg");
        assert_eq!(image_mime("a.JPEG", Some("IMAGE/JPEG")).unwrap(), "image/jpeg");
        // a declared type that disagrees is a refusal
        assert_eq!(
            image_mime("a.png", Some("image/gif")).unwrap_err(),
            "Choose PNG, JPEG, GIF or WebP images with matching file types."
        );
        assert!(image_mime("a.txt", None).is_err());
        assert!(image_mime("a.bmp", None).is_err());
        assert_eq!(check_size(0).unwrap_err(), "Each image must be nonempty and at most 20 MiB.");
        assert!(check_size(MAX_IMAGE_BYTES + 1).is_err());
        assert!(check_size(MAX_IMAGE_BYTES).is_ok());
    }

    // ---- attachment-drafts.test.ts "removing an in-flight entry aborts it and
    // ---- ignores a late receipt"
    #[test]
    fn removing_an_in_flight_entry_aborts_it_and_ignores_a_late_receipt() {
        let s = store();
        s.select_files(vec![file("a.png", 10)]).unwrap();
        let id = s.entries()[0].id.clone();
        let (_f, _mime, abort) = s.begin_upload(&id).expect("upload starts");
        s.remove(&id);
        assert!(abort.load(Ordering::SeqCst), "removal aborts the in-flight upload");
        // the receipt that lands afterwards is IGNORED, not applied
        let media = TurnMedia { path: handle_for("a.png"), mime: "image/png".into(), size_bytes: 10 };
        assert_eq!(s.complete_upload(&id, &media).unwrap(), false);
        assert_eq!(s.len(), 0, "the removed entry is not resurrected by a late receipt");
    }

    #[test]
    fn a_cancelled_upload_ignores_its_late_receipt() {
        let s = store();
        s.select_files(vec![file("a.png", 10)]).unwrap();
        let id = s.entries()[0].id.clone();
        let (_f, _m, abort) = s.begin_upload(&id).expect("upload starts");
        abort.store(true, Ordering::SeqCst); // the cancel arm
        let media = TurnMedia { path: handle_for("a.png"), mime: "image/png".into(), size_bytes: 10 };
        assert_eq!(s.complete_upload(&id, &media).unwrap(), false, "cancelled -> ignore");
        assert_eq!(s.entries()[0].status, DraftStatus::Uploading, "untouched by the late receipt");
    }

    // ---- the same-tick dedup on the upload path
    #[test]
    fn a_same_tick_upload_click_cannot_duplicate_work() {
        let s = store();
        s.select_files(vec![file("a.png", 10)]).unwrap();
        let id = s.entries()[0].id.clone();
        assert!(s.begin_upload(&id).is_ok());
        // a second begin for the same entry is refused
        assert_eq!(s.begin_upload(&id).unwrap_err(), "This attachment is already uploading.");
        // and upload_candidates no longer offers it (status is uploading)
        assert!(s.upload_candidates().is_empty());
    }

    #[test]
    fn a_receipt_must_match_this_profile_mime_and_size() {
        let s = store();
        s.select_files(vec![file("a.png", 10)]).unwrap();
        let id = s.entries()[0].id.clone();
        s.begin_upload(&id).unwrap();
        // another Profile's handle is refused
        let wrong_profile = TurnMedia {
            path: format!("up/{}/opaque", b64url(b"other/uploads/a.png")),
            mime: "image/png".into(),
            size_bytes: 10,
        };
        assert_eq!(s.complete_upload(&id, &wrong_profile).unwrap_err(), "Invalid attachment receipt");
        assert_eq!(s.entries()[0].status, DraftStatus::Error, "a bad receipt is retryable");
        // a size mismatch is refused too
        s.begin_upload(&id).unwrap();
        let wrong_size = TurnMedia { path: handle_for("a.png"), mime: "image/png".into(), size_bytes: 99 };
        assert!(s.complete_upload(&id, &wrong_size).is_err());
    }

    #[test]
    fn a_good_receipt_marks_the_entry_ready_and_releases_the_bytes() {
        let s = store();
        s.select_files(vec![file("a.png", 10)]).unwrap();
        let id = s.entries()[0].id.clone();
        s.begin_upload(&id).unwrap();
        let media = TurnMedia { path: handle_for("a.png"), mime: "image/png".into(), size_bytes: 10 };
        assert!(s.complete_upload(&id, &media).unwrap());
        assert_eq!(s.entries()[0].status, DraftStatus::Ready);
        assert!(!s.uploading());
    }

    // ---- the uploaded-handle rule
    #[test]
    fn the_uploaded_handle_must_name_this_profile() {
        assert!(uploaded_handle_for_profile(&handle_for("a.png"), "dsflash"));
        // another profile
        assert!(!uploaded_handle_for_profile(
            &format!("up/{}/opaque", b64url(b"other/a.png")),
            "dsflash"
        ));
        // wrong shape
        assert!(!uploaded_handle_for_profile("up//opaque", "dsflash"));
        assert!(!uploaded_handle_for_profile("nope", "dsflash"));
        assert!(!uploaded_handle_for_profile("up/!!/opaque", "dsflash"));
        // a path escaping the profile root
        assert!(!uploaded_handle_for_profile(
            &format!("up/{}/opaque", b64url(b"dsflash/../etc/passwd")),
            "dsflash"
        ));
    }

    // ---- "consumes attachments only after successful local queue admission"
    #[test]
    fn attachments_are_consumed_only_after_a_successful_local_admission() {
        let s = store();
        s.select_files(vec![file("a.png", 10)]).unwrap();
        let id = s.entries()[0].id.clone();
        s.begin_upload(&id).unwrap();
        s.complete_upload(
            &id,
            &TurnMedia { path: handle_for("a.png"), mime: "image/png".into(), size_bytes: 10 },
        )
        .unwrap();

        // A REJECTED queue keeps the complete draft.
        assert_eq!(s.submit_turn(&scope(), |_| false).unwrap(), false);
        assert_eq!(s.len(), 1, "a rejected queue keeps the draft");
        assert_eq!(s.entries()[0].status, DraftStatus::Ready);

        // An ACCEPTED queue consumes it and hands the media to that turn.
        let mut taken: Vec<TurnMedia> = Vec::new();
        assert_eq!(s.submit_turn(&scope(), |m| { taken = m.to_vec(); true }).unwrap(), true);
        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].path, handle_for("a.png"));
        assert_eq!(s.len(), 0, "consumed exactly once");
    }

    #[test]
    fn an_unready_entry_refuses_the_turn() {
        let s = store();
        s.select_files(vec![file("a.png", 10), file("b.png", 10)]).unwrap();
        let first = s.entries()[0].id.clone();
        s.begin_upload(&first).unwrap();
        s.complete_upload(
            &first,
            &TurnMedia { path: handle_for("a.png"), mime: "image/png".into(), size_bytes: 10 },
        )
        .unwrap();
        // b.png is still `selected`
        assert_eq!(
            s.take_for_turn(&scope()).unwrap_err(),
            "Upload or remove every selected image before sending this turn."
        );
        assert_eq!(s.len(), 2, "the refusal keeps the draft");
    }

    #[test]
    fn another_sessions_authority_is_refused() {
        let s = store();
        s.select_files(vec![file("a.png", 10)]).unwrap();
        let id = s.entries()[0].id.clone();
        s.begin_upload(&id).unwrap();
        s.complete_upload(
            &id,
            &TurnMedia { path: handle_for("a.png"), mime: "image/png".into(), size_bytes: 10 },
        )
        .unwrap();
        let other = AttachmentScope {
            authority_key: "auth-2".into(),
            ..scope()
        };
        assert_eq!(
            s.take_for_turn(&other).unwrap_err(),
            "Attachments belong to another Session authority."
        );
        assert_eq!(s.len(), 1, "the draft survives a refused handoff");
    }

    #[test]
    fn unadvertised_upload_fails_closed_before_any_selection() {
        let s = AttachmentDraftStore::new(scope(), false).unwrap();
        assert_eq!(
            s.select_files(vec![file("a.png", 10)]).unwrap_err(),
            "Image uploads are unavailable on this server."
        );
        assert_eq!(s.len(), 0);
    }

    #[test]
    fn an_incomplete_scope_is_refused_at_construction() {
        let bad = AttachmentScope { authority_key: "  ".into(), ..scope() };
        assert_eq!(
            AttachmentDraftStore::new(bad, true).unwrap_err(),
            "A confirmed attachment authority, Profile and Session are required."
        );
    }

    #[test]
    fn restore_readopts_uploaded_media_without_uploading_again() {
        let s = store();
        let media = vec![TurnMedia {
            path: handle_for("a.png"),
            mime: "image/png".into(),
            size_bytes: 10,
        }];
        assert!(s.restore_uploaded(media).unwrap());
        assert_eq!(s.entries()[0].status, DraftStatus::Ready);
        assert_eq!(
            s.entries()[0].name, "opaque",
            "the name is the handle's third path component (attachment-drafts.ts:302), which is opaque"
        );
        // a second restore is refused while the draft is not empty
        assert!(!s.restore_uploaded(vec![]).unwrap());
        // another Profile's media is refused
        let other = store();
        assert_eq!(
            other
                .restore_uploaded(vec![TurnMedia {
                    path: format!("up/{}/o", b64url(b"other/a.png")),
                    mime: "image/png".into(),
                    size_bytes: 10,
                }])
                .unwrap_err(),
            "Attachments belong to another Profile."
        );
    }

    #[test]
    fn invalidation_aborts_work_and_releases_every_draft() {
        let s = store();
        s.select_files(vec![file("a.png", 10)]).unwrap();
        let id = s.entries()[0].id.clone();
        let (_f, _m, abort) = s.begin_upload(&id).unwrap();
        s.invalidate();
        assert!(abort.load(Ordering::SeqCst), "invalidation aborts in-flight work");
        assert!(s.disposed());
        assert_eq!(s.len(), 0, "every retained local file is released");
        // a late receipt after invalidation is ignored
        let media = TurnMedia { path: handle_for("a.png"), mime: "image/png".into(), size_bytes: 10 };
        assert_eq!(s.complete_upload(&id, &media).unwrap(), false);
        // and the disposed store refuses further work
        assert!(s.select_files(vec![file("b.png", 10)]).is_err());
    }

    #[test]
    fn remove_never_deletes_a_server_file_and_is_idempotent() {
        let s = store();
        s.select_files(vec![file("a.png", 10)]).unwrap();
        let id = s.entries()[0].id.clone();
        s.begin_upload(&id).unwrap();
        s.complete_upload(
            &id,
            &TurnMedia { path: handle_for("a.png"), mime: "image/png".into(), size_bytes: 10 },
        )
        .unwrap();
        s.remove(&id);
        assert_eq!(s.len(), 0);
        s.remove(&id); // removing an absent id is a no-op
        assert_eq!(s.len(), 0);
    }
}

// ------------------------------------------------- the production surface

/// The action ids this screen owns TODAY.
///
/// Only `media.submit` is dispatched; `media.select`/`media.upload`/
/// `media.remove` have no production caller until the AttachmentsDialog lands
/// (a design-flow surface), so claiming them here would be RULES 3's
/// "test-only == missing" in reverse.
pub fn owns(action: &str) -> bool {
    action == "media.submit"
}

/// The production send path: the draft is consumed ONLY at the ACCEPTED local
/// enqueue boundary, and the accepted batch rides THIS turn's `turn/start`
/// params as `media`.
///
/// `turn/start` omits `media` entirely when the batch is empty (octos-core
/// `ui_protocol.rs:2044-2045`, `skip_serializing_if = "Vec::is_empty"`), so a
/// text-only turn is byte-identical to the no-attachment path.
///
/// `admit` is the LOCAL admission decision (the web's `submitTurn` callback,
/// attachment-drafts.ts:281-289). A REJECTED queue returns `Ok(false)`, keeps
/// the complete draft, and NEVER issues a turn.
pub async fn perform(
    conv: &crate::flow::Conversation,
    action: &str,
    store: &Store,
    value: Option<&str>,
) -> Result<String, String> {
    let _ = store;
    if action != "media.submit" {
        return Err(format!("media: unhandled action {action:?}"));
    }
    let text = value.unwrap_or_default();
    let scope = current_scope(conv);
    let drafts = drafts_for(&scope);
    // The batch is captured INSIDE the accepted boundary and is exactly what
    // the turn carries — the draft is never read later, when the queued turn
    // starts (attachment-drafts.ts:264-272).
    let mut batch: Vec<TurnMedia> = Vec::new();
    let accepted = drafts.submit_turn(&scope, |media| {
        batch = media.to_vec();
        true
    })?;
    if !accepted {
        // A refused local queue: the draft is intact and no turn was sent.
        return Ok("The turn was not admitted; the attachments were kept.".to_owned());
    }
    conv.start_turn_with_media(text, batch)
        .await
        .map_err(|e| e.to_string())?;
    Ok(text.to_owned())
}

/// Install a prepared draft on this authority, so a caller that has already
/// selected/uploaded through the store's own API hands the SAME store to the
/// send path. Without it `perform` would mint its own empty draft and the batch
/// would never be built.
pub fn seed_draft_for_test(
    conv: &crate::flow::Conversation,
    scope: &AttachmentScope,
    drafts: &Arc<AttachmentDraftStore>,
) {
    // Key on the conversation's own authority so `perform` finds this store.
    draft_map()
        .lock()
        .unwrap()
        .insert(conv.session_id(), drafts.clone());
    let _ = scope;
}

/// Forget this authority's draft, so a text-only send sees an EMPTY draft
/// (the web's `takeForTurn` on an empty store yields an empty batch).
pub fn clear_draft_for_test(conv: &crate::flow::Conversation) {
    draft_map().lock().unwrap().remove(&conv.session_id());
}

/// The bound authority for this conversation: the Session is the authority key
/// and the Profile is the scope's Profile.
fn current_scope(conv: &crate::flow::Conversation) -> AttachmentScope {
    AttachmentScope {
        authority_key: conv.session_id(),
        session_id: conv.session_id(),
        profile_id: conv.profile(),
    }
}

/// THE one draft map, shared by the seed helper and the send path. Two
/// separate `static OnceLock`s would silently give them different maps.
fn draft_map() -> &'static Mutex<HashMap<String, Arc<AttachmentDraftStore>>> {
    use std::sync::OnceLock;
    static DRAFTS: OnceLock<Mutex<HashMap<String, Arc<AttachmentDraftStore>>>> = OnceLock::new();
    DRAFTS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// The per-Session draft store, keyed on the authority in an `Arc`, so a draft
/// survives an ordinary Session switch and is dropped when the last handle goes
/// (attachment-drafts.ts:54-59). No leak, and no second throwaway store.
fn drafts_for(scope: &AttachmentScope) -> Arc<AttachmentDraftStore> {
    let map = draft_map();
    let key = scope.authority_key.clone();
    let mut m = map.lock().unwrap();
    // A retired authority starts a fresh, empty draft.
    if let Some(existing) = m.get(&key) {
        return existing.clone();
    }
    let store = Arc::new(AttachmentDraftStore::new(scope.clone(), true).expect("complete scope"));
    m.insert(key, store.clone());
    store
}
