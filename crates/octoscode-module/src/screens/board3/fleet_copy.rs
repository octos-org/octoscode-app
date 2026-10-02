//! A10 — the Fleet's zh catalog (web `features/fleet/fleet-copy.ts`, every
//! key verbatim): English is the source key (the web's `t()` convention) and
//! no value carries protocol vocabulary (seat / epoch / lane / slug / fence /
//! operation id / binding — the catalog test rejects them, `fleet-copy.test.ts`).
//!
//! The language is the display preference's `language` (A24: `crate::i18n`,
//! set by Settings > Preferences > Language and adopted at launch from
//! `screens::a9_prefs`), so the Fleet switches live with every surface.

/// English source key → Simplified Chinese copy (`FLEET_ZH_COPY`).
pub const FLEET_ZH: &[(&str, &str)] = &[
    ("Fleet", "舰队"),
    ("Back to Fleet", "返回舰队"),
    ("No peers yet", "还没有同侪"),
    ("Start a peer", "启动一个同侪"),
    ("Model", "模型"),
    ("Brief", "任务简报"),
    ("Session", "会话"),
    ("Start", "启动"),
    ("Starting…", "正在启动…"),
    ("Couldn't start: {value0}", "无法启动：{value0}"),
    ("Settings › Providers", "设置 › 提供商"),
    ("Approve", "批准"),
    ("Deny", "拒绝"),
    ("Steer", "引导"),
    ("Steer {value0}", "引导 {value0}"),
    ("Approve for {value0}", "为 {value0} 批准"),
    ("Deny for {value0}", "为 {value0} 拒绝"),
    ("Stop {value0}", "停止 {value0}"),
    ("Only while the peer is running", "仅在同侪运行时可用"),
    ("Fleet live region", "舰队实时区域"),
    ("Stop", "停止"),
    ("Send", "发送"),
    ("Advanced", "高级"),
    ("Peers", "同侪"),
    ("Requested", "已请求"),
    ("Starting", "正在启动"),
    ("Still starting…", "仍在启动…"),
    ("Working", "工作中"),
    ("Waiting for your approval", "等待你的批准"),
    ("Waiting for your answer", "等待你的回答"),
    ("Finished", "已完成"),
    ("Stopped", "已停止"),
    ("Failed", "已失败"),
    ("Outcome unknown", "结果未知"),
    ("Sent", "已发送"),
    ("Stop requested", "已请求停止"),
    ("Already handled", "已被处理"),
    ("Peer started a new turn", "同侪开始了新的回合"),
    ("Take control of {value0} to do this", "需要先取得 {value0} 的控制权"),
    ("Couldn't restore this peer", "无法恢复这个同侪"),
    ("Retry", "重试"),
    ("Restoring…", "正在恢复…"),
    ("Peer started {value0}", "同侪启动于 {value0}"),
    ("Finished ({value0})", "已完成（{value0}）"),
    ("Loading peers for {value0} sessions", "正在为 {value0} 个会话加载同侪"),
    ("This server does not support remote control of peers", "此服务器不支持远程控制同侪"),
    ("This server does not support starting peers", "此服务器不支持启动同侪"),
    ("Peer controls are not ready", "同侪控制尚未就绪"),
    (
        "No peer models are configured — add one under Settings › Providers",
        "尚未配置同侪模型 — 请在 设置 › 提供商 中添加",
    ),
    ("Open a project first", "请先打开一个项目"),
    ("Loading models…", "正在加载模型…"),
    ("Not sure it started — Retry resends the same request.", "不确定是否已启动 — 重试会重发同一请求。"),
    ("Dismiss", "关闭"),
    ("Only while waiting for approval", "仅在等待批准时可用"),
    ("Only while waiting for your answer", "仅在等待你的回答时可用"),
    ("Only while working", "仅在工作中可用"),
    ("Enter steering text", "输入引导文字"),
    ("{value0} is waiting for your approval", "{value0} 正在等待你的批准"),
    ("{value0} is waiting for your answer", "{value0} 正在等待你的回答"),
    ("{value0} finished", "{value0} 已完成"),
    ("{value0} stopped", "{value0} 已停止"),
    ("{value0} failed", "{value0} 已失败"),
    ("Peer {value0}", "同侪 {value0}"),
    ("Session peers", "会话同侪"),
    ("Goal {value0}", "目标 {value0}"),
];

/// The gather copy the Fleet pane shares with the web's `/gather`
/// (`features/preferences/zh.ts:910-913`, verbatim); the two progress words
/// are the pane's own.
pub const GATHER_ZH: &[(&str, &str)] = &[
    ("Peer gather", "汇总协作结果"),
    ("No peers staged on the blackboard.", "黑板上没有已准备的协作会话。"),
    (
        "Peer synthesis was not queued. Check this Session’s authority and write availability, then retry.",
        "协作结果汇总未入队。请检查此会话的授权和写入权限后重试。",
    ),
    ("Gathering…", "正在汇总…"),
    ("Peer synthesis queued", "协作结果汇总已入队"),
    // The answer card (`PeerDock.tsx:371-440`): the web's "needs your
    // answer" / "Answer" keys (no zh value in its catalog), the field's
    // placeholder the pane's own.
    ("needs your answer", "需要你的回答"),
    ("Answer", "回答"),
    ("Type your answer", "输入你的回答"),
];

/// A30 — the peer dock's own copy (board 4 regions 6/7), the copy no web
/// table has: the collapsed pill rewrites the web's `formatPeerDockPill`
/// ("3 · 1 live · 1/3 landed · 1 blocked", never translated by the web) in
/// the Fleet's words, the board names "Approve for this session"
/// (`PeerDock.tsx:118`, peer-copy.ts) "Approve for session" (its zh is the
/// web's), and the control chain's three fail-closed labels
/// (`fleet_driver::row_control`) read in the same voice as the Fleet's
/// "Take control of {value0} to do this".
pub const DOCK_ZH: &[(&str, &str)] = &[
    ("{value0} working", "{value0} 个工作中"),
    ("{value0} waiting", "{value0} 个等待中"),
    ("{value0}/{value1} finished", "{value0}/{value1} 已完成"),
    ("Approve for session", "本次会话内批准"),
    ("This peer is no longer in the roster.", "此同侪已不在名单中。"),
    ("Take control of this session to do this", "需要先取得此会话的控制权"),
    ("That action is not available right now.", "此操作当前不可用。"),
];

/// A30 — the Fleet's native copy (this pane's [`GATHER_ZH`] and the dock's
/// [`DOCK_ZH`]): `i18n::zh_for`'s last fallback, after every web table.
pub fn native_zh(source: &str) -> Option<&'static str> {
    GATHER_ZH.iter().chain(DOCK_ZH).find(|(k, _)| *k == source).map(|(_, v)| *v)
}

/// The UI language: `zh` or `en` — A24: the ONE interface language
/// (`crate::i18n`, the display preference's `language`), so the Fleet
/// follows the Settings > Preferences switch live like every surface.
pub fn lang() -> &'static str {
    crate::i18n::language().code()
}

/// `t(source, {value0})` for `lang`: the web's merged catalog
/// (`crate::i18n`, which carries every `FLEET_ZH_COPY` entry verbatim), then
/// this pane's gather table.
pub fn t_in(lang: &str, source: &str, value0: Option<&str>) -> String {
    let text = if lang == "zh" {
        crate::i18n::zh_for(source)
            .or_else(|| GATHER_ZH.iter().find(|(k, _)| *k == source).map(|(_, v)| *v))
            .unwrap_or(source)
    } else {
        source
    };
    match value0 {
        Some(v) => text.replace("{value0}", v),
        None => text.to_owned(),
    }
}

/// `t(source)` in the current language.
pub fn t(source: &str) -> String {
    t_in(lang(), source, None)
}

/// `t(source, {value0})` in the current language.
pub fn t1(source: &str, value0: &str) -> String {
    t_in(lang(), source, Some(value0))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `fleet-copy.test.ts:12-33`: every key has non-empty zh text, every
    /// placeholder survives, and no value carries protocol vocabulary.
    #[test]
    fn the_catalog_covers_every_key_without_protocol_vocabulary() {
        assert_eq!(FLEET_ZH.len(), 65);
        for (k, v) in FLEET_ZH {
            assert!(!v.trim().is_empty(), "{k}");
            if k.contains("{value0}") {
                assert!(v.contains("{value0}"), "{k} keeps its placeholder");
            }
            for banned in ["seat", "epoch", "lane", "slug", "fence", "operation id", "binding"] {
                assert!(!v.to_ascii_lowercase().contains(banned), "{k}: {v}");
            }
        }
        assert_eq!(t_in("zh", "Still starting…", None), "仍在启动…");
        assert_eq!(t_in("zh", "Finished ({value0})", Some("2")), "已完成（2）");
        assert_eq!(t_in("en", "Finished ({value0})", Some("2")), "Finished (2)");
        assert_eq!(t_in("zh", "not in the catalog", None), "not in the catalog");
        assert_eq!(t_in("zh", "Peer gather", None), "汇总协作结果");
    }

    /// A30 — the dock's native copy: every placeholder survives, no protocol
    /// vocabulary, never a web key (a web key would make the entry dead), and
    /// `tr()` reaches it (its last fallback) while English stays the source.
    #[test]
    fn the_dock_copy_is_reachable_through_tr_and_never_shadows_a_web_key() {
        use crate::i18n::{catalog, text_in, tr_in, zh, Lang};
        for (k, v) in DOCK_ZH.iter().chain(GATHER_ZH) {
            assert!(!v.trim().is_empty(), "{k}");
            for p in ["{value0}", "{value1}"] {
                assert_eq!(k.contains(p), v.contains(p), "{k} keeps {p}");
            }
            for banned in ["seat", "epoch", "lane", "slug", "fence", "operation id", "binding"] {
                assert!(!v.to_ascii_lowercase().contains(banned), "{k}: {v}");
            }
            assert!(!catalog().contains_key(k), "{k} is a web key: the catalog wins, the entry is dead");
            assert!(zh::PEER_ZH.iter().all(|(w, _)| w != k), "{k} is a peer-copy key");
            assert_eq!(tr_in(Lang::Zh, k), *v);
            assert_eq!(tr_in(Lang::En, k), *k);
        }
        assert_eq!(text_in(Lang::Zh, "{value0}/{value1} finished", &[("value0", "1"), ("value1", "3")]), "1/3 已完成");
        // "Approve for session" is the board's wording of the web's own key.
        let web = zh::PEER_ZH.iter().find(|(k, _)| *k == "Approve for this session").unwrap().1;
        assert_eq!(tr_in(Lang::Zh, "Approve for session"), web);
    }
}
