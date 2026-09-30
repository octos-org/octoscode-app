//! #32e — the one accessor for every design-file read.
//!
//! The phone has no `design/` tree on disk: the APK embeds the runtime-read
//! files at build time (`build.rs` generates the table), and this module
//! serves them. Desktop design work keeps live reload: setting
//! `OCTOSCODE_DESIGN_DIR` reads from that directory instead of the embed.
//!
//! Two consumption shapes, one source of truth:
//! - [`file`] — text reads inside this crate (the former `fs::read_to_string`
//!   sites): the embedded copy, no disk.
//! - [`root`] — a REAL directory: the renderer's lowering
//!   (`octoscript_makepad::l0::prepare` reads the kit pack itself,
//!   l0.rs:57) needs paths, so the embed is materialized once under
//!   `$HOME/.octoscode/design` (marker short-circuit) and every screen's
//!   card dir re-roots there. With the env var set, `root` IS that
//!   directory (live reload, nothing written).

use std::borrow::Cow;
use std::path::{PathBuf, Path};
use std::sync::OnceLock;

/// The generated embed table (path relative to `design/` -> bytes).
fn table() -> &'static [(&'static str, &'static [u8])] {
    static TABLE: OnceLock<&'static [(&'static str, &'static [u8])]> = OnceLock::new();
    TABLE.get_or_init(|| include!(concat!(env!("OUT_DIR"), "/design_embedded.rs")))
}

/// The override directory (`OCTOSCODE_DESIGN_DIR`), when set.
fn override_dir() -> Option<PathBuf> {
    std::env::var_os("OCTOSCODE_DESIGN_DIR").map(PathBuf::from)
}

/// One design file, by its path relative to `design/` (e.g.
/// `"stage-b/setup/cards/setup-01/page.card"`).
///
/// The embedded copy is the default; with `OCTOSCODE_DESIGN_DIR` set the
/// on-disk copy wins (desktop live reload for design work). `Err` names the
/// reason, in the shape the disk reads used to produce.
pub fn file(rel: &str) -> Result<Cow<'static, str>, String> {
    if let Some(dir) = override_dir() {
        let path = dir.join(rel);
        return std::fs::read_to_string(&path)
            .map(Cow::Owned)
            .map_err(|e| format!("read {}: {e}", path.display()));
    }
    table()
        .iter()
        .find(|(p, _)| *p == rel)
        .map(|(_, bytes)| Cow::Borrowed(
            // The whitelist only admits UTF-8 text sources (card/l0/splash/
            // json/svg), so this cannot fail on a well-formed table.
            std::str::from_utf8(bytes).unwrap_or_default(),
        ))
        .ok_or_else(|| format!("read {rel}: not in the embedded design table"))
}

/// Whether `rel` is present in the embed (the coverage test walks this).
pub fn embedded(rel: &str) -> bool {
    table().iter().any(|(p, _)| *p == rel)
}

/// How many files the embed carries (the report/ACK byte-count probe).
pub fn embedded_count() -> usize {
    table().len()
}

/// Total embedded bytes (the ACK reports it; also the materialize marker).
pub fn embedded_bytes() -> usize {
    table().iter().map(|(_, b)| b.len()).sum()
}

/// The design root every card dir re-roots to: `$OCTOSCODE_DESIGN_DIR` when
/// set (desktop live reload — nothing is written), else the embed
/// materialized once under `$HOME/.octoscode/design`. The renderer's own
/// lowering reads the kit packs from a real directory, so the table becomes
/// files there; a marker carrying the embed's byte total short-circuits the
/// re-write on the next run.
pub fn root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| match override_dir() {
        Some(dir) => dir,
        None => {
            let dir = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(std::env::temp_dir)
                .join(".octoscode")
                .join("design");
            let marker = dir.join(".embed-marker");
            let total = embedded_bytes().to_string();
            if std::fs::read_to_string(&marker).is_ok_and(|m| m == total) {
                return dir;
            }
            for (rel, bytes) in table() {
                let path = dir.join(rel);
                if let Some(parent) = path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                let _ = std::fs::write(&path, bytes);
            }
            let _ = std::fs::create_dir_all(&dir);
            let _ = std::fs::write(&marker, total);
            dir
        }
    })
    .clone()
}

/// The crate dir, for `ScriptMod::cargo_manifest_path` diagnostic metadata
/// only (never read as a path by this module) — the one place the manifest
/// env appears in `src/`.
pub fn manifest_dir() -> &'static str {
    env!("CARGO_MANIFEST_DIR")
}

/// The path under [`root`] for `rel` (relative to `design/`).
pub fn path(rel: &str) -> PathBuf {
    root().join(rel)
}

/// The design tree beside this crate (the dev checkout), when present.
fn repo_tree() -> Option<PathBuf> {
    let base = Path::new(manifest_dir()).join("../../design");
    base.is_dir().then_some(base)
}

/// The design subtree `rel` resolves to: the CHECKOUT's tree while it exists
/// (dev live edits), else the materialized embed (the phone). Every screen's
/// card dir resolves through this one accessor, so the kit packs the
/// renderer reads itself land in the same tree.
pub fn dir(rel: &str) -> PathBuf {
    match repo_tree() {
        Some(base) if base.join(rel).is_dir() => base.join(rel),
        _ => root().join(rel),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embed_carries_the_cards_the_screens_read() {
        // The phone failure from the board entry, per file class:
        assert!(embedded("cards/index.json"));
        assert!(embedded("stage-b/setup/cards/setup-01/page.card"),
            "the logcat failure file is embedded");
        assert!(embedded("stage-b/setup/cards/setup-01/page.data.json"));
        assert!(embedded("components/index.json"));
        assert!(embedded_count() > 100, "{} embedded files", embedded_count());
    }

    #[test]
    fn every_manifest_artifact_is_embedded() {
        // Do4's coverage proof: walk the SAME list the screens use (the
        // card manifest's artifacts) and require every path in the table.
        // The manifest's artifact paths are relative to `design/cards/`
        // (what `cards::mounted().dir` resolves to); the embed table is
        // keyed relative to `design/` — hence the `cards/` prefix.
        let in_embed = |p: &str| embedded(&format!("cards/{p}"));
        for card in crate::cards::mounted().manifest.cards.iter() {
            let a = &card.artifacts;
            assert!(in_embed(&a.card), "card artifact: {}", a.card);
            assert!(in_embed(&a.mapped), "mapped: {}", a.mapped);
            if let Some(kit) = &a.kit_dir {
                assert!(in_embed(&format!("{kit}/native/light/kit.json")),
                    "kit pack for {}", card.slot);
            }
        }
    }

    #[test]
    fn the_root_materializes_the_embed_or_honors_the_override() {
        let dir = root();
        let on = dir.join("cards/index.json");
        let text = std::fs::read_to_string(&on)
            .unwrap_or_else(|e| panic!("materialized {}: {e}", on.display()));
        assert!(text.contains("\"cards\""), "the materialized manifest shape");
        // The disk copy equals the embedded copy (byte-for-byte).
        let embedded_text = file("cards/index.json").expect("embedded");
        assert_eq!(text, embedded_text.as_ref());
    }

    #[test]
    fn a_missing_file_errs_with_the_disk_read_shape() {
        let e = file("no/such/file.card").unwrap_err();
        assert!(e.starts_with("read no/such/file.card"), "{e}");
    }

    #[test]
    fn the_embedded_copy_parses_as_utf8_text() {
        let text = file("cards/index.json").expect("embedded");
        assert!(text.contains("\"cards\""), "the manifest shape");
    }
}
