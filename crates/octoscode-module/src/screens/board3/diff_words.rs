//! A28 — parity row 23: the diff review's word-level change marks and
//! per-file syntax colours, bounded (the web's
//! `features/review/diff-presentation.ts:19-205`).
//!
//! FAILING-FIRST STUB: the signatures the tests drive; the port lands next.
use std::ops::Range;

use octos_core::ui_protocol::{DiffPreviewFile, DiffPreviewLine};

use crate::highlight::Tok;

/// `canDecorateDiff` (diff-presentation.ts:26-40): more lines than this and
/// nothing is decorated.
pub const MAX_LINES: usize = 400;
/// … more characters (UTF-16 code units, the web's `.length`) in all lines.
pub const MAX_CHARS: usize = 40_000;
/// … a single line longer than this.
pub const MAX_LINE_CHARS: usize = 2_000;
/// `changedWords` (:118-175): a side with more words than this is not compared.
pub const MAX_WORDS: usize = 160;
/// … a pair sharing less than this share of its words keeps the line tint only.
pub const MIN_SHARED: f64 = 0.25;

/// One decorated piece of a line (`DiffToken`, :10-12): its text, its syntax
/// class (`None` = no grammar), whether it is a changed word.
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
    s.chars().count()
}

/// `canDecorateDiff`.
pub fn can_decorate(_files: &[DiffPreviewFile]) -> bool {
    true
}

/// `diffLanguage` (:42-46).
pub fn diff_language(_path: &str) -> Option<String> {
    None
}

/// `changedWords`: the changed byte ranges of each side, or `None`.
pub fn changed_words(_before: &str, _after: &str) -> Option<(Vec<Range<usize>>, Vec<Range<usize>>)> {
    None
}

/// `decorateDiffHunk` (:48-116).
pub fn decorate_hunk(lines: &[DiffPreviewLine], _language: Option<&str>) -> Vec<Vec<DiffToken>> {
    lines
        .iter()
        .map(|l| vec![DiffToken { text: l.content.clone(), tok: None, changed: false }])
        .collect()
}

/// The whole preview, or `None` past the bound (all or nothing).
pub fn decorate_preview(files: &[DiffPreviewFile]) -> Option<Decorations> {
    Some(files.iter().map(|f| f.hunks.iter().map(|h| decorate_hunk(&h.lines, None)).collect()).collect())
}
