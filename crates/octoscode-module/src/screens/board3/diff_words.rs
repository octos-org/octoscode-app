//! A28 — parity row 23: the diff review's word-level change marks and
//! per-file syntax colours, bounded so a large preview falls back to plain
//! text. The web's `features/review/diff-presentation.ts:19-205`, natively:
//!
//! * [`can_decorate`] — `canDecorateDiff` (:26-40): over 400 lines, 40,000
//!   characters or any line over 2,000 characters (UTF-16 code units, the
//!   web's `.length`), NOTHING is decorated (all or nothing, over the whole
//!   preview); the content itself is never truncated (the dialog shows every
//!   line plain, under the note `PLAIN_NOTE`).
//! * [`diff_language`] — `diffLanguage` (:42-46): the file name's extension
//!   (the shell dot files read as `sh`), resolved by the highlighter's alias
//!   table ([`crate::highlight::grammar`], the web's `highlight.ts:39-68`); a
//!   file without a grammar (`.conf`, `.lock`) stays plain text but keeps its
//!   word marks (e2e `diff-review.spec.ts:188`).
//! * [`decorate_hunk`] — `decorateDiffHunk` (:48-116): each side (context +
//!   removed, context + added) is lexed on its own, so a construct opened in
//!   a context line colours both sides; the added pass wins on context lines;
//!   a side holding an embedded line break is left plain (shiki splits it and
//!   the web drops that side's syntax). Word marks only inside a change block
//!   with as many removed lines as added lines, pairing lines by position.
//! * [`changed_words`] — `changedWords` (:118-175): Unicode words
//!   (`[\p{L}\p{N}\p{M}_$]+`, whitespace runs, single other code points), a
//!   word LCS of at most 160 tokens per side, and a pair is marked only when
//!   it shares at least 25 % of its (trimmed) characters.
//! * `annotateTokens` (:177-205) — the syntax tokens cut at the changed
//!   ranges, each piece flagged.
use std::ops::Range;
use std::sync::OnceLock;

use octos_core::ui_protocol::{DiffPreviewFile, DiffPreviewLine, DiffPreviewLineKind};

use crate::highlight::{self, Carry, Tok};

/// `canDecorateDiff`: more lines than this and nothing is decorated.
pub const MAX_LINES: usize = 400;
/// … more characters (UTF-16 code units) in all lines together.
pub const MAX_CHARS: usize = 40_000;
/// … one line longer than this.
pub const MAX_LINE_CHARS: usize = 2_000;
/// `changedWords`: a side with more words than this is not compared.
pub const MAX_WORDS: usize = 160;
/// `changedWords`: a pair sharing less than this keeps its line tint only.
pub const MIN_SHARED: f64 = 0.25;

/// One decorated piece of a line (`DiffToken`, :10-12): its text, its syntax
/// class (`None` = the file has no grammar, or this side was left plain),
/// and whether it is a changed word.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffToken {
    pub text: String,
    pub tok: Option<Tok>,
    pub changed: bool,
}

/// Every file's hunks' lines' tokens: `[file][hunk][line]`.
pub type Decorations = Vec<Vec<Vec<Vec<DiffToken>>>>;

/// The web's `.length`: UTF-16 code units.
pub fn js_len(s: &str) -> usize {
    s.chars().map(char::len_utf16).sum()
}

/// `canDecorateDiff` (:26-40), counting exactly as the web does.
pub fn can_decorate(files: &[DiffPreviewFile]) -> bool {
    let (mut lines, mut chars) = (0usize, 0usize);
    for file in files {
        for hunk in &file.hunks {
            for line in &hunk.lines {
                let len = js_len(&line.content);
                lines += 1;
                chars += len;
                if lines > MAX_LINES || chars > MAX_CHARS || len > MAX_LINE_CHARS {
                    return false;
                }
            }
        }
    }
    true
}

/// `diffLanguage` (:42-46): the last path segment's extension, lower case;
/// `.bashrc` / `.zshrc` / `.bash_profile` read as `sh`; no dot, no language.
pub fn diff_language(path: &str) -> Option<String> {
    let name = path.rsplit(['/', '\\']).next().unwrap_or("").to_lowercase();
    if [".bashrc", ".zshrc", ".bash_profile"].contains(&name.as_str()) {
        return Some("sh".into());
    }
    name.contains('.').then(|| name.rsplit('.').next().unwrap_or("").to_owned())
}

/// JavaScript's `\s` (and what `String.prototype.trim` strips): WhiteSpace
/// plus LineTerminator. Rust's `char::is_whitespace` differs (U+0085 in,
/// U+FEFF out), so the web's set is spelled out.
const JS_SPACE: &str = r"\t\n\x0B\x0C\r \x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}\x{FEFF}";

fn js_space(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}'
        | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

/// `String.prototype.trim`.
fn js_trim(s: &str) -> &str {
    s.trim_matches(js_space)
}

/// The web's word pattern (`/[\p{L}\p{N}\p{M}_$]+|\s+|[^\p{L}\p{N}\p{M}_$\s]/gu`).
fn words_re() -> &'static regex::Regex {
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    RE.get_or_init(|| {
        regex::Regex::new(&format!(r"[\p{{L}}\p{{N}}\p{{M}}_$]+|[{JS_SPACE}]+|[^\p{{L}}\p{{N}}\p{{M}}_${JS_SPACE}]"))
            .expect("the word pattern compiles")
    })
}

/// `changedWords` (:118-175): the changed byte ranges of each side of a pair
/// (adjacent changed words merged), or `None` when the sides are not
/// compared (empty, over [`MAX_WORDS`]) or share less than [`MIN_SHARED`].
pub fn changed_words(before: &str, after: &str) -> Option<(Vec<Range<usize>>, Vec<Range<usize>>)> {
    let left: Vec<&str> = words_re().find_iter(before).map(|m| m.as_str()).collect();
    let right: Vec<&str> = words_re().find_iter(after).map(|m| m.as_str()).collect();
    if left.is_empty() || right.is_empty() || left.len() > MAX_WORDS || right.len() > MAX_WORDS {
        return None;
    }
    // The LCS table, filled from the end (the web's Uint16Array).
    let width = right.len() + 1;
    let mut m = vec![0u16; (left.len() + 1) * width];
    for i in (0..left.len()).rev() {
        for j in (0..right.len()).rev() {
            m[i * width + j] = if left[i] == right[j] {
                1 + m[(i + 1) * width + j + 1]
            } else {
                m[(i + 1) * width + j].max(m[i * width + j + 1])
            };
        }
    }
    let mut kept_left = vec![false; left.len()];
    let mut kept_right = vec![false; right.len()];
    let (mut i, mut j, mut shared) = (0usize, 0usize, 0usize);
    while i < left.len() && j < right.len() {
        if left[i] == right[j] {
            kept_left[i] = true;
            kept_right[j] = true;
            shared += js_len(js_trim(left[i]));
            i += 1;
            j += 1;
        } else if m[(i + 1) * width + j] >= m[i * width + j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    let denom = js_len(js_trim(before)).max(js_len(js_trim(after))).max(1);
    if (shared as f64) / (denom as f64) < MIN_SHARED {
        return None;
    }
    let changed = |tokens: &[&str], kept: &[bool]| {
        let mut out: Vec<Range<usize>> = Vec::new();
        let mut offset = 0usize;
        for (k, t) in tokens.iter().enumerate() {
            let end = offset + t.len();
            if !kept[k] {
                match out.last_mut() {
                    Some(last) if last.end == offset => last.end = end,
                    _ => out.push(offset..end),
                }
            }
            offset = end;
        }
        out
    };
    Some((changed(&left, &kept_left), changed(&right, &kept_right)))
}

/// `annotateTokens` (:177-205): cut each token at the range edges inside it;
/// a piece is changed when one range covers it.
fn annotate(tokens: Vec<(Option<Tok>, String)>, ranges: &[Range<usize>]) -> Vec<DiffToken> {
    let mut out = Vec::new();
    let mut offset = 0usize;
    for (tok, text) in tokens {
        let end = offset + text.len();
        let mut cuts = vec![offset, end];
        for r in ranges {
            if r.start > offset && r.start < end {
                cuts.push(r.start);
            }
            if r.end > offset && r.end < end {
                cuts.push(r.end);
            }
        }
        cuts.sort_unstable();
        for w in cuts.windows(2) {
            let (start, stop) = (w[0], w[1]);
            out.push(DiffToken {
                text: text[start - offset..stop - offset].to_owned(),
                tok,
                changed: ranges.iter().any(|r| r.start <= start && r.end >= stop),
            });
        }
        offset = end;
    }
    out
}

/// `decorateDiffHunk` (:48-116): every line's syntax tokens with its changed
/// words flagged. `language` is [`diff_language`]'s answer for the file.
pub fn decorate_hunk(lines: &[DiffPreviewLine], language: Option<&str>) -> Vec<Vec<DiffToken>> {
    let mut syntax: Vec<Option<Vec<(Tok, String)>>> = vec![None; lines.len()];
    // Hunks can begin midway through a lexical construct: each side is lexed
    // with its context only, and the original text stays authoritative.
    if let Some(g) = highlight::grammar(language) {
        for side in [DiffPreviewLineKind::Removed, DiffPreviewLineKind::Added] {
            let other = if side == DiffPreviewLineKind::Removed { DiffPreviewLineKind::Added } else { DiffPreviewLineKind::Removed };
            let indexes: Vec<usize> = (0..lines.len()).filter(|&i| lines[i].kind != other).collect();
            // shiki splits the joined side at its line breaks: a line that
            // carries one breaks the count and the web leaves the side plain.
            if indexes.is_empty() || indexes.iter().any(|&i| lines[i].content.contains('\n')) {
                continue;
            }
            let mut carry = Carry::None;
            for &i in &indexes {
                let (spans, next) = highlight::line(g, &lines[i].content, carry);
                carry = next;
                if spans.iter().map(|(_, s)| s.as_str()).collect::<String>() == lines[i].content {
                    syntax[i] = Some(spans);
                }
            }
        }
    }
    let mut ranges: Vec<Vec<Range<usize>>> = vec![Vec::new(); lines.len()];
    let mut start = 0usize;
    while start < lines.len() {
        if lines[start].kind == DiffPreviewLineKind::Context {
            start += 1;
            continue;
        }
        let mut end = start;
        let (mut removed, mut added) = (Vec::new(), Vec::new());
        while end < lines.len() && lines[end].kind != DiffPreviewLineKind::Context {
            if lines[end].kind == DiffPreviewLineKind::Removed {
                removed.push(end);
            } else {
                added.push(end);
            }
            end += 1;
        }
        // Unequal blocks don't establish which lines replace one another:
        // they keep the line tint only rather than imply a false pairing.
        if removed.len() == added.len() {
            for (&before, &after) in removed.iter().zip(&added) {
                if let Some((b, a)) = changed_words(&lines[before].content, &lines[after].content) {
                    ranges[before] = b;
                    ranges[after] = a;
                }
            }
        }
        start = end;
    }
    lines
        .iter()
        .enumerate()
        .map(|(i, l)| {
            let tokens = match syntax[i].take() {
                Some(spans) => spans.into_iter().map(|(t, s)| (Some(t), s)).collect(),
                None => vec![(None, l.content.clone())],
            };
            annotate(tokens, &ranges[i])
        })
        .collect()
}

/// The dialog's `decorations` (`DiffReviewDialog.tsx:49-57`): every hunk
/// decorated with its file's language, or `None` past the bound.
pub fn decorate_preview(files: &[DiffPreviewFile]) -> Option<Decorations> {
    if !can_decorate(files) {
        return None;
    }
    Some(
        files
            .iter()
            .map(|f| {
                let lang = diff_language(&f.path);
                f.hunks.iter().map(|h| decorate_hunk(&h.lines, lang.as_deref())).collect()
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(kind: &str, content: &str) -> DiffPreviewLine {
        serde_json::from_value(serde_json::json!({"kind": kind, "content": content})).unwrap()
    }

    #[test]
    fn js_whitespace_and_trim_follow_the_web() {
        assert_eq!(js_trim("\u{feff} x \u{3000}"), "x", "U+FEFF is JS whitespace");
        assert_eq!(js_trim("\u{85}x"), "\u{85}x", "U+0085 is not");
        let w: Vec<&str> = words_re().find_iter("a_b$c  d-é1").map(|m| m.as_str()).collect();
        assert_eq!(w, ["a_b$c", "  ", "d", "-", "é1"]);
    }

    #[test]
    fn an_empty_side_or_identical_lines_mark_nothing() {
        assert_eq!(changed_words("", "x"), None);
        assert_eq!(changed_words("same", "same"), Some((vec![], vec![])));
        let d = decorate_hunk(&[line("removed", ""), line("added", "")], Some("rs"));
        assert!(d.iter().flatten().all(|t| !t.changed));
    }
}
