//! #32g — no lowered DSL may still name `self:resources/…`.
//!
//! A `crate_resource("self:resources/…")` reference resolves against the
//! ScriptMod's BUILD-machine manifest path (makepad script/res.rs:1067
//! concatenates `cargo_manifest_path`), which does not exist on the phone —
//! the device log proved it (`font member latin (<build-machine>/…)` ×11).
//! Every lowering path must come out of `design::with_fonts` referencing
//! the materialized root's absolute files (or keep the original form ONLY
//! when the file is missing — the mono fallback).

use octoscode_module::design;
use octoscode_module::screens::connect::{self, ConnectUi, Screen};

#[test]
fn no_lowered_dsl_names_self_resources() {
    // The screens (the first-run Connect card is what the 6T renders).
    for (screen, ui) in [
        (Screen::Connect, ConnectUi::default()),
        (Screen::ConnectFailed, ConnectUi::default()),
        (Screen::Onboarding, ConnectUi::default()),
    ] {
        let dsl = connect::lower_screen(screen, &ui).expect("lower");
        assert!(
            !dsl.contains("self:resources/"),
            "{screen:?} still names self:resources/"
        );
    }
    // The components (#21's thread-row/user-bubble/composer … — mounted
    // FIRST at launch, which is how the Inter family got registered with
    // the dead path before Connect ever lowered).
    for kind in octoscode_module::components::ItemKind::ALL {
        let dsl = octoscode_module::components::lower(*kind, "t00", &[])
            .or_else(|_| octoscode_module::components::lower(*kind, "t01", &[]))
            .expect("lower");
        assert!(
            !dsl.contains("self:resources/"),
            "component {} still names self:resources/",
            kind.id()
        );
    }
}

#[test]
fn icon_resources_resolve_to_existing_absolute_files() {
    for name in ["chevron_down.svg", "icon_ring.svg", "icon_menu.svg", "icon_close.svg"] {
        let p = design::icon_resource(name);
        assert!(p.starts_with('/'), "{name}: {p}");
        assert!(std::path::Path::new(&p).is_file(), "{name}: {p}");
    }
}

#[test]
fn svg_properties_never_carry_a_bare_string_splice() {
    // The 42d4cad regression (device: lib.rs:122:38 "type mismatch for
    // property svg: expected object, got string" — one bad property aborts
    // the whole module script_mod, the module rendered EMPTY): a splice of a
    // plain STRING into `draw_svg.svg` is a type error the compiler cannot
    // see (the property is evaluated at script time). The value must be the
    // file_resource OBJECT — `file_resource(#(…))` — or a plain
    // crate_resource literal. A #[test] cannot construct ScriptVm outside an
    // app (no public constructor; keys_probe registers the module through
    // AppMain), so this source-level guard pins the shape in CI and
    // examples/keys_probe.rs (headless) is the executable mount check.
    let src = include_str!("../src/lib.rs");
    for (i, line) in src.lines().enumerate() {
        if line.contains("draw_svg.svg:") && line.contains("#(") {
            assert!(
                line.contains("file_resource(#("),
                "lib.rs:{}: a bare string splice on draw_svg.svg (needs the file_resource object): {line}",
                i + 1
            );
        }
        if line.contains("draw_svg.svg:") {
            assert!(
                !line.contains("self:resources/"),
                "lib.rs:{}: a self:resources reference (build-machine path): {line}",
                i + 1
            );
        }
    }
}
