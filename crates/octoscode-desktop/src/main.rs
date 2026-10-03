//! Standalone OctosCode application. No shell, module host, or local kernel.
pub use makepad_widgets;

use makepad_widgets::*;

app_main!(App, configure: |_cx: &mut Cx| {
    if let Err(error) = configure_connection() {
        eprintln!("OctosCode: {error}");
        std::process::exit(2);
    }
});

fn configure_connection() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut token_file = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--server" | "--token-file" | "--workspace" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("{arg} requires a value"))?;
                match arg.as_str() {
                    "--server" => std::env::set_var("OCTOS_BASE_URL", value),
                    "--workspace" => std::env::set_var("OCTOS_WORKSPACE_CWD", value),
                    _ => token_file = Some(value),
                }
            }
            _ => {} // Makepad owns its own flags, including --remote.
        }
    }
    if let Some(file) = token_file {
        let server = std::env::var("OCTOS_BASE_URL")
            .map_err(|_| "--token-file requires --server".to_owned())?;
        let token = std::fs::read_to_string(file)
            .map_err(|e| format!("cannot read access token file: {e}"))?;
        let token = token.trim();
        if token.is_empty() {
            return Err("access token file is empty".into());
        }
        std::env::set_var("OCTOS_BEARER", token);
        octoscode_module::credentials::remember_server(&server)?;
        octoscode_module::credentials::remember_token(&server, token)?;
    }
    Ok(())
}

script_mod! {
    use mod.prelude.widgets.*

    startup() do #(App::script_component(vm)) {
        ui: Root {
            main_window := Window {
                window.title: "OctosCode"
                window.inner_size: vec2(1280, 800)
                caption_bar +: {
                    caption_label +: {
                        label +: { draw_text.text_style: theme.oc_text_row }
                    }
                }
                body +: {
                    content := View {
                        width: Fill height: Fill
                        flow: Overlay
                    }
                }
            }
        }
    }
}

#[derive(Script, ScriptHook)]
pub struct App {
    #[live]
    ui: WidgetRef,
    #[rust]
    sized: bool,
}

impl MatchEvent for App {
    fn handle_startup(&mut self, cx: &mut Cx) {
        let root = cx.with_vm(octoscode_desktop::create_view);
        let content = self.ui.view(cx, ids!(content));
        if let Some(mut view) = content.borrow_mut() {
            view.children.push((live_id!(octoscode), root));
            cx.widget_tree_mark_dirty(view.widget_uid());
            view.redraw(cx);
        };
    }
}

impl AppMain for App {
    fn script_mod(vm: &mut ScriptVm) -> ScriptValue {
        makepad_widgets::script_mod(vm);
        octoscode_desktop::register_widgets(vm);
        self::script_mod(vm)
    }

    fn handle_event(&mut self, cx: &mut Cx, event: &Event) {
        if let Event::Draw(_) = event {
            if !self.sized {
                self.sized = true;
                let (w, h) = octoscode_desktop::window_size();
                if (w, h) != octoscode_desktop::DEFAULT_WINDOW_SIZE {
                    self.ui.window(cx, ids!(main_window)).resize(cx, dvec2(w, h));
                }
            }
        }
        self.match_event(cx, event);
        self.ui.handle_event(cx, event, &mut Scope::empty());
    }
}
