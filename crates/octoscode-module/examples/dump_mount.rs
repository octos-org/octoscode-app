//! #28e4 probe — print the EXACT DSL `screens::connect::lower_screen` mounts
//! (post-`inspectable`), so the seated geometry can be compared with the
//! pre-inspectable `lower_probe` dump.
use octoscode_module::screens::connect::{self, Screen};

fn main() {
    let ui = connect::ConnectUi::default();
    let dsl = connect::lower_screen(Screen::Connect, &ui).expect("connect lowers");
    let mut n = 0;
    for line in dsl.lines() {
        println!("{line}");
        n += 1;
        if n >= 14 {
            break;
        }
    }
    println!("--- width/height lines ---");
    for line in dsl.lines() {
        if line.contains("width:") && line.contains("height:") {
            println!("{}", line.trim());
        }
    }
}
