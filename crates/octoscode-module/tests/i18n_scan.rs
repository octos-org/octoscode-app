//! A24 — no user-visible string bypasses `tr()`.
//!
//! A source scan of the display builders the module uses: every TEXT
//! argument of a label / text / title / button / link / placeholder /
//! notice builder must be a value, a `tr*()` call, or non-user text (an id,
//! a protocol method, a format spec, monospace data). A prose literal there
//! (`d.text(id, "Recent", …)`, `label(id, &format!("{n} queued"), …)`) is
//! copy the language switch cannot reach.
//!
//! The shell's static `script_mod!` DSL (chrome.rs, lib.rs) is checked too:
//! every non-empty `text:` / `empty_text:` literal must be re-texted by the
//! static pass (`i18n::tree`): anonymous, listed in `chrome::static_named`,
//! or set by code (then its setter is a builder call, scanned above).
//!
//! Phase 1 (A24) holds the CONVERTED files to zero; the rest of `screens/`
//! is counted and may only go DOWN (the ratchet below) — phase 2 drives it
//! to zero.
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The converted files (phase 1, then phase 2): zero bypasses allowed, and
/// every literal they wrap reads in Chinese.
const CONVERTED: &[&str] = &[
    "chrome.rs",
    "lib.rs",
    "fluid.rs",
    "seat.rs",
    "screens/sidebar.rs",
    "screens/settings.rs",
    "screens/a9_settings.rs",
    "screens/a9_prefs.rs",
    "screens/a9_connect.rs",
    "screens/copy_button.rs",
    "screens/reconnect.rs",
    "screens/discovery.rs",
    "screens/connect.rs",
    "screens/palette.rs",
    "screens/board1.rs",
    "screens/board1_kit.rs",
    "screens/pairing.rs",
    "screens/browser.rs",
    "screens/board3/ui.rs",
    "screens/board3/seats.rs",
    "screens/board3/strip.rs",
    "screens/saved_link.rs",
    // phase 2
    "screens/a9_boundary.rs",
    "screens/activity.rs",
    "screens/drafts.rs",
    "screens/board3/agents.rs",
    "screens/board3/checkpoints.rs",
    "screens/board3/fleet_console.rs",
    "screens/board3/images.rs",
    "screens/board3/inventory.rs",
    "screens/board3/research.rs",
    "screens/board3/resume.rs",
    "screens/board3/routes.rs",
    "screens/board3/rows.rs",
    "screens/board3/session_pane.rs",
];

/// The phase-2 ceiling: bypasses left in the rest of `screens/` (A24 phase 1
/// measured this). Lower it as screens are converted; it must reach 0.
const REMAINING_CEILING: usize = 142;

/// (call prefix, text-argument indices). A prefix starting with `.` or `::`
/// matches a method / path call; otherwise the name must stand alone.
const BUILDERS: &[(&str, &[usize])] = &[
    // board-3 kit (screens/board3/ui.rs) and its callers.
    (".text(", &[1]),
    (".button(", &[1]),
    (".button_ids(", &[3]),
    (".link(", &[1]),
    (".link_ids(", &[3]),
    (".input(", &[3]),
    (".input_icon(", &[3]),
    (".input_multiline(", &[3]),
    (".input_secret(", &[2]),
    ("header(", &[1]),
    ("section_title(", &[2]),
    ("field_label(", &[2]),
    ("banner(", &[2, 3]),
    ("failure(", &[2]),
    ("error_line(", &[2]),
    ("dialog_error(", &[4]),
    ("status_line(", &[2]),
    ("menu_title(", &[1]),
    ("danger_button(", &[2]),
    ("action_button(", &[2]),
    // board-1 kit (screens/board1_kit.rs).
    ("Text::new(", &[1]),
    ("Field::new(", &[]),
    (".label(", &[0]),
    (".placeholder(", &[0]),
    ("pill_primary(", &[1]),
    ("pill_outline(", &[1]),
    ("kit::link(", &[1]),
    (".header(", &[3]),
    ("callout(", &[2, 3]),
    ("kv_row(", &[0]),
    ("note_row(", &[0]),
    // fluid.rs's runs.
    ("label(", &[1]),
    ("caption(", &[0]),
    ("btn(", &[1]),
    // chrome.rs / lib.rs setters, notices.
    ("text(cx, view, ", &[1]),
    ("set_segment(", &[3]),
    (".set_text(", &[1]),
    (".set_empty_text(", &[1]),
    ("set_notice(", &[0]),
    ("set_info(", &[0]),
    ("note_held(", &[0]),
];

/// Non-user text that may stand in a builder's text position.
const ALLOW: &[&str] = &[
    "OctosCode",            // the product name (the web never translates it)
    "English",              // the language options are endonyms (PreferencesDialog.tsx:51-52)
    "简体中文",
    "Octos",                // the product name
    "sessions: {sessions}", // lib.rs's 0x0 `/g` metadata label (never drawn)
];

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// One string literal: its byte span (quotes included) and its value.
struct Lit {
    start: usize,
    end: usize,
    value: String,
}

/// The source with comments blanked and `#[cfg(test)]` items removed, kept
/// at the same byte offsets (blanked to spaces) so lines stay true.
fn code_only(src: &str) -> String {
    let b = src.as_bytes();
    let mut out = b.to_vec();
    let mut i = 0;
    let blank = |out: &mut Vec<u8>, a: usize, z: usize| {
        for c in out.iter_mut().take(z).skip(a) {
            if *c != b'\n' {
                *c = b' ';
            }
        }
    };
    while i < b.len() {
        if b[i..].starts_with(b"//") {
            let z = src[i..].find('\n').map(|n| i + n).unwrap_or(b.len());
            blank(&mut out, i, z);
            i = z;
        } else if b[i..].starts_with(b"/*") {
            let mut depth = 1;
            let mut j = i + 2;
            while j < b.len() && depth > 0 {
                if b[j..].starts_with(b"/*") {
                    depth += 1;
                    j += 2;
                } else if b[j..].starts_with(b"*/") {
                    depth -= 1;
                    j += 2;
                } else {
                    j += 1;
                }
            }
            blank(&mut out, i, j);
            i = j;
        } else if b[i] == b'"' || (b[i] == b'r' && (b.get(i + 1) == Some(&b'"') || b.get(i + 1) == Some(&b'#'))) {
            i = skip_string(b, i).unwrap_or(i + 1);
        } else if b[i] == b'\'' {
            i = skip_char(b, i);
        } else if b[i..].starts_with(b"#[cfg(test)]") {
            // Drop the item: through its matching brace (or its `;`).
            let mut j = i + "#[cfg(test)]".len();
            let mut depth = 0usize;
            while j < b.len() {
                match b[j] {
                    b'"' => {
                        j = skip_string(b, j).unwrap_or(j + 1);
                        continue;
                    }
                    b'\'' => {
                        j = skip_char(b, j);
                        continue;
                    }
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            j += 1;
                            break;
                        }
                    }
                    b';' if depth == 0 => {
                        j += 1;
                        break;
                    }
                    _ => {}
                }
                j += 1;
            }
            blank(&mut out, i, j);
            i = j;
        } else {
            i += 1;
        }
    }
    String::from_utf8(out).expect("utf8 kept")
}

/// Past a `"…"` or raw `r#"…"#` literal starting at `i` (identifiers that
/// merely end in `r` are not raw strings).
fn skip_string(b: &[u8], i: usize) -> Option<usize> {
    if b[i] == b'r' {
        if i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_') {
            return None;
        }
        let mut j = i + 1;
        let mut hashes = 0;
        while b.get(j) == Some(&b'#') {
            hashes += 1;
            j += 1;
        }
        if b.get(j) != Some(&b'"') {
            return None;
        }
        j += 1;
        loop {
            if j >= b.len() {
                return Some(j);
            }
            if b[j] == b'"' && b[j + 1..].iter().take(hashes).filter(|c| **c == b'#').count() == hashes {
                return Some(j + 1 + hashes);
            }
            j += 1;
        }
    }
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'"' => return Some(j + 1),
            _ => j += 1,
        }
    }
    Some(j)
}

/// Past a char literal, or one byte for a lifetime (`'a`).
fn skip_char(b: &[u8], i: usize) -> usize {
    if b.get(i + 1) == Some(&b'\\') {
        let mut j = i + 2;
        while j < b.len() && b[j] != b'\'' {
            j += 1;
        }
        return j + 1;
    }
    // One (possibly multi-byte) char then a quote = a char literal.
    let s = std::str::from_utf8(&b[i + 1..(i + 6).min(b.len())]).unwrap_or("");
    if let Some(c) = s.chars().next() {
        if b.get(i + 1 + c.len_utf8()) == Some(&b'\'') {
            return i + 2 + c.len_utf8();
        }
    }
    i + 1
}

fn unescape(raw: &str) -> String {
    let mut out = String::new();
    let mut it = raw.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('u') => {
                let hex: String = it.by_ref().skip(1).take_while(|c| *c != '}').collect();
                if let Some(ch) = u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    out.push(ch);
                }
            }
            Some('\n') => {
                while it.peek().is_some_and(|c| c.is_whitespace()) {
                    it.next();
                }
            }
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

/// Whether a literal reads as product prose (an id, a method, a spec, a
/// path, a DSL fragment or a symbol does not).
fn is_prose(s: &str) -> bool {
    if ALLOW.contains(&s) {
        return false;
    }
    // Placeholders (`{n}`, `{value0}`) are data, not words.
    let mut words = String::new();
    let mut depth = 0;
    for c in s.chars() {
        match c {
            '{' => depth += 1,
            '}' if depth > 0 => depth -= 1,
            _ if depth == 0 => words.push(c),
            _ => {}
        }
    }
    let letters = words.chars().filter(|c| c.is_alphabetic()).count();
    if letters < 2 {
        return false;
    }
    // An environment / constant name (`CARGO_PKG_VERSION`).
    if s.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_') {
        return false;
    }
    if s.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)) {
        return true; // a Chinese literal outside the catalog is a bypass too
    }
    let id_like = s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_.:/#-{}*@".contains(c));
    if id_like {
        return false;
    }
    if [":=", "width:", "height:", "draw_", "flow:", "padding:", "TextStyle", "file_resource", "Align{"]
        .iter()
        .any(|k| s.contains(k))
    {
        return false;
    }
    if s.starts_with('/') || s.starts_with("http") || s.starts_with('#') || s.starts_with('.') {
        return false;
    }
    if s == "Fit" || s == "Fill" || s == "Down" || s == "Right" || s == "Overlay" {
        return false;
    }
    // A capitalised word, or words with a space: prose.
    s.contains(' ') || s.chars().next().is_some_and(|c| c.is_uppercase())
}

/// The argument spans of the call whose `(` is at `open`.
fn args_of(code: &str, open: usize) -> Vec<(usize, usize)> {
    let b = code.as_bytes();
    let mut out = Vec::new();
    let (mut depth, mut start, mut j) = (0usize, open + 1, open + 1);
    while j < b.len() {
        match b[j] {
            b'"' | b'r' if b[j] == b'"' || skip_string(b, j).is_some() => {
                j = skip_string(b, j).unwrap_or(j + 1);
                continue;
            }
            b'\'' => {
                j = skip_char(b, j);
                continue;
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                if depth == 0 {
                    out.push((start, j));
                    return out;
                }
                depth -= 1;
            }
            b',' if depth == 0 => {
                out.push((start, j));
                start = j + 1;
            }
            _ => {}
        }
        j += 1;
    }
    out
}

/// The spans of the `tr*( … )` calls inside an argument: a literal anywhere
/// inside one (`tr(if busy { "Stopping…" } else { "Stop server" })`) is
/// translated.
fn tr_spans(arg: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    for w in ["tr(", "tr1(", "tr_with(", "tr_in(", "tr_ctx(", "text_in(", "keep("] {
        let mut from = 0;
        while let Some(at) = arg[from..].find(w).map(|n| n + from) {
            from = at + 1;
            let b = arg.as_bytes();
            if at > 0 && (b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_') {
                continue;
            }
            let open = at + w.len() - 1;
            let spans = args_of(arg, open);
            if let (Some(first), Some(last)) = (spans.first(), spans.last()) {
                out.push((first.0, last.1));
            }
        }
    }
    out
}

/// The string literals in an argument that no `tr*()` call wraps.
fn bare_literals(arg: &str) -> Vec<Lit> {
    let b = arg.as_bytes();
    let spans = tr_spans(arg);
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'"' {
            let end = skip_string(b, i).unwrap_or(b.len());
            let wrapped = spans.iter().any(|(a, z)| *a <= i && end <= *z);
            if !wrapped {
                out.push(Lit { start: i, end, value: unescape(&arg[i + 1..end.saturating_sub(1)]) });
            }
            i = end;
        } else {
            i += 1;
        }
    }
    out
}

/// Every builder-argument bypass in one file: (line, call, text).
fn bypasses(src: &str) -> Vec<(usize, String, String)> {
    let code = code_only(src);
    let mut out = Vec::new();
    for (prefix, idxs) in BUILDERS {
        let mut from = 0;
        while let Some(at) = code[from..].find(prefix).map(|n| n + from) {
            from = at + 1;
            // A bare name is a free call: `ui::header(` counts, a method
            // of the same name (`req.header("Authorization", …)`) does not.
            let standalone = prefix.starts_with('.')
                || prefix.contains("::")
                || at == 0
                || !(code.as_bytes()[at - 1].is_ascii_alphanumeric() || matches!(code.as_bytes()[at - 1], b'_' | b'.'));
            if !standalone {
                continue;
            }
            // A definition (`fn label(`) is not a call.
            if code[..at].trim_end().ends_with("fn") {
                continue;
            }
            let open = at + prefix.len() - 1;
            let args = args_of(&code, open);
            for &ix in idxs.iter() {
                let Some(&(a, z)) = args.get(ix) else { continue };
                let arg = &code[a..z];
                for lit in bare_literals(arg) {
                    if is_prose(&lit.value) {
                        let line = code[..a + lit.start].matches('\n').count() + 1;
                        let _ = lit.end;
                        out.push((line, prefix.to_string(), lit.value));
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(root().join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

fn screens_files() -> Vec<String> {
    rs_files("screens")
}

/// Every source file under `src/<dir>` (`""` = the whole crate), as a path
/// relative to `src/`.
fn rs_files(dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![root().join(dir)];
    while let Some(dir) = stack.pop() {
        for e in std::fs::read_dir(&dir).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "rs") {
                out.push(p.strip_prefix(root()).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_scanner_sees_a_bypass_and_accepts_wrapped_copy() {
    let src = r#"
fn view(d: &mut Dsl) {
    d.text("t_title", "Recent", &t);          // bypass
    d.text("t_ok", tr("Recent"), &t);        // wrapped
    d.button("b", &format!("{} queued", n), "ev", Btn::Primary, W::Fit, 32.0); // bypass
    d.text("t_id", "b3_title_id", &t);        // an id
    label("x", tr1("{value0} queued", &n), &st, INK, "width: Fit");
    // d.text("c", "Commented out", &t);
}
#[cfg(test)]
mod tests { fn f(d: &mut Dsl) { d.text("x", "Test only", &t); } }
"#;
    let found: Vec<String> = bypasses(src).into_iter().map(|(_, _, t)| t).collect();
    assert_eq!(found, vec!["Recent".to_owned(), "{} queued".to_owned()], "{found:?}");
}

/// The converted surfaces carry no bypass at all.
#[test]
fn no_converted_surface_bypasses_tr() {
    let mut bad = Vec::new();
    for rel in CONVERTED {
        for (line, call, text) in bypasses(&read(rel)) {
            bad.push(format!("{rel}:{line} {call}… {text:?}"));
        }
    }
    assert!(bad.is_empty(), "{} user-visible literal(s) bypass tr():\n{}", bad.len(), bad.join("\n"));
}

/// The rest of `screens/` (phase 2): counted, and the count may only fall.
#[test]
fn the_remaining_screens_only_get_fewer_bypasses() {
    let mut per_file: BTreeMap<String, Vec<(usize, String, String)>> = BTreeMap::new();
    for rel in screens_files() {
        if CONVERTED.contains(&rel.as_str()) {
            continue;
        }
        let found = bypasses(&read(&rel));
        if !found.is_empty() {
            per_file.insert(rel, found);
        }
    }
    let total: usize = per_file.values().map(Vec::len).sum();
    println!("A24 phase-2 remainder: {total} bypass(es) in {} file(s):", per_file.len());
    for (f, found) in &per_file {
        println!("  {:4}  {f}", found.len());
    }
    // `--nocapture` lists each one (file:line, the builder, the text).
    for (f, found) in &per_file {
        for (line, call, text) in found {
            println!("    {f}:{line} {call}… {text:?}");
        }
    }
    assert!(total <= REMAINING_CEILING, "{total} > the ceiling {REMAINING_CEILING}: new copy bypasses tr()");
}

/// The shell's static DSL: every non-empty `text:` / `empty_text:` literal
/// is re-texted by the static pass — anonymous, a listed static name, or a
/// name the code sets (through a scanned builder).
#[test]
fn every_static_shell_literal_is_re_texted() {
    let named: Vec<String> = {
        let chrome = read("chrome.rs");
        let at = chrome.find("pub fn static_named()").expect("static_named");
        let body = &chrome[at..at + chrome[at..].find("\n}\n").unwrap()];
        body.split("live_id!(").skip(1).map(|s| s[..s.find(')').unwrap()].to_owned()).collect()
    };
    assert!(named.len() >= 7, "{named:?}");
    let all_code: String = ["chrome.rs", "lib.rs", "a9_host.rs"].iter().map(|f| code_only(&read(f))).collect();
    let mut bad = Vec::new();
    for rel in ["chrome.rs", "lib.rs"] {
        let src = read(rel);
        for (n, line) in src.lines().enumerate() {
            for key in ["text: \"", "empty_text: \""] {
                let Some(k) = line.find(key) else { continue };
                if key == "text: \"" && line[..k].ends_with("empty_") {
                    continue;
                }
                let rest = &line[k + key.len()..];
                let Some(q) = rest.find('"') else { continue };
                let value = &rest[..q];
                if value.is_empty() || !is_prose(value) {
                    continue;
                }
                if key == "empty_text: \"" {
                    continue; // every TextInput placeholder is re-texted
                }
                // `name := Kind{… text: "…"` on this line, or a template
                // override `name +: {text: "…"}`.
                let head = &line[..k];
                // The nearest naming marker before the literal: `name :=`
                // or a template override `name +: {`.
                let marker = match (head.rfind(":="), head.rfind("+:")) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (a, b) => a.or(b),
                };
                let name = marker
                    .map(|p| head[..p].trim_end())
                    .and_then(|h| h.rsplit(|c: char| !(c.is_alphanumeric() || c == '_')).next())
                    .filter(|s| !s.is_empty());
                let ok = match name {
                    None => true, // anonymous
                    Some(id) if named.iter().any(|x| x == id) => true,
                    Some(id) => all_code.contains(&format!("ids!({id})")) || all_code.contains(&format!("live_id!({id})")),
                };
                if !ok {
                    bad.push(format!("{rel}:{} {:?}", n + 1, value));
                }
            }
        }
    }
    assert!(bad.is_empty(), "static shell copy the language switch cannot reach:\n{}", bad.join("\n"));
}

/// The literal copy each `tr*()` call names in one file: (line, key). A
/// literal inside the source argument counts (`tr(if busy { "Saving…" } else
/// { "Save" })`), except a match pattern or a comparison operand (wire
/// values that pick the copy). `tr_ctx(ctx, "…")` names `ctx|…`.
fn wrapped_literals(src: &str) -> Vec<(usize, String)> {
    let code = code_only(src);
    let b = code.as_bytes();
    let mut out = Vec::new();
    for w in ["tr(", "tr1(", "tr_with(", "tr_ctx("] {
        let mut from = 0;
        while let Some(at) = code[from..].find(w).map(|n| n + from) {
            from = at + 1;
            if at > 0 && (b[at - 1].is_ascii_alphanumeric() || b[at - 1] == b'_') {
                continue;
            }
            if code[..at].trim_end().ends_with("fn") {
                continue;
            }
            let args = args_of(&code, at + w.len() - 1);
            let (ctx, source) = if w == "tr_ctx(" {
                let ctx = args.first().and_then(|&(a, z)| {
                    let t = code[a..z].trim();
                    (t.starts_with('"') && t.ends_with('"')).then(|| unescape(&t[1..t.len() - 1]))
                });
                (ctx, args.get(1).copied())
            } else {
                (None, args.first().copied())
            };
            let Some((a, z)) = source else { continue };
            let mut i = a;
            while i < z {
                if b[i] != b'"' {
                    i += 1;
                    continue;
                }
                let end = skip_string(b, i).unwrap_or(z).min(z);
                let after = code[end..z].trim_start();
                let before = code[a..i].trim_end();
                // A nested call's argument (`tr(x.trim_start_matches("Lease "))`,
                // `tr(label("driver_fence_stale"))`) is not the copy either.
                let operand = after.starts_with("=>")
                    || after.starts_with('|')
                    || after.starts_with("==")
                    || after.starts_with("!=")
                    || before.ends_with('|')
                    || before.ends_with("==")
                    || before.ends_with("!=")
                    || before.ends_with('(')
                    || before.ends_with(',');
                let value = unescape(&code[i + 1..end.saturating_sub(1)]);
                if !operand && value.chars().filter(|c| c.is_alphabetic()).count() >= 2 {
                    let key = match &ctx {
                        Some(c) => format!("{c}|{value}"),
                        None => value,
                    };
                    out.push((code[..i].matches('\n').count() + 1, key));
                }
                i = end;
            }
        }
    }
    out
}

/// Whether a wrapped key reads in Chinese: the web's catalog (or an alias of
/// it) or the native supplement; a context key falls back to its source.
fn reads_in_chinese(key: &str) -> bool {
    use octoscode_module::i18n::{native_zh, zh_for};
    match key.split_once('|') {
        Some((_, source)) if native_zh(key).is_some() || zh_for(source).is_some() => true,
        _ => zh_for(key).is_some(),
    }
}

/// Every literal the converted surfaces route through `tr*()` has Chinese:
/// the web's catalog first, else the reviewed native supplement
/// (`i18n/native.rs`) — wrapped copy that would still render English in
/// Chinese fails here. The rest of the crate is listed, not asserted (its
/// owners add their supplement entries as they convert).
#[test]
fn every_wrapped_literal_reads_in_chinese() {
    let mut bad = Vec::new();
    for rel in CONVERTED {
        for (line, key) in wrapped_literals(&read(rel)) {
            if !reads_in_chinese(&key) {
                bad.push(format!("{rel}:{line} {key:?}"));
            }
        }
    }
    let mut rest = Vec::new();
    for rel in rs_files("") {
        if CONVERTED.contains(&rel.as_str()) || rel.starts_with("i18n/") {
            continue;
        }
        for (line, key) in wrapped_literals(&read(&rel)) {
            if !reads_in_chinese(&key) {
                rest.push(format!("{rel}:{line} {key:?}"));
            }
        }
    }
    println!("A24: {} wrapped literal(s) outside the converted files still read English:", rest.len());
    for r in &rest {
        println!("  {r}");
    }
    assert!(bad.is_empty(), "{} wrapped literal(s) with no Chinese:\n{}", bad.len(), bad.join("\n"));
}

/// The scanner reads the source argument and skips the values that pick it.
#[test]
fn the_wrapped_literal_scan_reads_sources_not_operands() {
    let src = r#"
fn f() {
    d.text("a", tr("Recent"), &t);
    let l = tr(if busy { "Saving…" } else { "Save" });
    let m = tr(match mode { "read_only" => "Read", _ => "Full access" });
    let n = tr1("{value0} queued", &n.to_string());
    let v = tr_ctx("verb", "Type");
    let x = attr("Not a call");
}
"#;
    let keys: Vec<String> = wrapped_literals(src).into_iter().map(|(_, k)| k).collect();
    for want in ["Recent", "Saving…", "Save", "Read", "Full access", "{value0} queued", "verb|Type"] {
        assert!(keys.contains(&want.to_owned()), "{want}: {keys:?}");
    }
    assert!(!keys.contains(&"read_only".to_owned()) && !keys.contains(&"Not a call".to_owned()), "{keys:?}");
    assert!(reads_in_chinese("verb|Type") && reads_in_chinese("Recent") && !reads_in_chinese("No such copy anywhere"));
}
