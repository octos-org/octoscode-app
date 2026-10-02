//! A7 — code-block syntax colouring, the native counterpart of the web's
//! lazy shiki highlighter (`features/markdown/highlight.ts:22-215`,
//! `CodeBlock.tsx:27-121`).
//!
//! The web loads a grammar only for a block scrolled into view and falls back
//! to plain, copyable code when a grammar is missing. Natively the same two
//! rules hold by construction: a transcript row is lowered only when the
//! virtualized list instantiates it (the visible rows), the lowering is
//! memoised, and [`grammar`] returns `None` for a language outside the web's
//! table, which renders the block plain. A streaming reply is never
//! highlighted (`MarkdownBody.tsx:118-136`: highlighting re-tokenises the whole
//! block on every token; the finished reply is highlighted once).
//!
//! This is a compact lexer — keywords, strings, comments, numbers, calls and
//! punctuation per language, the token classes the web's CSS colours
//! (`app/theme.css:101-112`, `--shiki-token-*`) — not a full TextMate engine.

/// The language ids the web registers (`highlight.ts:22-37`) and their
/// aliases (`:39-68`).
pub fn grammar(lang: Option<&str>) -> Option<Grammar> {
    let id = lang?.trim().to_ascii_lowercase();
    let id = match id.as_str() {
        "typescript" | "ts" | "tsx" | "javascript" | "js" | "jsx" => "typescript",
        "shellscript" | "bash" | "sh" | "shell" | "zsh" => "shellscript",
        "json" | "jsonc" => "json",
        "python" | "py" => "python",
        "rust" | "rs" => "rust",
        "go" => "go",
        "java" => "java",
        "c" => "c",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "markdown" | "md" => "markdown",
        "html" => "html",
        "css" => "css",
        "sql" => "sql",
        _ => return None,
    };
    Some(Grammar { id })
}

/// One registered grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Grammar {
    pub id: &'static str,
}

/// The token classes the web colours (`--shiki-token-*`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tok {
    Plain,
    Keyword,
    String,
    Comment,
    Constant,
    Function,
    Punctuation,
}

impl Tok {
    /// `#rrggbbaa` for the light or dark theme (`theme.css:101-112`).
    pub fn color(self, dark: bool) -> &'static str {
        match (self, dark) {
            (Tok::Plain, false) => "#1d1d1fff",
            (Tok::Plain, true) => "#f5f5f7ff",
            (Tok::Keyword, false) => "#b4235aff",
            (Tok::Keyword, true) => "#faa2c1ff",
            (Tok::String, false) => "#2b6f3aff",
            (Tok::String, true) => "#69db7cff",
            (Tok::Comment, false) => "#5f666dff",
            (Tok::Comment, true) => "#adb5bdff",
            (Tok::Constant, false) => "#1864abff",
            (Tok::Constant, true) => "#4dabf7ff",
            (Tok::Function, false) => "#5f3dc4ff",
            (Tok::Function, true) => "#b197fcff",
            (Tok::Punctuation, false) => "#495057ff",
            (Tok::Punctuation, true) => "#ced4daff",
        }
    }
}

/// Multi-line lexer state carried from one line to the next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Carry {
    #[default]
    None,
    /// Inside a `/* … */` (or `<!-- … -->`) comment.
    BlockComment,
    /// Inside a triple-quoted (python) or back-tick (typescript) string.
    LongString(char),
}

struct Spec {
    line_comments: &'static [&'static str],
    block: Option<(&'static str, &'static str)>,
    quotes: &'static [char],
    /// A quote char whose string may span lines (python `"""`, ts template).
    long: Option<char>,
    keywords: &'static [&'static str],
    constants: &'static [&'static str],
    /// `key:` / `key =` at line start is a property (yaml/toml).
    keyed: bool,
}

fn spec(g: Grammar) -> Spec {
    const NONE: &[&str] = &[];
    match g.id {
        "rust" => Spec {
            line_comments: &["//"],
            block: Some(("/*", "*/")),
            quotes: &['"'],
            long: None,
            keywords: &[
                "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern", "fn",
                "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self",
                "Self", "static", "struct", "super", "trait", "type", "unsafe", "use", "where", "while",
            ],
            constants: &["true", "false", "None", "Some", "Ok", "Err"],
            keyed: false,
        },
        "typescript" => Spec {
            line_comments: &["//"],
            block: Some(("/*", "*/")),
            quotes: &['"', '\''],
            long: Some('`'),
            keywords: &[
                "async", "await", "break", "case", "catch", "class", "const", "continue", "default", "delete", "do",
                "else", "enum", "export", "extends", "finally", "for", "from", "function", "if", "implements", "import",
                "in", "instanceof", "interface", "let", "new", "of", "return", "static", "switch", "this", "throw",
                "try", "type", "typeof", "var", "void", "while", "yield",
            ],
            constants: &["true", "false", "null", "undefined", "NaN"],
            keyed: false,
        },
        "python" => Spec {
            line_comments: &["#"],
            block: None,
            quotes: &['"', '\''],
            long: Some('"'),
            keywords: &[
                "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif", "else",
                "except", "finally", "for", "from", "global", "if", "import", "in", "is", "lambda", "nonlocal", "not",
                "or", "pass", "raise", "return", "try", "while", "with", "yield",
            ],
            constants: &["True", "False", "None", "self"],
            keyed: false,
        },
        "go" => Spec {
            line_comments: &["//"],
            block: Some(("/*", "*/")),
            quotes: &['"', '\''],
            long: Some('`'),
            keywords: &[
                "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for", "func",
                "go", "goto", "if", "import", "interface", "map", "package", "range", "return", "select", "struct",
                "switch", "type", "var",
            ],
            constants: &["true", "false", "nil", "iota"],
            keyed: false,
        },
        "java" | "c" => Spec {
            line_comments: &["//"],
            block: Some(("/*", "*/")),
            quotes: &['"', '\''],
            long: None,
            keywords: &[
                "abstract", "break", "case", "catch", "char", "class", "const", "continue", "default", "do", "double",
                "else", "enum", "extends", "final", "finally", "float", "for", "if", "implements", "import", "int",
                "interface", "long", "new", "package", "private", "protected", "public", "return", "short", "sizeof",
                "static", "struct", "switch", "this", "throw", "throws", "try", "typedef", "union", "unsigned", "void",
                "volatile", "while", "#include", "#define",
            ],
            constants: &["true", "false", "null", "NULL"],
            keyed: false,
        },
        "shellscript" => Spec {
            line_comments: &["#"],
            block: None,
            quotes: &['"', '\''],
            long: None,
            keywords: &[
                "if", "then", "else", "elif", "fi", "for", "while", "do", "done", "case", "esac", "in", "function",
                "return", "export", "local", "set", "unset", "echo", "cd", "exit",
            ],
            constants: &["true", "false"],
            keyed: false,
        },
        "json" => Spec {
            line_comments: &["//"],
            block: Some(("/*", "*/")),
            quotes: &['"'],
            long: None,
            keywords: NONE,
            constants: &["true", "false", "null"],
            keyed: false,
        },
        "yaml" => Spec {
            line_comments: &["#"],
            block: None,
            quotes: &['"', '\''],
            long: None,
            keywords: NONE,
            constants: &["true", "false", "null", "yes", "no", "on", "off"],
            keyed: true,
        },
        "toml" => Spec {
            line_comments: &["#"],
            block: None,
            quotes: &['"', '\''],
            long: None,
            keywords: NONE,
            constants: &["true", "false"],
            keyed: true,
        },
        "sql" => Spec {
            line_comments: &["--"],
            block: Some(("/*", "*/")),
            quotes: &['\''],
            long: None,
            keywords: &[
                "select", "from", "where", "insert", "into", "values", "update", "set", "delete", "create", "table",
                "drop", "alter", "join", "left", "right", "inner", "outer", "on", "group", "by", "order", "having",
                "limit", "and", "or", "not", "as", "distinct", "union", "primary", "key", "index",
            ],
            constants: &["null", "true", "false"],
            keyed: false,
        },
        "css" => Spec {
            line_comments: NONE,
            block: Some(("/*", "*/")),
            quotes: &['"', '\''],
            long: None,
            keywords: &["important", "media", "import", "supports", "keyframes"],
            constants: NONE,
            keyed: false,
        },
        "html" => Spec {
            line_comments: NONE,
            block: Some(("<!--", "-->")),
            quotes: &['"', '\''],
            long: None,
            keywords: NONE,
            constants: NONE,
            keyed: false,
        },
        // markdown: prose; headings and fences read as keywords below.
        _ => Spec {
            line_comments: NONE,
            block: None,
            quotes: &[],
            long: None,
            keywords: NONE,
            constants: NONE,
            keyed: false,
        },
    }
}

/// Lex one line, continuing `carry` from the previous line. Returns the
/// spans (adjacent spans of one class merged) and the carry for the next.
pub fn line(g: Grammar, text: &str, carry: Carry) -> (Vec<(Tok, String)>, Carry) {
    let sp = spec(g);
    let c: Vec<char> = text.chars().collect();
    let mut out: Vec<(Tok, String)> = Vec::new();
    let mut push = |tok: Tok, s: &str| {
        if s.is_empty() {
            return;
        }
        if let Some(last) = out.last_mut() {
            if last.0 == tok {
                last.1.push_str(s);
                return;
            }
        }
        out.push((tok, s.to_owned()));
    };
    let mut i = 0usize;
    let mut carry = carry;
    let starts = |at: usize, pat: &str| -> bool {
        let p: Vec<char> = pat.chars().collect();
        at + p.len() <= c.len() && c[at..at + p.len()] == p[..]
    };
    // Continue a multi-line construct.
    match carry {
        Carry::BlockComment => {
            let (_, close) = sp.block.unwrap_or(("/*", "*/"));
            let mut j = 0;
            while j < c.len() && !starts(j, close) {
                j += 1;
            }
            if j >= c.len() {
                push(Tok::Comment, text);
                return (out, Carry::BlockComment);
            }
            let end = j + close.chars().count();
            push(Tok::Comment, &c[..end].iter().collect::<String>());
            i = end;
            carry = Carry::None;
        }
        Carry::LongString(q) => {
            let close: String = if g.id == "python" { q.to_string().repeat(3) } else { q.to_string() };
            let mut j = 0;
            while j < c.len() && !starts(j, &close) {
                if c[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            if j >= c.len() {
                push(Tok::String, text);
                return (out, carry);
            }
            let end = j + close.chars().count();
            push(Tok::String, &c[..end].iter().collect::<String>());
            i = end;
            carry = Carry::None;
        }
        Carry::None => {}
    }
    // yaml/toml: a leading `key:` / `key =` / `[table]`.
    if sp.keyed && i == 0 {
        let trimmed_start = c.iter().take_while(|ch| ch.is_whitespace()).count();
        let rest: String = c[trimmed_start..].iter().collect();
        if rest.starts_with('[') && g.id == "toml" {
            push(Tok::Plain, &c[..trimmed_start].iter().collect::<String>());
            push(Tok::Keyword, &rest);
            return (out, Carry::None);
        }
        let key_end = c[trimmed_start..]
            .iter()
            .position(|ch| *ch == ':' || *ch == '=' || ch.is_whitespace() || *ch == '"' || *ch == '\'')
            .map(|p| p + trimmed_start);
        if let Some(k) = key_end {
            let mut after = k;
            while after < c.len() && c[after] == ' ' {
                after += 1;
            }
            if k > trimmed_start && after < c.len() && (c[after] == ':' || c[after] == '=') && !rest.starts_with('#') {
                push(Tok::Plain, &c[..trimmed_start].iter().collect::<String>());
                push(Tok::Function, &c[trimmed_start..k].iter().collect::<String>());
                i = k;
            }
        }
    }
    if g.id == "markdown" {
        let t = text.trim_start();
        let tok = if t.starts_with('#') || t.starts_with("```") || t.starts_with("~~~") {
            Tok::Keyword
        } else if t.starts_with("- ") || t.starts_with("* ") || t.starts_with("> ") {
            Tok::Punctuation
        } else {
            Tok::Plain
        };
        push(tok, text);
        return (out, Carry::None);
    }
    while i < c.len() {
        let ch = c[i];
        // Line comment: the rest of the line.
        if let Some(lc) = sp.line_comments.iter().find(|lc| starts(i, lc)) {
            // `#` in a shell word (`a#b`) is not a comment.
            let word_char_before = i > 0 && !c[i - 1].is_whitespace();
            if !(*lc == "#" && word_char_before && g.id == "shellscript") {
                push(Tok::Comment, &c[i..].iter().collect::<String>());
                return (out, Carry::None);
            }
        }
        if let Some((open, close)) = sp.block {
            if starts(i, open) {
                let from = i;
                let mut j = i + open.chars().count();
                while j < c.len() && !starts(j, close) {
                    j += 1;
                }
                if j >= c.len() {
                    push(Tok::Comment, &c[from..].iter().collect::<String>());
                    return (out, Carry::BlockComment);
                }
                let end = j + close.chars().count();
                push(Tok::Comment, &c[from..end].iter().collect::<String>());
                i = end;
                continue;
            }
        }
        // Long strings (python triple quotes, ts/go back-ticks).
        if let Some(q) = sp.long {
            let triple = g.id == "python";
            let opener: String = if triple { q.to_string().repeat(3) } else { q.to_string() };
            if starts(i, &opener) && (triple || ch == q) {
                let from = i;
                let mut j = i + opener.chars().count();
                while j < c.len() && !starts(j, &opener) {
                    if c[j] == '\\' {
                        j += 1;
                    }
                    j += 1;
                }
                if j >= c.len() {
                    push(Tok::String, &c[from..].iter().collect::<String>());
                    return (out, Carry::LongString(q));
                }
                let end = j + opener.chars().count();
                push(Tok::String, &c[from..end].iter().collect::<String>());
                i = end;
                continue;
            }
        }
        if sp.quotes.contains(&ch) {
            let from = i;
            let mut j = i + 1;
            while j < c.len() && c[j] != ch {
                if c[j] == '\\' {
                    j += 1;
                }
                j += 1;
            }
            let end = (j + 1).min(c.len());
            push(Tok::String, &c[from..end].iter().collect::<String>());
            i = end;
            continue;
        }
        if g.id == "html" && ch == '<' {
            // A tag name after `<` / `</`.
            let from = i;
            let mut j = i + 1;
            if j < c.len() && c[j] == '/' {
                j += 1;
            }
            while j < c.len() && (c[j].is_ascii_alphanumeric() || c[j] == '-') {
                j += 1;
            }
            push(Tok::Punctuation, &c[from..from + 1].iter().collect::<String>());
            push(Tok::Keyword, &c[from + 1..j].iter().collect::<String>());
            i = j;
            continue;
        }
        if ch.is_ascii_digit() && (i == 0 || !is_ident(c[i - 1])) {
            let from = i;
            let mut j = i;
            while j < c.len() && (c[j].is_ascii_alphanumeric() || c[j] == '.' || c[j] == '_') {
                j += 1;
            }
            push(Tok::Constant, &c[from..j].iter().collect::<String>());
            i = j;
            continue;
        }
        if is_ident_start(ch) || (ch == '#' && matches!(g.id, "c")) || (ch == '$' && g.id == "shellscript") {
            let from = i;
            let mut j = i + 1;
            while j < c.len() && is_ident(c[j]) {
                j += 1;
            }
            let word: String = c[from..j].iter().collect();
            let lower = word.to_ascii_lowercase();
            let is_kw = if g.id == "sql" {
                sp.keywords.contains(&lower.as_str())
            } else {
                sp.keywords.contains(&word.as_str())
            };
            let tok = if word.starts_with('$') {
                Tok::Constant
            } else if is_kw {
                Tok::Keyword
            } else if sp.constants.contains(&word.as_str()) || (g.id == "sql" && sp.constants.contains(&lower.as_str())) {
                Tok::Constant
            } else if j < c.len() && (c[j] == '(' || (g.id == "rust" && c[j] == '!')) {
                Tok::Function
            } else {
                Tok::Plain
            };
            push(tok, &word);
            i = j;
            continue;
        }
        if ch.is_ascii_punctuation() {
            push(Tok::Punctuation, &ch.to_string());
        } else {
            push(Tok::Plain, &ch.to_string());
        }
        i += 1;
    }
    (out, carry)
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Lex a whole block: one span list per line. `None` grammar = plain.
pub fn block(g: Option<Grammar>, code: &str) -> Vec<Vec<(Tok, String)>> {
    let mut carry = Carry::None;
    code.split('\n')
        .map(|l| match g {
            Some(g) => {
                let (spans, next) = line(g, l, carry);
                carry = next;
                spans
            }
            None => {
                if l.is_empty() {
                    Vec::new()
                } else {
                    vec![(Tok::Plain, l.to_owned())]
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_web_language_table_and_its_aliases() {
        for (alias, id) in [("ts", "typescript"), ("JSX", "typescript"), ("zsh", "shellscript"), ("yml", "yaml"), ("rs", "rust"), ("md", "markdown"), ("jsonc", "json")] {
            assert_eq!(grammar(Some(alias)).map(|g| g.id), Some(id), "{alias}");
        }
        // A missing grammar falls back to plain, copyable code (e2e
        // final-reading.spec.ts:583).
        assert_eq!(grammar(Some("brainfuck")), None);
        assert_eq!(grammar(None), None);
        let plain = block(None, "a < b\n\nc");
        assert_eq!(plain, vec![vec![(Tok::Plain, "a < b".to_owned())], vec![], vec![(Tok::Plain, "c".to_owned())]]);
    }

    #[test]
    fn rust_lines_lex_into_the_web_token_classes() {
        let g = grammar(Some("rust")).unwrap();
        let (spans, carry) = line(g, "fn main() { let x = \"hi\"; // done", Carry::None);
        assert_eq!(carry, Carry::None);
        let kinds: Vec<Tok> = spans.iter().map(|(t, _)| *t).collect();
        assert!(kinds.contains(&Tok::Keyword));
        assert!(spans.iter().any(|(t, s)| *t == Tok::Function && s == "main"));
        assert!(spans.iter().any(|(t, s)| *t == Tok::String && s == "\"hi\""));
        assert!(spans.iter().any(|(t, s)| *t == Tok::Comment && s == "// done"));
        // Every character survives (copyable code is the same text).
        let joined: String = spans.iter().map(|(_, s)| s.as_str()).collect();
        assert_eq!(joined, "fn main() { let x = \"hi\"; // done");
    }

    #[test]
    fn block_comments_and_long_strings_carry_across_lines() {
        let g = grammar(Some("ts")).unwrap();
        let lines = block(Some(g), "/* a\nb */ const t = `x\ny`;");
        assert_eq!(lines[0], vec![(Tok::Comment, "/* a".to_owned())]);
        assert_eq!(lines[1][0], (Tok::Comment, "b */".to_owned()));
        assert!(lines[1].iter().any(|(t, s)| *t == Tok::String && s == "`x"));
        assert_eq!(lines[2][0], (Tok::String, "y`".to_owned()));
        let py = block(grammar(Some("python")), "s = \"\"\"doc\nmore\"\"\" # c");
        assert_eq!(py[1][0], (Tok::String, "more\"\"\"".to_owned()));
        assert!(py[1].iter().any(|(t, s)| *t == Tok::Comment && s == "# c"));
    }

    #[test]
    fn keyed_formats_colour_their_keys() {
        let toml = block(grammar(Some("toml")), "[workspace]\nkind = \"session\"");
        assert_eq!(toml[0], vec![(Tok::Keyword, "[workspace]".to_owned())]);
        assert_eq!(toml[1][0], (Tok::Function, "kind".to_owned()));
        assert!(toml[1].iter().any(|(t, s)| *t == Tok::String && s == "\"session\""));
    }
}
