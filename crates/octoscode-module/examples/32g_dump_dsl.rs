//! #32h — dump the FULL lowered Connect DSL (the #32g tool capped at 26
//! lines and hid the button block; this prints everything, numbered).
use octoscode_module::screens::connect::{self, ConnectUi, Screen};

fn main() {
    let ui = ConnectUi::default();
    let dsl = connect::lower_screen(Screen::Connect, &ui).expect("lower");
    for (i, line) in dsl.lines().enumerate() {
        println!("{:3}| {}", i + 1, line);
    }
    println!("--- total {} lines ---", dsl.lines().count());
}
