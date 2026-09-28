//! Card #17 scratch probe — find the minimal ledger form that lowers through the
//! `l0::prepare` → `design::to_makepad_ui` chain WITHOUT a kit.json pack.
//!
//! ```sh
//! cargo run -p octoscode-module --example l0try
//! ```
//! Prints, per candidate, whether it lowered and what it produced.
use std::path::Path;

fn try_form(name: &str, ledger: &str, data: serde_json::Value) {
    let dir = Path::new("/tmp/l0try-empty-kit");
    match octoscript_makepad::l0::prepare(ledger, &data, dir) {
        Ok(p) => {
            let mut tree = p.tree;
            octoscript_makepad::l0::inspectable(&mut tree);
            match octoscript_makepad::design::to_makepad_ui(&tree) {
                Ok(ui) => println!(
                    "[{name}] OK native_components={} dsl={} bytes widgets={}\n{}",
                    p.native_components,
                    ui.len(),
                    ui.matches(" := ").count(),
                    ui
                ),
                Err(e) => println!("[{name}] design::to_makepad_ui FAILED: {e}"),
            }
        }
        Err(e) => println!("[{name}] l0::prepare FAILED: {e}"),
    }
}

fn main() {
    let _ = std::fs::create_dir_all("/tmp/l0try-empty-kit");

    // A. plain design vocabulary, sizes on the nodes.
    try_form(
        "A-plain-view",
        "# level: L0\ntheme light\nview root View {\n  Label { text: \"hi\" width: 100 height: 20 }\n}\n",
        serde_json::json!({}),
    );

    // B. plain, with a copy.
    try_form(
        "B-plain-copy",
        "# level: L0\ntheme light\ncopy t { class: user-copy, en: \"hi\" }\nview root View {\n  Label { text: copy.t width: 100 height: 20 }\n}\n",
        serde_json::json!({}),
    );

    // C. Kit with a local component decl but NO kit.json (does it hit the pack branch?).
    try_form(
        "C-kit-nopack",
        "# level: L0\ntheme light\ncopy t { class: user-copy, en: \"hi\" }\ncomponent TextP(instance: text, text: text) {\n  view Kit(component: \"TextP\", instance: instance, text: text)\n}\nview root View {\n  TextP(instance: \"t1\", text: copy.t)\n}\n",
        serde_json::json!({}),
    );

    // D. the real card, with ITS kit dir (proves the pack path works at all).
    let cards = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/cards/conversation-01");
    if let (Ok(card), Ok(data)) = (
        std::fs::read_to_string(cards.join("page.card")),
        std::fs::read_to_string(cards.join("page.data.json")),
    ) {
        let data: serde_json::Value = serde_json::from_str(&data).unwrap_or(serde_json::json!({}));
        try_form("D-card-with-kit", &card, data);
        // show how much of the kit.json the pack branch needs
        let kit = std::fs::read_to_string(cards.join("kit/native/light/kit.json")).unwrap_or_default();
        let kit: serde_json::Value = serde_json::from_str(&kit).unwrap_or(serde_json::json!({}));
        println!("--- kit.json keys: {:?}", kit.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>()));
        println!("--- kit.json components: {:?}", kit.get("components").and_then(|c| c.as_object()).map(|o| o.keys().cloned().collect::<Vec<_>>()));
        if let Some(c) = kit.get("components").and_then(|c| c.as_object()).and_then(|o| o.values().next()) {
            println!("--- one component def: {}", serde_json::to_string_pretty(c).unwrap_or_default());
        }
        // E. a PLACEHOLDER: my own ledger + placements, reusing a real card's
        // kit components (Surface16fae9a63e16 + Text50ebf376856a) and its kit dir.
        let rel = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/cards/conversation-01");
        let ledger = "# level: L0\ntheme light\n\
            copy thread_row_title { class: user-copy, en: \"Thread title\" }\n\
            copy thread_row_meta { class: user-copy, en: \"0 messages\" }\n\
            component SurfaceP(instance: text) {\n  view Kit(component: \"Surface16fae9a63e16\", instance: instance) { slot }\n}\n\
            component TextP(instance: text, text: text) {\n  view Kit(component: \"Text50ebf376856a\", instance: instance, text: text)\n}\n\
            view root SurfaceP(instance: \"page\") {\n\
              TextP(instance: \"title\", text: copy.thread_row_title)\n\
              TextP(instance: \"meta\", text: copy.thread_row_meta)\n}\n";
        let data = serde_json::json!({"$kit": {"theme": "light", "placements": {
            "page": {"component": "Surface16fae9a63e16", "layout": {"x": 0, "y": 0, "w": 360, "h": 44}},
            "title": {"component": "Text50ebf376856a", "layout": {"x": 10, "y": 4, "w": 320, "h": 20}},
            "meta": {"component": "Text50ebf376856a", "layout": {"x": 10, "y": 26, "w": 320, "h": 16}}
        }}});
        let kit = rel.join("kit");
        match octoscript_makepad::l0::prepare(ledger, &data, &kit) {
            Ok(p) => {
                let mut tree = p.tree;
                octoscript_makepad::l0::inspectable(&mut tree);
                match octoscript_makepad::design::to_makepad_ui(&tree) {
                    Ok(ui) => println!("[E-placeholder-realkit] OK native_components={} dsl={} bytes widgets={}\n{}", p.native_components, ui.len(), ui.matches(" := ").count(), ui),
                    Err(e) => println!("[E-placeholder-realkit] design FAILED: {e}"),
                }
            }
            Err(e) => println!("[E-placeholder-realkit] l0::prepare FAILED: {e}"),
        }
        // E2. same placeholder ledger, but against a MINIMAL shared kit pack.
        // The ledger's `Kit(component: "SurfaceP")` must name a kit component,
        // so this ledger references SurfaceP/TextP (the minimal kit's names).
        let ledger2 = "# level: L0\ntheme light\n\
            copy thread_row_title { class: user-copy, en: \"Thread title\" }\n\
            copy thread_row_meta { class: user-copy, en: \"0 messages\" }\n\
            component SurfaceP(instance: text) {\n  view Kit(component: \"SurfaceP\", instance: instance) { slot }\n}\n\
            component TextP(instance: text, text: text) {\n  view Kit(component: \"TextP\", instance: instance, text: text)\n}\n\
            view root SurfaceP(instance: \"page\") {\n\
              TextP(instance: \"title\", text: copy.thread_row_title)\n\
              TextP(instance: \"meta\", text: copy.thread_row_meta)\n}\n";
        let kit2 = std::path::Path::new("/tmp/minimal-kit");
        match octoscript_makepad::l0::prepare(ledger2, &data, kit2) {
            Ok(p) => {
                let mut tree = p.tree;
                octoscript_makepad::l0::inspectable(&mut tree);
                match octoscript_makepad::design::to_makepad_ui(&tree) {
                    Ok(ui) => println!("[E2-minimal-kit] OK native_components={} dsl={} bytes widgets={}\n{}", p.native_components, ui.len(), ui.matches(" := ").count(), &ui[..ui.len().min(700)]),
                    Err(e) => println!("[E2-minimal-kit] design FAILED: {e}"),
                }
            }
            Err(e) => println!("[E2-minimal-kit] l0::prepare FAILED: {e}"),
        }
    } else {
        println!("[D-card-with-kit] could not read the card");
    }
}
