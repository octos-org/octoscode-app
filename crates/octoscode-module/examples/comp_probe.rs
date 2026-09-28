//! Card #17 probe — lower ONE item component through the same chain a card uses
//! (`components::lower` → `l0::prepare` → `design::to_makepad_ui`) and report
//! what came out, so the screen work is built on a proven path.
//!
//! ```sh
//! cargo run -p octoscode-module --example comp_probe -- user-bubble
//! ```
use octoscode_module::components::{self, ItemKind};

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_else(|| "user-bubble".into());
    let kind = ItemKind::from_id(&arg).unwrap_or_else(|| {
        eprintln!("unknown component id {arg:?}; known: {:?}", components::all_ids());
        std::process::exit(2);
    });

    let (component, on_disk) = components::resolve(kind);
    println!(
        "component {} ({}), on_disk={on_disk}, binds={:?}",
        component.id,
        kind.id(),
        component.bindings.iter().map(|b| (b.copy, b.binding)).collect::<Vec<_>>()
    );

    let copies: Vec<(String, String)> = component
        .bindings
        .iter()
        .map(|b| (b.copy.to_owned(), format!("<{} value>", b.copy)))
        .collect();

    match components::lower(kind, "0", &copies) {
        Ok(dsl) => {
            println!("LOWER OK: {} bytes, {} widgets", dsl.len(), dsl.matches(" := ").count());
            let hits = copies.iter().filter(|(_, v)| dsl.contains(v)).count();
            println!("  live value hits in DSL: {}/{}", hits, copies.len());
            if let Ok(path) = std::env::var("COMP_PROBE_OUT") {
                let _ = std::fs::write(path, &dsl);
            }
            println!("  first 400 chars:\n{}", &dsl[..dsl.len().min(400)]);
        }
        Err(e) => {
            println!("LOWER FAILED: {e}");
            std::process::exit(1);
        }
    }
}
