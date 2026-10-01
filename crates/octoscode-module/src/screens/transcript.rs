//! P4a3 — transcript export: the composer's "copy as Markdown" path, ported
//! field-by-field from the web oracle
//! (`apps/web/src/features/transcript-export/conversation-markdown.ts:25-89`,
//! `copy-conversation.ts`): the chat ONLY (user/assistant text), read from
//! the server's canonical `session/hydrate` history (never the bounded
//! rendered timeline), with the header, the incomplete-history disclosure and
//! the empty/foreign-session outcomes.
use serde_json::{json, Value};

/// The hydrated message slice this reads (the fields the web's
/// `HydratedMessage` carries and the markdown uses).
fn message_role(m: &Value) -> Option<String> {
    m.get("role").and_then(|v| v.as_str()).map(str::to_owned)
}

fn message_text(m: &Value) -> Option<String> {
    m.get("content").and_then(|v| v.as_str()).map(str::to_owned)
}

fn message_seq(m: &Value) -> Option<i64> {
    m.get("seq").and_then(|v| v.as_i64())
}

/// Drop leading/trailing blank lines but keep indentation (the web's
/// `trimBlankLines`, conversation-markdown.ts:65).
fn trim_blank_lines(text: &str) -> String {
    let trimmed = text.trim_matches(|c| c == '\n' || c == '\r');
    let cut = trimmed.trim_end();
    // Leading blank lines only — the web replaces a leading `\s*\n` once.
    let out = cut.strip_prefix('\n').unwrap_or(cut);
    out.to_owned()
}

/// The markdown heading: explicit title, else the workspace leaf name,
/// else "Conversation" (conversation-markdown.ts:73-81).
pub fn heading_title(title: Option<&str>, workspace_root: Option<&str>) -> String {
    if let Some(t) = title.map(str::trim).filter(|t| !t.is_empty()) {
        return t.to_owned();
    }
    if let Some(w) = workspace_root {
        let w = w.trim_end_matches(['/', '\\']);
        if let Some(leaf) = w.rsplit(['/', '\\']).next() {
            if !leaf.is_empty() {
                return leaf.to_owned();
            }
        }
    }
    "Conversation".to_owned()
}

/// `> Incomplete: …` when the server returned fewer rows than the canonical
/// seq span they cover (conversation-markdown.ts:84-89). Messages are a
/// window, not necessarily sorted — use min/max seq.
pub fn incomplete_disclosure(messages: &[Value]) -> Option<String> {
    let first = messages.iter().filter_map(message_seq).min()?;
    let last = messages.iter().filter_map(message_seq).max()?;
    if (messages.len() as i64) >= last - first + 1 {
        return None;
    }
    Some(format!(
        "> Incomplete: the server returned {n} messages spanning seq {first}–{last}; the missing ones are not included.",
        n = messages.len()
    ))
}

/// The conversation as Markdown, from the canonical history. Chat text ONLY:
/// user/assistant rows with non-empty text; tool/reasoning/system/empty rows
/// are dropped; consecutive same-speaker messages merge into one section.
pub fn conversation_markdown(
    session_id: &str,
    workspace_root: Option<&str>,
    title: Option<&str>,
    messages: &[Value],
) -> String {
    let mut sections: Vec<(String, Vec<String>)> = Vec::new();
    for m in messages {
        let Some(role) = message_role(m) else { continue };
        let speaker = match role.trim().to_lowercase().as_str() {
            "user" => "User",
            "assistant" => "Assistant",
            _ => continue, // tool / system / reasoning rows: dropped
        };
        let Some(text) = message_text(m).map(|t| trim_blank_lines(&t)) else {
            continue;
        };
        if text.is_empty() {
            continue; // empty rows: dropped (the "nothing to copy" input)
        }
        match sections.last_mut() {
            Some((s, parts)) if s == speaker => parts.push(text),
            _ => sections.push((speaker.to_owned(), vec![text])),
        }
    }
    if sections.is_empty() {
        return String::new(); // empty conversation -> the caller reports "empty"
    }
    let mut lines = vec![format!("# {}", heading_title(title, workspace_root)), String::new()];
    if let Some(w) = workspace_root {
        lines.push(format!("- Workspace: `{w}`"));
    }
    lines.push(format!("- Session: `{session_id}`"));
    lines.push(String::new());
    if let Some(gap) = incomplete_disclosure(messages) {
        lines.push(gap);
        lines.push(String::new());
    }
    for (speaker, parts) in &sections {
        lines.push(format!("## {speaker}"));
        lines.push(String::new());
        lines.push(parts.join("\n\n"));
        lines.push(String::new());
    }
    lines.join("\n")
}

/// The outcome of reading the history for a copy. `Foreign` mirrors the web's
/// "History belongs to another Session." error (copy-conversation.ts:92).
pub enum CopyOutcome {
    /// The markdown text, ready for the clipboard.
    Copied(String),
    /// An empty conversation: the web writes NOTHING to the clipboard.
    Empty,
    /// The history's session_id is not the requested one.
    Foreign,
}

/// `copyConversationMarkdown`'s read+build path
/// (copy-conversation.ts:83-100): hydrate the session (messages only), refuse
/// a foreign session, build the markdown.
pub async fn copy_conversation(
    client: &octoscode_client::Client,
    session_id: &str,
    workspace_root: Option<&str>,
    title: Option<&str>,
) -> Result<CopyOutcome, String> {
    let reply = client
        .request("session/hydrate", json!({ "session_id": session_id }))
        .await
        .map_err(|e| format!("session/hydrate: {e}"))?;
    // Row 6: a foreign history is an ERROR, not a copy.
    match reply.get("session_id").and_then(|v| v.as_str()) {
        Some(sid) if sid == session_id => {}
        _ => return Ok(CopyOutcome::Foreign),
    }
    let empty = reply.get("messages").and_then(|v| v.as_array()).map(|a| a.is_empty()).unwrap_or(true);
    let messages: Vec<Value> = reply
        .get("messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if empty || conversation_markdown(session_id, workspace_root, title, &messages).is_empty() {
        return Ok(CopyOutcome::Empty);
    }
    let markdown = conversation_markdown(session_id, workspace_root, title, &messages);
    // The desktop clipboard write lives behind the UI layer; the exported
    // text is the outcome — the button layer owns the write + phases.
    Ok(CopyOutcome::Copied(markdown))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- the web's own cases (conversation-markdown.test.ts) --------------

    fn msg(role: &str, content: &str, seq: i64) -> Value {
        json!({"role": role, "content": content, "seq": seq})
    }

    #[test]
    fn chat_only_drops_tool_reasoning_system_and_empty_and_merges_runs() {
        let messages = vec![
            msg("user", "first ask", 1),
            msg("assistant", "", 2),              // empty -> dropped
            msg("tool", "hidden tool output", 3), // tool -> dropped
            msg("assistant", "part one", 4),
            msg("Assistant", "part two", 5), // same run (case-insensitive) -> merged
            msg("system", "hidden system row", 6),
            msg("user", "follow-up", 7),
        ];
        let md = conversation_markdown("s1", None, None, &messages);
        assert!(md.contains("## User"));
        assert!(md.contains("## Assistant"));
        assert!(md.contains("first ask"));
        assert!(md.contains("part one"));
        // merged run: one section, both parts, joined by a blank line
        let assistant = md.split("## Assistant").nth(1).unwrap();
        assert!(assistant.contains("part one\n\npart two"));
        assert!(!md.contains("hidden tool output"));
        assert!(!md.contains("hidden system row"));
        // the empty assistant row contributes nothing
        assert_eq!(md.matches("part one").count(), 1);
        assert!(md.starts_with("# Conversation"));
    }

    #[test]
    fn header_is_title_else_leaf_else_conversation_with_ws_and_session_lines() {
        assert_eq!(heading_title(Some("  My title "), None), "My title");
        assert_eq!(heading_title(None, Some("/tmp/ws-group/")), "ws-group");
        assert_eq!(heading_title(None, Some("C:\\repo\\proj")), "proj");
        assert_eq!(heading_title(None, None), "Conversation");
        let md = conversation_markdown("dsflash:main", Some("/tmp/43a-ws"), Some("T"), &[
            msg("user", "hi", 1),
            msg("assistant", "ho", 2),
        ]);
        assert!(md.starts_with("# T\n"));
        assert!(md.contains("- Workspace: `/tmp/43a-ws`"));
        assert!(md.contains("- Session: `dsflash:main`"));
    }

    #[test]
    fn incomplete_history_is_disclosed_not_hidden() {
        // seq 1..=5 span = 5, only 3 rows returned -> disclosure.
        let messages = vec![
            msg("user", "a", 1),
            msg("assistant", "b", 3),
            msg("user", "c", 5),
        ];
        let md = conversation_markdown("s", None, None, &messages);
        assert!(md.contains("> Incomplete: the server returned 3 messages spanning seq 1–5"));
        // full span -> no disclosure
        let full = vec![msg("user", "a", 1), msg("assistant", "b", 2)];
        assert!(incomplete_disclosure(&full).is_none());
        // rows out of order still compare min/max
        let unordered = vec![msg("user", "c", 9), msg("assistant", "b", 1)];
        assert_eq!(
            incomplete_disclosure(&unordered).as_deref(),
            Some("> Incomplete: the server returned 2 messages spanning seq 1–9; the missing ones are not included.")
        );
    }

    #[test]
    fn empty_history_produces_no_markdown() {
        assert!(conversation_markdown("s", None, None, &[]).is_empty());
        let only_dropped = vec![msg("tool", "out", 1), msg("assistant", "", 2)];
        assert!(conversation_markdown("s", None, None, &only_dropped).is_empty());
    }
}
