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
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

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

/// The app dir the HOST hands the module (Android:
/// `/data/user/0/<package>/files`, from `Cx::get_data_dir()`); installed
/// once, before the first design read (lib.rs's mount arm). `None` until a
/// host with the API connects.
static HOST_DIR: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Called from the module's FIRST Cx hook (`register`, via `vm.cx_mut()`)
/// and again at mount: the app's writable files dir (`Cx::get_data_dir()`;
/// on Android `/data/user/0/<package>/files`, the dir the shell already
/// logs). Seeding must happen BEFORE the first design read — register()
/// itself reads the component ledger (`components::log_resolutions`), and
/// on the phone root() would otherwise bake the unwritable temp fallback
/// into the OnceLock (device log: "no HOME and no host files dir").
/// `None` is recorded too: no dir is a fact like any other.
pub fn set_host_dir(dir: Option<String>) {
    *HOST_DIR.write().unwrap() = dir.map(PathBuf::from);
}

/// The base-dir resolution, PURE for tests: the explicit override (the env),
/// then the host's files dir, then `$HOME` — NEVER `temp_dir` (Android's is
/// unwritable for an app and HOME is usually unset there). `None` = no
/// writable base anywhere; the caller logs and falls back (desktop dev only).
fn resolve_base(env: Option<&Path>, host: Option<&Path>, home: Option<&std::ffi::OsStr>) -> Option<PathBuf> {
    if let Some(dir) = env {
        return Some(PathBuf::from(dir));
    }
    if let Some(dir) = host {
        return Some(dir.join(".octoscode").join("design"));
    }
    home.map(|h| Path::new(h).join(".octoscode").join("design"))
}

/// The design root every card dir re-roots to: `$OCTOSCODE_DESIGN_DIR` when
/// set (desktop live reload — nothing is written), else the host's files dir
/// (Android), else `$HOME/.octoscode/design` (desktop). The chosen root is
/// LOGGED once (#32f item 1: the #32e build failed silently on the phone —
/// never again), the embed is materialized there (the renderer's own
/// lowering reads the kit packs from a real directory, l0.rs:57/65), and a
/// marker carrying the embed's byte total short-circuits the re-write.
pub fn root() -> PathBuf {
    static ROOT: OnceLock<PathBuf> = OnceLock::new();
    ROOT.get_or_init(|| {
        let dir = match resolve_base(
            override_dir().as_deref(),
            HOST_DIR.read().unwrap().as_deref(),
            std::env::var_os("HOME").as_deref(),
        ) {
            Some(dir) => dir,
            // No writable base anywhere: never temp_dir. The process dir is
            // the honest last resort on a dev box — logged loudly.
            None => {
                let fallback = std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join(".octoscode-design");
                makepad_widgets::log!(
                    "[octoscode] design root: no HOME and no host files dir — falling back to {}",
                    fallback.display()
                );
                fallback
            }
        };
        makepad_widgets::log!("[octoscode] design root: {}", dir.display());
        let marker = dir.join(".embed-marker");
        let total = embedded_bytes().to_string();
        if std::fs::read_to_string(&marker).is_ok_and(|m| m == total) {
            return dir;
        }
        if let Err(e) = std::fs::create_dir_all(&dir) {
            makepad_widgets::log!(
                "[octoscode] design root: cannot create {}: {e} (design reads will fail)",
                dir.display()
            );
            return dir;
        }
        let mut failed = 0usize;
        for (rel, bytes) in table() {
            let path = dir.join(rel);
            if let Some(parent) = path.parent() {
                if std::fs::create_dir_all(parent).is_err() {
                    failed += 1;
                    continue;
                }
            }
            if std::fs::write(&path, bytes).is_err() {
                failed += 1;
            }
        }
        if failed > 0 {
            makepad_widgets::log!(
                "[octoscode] design embed: {failed} file(s) failed to materialize under {}",
                dir.display()
            );
        }
        // (a) the faces are visible in the log: count + one full path.
        let faces: Vec<&str> = table()
            .iter()
            .filter(|(p, _)| p.starts_with("ux/") && p.ends_with(".ttf"))
            .map(|(p, _)| *p)
            .collect();
        if !faces.is_empty() {
            makepad_widgets::log!(
                "[octoscode] design embed: {} font face(s) under {} (e.g. {})",
                faces.len(),
                dir.join("ux/").display(),
                dir.join(faces[0]).display()
            );
        }
        let _ = std::fs::write(&marker, total);
        dir
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

/// #32g item 1: the lowered DSL names kit faces as
/// `crate_resource("self:resources/ux/<file>")` — the crate's own resources,
/// which the APK does NOT package (the phone drew every card in the mono
/// fallback). Rewrite to the renderer's other supported form,
/// `file_resource("<absolute path>")` (the renderer itself lowers `file:`
/// sources to exactly this): the dev checkout's copy when present (desktop
/// byte-identical), else the materialized design root (`ux/…` rides in the
/// embed).
pub fn with_fonts(lowered: Result<String, String>) -> Result<String, String> {
    const NEEDLE: &str = "crate_resource(\"self:resources/ux/";
    let dsl = lowered?;
    if !dsl.contains(NEEDLE) {
        return Ok(dsl);
    }
    let mut out = String::with_capacity(dsl.len());
    let mut rest = dsl.as_str();
    while let Some(i) = rest.find(NEEDLE) {
        out.push_str(&rest[..i]);
        rest = &rest[i + NEEDLE.len()..];
        let Some(end) = rest.find('"') else {
            out.push_str(NEEDLE);
            break;
        };
        let name = &rest[..end];
        // The needle consumed the `ux/` prefix — the faces live in
        // resources/ux/ (the checkout) and <root>/ux/ (the embed). b408a89
        // sought them at the ROOT (device log: ".../design/Inter-600.ttf is
        // not available in this build") and the family rendered EMPTY — no
        // text at all.
        let path = font_file(&format!("ux/{name}"));
        // Skip past the quoted name FIRST — the original crate_resource's
        // closing `)` follows the quote. (The previous strip ran BEFORE the
        // skip: a silent no-op, the `)` survived, and the doubled `))`
        // broke the DSL parse — "Expected } not found" — the whole card
        // drew nothing.)
        rest = &rest[end + 1..];
        if path.is_file() {
            // (b) the absolute, packaged path; consume the original `)`.
            out.push_str(&format!("file_resource({path:?})"));
            rest = rest.strip_prefix(')').unwrap_or(rest);
        } else {
            // (c) never draw nothing: keep the crate-relative form (a
            // broken materialization degrades to the mono fallback, not an
            // empty family); the `)` in rest closes it.
            out.push_str(NEEDLE);
            out.push_str(name);
            out.push('"');
        }
    }
    out.push_str(rest);
    Ok(out)
}

/// The absolute file for a kit face (`ux/<file>.ttf`): the dev checkout's
/// own resources first, else the materialized design root.
pub fn font_file(rel: &str) -> PathBuf {
    let own = Path::new(manifest_dir()).join("resources").join(rel);
    if own.is_file() {
        return own;
    }
    root().join(rel)
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
    fn the_root_base_prefers_the_host_and_never_temp_dir() {
        // The entry's case: HOME unset + the host files dir given -> under
        // the host dir, never temp_dir (Android's /data/local/tmp is
        // unwritable for an app).
        let host =
            PathBuf::from("/data/user/0/dev.makepad.octosense.octoscode/files");
        let got = resolve_base(None, Some(host.as_path()), None).expect("host dir is a base");
        assert_eq!(got, host.join(".octoscode").join("design"));
        assert!(!got.starts_with(std::env::temp_dir()), "never temp_dir");
        // Desktop unchanged: no host, HOME set -> $HOME/.octoscode/design.
        // The path is ASSEMBLED, never a machine-path literal — the hermetic
        // gate scans source lines for absolute-path shapes (its own literals
        // are assembled the same way; repo_hermetic.rs:51).
        let home = PathBuf::from(format!("/{}/dev", "home"));
        let got = resolve_base(None, None, Some(home.as_os_str())).expect("home is a base");
        assert_eq!(got, home.join(".octoscode").join("design"));
        // No writable base anywhere -> None (the caller logs; no temp_dir).
        assert_eq!(resolve_base(None, None, None), None);
        // The env override (OCTOSCODE_DESIGN_DIR) wins over both.
        let got = resolve_base(
            Some(Path::new("/tmp/design-override")),
            Some(host.as_path()),
            None,
        )
        .expect("env override");
        assert_eq!(got, PathBuf::from("/tmp/design-override"));
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
    fn kit_fonts_rewrite_to_packaged_files() {
        let dsl = "x := crate_resource(\"self:resources/ux/Inter-400.ttf\")";
        let out = with_fonts(Ok(dsl.to_owned())).expect("ok");
        assert!(out.contains("file_resource("), "{out}");
        assert!(!out.contains("self:resources/ux/"), "{out}");
        // The emitted path carries the ux/ DIRECTORY (b408a89's regression:
        // the needle consumed the prefix and the face was sought at the
        // root — the family rendered empty and the card drew no text).
        let start = out.find('"').expect("quoted path") + 1;
        let end = out[start..].find('"').expect("closing") + start;
        let path = &out[start..end];
        assert!(path.ends_with("ux/Inter-400.ttf"), "{path}");
        assert!(Path::new(path).is_file(), "{path}");
        // The emitted call is well-formed: the original `crate_resource`'s
        // closing paren is consumed — a doubled `))` is the parse breaker
        // that blanked the whole card (b408a89).
        assert!(!out.contains("))"), "{out}");
        assert!(out.ends_with(".ttf\")"), "{out}");
    }

    #[test]
    fn a_face_missing_on_disk_keeps_the_original_reference() {
        // (c): the guard never points the renderer at a nonexistent file —
        // the original form stays (the mono fallback), never an empty family.
        let dsl = "x := crate_resource(\"self:resources/ux/NotARealFace.ttf\")";
        let out = with_fonts(Ok(dsl.to_owned())).expect("ok");
        assert!(
            out.contains("crate_resource(\"self:resources/ux/NotARealFace.ttf\")"),
            "{out}"
        );
        assert!(!out.contains("file_resource("), "{out}");
    }

    #[test]
    fn the_materialized_root_carries_the_five_faces() {
        let dir = root();
        for f in [
            "ux/Inter-400.ttf",
            "ux/Inter-500.ttf",
            "ux/Inter-600.ttf",
            "ux/Inter-700.ttf",
            "ux/LiberationMono-Regular.ttf",
        ] {
            let p = dir.join(f);
            assert!(p.is_file(), "missing {}", p.display());
            assert!(
                std::fs::metadata(&p).map(|m| m.len() > 100_000).unwrap_or(false),
                "{} is not a real face",
                p.display()
            );
        }
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
