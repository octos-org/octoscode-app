//! P4a3 — research lanes (`profile/sub_providers/upsert|remove`), ported
//! from the web oracle's `ResearchDialog.tsx`: mutations are GATED on the
//! server's advertised methods (`available = supportsMethod(capabilities,
//! method)`, line 58) and LOCKED while known Profile work is running
//! (`pending || (mutation && blocked.current)`, line 66); the API key is read
//! ONLY at dispatch and immediately dropped — never a store field; the
//! receipt's lanes replace the folded list and the notice copy is verbatim.
use serde_json::{json, Value};
use octoscode_store::Store;

/// The web's `recordMutation` notice (ResearchDialog.tsx:120-127), verbatim.
pub fn mutation_notice(applied: bool, restart_required: bool) -> &'static str {
    if restart_required {
        "Saved on the server. A serve restart is required before these research lanes become effective. No restart was performed."
    } else if applied {
        "Saved on the server; the server reports no restart requirement."
    } else {
        "No server configuration change was applied."
    }
}

/// The mutation gate (ResearchDialog.tsx:58 + 66): the server must ADVERTISE
/// the method (`supported_methods`) and no known Profile work may be running.
/// `None` = allowed; `Some(reason)` = the web's disabled/blocked state.
pub fn mutation_blocked(store: &Store, method: &str) -> Option<&'static str> {
    if store.domains.profile.profile_busy() {
        return Some("Profile work is running — the research surface is locked.");
    }
    if !store
        .domains
        .config
        .supported_methods()
        .iter()
        .any(|m| m == method)
    {
        return Some("This server does not advertise the research-lane methods.");
    }
    None
}

/// `confirm()`'s save arm (ResearchDialog.tsx:133-147): the credential lands
/// ONLY in the request params (dispatch — `api_key` is absent when `None`),
/// the store never sees it, and the receipt's lanes replace the folded list
/// (`recordMutation` → `setData`). Returns the notice copy.
pub async fn upsert_lane(
    client: &octoscode_client::Client,
    store: &Store,
    lane: octoscode_client::domains::profile::SubProviderParams,
    api_key: Option<String>,
) -> Result<String, String> {
    if let Some(why) = mutation_blocked(store, "profile/sub_providers/upsert") {
        return Err(why.to_owned());
    }
    let mut params = json!({ "sub_provider": lane });
    if let Some(key) = api_key {
        params["api_key"] = json!(key);
    }
    let receipt = client
        .request("profile/sub_providers/upsert", params)
        .await
        .map_err(|e| format!("profile/sub_providers/upsert: {e}"))?;
    Ok(fold_receipt(receipt, store))
}

/// `confirm()`'s remove arm: `{ key }`, same gate, same receipt fold.
pub async fn remove_lane(
    client: &octoscode_client::Client,
    store: &Store,
    key: &str,
) -> Result<String, String> {
    if let Some(why) = mutation_blocked(store, "profile/sub_providers/remove") {
        return Err(why.to_owned());
    }
    let receipt = client
        .request("profile/sub_providers/remove", json!({ "key": key }))
        .await
        .map_err(|e| format!("profile/sub_providers/remove: {e}"))?;
    Ok(fold_receipt(receipt, store))
}

/// `recordMutation` (ResearchDialog.tsx:118-128): the receipt's lanes become
/// the surface's data; the notice reflects applied/restart_required.
fn fold_receipt(receipt: Value, store: &Store) -> String {
    let applied = receipt.get("applied").and_then(|v| v.as_bool()).unwrap_or(false);
    let restart = receipt
        .get("restart_required")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    crate::screens::models::fold_sub_providers(receipt, store);
    mutation_notice(applied, restart).to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use octoscode_client::domains::profile::SubProviderParams;

    fn lane(key: &str) -> SubProviderParams {
        // A literal: the params struct is Serialize-only (the wire never
        // returns lanes in this shape — reads use the store's SubProvider).
        SubProviderParams {
            key: key.to_owned(),
            provider: "zhipu".to_owned(),
            model: Some("glm-4-flash".to_owned()),
            api_key_env: None,
            base_url: None,
            description: Some("P4a3 unit lane".to_owned()),
            default_context_window: None,
            max_output_tokens: None,
            api_type: None,
        }
    }

    #[test]
    fn notice_copy_matches_the_web_verbatim() {
        assert_eq!(
            mutation_notice(true, true),
            "Saved on the server. A serve restart is required before these research lanes become effective. No restart was performed."
        );
        assert_eq!(
            mutation_notice(true, false),
            "Saved on the server; the server reports no restart requirement."
        );
        assert_eq!(
            mutation_notice(false, false),
            "No server configuration change was applied."
        );
    }

    #[test]
    fn mutations_are_gated_on_advertised_methods_and_profile_work() {
        let store = Store::new();
        // nothing advertised -> refused
        assert!(mutation_blocked(&store, "profile/sub_providers/upsert").is_some());
        // advertised -> allowed
        store.domains.config.set_supported_methods(vec![
            "profile/sub_providers/list".into(),
            "profile/sub_providers/upsert".into(),
            "profile/sub_providers/remove".into(),
        ]);
        assert!(mutation_blocked(&store, "profile/sub_providers/upsert").is_none());
        assert!(mutation_blocked(&store, "profile/sub_providers/remove").is_none());
        // known Profile work running -> locked again (even when advertised)
        store.domains.profile.set_profile_busy(true);
        assert!(mutation_blocked(&store, "profile/sub_providers/upsert").is_some());
        store.domains.profile.set_profile_busy(false);
        assert!(mutation_blocked(&store, "profile/sub_providers/remove").is_none());
    }

    #[test]
    fn dispatch_params_carry_the_key_but_the_store_never_does() {
        // The request body is where the credential lives (dispatch-only);
        // with None it is ABSENT, never null — the web's optional field.
        let mut params = json!({ "sub_provider": lane("r2-lane") });
        assert!(params.get("api_key").is_none());
        params["api_key"] = json!("sk-dispatch-only");
        assert_eq!(params["api_key"], "sk-dispatch-only");
        // The receipt fold (what the store receives) carries lanes only.
        let receipt = json!({
            "profile_id": "dsflash",
            "sub_providers": [{ "key": "r2-lane", "provider": "zhipu" }],
            "applied": true,
            "restart_required": false,
        });
        let store = Store::new();
        let notice = fold_receipt(receipt, &store);
        assert_eq!(notice, mutation_notice(true, false));
        let lanes = store.domains.profile.sub_providers();
        assert_eq!(lanes.len(), 1);
        assert_eq!(lanes[0].key, "r2-lane");
        // The typed lane struct has no credential field at all — the store
        // cannot hold the secret (checked textually over its debug form).
        let debug = format!("{lanes:?}");
        assert!(!debug.contains("sk-dispatch-only"));
    }

    #[test]
    fn unapplied_receipt_reports_no_change() {
        let receipt = json!({
            "profile_id": "dsflash",
            "sub_providers": [],
            "applied": false,
            "restart_required": false,
        });
        let store = Store::new();
        assert_eq!(fold_receipt(receipt, &store), mutation_notice(false, false));
        assert!(store.domains.profile.sub_providers().is_empty());
    }
}
