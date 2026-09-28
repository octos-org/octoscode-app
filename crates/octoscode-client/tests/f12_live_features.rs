//! Card #12 §1 (live half): the negotiated feature set reaches the server.
//!
//! `#[ignore]` by default. **Lane p0-build owns serve port 50110**
//! (LESSONS "Domain fan-out: your own `octos serve` port").
//!
//! ```sh
//! tmp/octos-target/release/octos serve --solo --port 50110 --host 127.0.0.1 \
//!   --auth-token f12-dummy-token --data-dir tmp/f12-serve-home --instance-data-dir tmp/f12-serve-data
//! OCTOS_BASE_URL=http://127.0.0.1:50110 OCTOS_BEARER=f12-dummy-token \
//!   ctest -p octoscode-client --test f12_live_features -- --ignored --nocapture
//! ```
//!
//! The claim: requesting the web's 21 features (which our
//! [`octoscode_client::features::web_capabilities`] does) makes the serve
//! advertise **≥ 95** methods, where the transport default sees 73.
use octos_app_transport::{
    ws, Capabilities, OutboundCommand, ProfileId, SecretString, TransportConfig, TransportEvent,
};
use octoscode_client::features;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};
use url::Url;

fn base_url() -> String {
    std::env::var("OCTOS_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:50110".to_string())
}

/// Connect with the given capabilities and return the methods the server
/// advertises in `config/capabilities/list`.
///
/// `apply_web_features` controls the **query-param** half of the negotiation
/// (`url.ts:24-28`); `caps` controls the header half. The baseline arm passes
/// `false` + the transport default, so the two arms are genuinely different.
async fn advertised_methods(caps: Capabilities, apply_web_features: bool) -> Vec<String> {
    let mut url = Url::parse(&base_url()).expect("OCTOS_BASE_URL");
    {
        let pairs: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(k, _)| k != "ui_feature")
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        url.set_query(None);
        let mut q = url.query_pairs_mut();
        for (k, v) in &pairs {
            q.append_pair(k, v);
        }
        if apply_web_features {
            for f in features::WEB_UI_FEATURES {
                q.append_pair("ui_feature", f);
            }
        }
    }
    let cfg = TransportConfig {
        base_url: url,
        bearer: SecretString::new(
            std::env::var("OCTOS_BEARER").unwrap_or_else(|_| "f12-dummy-token".to_string()),
        ),
        profile_id: ProfileId::new(format!("f12-{}", std::process::id())),
        cursor: None,
        cursor_file: None,
        requested_capabilities: caps,
        workspace_cwd: None,
        local_kernel: false,
    };
    let (cmd_tx, mut evt_rx) = ws::spawn(cfg);

    // Issue `config/capabilities/list` through the transport's generic request
    // (the same path the Client uses).
    let (reply_tx, reply_rx) = oneshot::channel();
    cmd_tx
        .send(OutboundCommand::Request {
            method: "config/capabilities/list".to_owned(),
            params: Value::Object(Default::default()),
            reply: reply_tx,
        })
        .await
        .expect("send");

    // Drain events so the channel never back-pressures, while awaiting reply.
    let drain = tokio::spawn(async move {
        while let Some(_e) = evt_rx.recv().await {}
    });
    let result = tokio::time::timeout(std::time::Duration::from_secs(20), reply_rx)
        .await
        .expect("reply timeout")
        .expect("reply channel")
        .expect("config/capabilities/list");
    let _ = cmd_tx.send(OutboundCommand::Disconnect).await;
    drain.abort();

    let node = result
        .pointer("/capabilities")
        .cloned()
        .unwrap_or(result);
    node.get("supported_methods")
        .and_then(|v| v.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect())
        .unwrap_or_default()
}

#[tokio::test]
#[ignore = "needs a local octos serve on 50110; run with --ignored"]
async fn the_web_feature_list_negotiates_at_least_95_methods() {
    // (a) The transport default: a small slice.
    let default_methods = advertised_methods(Capabilities::requested(), false).await;
    println!(
        "transport default (Capabilities::requested) -> {} methods",
        default_methods.len()
    );

    // (b) The web's 21 features (our web_capabilities + query params).
    let web_methods = advertised_methods(features::web_capabilities(), true).await;
    println!(
        "web feature list ({} features) -> {} methods",
        features::WEB_UI_FEATURES.len(),
        web_methods.len()
    );

    // The card's gate: >= 95 with the web's list.
    assert!(
        web_methods.len() >= 95,
        "expected >= 95 methods with the web's feature list, got {}",
        web_methods.len()
    );
    // And the features genuinely widen the surface (F1 measured 73 -> 95).
    assert!(
        web_methods.len() > default_methods.len(),
        "the web list must advertise more than the default ({} vs {})",
        web_methods.len(),
        default_methods.len()
    );

    // The F1/F4/F5 surfaces every lane needed must be present.
    for m in [
        "agent/list",
        "loop/list",
        "monitor/list",
        "session/goal/get",
        "turn/steer",
        "tool/status/list",
        "thread/graph/get",
        "turn/state/get",
    ] {
        assert!(
            web_methods.contains(&m.to_owned()),
            "{m} must be advertised with the web's features; got {} methods",
            web_methods.len()
        );
    }

    // No `mpsc` import left unused: assert the channel helper type compiles.
    let (_tx, _rx): (mpsc::Sender<TransportEvent>, _) = mpsc::channel(1);
}
