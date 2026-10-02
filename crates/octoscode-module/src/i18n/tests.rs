//! A24 — the catalog is the web's, `tr()` behaves like the web's
//! `createUiText`, and the device default follows the locale.
use super::*;

// The web's own files (verbatim copies, tools/i18n/web/SOURCE.json).
const ZH_TS: &str = include_str!("../../../../tools/i18n/web/features/preferences/zh.ts");
const FLEET_TS: &str = include_str!("../../../../tools/i18n/web/features/fleet/fleet-copy.ts");
const REASONING_TS: &str = include_str!("../../../../tools/i18n/web/features/reasoning/reasoning-copy.ts");
const SESSION_CONFIG_TS: &str =
    include_str!("../../../../tools/i18n/web/features/session-config/session-config-copy.ts");

/// An independent reading of one `const <binding> = {…}` (or
/// `Object.freeze({…})`) literal: JS semantics — a repeated key keeps its
/// first position and its last value.
fn web_table(src: &str, binding: &str) -> Vec<(String, String)> {
    let decl = format!("const {binding}");
    let at = src.find(&decl).unwrap_or_else(|| panic!("no `{decl}`"));
    let eq = at + src[at..].find('=').expect("=");
    let open = eq + src[eq..].find('{').expect("{");
    let b = src.as_bytes();
    let mut i = open + 1;
    let mut rows: Vec<(String, String)> = Vec::new();
    let skip = |i: &mut usize| loop {
        while *i < b.len() && (b[*i] as char).is_whitespace() {
            *i += 1;
        }
        if src[*i..].starts_with("//") {
            *i += src[*i..].find('\n').unwrap_or(src.len() - *i);
        } else if src[*i..].starts_with("/*") {
            *i += src[*i..].find("*/").expect("*/") + 2;
        } else {
            break;
        }
    };
    let string = |i: &mut usize| -> String {
        let q = b[*i];
        assert!(q == b'"' || q == b'\'', "a string at {}", &src[*i..*i + 20]);
        *i += 1;
        let mut out = String::new();
        loop {
            let c = src[*i..].chars().next().expect("unterminated");
            *i += c.len_utf8();
            if c as u32 == q as u32 {
                return out;
            }
            if c == '\\' {
                let e = src[*i..].chars().next().unwrap();
                *i += e.len_utf8();
                match e {
                    'n' => out.push('\n'),
                    't' => out.push('\t'),
                    'u' => {
                        let hex = &src[*i..*i + 4];
                        out.push(char::from_u32(u32::from_str_radix(hex, 16).unwrap()).unwrap());
                        *i += 4;
                    }
                    other => out.push(other),
                }
            } else {
                out.push(c);
            }
        }
    };
    loop {
        skip(&mut i);
        if b[i] == b'}' {
            break;
        }
        let key = if b[i] == b'"' || b[i] == b'\'' {
            string(&mut i)
        } else {
            let start = i;
            while (b[i] as char).is_alphanumeric() || b[i] == b'_' || b[i] == b'$' {
                i += 1;
            }
            src[start..i].to_owned()
        };
        skip(&mut i);
        assert_eq!(b[i], b':', "{key}: a colon");
        i += 1;
        skip(&mut i);
        let mut value = string(&mut i);
        skip(&mut i);
        while b[i] == b'+' {
            i += 1;
            skip(&mut i);
            value.push_str(&string(&mut i));
            skip(&mut i);
        }
        match rows.iter_mut().find(|(k, _)| *k == key) {
            Some(row) => row.1 = value,
            None => rows.push((key, value)),
        }
        if b[i] == b',' {
            i += 1;
        }
    }
    rows
}

fn owned(t: &[(&str, &str)]) -> Vec<(String, String)> {
    t.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
}

/// The native catalog is generated from the web's zh.ts (and the tables the
/// web's loader merges): every table, every entry, the web's order.
#[test]
fn the_native_catalog_matches_the_web_zh_ts() {
    assert_eq!(owned(zh::ZH_BASE), web_table(ZH_TS, "catalog"), "zh.ts catalog");
    assert_eq!(owned(zh::FLEET_ZH), web_table(FLEET_TS, "FLEET_ZH_COPY"), "fleet-copy.ts");
    assert_eq!(owned(zh::REASONING_ZH), web_table(REASONING_TS, "REASONING_ZH_COPY"), "reasoning-copy.ts");
    assert_eq!(
        owned(zh::SESSION_CONFIG_ZH),
        web_table(SESSION_CONFIG_TS, "SESSION_CONFIG_ZH_COPY"),
        "session-config-copy.ts"
    );
    // zh.ts spreads the Fleet table LAST; the loader merges reasoning, then
    // session-config (ui-text.tsx:42-67): a later table wins.
    assert!(ZH_TS.contains("{ ...catalog, ...FLEET_ZH_COPY }"));
    let mut want: HashMap<String, String> = HashMap::new();
    for table in [
        web_table(ZH_TS, "catalog"),
        web_table(FLEET_TS, "FLEET_ZH_COPY"),
        web_table(REASONING_TS, "REASONING_ZH_COPY"),
        web_table(SESSION_CONFIG_TS, "SESSION_CONFIG_ZH_COPY"),
    ] {
        want.extend(table);
    }
    let got: HashMap<String, String> =
        catalog().iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect();
    assert_eq!(got.len(), zh::MERGED_LEN);
    assert_eq!(got, want);
    assert_eq!(zh::MERGED_LEN, 1167, "the web catalog at {}", zh::WEB_COMMIT);
}

fn placeholders(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(o) = rest.find('{') {
        let after = &rest[o + 1..];
        match after.find('}') {
            Some(c) if c > 0 && after[..c].chars().all(|ch| ch.is_alphanumeric() || ch == '_') => {
                out.push(after[..c].to_owned());
                rest = &after[c + 1..];
            }
            _ => rest = after,
        }
    }
    out.sort();
    out
}

/// `fleet-copy-merge.test.ts:33` / `localized-features.test.tsx:31`.
#[test]
fn every_translation_keeps_its_placeholders() {
    for (source, translated) in catalog() {
        assert_eq!(placeholders(translated), placeholders(source), "{source}");
    }
}

/// `fleet-copy-merge.test.ts:17-31`: the Fleet values win collisions.
#[test]
fn the_fleet_table_wins_its_collisions() {
    for (source, translated) in zh::FLEET_ZH {
        assert_eq!(catalog().get(source), Some(translated), "{source}");
    }
    assert_eq!(tr_in(Lang::Zh, "No peers yet"), "还没有同侪");
    assert_eq!(tr_in(Lang::Zh, "Session peers"), zh::FLEET_ZH.iter().find(|(k, _)| *k == "Session peers").unwrap().1);
    assert_eq!(text_in(Lang::Zh, "Peer {value0}", &[("value0", "a")]), "同侪 a");
}

/// `fleet-copy.test.ts:33`: no Fleet value carries protocol vocabulary
/// (row 117's "Chinese product copy with no protocol vocabulary").
#[test]
fn fleet_copy_carries_no_protocol_vocabulary() {
    assert!(zh::FLEET_ZH.len() > 20);
    for (source, translated) in zh::FLEET_ZH {
        assert!(!translated.trim().is_empty(), "{source}");
        let lower = translated.to_ascii_lowercase();
        for banned in ["seat", "epoch", "lane", "slug", "fence", "operation id", "binding"] {
            assert!(!lower.contains(banned), "{source}: {translated}");
        }
    }
}

/// `createUiText` (`localized-features.test.tsx:36-44`).
#[test]
fn tr_behaves_like_the_web_create_ui_text() {
    assert_eq!(text_in(Lang::Zh, "Session {id}", &[("id", "A#peer")]), "会话 A#peer");
    assert_eq!(text_in(Lang::En, "Session {id}", &[("id", "A#peer")]), "Session A#peer");
    assert_eq!(text_in(Lang::Zh, "Unknown native error", &[]), "Unknown native error");
    // A token with no param stays as written; no params, no replacement.
    assert_eq!(text_in(Lang::En, "{a} and {b}", &[("a", "1")]), "1 and {b}");
    assert_eq!(text_in(Lang::En, "{a}", &[]), "{a}");
    assert_eq!(interpolate("x {} {value0} {", &[("value0", "v")]), "x {} v {");
    assert_eq!(tr_in(Lang::Zh, "Settings"), "设置");
    assert_eq!(tr_in(Lang::En, "Settings"), "Settings");
}

/// A key the catalog lacks falls back to the English source.
#[test]
fn tr_falls_back_to_english_for_a_missing_key() {
    let missing = "A string the web never translated";
    assert!(!catalog().contains_key(missing));
    assert_eq!(tr_in(Lang::Zh, missing), missing);
    assert_eq!(text_in(Lang::Zh, "Nothing like {value0}", &[("value0", "x")]), "Nothing like x");
}

/// The current language drives `tr`, and a switch bumps the generation
/// (what makes the host re-render).
#[test]
fn a_language_switch_changes_tr_and_the_generation() {
    set_language(Lang::En);
    let g0 = generation();
    assert_eq!(tr("Settings"), "Settings");
    assert!(set_language(Lang::Zh));
    assert!(generation() > g0);
    assert_eq!(tr("Settings"), "设置");
    assert_eq!(tr1("Background tasks: {value0}", "3"), "后台任务：3");
    let g1 = generation();
    assert!(!set_language(Lang::Zh), "no change, no re-render");
    assert_eq!(generation(), g1);
    assert!(set_language(Lang::En));
    assert_eq!(tr("Settings"), "Settings");
}

/// model.ts:102-104: `/^zh(?:-|_|$)/i` is Chinese, everything else English.
#[test]
fn the_default_language_follows_the_device_locale() {
    for zh in ["zh", "zh-CN", "zh_TW", "zh-Hans-CN", "ZH-cn", "zh_CN.UTF-8", " zh-Hant "] {
        assert_eq!(Lang::for_locale(zh), Lang::Zh, "{zh}");
    }
    for en in ["", "C", "en", "en-US", "en_US.UTF-8", "zhx", "fr-FR", "z"] {
        assert_eq!(Lang::for_locale(en), Lang::En, "{en}");
    }
    assert_eq!(Lang::parse("zh"), Some(Lang::Zh));
    assert_eq!(Lang::parse("zh-CN"), None, "isUiLanguage is exact");
    assert_eq!(device_language(), Lang::for_locale(&device_locale()));
}

/// The live switch re-renders: after a switch the next lowering of each
/// surface is in the new language and its DSL differs, so the mount cache
/// (which compares the DSL) remounts it; switching back restores English.
#[test]
fn a_switch_re_renders_the_lowered_surfaces() {
    let m = crate::conv_layout::Metrics::for_window(990.0, true);
    let card = || crate::fluid::connect_card(&crate::fluid::ConnectView::default(), &m, 0.0);
    let kit_header = || {
        let mut d = crate::screens::board3::ui::Dsl::new();
        crate::screens::board3::ui::header(&mut d, "Session settings", "b3.close");
        d.button("b3_cancel", "Cancel", "b3.close", crate::screens::board3::ui::Btn::Outline, crate::screens::board3::ui::W::Fit, 36.0);
        d.finish()
    };
    let ledger = "copy t_title_text { class: user-copy, en: \"Settings\" }\ncopy t02_text { class: user-copy, en: \"git push origin main\" }\n";
    set_language(Lang::En);
    let (card_en, kit_en) = (card(), kit_header());
    assert!(card_en.contains("\"Connect to Octos\"") && card_en.contains("\"Access token\""));
    assert_eq!(crate::l0_host::localize(ledger), ledger, "English: the authored copy as is");
    assert!(set_language(Lang::Zh));
    let (card_zh, kit_zh) = (card(), kit_header());
    assert_ne!(card_en, card_zh, "a new DSL: the mount cache remounts the card");
    assert!(card_zh.contains("\"连接 Octos\"") && card_zh.contains("\"认证令牌\""), "the web's own copy");
    assert!(kit_zh.contains("\"会话设置\"") && kit_zh.contains("\"取消\""), "the kit's shared chrome");
    let localized = crate::l0_host::localize(ledger);
    assert!(localized.contains("en: \"设置\""), "{localized}");
    assert!(localized.contains("en: \"git push origin main\""), "data is never translated");
    assert!(set_language(Lang::En));
    assert_eq!(card(), card_en, "switching back restores English");
    assert_eq!(kit_header(), kit_en);
}

/// Times read in the interface language: the sidebar's buckets (the web's
/// `formatRelativeTime`) and the short date (its Intl month + day).
#[test]
fn times_read_in_the_interface_language() {
    use crate::screens::board3::ui::{rel_ago, short_date};
    use crate::screens::sidebar::relative_label;
    const DAY: u64 = 86_400_000;
    let oct1 = 1_790_812_800_000; // 2026-10-01T00:00:00Z
    set_language(Lang::En);
    assert_eq!(relative_label(oct1, oct1 + 3 * DAY), "3d");
    assert_eq!(relative_label(oct1, oct1 + 30_000), "now");
    assert_eq!(short_date(oct1), "Oct 1");
    assert!(set_language(Lang::Zh));
    assert_eq!(relative_label(oct1, oct1 + 3 * DAY), "3 天前");
    assert_eq!(relative_label(oct1, oct1 + 5 * 60_000), "5 分钟前");
    assert_eq!(relative_label(oct1, oct1 + 30_000), "刚刚");
    assert_eq!(relative_label(oct1, oct1 + 9 * DAY), "10月1日");
    assert_eq!(short_date(oct1), "10月1日");
    assert_eq!(rel_ago(oct1 + 2 * 3_600_000, oct1), "2 小时前");
    assert!(set_language(Lang::En));
}

/// The native-only supplement never shadows the web: a key the web catalog
/// (or an alias) translates must not be here — the web's wording wins, and
/// the day the web adds a key this test names the entry to delete.
#[test]
fn the_native_supplement_never_shadows_the_web() {
    let mut seen = std::collections::HashSet::new();
    for (en, zh) in native::NATIVE_ZH {
        assert!(seen.insert(*en), "duplicate native key {en:?}");
        assert!(web_zh(en).is_none(), "{en:?} has the web's Chinese ({:?}): drop the native entry", web_zh(en));
        assert!(alias::web_key(en).is_none(), "{en:?} is an alias");
        assert_eq!(tr_in(Lang::Zh, en), *zh, "{en:?} resolves to the native entry");
        assert_eq!(tr_in(Lang::En, en), *en);
    }
}

/// Every native value keeps its placeholders and reads Chinese.
#[test]
fn every_native_entry_keeps_its_placeholders_and_is_chinese() {
    for (en, zh) in native::NATIVE_ZH {
        assert_eq!(placeholders(zh), placeholders(en), "{en:?}");
        assert!(
            zh.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)),
            "{en:?} -> {zh:?}: no Chinese"
        );
        assert!(!zh.trim().is_empty() && zh.trim() == *zh || zh.starts_with('\u{b7}'), "{en:?}: stray whitespace");
    }
}

/// The native copy uses the web's own vocabulary (`native::GLOSSARY`, the
/// web catalog's rendering of each term): a native entry naming "Session"
/// says 会话, "Profile" 配置档案, "workspace" 工作区, …
#[test]
fn the_native_copy_uses_the_web_vocabulary() {
    fn has_word(text: &str, term: &str) -> bool {
        // A `{placeholder}` name is not a word of the copy.
        let mut words = String::new();
        let mut depth = 0;
        for c in text.chars() {
            match c {
                '{' => depth += 1,
                '}' if depth > 0 => depth -= 1,
                _ if depth == 0 => words.push(c),
                _ => words.push(' '),
            }
        }
        let lower = words.to_lowercase();
        let mut from = 0;
        while let Some(at) = lower[from..].find(term).map(|n| n + from) {
            let before = lower[..at].chars().next_back();
            let after_at = at + term.len();
            let rest = &lower[after_at..];
            let rest = rest.strip_prefix("es").or_else(|| rest.strip_prefix('s')).unwrap_or(rest);
            let after = rest.chars().next();
            let word = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
            if !word(before) && !word(after) {
                return true;
            }
            from = at + 1;
        }
        false
    }
    let mut bad = Vec::new();
    for (en, zh) in native::NATIVE_ZH {
        for (term, renderings) in native::GLOSSARY {
            if !has_word(en, term) || native::GLOSSARY_EXEMPT.contains(&(*en, *term)) {
                continue;
            }
            if !renderings.iter().any(|r| zh.contains(r)) {
                bad.push(format!("{en:?} -> {zh:?}: {term:?} should read {renderings:?}"));
            }
        }
    }
    assert!(bad.is_empty(), "native copy off the web's vocabulary:\n{}", bad.join("\n"));
    // The glossary itself is the web's: each rendering occurs in the catalog.
    for (term, renderings) in native::GLOSSARY {
        for r in *renderings {
            assert!(catalog().values().any(|v| v.contains(r)), "{term}: {r} is not the web's vocabulary");
        }
    }
}

/// One string, two spellings: native copy with typographic quotes finds the
/// web key written with straight ones (and only that — no other folding).
#[test]
fn typographic_quotes_find_the_straight_quoted_web_key() {
    let web = "That path can't be browsed.";
    let zh = catalog().get(web).copied().expect("a web key with a straight apostrophe");
    assert_eq!(tr_in(Lang::Zh, "That path can\u{2019}t be browsed."), zh);
    assert_eq!(tr_in(Lang::En, "That path can\u{2019}t be browsed."), "That path can\u{2019}t be browsed.");
    assert_eq!(tr_in(Lang::Zh, "That path can\u{2019}t be browsed"), "That path can\u{2019}t be browsed", "no other folding");
}

/// Every alias renders a real web key's translation, keeps the native
/// string's placeholders, and is not itself a web key (that would be dead).
#[test]
fn every_alias_is_a_web_key_of_the_same_shape() {
    let mut seen = std::collections::HashSet::new();
    for a in alias::ALIASES {
        assert!(seen.insert(a.native), "duplicate alias {}", a.native);
        assert!(catalog().contains_key(a.web), "{} -> {}: not a web key", a.native, a.web);
        assert!(!catalog().contains_key(a.native), "{} is itself a web key", a.native);
        assert_eq!(placeholders(a.native), placeholders(a.web), "{}", a.native);
        assert!(a.cite.starts_with("features/") || a.cite.starts_with("app/") || a.cite.starts_with("ui/"), "{}", a.cite);
        assert_eq!(tr_in(Lang::Zh, a.native), catalog()[a.web]);
        assert_eq!(tr_in(Lang::En, a.native), a.native);
    }
}
