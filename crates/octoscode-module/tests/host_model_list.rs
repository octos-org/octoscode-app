//! Startup model refresh over the real hosted transport, with a scripted port.
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;

use octos_app_transport::host::{HostPort, HostPortEvent};
use octoscode_module::{
    flow::{Conversation, FlowEvent},
    screens::{board3::seats, models},
};
use serde_json::{json, Value};

struct ClosePort(mpsc::Sender<HostPortEvent>);
impl Drop for ClosePort {
    fn drop(&mut self) {
        let _ = self.0.send(HostPortEvent::Closed("test finished".into()));
    }
}

async fn refresh(wrong_session: bool) -> (Result<usize, String>, Vec<Value>, String) {
    let (tx, rx) = mpsc::channel();
    let _close = ClosePort(tx.clone());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let captured = seen.clone();
    let port = HostPort {
        send: Box::new(move |text| {
            let request: Value = serde_json::from_str(&text).unwrap();
            captured.lock().unwrap().push(request.clone());
            let session = &request["params"]["session_id"];
            let mut reply = json!({"jsonrpc": "2.0", "id": request["id"]});
            reply["result"] = match request["method"].as_str().unwrap() {
                "session/open" => json!({"opened": {
                    "session_id": session, "active_profile_id": "_main",
                    "cursor": {"stream": session, "seq": 1},
                    "capabilities": {
                        "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                        "capabilities_schema_version": 2,
                        "supported_methods": ["session/open", "session/hydrate", "profile/llm/list"],
                        "supported_notifications": [],
                        "supported_features": ["state.session_hydrate.v1"]
                    }
                }}),
                "session/hydrate" => {
                    json!({"session_id": session, "cursor": {"stream": session, "seq": 2}, "messages": []})
                }
                "profile/llm/list" if session.is_null() => {
                    reply["error"] =
                        json!({"code": -32602, "message": "it names none of its sessions"});
                    Value::Null
                }
                "profile/llm/list" => json!({
                    "session_id": if wrong_session { json!("_main:api:code-other") } else { session.clone() },
                    "models": [{"model": "glm-5", "provider": "zai-coding", "title": "GLM 5", "family": "zai-coding", "route": "zai-coding", "selected": true, "available": true}]
                }),
                _ => {
                    reply["error"] =
                        json!({"code": -32601, "message": "not a coding client's method"});
                    Value::Null
                }
            };
            if reply.get("error").is_some() {
                reply.as_object_mut().unwrap().remove("result");
            }
            tx.send(HostPortEvent::Frame(reply.to_string()))
                .map_err(|e| e.to_string())
        }),
        recv: Box::new(move || {
            rx.recv()
                .unwrap_or(HostPortEvent::Closed("test ended".into()))
        }),
    };
    let (conv, mut events) = Conversation::connect_host(port, "_main", None, None).unwrap();
    conv.open_workspace(None).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = events.recv().await {
            if matches!(conv.on_event(event), FlowEvent::WorkspaceOpened(_)) {
                return;
            }
        }
        panic!("no opened session");
    })
    .await
    .unwrap();
    let conv = Arc::new(conv);
    let drain = {
        let conv = conv.clone();
        tokio::spawn(async move {
            while let Some(event) = events.recv().await {
                conv.on_event(event);
            }
        })
    };
    let result = tokio::time::timeout(Duration::from_secs(5), models::refresh(&conv, &conv.store))
        .await
        .unwrap();
    let label = seats::model_seat_label(&conv.store);
    let requests = seen.lock().unwrap().clone();
    drain.abort();
    (result, requests, label)
}

#[tokio::test]
async fn hosted_startup_reads_the_opened_sessions_models_and_updates_the_seat() {
    let (result, requests, label) = refresh(false).await;
    let open = requests
        .iter()
        .find(|v| v["method"] == "session/open")
        .unwrap();
    let lists: Vec<_> = requests
        .iter()
        .filter(|v| v["method"] == "profile/llm/list")
        .collect();
    assert_eq!(lists.len(), 1);
    assert_eq!(
        lists[0]["params"]["session_id"], open["params"]["session_id"],
        "the coding scope requires the app's session"
    );
    assert_eq!(lists[0]["params"]["profile_id"], "_main");
    assert_eq!(result, Ok(1));
    assert_eq!(
        label, "GLM 5",
        "the session-shaped result must reach the composer seat"
    );
    assert!(!requests
        .iter()
        .any(|v| v["method"] == "turn/start" || v["method"] == "profile/llm/select"));
}

#[tokio::test]
async fn hosted_startup_rejects_models_returned_for_another_session() {
    let (result, _, label) = refresh(true).await;
    assert!(
        result.is_err(),
        "another session's result must not count as a refreshed read"
    );
    assert_eq!(label, seats::MODEL_SELECT);
}
