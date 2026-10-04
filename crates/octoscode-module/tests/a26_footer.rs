//! Sidebar footer routing: Fleet, the unified Theme shortcut, and Settings.
//! Theme opens Preferences on desktop and closes the phone drawer before
//! opening Preferences. The legacy cycle resolver remains covered separately.
use std::sync::Mutex;

use octoscode_module::chrome::{self, FooterEntry, Intent};
use octoscode_module::screens::theme;

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn ctx() -> octoscode_module::bindings::Ctx<'static> {
    static STORE: std::sync::OnceLock<std::sync::Arc<octoscode_store::Store>> = std::sync::OnceLock::new();
    static UI: std::sync::OnceLock<std::sync::Arc<std::sync::Mutex<octoscode_module::flow::FlowUi>>> =
        std::sync::OnceLock::new();
    let store = STORE.get_or_init(|| std::sync::Arc::new(octoscode_store::Store::new()));
    let ui = UI.get_or_init(|| std::sync::Arc::new(std::sync::Mutex::new(Default::default())));
    octoscode_module::bindings::Ctx::new(store, ui)
}

#[test]
fn the_footer_entries_keep_their_order_and_open_the_shared_theme_selector() {
    // ProductSidebar.tsx:969-1016: Fleet, the theme toggle, Settings.
    assert_eq!(FooterEntry::ALL, [FooterEntry::Fleet, FooterEntry::Theme, FooterEntry::Settings]);
    assert_eq!(
        FooterEntry::ALL.map(FooterEntry::hit),
        ["fleet_nav_hit", "sb_theme_hit", "sb_settings_hit"]
    );
    // The shortcut opens the same Preferences selector at both densities.
    for compact in [false, true] {
        let expected = if compact {
            vec![Intent::Action("drawer.close", 0), Intent::Action("settings.section.preferences", 0)]
        } else {
            vec![Intent::Action("settings.section.preferences", 0)]
        };
        assert_eq!(chrome::footer_intents(FooterEntry::Theme, compact), expected);
    }
    // Settings opens Settings; on a phone the drawer closes first
    // (App.tsx:2333-2336).
    assert_eq!(chrome::footer_intents(FooterEntry::Settings, false), vec![Intent::OpenSettings]);
    assert_eq!(
        chrome::footer_intents(FooterEntry::Settings, true),
        vec![Intent::Action("drawer.close", 0), Intent::OpenSettings]
    );
    // The collapsed rail keeps the icons only (ProductSidebar.tsx:981/995/
    // 1015 render no label when collapsed), centred on the rail buttons'
    // line: the 15 px icon in the rail's 36 px content box.
    assert_eq!(chrome::footer_row_inset(true) * 2.0 + 15.0, 36.0);
    assert_eq!(chrome::footer_row_inset(false), 8.0, "the column keeps the rows' 8 px inset");
    assert_eq!(
        FooterEntry::ALL.map(FooterEntry::label),
        ["fleet_nav_label", "sb_theme_label", "sb_settings_label"]
    );
}

#[test]
fn legacy_theme_actions_still_cycle_and_relabel() {
    let _g = serial();
    let dir = std::env::temp_dir().join(format!("a26-footer-theme-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pref = dir.join("display.json");
    std::env::set_var("OCTOSCODE_PREF_PATH", &pref);
    theme::reset_state();
    assert_eq!((theme::preference(), chrome::theme_label("system")), ("system".to_owned(), "System"));
    assert_eq!(chrome::footer_theme_icon("system"), "sb_theme_ic_system");
    // system -> dark -> light -> system (use-theme.ts:44-49), through the
    // footer's own action id and the theme table the host routes it to.
    let (action, index) = ("theme.cycle", 0);
    for (want, label, icon, stored) in [
        ("dark", "Dark", "sb_theme_ic_dark", Some("dark")),
        ("light", "Light", "sb_theme_ic_light", Some("light")),
        ("system", "System", "sb_theme_ic_system", None),
    ] {
        assert!(theme::is_action(action));
        match theme::resolve(action, index, &ctx()) {
            theme::Effect::Cycle { preference, .. } => assert_eq!(preference, want),
            other => panic!("{other:?}"),
        }
        // The host saves at once (use-theme.ts:35-42): only light / dark are
        // stored, system removes the key.
        theme::save_preference();
        let on_disk = std::fs::read_to_string(&pref).ok().and_then(|t| {
            serde_json::from_str::<serde_json::Value>(&t).ok().and_then(|v| v["theme"].as_str().map(str::to_owned))
        });
        assert_eq!(on_disk.as_deref(), stored, "after {want}");
        // The footer row re-labels (ProductSidebar.tsx:996-1003) and swaps its
        // icon (ThemeIcon: moon / sun / monitor).
        assert_eq!(chrome::theme_label(&theme::preference()), label);
        assert_eq!(chrome::footer_theme_icon(&theme::preference()), icon);
        // And the next launch adopts the stored choice.
        assert_eq!(theme::load_preference().map(|t| t.label()), stored.map(|_| label));
    }
    theme::reset_state();
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_shell_template_carries_the_footer_after_fleet_with_its_three_icons() {
    let chrome_src = include_str!("../src/chrome.rs");
    let lib_src = include_str!("../src/lib.rs");
    // lib.rs: + Add workspace, Fleet, then the footer nav (theme, Settings),
    // all inside the sidebar column.
    let foot = lib_src.find("oc_sidebar_foot := mod.widgets.OcSidebarFoot").expect("+ Add workspace");
    let fleet = lib_src.find("fleet_nav := View").expect("Fleet");
    let nav = lib_src.find("oc_sidebar_footnav := mod.widgets.OcSidebarFootNav").expect("the footer nav");
    let rule = lib_src.find("sidebar_rule := SolidView").expect("the column's end");
    assert!(foot < fleet && fleet < nav && nav < rule, "the web's order inside the column");
    // chrome.rs: the theme row's three icons and label, then Settings.
    let tpl = chrome_src.find("mod.widgets.OcSidebarFootNav = View").expect("the template");
    let body = &chrome_src[tpl..tpl + 3200];
    let t = body.find("sb_theme := View").unwrap();
    let s = body.find("sb_settings := View").unwrap();
    assert!(t < s, "the theme toggle, then Settings");
    for needle in [
        "sb_theme_ic_system", "icon(\"monitor\")", "sb_theme_ic_light", "icon(\"sun\")", "sb_theme_ic_dark",
        "icon(\"moon\")", "sb_theme_label", "sb_theme_hit := OcHit", "icon(\"gear\")", "sb_settings_label",
        "sb_settings_hit := OcHit",
    ] {
        assert!(body.contains(needle), "{needle}");
    }
    // The icons ship for both inks.
    for name in ["monitor", "sun", "moon"] {
        for v in ["", "-dark"] {
            let path = format!("{}/resources/icons/oc_{name}{v}.svg", env!("CARGO_MANIFEST_DIR"));
            assert!(std::path::Path::new(&path).is_file(), "{path}");
        }
    }
}
