//! A22 row 228 — the per-workspace Session catalog's PROJECTION: what one
//! `session/list` reply may put in the sidebar.
//!
//! Web oracle: `features/session/workspace-session-catalog.ts`
//! * `:197-209` — only a listing the server ATTESTS as this profile's
//!   project store is placed under the workspace (`result.workspace_root &&
//!   result.profile_id === profileId`), else the workspace is `unscoped`:
//!   nothing projected. octos-core a6ea8505 `SessionListResult` says the
//!   same from the server side ("a client must not place rows under a
//!   workspace unless this attests the scope");
//! * `:66-92` `catalogSessionsFromList` — only FULL Sessions of the
//!   requested profile (`isFullSessionForProfile`,
//!   `features/resume/resume-binding.ts:76`): another profile's row and a
//!   bare id are dropped, never guessed; title / last prompt trimmed (blank =
//!   none); newest first, an absent or unparseable time last;
//! * the legacy global listing is never a product catalog
//!   (`features/workspace/use-workspace-product.ts:60-63` and `:146-149`:
//!   "accepting it here would assign unproven Sessions to `cwd`").
//!
//! The sidebar then merges the catalog with the Sessions this app opened
//! (`SessionSidebar.tsx:116-140`, `mergeWorkspaceSessionRows`
//! `workspace-session-catalog.ts:107-135`): the store keeps every known
//! Session (`Sessions::set_catalog`).
//!
//! One native adaptation, stated: a catalog row of a Session THIS APP OPENED
//! keeps its server metadata (title, time) even when its id is not in the
//! full grammar — the native app's own `<profile>:main` (the web only ever
//! mints `<profile>:api:web-…` ids, so it never meets one). Nothing new is
//! projected by it: the row is shown because it is known, the catalog only
//! names it.
use octoscode_client::domains::session::{SessionListParams, SessionListResult, SessionListRow};
use octoscode_store::Session;

use crate::screens::session_identity::is_full_session_for_profile;

/// What one `session/list` reply projects.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Listing {
    /// An attested catalog of `workspace`: its routable rows, newest first.
    Catalog { workspace: String, rows: Vec<Session> },
    /// A scoped read the server did not attest for the requested profile:
    /// the workspace is `unscoped` — none of its rows is projected.
    Unscoped { workspace: String, rows: usize },
    /// The legacy global listing (no `{cwd, profile_id}` asked): not a
    /// product catalog.
    Legacy { rows: usize },
}

/// Project one reply to the request that asked for it. `known` says whether
/// this app opened a Session (the merge keeps those regardless).
pub fn project(params: &SessionListParams, result: SessionListResult, known: impl Fn(&str) -> bool) -> Listing {
    let (Some(workspace), Some(profile)) = (
        params.cwd.as_deref().filter(|c| !c.trim().is_empty()),
        params.profile_id.as_deref().filter(|p| !p.trim().is_empty()),
    ) else {
        return Listing::Legacy { rows: result.sessions.len() };
    };
    let attested = result.workspace_root.as_deref().is_some_and(|r| !r.trim().is_empty())
        && result.profile_id.as_deref() == Some(profile);
    if !attested {
        return Listing::Unscoped { workspace: workspace.to_owned(), rows: result.sessions.len() };
    }
    Listing::Catalog { workspace: workspace.to_owned(), rows: catalog_rows(profile, result.sessions, known) }
}

/// `catalogSessionsFromList(workspaceRoot, profileId, entries)`
/// (`workspace-session-catalog.ts:66-92`).
pub fn catalog_rows(profile: &str, entries: Vec<SessionListRow>, known: impl Fn(&str) -> bool) -> Vec<Session> {
    let trimmed = |v: Option<String>| v.map(|t| t.trim().to_owned()).filter(|t| !t.is_empty());
    let prefix = format!("{profile}:");
    let mut rows: Vec<(Option<u64>, Session)> = entries
        .into_iter()
        .filter(|e| is_full_session_for_profile(&e.id, profile) || (e.id.starts_with(&prefix) && known(&e.id)))
        .map(|e| {
            let at = e.updated_at.as_deref().and_then(crate::screens::sidebar::parse_rfc3339_ms);
            (
                at,
                Session {
                    id: e.id,
                    title: trimmed(e.title),
                    message_count: e.message_count,
                    updated_at: at.and(e.updated_at),
                    last_prompt: trimmed(e.last_prompt),
                    active_turn: e.active_turn,
                },
            )
        })
        .collect();
    // Newest first; an absent/unparseable time sorts last (`-Infinity`),
    // the sort is stable like the web's.
    rows.sort_by(|a, b| match (a.0, b.0) {
        (Some(x), Some(y)) => y.cmp(&x),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    rows.into_iter().map(|(_, s)| s).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn result(v: serde_json::Value) -> SessionListResult {
        serde_json::from_value(v).expect("a list result")
    }

    fn scoped(cwd: &str, profile: &str) -> SessionListParams {
        SessionListParams { cwd: Some(cwd.into()), profile_id: Some(profile.into()) }
    }

    /// `workspace-session-catalog.test.ts:22` "keeps only full sessions of
    /// the requested profile and maps server metadata", case for case.
    #[test]
    fn keeps_only_full_sessions_of_the_requested_profile_and_maps_server_metadata() {
        let r = result(json!({"workspace_root": "/srv/project", "profile_id": "dev", "sessions": [
            {"id": "dev:api:web-old", "message_count": 730, "title": "学习一下如何做editable pptx",
             "updated_at": "2026-09-22T01:37:00.222Z", "last_prompt": "记住现在的skills", "active_turn": false},
            {"id": "dev:api:web-new", "message_count": 9},
            {"id": "other:api:web-x", "message_count": 1},
            {"id": "bare", "message_count": 1}
        ]}));
        let Listing::Catalog { workspace, rows } = project(&scoped("/srv/project", "dev"), r, |_| false) else {
            panic!("an attested listing is a catalog")
        };
        assert_eq!(workspace, "/srv/project");
        assert_eq!(
            rows,
            vec![
                Session {
                    id: "dev:api:web-old".into(),
                    title: Some("学习一下如何做editable pptx".into()),
                    message_count: 730,
                    updated_at: Some("2026-09-22T01:37:00.222Z".into()),
                    last_prompt: Some("记住现在的skills".into()),
                    active_turn: false,
                },
                Session {
                    id: "dev:api:web-new".into(),
                    title: None,
                    message_count: 9,
                    updated_at: None,
                    last_prompt: None,
                    active_turn: false,
                },
            ]
        );
    }

    /// `:58` "orders newest first and tolerates an unparseable timestamp".
    #[test]
    fn orders_newest_first_and_tolerates_an_unparseable_timestamp() {
        let rows = catalog_rows(
            "dev",
            serde_json::from_value(json!([
                {"id": "dev:api:a", "message_count": 1, "updated_at": "2026-01-01T00:00:00Z"},
                {"id": "dev:api:b", "message_count": 1, "updated_at": "not a date"},
                {"id": "dev:api:c", "message_count": 1, "updated_at": "2026-02-01T00:00:00Z"}
            ]))
            .unwrap(),
            |_| false,
        );
        let ids: Vec<&str> = rows.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["dev:api:c", "dev:api:a", "dev:api:b"]);
        assert_eq!(rows[2].updated_at, None, "an unparseable time is no time");
    }

    /// `:197-209`: a reply that does not attest THIS profile's store is
    /// unscoped; a legacy read is not a catalog at all.
    #[test]
    fn only_an_attested_scoped_listing_is_a_catalog() {
        let rows = json!([{"id": "dev:api:a", "message_count": 1}]);
        let p = scoped("/srv/project", "dev");
        assert_eq!(
            project(&p, result(json!({"sessions": rows})), |_| false),
            Listing::Unscoped { workspace: "/srv/project".into(), rows: 1 },
            "no workspace_root: the server's legacy global listing"
        );
        assert_eq!(
            project(&p, result(json!({"sessions": rows, "workspace_root": "/srv/project", "profile_id": "other"})), |_| false),
            Listing::Unscoped { workspace: "/srv/project".into(), rows: 1 },
            "another profile's store"
        );
        assert_eq!(
            project(&SessionListParams::default(), result(json!({"sessions": rows, "workspace_root": "/srv/project", "profile_id": "dev"})), |_| false),
            Listing::Legacy { rows: 1 },
            "an unscoped request is never a product catalog"
        );
    }

    /// The stated native adaptation: the catalog row of a Session this app
    /// opened names it (title, time) even when its id is the native
    /// `<profile>:main`; an unknown `<profile>:main` stays out.
    #[test]
    fn a_known_sessions_own_row_keeps_its_server_title() {
        let entries: Vec<SessionListRow> = serde_json::from_value(json!([
            {"id": "dev:main", "message_count": 4, "title": "Fix the build"},
            {"id": "dev:legacy", "message_count": 2, "title": "Never opened here"},
            {"id": "other:main", "message_count": 2, "title": "Another profile"}
        ]))
        .unwrap();
        let rows = catalog_rows("dev", entries, |id| id == "dev:main" || id == "other:main");
        let ids: Vec<&str> = rows.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["dev:main"]);
        assert_eq!(rows[0].title.as_deref(), Some("Fix the build"));
    }
}
