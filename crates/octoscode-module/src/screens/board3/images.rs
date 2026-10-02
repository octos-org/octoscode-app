//! Board-3 screen 10 — ATTACHMENTS (row: media × 1, "AttachmentsDialog UI
//! (select/upload/receipts/scope+limits)").
//!
//! Web: `features/media/AttachmentsDialog.tsx` (title "Turn images"), opened
//! by `/images` (`registry.ts:254-262`, requires `turn/start`). The draft
//! itself is `screens/media.rs` (the P4d4 port of `attachment-drafts.ts`):
//! at most four PNG/JPEG/GIF/WebP images, 20 MiB each, selection never
//! uploads, the (name, size, mime) dedupe, atomic batches, explicit upload
//! (`POST <base>/api/upload` multipart, one file per request, receipt =
//! this Profile's `up/<b64>/<name>` handle, `packages/client/src/media.ts:
//! 114-146`), remove never deletes a server file, cancel resets in-flight
//! rows to `selected`, and the ready batch rides the next `turn/start` as
//! `media` (the flow's submit takes it at the accepted boundary).
//!
//! Selection natively: the platform file dialog (`cx.open_select_file_dialog`,
//! filtered to the four types) or a file dropped onto the open dialog; both
//! hand PATHS to [`select_paths`]. The board's slot grid shows a real
//! thumbnail of each selected file.
use std::sync::Arc;


use crate::screens::media::{self, AttachmentDraft, DraftStatus, LocalFile};

use super::host::Outcome;
use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

#[derive(Debug, Clone, Default)]
pub struct ImgState {
    pub error: Option<String>,
    pub notice: Option<String>,
    /// draft id -> the local path it was read from (thumbnails only; the
    /// bytes live in the draft, never re-read).
    pub paths: Vec<(String, String)>,
    pub session: String,
    pub profile: String,
}

/// The size line (`AttachmentsDialog.tsx:131-157`): `0.48 MiB · image/png`.
pub fn size_line(bytes: u64, mime: &str) -> String {
    format!("{:.2} MiB · {mime}", bytes as f64 / 1_048_576.0)
}

/// The status copy per draft state (`AttachmentsDialog.tsx:131-157`).
pub fn status_copy(s: DraftStatus) -> &'static str {
    match s {
        DraftStatus::Ready => "Uploaded; ready for this turn",
        DraftStatus::Uploading => "Uploading…",
        DraftStatus::Error => "Upload not confirmed",
        _ => "Selected; not uploaded",
    }
}

/// Read local files into the draft (atomic: a bad batch changes nothing).
pub fn select_paths(drafts: &media::AttachmentDraftStore, paths: &[std::path::PathBuf]) -> Result<Vec<(String, String)>, String> {
    let mut files = Vec::new();
    for p in paths {
        let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mime = media::image_mime(&name, None)?;
        let bytes = std::fs::read(p).map_err(|_| "Could not select these images.".to_owned())?;
        media::check_size(bytes.len() as u64)?;
        files.push(LocalFile { name, bytes: bytes.len() as u64, mime, content: Arc::new(bytes) });
    }
    let before: Vec<String> = drafts.entries().into_iter().map(|e| e.id).collect();
    drafts.select_files(files)?;
    let added: Vec<AttachmentDraft> = drafts.entries().into_iter().filter(|e| !before.contains(&e.id)).collect();
    Ok(added
        .into_iter()
        .filter_map(|e| {
            paths
                .iter()
                .find(|p| p.file_name().map(|n| n.to_string_lossy() == e.name).unwrap_or(false))
                .map(|p| (e.id.clone(), p.to_string_lossy().into_owned()))
        })
        .collect())
}

/// Upload every selected/errored draft (the explicit action), one request
/// per file, receipts validated by the draft store.
pub async fn upload(conv: &crate::flow::Conversation) -> Result<String, String> {
    let drafts = media::drafts_for_conv(conv);
    let ids = drafts.upload_candidates();
    if ids.is_empty() {
        return Ok("nothing to upload".into());
    }
    let mut ok = 0usize;
    for id in ids {
        let (file, _key, abort) = match drafts.begin_upload(&id) {
            Ok(x) => x,
            Err(e) => {
                super::host::state().img.error = Some(e);
                continue;
            }
        };
        super::host::wake();
        match conv.upload_file(&file.name, &file.mime, (*file.content).clone()).await {
            Ok(handle) => {
                if abort.load(std::sync::atomic::Ordering::SeqCst) {
                    continue; // removed/cancelled while in flight: ignore the receipt
                }
                let media = media::TurnMedia { path: handle, mime: file.mime.clone(), size_bytes: file.bytes };
                match drafts.complete_upload(&id, &media) {
                    Ok(true) => ok += 1,
                    Ok(false) => {}
                    Err(e) => {
                        drafts.fail_upload(&id, &e);
                        super::host::state().img.error = Some(e);
                    }
                }
            }
            Err(e) => {
                drafts.fail_upload(&id, "Upload was not confirmed. Retry explicitly; the server may retain an earlier upload.");
                super::host::state().img.error = Some(e);
            }
        }
        super::host::wake();
    }
    Ok(format!("{ok} uploaded"))
}

pub fn perform(st: &mut ImgState, action: &str, index: usize, drafts: Option<&media::AttachmentDraftStore>) -> Outcome {
    let Some(drafts) = drafts else {
        st.error = Some("This Session cannot upload images now. Reopen images from its current authority.".into());
        return Outcome::Done;
    };
    match action {
        "b3.img.choose" => {
            if drafts.len() >= media::MAX_TURN_IMAGES || drafts.uploading() {
                return Outcome::Done; // the picker is disabled (`:101-105`)
            }
            Outcome::PickFiles
        }
        "b3.img.upload" => {
            st.error = None;
            Outcome::Spawn(super::host::Job::ImagesUpload)
        }
        "b3.img.remove" => {
            if let Some(e) = drafts.entries().get(index) {
                drafts.remove(&e.id);
                st.paths.retain(|(id, _)| id != &e.id);
            }
            Outcome::Done
        }
        "b3.img.cancel" => {
            drafts.cancel_uploads();
            st.notice = Some("Transfers canceled locally. No server files were deleted.".into());
            Outcome::Done
        }
        "b3.img.close" => {
            // Close (or Escape) cancels pending transfers and keeps the
            // selections (`AttachmentsDialog.tsx:36-45`).
            drafts.cancel_uploads();
            Outcome::Close
        }
        _ => Outcome::Unrouted,
    }
}

// -------------------------------------------------------------------- view

fn slot(d: &mut Dsl, i: usize, size: f64, entry: Option<(&AttachmentDraft, Option<&str>)>) {
    let id = format!("b3_img_slot_{i}");
    match entry {
        Some((e, path)) => {
            d.surface(&id, &format!("width: {size} height: {size} flow: Overlay"), tok::CHIP, 10.0, Some(tok::HAIRLINE));
            match path {
                Some(p) => {
                    d.open(
                        &format!("{id}_img"),
                        "Image",
                        &format!("width: Fill height: Fill fit: ImageFit.Biggest src: file_resource({p:?})"),
                    );
                    d.close();
                }
                None => {
                    let c = d.anon();
                    d.view(&c, "width: Fill height: Fill align: Align{x: 0.5 y: 0.5}");
                    d.icon("", "b3_image.svg", size * 0.5, tok::FAINT);
                    d.close();
                }
            }
            // The remove affordance (top-right).
            let corner = d.anon();
            d.view(&corner, "width: Fill height: Fill align: Align{x: 1.0 y: 0.0} padding: Inset{top: 4 right: 4}");
            d.surface(&format!("{id}_x_box"), "width: 20 height: 20 flow: Overlay align: Align{x: 0.5 y: 0.5}", tok::WHITE, 10.0, Some(tok::HAIRLINE));
            d.icon("", "b3_x_small.svg", 11.0, tok::TEXT);
            d.tap(&format!("{id}_x"), &format!("b3.img.remove#{i}"));
            d.close();
            d.close();
            let _ = e;
            d.close();
        }
        None => {
            // The empty slot: the board's dashed frame with a "+" (choose
            // files). The dash is an SVG — a DesignSurface stroke is solid.
            d.view(&id, &format!("width: {size} height: {size} flow: Overlay align: Align{{x: 0.5 y: 0.5}}"));
            d.icon(&format!("{id}_frame"), "b3_dashed_slot.svg", size, tok::FAINT);
            d.icon("", "b3_plus.svg", 22.0, tok::FAINT);
            d.tap(&format!("{id}_add"), "b3.img.choose");
            d.close();
        }
    }
}

pub fn build(d: &mut Dsl, st: &ImgState, frame: &Frame, drafts: Option<&media::AttachmentDraftStore>) {
    let width = frame.dialog_w(680.0);
    let pad = ui::dialog_pad(frame, width);
    let inner_w = width - 2.0 * pad;
    let entries = drafts.map(|s| s.entries()).unwrap_or_default();
    let uploading = drafts.is_some_and(|s| s.uploading());
    ui::shell_open(d, frame, width);
    let row = d.anon();
    d.view(&row, "width: Fill height: 32 flow: Right align: Align{x: 0.0 y: 0.5}");
    d.text("b3_title", "Turn images", &ui::title().w(W::Fill));
    ui::close_glyph(d, "b3.img.close");
    d.close();
    d.text(
        "b3_img_desc",
        "Choose up to four PNG, JPEG, GIF or WebP images. Selecting a file does not upload it; upload explicitly, then send it with this Session's next prompt.",
        &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    d.gap(W::Fill, 14.0);
    ui::body_open(d, frame, width, 84.0);
    // The board's four slots.
    let gap = 10.0;
    let size = ((inner_w - 3.0 * gap - 10.0) / 4.0).clamp(64.0, 110.0).floor();
    let grid = d.anon();
    d.view(&grid, &format!("width: Fill height: Fit flow: Right spacing: {gap}"));
    for i in 0..media::MAX_TURN_IMAGES {
        let e = entries.get(i).map(|e| {
            let p = st.paths.iter().find(|(id, _)| id == &e.id).map(|(_, p)| p.as_str());
            (e, p)
        });
        slot(d, i, size, e);
    }
    d.close();
    d.gap(W::Fill, 10.0);
    d.text(
        "b3_img_counter",
        &format!("{} of {} image slots used", entries.len(), media::MAX_TURN_IMAGES),
        &Txt::new(12.5, Face::Regular, tok::TEXT),
    );
    for (i, e) in entries.iter().enumerate() {
        d.text(
            &format!("b3_img_row_{i}"),
            &format!("{} · {} · {}", super::inventory::fit(&e.name, 220.0, 12.0, false), size_line(e.bytes, &e.mime), status_copy(e.status)),
            &Txt::new(11.5, Face::Regular, if e.status == DraftStatus::Error { tok::RED } else { tok::MUTED }).w(W::Fill),
        );
    }
    d.gap(W::Fill, 12.0);
    d.hairline();
    d.gap(W::Fill, 12.0);
    // Scope (`AttachmentsDialog.tsx:68-73`, the web's copy — not the board's
    // stray "Session: dsflash" line), between hairlines as the board draws it.
    let scope = d.anon();
    d.view(&scope, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} spacing: 8");
    d.text("", "Profile:", &Txt::new(13.0, Face::Regular, tok::MUTED));
    d.text("b3_img_profile", &st.profile, &Txt::new(13.0, Face::Mono, tok::TEXT));
    d.text("", "· Session:", &Txt::new(13.0, Face::Regular, tok::MUTED));
    d.text("b3_img_session", &super::inventory::fit(&st.session, inner_w - 240.0, 13.0, true), &Txt::new(13.0, Face::Mono, tok::TEXT).w(W::Fill));
    d.close();
    d.gap(W::Fill, 12.0);
    d.hairline();
    d.gap(W::Fill, 12.0);
    d.text("b3_img_limit", "20 MiB per image", &ui::meta());
    if let Some(e) = &st.error {
        d.gap(W::Fill, 6.0);
        d.text("b3_img_error", e, &Txt::new(12.0, Face::Regular, tok::RED).w(W::Fill).wrap());
    }
    if let Some(n) = &st.notice {
        d.gap(W::Fill, 6.0);
        d.text("b3_img_notice", n, &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    }
    d.gap(W::Fill, 14.0);
    let full = entries.len() >= media::MAX_TURN_IMAGES || uploading;
    let col = d.anon();
    d.view(&col, "width: Fill height: Fit flow: Down spacing: 10");
    // The board's rounded outline buttons; an unavailable control keeps its
    // place and reads as unavailable (no tap).
    d.button("b3_img_choose", "Choose image files", "b3.img.choose", if full { Btn::OutlineOff } else { Btn::Outline }, W::Fill, 42.0);
    let uploadable = entries.iter().any(|e| matches!(e.status, DraftStatus::Selected | DraftStatus::Error));
    if uploadable || uploading {
        d.button(
            "b3_img_upload",
            if uploading { "Uploading…" } else { "Upload selected images" },
            "b3.img.upload",
            if uploadable && !uploading { Btn::Primary } else { Btn::Disabled },
            W::Fill,
            42.0,
        );
    }
    d.button("b3_img_cancel", "Cancel uploads", "b3.img.cancel", if uploading { Btn::Outline } else { Btn::OutlineOff }, W::Fill, 42.0);
    d.button("b3_img_close_btn", if uploading { "Cancel uploads and close" } else { "Close images" }, "b3.img.close", Btn::Outline, W::Fill, 42.0);
    d.close();
    ui::body_close(d);
    ui::shell_close(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_matches_the_web_dialog() {
        assert_eq!(size_line(503_316, "image/png"), "0.48 MiB · image/png");
        assert_eq!(status_copy(DraftStatus::Ready), "Uploaded; ready for this turn");
        assert_eq!(status_copy(DraftStatus::Selected), "Selected; not uploaded");
    }

    fn drafts() -> media::AttachmentDraftStore {
        media::AttachmentDraftStore::new(
            media::AttachmentScope { authority_key: "s".into(), session_id: "s".into(), profile_id: "p".into() },
            true,
        )
        .unwrap()
    }

    #[test]
    fn selecting_reads_paths_into_the_draft_and_remove_drops_one() {
        let dir = std::env::temp_dir().join(format!("b3img-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = dir.join("a.png");
        std::fs::write(&a, b"\x89PNG....").unwrap();
        let bad = dir.join("b.txt");
        std::fs::write(&bad, b"x").unwrap();
        let s = drafts();
        assert!(select_paths(&s, &[a.clone(), bad]).is_err(), "a bad batch selects nothing");
        assert_eq!(s.len(), 0);
        let added = select_paths(&s, &[a.clone()]).unwrap();
        assert_eq!(added.len(), 1);
        let mut st = ImgState { paths: added, ..Default::default() };
        assert_eq!(perform(&mut st, "b3.img.choose", 0, Some(&s)), Outcome::PickFiles);
        perform(&mut st, "b3.img.remove", 0, Some(&s));
        assert_eq!(s.len(), 0);
        assert!(st.paths.is_empty());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn without_a_draft_store_the_dialog_fails_closed() {
        let mut st = ImgState::default();
        assert_eq!(perform(&mut st, "b3.img.choose", 0, None), Outcome::Done);
        assert!(st.error.is_some());
    }

    #[test]
    fn the_dialog_lowers_balanced() {
        let s = drafts();
        let mut d = Dsl::new();
        build(&mut d, &ImgState::default(), &Frame::DESKTOP, Some(&s));
        let dsl = d.finish();
        assert_eq!(dsl.matches('{').count(), dsl.matches('}').count());
        let taps = crate::screens::taps::wired_taps(&dsl);
        assert!(taps.iter().any(|(_, e)| e == "b3.img.choose"));
        assert!(taps.iter().any(|(_, e)| e == "b3.img.close"));
    }
}
