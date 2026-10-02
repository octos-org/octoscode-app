//! Board-3 screen 12 (right) — the composer's VIM EDITING subset (row:
//! composer × 1, "Vim editing mode in the composer: Insert/Normal, pending
//! operators, caret clamping inside surrogate pairs, Escape semantics that do
//! not swallow an interruption").
//!
//! Web: `features/composer/vim-edit.ts:1-313` (the pinned TUI's subset, a
//! pure reducer), driven by `features/composer/ComposerInput.tsx:160-235`
//! and toggled by `/vimmode` (alias `/vim-mode`, `registry.ts:104-111`;
//! `App.tsx:1267-1269` flips the preference). The composer shows `Vim ·
//! Insert` / `Vim · Normal` under the input (`ComposerInput.tsx:270-274`).
//!
//! [`reduce`] is a line-for-line port. Offsets are UTF-8 byte offsets on
//! Unicode-scalar boundaries — the TUI's own unit, which the web adapted to
//! UTF-16 (`scalarBoundary`, :236-246): a caret inside a multi-byte scalar
//! clamps back to its start exactly as a DOM caret inside a surrogate pair
//! clamps to the high surrogate.
//!
//! The native input differs from a textarea in one way that matters: a key
//! the reducer does NOT consume still reaches the browser's own editing,
//! while the native composer is read-only in Normal mode (so a plain key can
//! never type text). [`handle_key`] therefore performs the few native
//! actions the web leaves to the browser in Normal mode — Backspace/Delete
//! (`:111-117`, "Arrows/Home/End/Tab/Delete etc. remain native"), the empty
//! draft's `!` (`:166-171`), and paste — so the outcome matches.
//!
//! The approved board draws the subset as a key legend beside the session
//! switcher (mode badges, `Key | Action` rows, "Press ? for more help"). The
//! board's `ciw` / `yy` rows name operations the closed set does not have
//! (`VIM_NORMAL_OPERATIONS`, :45-67), so the legend lists real ones only.
use makepad_widgets::KeyCode;

use super::ui::{self, tok, Btn, Dsl, Face, Frame, Txt, W};

/// Insert or Normal (`VimModeState.mode`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Insert,
    Normal,
}

/// The preference + the composer's mode (`ComposerInput.tsx:65-90`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VimState {
    /// The `vimMode` preference (default off, `preferences/model.ts:88`).
    pub enabled: bool,
    pub mode: Mode,
    /// The pending operator (`"g" | "d" | "c" | null`).
    pub pending: Option<char>,
}

impl VimState {
    /// `resetVimMode` (:31-33): enabling OR disabling returns to Insert.
    pub fn toggled(self) -> Self {
        VimState { enabled: !self.enabled, mode: Mode::Insert, pending: None }
    }
    /// `clearVimPending` (:35-37): on blur / record change an unfinished
    /// operator must never affect another draft.
    pub fn cleared(self) -> Self {
        VimState { pending: None, ..self }
    }
    /// Normal mode is live: the native composer is read-only.
    pub fn normal_active(&self) -> bool {
        self.enabled && self.mode == Mode::Normal
    }
}

/// The closed Normal-mode operation set (`VIM_NORMAL_OPERATIONS`, :45-67).
pub const NORMAL_OPERATIONS: [&str; 21] = [
    "h", "l", "j", "k", "0", "$", "w", "b", "e", "G", "x", "gg", "dd", "dw", "cc", "i", "a", "A", "I", "o", "O",
];

/// One key, the `KeyboardEvent` fields the reducer reads (`VimKey`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Key {
    /// `KeyboardEvent.key`: one character, or a name (`Escape`, `Enter`,
    /// `ArrowLeft`, `Shift`, …).
    pub key: String,
    pub composing: bool,
    pub ctrl: bool,
    pub meta: bool,
    pub alt: bool,
}

impl Key {
    pub fn plain(key: &str) -> Self {
        Key { key: key.to_owned(), composing: false, ctrl: false, meta: false, alt: false }
    }
}

/// `VimEditResult`: the text, the selection, the next mode/pending, and
/// whether the key was consumed (the host suppresses its default).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    pub text: String,
    pub start: usize,
    pub end: usize,
    /// `selectionDirection === "backward"` (the caret is at `start`).
    pub backward: bool,
    pub mode: Mode,
    pub pending: Option<char>,
    pub consumed: bool,
}

/// `reduceVimEdit` (:70-232).
pub fn reduce(st: VimState, text: &str, sel_start: usize, sel_end: usize, backward: bool, key: &Key) -> Edit {
    let first = scalar_boundary(text, sel_start);
    let last = scalar_boundary(text, sel_end);
    let mut r = Edit {
        text: text.to_owned(),
        start: first.min(last),
        end: first.max(last),
        backward,
        mode: st.mode,
        pending: st.pending,
        consumed: false,
    };
    // Clipboard, assistive shortcuts, AltGr and IME own their keys; clearing
    // the operator stops a later plain key from completing it (:84-96).
    if !st.enabled
        || key.composing
        || key.ctrl
        || key.meta
        || key.alt
        || matches!(key.key.as_str(), "Process" | "Dead" | "Unidentified")
    {
        r.pending = None;
        return r;
    }
    if r.mode == Mode::Insert {
        if key.key == "Escape" {
            r.mode = Mode::Normal;
            r.pending = None;
            r.consumed = true;
        }
        return r;
    }
    if key.key == "Enter" {
        r.pending = None;
        return r; // the FIFO/steering dispatch alone owns Enter (:100-103)
    }
    if key.key == "Escape" {
        r.consumed = r.pending.is_some();
        r.pending = None;
        return r; // a second bare Esc may reach the interrupt path (:104-108)
    }
    let mut chars = key.key.chars();
    let (Some(k), None) = (chars.next(), chars.next()) else {
        // Arrows/Home/End/Tab/Delete stay native; never keep an operator
        // across a move the reducer cannot observe (:111-117).
        r.pending = None;
        return r;
    };
    r.consumed = true;
    // Selection is not Visual mode: an operation acts at the active edge.
    let cursor = if r.backward { r.start } else { r.end };
    let start = line_start(text, cursor);
    let end = line_end(text, cursor);
    if let Some(p) = r.pending.take() {
        match (p, k) {
            ('g', 'g') => move_to(&mut r, 0),
            ('d', 'w') => replace(&mut r, text, cursor, word_forward(text, cursor), ""),
            ('d', 'd') => {
                if end < text.len() {
                    replace(&mut r, text, start, end + 1, "");
                } else if start > 0 {
                    replace(&mut r, text, start - 1, end, "");
                } else {
                    replace(&mut r, text, 0, end, "");
                }
            }
            ('c', 'c') => {
                replace(&mut r, text, start, end, "");
                r.mode = Mode::Insert;
            }
            // Unknown two-key sequences are consumed without editing.
            _ => {}
        }
        return r;
    }
    if k == '!' && text.is_empty() {
        // The command resolver still rejects shell execution (:166-171).
        r.mode = Mode::Insert;
        r.consumed = false;
        return r;
    }
    match k {
        'h' => move_to(&mut r, previous_scalar(text, cursor)),
        'l' => move_to(&mut r, next_scalar(text, cursor)),
        'j' | 'k' => {
            let column = text[start..cursor].chars().count();
            if k == 'k' && start > 0 {
                let above = line_start(text, start - 1);
                move_to(&mut r, column_offset(text, above, start - 1, column));
            } else if k == 'j' && end < text.len() {
                let below = end + 1;
                move_to(&mut r, column_offset(text, below, line_end(text, below), column));
            }
        }
        '0' | 'I' => {
            move_to(&mut r, start);
            if k == 'I' {
                r.mode = Mode::Insert;
            }
        }
        '$' | 'A' => {
            move_to(&mut r, end);
            if k == 'A' {
                r.mode = Mode::Insert;
            }
        }
        'w' => move_to(&mut r, word_forward(text, cursor)),
        'b' => move_to(&mut r, word_backward(text, cursor)),
        'e' => move_to(&mut r, word_end(text, cursor)),
        'G' => move_to(&mut r, text.len()),
        'x' => replace(&mut r, text, cursor, next_scalar(text, cursor), ""),
        'g' | 'd' | 'c' => r.pending = Some(k),
        'i' => {
            move_to(&mut r, cursor);
            r.mode = Mode::Insert;
        }
        'a' => {
            move_to(&mut r, next_scalar(text, cursor));
            r.mode = Mode::Insert;
        }
        'o' => {
            replace(&mut r, text, end, end, "\n");
            r.mode = Mode::Insert;
        }
        'O' => {
            replace(&mut r, text, start, start, "\n");
            move_to(&mut r, start);
            r.mode = Mode::Insert;
        }
        // Unknown Normal-mode characters are swallowed, never typed.
        _ => {}
    }
    r
}

fn move_to(r: &mut Edit, position: usize) {
    let p = scalar_boundary(&r.text, position);
    r.start = p;
    r.end = p;
    r.backward = false;
}

fn replace(r: &mut Edit, text: &str, start: usize, end: usize, insert: &str) {
    r.text = format!("{}{insert}{}", &text[..start], &text[end..]);
    move_to(r, start + insert.len());
}

/// `scalarBoundary` (:236-246): clamp into the text and never split a
/// scalar (a UTF-8 continuation byte steps back to its scalar's start).
pub fn scalar_boundary(text: &str, value: usize) -> usize {
    let mut at = value.min(text.len());
    while at > 0 && !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

fn previous_scalar(text: &str, cursor: usize) -> usize {
    scalar_boundary(text, cursor.saturating_sub(1))
}

fn next_scalar(text: &str, cursor: usize) -> usize {
    if cursor >= text.len() {
        return text.len();
    }
    cursor + text[cursor..].chars().next().map_or(1, char::len_utf8)
}

fn line_start(text: &str, cursor: usize) -> usize {
    text[..cursor].rfind('\n').map_or(0, |i| i + 1)
}

fn line_end(text: &str, cursor: usize) -> usize {
    text[cursor..].find('\n').map_or(text.len(), |i| cursor + i)
}

fn column_offset(text: &str, start: usize, end: usize, column: usize) -> usize {
    start + text[start..end].chars().take(column).map(char::len_utf8).sum::<usize>()
}

/// Rust `char::is_whitespace` is Unicode White_Space — the TUI's predicate
/// the web reproduces with `\p{White_Space}` (:270-273).
fn whitespace(text: &str, at: usize) -> bool {
    text[at..].chars().next().is_some_and(char::is_whitespace)
}

fn word_forward(text: &str, cursor: usize) -> usize {
    let mut at = cursor;
    while at < text.len() && !whitespace(text, at) {
        at = next_scalar(text, at);
    }
    while at < text.len() && whitespace(text, at) {
        at = next_scalar(text, at);
    }
    at
}

fn word_backward(text: &str, cursor: usize) -> usize {
    let mut at = cursor;
    while at > 0 && whitespace(text, previous_scalar(text, at)) {
        at = previous_scalar(text, at);
    }
    while at > 0 && !whitespace(text, previous_scalar(text, at)) {
        at = previous_scalar(text, at);
    }
    at
}

fn word_end(text: &str, cursor: usize) -> usize {
    let mut at = next_scalar(text, cursor);
    while at < text.len() && whitespace(text, at) {
        at = next_scalar(text, at);
    }
    if at >= text.len() {
        return text.len();
    }
    while next_scalar(text, at) < text.len() && !whitespace(text, next_scalar(text, at)) {
        at = next_scalar(text, at);
    }
    at
}

/// The `KeyboardEvent.key` a native key press produces (US layout: the
/// shifted digit row gives `!@#$%^&*()`, so `Shift+4` is `$`). Keys with no
/// character get the DOM name; an unknown code is `Unidentified`.
pub fn key_name(code: KeyCode, shift: bool) -> String {
    use KeyCode::*;
    let pick = |plain: char, shifted: char| if shift { shifted } else { plain };
    let letter = |c: char| if shift { c.to_ascii_uppercase() } else { c };
    let ch = match code {
        KeyA => Some(letter('a')),
        KeyB => Some(letter('b')),
        KeyC => Some(letter('c')),
        KeyD => Some(letter('d')),
        KeyE => Some(letter('e')),
        KeyF => Some(letter('f')),
        KeyG => Some(letter('g')),
        KeyH => Some(letter('h')),
        KeyI => Some(letter('i')),
        KeyJ => Some(letter('j')),
        KeyK => Some(letter('k')),
        KeyL => Some(letter('l')),
        KeyM => Some(letter('m')),
        KeyN => Some(letter('n')),
        KeyO => Some(letter('o')),
        KeyP => Some(letter('p')),
        KeyQ => Some(letter('q')),
        KeyR => Some(letter('r')),
        KeyS => Some(letter('s')),
        KeyT => Some(letter('t')),
        KeyU => Some(letter('u')),
        KeyV => Some(letter('v')),
        KeyW => Some(letter('w')),
        KeyX => Some(letter('x')),
        KeyY => Some(letter('y')),
        KeyZ => Some(letter('z')),
        Key0 => Some(pick('0', ')')),
        Key1 => Some(pick('1', '!')),
        Key2 => Some(pick('2', '@')),
        Key3 => Some(pick('3', '#')),
        Key4 => Some(pick('4', '$')),
        Key5 => Some(pick('5', '%')),
        Key6 => Some(pick('6', '^')),
        Key7 => Some(pick('7', '&')),
        Key8 => Some(pick('8', '*')),
        Key9 => Some(pick('9', '(')),
        Backtick => Some(pick('`', '~')),
        Minus => Some(pick('-', '_')),
        Equals => Some(pick('=', '+')),
        LBracket => Some(pick('[', '{')),
        RBracket => Some(pick(']', '}')),
        Backslash => Some(pick('\\', '|')),
        Semicolon => Some(pick(';', ':')),
        Quote => Some(pick('\'', '"')),
        Comma => Some(pick(',', '<')),
        Period => Some(pick('.', '>')),
        Slash => Some(pick('/', '?')),
        Space => Some(' '),
        Numpad0 => Some('0'),
        Numpad1 => Some('1'),
        Numpad2 => Some('2'),
        Numpad3 => Some('3'),
        Numpad4 => Some('4'),
        Numpad5 => Some('5'),
        Numpad6 => Some('6'),
        Numpad7 => Some('7'),
        Numpad8 => Some('8'),
        Numpad9 => Some('9'),
        NumpadDecimal => Some('.'),
        NumpadAdd => Some('+'),
        NumpadSubtract => Some('-'),
        NumpadMultiply => Some('*'),
        NumpadDivide => Some('/'),
        NumpadEquals => Some('='),
        _ => None,
    };
    if let Some(c) = ch {
        return c.to_string();
    }
    match code {
        Escape => "Escape",
        ReturnKey | NumpadEnter => "Enter",
        Backspace => "Backspace",
        Delete => "Delete",
        Tab => "Tab",
        ArrowUp => "ArrowUp",
        ArrowDown => "ArrowDown",
        ArrowLeft => "ArrowLeft",
        ArrowRight => "ArrowRight",
        Home => "Home",
        End => "End",
        PageUp => "PageUp",
        PageDown => "PageDown",
        Insert => "Insert",
        Shift => "Shift",
        Control => "Control",
        Alt => "Alt",
        Logo => "Meta",
        Capslock => "CapsLock",
        F1 | F2 | F3 | F4 | F5 | F6 | F7 | F8 | F9 | F10 | F11 | F12 => "F",
        _ => "Unidentified",
    }
    .to_owned()
}

/// What the host does with one key (see the module note on native actions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyOutcome {
    /// Suppress the host's own handling of this key (`preventDefault`).
    pub consumed: bool,
    /// Text + caret (byte offset) to write into the composer.
    pub write: Option<(String, usize)>,
    /// `?` in Normal mode: show the key legend.
    pub help: bool,
    /// The next mode state.
    pub state: VimState,
}

/// One key against the composer's text and selection (`anchor`/`cursor`
/// are the native selection's byte offsets; the caret is `cursor`).
pub fn handle_key(st: VimState, text: &str, anchor: usize, cursor: usize, key: &Key) -> KeyOutcome {
    let backward = cursor < anchor;
    let r = reduce(st, text, anchor.min(cursor), anchor.max(cursor), backward, key);
    let state = VimState { enabled: st.enabled, mode: r.mode, pending: r.pending };
    let mut out = KeyOutcome { consumed: r.consumed, write: None, help: false, state };
    if r.consumed {
        let caret = if r.backward { r.start } else { r.end };
        out.help = st.mode == Mode::Normal && st.pending.is_none() && key.key == "?";
        out.write = Some((r.text, caret));
        return out;
    }
    if !st.normal_active() {
        return out;
    }
    let plain = !(key.ctrl || key.meta || key.alt || key.composing);
    match key.key.as_str() {
        // `!` on an empty draft: Insert, and the textarea types the `!`.
        "!" if r.mode == Mode::Insert && plain => out.write = Some(("!".into(), 1)),
        // Backspace/Delete stay native in Normal mode (:111-117).
        "Backspace" | "Delete" if plain => {
            let (s, e) = (scalar_boundary(text, anchor.min(cursor)), scalar_boundary(text, anchor.max(cursor)));
            out.write = if s != e {
                Some((format!("{}{}", &text[..s], &text[e..]), s))
            } else if key.key == "Backspace" && s > 0 {
                let p = previous_scalar(text, s);
                Some((format!("{}{}", &text[..p], &text[s..]), p))
            } else if key.key == "Delete" && s < text.len() {
                let n = next_scalar(text, s);
                Some((format!("{}{}", &text[..s], &text[n..]), s))
            } else {
                None
            };
        }
        _ => {}
    }
    out
}

/// A paste in Normal mode: the browser pastes into the textarea whatever
/// the mode (Cmd/Ctrl+V is never consumed, :84-96); the read-only native
/// input does not, so the host inserts it at the selection.
pub fn paste(text: &str, anchor: usize, cursor: usize, pasted: &str) -> (String, usize) {
    let (s, e) = (scalar_boundary(text, anchor.min(cursor)), scalar_boundary(text, anchor.max(cursor)));
    (format!("{}{pasted}{}", &text[..s], &text[e..]), s + pasted.len())
}

/// The composer's field note (`ComposerInput.tsx:270-274`).
pub fn note(st: &VimState) -> &'static str {
    match st.mode {
        Mode::Insert => "Vim · Insert",
        Mode::Normal => "Vim · Normal",
    }
}

// ---- the board's legend -------------------------------------------------

/// The board's six legend rows, restricted to real operations.
pub const LEGEND: [(&str, &str); 6] = [
    ("Enter", "Send the prompt"),
    ("Escape", "Normal mode / cancel"),
    ("w", "Next word"),
    ("dd", "Delete line"),
    ("cc", "Change line"),
    ("x", "Delete character"),
];

/// The full key table the `?` legend shows: every operation of the closed
/// set (`NORMAL_OPERATIONS`), grouped.
pub const HELP: [(&str, &[(&str, &str)]); 3] = [
    (
        "Modes",
        &[
            ("Escape", "Normal mode; cancels a pending g, d or c"),
            ("i", "Insert before the caret"),
            ("a", "Insert after the caret"),
            ("I", "Insert at the line start"),
            ("A", "Insert at the line end"),
            ("o", "Open a line below"),
            ("O", "Open a line above"),
        ],
    ),
    (
        "Motions",
        &[
            ("h  l", "Left / right"),
            ("j  k", "Down / up a line"),
            ("0  $", "Line start / end"),
            ("w  b", "Next / previous word"),
            ("e", "End of the word"),
            ("gg  G", "Start / end of the draft"),
        ],
    ),
    (
        "Edits",
        &[
            ("x", "Delete character"),
            ("dw", "Delete to the next word"),
            ("dd", "Delete line"),
            ("cc", "Change line"),
        ],
    ),
];

fn badge(d: &mut Dsl, id: &str, label: &str, live: bool) {
    let w = ui::text_w(label, 12.0, Face::Medium) + 22.0;
    d.surface(
        &format!("{id}_box"),
        &format!("width: {} height: 30 flow: Right align: Align{{x: 0.5 y: 0.5}}", w.ceil()),
        tok::SURFACE2,
        8.0,
        Some("#d9d9dcff"),
    );
    d.text(id, label, &Txt::new(12.0, Face::Medium, if live { tok::TEXT } else { tok::FAINT }));
    d.close();
}

fn badges(d: &mut Dsl, st: &VimState) {
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right{wrap: true} align: Align{x: 0.0 y: 0.5} spacing: 8");
    let mode = if !st.enabled {
        "OFF"
    } else if st.mode == Mode::Normal {
        "NORMAL"
    } else {
        "INSERT"
    };
    badge(d, "b3_vim_mode", mode, st.enabled);
    match st.pending {
        Some(p) => badge(d, "b3_vim_pending", &format!("PENDING  {p}"), true),
        None => badge(d, "b3_vim_pending", "PENDING  —", false),
    }
    d.close();
}

fn key_row(d: &mut Dsl, id: &str, key: &str, action: &str, key_w: f64) {
    let row = d.anon();
    d.view(&row, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5} padding: Inset{top: 7 bottom: 7}");
    d.text(&format!("{id}_key"), key, &Txt::new(13.0, Face::Mono, tok::TEXT).w(W::Px(key_w)));
    d.text(&format!("{id}_action"), action, &Txt::new(13.0, Face::Regular, tok::MUTED).w(W::Fill).wrap());
    d.close();
}

/// The board's legend column: mode badges, `Key | Action`, the help line.
pub fn legend(d: &mut Dsl, st: &VimState, width: W) {
    d.surface(
        "b3_vim_legend",
        &format!(
            "width: {} height: Fit flow: Down spacing: 10 padding: Inset{{left: 14 right: 14 top: 14 bottom: 14}}",
            match width {
                W::Fill => "Fill".to_owned(),
                W::Fit => "Fit".to_owned(),
                W::Px(v) => format!("{v}"),
            }
        ),
        tok::SURFACE2,
        12.0,
        Some(tok::HAIRLINE),
    );
    badges(d, st);
    d.gap(W::Fill, 4.0);
    let head = d.anon();
    d.view(&head, "width: Fill height: Fit flow: Right align: Align{x: 0.0 y: 0.5}");
    d.text("b3_vim_head_key", "Key", &Txt::new(13.0, Face::Medium, tok::TEXT).w(W::Px(72.0)));
    d.text("b3_vim_head_action", "Action", &Txt::new(13.0, Face::Medium, tok::TEXT).w(W::Fill));
    d.close();
    d.hairline();
    let rows = d.anon();
    d.view(&rows, "width: Fill height: Fit flow: Down");
    for (i, (k, a)) in LEGEND.iter().enumerate() {
        key_row(d, &format!("b3_vim_row_{i}"), k, a, 72.0);
    }
    d.close();
    d.gap(W::Fill, 6.0);
    let help = "Press ? for more help";
    d.view(
        "b3_vim_help_box",
        &format!("width: {} height: 22 flow: Overlay align: Align{{x: 0.0 y: 0.5}}", ui::text_w(help, 13.0, Face::Regular) + 4.0),
    );
    d.text("b3_vim_help_label", help, &Txt::new(13.0, Face::Regular, tok::MUTED));
    d.tap("b3_vim_help", "b3.vim.help");
    d.close();
    d.close();
}

/// `?` in Normal mode (or the legend's help line): the full key table.
pub fn build_help(d: &mut Dsl, st: &VimState, frame: &Frame) {
    let width = frame.dialog_w(520.0);
    let compact = frame.compact(width);
    ui::shell_open(d, frame, width);
    ui::header(d, "Vim keys", "b3.close");
    d.text(
        "b3_vim_scope",
        "Composer editing · Normal mode keys",
        &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill),
    );
    d.gap(W::Fill, 12.0);
    ui::body_open(d, frame, width, 140.0);
    badges(d, st);
    d.gap(W::Fill, 12.0);
    let key_w = if compact { 64.0 } else { 88.0 };
    for (g, (title, rows)) in HELP.iter().enumerate() {
        ui::card_open(d, &format!("b3_vim_group_{g}"), 0.0);
        ui::section_title(d, &format!("b3_vim_group_{g}_title"), title);
        d.gap(W::Fill, 4.0);
        for (i, (k, a)) in rows.iter().enumerate() {
            if i > 0 {
                d.hairline();
            }
            key_row(d, &format!("b3_vim_g{g}_{i}"), k, a, key_w);
        }
        d.close();
        d.gap(W::Fill, 10.0);
    }
    d.text(
        "b3_vim_note",
        "Enter sends the prompt in both modes. Any other key does nothing in Normal mode — it never types text. A second Escape still stops a running turn.",
        &Txt::new(12.0, Face::Regular, tok::MUTED).w(W::Fill).wrap(),
    );
    ui::body_close(d);
    d.gap(W::Fill, 12.0);
    let foot = d.anon();
    d.view(&foot, "width: Fill height: Fit flow: Right align: Align{x: 1.0 y: 0.5} spacing: 8");
    d.button("b3_vim_off", "Turn off Vim editing", "b3.vim.off", Btn::Outline, W::Fit, 36.0);
    d.button("b3_vim_done", "Done", "b3.close", Btn::Primary, W::Fit, 36.0);
    d.close();
    ui::shell_close(d);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn normal() -> VimState {
        VimState { enabled: true, mode: Mode::Normal, pending: None }
    }

    /// Run keys from a caret; return (text, caret, state).
    fn run(text: &str, caret: usize, st: VimState, keys: &[&str]) -> (String, usize, VimState) {
        let (mut t, mut c, mut s) = (text.to_owned(), caret, st);
        for k in keys {
            let out = handle_key(s, &t, c, c, &Key::plain(k));
            if let Some((nt, nc)) = out.write {
                t = nt;
                c = nc;
            }
            s = out.state;
        }
        (t, c, s)
    }

    #[test]
    fn the_closed_set_is_the_webs_21_operations() {
        assert_eq!(NORMAL_OPERATIONS.len(), 21);
        // Every legend key is a real operation (or Enter/Escape).
        for (k, _) in LEGEND {
            assert!(NORMAL_OPERATIONS.contains(&k) || k == "Enter" || k == "Escape", "{k}");
        }
        for (_, rows) in HELP {
            for (k, _) in rows {
                for part in k.split_whitespace() {
                    assert!(NORMAL_OPERATIONS.contains(&part) || part == "Escape", "{part}");
                }
            }
        }
        let listed: usize = HELP.iter().map(|(_, r)| r.iter().map(|(k, _)| k.split_whitespace().filter(|p| *p != "Escape").count()).sum::<usize>()).sum();
        assert_eq!(listed, 21, "the help table lists every operation once");
    }

    #[test]
    fn escape_enters_normal_and_only_a_pending_escape_is_consumed() {
        let ins = VimState { enabled: true, ..Default::default() };
        let r = reduce(ins, "abc", 3, 3, false, &Key::plain("Escape"));
        assert!(r.consumed && r.mode == Mode::Normal, "Insert Esc -> Normal, consumed");
        // Normal + no pending: NOT consumed, so the interrupt path still sees it.
        let r = reduce(normal(), "abc", 1, 1, false, &Key::plain("Escape"));
        assert!(!r.consumed);
        // Normal + pending: consumed, operator cleared.
        let r = reduce(VimState { pending: Some('d'), ..normal() }, "abc", 1, 1, false, &Key::plain("Escape"));
        assert!(r.consumed && r.pending.is_none());
        // Enter is never consumed and clears the operator.
        let r = reduce(VimState { pending: Some('g'), ..normal() }, "abc", 1, 1, false, &Key::plain("Enter"));
        assert!(!r.consumed && r.pending.is_none());
    }

    #[test]
    fn modifiers_ime_and_named_keys_are_never_consumed_and_clear_the_operator() {
        let st = VimState { pending: Some('d'), ..normal() };
        for key in [
            Key { ctrl: true, ..Key::plain("d") },
            Key { meta: true, ..Key::plain("v") },
            Key { alt: true, ..Key::plain("d") },
            Key { composing: true, ..Key::plain("d") },
            Key::plain("Dead"),
            Key::plain("Process"),
            Key::plain("ArrowLeft"),
            Key::plain("Shift"),
        ] {
            let r = reduce(st, "one two", 0, 0, false, &key);
            assert!(!r.consumed, "{key:?}");
            assert_eq!(r.pending, None, "{key:?}");
            assert_eq!(r.text, "one two");
        }
        // Disabled: nothing.
        let r = reduce(VimState::default(), "x", 0, 0, false, &Key::plain("x"));
        assert!(!r.consumed && r.text == "x");
    }

    #[test]
    fn motions_match_the_reducer() {
        let t = "one two three\nfour five";
        assert_eq!(run(t, 0, normal(), &["w"]).1, 4);
        assert_eq!(run(t, 0, normal(), &["w", "w"]).1, 8);
        assert_eq!(run(t, 4, normal(), &["b"]).1, 0);
        assert_eq!(run(t, 0, normal(), &["e"]).1, 2);
        assert_eq!(run(t, 5, normal(), &["$"]).1, 13);
        assert_eq!(run(t, 5, normal(), &["0"]).1, 0);
        assert_eq!(run(t, 5, normal(), &["G"]).1, t.len());
        assert_eq!(run(t, 9, normal(), &["g", "g"]).1, 0);
        // j keeps the column; k goes back up.
        assert_eq!(run(t, 2, normal(), &["j"]).1, 16);
        assert_eq!(run(t, 16, normal(), &["k"]).1, 2);
        // j past a short line clamps to its end.
        assert_eq!(run(t, 12, normal(), &["j"]).1, t.len());
        assert_eq!(run(t, 3, normal(), &["h"]).1, 2);
        assert_eq!(run(t, 3, normal(), &["l"]).1, 4);
        // Unknown keys are swallowed without editing.
        let (text, caret, st) = run(t, 3, normal(), &["z", "q", " "]);
        assert_eq!((text.as_str(), caret, st.mode), (t, 3, Mode::Normal));
    }

    #[test]
    fn edits_and_insert_entries_match_the_reducer() {
        // x
        assert_eq!(run("abc", 1, normal(), &["x"]).0, "ac");
        // dd on a middle, the last, and the only line
        assert_eq!(run("a\nb\nc", 2, normal(), &["d", "d"]).0, "a\nc");
        assert_eq!(run("a\nb\nc", 4, normal(), &["d", "d"]).0, "a\nb");
        assert_eq!(run("only", 2, normal(), &["d", "d"]).0, "");
        // dw
        assert_eq!(run("one two", 0, normal(), &["d", "w"]).0, "two");
        // cc clears the line and enters Insert
        let (t, c, s) = run("a\nbcd\ne", 3, normal(), &["c", "c"]);
        assert_eq!((t.as_str(), c, s.mode), ("a\n\ne", 2, Mode::Insert));
        // unknown pair: consumed, nothing edited
        let out = handle_key(VimState { pending: Some('d'), ..normal() }, "abc", 1, 1, &Key::plain("z"));
        assert!(out.consumed && out.write == Some(("abc".into(), 1)) && out.state.pending.is_none());
        // i / a / I / A / o / O
        assert_eq!(run("abc", 1, normal(), &["i"]).1, 1);
        assert_eq!(run("abc", 1, normal(), &["a"]).1, 2);
        assert_eq!(run("  abc", 4, normal(), &["I"]).1, 0);
        assert_eq!(run("abc\nd", 1, normal(), &["A"]).1, 3);
        let (t, c, s) = run("abc\nd", 1, normal(), &["o"]);
        assert_eq!((t.as_str(), c, s.mode), ("abc\n\nd", 4, Mode::Insert));
        let (t, c, _) = run("abc\nd", 5, normal(), &["O"]);
        assert_eq!((t.as_str(), c), ("abc\n\nd", 4));
    }

    #[test]
    fn carets_clamp_to_scalar_boundaries() {
        // "a😀b": 😀 is 4 bytes at 1..5. A caret inside it clamps to 1.
        let t = "a😀b";
        assert_eq!(scalar_boundary(t, 3), 1);
        assert_eq!(scalar_boundary(t, 99), t.len());
        let r = reduce(normal(), t, 3, 3, false, &Key::plain("l"));
        assert_eq!(r.start, 5, "l from the clamped caret steps over the whole scalar");
        let r = reduce(normal(), t, 5, 5, false, &Key::plain("h"));
        assert_eq!(r.start, 1);
        let r = reduce(normal(), t, 1, 1, false, &Key::plain("x"));
        assert_eq!(r.text, "ab", "x deletes the whole scalar");
        // j keeps a column counted in scalars.
        let r = reduce(normal(), "😀😀x\nabcd", 8, 8, false, &Key::plain("j"));
        assert_eq!(r.start, 12, "column 2 of the next line");
    }

    #[test]
    fn a_selection_acts_at_its_active_edge_never_deleting_the_range() {
        // backward selection: caret at start (anchor 4, cursor 1)
        let out = handle_key(normal(), "abcdef", 4, 1, &Key::plain("x"));
        assert_eq!(out.write, Some(("acdef".into(), 1)));
        // forward selection: caret at end
        let out = handle_key(normal(), "abcdef", 1, 4, &Key::plain("x"));
        assert_eq!(out.write, Some(("abcdf".into(), 4)));
    }

    #[test]
    fn native_actions_the_read_only_input_cannot_do_are_performed() {
        // `!` on an empty draft: Insert + the `!` typed.
        let out = handle_key(normal(), "", 0, 0, &Key::plain("!"));
        assert!(!out.consumed);
        assert_eq!(out.state.mode, Mode::Insert);
        assert_eq!(out.write, Some(("!".into(), 1)));
        // `!` on a non-empty draft is just swallowed.
        let out = handle_key(normal(), "x", 0, 0, &Key::plain("!"));
        assert!(out.consumed && out.write == Some(("x".into(), 0)));
        // Backspace / Delete stay native in Normal mode.
        let out = handle_key(normal(), "abc", 2, 2, &Key::plain("Backspace"));
        assert_eq!((out.consumed, out.write), (false, Some(("ac".into(), 1))));
        let out = handle_key(normal(), "abc", 1, 1, &Key::plain("Delete"));
        assert_eq!(out.write, Some(("ac".into(), 1)));
        let out = handle_key(normal(), "abcd", 1, 3, &Key::plain("Backspace"));
        assert_eq!(out.write, Some(("ad".into(), 1)));
        // In Insert the input itself edits: nothing to do here.
        let out = handle_key(VimState { enabled: true, ..Default::default() }, "abc", 2, 2, &Key::plain("Backspace"));
        assert_eq!(out.write, None);
        // Paste lands at the selection.
        assert_eq!(paste("abcd", 1, 3, "XY"), ("aXYd".into(), 3));
    }

    #[test]
    fn question_mark_opens_the_legend_only_from_idle_normal_mode() {
        assert!(handle_key(normal(), "x", 0, 0, &Key::plain("?")).help);
        assert!(!handle_key(VimState { pending: Some('d'), ..normal() }, "x", 0, 0, &Key::plain("?")).help);
        assert!(!handle_key(VimState { enabled: true, ..Default::default() }, "x", 0, 0, &Key::plain("?")).help);
    }

    #[test]
    fn toggling_resets_to_insert_and_native_keys_map_like_a_us_keyboard() {
        let st = VimState { enabled: true, mode: Mode::Normal, pending: Some('d') };
        assert_eq!(st.toggled(), VimState { enabled: false, mode: Mode::Insert, pending: None });
        assert_eq!(VimState::default().toggled(), VimState { enabled: true, mode: Mode::Insert, pending: None });
        assert_eq!(key_name(KeyCode::KeyG, true), "G");
        assert_eq!(key_name(KeyCode::KeyG, false), "g");
        assert_eq!(key_name(KeyCode::Key4, true), "$");
        assert_eq!(key_name(KeyCode::Key0, false), "0");
        assert_eq!(key_name(KeyCode::Slash, true), "?");
        assert_eq!(key_name(KeyCode::Key1, true), "!");
        assert_eq!(key_name(KeyCode::ReturnKey, false), "Enter");
        assert_eq!(key_name(KeyCode::Escape, false), "Escape");
        assert_eq!(key_name(KeyCode::Shift, true), "Shift");
        assert_eq!(key_name(KeyCode::Unknown, false), "Unidentified");
        assert_eq!(note(&st), "Vim · Normal");
    }

    #[test]
    fn the_legend_and_help_publish_their_taps_on_the_shared_path() {
        let mut d = Dsl::new();
        legend(&mut d, &normal(), W::Px(250.0));
        let dsl = d.finish();
        assert!(dsl.contains("NORMAL") && dsl.contains("Press ? for more help"));
        assert!(!dsl.contains("ciw") && !dsl.contains("yy"), "no operation the closed set lacks");
        assert_eq!(crate::screens::taps::wired_taps(&dsl), vec![("b3_vim_help".to_owned(), "b3.vim.help".to_owned())]);
        let mut d = Dsl::new();
        build_help(&mut d, &VimState { pending: Some('g'), ..normal() }, &Frame::DESKTOP);
        let dsl = d.finish();
        assert!(dsl.contains("PENDING  g"));
        let taps = crate::screens::taps::wired_taps(&dsl);
        assert!(taps.contains(&("b3_vim_off".to_owned(), "b3.vim.off".to_owned())));
        assert!(taps.contains(&("b3_close".to_owned(), "b3.close".to_owned())));
    }
}
