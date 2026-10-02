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

/// Total embedded bytes (the ACK reports it).
pub fn embedded_bytes() -> usize {
    table().iter().map(|(_, b)| b.len()).sum()
}

/// The materialize marker: byte total plus an FNV-1a 64 fingerprint over every
/// embedded path and its bytes. The byte total alone missed same-size edits:
/// an icon edit that kept its byte count was never re-materialized, so the app
/// kept drawing the stale file (found by #A2 with its spinner and eye icons).
pub fn embedded_fingerprint() -> String {
    static FP: OnceLock<String> = OnceLock::new();
    FP.get_or_init(|| {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |bytes: &[u8]| {
            for b in bytes {
                h ^= u64::from(*b);
                h = h.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        for (path, bytes) in table() {
            eat(path.as_bytes());
            eat(&[0]);
            eat(bytes);
        }
        format!("{}-{:016x}", embedded_bytes(), h)
    })
    .clone()
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

/// The host's app data dir, when the host handed one over (Android's files
/// dir); `None` on the desktop (A1: the credential store roots there).
pub fn host_dir() -> Option<PathBuf> {
    HOST_DIR.read().unwrap().clone()
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
        let total = embedded_fingerprint();
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
    // Generalised (#32g, outer loop device evidence): rewrite EVERY
    // crate_resource("self:resources/…") — the kit faces AND the kit/icon
    // svgs — to the materialized root's absolute path. The needle captures
    // the full relative path after self:resources/ (ux/Inter-400.ttf,
    // icons/chevron_down.svg, …).
    const NEEDLE: &str = "crate_resource(\"self:resources/";
    // A1: the kit's CJK member is a calligraphic face (LXGW WenKai) that
    // clashes with Inter; every lowered text style gets the sans face first.
    let dsl = cjk_sans(&lowered?);
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
        let rel = &rest[..end];
        // Skip past the quoted name FIRST — the original crate_resource's
        // closing `)` follows the quote. (The previous strip ran BEFORE the
        // skip: a silent no-op, the `)` survived, and the doubled `))`
        // broke the DSL parse — "Expected } not found" — the whole card
        // drew nothing.)
        rest = &rest[end + 1..];
        let path = font_file(rel);
        if path.is_file() {
            // (b) the absolute, packaged path; consume the original `)`.
            out.push_str(&format!("file_resource({path:?})"));
            rest = rest.strip_prefix(')').unwrap_or(rest);
        } else {
            // (c) never draw nothing: keep the crate-relative form (a
            // broken materialization degrades to the mono fallback, not an
            // empty family); the `)` in rest closes it.
            out.push_str(NEEDLE);
            out.push_str(rel);
            out.push('"');
        }
    }
    out.push_str(rest);
    Ok(out)
}

/// The bundled sans CJK face for a text weight (A1): Noto Sans SC, subset to
/// GB2312 (Regular) / GB2312 level 1 (SemiBold) — `resources/ux/NotoSansSC-
/// OFL.txt` carries the license and the subset recipe. `None` when the file
/// cannot be resolved (a broken materialization): callers then keep the
/// renderer's own member, so Chinese never draws blank.
pub fn cjk_face(weight: u32) -> Option<PathBuf> {
    let rel = if weight >= 600 {
        "ux/NotoSansSC-SemiBold.ttf"
    } else {
        "ux/NotoSansSC-Regular.ttf"
    };
    let path = font_file(rel);
    path.is_file().then_some(path)
}

/// The sans CJK face's path as a string, for the shell's static `script_mod!`
/// families (`file_resource(#(crate::design::cjk_face_path(400)))`): the
/// operator chose Noto Sans SC for ALL UI text (2026-10-02, board 4). The
/// materialized design root carries it like the Inter faces.
pub fn cjk_face_path(weight: u32) -> String {
    let rel = if weight >= 600 { "ux/NotoSansSC-SemiBold.ttf" } else { "ux/NotoSansSC-Regular.ttf" };
    font_file(rel).display().to_string()
}

/// The CJK members of a font family (A1), as DSL: the sans face eagerly, then
/// the renderer's LXGW WenKai as a LAZY last resort (`FontMember.lazy: 1`
/// loads it only after a glyph the subset lacks — draw_text.rs:3500-3507), so
/// a rare hanzi still renders instead of a tofu box.
///
/// The row metrics come from the family's FIRST member (`layouter.rs:522`,
/// `finish_current_row` reads `font_family.fonts().first()`), so a mixed
/// Chinese/Latin line keeps Inter's baseline and line pitch: the CJK members
/// carry no ascender/descender fudge of their own.
pub fn cjk_members(weight: u32) -> String {
    let wenkai = if weight >= 600 {
        "LXGWWenKaiBold.ttf"
    } else {
        "LXGWWenKaiRegular.ttf"
    };
    let fallback = format!(
        "crate_resource(\"makepad_widgets:resources/{wenkai}\") asc: 0.0 desc: 0.0"
    );
    match cjk_face(weight) {
        Some(sans) => format!(
            "cjk := FontMember{{res: file_resource({:?}) asc: 0.0 desc: 0.0}} \
             cjk_rare := FontMember{{res: {fallback} lazy: 1}}",
            sans.display().to_string()
        ),
        None => format!("cjk := FontMember{{res: {fallback}}}"),
    }
}

/// Rewrite every renderer-emitted WenKai CJK member to [`cjk_members`] (A1).
///
/// The renderer (octoscript-makepad `design.rs:536/632`, `lib.rs:829`) emits
/// `cjk := FontMember{res: crate_resource("makepad_widgets:resources/
/// LXGWWenKai{Regular,Bold}.ttf") asc: … desc: … [weight: …]}`; the member
/// ends at its first `}`. A DSL with no such member passes through unchanged.
pub fn cjk_sans(dsl: &str) -> String {
    const MEMBER: &str = "cjk := FontMember{res: crate_resource(\"makepad_widgets:resources/LXGWWenKai";
    if !dsl.contains(MEMBER) {
        return dsl.to_owned();
    }
    let mut out = String::with_capacity(dsl.len() + 256);
    let mut rest = dsl;
    while let Some(at) = rest.find(MEMBER) {
        out.push_str(&rest[..at]);
        let tail = &rest[at..];
        let Some(close) = tail.find('}') else {
            out.push_str(tail);
            return out;
        };
        let bold = tail[MEMBER.len()..close].starts_with("Bold");
        out.push_str(&cjk_members(if bold { 600 } else { 400 }));
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    out
}

/// The resource STRING for one of the module's own icons
/// (`icons/<file>.svg`) — the spliced value for the script_mod! literals
/// (`draw_svg.svg: #(crate::design::icon_resource("chevron_down.svg"))`):
/// a literal `crate_resource("self:…")` resolves against the BUILD
/// machine's manifest path (script/res.rs:1067 concatenates
/// ScriptMod::cargo_manifest_path), which does not exist on the phone; the
/// spliced runtime value carries the materialized root's absolute path.
pub fn icon_resource(name: &str) -> String {
    font_file(&format!("icons/{name}")).display().to_string()
}

/// The absolute file for a kit face (`ux/<file>.ttf`): the MATERIALIZED
/// design root first — it exists on every target (desktop: `$HOME`/host
/// files dir; phone: the app's own storage, device-verified in #32f) and is
/// what the outer loop's device log demanded (`font member` must never name
/// the build machine: 9394a44 resolved the checkout path, which only exists
/// where the APK was built). The dev checkout is the LAST resort. The first
/// resolution is logged once — the device log then shows which tree won.
pub fn font_file(rel: &str) -> PathBuf {
    let resolved = {
        let from_root = root().join(rel);
        if from_root.is_file() {
            from_root
        } else {
            let own = Path::new(manifest_dir()).join("resources").join(rel);
            if own.is_file() {
                own
            } else {
                from_root
            }
        }
    };
    static LOGGED: OnceLock<()> = OnceLock::new();
    LOGGED.get_or_init(|| {
        makepad_widgets::log!(
            "[octoscode] font faces resolve under {}",
            resolved.display()
        );
    });
    resolved
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
        // #FX1 (the merge-tree suite catch): lane isolation re-roots the tree
        // (OCTOSCODE_DESIGN_DIR -> the repo's design/, which carries NO ux/
        // faces — they ride in crate resources, and the renderer's
        // missing-face guard above falls back to them, so production is
        // correct there). The five-faces premise holds on the DEFAULT
        // materialization only; under an override assert what DOES hold
        // (the embed materialized) and say what was skipped.
        if override_dir().is_some() {
            let marker = dir.join(".embed-marker");
            assert!(
                marker.is_file(),
                "the override root is materialized: {}",
                marker.display()
            );
            eprintln!(
                "override root {} (no ux/ faces in the repo tree — the five-faces \
                 assert applies to the default materialization only)",
                dir.display()
            );
            return;
        }
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
        // The ONE shape both resolution paths produce: a read error that
        // names the file asked for. Under the embedded default it is exactly
        // "read no/such/file.card: …"; under an override dir the path is
        // absolute, so pin the prefix + the name, not the whole prefix match.
        assert!(e.starts_with("read "), "{e}");
        assert!(e.contains("no/such/file.card"), "{e}");
    }

    #[test]
    fn the_embedded_copy_parses_as_utf8_text() {
        let text = file("cards/index.json").expect("embedded");
        assert!(text.contains("\"cards\""), "the manifest shape");
    }

    /// A1 — Chinese rendered in LXGW WenKai, a calligraphic face beside Inter.
    /// Every renderer-emitted WenKai member becomes the bundled sans face
    /// (eager) with WenKai kept as a LAZY rare-glyph fallback; the weight
    /// picks the SemiBold subset. FAILS before A1 (no rewrite: the WenKai
    /// member stays first).
    #[test]
    fn the_kit_cjk_member_is_the_sans_face_with_wenkai_as_a_lazy_fallback() {
        let regular = "TextStyle{font_family: FontFamily{latin := FontMember{res: file_resource(\"/x/Inter-400.ttf\") asc: 0.04 desc: 0.04 weight: 400} cjk := FontMember{res: crate_resource(\"makepad_widgets:resources/LXGWWenKaiRegular.ttf\") asc: 0.0 desc: 0.0 weight: 400} emoji := FontMember{res: x asc: 0 desc: 0}} font_size: 11.25}";
        let out = cjk_sans(regular);
        let sans = out.find("NotoSansSC-Regular.ttf").expect("the sans face is a member");
        let wenkai = out.find("LXGWWenKaiRegular.ttf").expect("WenKai stays as the rare-glyph fallback");
        assert!(sans < wenkai, "the sans face must come first: {out}");
        assert!(out.contains("lazy: 1"), "WenKai loads only after a miss: {out}");
        assert!(out.starts_with("TextStyle{font_family: FontFamily{latin := "), "latin stays first (row metrics): {out}");
        assert!(out.ends_with("font_size: 11.25}"), "the rest of the style is untouched: {out}");
        let bold = regular.replace("LXGWWenKaiRegular", "LXGWWenKaiBold");
        let out = cjk_sans(&bold);
        assert!(out.contains("NotoSansSC-SemiBold.ttf"), "bold text takes the SemiBold subset: {out}");
        assert!(out.contains("LXGWWenKaiBold.ttf"), "{out}");
        // No WenKai member: byte-identical.
        let plain = "Label{text: \"x\"}";
        assert_eq!(cjk_sans(plain), plain);
    }

    #[test]
    fn the_bundled_cjk_faces_are_real_ofl_subsets() {
        for w in [400, 600] {
            let p = cjk_face(w).expect("the CJK face resolves");
            let len = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            assert!(len > 1_000_000, "{} is not a real face ({len} bytes)", p.display());
            assert!(len < 3_000_000, "{} must stay a subset ({len} bytes)", p.display());
        }
        let ofl = Path::new(manifest_dir()).join("resources/ux/NotoSansSC-OFL.txt");
        let text = std::fs::read_to_string(&ofl).expect("the OFL text ships beside the faces");
        assert!(text.contains("SIL OPEN FONT LICENSE Version 1.1"));
        assert!(text.contains("Reserved Font Name 'Source'"));
    }
}
