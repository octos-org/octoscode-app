//! A10 — the Fleet's zh catalog (web `features/fleet/fleet-copy.ts`, every
//! key verbatim): English is the source key (the web's `t()` convention) and
//! no value carries protocol vocabulary (seat / epoch / lane / slug / fence /
//! operation id / binding — the catalog test rejects them, `fleet-copy.test.ts`).
//!
//! The language is the persisted display preference's `language`
//! (`{version, theme, language, vimMode}` — the web's
//! `octoscode.web.display.v1`, natively `$HOME/.octoscode/display.json`,
//! `screens::theme`), with `OCTOSCODE_UI_LANG` as the capture override. The
//! native build has no control that sets the language yet (preferences
//! surface: not this lane), so `en` is the default.

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
];

/// The UI language: `zh` or `en`.
pub fn lang() -> &'static str {
    if let Ok(v) = std::env::var("OCTOSCODE_UI_LANG") {
        return if v.trim().to_ascii_lowercase().starts_with("zh") { "zh" } else { "en" };
    }
    let path = std::env::var("OCTOSCODE_PREF_PATH").map(std::path::PathBuf::from).unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        std::path::Path::new(&home).join(".octoscode").join("display.json")
    });
    let stored = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v.get("language").and_then(|l| l.as_str()).map(str::to_owned));
    match stored.as_deref() {
        Some(l) if l.starts_with("zh") => "zh",
        _ => "en",
    }
}

/// `t(source, {value0})` for `lang`.
pub fn t_in(lang: &str, source: &str, value0: Option<&str>) -> String {
    let text = if lang == "zh" {
        FLEET_ZH.iter().chain(GATHER_ZH).find(|(k, _)| *k == source).map(|(_, v)| *v).unwrap_or(source)
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
}
