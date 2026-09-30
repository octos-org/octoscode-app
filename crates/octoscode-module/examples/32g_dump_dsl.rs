use octoscode_module::screens::connect::{self, ConnectUi, Screen};
fn main() {
    let ui = ConnectUi::default();
    let dsl = connect::lower_screen(Screen::Connect, &ui).expect("lower");
    for (i, line) in dsl.lines().enumerate().take(26) {
        let mark = if i + 1 == 20 { " >>>" } else { "    " };
        println!("{}{:3}| {}", mark, i + 1, line);
    }
    let l20 = dsl.lines().nth(19).unwrap_or("");
    println!("--- line 20 len = {} ---", l20.len());
    if l20.len() > 130 {
        println!("col 130..: {:?}", &l20[130..]);
    }
}
