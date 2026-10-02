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
    // The Skills dialog's warning in its native wording ("on this device"
    // for the web's "in your browser", SkillsDialog.tsx:158-162): the web's
    // own Chinese with 此设备 for 浏览器.
    (
        "Skills are shared by this Profile, not installed on this device. Installation may download executable \
         tools and dependencies. Review and trust the source first.",
        "技能由此配置档案共享，不会安装到此设备。安装可能下载可执行工具和依赖。请先审查并信任来源。",
        "screens/dialog.rs SKILLS_WARNING (the web's zh.ts key with 'in your browser')",
    ),
    // ---- A30: the sidebar peer dock (parity row 270; board 4 regions 6/7 and
    // README "Row 270"). The collapsed pill rewrites the web's
    // `formatPeerDockPill` ("3 · 1 live · 1/3 landed · 1 blocked", which the
    // web never translates) in the Fleet's words (工作中 / 已完成, FLEET_ZH);
    // the board names the web's "Approve for this session" (peer-copy.ts:
    // 本次会话内批准) "Approve for session"; the control chain's three
    // fail-closed labels (`fleet_driver::row_control`) read like the Fleet's
    // "Take control of {value0} to do this" (需要先取得 {value0} 的控制权).
    ("{value0} working", "{value0} 个工作中", "board 4 region 7, the pill's working count"),
    ("{value0} waiting", "{value0} 个等待中", "board 4 region 7, the pill's waiting count"),
    ("{value0}/{value1} finished", "{value0}/{value1} 已完成", "board 4 region 7, the pill's finished of total"),
    ("Approve for session", "本次会话内批准", "board 4 region 6, the threaded card's link (the web's 'Approve for this session')"),
    ("This peer is no longer in the roster.", "此同侪已不在名单中。", "fleet_driver::row_control's fail-closed label"),
    ("Take control of this session to do this", "需要先取得此会话的控制权", "fleet_driver::row_control's fail-closed label"),
    ("That action is not available right now.", "此操作当前不可用。", "fleet_driver::row_control's fail-closed label"),
    // ---- A28: the diff review (parity row 23; board 4 frames 1 / 1b / 2,
    // README "Row 23"; screens/board3/diff_review.rs). Terms as the web's
    // zh: diff 差异, preview 预览, review 审查, plain text 纯文本.
    (
        "Large preview shown as plain text. All lines are included.",
        "大型预览以纯文本显示，已包含所有行。",
        "DiffReviewDialog.tsx:107-109 plainNotice (the web renders it without t()); board 4 frame 1b",
    ),
    ("No diff preview yet", "尚无差异预览", "A10: the header Review entry before any preview id"),
    (
        "A preview appears here once this Session proposes file changes, such as an approval that edits files.",
        "此会话提出文件更改（例如需要批准的文件编辑）后，预览会显示在这里。",
        "A10: the no-preview state's body",
    ),
    ("This server does not provide diff previews.", "此服务器不提供差异预览。", "A10: a server without diff/preview/get"),
    ("Code review…", "代码审查…", "A10: the no-preview state's way to the Code review dialog (/review)"),
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
