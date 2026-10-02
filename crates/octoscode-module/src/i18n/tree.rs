//! A24 — the static shell's copy in the current language.
//!
//! The shell's `script_mod!` DSL (chrome.rs, lib.rs) is evaluated once at
//! startup, so its literals (`OcRowTitle{text: "Theme"}`) cannot call
//! [`super::tr`] themselves and still follow a live switch. This pass gives
//! them the web's behaviour: on the first sync and on every language switch
//! it walks the shell, remembers each static label's ENGLISH source the first
//! time it sees it, and sets `tr(source)` — so switching back restores the
//! English exactly, and a key the catalog lacks stays English.
//!
//! Which widgets are static copy:
//! - an ANONYMOUS label or button (`Label{text: …}` with no `name :=`):
//!   makepad numbers anonymous children `LiveId(0..n)` (view.rs
//!   `on_after_apply`), and code can only address NAMED widgets, so an
//!   anonymous label's text can only be the DSL's own literal;
//! - a named label the caller lists as static (`hd_tab_chat_on`, …);
//! - every `TextInput`'s placeholder (`empty_text`) — the field's TEXT is
//!   the person's and is never touched.
//!
//! The walk never enters a `Splash` (every lowered surface: its DSL already
//! carries `tr()` copy and re-lowers on a switch), a list (`PortalList`,
//! `FlatList`: rows are drawn from data each frame, their static rows call
//! `tr()` where they are drawn) or rich text (`Markdown`, `TextFlow`,
//! `Html`: model and user prose is never translated).
use std::collections::HashMap;

use makepad_widgets::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Placeholder,
}

/// The per-view memory of the shell's English sources.
#[derive(Default)]
pub struct StaticTexts {
    /// widget uid -> (what is localized, its English source).
    sources: HashMap<u64, (Kind, String)>,
    /// The language generation last applied (`None`: never).
    applied: Option<u64>,
}

/// Whether a child id is makepad's numbered id for an anonymous child.
pub fn is_anonymous(id: LiveId) -> bool {
    id.0 < (1 << 24)
}

fn skipped(w: &WidgetRef) -> bool {
    w.borrow::<Splash>().is_some()
        || w.borrow::<PortalList>().is_some()
        || w.borrow::<FlatList>().is_some()
        || w.borrow::<Markdown>().is_some()
        || w.borrow::<TextFlow>().is_some()
        || w.borrow::<Html>().is_some()
}

impl StaticTexts {
    /// Re-text the static copy under `root` when the language changed since
    /// the last pass (or on the first). `named` lists the NAMED labels whose
    /// DSL text is static copy. Returns how many texts were set.
    pub fn sync(&mut self, cx: &mut Cx, root: &dyn Widget, named: &[LiveId]) -> usize {
        let gen = super::generation();
        if self.applied == Some(gen) {
            return 0;
        }
        self.applied = Some(gen);
        let mut kids = Vec::new();
        root.children(&mut |id, child| kids.push((id, child)));
        let mut n = 0;
        for (id, child) in kids {
            n += self.visit(cx, id, &child, named);
        }
        n
    }

    /// The English sources recorded so far (for tests and the walk log).
    pub fn sources(&self) -> usize {
        self.sources.len()
    }

    fn visit(&mut self, cx: &mut Cx, id: LiveId, w: &WidgetRef, named: &[LiveId]) -> usize {
        if skipped(w) {
            return 0;
        }
        let mut n = 0;
        let uid = w.widget_uid().0;
        if let Some(mut input) = w.borrow_mut::<TextInput>() {
            let entry = self
                .sources
                .entry(uid)
                .or_insert_with(|| (Kind::Placeholder, input.empty_text().to_owned()));
            if entry.0 == Kind::Placeholder && !entry.1.is_empty() {
                let next = super::tr(&entry.1).to_owned();
                if input.empty_text() != next {
                    input.set_empty_text(cx, next);
                    n += 1;
                }
            }
            return n; // a field's own text is the person's: never descend
        }
        let text_widget = w.borrow::<Label>().is_some() || w.borrow::<Button>().is_some();
        if text_widget && (is_anonymous(id) || named.contains(&id)) {
            let current = w.text();
            let entry = self.sources.entry(uid).or_insert_with(|| (Kind::Text, current.clone()));
            if entry.0 == Kind::Text && !entry.1.is_empty() {
                let next = super::tr(&entry.1);
                if current != next {
                    let next = next.to_owned();
                    w.set_text(cx, &next);
                    n += 1;
                }
            }
        }
        let mut kids = Vec::new();
        w.children(&mut |cid, child| kids.push((cid, child)));
        for (cid, child) in kids {
            n += self.visit(cx, cid, &child, named);
        }
        n
    }
}
