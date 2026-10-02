//! A26 — parity row `preferences/g-timeline` "Choose one of five named display
//! palettes (terminal, codex, claude, slate, solarized); Terminal follows the
//! browser appearance" (web `features/preferences/model.ts:2-8`,
//! `PreferencesDialog.tsx:55-75`, `app/theme.css:122-258`,
//! `palettes.test.ts`), on the production path a palette row's click takes:
//!
//!   the row's action id (`a9.prefs.palette.<id>`, chrome.rs `pl_hit`)
//!   -> `a9_prefs::routes` -> `a9_prefs::choose_palette` (the whitelist's
//!   `theme`, unsaved) + `theme::set_palette` -> the host re-themes
//!   (`a26_host::retheme`): the shell roles, the shell inks and every
//!   lowered surface's retint read the palette; Save writes the whitelist;
//!   the next launch adopts it (`theme::init_persistence`).
//!
//! The contrast guard over every palette lives with the guard
//! (`screens::theme::contrast_tests`).
use std::sync::Mutex;

use octoscode_module::screens::a9_prefs;
use octoscode_module::screens::theme::{self, Look, Palette, CODEX, SOLARIZED};

/// The theme / prefs statics are process-wide: one test at a time.
static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn temp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("a26-palettes-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A fresh profile: Terminal on a light appearance, the stores in `dir`.
fn fresh(dir: &std::path::Path) {
    std::env::set_var("OCTOSCODE_DISPLAY_PREFS_PATH", dir.join("display-v1.json"));
    std::env::set_var("OCTOSCODE_PREF_PATH", dir.join("display.json"));
    a9_prefs::reset();
    theme::reset_state();
    assert!(theme::set_preference("light"));
}

/// The user bubble as the transcript lowers it (the production component
/// path, retint included).
fn bubble() -> String {
    octoscode_module::components::lower(
        octoscode_module::components::ItemKind::UserBubble,
        "0",
        &[("t01_text".to_owned(), "Fix the steer queue".to_owned()), ("t02_text".to_owned(), String::new())],
    )
    .expect("the user bubble lowers")
}

#[test]
fn a_palette_row_applies_its_palette_everywhere_and_save_keeps_it_for_the_next_launch() {
    let _g = serial();
    let dir = temp("save");
    fresh(&dir);
    assert_eq!(theme::current_look(), Look::Light, "a fresh profile draws Terminal's light look");
    let light_bubble = bubble();

    // The Codex row's click.
    let action = "a9.prefs.palette.codex";
    assert!(a9_prefs::routes(action), "the palette rows' ids have an owner");
    assert_eq!(a9_prefs::choose_palette(action), Some(Palette::Codex), "the look changed: the host re-themes");
    assert_eq!(theme::current_look(), Look::Named(Palette::Codex));
    assert_eq!(theme::resolved(), "dark", "a named palette is dark by construction (theme.css:133-137)");
    // Applied, not saved (the web's setTheme; Save remembers it).
    let snap = a9_prefs::snapshot();
    assert_eq!(snap.current.theme, "codex");
    assert_eq!(snap.status(), "Unsaved preferences.");

    // The shell: its roles and inks are the palette's.
    let roles = theme::role_assignments();
    assert!(roles.contains(&format!("mod.theme.color_bg_app = {}", CODEX.surface)), "{roles}");
    assert!(roles.contains(&format!("mod.theme.color_fg_app = {}", CODEX.text)), "{roles}");
    assert!(roles.contains(&format!("mod.theme.color_bg_even = {}", CODEX.alt)), "{roles}");
    assert!(roles.contains(&format!("mod.theme.color_text_muted = {}", CODEX.muted)), "{roles}");
    assert!(!roles.contains("#1c1f22") && !roles.contains("#f5f5f7"), "no stock dark role leaks in: {roles}");
    assert_eq!(theme::shell_ink("link"), CODEX.accent_text);
    assert_eq!(theme::shell_ink("accent"), CODEX.accent);

    // A lowered conversation surface: the bubble in the palette's colours.
    let codex_bubble = bubble();
    assert_ne!(codex_bubble, light_bubble);
    assert!(codex_bubble.contains(&format!("{}ff", &CODEX.alt)), "the bubble's raised fill: {codex_bubble}");
    assert!(codex_bubble.contains(&format!("{}ff", &CODEX.text)), "the bubble's ink");
    assert!(!codex_bubble.contains("#2c2c2eff") && !codex_bubble.contains("#f5f5f7ff"), "no stock dark ink");

    // Code highlighting follows the palette (the web's note:
    // "Named palettes also update code highlighting").
    let answer = octoscode_module::fluid::assistant_answer(
        "0",
        &octoscode_module::markdown::display("Run:\n\n```rust\nfn main() {}\n```\n", false),
        None,
        &octoscode_module::conv_layout::Metrics::for_window(990.0, true),
    );
    assert!(answer.contains("palette: \"codex\""), "the code body is coloured for the palette: {answer}");
    use octoscode_module::highlight::Tok;
    assert_eq!(Tok::Keyword.color_look(theme::current_look()), CODEX.danger_text);
    assert_eq!(Tok::String.color_look(theme::current_look()), CODEX.success_text);

    // Save writes exactly the whitelist with the palette.
    assert!(a9_prefs::save());
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("display-v1.json")).unwrap()).unwrap();
    assert_eq!(doc["theme"], "codex");
    assert_eq!(doc.as_object().unwrap().len(), 4, "only the whitelist: {doc}");
    assert_eq!(a9_prefs::snapshot().status(), "Preferences saved.");

    // The next launch: a fresh process state adopts the saved palette at
    // startup (the shell's eval_roles -> init_persistence).
    a9_prefs::reset();
    theme::reset_state();
    assert_eq!(theme::palette(), Palette::Terminal);
    theme::init_persistence();
    theme::clear_os_reader();
    assert_eq!(theme::palette(), Palette::Codex, "the saved palette is drawn from the first frame");
    assert!(theme::role_assignments().contains(CODEX.surface));

    theme::reset_state();
    a9_prefs::reset();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_palette_draws_the_shell_and_the_conversation_in_its_own_colours() {
    let _g = serial();
    let dir = temp("every");
    fresh(&dir);
    let mut seen_surfaces = Vec::new();
    for p in Palette::ALL {
        let action = format!("a9.prefs.palette.{}", p.id());
        assert!(a9_prefs::routes(&action), "{action}");
        a9_prefs::choose_palette(&action);
        assert_eq!(theme::palette(), p);
        let roles = theme::role_assignments();
        let surface = roles
            .lines()
            .find_map(|l| l.strip_prefix("mod.theme.color_bg_app = "))
            .expect("the window role")
            .to_owned();
        match p.named() {
            // Terminal on the light appearance: the board's light shell.
            None => assert_eq!(surface, "#ffffff"),
            Some(c) => {
                assert_eq!(surface, c.surface, "{}", p.id());
                let b = bubble();
                assert!(b.contains(&format!("{}ff", c.alt)) && b.contains(&format!("{}ff", c.text)), "{}: {b}", p.id());
            }
        }
        seen_surfaces.push(surface);
    }
    seen_surfaces.sort();
    seen_surfaces.dedup();
    assert_eq!(seen_surfaces.len(), 5, "five distinct palettes");
    // The labels are the web's (PreferencesDialog.tsx:9-15).
    let labels: Vec<&str> = Palette::ALL.iter().map(|p| p.label()).collect();
    assert_eq!(labels, ["Terminal", "Codex", "Claude", "Slate", "Solarized"]);
    theme::reset_state();
    a9_prefs::reset();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn terminal_follows_the_appearance_and_a_named_palette_overrides_it() {
    let _g = serial();
    let dir = temp("follow");
    fresh(&dir);
    // Terminal: the appearance decides (palettes.test.ts:21-24).
    assert_eq!(theme::current_look(), Look::Light);
    assert!(theme::set_preference("dark"));
    assert_eq!(theme::current_look(), Look::Dark);
    assert!(theme::role_assignments().contains("mod.theme.color_bg_app = #1c1f22"));
    // A named palette overrides either appearance.
    a9_prefs::choose_palette("a9.prefs.palette.solarized");
    for pref in ["light", "dark", "system"] {
        assert!(theme::set_preference(pref));
        assert_eq!(theme::current_look(), Look::Named(Palette::Solarized), "{pref}");
        assert!(theme::role_assignments().contains(SOLARIZED.surface), "{pref}");
    }
    // Back to Terminal: the appearance decides again.
    a9_prefs::choose_palette("a9.prefs.palette.terminal");
    assert!(theme::set_preference("light"));
    assert_eq!(theme::current_look(), Look::Light);
    theme::reset_state();
    a9_prefs::reset();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_unknown_palette_is_refused_and_changes_nothing() {
    let _g = serial();
    let dir = temp("unknown");
    fresh(&dir);
    for bad in ["a9.prefs.palette.neon", "a9.prefs.palette.CODEX", "a9.prefs.palette.", "a9.prefs.palette.system"] {
        assert!(!a9_prefs::routes(bad), "{bad}");
        assert_eq!(a9_prefs::choose_palette(bad), None, "{bad}");
    }
    assert_eq!(theme::palette(), Palette::Terminal);
    assert_eq!(a9_prefs::snapshot().current.theme, "terminal");
    assert!(!a9_prefs::snapshot().dirty());
    // Choosing the palette already in effect is no change (no re-theme).
    assert_eq!(a9_prefs::choose_palette("a9.prefs.palette.terminal"), None);
    theme::reset_state();
    a9_prefs::reset();
    let _ = std::fs::remove_dir_all(&dir);
}
