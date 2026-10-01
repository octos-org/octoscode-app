//! #32h — dump the FULL lowered component DSL (comp_probe caps the preview
//! at 400 chars, which hid the label block from the dark-on-dark triage).
//!
//! ```sh
//! cargo run -p octoscode-module --example 32h_comp_dsl -- new-chat
//! ```
use octoscode_module::components::{self, ItemKind};

fn main() {
    let arg = std::env::args().nth(1).unwrap_or_else(|| "new-chat".into());
    let kind = ItemKind::from_id(&arg).unwrap_or_else(|| {
        eprintln!("unknown component id {arg:?}; known: {:?}", components::all_ids());
        std::process::exit(2);
    });
    let dsl = components::lower(kind, "t0", &[]).expect("lower");
    println!("{dsl}");
}
