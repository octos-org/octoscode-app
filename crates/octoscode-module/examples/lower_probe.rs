//! Card #15b probe — lower one card through the SAME path the Gate-B renders
//! used, and print what comes out.
//!
//! ```sh
//! cargo run -p octoscode-module --example lower_probe -- conversation-01
//! ```
//!
//! The chain (card-host/src/host.rs:144-154):
//!   `octoscript_makepad::l0::prepare(card, data, kit_dir)`
//!     → `octoscript_makepad::design::to_makepad_ui(&prepared.tree)` → DSL string.
use std::path::PathBuf;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let id = args.get(1).cloned().unwrap_or_else(|| "conversation-01".to_owned());
    let cards = PathBuf::from(
        std::env::var("OCTOSCODE_CARDS_DIR").unwrap_or_else(|_| "design/cards".to_owned()),
    );
    let dir = cards.join(&id);

    let card = std::fs::read_to_string(dir.join("page.card")).expect("page.card");
    let data_text = std::fs::read_to_string(dir.join("page.data.json")).unwrap_or_else(|_| "{}".into());
    let data: serde_json::Value = serde_json::from_str(&data_text).expect("page.data.json");

    let kit = dir.join("kit");
    println!("--- prepare({id}) kit={}", kit.display());
    let prepared = match octoscript_makepad::l0::prepare(&card, &data, &kit) {
        Ok(p) => p,
        Err(e) => {
            println!("PREPARE FAILED: {e}");
            std::process::exit(1);
        }
    };
    println!("prepared: native_components={} source_len={}", prepared.native_components, prepared.source.len());

    let want: Vec<String> = ["New chat", "Fix steer queue drop", "OctosCode v", "Ask Octos anything", "Worked for"]
        .iter().map(|s| s.to_string()).collect();

    println!("--- design::to_makepad_ui (the `measured` branch)");
    match octoscript_makepad::design::to_makepad_ui(&prepared.tree) {
        Ok(ui) => {
            let hits = want.iter().filter(|w| ui.contains(*w)).count();
            println!("DESIGN OK: {} bytes, label hits={}/{}, nodes={}", ui.len(), hits, want.len(), ui.matches(" := ").count());
            let _ = std::fs::write(format!("/tmp/probe-{id}-design.splash"), &ui);
        }
        Err(e) => println!("DESIGN FAILED: {e}"),
    }

    println!("--- to_makepad_l0_ui (the `l0-kit` branch)");
    let mut tree = prepared.tree.clone();
    octoscript_makepad::l0::inspectable(&mut tree);
    let ui = octoscript_makepad::to_makepad_l0_ui(&tree);
    let hits = want.iter().filter(|w| ui.contains(*w)).count();
    println!("L0KIT: {} bytes, label hits={}/{}, nodes={}", ui.len(), hits, want.len(), ui.matches(" := ").count());
    let _ = std::fs::write(format!("/tmp/probe-{id}-l0kit.splash"), &ui);
}
