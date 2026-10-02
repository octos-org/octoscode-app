//! A8 — dump the Session settings pane's lowered DSL (numbered), for checking
//! a mount error's line against the text the app evaluated.
//! `cargo run -p octoscode-module --example a8_dump_pane -- [loading|loaded] [lines]`
use octoscode_module::screens::board3::{session_pane, ui};
use octoscode_store::Store;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let which = args.get(1).map(String::as_str).unwrap_or("loaded");
    let store = Store::new();
    store.set_active(Some("a8:main".into()));
    store.set_connection("Live".into(), true);
    store.domains.config.set_supported_methods(
        ["session/status/read", "permission/profile/list", "permission/profile/set", "profile/llm/list"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );
    let mut st = session_pane::PaneState::default();
    if which == "loaded" {
        st.status = Some(session_pane::StatusFacts {
            model: Some("deepseek-v4-flash".into()),
            approval_policy: Some("on-request".into()),
            sandbox: Some("workspace-write".into()),
            network: Some("allowed".into()),
            read_paths: None,
            ..Default::default()
        });
        st.models = session_pane::parse_models(&serde_json::json!({"llm": {
            "primary": {"model_id": "deepseek-v4-flash", "family_id": "deepseek", "route_id": "deepseek", "route": {"route_id": "deepseek", "label": "Official API"}, "selected": true, "available": true},
            "fallbacks": [{"model_id": "glm-5", "family_id": "zhipu", "route_id": "zhipu", "route": {"route_id": "zhipu", "label": "BigModel API"}, "selected": false, "available": true}]}}));
    } else {
        st.loading = true;
    }
    let mut d = ui::Dsl::new();
    session_pane::build(&mut d, &st, &mut Default::default(), &ui::Frame::DESKTOP, &store);
    let dsl = d.finish();
    let want: Vec<usize> = args.iter().skip(2).filter_map(|a| a.parse().ok()).collect();
    for (i, line) in dsl.lines().enumerate() {
        let n = i + 1;
        if want.is_empty() || want.iter().any(|w| n + 3 >= *w && n <= w + 3) {
            println!("{n:4}: {}", &line[..line.len().min(220)]);
        }
    }
}
