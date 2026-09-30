//! Card #30e §Tests — the theme preference's cycle order and stored shape, the
//! card selection it drives (dark Stage B cards vs their light twins), the
//! one-owner isolation of the `theme.*` ids, and that all four selectable
//! screens lower through the module's own path.
//!
//! The web contract under test: the sidebar button cycles **system → dark →
//! light → system** (`use-theme.ts:44-49`), only light/dark persist and
//! `system` means the stored key is ABSENT (`use-theme.ts:9-27`).
use octoscode_module::screens::theme;

/// The four state-reading tests share the module's THEME static; `cargo test`
/// runs them in parallel, so they take this lock (the lowering test is
/// state-independent — it pins explicit card names).
static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// The module's own Ctx shape; the theme table reads neither store nor ui.
fn ctx() -> octoscode_module::bindings::Ctx<'static> {
    static STORE: std::sync::OnceLock<std::sync::Arc<octoscode_store::Store>> =
        std::sync::OnceLock::new();
    static UI: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<octoscode_module::flow::FlowUi>>> =
        std::sync::OnceLock::new();
    let store = STORE.get_or_init(|| std::sync::Arc::new(octoscode_store::Store::new()));
    let ui = UI
        .get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(Default::default())));
    octoscode_module::bindings::Ctx::new(store, ui)
}

#[test]
fn f30e_cycle_order_and_stored_shape() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    theme::reset_state();
    // Fresh profile = system, and system persists NOTHING (use-theme.ts:20-22).
    assert_eq!(theme::preference(), "system");
    assert_eq!(theme::Theme::System.stored(), None);
    // system -> dark -> light -> system (use-theme.ts:44-49).
    let (p1, r1) = theme::cycle();
    assert_eq!((p1.as_str(), r1), ("dark", "dark"));
    let (p2, r2) = theme::cycle();
    assert_eq!((p2.as_str(), r2), ("light", "light"));
    let (p3, r3) = theme::cycle();
    assert_eq!((p3.as_str(), r3), ("system", "dark")); // system resolves dark
    assert_eq!(theme::preference(), "system");
    // Only light/dark have a stored value; labels match the sidebar.
    assert_eq!(theme::Theme::Dark.stored(), Some("dark"));
    assert_eq!(theme::Theme::Light.stored(), Some("light"));
    assert_eq!(theme::Theme::Dark.label(), "Dark");
    assert_eq!(theme::Theme::Light.label(), "Light");
    assert_eq!(theme::Theme::System.label(), "System");
    theme::reset_state();
}

#[test]
fn f30e_set_preference_rejects_unknown() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    theme::reset_state();
    // parseDisplayPreferences rejects junk and keeps nothing (model.ts:35-56):
    // the setter fails and the preference stands.
    assert!(!theme::set_preference("midnight"));
    assert_eq!(theme::preference(), "system");
    assert!(theme::set_preference("dark"));
    assert!(!theme::set_preference(""));
    assert_eq!(theme::preference(), "dark");
    theme::reset_state();
}

#[test]
fn f30e_card_selection_follows_resolved_theme() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    theme::reset_state();
    // #31d: card selection is FIXED — the follow names ALWAYS select the light
    // card (the palette every kit ships); the theme lives in retint_dsl, not
    // card swapping. The explicit names pin their atlas card.
    for (which, card) in [
        ("conversation", "conversation-03"),
        ("settings", "setup-06"),
        ("light_conv", "conversation-03"),
        ("light_settings", "setup-06"),
        ("dark_conv", "autonomy-11"),
        ("dark_settings", "autonomy-12"),
    ] {
        assert_eq!(theme::card_for(which).unwrap().0, card, "which={which}");
    }
    assert!(theme::card_for("palette").is_none());
    // The FOLLOW lowers carry the RESOLVED palette: system (no reader) falls
    // back to dark, so the light card's DSL comes back retinted to dark
    // tokens; pinned names stay verbatim.
    let conv = theme::lower("conversation", ctx().store).expect("conversation lowers");
    assert!(
        conv.contains("#1c1f22") || conv.contains("#1c1c1e"),
        "conversation under system should retint to dark tokens"
    );
    let pinned = theme::lower("dark_conv", ctx().store).expect("dark_conv lowers");
    assert!(pinned.contains("#1c1c1e"), "dark_conv stays the dark atlas card");
    theme::set_preference("light");
    let conv_light = theme::lower("conversation", ctx().store).expect("light lowers");
    assert!(
        conv_light.contains("#fefefe"),
        "conversation under light is the byte-identical light card"
    );
    theme::reset_state();
}

#[test]
fn f30e_ids_have_a_single_owner() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    theme::reset_state();
    // theme.* binds/actions are THIS screen's (one-owner rule, LESSONS).
    assert!(theme::owns_binding("theme.preference"));
    assert!(theme::owns_binding("theme.resolved"));
    assert!(theme::is_action("theme.cycle"));
    assert!(!theme::owns_binding("palette.query"));
    assert!(!theme::is_action("palette.run"));
    assert!(!theme::is_action("error.reload"));
    assert!(!theme::is_action("connection.retry"));
    assert!(!theme::is_action("settings.close"));
    // ...and the theme screen owns nobody else's ids.
    assert!(!theme::owns_binding("palette.query"));
    assert!(!theme::is_action("composer.draft"));
    // resolve maps the declared id and names the unknown one.
    match theme::resolve("theme.cycle", 0, &ctx()) {
        theme::Effect::Cycle { preference, resolved } => {
            assert_eq!(preference, "dark"); // system -> dark
            assert_eq!(resolved, "dark");
        }
        other => panic!("expected Cycle, got {other:?}"),
    }
    match theme::resolve("theme.nope", 0, &ctx()) {
        theme::Effect::Unhandled(id) => assert_eq!(id, "theme.nope"),
        other => panic!("expected Unhandled, got {other:?}"),
    }
    // The binding table answers JSON for its own ids only. (Reset first: the
    // resolve arms above cycled the static to dark — this section pins the
    // fresh-profile answers.)
    theme::reset_state();
    let c = ctx();
    assert_eq!(
        theme::query(&c, "theme.preference").and_then(|v| v.as_str().map(str::to_owned)),
        Some("system".to_owned())
    );
    assert_eq!(
        theme::query(&c, "theme.resolved").and_then(|v| v.as_str().map(str::to_owned)),
        Some("dark".to_owned())
    );
    assert!(theme::query(&c, "palette.query").is_none());
    theme::reset_state();
}

#[test]
fn f30e_all_four_cards_lower_through_the_module_path() {
    // Each selectable card lowers through the DESIGN branch (the artifact
    // Gate B rendered) and carries its OWN palette: the dark cards' bg token
    // is the dark disc (autonomy-11 #1C1C1E = 0xff1c1c1e), the light twins'
    // is white/near-white. A handful of assertions pin the DSL, not files.
    let dark = theme::lower("dark_conv", ctx().store).expect("dark_conv lowers");
    assert!(dark.contains("#1c1c1e"),
        "dark_conv should carry the dark disc bg (#1c1c1e..), got {} bytes", dark.len());
    let dark_set = theme::lower("dark_settings", ctx().store).expect("dark_settings lowers");
    assert!(dark_set.contains("#191c1f"), "dark_settings should carry #191c1f..");
    let light_conv = theme::lower("light_conv", ctx().store).expect("light_conv lowers");
    assert!(light_conv.contains("#fefefe"), "light_conv should carry the near-white bg");
    let light_set = theme::lower("light_settings", ctx().store).expect("light_settings lowers");
    assert!(light_set.contains("#ffffff"), "light_settings should carry the white bg");
    assert!(theme::lower("nope", ctx().store).is_err());
}
