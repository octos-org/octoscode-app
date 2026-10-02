//! Native copy with NO web counterpart, translated here.
//!
//! [`super::zh`] is the web's own catalog and [`super::alias`] maps native
//! wording onto a web key for the SAME control. Board 4 (approved by the
//! operator 2026-10-02, `design/stage-a/phase4-new4/README.md`) adds surfaces
//! the web has no screen for at all, so their copy has no web key to borrow;
//! the task that builds such a surface asks for its Chinese ("strings
//! through tr() with zh"). Each entry here is that surface's English source,
//! its Simplified Chinese, and where the English comes from. The web catalog
//! always wins: a test proves no entry is a web key or an alias (it would be
//! dead), that both sides carry the same placeholders, and that every value
//! is Chinese. Wording follows the web catalog's own terms (会话 Session,
//! 运行中 running, 排队中 queued, 已停止 / 已失败 / 已完成).
use std::collections::HashMap;
use std::sync::OnceLock;

/// (English source, Chinese, source of the English).
pub static NATIVE_ZH: &[(&str, &str, &str)] = &[
    // ---- A31: the Skills dialog's "Background jobs" (parity row 15; board 4
    // region 3 and README "Row 15: skill-job status"; the web's
    // SkillsDialog.tsx has no job UI). "Job" is 作业, apart from the web's
    // background TASKS (后台任务), which are another surface.
    ("Background jobs", "后台作业", "board 4 region 3, the section title"),
    // The header count's queued half: the web's "{count} queued" counts
    // queued PROMPTS (条); jobs take 个, like the web's "{count} 个运行中".
    ("{value0} queued", "{value0} 个排队中", "board 4 region 3, the header count"),
    ("Queued", "排队中", "README row 15 status table: queued -> ○ Queued"),
    ("Couldn't finish this job.", "无法完成此作业。", "README row 15 status table: failed"),
    (
        "The server restarted before this job finished.",
        "服务器在此作业完成前已重启。",
        "README row 15 status table: abandoned",
    ),
    ("No background jobs in this Session.", "此会话没有后台作业。", "A31 empty state"),
    ("Loading background jobs…", "正在加载后台作业…", "A31 loading state"),
    ("Couldn't load background jobs.", "无法加载后台作业。", "A31 list failure"),
    (
        "Only jobs announced since the app connected are shown; this server doesn't list earlier jobs.",
        "仅显示应用连接后通知的作业；此服务器不提供更早作业的列表。",
        "A31: a server without skill.action_jobs.v1 (operator default)",
    ),
];

fn table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| NATIVE_ZH.iter().map(|(en, zh, _)| (*en, *zh)).collect())
}

/// The native Chinese for `source`, if it is native copy.
pub fn zh(source: &str) -> Option<&'static str> {
    table().get(source).copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::{alias, catalog, tr_in, Lang};

    fn placeholders(s: &str) -> Vec<String> {
        let mut out: Vec<String> = s
            .split('{')
            .skip(1)
            .filter_map(|p| p.split_once('}').map(|(name, _)| name.to_owned()))
            .collect();
        out.sort();
        out
    }

    #[test]
    fn every_native_entry_is_new_copy_with_chinese_and_its_placeholders() {
        let mut seen = std::collections::HashSet::new();
        for (en, zh, source) in NATIVE_ZH {
            assert!(seen.insert(*en), "duplicate {en:?}");
            assert!(!source.is_empty());
            assert!(!catalog().contains_key(en), "{en:?} is a web key: use the web's translation");
            assert!(alias::web_key(en).is_none(), "{en:?} is an alias of a web key");
            assert_eq!(placeholders(en), placeholders(zh), "{en:?}");
            assert!(zh.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)), "{en:?} -> {zh:?} is not Chinese");
            assert_eq!(tr_in(Lang::Zh, en), *zh);
            assert_eq!(tr_in(Lang::En, en), *en);
        }
    }
}
