//! Card #21b — mount a lowered #16 component the way `beauty-host` does.
//!
//! ## Why not `Splash::set_text`
//!
//! `set_text` evaluates the body in the Splash's **own nested isolate** and
//! assigns the freshly-minted `View` to `Splash::view`. The widgets that view
//! contains are minted as a **standalone tree**: nothing calls
//! `widget_tree_insert_child_deep`, so they never enter the shared widget-tree
//! graph the draw pass walks. They instantiate with a zero rect and paint
//! nothing — the exact symptom card #21b measured (`/snap` showed the real DSL
//! and real authored sizes, but every `i*_*` node seated at `[0,0,0,0]`).
//!
//! `beauty-host` (the reference that DOES paint this DSL,
//! `apps/kit-host/src/beauty.rs:125-195`) instead:
//!
//! 1. evaluates `return View{… <ui>}` with `vm.eval_checked(..)` →
//!    `View::script_from_value(vm, value)`;
//! 2. `std::mem::replace(&mut splash.view, view)` — the component tree becomes
//!    the Splash's own view;
//! 3. for each child `cx.widget_tree_insert_child_deep(splash_uid, id, child)`
//!    (this is what "reparents the nodes created under the temporary standalone
//!    View during script evaluation"), then `widget_tree_mark_dirty`.
//!
//! ## Which VM
//!
//! Our module is not a plain `app_main!` app: the OctoSense shell hosts it in an
//! **isolate** (`module_host.rs:133` `alloc_splash_vm_with_network(false)`, then
//! `module.register(vm)` / `module.create(vm, ..)` inside it). So "the main VM"
//! for us is *the VM that owns our widgets* — the isolate `OctoscodeView` and
//! its `Splash` children were minted in. Evaluating the component in any other
//! VM would mint a `View` whose script refs point at another heap, which
//! `View::script_call` refuses (view.rs:900, "a view whose isolate has since
//! been torn down"). We therefore recover the owning VM from the Splash's own
//! view source (`cx.script_ref_vm_id`) rather than assuming the installed VM.
//!
//! The design/kit vocabulary the DSL names must be registered in THAT VM.
//! `OctoscodeModule::register` registers `octoscript_widgets::{design,kit}::
//! script_mod` directly into it, as `beauty.rs:356-364` does in the App's
//! `script_mod`.
use std::collections::HashMap;

use makepad_widgets::*;
use makepad_widgets::makepad_script::ScriptMod;

/// The DSL prelude: name the design/kit vocabulary, then return the component's
/// own root inside a wrapper `View`.
///
/// Card #21c item 3: the wrapper is `flow: Down height: Fit`, not `Overlay`.
/// `Overlay` gives a `Fit`-height slot no measurable content height, so a
/// component whose root is a plain `View`/`DesignSurface` (the user-bubble,
/// measured `284x85`) collapsed to `[0,0,0,0]` and painted nothing, while a
/// `Markdown` root (intrinsic height) seated. A stacking flow measures its one
/// child, so every slot's height comes from its component. The host still owns
/// the box: `MountCache::mount` copies the Splash's declared walk onto the
/// evaluated view, so a `Splash{height:190}` (the composer dock) still gets 190.
const PRELUDE: &str = "use mod.prelude.widgets.*\nreturn View{width:Fill height:Fit flow:Down ";

/// One mounted slot: the DSL it was mounted from, and the view it displaced.
///
/// The displaced (`retired`) view is kept for one more mount so its GPU draw
/// lists stay alive through the frame that swapped them out — the same reason
/// `beauty-host` keeps `retired_view` (`beauty.rs:38,223`).
#[derive(Default)]
struct Slot {
    dsl: String,
    retired: Option<View>,
}

/// The per-widget mount state, keyed by the Splash's widget uid.
///
/// The card asks to cache the evaluated View per (item id, values hash) and not
/// re-eval every frame. The DSL string already IS that hash
/// (`screen::Cache` builds it from `(kind, index, values-json)`), so comparing
/// it is both the cache key and the change test.
#[derive(Default)]
pub struct MountCache {
    slots: HashMap<u64, Slot>,
}

impl MountCache {
    /// Mount `ui` into `splash` when its DSL differs from the last mount into
    /// that widget.
    ///
    /// Returns `Ok(true)` when a mount happened, `Ok(false)` when the widget was
    /// already mounted from the same DSL (the steady-state case a `PortalList`
    /// hits every frame).
    /// The DSL a Splash was last mounted with (a re-mount's diagnostics).
    pub fn mounted_dsl(&self, splash: &SplashRef) -> Option<String> {
        let uid = splash.borrow()?.widget_uid().0;
        self.slots.get(&uid).map(|s| s.dsl.clone())
    }

    pub fn mount(&mut self, cx: &mut Cx, splash: &SplashRef, ui: &str) -> Result<bool, String> {
        let (uid, source) = {
            let inner = splash.borrow().ok_or("mount: the Splash is not live")?;
            (inner.widget_uid().0, inner.view.source.clone())
        };
        if uid == 0 {
            return Err("mount: the Splash is not live".into());
        }
        if self.slots.get(&uid).is_some_and(|s| s.dsl == ui) {
            return Ok(false);
        }
        // The widgets the DSL mints must live in the heap that owns this Splash,
        // or their script refs are garbage in the wrong VM.
        let vm_id = cx
            .script_ref_vm_id(&source)
            .unwrap_or(MAIN_SPLASH_VM_ID);
        let mut view = eval_component(cx, vm_id, ui)?;

        let mut inner = splash.borrow_mut().ok_or("mount: not a Splash")?;
        // The HOST owns the Splash's slot, and `Splash::walk()` delegates to
        // `self.view.walk` — so the declared `Splash{height:56}` lives on the OLD
        // view. A `mem::replace` that drops it leaves the Splash on the DSL's own
        // `height:Fit`, which measures 0 for a component whose children are all
        // absolutely positioned (`abs_pos`), and nothing seats. Preserve it, the
        // same line `Splash::eval_styled_body_with_apply` runs
        // (splash.rs:352-355) so rebuilding a body never takes the slot away.
        view.walk = inner.view.walk;
        // A35b: where the retired view was drawn, read BEFORE the swap. The
        // new view has never been drawn: its area is empty, so redrawing it
        // (`inner.redraw` below, `View::redraw` -> `Area::redraw`) names no
        // draw list and asks for no frame. A mount that rides a click's own
        // Actions pass was drawn anyway (that pass redraws the whole module),
        // but one that lands on a Signal — the folder browser's listing after
        // the server answered — sat in the widget tree undrawn until some
        // unrelated input redrew the window (measured on the standalone app:
        // the rows in /snap?all=1, "Loading folders…" on screen).
        let drawn_at = inner.view.area();
        let old = std::mem::replace(&mut inner.view, view);
        // Reparent the new tree's nodes into the shared widget-tree graph. Do
        // this while the view is still owned by the Splash so its children are
        // reachable without a second lookup.
        let mut children: Vec<(LiveId, WidgetRef)> = Vec::new();
        inner.children(&mut |id, child| {
            if !child.is_empty() {
                children.push((id, child));
            }
        });
        inner.redraw(cx);
        drop(inner);
        // Ask for the frame through the draw list the retired view was drawn
        // in; a slot that was never drawn (no list to name) redraws the
        // window, so a mount can never wait for unrelated input. Inside a
        // draw pass (a PortalList row mounted in `draw_walk`) the caller
        // draws the new view in this very pass: nothing to ask for.
        if !cx.in_draw_event() {
            if drawn_at.draw_list_id().is_some() {
                cx.redraw_area(drawn_at);
            } else {
                cx.redraw_all();
            }
        }

        for (id, child) in children {
            cx.widget_tree_insert_child_deep(WidgetUid(uid), id, child);
        }
        cx.widget_tree_mark_dirty(WidgetUid(uid));

        let slot = self.slots.entry(uid).or_default();
        slot.dsl = ui.to_owned();
        slot.retired = Some(old);
        Ok(true)
    }

    /// Diagnose one mount: the widget uid, owning VM, the lowered children and
    /// their drawn areas — the evidence that separates "mounted but not walked"
    /// from "never mounted".
    pub fn diagnose(&self, cx: &mut Cx, splash: &SplashRef) -> String {
        let inner = match splash.borrow() {
            Some(i) => i,
            None => return "not live".into(),
        };
        let uid = inner.widget_uid().0;
        let srect = inner.area().rect(cx);
        let wrect = inner.view.area().rect(cx);
        let mut out = format!(
            "uid={uid} vm={:?} splash=({:.0},{:.0},{:.0},{:.0}) wrapper=({:.0},{:.0},{:.0},{:.0}) kids=",
            cx.script_ref_vm_id(&inner.view.source),
            srect.pos.x, srect.pos.y, srect.size.x, srect.size.y,
            wrect.pos.x, wrect.pos.y, wrect.size.x, wrect.size.y,
        );
        let mut n = 0;
        inner.children(&mut |id, child| {
            n += 1;
            let r = child.area().rect(cx);
            out.push_str(&format!(
                " [{:?} {} rect=({:.0},{:.0},{:.0},{:.0})]",
                id,
                child
                    .widget_type_id()
                    .map(|t| format!("{t:?}"))
                    .unwrap_or_default(),
                r.pos.x,
                r.pos.y,
                r.size.x,
                r.size.y
            ));
        });
        out.push_str(&format!(" count={n} dsl={}", self.slots.get(&uid).map(|s| s.dsl.len()).unwrap_or(0)));
        out
    }

    /// Forget a widget's slot: a `PortalList` pool hands a recycled uid to a
    /// different row, and the next mount must re-evaluate rather than compare
    /// against the previous row's DSL.
    pub fn forget(&mut self, uid: u64) {
        self.slots.remove(&uid);
    }

    /// How many mounts are memoised (a test reads this).
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

/// Evaluate `<PRELUDE><ui>}` in `vm_id` and return the resulting `View`.
pub fn eval_component(cx: &mut Cx, vm_id: SplashVmId, ui: &str) -> Result<View, String> {
    let code = format!("{PRELUDE}{ui}}}");
    let sm = ScriptMod {
        cargo_manifest_path: crate::design::manifest_dir().into(),
        module_path: module_path!().into(),
        file: file!().into(),
        line: 1,
        column: 0,
        code,
        values: Vec::new(),
    };
    // `_trusted` skips the per-entry app-script byte budget: this is host-driven
    // rendering of a lowered component, exactly like `module.register` /
    // `module.create` in `module_host.rs:150-152`.
    cx.with_script_vm_id_trusted(vm_id, |vm| {
        let value = vm
            .eval_checked(sm, 2_000_000)
            .ok_or_else(|| "the component DSL did not evaluate".to_string())?;
        Ok::<_, String>(View::script_from_value(vm, value))
    })
}

#[cfg(test)]
mod tests {
    //! The app-path evaluation: the same `eval_component` the screen calls,
    //! against the real lowered #16 component. The pixel proof is the headless
    //! capture (card #21b §4); this pins the eval + child mounting contract.
    use super::*;

    fn cx_with_vocabulary() -> Cx {
        let mut cx = Cx::new(Box::new(|_, _| {}));
        cx.with_vm(makepad_widgets::script_mod);
        cx.with_vm(octoscript_widgets::design::script_mod);
        cx.with_vm(octoscript_widgets::kit::script_mod);
        cx
    }

    #[test]
    fn the_app_vm_evaluates_a_lowered_component_into_a_root_with_children() {
        let ui = crate::components::lower(
            crate::components::ItemKind::UserBubble,
            "0",
            &[
                ("t01_text".to_owned(), "Fix the steer queue".to_owned()),
                ("t02_text".to_owned(), String::new()),
            ],
        )
        .expect("user-bubble lowers");
        let mut cx = cx_with_vocabulary();
        let view = eval_component(&mut cx, MAIN_SPLASH_VM_ID, &ui)
            .expect("the app VM evaluates the component DSL");
        // The lowered root is `i0_userbubble := DesignSurface { … }`; the wrapper
        // View the prelude returns must carry it as a child.
        let mut children = Vec::new();
        view.children(&mut |id, child| {
            if !child.is_empty() {
                children.push(id);
            }
        });
        assert!(
            !children.is_empty(),
            "the evaluated component View must own its lowered root as a child"
        );
    }

    /// A1: every fluid builder's DSL evaluates in the app VM, at the desktop
    /// and the phone density. A builder that names a property its widget does
    /// not have (an `Svg` with `visible:`) failed the whole Connect card's
    /// eval at runtime; this catches that class before a launch.
    #[test]
    fn every_fluid_builder_evaluates_in_the_app_vm() {
        use crate::conv_layout::Metrics;
        use crate::fluid::*;
        let mut cx = cx_with_vocabulary();
        // The controls: the same Svg evaluates, and with `visible:` (the bug
        // this test exists for) it does not.
        let svg = |extra: &str| {
            format!(
                "x := Svg{{width: 12 height: 12 {extra}draw_svg.svg: file_resource({:?})}}\n",
                icon("chevron_right.svg").display().to_string()
            )
        };
        assert!(eval_component(&mut cx, MAIN_SPLASH_VM_ID, &svg("")).is_ok(), "the control Svg evaluates");
        assert!(
            eval_component(&mut cx, MAIN_SPLASH_VM_ID, &svg("visible: false ")).is_err(),
            "an Svg cannot take `visible`"
        );
        for m in [Metrics::for_window(990.0, true), Metrics::for_window(360.0, false)] {
            let tool = ToolView {
                title: "read_file".into(),
                target: "README.md".into(),
                state: "done".into(),
                secs: Some(2),
                output: "line".into(),
            };
            let builders = [
                ("composer", composer(
                    &ComposerView {
                        placeholder: "Ask Octos anything".into(),
                        approval: "Ask for approval".into(),
                        model: "v4-flash".into(),
                    },
                    &m,
                )),
                ("connect", connect_card(&ConnectView { server: "http://127.0.0.1:50190".into(), ..Default::default() }, &m, 261.0)),
                ("bubble", user_bubble("0", "请用中文回答 hello", &m, false)),
                ("prose", assistant_prose("0", "**hi** `code`\n\n- a\n- b", &m)),
                ("tool", tool_row("0", &tool, GroupPos::Single, true, &m)),
                ("worked", worked_for("0", "Worked for 2s", 2, false, &m)),
                ("working", working_row("0", "Working…", &m)),
                ("actions", answer_actions("0", "now", &m)),
                ("empty", empty_state(Some("octos"), &m)),
            ];
            for (name, ui) in builders {
                assert!(
                    eval_component(&mut cx, MAIN_SPLASH_VM_ID, &ui).is_ok(),
                    "{name} at {:?} must evaluate",
                    m.density
                );
            }
        }
    }

    /// Parity row "Render settled GFM (headings, lists, tables, strong) and
    /// keep raw HTML inert" on the production chain: the store's settled
    /// answer -> the display order -> the row's copies -> the fluid lowering
    /// (the calls `lib.rs` draw_walk makes through `screen::Cache`) -> the
    /// app VM. The answer reaches ONE native `Markdown` region verbatim
    /// (makepad's parser renders headings / strong / tables / lists and
    /// drops raw HTML: markdown.rs `Options::ENABLE_TABLES`, `InlineHtml`
    /// keeps only sub/sup), and the lowered region evaluates.
    #[test]
    fn a_settled_gfm_answer_lowers_to_one_markdown_region_that_evaluates() {
        use std::sync::{Arc, Mutex};
        let store = Arc::new(octoscode_store::Store::new());
        store.set_sessions(vec![octoscode_store::Session {
            id: "s1".into(),
            title: Some("t".into()),
            message_count: 2,
            updated_at: None,
            last_prompt: None,
            active_turn: false,
        }]);
        store.set_active(Some("s1".into()));
        crate::components::seed_gfm_turn(&store, "s1", "t1");
        let ui = Arc::new(Mutex::new(crate::flow::FlowUi::default()));
        let rows = crate::screen::timeline_rows_folded(&store, false, &[]);
        let prose = rows
            .iter()
            .find(|r| r.kind == crate::components::ItemKind::AssistantProse)
            .expect("the settled answer row");
        let copies = {
            let ctx = crate::bindings::Ctx::new(&store, &ui);
            crate::components::item_copies(prose.kind, &ctx, prose.index, prose.turn.as_deref()).expect("copies")
        };
        let dsl = crate::components::lower(prose.kind, "0", &copies).expect("lowers");
        assert_eq!(dsl.matches("Markdown{").count(), 1, "one native Markdown region: {dsl}");
        let body = format!("body: {:?}", crate::components::GFM_SAMPLE);
        assert!(dsl.contains(&body), "the answer reaches the Markdown region verbatim: {dsl}");
        assert!(dsl.contains("heading_base_scale"), "headings get the prose scale");
        let mut cx = cx_with_vocabulary();
        assert!(eval_component(&mut cx, MAIN_SPLASH_VM_ID, &dsl).is_ok(), "the GFM answer region evaluates");
    }

    /// A7 — the production path for a model answer: store -> the transcript
    /// rows -> `item_copies` -> `lower` -> the app VM. A settled answer keeps
    /// only the absolute https link, never loads the remote image, typesets
    /// its math, highlights its fence (`A7CodeLines`) behind a Copy control
    /// that writes the trimmed code; the same text while streaming closes the
    /// open fence for display (plain code, math off) and the stored text is
    /// unchanged.
    #[test]
    fn a7_store_answers_lower_safely_and_evaluate() {
        use std::sync::{Arc, Mutex};
        const SETTLED: &str = "See [the notes](https://docs.example.com/steer) and [this](javascript:alert(1)); \
             ![depth](https://cdn.example.com/depth.png)\n\n\
             ```rust\nfn flush(q: &mut Vec<String>) -> usize {\n    q.len()\n}\n```\n\n\
             The cost is $O(n)$:\n\n$$\nT = \\sum_i t_i\n$$\n\nIt costs $12 and $5 more.";
        const STREAMING: &str = "Working on it:\n\n````ts\nconst x = 1;\n";
        let store = Arc::new(octoscode_store::Store::new());
        store.set_sessions(vec![octoscode_store::Session {
            id: "s1".into(),
            title: Some("t".into()),
            message_count: 4,
            updated_at: None,
            last_prompt: None,
            active_turn: true,
        }]);
        store.set_active(Some("s1".into()));
        let tl = &store.domains.session.timeline;
        tl.upsert_user_message("s1", "t1", "Explain the flush.", serde_json::json!({}));
        tl.append("s1", Some("t1".into()), octoscode_store::EntryKind::ASSISTANT_TEXT, SETTLED.to_owned());
        tl.finalize_assistant("s1", "t1", SETTLED);
        tl.close_turn("s1", "t1");
        store.domains.turn.started("t1");
        store.domains.turn.set_terminal("t1", "completed");
        tl.upsert_user_message("s1", "t2", "And the client?", serde_json::json!({}));
        tl.append("s1", Some("t2".into()), octoscode_store::EntryKind::ASSISTANT_TEXT, STREAMING.to_owned());
        store.domains.turn.started("t2");
        let ui = Arc::new(Mutex::new(crate::flow::FlowUi::default()));
        let rows = crate::screen::timeline_rows_folded(&store, true, &[]);
        let prose: Vec<_> = rows.iter().filter(|r| r.kind == crate::components::ItemKind::AssistantProse).collect();
        assert_eq!(prose.len(), 2, "one answer row per turn");
        let mut cx = cx_with_vocabulary();
        cx.with_vm(crate::code_view::script_mod);
        let lower = |r: &crate::screen::Row| {
            let ctx = crate::bindings::Ctx::new(&store, &ui);
            let copies =
                crate::components::item_copies(r.kind, &ctx, r.index, r.turn.as_deref()).expect("copies");
            let blocks = crate::components::prose_code_blocks(&ctx, r.index, r.turn.as_deref());
            (crate::components::lower(r.kind, "0", &copies).expect("lowers"), blocks)
        };
        // The settled answer.
        let (dsl, blocks) = lower(prose[0]);
        assert!(dsl.contains("https://docs.example.com/steer"), "the safe link stays: {dsl}");
        assert!(!dsl.contains("javascript:"), "an unsafe link is plain text");
        assert!(!dsl.contains("cdn.example.com"), "a remote image is never loaded (alt kept)");
        assert!(dsl.contains("A7CodeLines") && dsl.contains("code_copy_"), "highlighted fence + Copy");
        assert!(dsl.contains("use_math_widget: true") && dsl.contains("A7MathBlock"), "math typeset");
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].1, "fn flush(q: &mut Vec<String>) -> usize {\n    q.len()\n}", "Copy writes the trimmed code");
        assert!(eval_component(&mut cx, MAIN_SPLASH_VM_ID, &dsl).is_ok(), "the settled answer evaluates");
        // The same row while a reply streams: the open fence is closed for
        // display, the code is plain, the stored text is unchanged.
        let (dsl, blocks) = lower(prose[1]);
        assert!(!dsl.contains("A7CodeLines"), "no highlighting while streaming");
        assert!(dsl.contains("const x = 1;"), "the open fence's code shows");
        assert_eq!(blocks.len(), 1, "the closed-for-display fence is one block");
        assert_eq!(store.domains.session.timeline.assistant_text("s1").matches("````").count(), 1, "stored text unchanged");
        assert!(eval_component(&mut cx, MAIN_SPLASH_VM_ID, &dsl).is_ok(), "the streaming answer evaluates");
    }

    /// A13 (judge, 360 px: "Args::\nparse()") — a settled code block lays
    /// its lines out whole inside a `ScrollXView` (a known grammar coloured,
    /// an unknown fence plain), the streaming block keeps the wrapping flow,
    /// and both evaluate in the app VM at both densities.
    #[test]
    fn a13_settled_code_scrolls_sideways_and_evaluates() {
        use crate::conv_layout::Metrics;
        let mut cx = cx_with_vocabulary();
        cx.with_vm(crate::code_view::script_mod);
        let long = "let args = Args::parse(); // a line longer than any phone column, kept whole";
        for m in [Metrics::for_window(990.0, true), Metrics::for_window(360.0, false)] {
            for (fence, lang) in [("rust", "rust"), ("", ""), ("brainfuck", "")] {
                let md = format!("Run:\n\n```{fence}\n{long}\n```\n");
                let d = crate::markdown::display(&md, false);
                let dsl = crate::fluid::assistant_answer("0", &d, None, &m);
                let at = dsl.find("i0_code_1_scroll := ScrollXView{").unwrap_or_else(|| panic!("{fence:?}: {dsl}"));
                let lines = &dsl[at..];
                assert!(lines.contains("mod.widgets.A7CodeLines{width: Fit height: Fit\nwrap: false"), "{fence:?}");
                assert!(lines.contains(&format!("lang: {lang:?}")), "{fence:?}");
                assert!(lines.contains(&format!("{long:?}")), "the whole line, unbroken");
                assert!(eval_component(&mut cx, MAIN_SPLASH_VM_ID, &dsl).is_ok(), "{fence:?} evaluates: {dsl}");
            }
            let streaming = crate::markdown::display(&format!("Run:\n\n```rust\n{long}\n"), true);
            let dsl = crate::fluid::assistant_answer("0", &streaming, None, &m);
            assert!(!dsl.contains("ScrollXView") && !dsl.contains("A7CodeLines"), "the stream keeps the wrapping flow");
            assert!(eval_component(&mut cx, MAIN_SPLASH_VM_ID, &dsl).is_ok());
        }
    }

    /// A7: the answer's display variants (math typeset through MathView, code
    /// blocks with their banner + Copy hit, unsafe links/images stripped)
    /// evaluate in the app VM at both densities — a widget property the
    /// renderer lacks would fail the whole row at runtime.
    #[test]
    fn a7_answer_variants_evaluate_in_the_app_vm() {
        use crate::conv_layout::Metrics;
        let mut cx = cx_with_vocabulary();
        cx.with_vm(crate::code_view::script_mod);
        let samples = [
            ("math", "Energy $E = mc^2$ powers it.\n\n$$\na^2 + b^2 = c^2\n$$"),
            ("code", "Run it:\n\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n\nThen `cargo test`."),
            ("open fence", "Here:\n\n```ts\nconst x = 1;"),
            ("links", "[safe](https://example.com) [bad](javascript:alert(1)) ![chart](https://x.test/c.png)"),
        ];
        for m in [Metrics::for_window(990.0, true), Metrics::for_window(360.0, false)] {
            for (name, text) in samples {
                for streaming in [false, true] {
                    let d = crate::markdown::display(text, streaming);
                    let dsl = crate::fluid::assistant_answer("0", &d, Some(1), &m);
                    assert_eq!(dsl.matches('{').count(), dsl.matches('}').count(), "{name}: balanced");
                    assert!(
                        eval_component(&mut cx, MAIN_SPLASH_VM_ID, &dsl).is_ok(),
                        "{name} (streaming={streaming}) at {:?} must evaluate: {dsl}",
                        m.density
                    );
                }
            }
        }
    }

    /// A7 — the transcript's system-notice row (board-3 rows, the collision /
    /// not-sent / recovery notices land there) evaluates in the app VM; a
    /// failed row keeps the slot's previous content on screen.
    #[test]
    fn a7_notice_rows_evaluate_in_the_app_vm() {
        use octoscode_store::timeline::EntryKind;
        let store = octoscode_store::Store::new();
        store.set_active(Some("s".into()));
        store.domains.session.timeline.upsert_notice(
            "s",
            Some("t1".into()),
            "send-busy:t1",
            "Session busy",
            "Another client was working in this session, so this message was not sent.",
            "info",
        );
        let id = store.domains.session.timeline.entries("s")[0].id;
        assert_eq!(store.domains.session.timeline.entries("s")[0].kind, EntryKind::SYSTEM_NOTICE);
        let dsl = crate::screens::board3::rows::lower(&crate::screens::board3::rows::TRow::Notice(id), &store);
        let mut cx = cx_with_vocabulary();
        let r = eval_component(&mut cx, MAIN_SPLASH_VM_ID, &dsl);
        assert!(r.is_ok(), "the notice row must evaluate: {dsl}");
    }

    /// A13 — a command receipt's compact notice row (the production rows
    /// path: store -> `rows::timeline` -> `rows::lower`) evaluates in the
    /// app VM.
    #[test]
    fn a13_receipt_rows_evaluate_in_the_app_vm() {
        use crate::screens::board3::rows::{self, TRow};
        let store = std::sync::Arc::new(octoscode_store::Store::new());
        store.set_active(Some("s".into()));
        store.domains.session.timeline.append(
            "s",
            Some(crate::screens::palette::next_receipt_turn()),
            crate::screens::palette::REPORT_KIND,
            "/cost is not available in this native build — nothing was sent to the model.".into(),
        );
        let row = rows::timeline(&store, false)
            .into_iter()
            .find(|r| matches!(r, TRow::Receipt(_)))
            .expect("the receipt row");
        let dsl = rows::lower(&row, &store);
        let mut cx = cx_with_vocabulary();
        let r = eval_component(&mut cx, MAIN_SPLASH_VM_ID, &dsl);
        assert!(r.is_ok(), "the receipt row must evaluate: {dsl}");
    }

    #[test]
    fn the_prelude_wraps_the_component_in_a_slot_sized_view() {
        // Card #21c item 3: the wrapper is a stacking (`Down`) `Fit` view, so a
        // slot's height comes from its component. `Overlay` (the #21b shape) gave
        // a `Fit` slot no measurable content height, so a component whose root is
        // a plain `View`/`DesignSurface` (user-bubble, measured 284x85) collapsed
        // to [0,0,0,0] and painted nothing. The host still owns the box:
        // `MountCache::mount` copies the Splash's declared walk onto the view, so
        // `Splash{height:190}` (the composer dock) still gets 190.
        let code = format!("{PRELUDE}i0_x := View{{width:Fill height:Fit}}}}");
        assert!(code.starts_with("use mod.prelude.widgets.*\nreturn View{"));
        assert!(code.ends_with("}}"));
        assert!(code.contains("width:Fill height:Fit flow:Down"));
    }
}

/// Where two mount strings first differ, a short window of each side (a
/// re-mount's log line names what changed).
pub fn first_difference(old: &str, new: &str) -> String {
    let at = old
        .char_indices()
        .zip(new.chars())
        .find(|((_, a), b)| a != b)
        .map(|((i, _), _)| i)
        .unwrap_or_else(|| old.len().min(new.len()));
    let start = (0..=at.saturating_sub(24)).rev().find(|&i| old.is_char_boundary(i) && new.is_char_boundary(i)).unwrap_or(0);
    let cut = |s: &str| s[start..].chars().take(72).collect::<String>().replace('\n', " ");
    format!("{:?} -> {:?}", cut(old), cut(new))
}
