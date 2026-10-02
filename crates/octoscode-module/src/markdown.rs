//! A7 — the answer's markdown DISPLAY rules, ported from the web oracle
//! (`src-web/apps/web/src/features/markdown/*`).
//!
//! The settled answer is ONE native `Markdown` region (A1, `fluid.rs`); what
//! that region is given is decided here, as pure functions over the stored
//! text (the stored text itself is never changed — every rule below is
//! "what is rendered"):
//!
//! * [`safe_link_url`] / [`sanitize`] — `MarkdownBody.tsx:20-37`
//!   `safeUrlTransform`: only absolute `http:`/`https:`/`mailto:` links keep
//!   their destination; any other link renders its text only, and every
//!   model-authored image is stripped to its alt text (`:39-53`, the
//!   `md-image-alt` span), so nothing loads a remote image without a gesture.
//! * [`close_open_fence`] — `streaming-fence.ts:13-30`: mid-stream, a fence the
//!   model opened but has not closed is closed FOR DISPLAY with the same
//!   character and at least the same length.
//! * [`has_math`] — `math.ts:16-40`: the delimiters remark-math understands,
//!   minus money and shell text. Math is typeset only for a settled reply that
//!   carries math; otherwise every `$` outside code is escaped so the
//!   renderer's math extension can never typeset prose (`MarkdownBody.tsx:
//!   141-160`: math waits for the finished reply).
//! * [`segments`] — the answer split at its top-level fenced code blocks, so
//!   each block gets the web's banner (language + Copy, `CodeBlock.tsx:94-107`)
//!   and [`copy_text`] is the trimmed code (`CodeBlock.tsx:45`).
//!
//! Streaming keeps CommonMark's own rule for an unterminated emphasis marker:
//! `**still streaming` stays literal until its closer arrives (the renderer is
//! pulldown-cmark, a CommonMark parser — `MarkdownBody.test.tsx:44`).
use std::collections::HashMap;

/// `safeUrlTransform` for a LINK destination (`MarkdownBody.tsx:20-37`): the
/// destination survives only when it parses as an ABSOLUTE URL whose scheme
/// is `http`, `https` or `mailto` (the browser's `new URL(url)` — a relative
/// path or `#anchor` throws there, so it is dropped here too).
pub fn safe_link_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed = url::Url::parse(trimmed).ok()?;
    match parsed.scheme() {
        "http" | "https" | "mailto" => Some(trimmed.to_owned()),
        _ => None,
    }
}

/// `closeOpenFence` (`streaming-fence.ts:13-30`), verbatim: a line opening
/// with up to three spaces then ```` ``` ````/`~~~` (three or more) opens a
/// fence; a later fence of the SAME character and at least the same length
/// closes it. An open fence at the end is closed for display.
pub fn close_open_fence(text: &str) -> String {
    let mut open: Option<String> = None;
    for line in text.split('\n') {
        let Some(marker) = fence_marker(line) else { continue };
        match &open {
            None => open = Some(marker),
            Some(o) => {
                if marker.chars().next() == o.chars().next() && marker.len() >= o.len() {
                    open = None;
                }
            }
        }
    }
    match open {
        None => text.to_owned(),
        Some(o) => {
            let sep = if text.ends_with('\n') { "" } else { "\n" };
            format!("{text}{sep}{o}")
        }
    }
}

/// `/^ {0,3}(`{3,}|~{3,})/` — the fence marker run a line opens with.
fn fence_marker(line: &str) -> Option<String> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let ch = rest.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let run = rest.chars().take_while(|c| *c == ch).count();
    (run >= 3).then(|| ch.to_string().repeat(run))
}

// ------------------------------------------------------------------- math

/// `hasMath` (`math.ts:31-40`). Display `$$…$$` (one line or across lines),
/// `\(…\)` / `\[…\]`, or a single-line inline `$…$` whose content is neither
/// empty, whitespace-bounded nor a bare currency amount.
pub fn has_math(text: &str) -> bool {
    if !text.contains('$') && !text.contains("\\(") && !text.contains("\\[") {
        return false;
    }
    if has_display_math(text) || has_latex_math(text) {
        return true;
    }
    match first_inline_math(text) {
        Some(content) => !currency_only(&content),
        None => false,
    }
}

/// `DISPLAY = /\$\$[\s\S]+?\$\$/`.
fn has_display_math(text: &str) -> bool {
    let Some(start) = text.find("$$") else { return false };
    let after = start + 2;
    // At least one character between the delimiters.
    let mut it = text[after..].char_indices();
    match it.next() {
        Some((_, c)) => text[after + c.len_utf8()..].contains("$$"),
        None => false,
    }
}

/// `LATEX = /\\\((?:[\s\S]{1,400}?)\\\)|\\\[(?:[\s\S]{1,400}?)\\\]/`.
fn has_latex_math(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    for (open, close) in [('(', ')'), ('[', ']')] {
        let mut i = 0;
        while i + 1 < chars.len() {
            if chars[i] == '\\' && chars[i + 1] == open {
                // Lazy: the first closer after at least one content char,
                // within 400 content chars.
                let start = i + 2;
                let mut j = start + 1;
                while j + 1 < chars.len() && j - start <= 400 {
                    if chars[j] == '\\' && chars[j + 1] == close {
                        return true;
                    }
                    j += 1;
                }
            }
            i += 1;
        }
    }
    false
}

/// The FIRST match of
/// `INLINE = /(?<![\\$])\$(?!\s)([^$\n]{1,200})(?<![\\\s])\$(?!\$)/`,
/// returning its content group. Content cannot hold a `$`, so the only
/// possible closer for an opener is the next `$` on the line.
fn first_inline_math(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    for i in 0..n {
        if chars[i] != '$' {
            continue;
        }
        if i > 0 && (chars[i - 1] == '\\' || chars[i - 1] == '$') {
            continue;
        }
        if i + 1 >= n || chars[i + 1].is_whitespace() {
            continue;
        }
        // The content runs to the next `$` or newline.
        let mut k = i + 1;
        while k < n && chars[k] != '$' && chars[k] != '\n' {
            k += 1;
        }
        if k >= n || chars[k] != '$' {
            continue;
        }
        let len = k - (i + 1);
        if len == 0 || len > 200 {
            continue;
        }
        let last = chars[k - 1];
        if last == '\\' || last.is_whitespace() {
            continue;
        }
        if k + 1 < n && chars[k + 1] == '$' {
            continue;
        }
        return Some(chars[i + 1..k].iter().collect());
    }
    None
}

/// `CURRENCY_ONLY = /^[\s,.\d]+$/` — a bare amount is money, not math.
fn currency_only(s: &str) -> bool {
    !s.is_empty() && s.chars().all(|c| c.is_whitespace() || c == ',' || c == '.' || c.is_ascii_digit())
}

// ------------------------------------------------------------ segmenting

/// One top-level piece of a displayed answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Segment {
    /// Markdown prose for one native `Markdown` region.
    Prose(String),
    /// A top-level fenced code block: its info-string language (if any) and
    /// its code as the parser hands it over (every content line followed by
    /// `\n`, the value `CodeBlock.tsx` receives).
    Code { lang: Option<String>, code: String },
}

/// The display form of one answer: its segments, and whether the renderer
/// should typeset math (a settled reply that [`has_math`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Display {
    pub segments: Vec<Segment>,
    pub math: bool,
    /// The reply is still being written: code renders plain (the web's
    /// `streamingComponents`, `MarkdownBody.tsx:118-136`).
    pub streaming: bool,
}

/// The full display pipeline for one answer's stored text.
///
/// `streaming` = the reply is still being written: the open fence is closed
/// for display (`MarkdownBody.tsx:141-152`) and math waits for the finished
/// reply (a half-written `$…` cannot be typeset).
pub fn display(text: &str, streaming: bool) -> Display {
    let text = if streaming { close_open_fence(text) } else { text.to_owned() };
    let math = !streaming && has_math(&text);
    let mut segments = Vec::new();
    for seg in split_top_level_fences(&text) {
        match seg {
            Segment::Prose(p) => {
                let p = sanitize(&p, !math);
                if !p.trim().is_empty() {
                    segments.push(Segment::Prose(p));
                }
            }
            code => segments.push(code),
        }
    }
    Display { segments, math, streaming }
}

/// The text a block's Copy control writes (`CodeBlock.tsx:45`): the code
/// with ONE trailing newline removed.
pub fn copy_text(code: &str) -> String {
    code.strip_suffix('\n').unwrap_or(code).to_owned()
}

/// Split at fences that open at COLUMN 0 (a fence indented 1-3 spaces is
/// usually inside a list item, and splitting there would break the list; it
/// stays inside the prose region, rendered by the region's own code style).
/// A fence that never closes runs to the end, as CommonMark specifies.
fn split_top_level_fences(text: &str) -> Vec<Segment> {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Vec::new();
    let mut prose: Vec<&str> = Vec::new();
    // While inside an INDENTED fence (left in the prose), track it so a
    // column-0 fence-looking line inside it is content, not a block start.
    let mut nested_open: Option<String> = None;
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(open) = &nested_open {
            if let Some(m) = fence_marker(line) {
                if m.chars().next() == open.chars().next() && m.len() >= open.len() {
                    nested_open = None;
                }
            }
            prose.push(line);
            i += 1;
            continue;
        }
        let Some(marker) = fence_marker(line) else {
            prose.push(line);
            i += 1;
            continue;
        };
        let indent = line.len() - line.trim_start_matches(' ').len();
        if indent > 0 {
            nested_open = Some(marker);
            prose.push(line);
            i += 1;
            continue;
        }
        let info = line[marker.len()..].trim();
        // A backtick fence's info string may not contain a backtick
        // (CommonMark 4.5) — then the line is ordinary text.
        if marker.starts_with('`') && info.contains('`') {
            prose.push(line);
            i += 1;
            continue;
        }
        if !prose.is_empty() {
            out.push(Segment::Prose(prose.join("\n")));
            prose.clear();
        }
        let lang = info
            .split_whitespace()
            .next()
            .map(|l| l.to_owned())
            .filter(|l| !l.is_empty());
        let mut body: Vec<&str> = Vec::new();
        let mut j = i + 1;
        let mut closed = false;
        while j < lines.len() {
            if let Some(m) = fence_marker(lines[j]) {
                let rest = lines[j].trim_start_matches(' ')[m.len()..].trim();
                if m.chars().next() == marker.chars().next() && m.len() >= marker.len() && rest.is_empty() {
                    closed = true;
                    break;
                }
            }
            body.push(lines[j]);
            j += 1;
        }
        let mut code = String::new();
        for line in &body {
            code.push_str(line);
            code.push('\n');
        }
        out.push(Segment::Code { lang, code });
        i = if closed { j + 1 } else { j };
    }
    if !prose.is_empty() {
        out.push(Segment::Prose(prose.join("\n")));
    }
    out
}

// ------------------------------------------------------------- sanitizing

/// `safeUrlTransform` + the `img` component over one prose region: unsafe
/// link destinations render their text only, images render their alt text
/// (`*alt*`, or `Image` when it has none), unsafe autolinks render as text,
/// and unsafe reference definitions are dropped (their uses keep the text).
/// With `escape_dollars`, every `$` outside code is escaped so the
/// renderer's math extension stays off (the web's plain renderer has none).
/// Code spans and indented-fence bodies are never rewritten.
pub fn sanitize(prose: &str, escape_dollars: bool) -> String {
    // Reference definitions first: `[label]: <dest> "title"` lines.
    let mut defs: HashMap<String, bool> = HashMap::new();
    let mut kept: Vec<String> = Vec::new();
    let mut fence: Option<String> = None;
    for line in prose.split('\n') {
        if let Some(open) = &fence {
            if let Some(m) = fence_marker(line) {
                if m.chars().next() == open.chars().next() && m.len() >= open.len() {
                    fence = None;
                }
            }
            kept.push(line.to_owned());
            continue;
        }
        if let Some(m) = fence_marker(line) {
            fence = Some(m);
            kept.push(line.to_owned());
            continue;
        }
        if let Some((label, dest)) = reference_definition(line) {
            let safe = safe_link_url(&dest).is_some();
            defs.insert(normalize_label(&label), safe);
            if !safe {
                // Dropped: its uses render their text (below).
                continue;
            }
        }
        kept.push(line.to_owned());
    }
    // Then the inline pass, outside fences.
    let mut out = String::with_capacity(prose.len());
    let mut fence: Option<String> = None;
    let mut block = String::new();
    let flush = |block: &mut String, out: &mut String| {
        if !block.is_empty() {
            out.push_str(&rewrite_inline(block, &defs, escape_dollars));
            block.clear();
        }
    };
    for (n, line) in kept.iter().enumerate() {
        let nl = if n + 1 < kept.len() { "\n" } else { "" };
        if let Some(open) = &fence {
            if let Some(m) = fence_marker(line) {
                if m.chars().next() == open.chars().next() && m.len() >= open.len() {
                    fence = None;
                }
            }
            out.push_str(line);
            out.push_str(nl);
            continue;
        }
        if let Some(m) = fence_marker(line) {
            flush(&mut block, &mut out);
            fence = Some(m);
            out.push_str(line);
            out.push_str(nl);
            continue;
        }
        block.push_str(line);
        block.push_str(nl);
    }
    flush(&mut block, &mut out);
    out
}

/// CommonMark label normalisation: case-fold and collapse whitespace.
fn normalize_label(label: &str) -> String {
    label.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// `^ {0,3}\[label\]:\s*<dest>|dest` — a link reference definition line.
fn reference_definition(line: &str) -> Option<(String, String)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let rest = rest.strip_prefix('[')?;
    let close = rest.find("]:")?;
    let label = &rest[..close];
    if label.trim().is_empty() || label.contains('[') || label.contains(']') {
        return None;
    }
    let tail = rest[close + 2..].trim_start();
    let dest = if let Some(t) = tail.strip_prefix('<') {
        t.split('>').next()?.to_owned()
    } else {
        tail.split_whitespace().next()?.to_owned()
    };
    Some((label.to_owned(), dest))
}

/// The inline rewrite over a block of non-fenced text.
fn rewrite_inline(s: &str, defs: &HashMap<String, bool>, escape_dollars: bool) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 16);
    rewrite_range(&chars, 0, chars.len(), defs, escape_dollars, &mut out);
    out
}

fn rewrite_range(
    c: &[char],
    from: usize,
    to: usize,
    defs: &HashMap<String, bool>,
    escape_dollars: bool,
    out: &mut String,
) {
    let mut i = from;
    while i < to {
        let ch = c[i];
        // A backslash escape is copied whole (`\$`, `\[`, …).
        if ch == '\\' && i + 1 < to {
            out.push(ch);
            out.push(c[i + 1]);
            i += 2;
            continue;
        }
        // A code span: a backtick run closes at the next run of the SAME
        // length; nothing inside is rewritten.
        if ch == '`' {
            let run = count_run(c, i, to, '`');
            if let Some(end) = find_closing_backticks(c, i + run, to, run) {
                out.extend(&c[i..end + run]);
                i = end + run;
            } else {
                out.extend(&c[i..i + run]);
                i += run;
            }
            continue;
        }
        if ch == '!' && i + 1 < to && c[i + 1] == '[' {
            if let Some(link) = parse_link(c, i + 1, to, defs) {
                // An image: its alt text only (the web's `md-image-alt`).
                let alt: String = c[link.text.0..link.text.1].iter().collect();
                out.push_str(&image_alt(&alt));
                i = link.end;
                continue;
            }
        }
        if ch == '[' {
            if let Some(link) = parse_link(c, i, to, defs) {
                if link.safe {
                    // Kept verbatim, except the text itself (an image inside a
                    // link's text is still stripped).
                    out.push('[');
                    rewrite_range(c, link.text.0, link.text.1, defs, escape_dollars, out);
                    out.extend(&c[link.text.1..link.end]);
                } else {
                    rewrite_range(c, link.text.0, link.text.1, defs, escape_dollars, out);
                }
                i = link.end;
                continue;
            }
        }
        if ch == '<' {
            if let Some((inner_end, url)) = parse_autolink(c, i, to) {
                if safe_link_url(&url).is_some() || is_email(&url) {
                    out.extend(&c[i..=inner_end]);
                } else {
                    // Rendered as text: the angle brackets escaped.
                    out.push_str("\\<");
                    out.push_str(&escape_md(&url));
                    out.push_str("\\>");
                }
                i = inner_end + 1;
                continue;
            }
        }
        if ch == '$' && escape_dollars {
            out.push_str("\\$");
            i += 1;
            continue;
        }
        out.push(ch);
        i += 1;
    }
}

fn count_run(c: &[char], at: usize, to: usize, ch: char) -> usize {
    let mut n = 0;
    while at + n < to && c[at + n] == ch {
        n += 1;
    }
    n
}

/// The start of the next backtick run of exactly `run` after `from`.
fn find_closing_backticks(c: &[char], from: usize, to: usize, run: usize) -> Option<usize> {
    let mut i = from;
    while i < to {
        if c[i] == '`' {
            let r = count_run(c, i, to, '`');
            if r == run {
                return Some(i);
            }
            i += r;
        } else {
            i += 1;
        }
    }
    None
}

/// A parsed `[text](dest "title")` / `[text][ref]` / `[ref]` construct.
struct Link {
    /// The text's char range (inside the brackets).
    text: (usize, usize),
    /// One past the construct's last char.
    end: usize,
    /// Whether the destination survives `safeUrlTransform`.
    safe: bool,
}

/// Parse the link construct starting at `c[open] == '['`.
fn parse_link(c: &[char], open: usize, to: usize, defs: &HashMap<String, bool>) -> Option<Link> {
    let close = matching_bracket(c, open, to)?;
    let text = (open + 1, close);
    let after = close + 1;
    // Inline: `(` destination [title] `)`.
    if after < to && c[after] == '(' {
        if let Some((dest, end)) = parse_inline_dest(c, after, to) {
            return Some(Link { text, end, safe: safe_link_url(&dest).is_some() });
        }
    }
    // Full reference `[text][ref]` / collapsed `[text][]`.
    if after < to && c[after] == '[' {
        if let Some(rclose) = matching_bracket(c, after, to) {
            let label: String = c[after + 1..rclose].iter().collect();
            let label = if label.trim().is_empty() { c[text.0..text.1].iter().collect() } else { label };
            if let Some(safe) = defs.get(&normalize_label(&label)) {
                return Some(Link { text, end: rclose + 1, safe: *safe });
            }
        }
    }
    // Shortcut reference `[ref]`.
    let label: String = c[text.0..text.1].iter().collect();
    if let Some(safe) = defs.get(&normalize_label(&label)) {
        return Some(Link { text, end: after, safe: *safe });
    }
    None
}

/// The `]` matching `c[open] == '['`, honouring nesting, escapes and code
/// spans.
fn matching_bracket(c: &[char], open: usize, to: usize) -> Option<usize> {
    let mut depth = 0usize;
    let mut i = open;
    while i < to {
        match c[i] {
            '\\' => {
                i += 2;
                continue;
            }
            '`' => {
                let run = count_run(c, i, to, '`');
                match find_closing_backticks(c, i + run, to, run) {
                    Some(end) => i = end + run,
                    None => i += run,
                }
                continue;
            }
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// `(dest "title")` starting at `c[at] == '('`: the destination and one past
/// the closing `)`.
fn parse_inline_dest(c: &[char], at: usize, to: usize) -> Option<(String, usize)> {
    let mut i = at + 1;
    let skip_ws = |i: &mut usize| {
        while *i < to && (c[*i] == ' ' || c[*i] == '\t' || c[*i] == '\n') {
            *i += 1;
        }
    };
    skip_ws(&mut i);
    let mut dest = String::new();
    if i < to && c[i] == '<' {
        i += 1;
        while i < to && c[i] != '>' {
            if c[i] == '\n' || c[i] == '<' {
                return None;
            }
            dest.push(c[i]);
            i += 1;
        }
        if i >= to {
            return None;
        }
        i += 1;
    } else {
        let mut depth = 0i32;
        while i < to {
            let ch = c[i];
            if ch == '\\' && i + 1 < to {
                dest.push(c[i + 1]);
                i += 2;
                continue;
            }
            if ch.is_whitespace() || ch.is_control() {
                break;
            }
            if ch == '(' {
                depth += 1;
            } else if ch == ')' {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            dest.push(ch);
            i += 1;
        }
    }
    skip_ws(&mut i);
    // Optional title.
    if i < to && (c[i] == '"' || c[i] == '\'' || c[i] == '(') {
        let close = if c[i] == '(' { ')' } else { c[i] };
        i += 1;
        while i < to && c[i] != close {
            if c[i] == '\\' {
                i += 1;
            }
            i += 1;
        }
        if i >= to {
            return None;
        }
        i += 1;
        skip_ws(&mut i);
    }
    if i < to && c[i] == ')' {
        Some((dest, i + 1))
    } else {
        None
    }
}

/// `<scheme:rest>` or `<user@host>` starting at `c[at] == '<'`: the index of
/// the closing `>` and the inner text.
fn parse_autolink(c: &[char], at: usize, to: usize) -> Option<(usize, String)> {
    let mut i = at + 1;
    let mut inner = String::new();
    while i < to && c[i] != '>' {
        if c[i] == '<' || c[i].is_whitespace() || c[i].is_control() {
            return None;
        }
        inner.push(c[i]);
        i += 1;
    }
    if i >= to || inner.is_empty() {
        return None;
    }
    // A URI autolink needs a 2-32 char scheme then `:`.
    let scheme_ok = match inner.find(':') {
        Some(k) if (2..=32).contains(&k) => {
            let s = &inner[..k];
            s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '.' || c == '-')
        }
        _ => false,
    };
    if scheme_ok || is_email(&inner) {
        Some((i, inner))
    } else {
        None
    }
}

/// CommonMark's email autolink shape (simplified: `local@domain.tld`).
fn is_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else { return false };
    !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && local.chars().all(|c| c.is_ascii_alphanumeric() || ".!#$%&'*+/=?^_`{|}~-".contains(c))
        && domain.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

/// The alt text an image renders as (`img` component: the alt, else
/// `Image`), emphasised like the web's italic `md-image-alt` span.
fn image_alt(alt: &str) -> String {
    let plain: String = alt
        .chars()
        .filter(|c| !matches!(c, '*' | '_' | '`'))
        .collect();
    let plain = plain.split_whitespace().collect::<Vec<_>>().join(" ");
    let shown = if plain.is_empty() { "Image".to_owned() } else { plain };
    format!("*{}*", escape_md(&shown))
}

/// Escape the characters that would start markup in a plain run.
fn escape_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for ch in s.chars() {
        if matches!(ch, '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '>' | '$' | '!' | '#' | '|' | '~') {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- streaming-fence.test.ts (all seven cases)
    #[test]
    fn closes_a_fence_the_model_has_not_closed_yet() {
        assert_eq!(close_open_fence("Here:\n\n```ts\nconst x = 1;"), "Here:\n\n```ts\nconst x = 1;\n```");
        let balanced = "```\na\n```\n\nafter";
        assert_eq!(close_open_fence(balanced), balanced);
        assert_eq!(close_open_fence("just prose"), "just prose");
        assert_eq!(close_open_fence("~~~~\ncode"), "~~~~\ncode\n~~~~");
        assert_eq!(close_open_fence("```\na\n~~~\nb"), "```\na\n~~~\nb\n```");
        assert_eq!(close_open_fence("   ```\nx"), "   ```\nx\n```");
        assert_eq!(close_open_fence("```\nx\n"), "```\nx\n```");
    }

    // ---- math.test.tsx:7 / :17
    #[test]
    fn math_detection_matches_the_web_delimiters() {
        assert!(has_math("inline $E = mc^2$ here"));
        assert!(has_math("block:\n\n$$\\int_0^1 x^2 dx = \\frac{1}{3}$$\n"));
        assert!(has_math("across\n$$\na^2 + b^2 = c^2\n$$\nlines"));
        assert!(has_math("latex \\(x + y\\) form"));
        assert!(has_math("display \\[x + y\\] form"));
        assert!(!has_math("a plain summary of the change"));
        assert!(!has_math("it costs $12 and $1,000.50 more"));
        assert!(!has_math("run `echo $PATH` then `cd $HOME`"));
        assert!(!has_math("an escaped \\$price\\$ stays text"));
        assert!(!has_math("a lone $ sign"));
    }

    // ---- MarkdownBody.test.tsx:31 "allows only absolute safe links"
    #[test]
    fn only_absolute_safe_links_keep_their_destination() {
        let out = sanitize(
            "[safe](https://example.com) [relative](/secret) [script](javascript:alert(1)) ![mail](mailto:person@example.com)",
            true,
        );
        assert!(out.contains("[safe](https://example.com)"), "{out}");
        assert!(!out.contains("/secret"), "{out}");
        assert!(out.contains("relative"));
        assert!(!out.contains("javascript:"), "{out}");
        assert!(out.contains("script"));
        assert!(!out.contains("mailto:person"), "an image never keeps its src: {out}");
        assert!(out.contains("*mail*"), "the alt text stays: {out}");
        assert_eq!(safe_link_url("mailto:person@example.com").as_deref(), Some("mailto:person@example.com"));
        assert_eq!(safe_link_url("HTTPS://Example.com/a").as_deref(), Some("HTTPS://Example.com/a"));
        for bad in ["/secret", "#top", "javascript:alert(1)", "data:text/html,x", "file:///etc/passwd", "vbscript:x", "", "   "] {
            assert_eq!(safe_link_url(bad), None, "{bad}");
        }
    }

    #[test]
    fn images_render_their_alt_and_nested_images_in_links_too() {
        assert_eq!(sanitize("![a chart](https://x.test/c.png)", true), "*a chart*");
        assert_eq!(sanitize("![](https://x.test/c.png)", true), "*Image*");
        let out = sanitize("[![badge](https://ci.test/b.svg)](https://ci.test)", true);
        assert_eq!(out, "[*badge*](https://ci.test)");
        // Reference forms resolve through their definitions.
        let out = sanitize("see [docs][d] and [bad][b]\n\n[d]: https://docs.test\n[b]: javascript:x", true);
        assert!(out.contains("[docs][d]"), "{out}");
        assert!(out.contains("[d]: https://docs.test"), "{out}");
        assert!(!out.contains("javascript"), "{out}");
        assert!(out.contains(" bad"), "{out}");
        // Unsafe autolinks become text; safe ones stay links.
        assert_eq!(sanitize("<https://ok.test>", true), "<https://ok.test>");
        assert_eq!(sanitize("<javascript:alert(1)>", true), "\\<javascript:alert(1)\\>");
        assert_eq!(sanitize("<me@example.com>", true), "<me@example.com>");
    }

    #[test]
    fn code_is_never_rewritten() {
        let src = "use `![x](y)` and `$HOME`\n\n```sh\necho $PATH [a](javascript:x)\n```";
        let out = sanitize(src, true);
        assert!(out.contains("`![x](y)`"), "{out}");
        assert!(out.contains("`$HOME`"), "{out}");
        assert!(out.contains("echo $PATH [a](javascript:x)"), "fenced code is verbatim: {out}");
    }

    #[test]
    fn dollars_outside_code_are_escaped_unless_math_renders() {
        assert_eq!(sanitize("costs $12", true), "costs \\$12");
        assert_eq!(sanitize("costs $12", false), "costs $12");
        assert_eq!(sanitize("already \\$ escaped", true), "already \\$ escaped");
    }

    // ---- MarkdownBody.test.tsx:44/:55/:105 (streaming)
    #[test]
    fn streaming_closes_the_fence_and_keeps_an_unclosed_marker_literal() {
        let d = display("Here:\n\n```ts\nconst x = 1;", true);
        assert_eq!(
            d.segments,
            vec![
                Segment::Prose("Here:\n".into()),
                Segment::Code { lang: Some("ts".into()), code: "const x = 1;\n".into() },
            ]
        );
        // `**` with no closer is passed through untouched: CommonMark keeps it
        // literal (no bold run to the end).
        let d = display("**still streaming", true);
        assert_eq!(d.segments, vec![Segment::Prose("**still streaming".into())]);
        // Math waits for the finished reply.
        assert!(!display("inline $E = mc^2$ here", true).math);
        assert!(display("inline $E = mc^2$ here", false).math);
    }

    #[test]
    fn segments_split_only_at_column_zero_fences() {
        let src = "Intro\n\n```rust\nfn main() {}\n\n```\n\n1. step\n   ```sh\n   cargo test\n   ```\n2. done";
        let d = display(src, false);
        assert_eq!(d.segments.len(), 3, "{:?}", d.segments);
        assert_eq!(d.segments[1], Segment::Code { lang: Some("rust".into()), code: "fn main() {}\n\n".into() });
        // The copy text drops exactly one trailing newline (the blank line stays).
        if let Segment::Code { code, .. } = &d.segments[1] {
            assert_eq!(copy_text(code), "fn main() {}\n");
        }
        match &d.segments[2] {
            Segment::Prose(p) => assert!(p.contains("   ```sh") && p.contains("2. done"), "{p}"),
            other => panic!("{other:?}"),
        }
        // A tilde fence closes only with tildes; an unclosed fence runs to the end.
        let d = display("~~~\na\n```\nb", false);
        assert_eq!(d.segments, vec![Segment::Code { lang: None, code: "a\n```\nb\n".into() }]);
    }

    // ---- CodeBlock.tsx:45 — the trimmed code, no trailing newline
    #[test]
    fn copy_writes_the_trimmed_code() {
        assert_eq!(copy_text("fn main() {}\n"), "fn main() {}");
        assert_eq!(copy_text("a\n\n"), "a\n");
        assert_eq!(copy_text("x"), "x");
    }
}
