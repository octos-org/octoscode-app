//! Card #17 scratch probe — does a `PortalList` item template that contains a
//! `Splash` instantiate per row, and does each row accept its own lowered body?
//!
//! ```sh
//! MAKEPAD_HIDE_WINDOWS=1 cargo run -p octoscode-module --example pl_probe -- 8397
//! curl -s 'http://127.0.0.1:8397/snap?all=1'
//! ```
use makepad_widgets::*;

pub use makepad_widgets;

app_main!(App);

script_mod! {
    use mod.prelude.widgets.*
    use mod.widgets.*

    let ProbeRoot = #(PlProbe::register_widget(vm)) {
        width: Fill height: Fill flow: Down
        list := PortalList {
            width: 400 height: 220
            Row := View {
                width: Fill height: Fit flow: Down padding: 2
                row_label := Label { width: Fill height: Fit text: "row" }
                row_splash := Splash { width: Fill height: 24 }
            }
        }
    }

    mod.gc.set_static(ProbeRoot)
    mod.gc.run()

    startup() do #(App::script_component(vm)){
        ui: Root{
            main_window := Window{
                window.inner_size: vec2(420 260)
                body +: { probe := ProbeRoot{} }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
}

impl MatchEvent for App {}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        crate::makepad_widgets::script_mod(vm);
        self::script_mod(vm)
    }
    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}

#[derive(Script, ScriptHook, Widget)]
struct PlProbe {
    #[deref]
    view: View,
    #[rust]
    ticks: usize,
}

impl Widget for PlProbe {
    fn handle_event(&mut self, cx: &mut Cx, event: &Event, scope: &mut Scope) {
        if self.ticks < 2 {
            self.ticks += 1;
            self.view.redraw(cx);
        }
        self.view.handle_event(cx, event, scope);
    }

    fn draw_walk(&mut self, cx: &mut Cx2d, scope: &mut Scope, walk: Walk) -> DrawStep {
        while let Some(item) = self.view.draw_walk(cx, scope, walk).step() {
            if let Some(mut list) = item.as_portal_list().borrow_mut() {
                list.set_item_range(cx, 0, 40);
                while let Some(item_id) = list.next_visible_item(cx) {
                    let row = list.item(cx, item_id, id!(Row));
                    row.label(cx, ids!(row_label))
                        .set_text(cx, &format!("row {item_id}"));
                    // The lowered body is a Splash DSL string — the same shape
                    // `components::lower` returns.
                    let dsl = format!(
                        "View{{width:Fill height:24 flow:Down padding:1 \
                         body_label := Label{{width:Fill height:Fit text:\"body of row {item_id}\"}}}}"
                    );
                    row.splash(cx, ids!(row_splash)).set_text(cx, &dsl);
                    row.draw_all_unscoped(cx);
                }
            }
        }
        DrawStep::done()
    }
}
