//! #21 — a remount while a turn streams must not send the module's per-event
//! lookups back to whole-tree walks.
//!
//! What `OCTOSCODE_PERF=1` measured on the app: 2–3 `Signal`s per turn took
//! 32–38 ms, every one `lookups=+297 misses=+239 walked=+226333 inval=+28`.
//! A turn's start and end remount the session strip (and a keystroke can
//! remount the composer's seats); a remount drops the path cache of every
//! ancestor of what it replaced, the module root's included, and the
//! chrome sync's ~200 root lookups then each walked the whole tree again.
//!
//! This file pins the two halves of the fix on the module's REAL root view
//! (its own DSL, the production `MountCache::mount` and `ChromeRuntime::sync`):
//! the chrome sync roots its lookups at the containers that hold them, which
//! no remount climbs through, and the strip's row is a search barrier.
use makepad_widgets::*;
use octoscode_module::chrome::ChromeRuntime;
use octoscode_module::mount::MountCache;
use octoscode_module::OctoscodeView;
use octoscode_store::Store;

/// The tests share the widget-tree counters' process: one at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// No test reads or writes the operator's home: the design tree is the
/// checkout's own and every preference file lives in a temp dir.
fn isolate() {
    static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        let design = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design");
        std::env::set_var("OCTOSCODE_DESIGN_DIR", design);
        let state = std::env::temp_dir().join(format!("chrome-lookup-perf-{}", std::process::id()));
        std::fs::create_dir_all(state.join("recents")).unwrap();
        std::env::set_var("OCTOSCODE_RECENTS_DIR", state.join("recents"));
        std::env::set_var("OCTOSCODE_PREF_PATH", state.join("display.json"));
        std::env::set_var("OCTOSCODE_DISPLAY_PREFS_PATH", state.join("display-v1.json"));
        std::env::set_var("OCTOSCODE_NOTIFICATIONS_FILE", state.join("notifications.json"));
    });
}

fn module(cx: &mut Cx) -> WidgetRef {
    cx.with_vm(|vm| {
        makepad_widgets::script_mod(vm);
        octoscode_module::register_widgets(vm);
    });
    cx.with_vm(octoscode_module::create_view)
}

#[test]
fn a_composer_remount_leaves_the_chrome_sync_on_cached_lookups() {
    let _s = serial();
    isolate();
    let mut cx = Cx::new(Box::new(|_, _| {}));
    let root = module(&mut cx);
    let store = Store::new();
    let mut chrome = ChromeRuntime::default();
    let module = root.borrow::<OctoscodeView>().expect("the module's root view");
    // The first sync after launch warms every lookup (and applies the layout).
    chrome.sync(&mut cx, &module, &store, true, false, 1280.0, 800.0);
    chrome.sync(&mut cx, &module, &store, true, false, 1280.0, 800.0);

    // A remount of a splash the root's own map depends on, by the production
    // path: the composer's (a turn's start remounts the strip the same way).
    let splash = module.splash(&cx, ids!(composer_splash));
    let mut mounts = MountCache::default();
    assert_eq!(mounts.mount(&mut cx, &splash, "c21_a := View{height: 4}"), Ok(true));
    let before = cx.widget_tree().stats();
    assert_eq!(mounts.mount(&mut cx, &splash, "c21_b := View{height: 6}"), Ok(true));
    assert!(cx.widget_tree().stats().invalidations > before.invalidations, "the remount invalidated the root's map");

    let before = cx.widget_tree().stats();
    chrome.sync(&mut cx, &module, &store, true, false, 1280.0, 800.0);
    let after = cx.widget_tree().stats();
    let (lookups, misses) = (after.lookups - before.lookups, after.cache_misses - before.cache_misses);
    assert!(lookups >= 150, "the chrome sync makes its ~200 lookups ({lookups})");
    assert!(
        misses <= 10,
        "after a remount only the chrome's few root lookups (its five containers, three root \
         overlays) may walk the tree, not all {lookups}: {misses} missed"
    );
}

#[test]
fn a_strip_remount_keeps_the_module_roots_lookups_cached() {
    let _s = serial();
    isolate();
    let mut cx = Cx::new(Box::new(|_, _| {}));
    let root = module(&mut cx);
    // What the root looks up on every event, the strip's row among them.
    let root_lookups: &[&[LiveId]] = &[
        ids!(hd_title),
        ids!(sidebar_dock),
        ids!(set_title),
        ids!(composer_splash),
        ids!(takeover_row),
        ids!(strip_row),
    ];
    for path in root_lookups {
        assert!(!root.widget(&cx, path).is_empty(), "{path:?} is in the module's DSL");
    }

    // The strip is found FROM its row (`strip_row_ref`), never from the root.
    let row = root.widget(&cx, ids!(strip_row));
    let splash = row.splash(&cx, ids!(strip_splash));
    assert!(splash.borrow().is_some(), "the strip's splash is found from its row");
    assert!(root.widget(&cx, ids!(strip_splash)).is_empty(), "the row is a widget-tree search barrier");

    let mut mounts = MountCache::default();
    assert_eq!(mounts.mount(&mut cx, &splash, "b3_strip_tap := View{height: 4}"), Ok(true));
    // Warm the root's lookups, as the first sync after launch does.
    for path in root_lookups {
        let _ = root.widget(&cx, path);
    }
    // A turn starts: the strip's state word changes and the strip remounts.
    assert_eq!(mounts.mount(&mut cx, &splash, "b3_strip_tap := View{height: 6}"), Ok(true));
    assert!(!row.widget(&cx, ids!(b3_strip_tap)).is_empty(), "the remounted tap is found from the row");

    let before = cx.widget_tree().stats();
    for path in root_lookups {
        let _ = root.widget(&cx, path);
    }
    let after = cx.widget_tree().stats();
    assert_eq!(
        (after.cache_misses - before.cache_misses, after.walk_nodes - before.walk_nodes),
        (0, 0),
        "a strip remount must not send the module root's lookups back to whole-tree walks \
         (strip_row is a widget-tree search barrier)"
    );
}
