//! The NATIVE-ONLY supplement: Simplified Chinese for product copy that has
//! NO entry in the web's catalog (`zh.rs`) and no web control of the same
//! meaning (`alias.rs`) — native-only surfaces, native wording, and the few
//! strings the web renders without a zh entry (its Chinese UI shows them in
//! English; the operator asked for Chinese here).
//!
//! This table is NOT the web's. Rules, each pinned by `i18n::tests`:
//! - it is consulted LAST (`super::zh_for`: web key, then alias, then this);
//!   an entry the web catalog or an alias already covers is an error, so the
//!   web's wording wins the day the web adds one;
//! - every value keeps its source's `{placeholders}`;
//! - every value is Chinese, in the web catalog's vocabulary (diff -> 差异,
//!   review -> 审查, preview -> 预览, Session -> 会话, approval -> 批准,
//!   plain text -> 纯文本, server -> 服务器).
//!
//! (A24 phase 2 is building the full supplement in this same file; A28 adds
//! the diff review's rows. A merge is the union of the two tables.)

/// English source -> reviewed Simplified Chinese (native-only copy).
pub static NATIVE_ZH: &[(&str, &str)] = &[
    // ---- A28: the diff review (screens/board3/diff_review.rs)
    // The web's `plainNotice` (DiffReviewDialog.tsx:107-109) has no t() / zh.
    ("Large preview shown as plain text. All lines are included.", "大型预览以纯文本显示，已包含所有行。"),
    ("No diff preview yet", "尚无差异预览"),
    (
        "A preview appears here once this Session proposes file changes, such as an approval that edits files.",
        "此会话提出文件更改（例如需要批准的文件编辑）后，预览会显示在这里。",
    ),
    ("This server does not provide diff previews.", "此服务器不提供差异预览。"),
    ("Code review…", "代码审查…"),
];
