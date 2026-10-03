//! A35b — the folder browser is not "super slow" (the operator, 2026-10-03,
//! measured with the Makepad instrument on the standalone app).
//!
//! What the instrument found (`tools/walk/a35_browser_latency_walk.py`,
//! `OCTOSCODE_PERF=1` lines, `[ui-hang]` samples):
//! 1. every board-1 remount (a folder listed, a row picked) invalidated the
//!    widget tree's path cache all the way up to the module root, so the
//!    ~270 root lookups the next event makes (the chrome sync, the docks)
//!    each walked the whole tree again: 230k nodes, 28 ms in a release build
//!    and ~300 ms in a debug one, per Signal;
//! 2. a change that landed while the view was composing was lost (the
//!    `board1::tests` race test), and a remount on a Signal asked for no
//!    frame (the walk's absolute bars);
//! 3. + Add workspace listed the server's folder twice (`fd1t_transport`).
//!
//! This file pins (1) on the module's REAL root view (its own DSL, the
//! board-1 dock's search barrier, the production `MountCache::mount`), and
//! that typing in the path box neither recomposes nor remounts the dock.
use makepad_widgets::*;
use octoscode_module::mount::MountCache;
use octoscode_module::perf;
use octoscode_module::screens::{board1, browser};

/// The tests share the board-1 host and the perf counters (process
/// globals): one at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

/// No test writes the operator's home: the design tree is the checkout's
/// own (nothing is materialized) and the recents live in a temp dir.
fn isolate() {
    static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        let design = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design");
        std::env::set_var("OCTOSCODE_DESIGN_DIR", design);
        let state = std::env::temp_dir().join(format!("a35b-browser-perf-{}", std::process::id()));
        std::fs::create_dir_all(state.join("recents")).unwrap();
        std::env::set_var("OCTOSCODE_RECENTS_DIR", state.join("recents"));
    });
}

/// A browser standing in a 46-folder working directory (the walk's tree).
fn listed(path: &str, n: usize) -> browser::BrowserUi {
    browser::BrowserUi {
        path: path.to_owned(),
        path_draft: path.to_owned(),
        entries: (0..n)
            .map(|i| browser::Entry { name: format!("project-{i:02}"), path: format!("{path}/project-{i:02}") })
            .collect(),
        parent: Some("/home/user".to_owned()),
        writable: true,
        hidden_skipped: 5,
        ..Default::default()
    }
}

/// The lookups the chrome sync makes from the module root on every event
/// (`chrome::ChromeRuntime::sync`): the header, the sidebar, Settings.
const ROOT_LOOKUPS: &[&[LiveId]] = &[
    ids!(hd_title),
    ids!(hd_path),
    ids!(sidebar_dock),
    ids!(sb_sort_label),
    ids!(set_title),
    ids!(set_server_status),
    ids!(stop_dock),
    ids!(composer_splash),
];

#[test]
fn a_folder_listing_remount_keeps_the_module_roots_lookups_cached() {
    let _s = serial();
    isolate();
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.with_vm(|vm| {
        makepad_widgets::script_mod(vm);
        octoscode_module::register_widgets(vm);
    });
    let root = cx.with_vm(octoscode_module::create_view);
    // Warm the root's lookups, as the first sync after launch does.
    for path in ROOT_LOOKUPS {
        assert!(!root.widget(&cx, path).is_empty(), "{path:?} is in the module's DSL");
    }
    let warm = cx.widget_tree().stats();
    for path in ROOT_LOOKUPS {
        let _ = root.widget(&cx, path);
    }
    assert_eq!(cx.widget_tree().stats().cache_misses, warm.cache_misses, "a warm cache answers every lookup");

    // Board 1's dock, found from the root (the dock itself, never inside it).
    let dock = root.widget(&cx, ids!(board1_dock));
    assert!(!dock.is_empty(), "the dock is found from the module root");
    let splash = dock.splash(&cx, ids!(board1_splash));
    assert!(splash.borrow().is_some(), "its splash is found from the dock");

    // The production path: the browser's composed view (46 folders), mounted
    // by the host's MountCache, then a pick re-mounts it.
    board1::close_all();
    browser::set(listed("/home/user/work", 46));
    board1::note_context(&board1::Context { capabilities: vec![browser::BROWSE_FEATURE.into()], ..Default::default() });
    let _ = board1::route("picker.browse", None);
    browser::set(listed("/home/user/work", 46));
    let mut mounts = MountCache::default();
    let first = board1::view(990.0, 603.0).expect("the browser is open");
    assert_eq!(mounts.mount(&mut cx, &splash, &first), Ok(true));
    let row = dock.widget(&cx, ids!(b1_br_row_t3));
    assert!(!row.is_empty(), "the listing's rows are found from the dock");
    let _ = board1::route("browser.enter.3", None);
    let picked = board1::view(990.0, 603.0).expect("still open");
    assert_ne!(picked, first, "a pick changes the view");
    assert_eq!(mounts.mount(&mut cx, &splash, &picked), Ok(true));

    let before = cx.widget_tree().stats();
    for path in ROOT_LOOKUPS {
        let _ = root.widget(&cx, path);
    }
    let after = cx.widget_tree().stats();
    assert_eq!(
        (after.cache_misses - before.cache_misses, after.walk_nodes - before.walk_nodes),
        (0, 0),
        "a folder-listing remount must not send the module root's lookups back to whole-tree walks \
         (board1_dock is a widget-tree search barrier)"
    );
    board1::close_all();
}

#[test]
fn routing_a_click_on_a_fresh_listing_walks_no_tree() {
    let _s = serial();
    isolate();
    let mut cx = Cx::new(Box::new(|_, _| {}));
    cx.with_vm(|vm| {
        makepad_widgets::script_mod(vm);
        octoscode_module::register_widgets(vm);
    });
    let root = cx.with_vm(octoscode_module::create_view);
    let dock = root.widget(&cx, ids!(board1_dock));
    let splash = dock.splash(&cx, ids!(board1_splash));
    board1::close_all();
    browser::set(listed("/home/user/work", 46));
    board1::note_context(&board1::Context { capabilities: vec![browser::BROWSE_FEATURE.into()], ..Default::default() });
    let _ = board1::route("picker.browse", None);
    browser::set(listed("/home/user/work", 46));
    let mut mounts = MountCache::default();
    let dsl = board1::view(990.0, 603.0).expect("open");
    assert_eq!(mounts.mount(&mut cx, &splash, &dsl), Ok(true));

    // The host indexes the new controls once (lib.rs sync_board1)...
    let s0 = cx.widget_tree().stats();
    let controls = board1::index_controls(&dock);
    let s1 = cx.widget_tree().stats();
    assert_eq!(s1.lookups, s0.lookups, "indexing walks the children, no path lookup");
    let ui = board1::controls();
    for (id, action) in ui.buttons.iter().chain(&ui.inputs).chain(&ui.returns) {
        let w = controls.get(&LiveId::from_str(id)).unwrap_or_else(|| panic!("{id} ({action}) is indexed"));
        assert_eq!(w.widget_uid(), dock.widget(&cx, &[LiveId::from_str(id)]).widget_uid(), "{id} is the mounted one");
    }
    assert!(ui.buttons.len() > 40, "every row is a control: {}", ui.buttons.len());

    // ...so routing an Actions event on a freshly mounted listing walks no
    // tree: the only lookups are the always-mounted entries, from the root.
    let _ = board1::route("browser.enter.3", None);
    let picked = board1::view(990.0, 603.0).expect("open");
    assert_eq!(mounts.mount(&mut cx, &splash, &picked), Ok(true));
    let controls = board1::index_controls(&dock);
    let root_view = octoscode_module::mount::eval_component(&mut cx, MAIN_SPLASH_VM_ID, "x := View{width: 1 height: 1}")
        .expect("a root view");
    let before = cx.widget_tree().stats();
    let events = board1::collect(&mut cx, &root_view, &controls, &[]);
    let after = cx.widget_tree().stats();
    assert!(events.is_empty());
    assert!(
        after.walk_nodes - before.walk_nodes < 20,
        "routing must not walk the dock per control: {} nodes",
        after.walk_nodes - before.walk_nodes
    );
    // What it replaced: one path lookup per control right after a remount
    // walks the whole dock subtree each time (quadratic in the rows).
    let _ = board1::route("browser.enter.4", None);
    let again = board1::view(990.0, 603.0).expect("open");
    assert_eq!(mounts.mount(&mut cx, &splash, &again), Ok(true));
    let before = cx.widget_tree().stats();
    for (id, _) in &ui.buttons {
        let _ = dock.button(&cx, &[LiveId::from_str(id)]);
    }
    let walked = cx.widget_tree().stats().walk_nodes - before.walk_nodes;
    assert!(walked > 5_000, "the per-control lookups it replaced walked {walked} nodes");
    board1::close_all();
}

#[test]
fn typing_in_the_path_box_neither_recomposes_nor_remounts_and_a_navigation_composes_once() {
    let _s = serial();
    isolate();
    board1::close_all();
    browser::set(listed("/home/user/work", 46));
    board1::note_context(&board1::Context { capabilities: vec![browser::BROWSE_FEATURE.into()], ..Default::default() });
    let _ = board1::route("picker.browse", None);
    browser::set(listed("/home/user/work", 46));
    let mounted = board1::view(990.0, 603.0).expect("open");
    // What lib.rs does per key: the input's live text through `route`, then
    // the next sync's `view` (the Actions pass ends in sync_labels).
    let (composes, _) = perf::board1_counts();
    let mut typed = String::from("/home/user/work");
    for ch in "/monorepo".chars() {
        typed.push(ch);
        assert!(board1::route("browser.path", Some(&typed)).is_empty(), "a key sends no request");
        assert_eq!(board1::view(990.0, 603.0).as_deref(), Some(mounted.as_str()), "the mounted DSL is unchanged");
    }
    assert_eq!(perf::board1_counts().0, composes, "no compose per keystroke");
    assert_eq!(browser::state().path_draft, "/home/user/work/monorepo");

    // Return: one request, one compose for the loading state; the listing's
    // arrival is one more — never one per sync in between.
    let work = board1::route("browser.go", None);
    assert!(matches!(work.as_slice(), [board1::Work::BrowserList { path: Some(p), .. }] if p == "/home/user/work/monorepo"), "{work:?}");
    let _ = board1::view(990.0, 603.0);
    let _ = board1::view(990.0, 603.0);
    assert_eq!(perf::board1_counts().0, composes + 1, "one compose for the navigation");
    browser::set(listed("/home/user/work/monorepo", 5));
    board1::mark_dirty();
    let _ = board1::view(990.0, 603.0);
    let _ = board1::view(990.0, 603.0);
    assert_eq!(perf::board1_counts().0, composes + 2, "one compose for the listing that landed");
    board1::close_all();
}
