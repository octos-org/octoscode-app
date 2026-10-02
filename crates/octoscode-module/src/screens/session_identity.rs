//! A8 — the full-Session identity grammar, ported from the web's
//! `features/resume/resume-binding.ts:53-100` (`isFullSessionForProfile`:
//! Core `SessionKey::split_base_key` + the channel registry, rc11
//! `types.rs:607`). A full id is `<profile>:<channel>:<chat>[#<topic>]`:
//! the profile is the captured one and is NOT a channel name, the channel is
//! a registered one, the chat is non-empty and untrimmed-free, an optional
//! topic is non-blank, and no control character appears anywhere. Unknown
//! future channels fail closed until the identity contract is updated.
//!
//! The native "New chat" mints ids in this grammar (`<profile>:api:<uuid>`,
//! the shape the web's `bindWebSessionIdToProfile` produces), so a Session
//! this app created is resumable by the same rule.

/// The channel registry (`CHANNELS`, `resume-binding.ts:55-76`).
pub const CHANNELS: &[&str] = &[
    "acp", "api", "cli", "dingtalk", "discord", "email", "feishu", "line", "local", "matrix", "qq-bot", "slack", "system",
    "telegram", "test", "twilio", "wechat", "wecom", "wecom-bot", "whatsapp",
];

fn is_channel(s: &str) -> bool {
    CHANNELS.contains(&s)
}

/// `isFullSessionForProfile(sessionId, profileId)`.
pub fn is_full_session_for_profile(session_id: &str, profile_id: &str) -> bool {
    if session_id.is_empty() || session_id != session_id.trim() || session_id.chars().any(char::is_control) {
        return false;
    }
    let (base, topic) = match session_id.find('#') {
        Some(h) => (&session_id[..h], Some(&session_id[h + 1..])),
        None => (session_id, None),
    };
    let Some(first) = base.find(':') else { return false };
    if first < 1 {
        return false;
    }
    let Some(rel) = base[first + 1..].find(':') else { return false };
    let second = first + 1 + rel;
    if second < first + 2 {
        return false;
    }
    let (profile, channel, chat) = (&base[..first], &base[first + 1..second], &base[second + 1..]);
    profile == profile_id
        && !is_channel(profile)
        && is_channel(channel)
        && !chat.is_empty()
        && chat == chat.trim()
        && topic.is_none_or(|t| !t.trim().is_empty() && t == t.trim())
}

/// The id a native "New chat" mints: a fresh chat id on the `api` channel of
/// the profile (the web: `<profile>:api:web-<uuid>`).
pub fn fresh_full_id(profile: &str, chat: &str) -> String {
    format!("{profile}:api:{chat}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_ids_follow_split_base_key_and_the_channel_registry() {
        // resume-binding.test.ts:137 / :279 / :291 cases.
        assert!(is_full_session_for_profile("dsflash:api:main", "dsflash"));
        assert!(is_full_session_for_profile("dsflash:api:web-1:2", "dsflash"), "colon chat ids");
        assert!(is_full_session_for_profile("dsflash:api:main#peer-review", "dsflash"), "a topic");
        assert!(!is_full_session_for_profile("dsflash:main", "dsflash"), "no channel: never guessed");
        assert!(!is_full_session_for_profile("other:api:main", "dsflash"), "another profile");
        assert!(!is_full_session_for_profile("api:api:main", "api"), "a channel-like profile");
        assert!(!is_full_session_for_profile("dsflash:future:main", "dsflash"), "unknown channel fails closed");
        assert!(!is_full_session_for_profile(" dsflash:api:main", "dsflash"), "untrimmed");
        assert!(!is_full_session_for_profile("dsflash:api: main", "dsflash"), "untrimmed chat");
        assert!(!is_full_session_for_profile("dsflash:api:", "dsflash"), "empty chat");
        assert!(!is_full_session_for_profile("dsflash:api:main#", "dsflash"), "blank topic");
        assert!(!is_full_session_for_profile("dsflash:api:ma\u{7}in", "dsflash"), "control character");
        assert!(is_full_session_for_profile(&fresh_full_id("dsflash", "0192"), "dsflash"));
    }
}
