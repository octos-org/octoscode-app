//! Card #31d §Tests — the app-wide theme's four workflows:
//! 1. the token set (retint_dsl: light byte-passthrough, dark token rewrite,
//!    transparent/8-digit untouched, UTF-8 safe),
//! 2. persistence (the web's JSON shape; system = the file is ABSENT),
//! 3. the OS appearance (injected reader drives `resolved` under system),
//! 4. the shell role assignments (both modes carry full role sets).
//! State tests share the module statics + env, so they run under one lock.
use octoscode_module::screens::theme;

static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn lock() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

/// A scratch pref file per test (env-scoped, so no machine paths hard-coded).
fn pref_path(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("octoscode-f31d-{tag}"));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("display.json")
}

#[test]
fn f31d_retint_light_is_byte_passthrough() {
    let _l = lock();
    theme::reset_state();
    theme::set_preference("light");
    let dsl = "view a { draw_bg.color: #ffffff text: \"…—‘quoted’\" draw_text.color: #6e6e73 }\n";
    assert_eq!(theme::retint_dsl(dsl), dsl, "light mode is the byte passthrough");
    theme::reset_state();
}

#[test]
fn f31d_retint_dark_rewrites_tokens_and_keeps_the_rest() {
    let _l = lock();
    theme::reset_state();
    theme::set_preference("dark");
    let dsl = "draw_bg.color: #FFFFFF // surface\ndraw_text.color: #6e6e73\nalpha: #00000000\nkeep: #2f6feb\nbubble: #f4f4f5\nsend: #050505\ntext: \"…em dash stays\" \n";
    let out = theme::retint_dsl(dsl);
    assert!(out.contains("#1c1f22"), "white surface -> dark app bg");
    assert!(out.contains("#98989d"), "secondary text -> dark muted");
    assert!(out.contains("#00000000"), "8-digit transparent untouched");
    assert!(out.contains("#2f6feb"), "accent (blue) is theme-invariant");
    assert!(out.contains("#2c2c2e"), "bubble fill -> the raised dark fill");
    assert!(out.contains("#f5f5f7"), "send disc -> the dark-mode disc");
    assert!(out.contains("…em dash stays"), "UTF-8 text survives the byte scan");
    // Idempotent: the dark output has no light-key literals left to remap.
    assert_eq!(theme::retint_dsl(&out), out, "dark output is a fixed point");
    theme::reset_state();
}

#[test]
fn f31d_persistence_web_shape_and_absence_is_system() {
    let _l = lock();
    theme::reset_state();
    let path = pref_path("shape");
    std::env::set_var("OCTOSCODE_PREF_PATH", &path);
    let _ = std::fs::remove_file(&path);
    // Fresh profile: nothing persisted, preference system.
    assert_eq!(theme::preference(), "system");
    assert_eq!(theme::load_preference(), None);
    // dark persists the web's object shape (version/theme/language/vimMode).
    theme::set_preference("dark");
    theme::save_preference();
    let doc: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(doc["version"], 1);
    assert_eq!(doc["theme"], "dark");
    assert_eq!(doc["language"], "en");
    assert_eq!(doc["vimMode"], false);
    // It round-trips: a fresh load adopts dark.
    assert_eq!(theme::load_preference(), Some(theme::Theme::Dark));
    // system DELETES the key (the absence IS system, use-theme.ts:20-22).
    theme::set_preference("system");
    theme::save_preference();
    assert!(!path.exists(), "system removes the stored key");
    assert_eq!(theme::load_preference(), None);
    // Corrupt JSON reads as absent (parseDisplayPreferences -> null parity).
    std::fs::write(&path, "{not json").unwrap();
    assert_eq!(theme::load_preference(), None);
    let _ = std::fs::remove_file(&path);
    std::env::remove_var("OCTOSCODE_PREF_PATH");
    theme::reset_state();
}

#[test]
fn f31d_os_appearance_reader_drives_system_resolution() {
    let _l = lock();
    theme::reset_state();
    assert_eq!(theme::preference(), "system");
    // No reader (or the default) -> the #30e deterministic dark fallback.
    theme::clear_os_reader();
    assert_eq!(theme::resolved(), "dark");
    // Inject light OS: system resolves light; explicit prefs ignore the OS.
    theme::set_os_reader(|| false);
    assert_eq!(theme::resolved(), "light");
    theme::set_preference("dark");
    assert_eq!(theme::resolved(), "dark", "explicit dark ignores the OS");
    theme::set_preference("system");
    theme::set_os_reader(|| true);
    assert_eq!(theme::resolved(), "dark");
    theme::clear_os_reader();
    theme::reset_state();
}

#[test]
fn f31d_role_assignments_cover_both_modes() {
    let _l = lock();
    theme::reset_state();
    theme::set_preference("light");
    let light = theme::role_assignments();
    assert!(light.contains("color_bg_app = #ffffff"), "light pins the shell's literals");
    assert!(light.contains("color_text_muted = #61666b"), "A18: the web's secondary label (WCAG 4.5:1 on every shell fill)");
    theme::set_preference("dark");
    let dark = theme::role_assignments();
    assert!(dark.contains("color_bg_app = #1c1f22"));
    assert!(dark.contains("color_text_muted = #98989d"));
    assert!(dark.contains("clear_color = #1c1f22"), "the window clear color follows");
    // Every shell role the light set assigns, the dark set assigns too.
    for line in light.lines().filter(|l| l.starts_with("mod.theme.")) {
        let role = line.split('=').next().unwrap().trim();
        assert!(dark.contains(role), "dark must assign {role} as well");
    }
    theme::reset_state();
}
