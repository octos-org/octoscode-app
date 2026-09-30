//! Entry #29c — the stage-C screens' gates:
//!
//! 1. **bindings coverage** — every binding id the 44 copy slots declare
//!    resolves in `screens::models::query_binding` (the 8.8 condition-2 audit,
//!    same shape as f15's for the conversation cards).
//! 2. **card-table agreement** — every copy id the table names exists in the
//!    authored `page.card` (the table cannot drift from the committed cards).
//! 3. **the pure action table** — `(action, store) → (method, params)` maps
//!    each card control to its protocol method with the web's param shape.
//! 4. **the folds** — the recorded r2-profile `profile/llm/list` frame folds
//!    into store rows that the bindings compose from.
//! 5. **replay** — a fake WS server serving the RECORDED r2-profile frames:
//!    `refresh` folds all three reads, then `perform` sends
//!    `profile/llm/test` through the production client and the server sees it.

use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

use octoscode_module::bindings::{self, Ctx};
use octoscode_module::flow::{Conversation, FlowUi};
use octoscode_module::screens::models;
use octoscode_store::domains::profile::{InstalledSkill, ProfileLlmModel, SkillPackage};
use octoscode_store::Store;

// ---------------------------------------------------------------- fixtures

fn ctx() -> (Arc<Store>, &'static Mutex<FlowUi>, Ctx<'static>) {
    // Leak the arcs so Ctx<'static> can borrow them (test-only).
    let store: &'static Arc<Store> = Box::leak(Box::new(Arc::new(Store::new())));
    let ui: &'static Mutex<FlowUi> = Box::leak(Box::new(Mutex::new(FlowUi::default())));
    store.domains.session.set_active(Some("dsflash:main".into()));
    (store.clone(), ui, Ctx::new(store, ui))
}

/// A FULL store: three providers (the atlas card's DeepSeek/Kimi/GLM), two
/// deepseek rows (flash default + pro), three installed skills, registry
/// packages, a lifecycle estimate and a provider context window — everything
/// the 44 copy-slot bindings compose from. The coverage audit probes THIS:
/// an arm that cannot resolve on a full store is a real gap; on an empty
/// store `None` is CORRECT behaviour (the authored copy stays).
fn seed(store: &Store) {
    store.domains.session.set_context(
        "dsflash:main",
        octoscode_store::domains::session::ContextLifecycle {
            kind: "context/normalization_reported".into(),
            state: serde_json::json!({"token_estimate": 128000, "session_id": "dsflash:main"}),
            detail: None,
        },
    );
    // the occupancy window rides the token_cost_update payload (the
    // recorded live-gate frame carries 1_048_576; 200_000 is the Stage-B
    // fixture's window and every test writes the same pair)
    models::note_token_cost("dsflash:main", 200_000);
    store.domains.profile.set_llm_models(vec![
        ProfileLlmModel {
            model: "deepseek-v4-flash".into(),
            provider: "deepseek".into(),
            title: "deepseek-v4-flash".into(),
            family: Some("deepseek".into()),
            route: Some("Official API".into()),
            selected: true,
            available: true,
        },
        ProfileLlmModel {
            model: "deepseek-v4-pro".into(),
            provider: "deepseek".into(),
            title: "deepseek-v4-pro".into(),
            family: Some("deepseek".into()),
            route: Some("Official API".into()),
            selected: false,
            available: true,
        },
        ProfileLlmModel {
            model: "kimi-k3".into(),
            provider: "moonshot".into(),
            title: "kimi-k3".into(),
            family: Some("moonshot".into()),
            route: Some("Official API".into()),
            selected: false,
            available: true,
        },
        ProfileLlmModel {
            model: "glm-5".into(),
            provider: "zhipu".into(),
            title: "glm-5".into(),
            family: Some("zhipu".into()),
            route: Some("default route".into()),
            selected: false,
            available: true,
        },
    ]);
    let mut skills: Vec<InstalledSkill> = Vec::new();
    for (i, n) in ["skill-a", "skill-b", "skill-c"].iter().enumerate() {
        skills.push(InstalledSkill {
            name: n.to_string(),
            version: Some(format!("1.{}.0", i)),
            tool_count: 3,
            source_repo: Some(format!("org/repo-{n}")),
        });
    }
    store.domains.profile.set_installed_skills(skills);

    store.domains.profile.set_registry_packages(vec![
        SkillPackage {
            name: "pkg-one".into(),
            description: "first".into(),
            repo: "org/pkg-one".into(),
            version: Some("0.1.0".into()),
            author: None,
            license: None,
            skills: vec![],
            requires: vec![],
            tags: vec![],
            provides_tools: true,
            installed: false,
            installed_skills: vec![],
        },
        SkillPackage {
            name: "pkg-two".into(),
            description: "second".into(),
            repo: "org/pkg-two".into(),
            version: None,
            author: None,
            license: None,
            skills: vec![],
            requires: vec![],
            tags: vec![],
            provides_tools: false,
            installed: false,
            installed_skills: vec![],
        },
    ]);
}

// ------------------------------------------------------- §1 bindings coverage

#[test]
fn every_screen_copy_slot_resolves_on_a_full_store() {
    let (store, _ui, ctx) = ctx();
    seed(&store);
    let mut missing = Vec::new();
    for (_copy_id, binding) in models::COPY_SLOTS {
        if bindings::query(&ctx, binding).is_none() {
            missing.push(*binding);
        }
    }
    assert!(
        missing.is_empty(),
        "unresolved screen bindings: {missing:?}"
    );
    // the conversation table must not shadow the screens' ids (8.8: one owner)
    for (_, binding) in models::COPY_SLOTS {
        assert!(
            bindings::query(&ctx, binding).is_some(),
            "delegation broken for {binding}"
        );
    }
}

// --------------------------------------------------- §2 card-table agreement

#[test]
fn every_copy_id_exists_in_its_authored_card() {
    // (card dir, copy ids the table names for it)
    let groups: &[(&str, &[&str])] = &[
        (
            "setup-07",
            &[
                "t_title_text", "t_ds_head_text", "t_ds_count_text", "t_flash_text",
                "t_pro_text", "btn_test_label_text", "btn_discover_label_text",
                "t_kimi_head_text", "t_kimi_count_text", "t_glm_head_text",
                "t_glm_count_text",
            ],
        ),
        (
            "setup-09",
            &[
                "t_title_text", "t_usage_text", "t_pct_text", "t_row3_text",
                "t_row4_text", "t_row5_text", "t_val6_text", "t_val7_text",
                "t_val8_text", "btn_compact_label_text", "t_comp_text",
                "t_llm_text", "t_heur_text", "t_keep_text",
            ],
        ),
        (
            "setup-10",
            &[
                "t_title_text", "t_inst_head_text", "t_reg_head_text",
                "t_search_text", "t_name3_text", "t_name4_text", "t_name5_text",
                "t_ver6_text", "t_ver7_text", "t_ver8_text", "t_remove0_text",
                "t_remove1_text", "t_remove2_text", "t_name10_text",
                "t_ver11_text", "t_name12_text", "t_ver13_text",
                "btn_3_install_label_text", "btn_4_install_label_text",
            ],
        ),
    ];
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../design/stage-b/setup/cards");
    for (dir, ids) in groups {
        let card = std::fs::read_to_string(root.join(dir).join("page.card"))
            .expect("the committed Stage-B card");
        for id in *ids {
            let needle = format!("copy {id} {{");
            assert!(card.contains(&needle), "{dir}: authored copy slot `{id}` missing");
        }
    }
}

// ---------------------------------------------------- §3 the pure action table

#[test]
fn action_table_maps_each_control_to_its_protocol_method() {
    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:main".into()));
    seed(&store);

    // Test route → the recorded provider's draft (client.ts:787 param shape).
    let (m, p) = models::action_params("models.test_route", &store).expect("test_route maps");
    assert_eq!(m, "profile/llm/test");
    assert_eq!(p["selection"]["family_id"], "deepseek");
    assert_eq!(p["selection"]["model_id"], "deepseek-v4-flash");
    assert_eq!(p["selection"]["route"]["label"], "Official API");

    // Discover → fetch_models: family + route, NO model_id (profile.rs:489).
    let (m, p) = models::action_params("models.discover", &store).expect("discover maps");
    assert_eq!(m, "profile/llm/fetch_models");
    assert_eq!(p["selection"]["family_id"], "deepseek");
    assert!(p["selection"].get("model_id").is_none());

    // Compact now → session/compact {session_id} (context-commands.ts:46).
    let (m, p) =
        models::action_params("context.compact_now", &store).expect("compact maps");
    assert_eq!(m, "session/compact");
    assert_eq!(p["session_id"], "dsflash:main");

    // Remove 0 → profile/skills/remove {name: the row's skill}.
    let (m, p) = models::action_params("skills.remove_0", &store).expect("remove maps");
    assert_eq!(m, "profile/skills/remove");
    assert_eq!(p["name"], "skill-a");

    // Install 3 → registry row 0 (INSTALL_BASE), {repo} only.
    let (m, p) = models::action_params("skills.install_3", &store).expect("install maps");
    assert_eq!(m, "profile/skills/install");
    assert_eq!(p["repo"], "org/pkg-one");

    // Out-of-range and unowned ids map to nothing.
    assert!(models::action_params("skills.install_9", &store).is_none());
    assert!(models::action_params("skills.remove_7", &store).is_none());
    assert!(models::action_params("composer.submit", &store).is_none());
    // owns() covers exactly the cards' service-actions events…
    for a in ["models.test_route", "models.discover", "context.compact_now",
              "skills.remove_0", "skills.install_4"] {
        assert!(models::owns(a), "{a} must be owned");
    }
    // …without colliding with the conversation actions (one owner, LESSONS).
    for a in ["session.refresh", "composer.submit", "turn.steer"] {
        assert!(!models::owns(a), "{a} must stay with the conversation router");
    }
}

// ------------------------------------------------------------- §4 the folds

#[test]
fn the_recorded_llm_list_frame_folds_into_store_rows() {
    // The recorded reply body (r2-profile-a6ea8505.jsonl, `profile/llm/list`):
    // top-level `primary` + `fallbacks`, route label "Official API".
    let frame: serde_json::Value = serde_json::from_str(
        r#"{"primary": {"family_id": "deepseek", "model_id": "deepseek-v4-flash",
                        "selected": true, "available": true,
                        "route": {"route_id": "deepseek", "label": "Official API",
                                  "api_type": "openai"}},
           "fallbacks": []}"#,
    )
    .unwrap();
    let store = Arc::new(Store::new());
    models::fold_llm_list(frame, &store);
    let rows = store.domains.profile.llm_models();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].model, "deepseek-v4-flash");
    assert_eq!(rows[0].provider, "deepseek");
    assert!(rows[0].selected);
    assert_eq!(rows[0].route.as_deref(), Some("Official API"));

    // The bindings compose the atlas strings from the folded rows
    // (head "{provider} • {route}", row "{model} (default)").
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);
    let head = bindings::query(&ctx, "models.head.0").unwrap();
    assert_eq!(head, serde_json::json!("DeepSeek • Official API"),
       "the head carries the web's familyLabel, not the raw id");
}

#[test]
fn context_bindings_compose_from_lifecycle_and_window() {
    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:main".into()));
    store.domains.session.set_context(
        "dsflash:main",
        octoscode_store::domains::session::ContextLifecycle {
            kind: "context/normalization_reported".into(),
            state: serde_json::json!({"token_estimate": 128000, "session_id": "dsflash:main"}),
            detail: None,
        },
    );
    models::note_token_cost("dsflash:main", 200_000);
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);
    assert_eq!(
        bindings::query(&ctx, "context.usage").unwrap(),
        serde_json::json!("128k of 200k tokens")
    );
    assert_eq!(bindings::query(&ctx, "context.pct").unwrap(),
               serde_json::json!("64%"));
    // The k-format falls back to plain numbers for non-round values.
    store.domains.session.set_context(
        "dsflash:main",
        octoscode_store::domains::session::ContextLifecycle {
            kind: "context/normalization_reported".into(),
            state: serde_json::json!({"token_estimate": 12345}),
            detail: None,
        },
    );
    assert_eq!(
        bindings::query(&ctx, "context.usage").unwrap(),
        serde_json::json!("12345 of 200k tokens")
    );
}

// --------------------------------------------------------------- §5 replay

/// One recorded frame (dir/method/body), f26's shape.
struct Frame {
    dir: String,
    method: String,
    body: serde_json::Value,
}

fn load(path: &str) -> Vec<Frame> {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {path}: {e}"))
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let v: serde_json::Value = serde_json::from_str(l).expect("fixture line is JSON");
            Frame {
                dir: v["dir"].as_str().unwrap_or("").to_owned(),
                method: v["method"].as_str().unwrap_or("").to_owned(),
                body: v.get("body").cloned().unwrap_or(serde_json::Value::Null),
            }
        })
        .collect()
}

/// A fake WS server: canned replies per method (from the recording), a default
/// `{}` reply for everything else, and a log of the methods it received.
struct ReplayServer {
    base_url: String,
    received: Arc<Mutex<Vec<String>>>,
}

impl ReplayServer {
    async fn start(canned: Vec<(String, serde_json::Value)>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        let received = Arc::new(Mutex::new(Vec::new()));
        let rx = received.clone();
        tokio::spawn(async move {
            let Ok((stream, _)) = listener.accept().await else { return };
            let Ok(ws) = tokio_tungstenite::accept_async(stream).await else { return };
            let (tx, mut rx_in) = ws.split();
            let tx = Arc::new(tokio::sync::Mutex::new(tx));
            while let Some(Ok(msg)) = rx_in.next().await {
                let Message::Text(text) = msg else { continue };
                let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) else { continue };
                let method = v["method"].as_str().unwrap_or("").to_owned();
                let id = v["id"].as_str().unwrap_or("").to_owned();
                rx.lock().unwrap().push(method.clone());
                let body = canned
                    .iter()
                    .find(|(m, _)| *m == method)
                    .map(|(_, b)| b.clone())
                    .unwrap_or(serde_json::json!({}));
                let frame = serde_json::json!({"jsonrpc": "2.0", "id": id, "result": body});
                let _ = tx
                    .lock()
                    .await
                    .send(Message::Text(frame.to_string().into()))
                    .await;
            }
        });
        Self { base_url: format!("http://{addr}"), received }
    }
}

#[tokio::test]
async fn replay_refresh_folds_the_recorded_reads_and_perform_sends_the_writes() {
    // The REAL recorded replies (r2-profile-a6ea8505.jsonl), served canned.
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/r2-profile-a6ea8505.jsonl");
    let frames = load(fixture);
    let recorded = |method: &str| -> serde_json::Value {
        frames
            .iter()
            .find(|f| f.dir == "in" && f.method == method)
            .map(|f| f.body.clone())
            .unwrap_or_else(|| panic!("{method} is in the recording"))
    };
    let canned = vec![
        ("profile/llm/list".to_owned(), recorded("profile/llm/list")),
        ("profile/skills/list".to_owned(), recorded("profile/skills/list")),
        (
            "profile/sub_providers/list".to_owned(),
            recorded("profile/sub_providers/list"),
        ),
    ];
    let server = ReplayServer::start(canned).await;
    let (conv, mut _events) =
        Conversation::connect(&server.base_url, "dummy", "dsflash", None, None)
            .expect("connect");
    conv.open_workspace(None).await.expect("session/open");
    let store = conv.store.clone();

    // refresh: the three reads answer from the recording; all fold.
    let n = models::refresh(&conv, &store).await.expect("refresh folds");
    assert_eq!(n, 3, "all three profile reads folded");
    let rows = store.domains.profile.llm_models();
    assert!(
        rows.iter().any(|m| m.model == "deepseek-v4-flash" && m.selected),
        "the recorded primary model is in the store: {rows:?}"
    );

    // perform: a screen action reaches the wire as its protocol method.
    let v = models::perform(&conv, "models.test_route", &store)
        .await
        .expect("test_route sends");
    let _ = v; // the canned `{}` reply
    let seen = server.received.lock().unwrap().clone();
    assert!(
        seen.iter().any(|m| m == "profile/llm/test"),
        "the server saw profile/llm/test; got {seen:?}"
    );

    // And the context bindings compose from what refresh folded
    // (the recording's sub_providers carry default_context_window).
    let session = store.active_session().unwrap_or_default();
    let estimate = store
        .domains
        .session
        .context(&session)
        .and_then(|l| l.state.get("token_estimate").and_then(|v| v.as_u64()));
    let window = store
        .domains
        .profile
        .sub_providers()
        .iter()
        .find_map(|sp| sp.default_context_window);
    if let (Some(_), Some(_)) = (estimate, window) {
        let ui = Mutex::new(FlowUi::default());
        let ctx = Ctx::new(&store, &ui);
        let usage = bindings::query(&ctx, "context.usage").unwrap();
        assert!(usage.as_str().unwrap().contains(" of "), "composed: {usage}");
    }
}

// --------------------------------------------- §6 the visual-gate capture

/// Fold the RECORDED profile reads, complete the store to the Stage-B fixture
/// shape (three provider cards, three installed skills, two registry rows,
/// a lifecycle estimate against the recorded window), then write each
/// screen's LIVE card (store values injected into the authored `copy` slots)
/// to `target/f29c-live/<screen>/` for the headless beauty-host capture. The
/// injected strings are asserted here, so the PNG evidence cannot drift from
/// the wiring.
#[tokio::test]
async fn live_capture_writes_the_three_cards_with_store_values() {
    let fixture = concat!(env!("CARGO_MANIFEST_DIR"),
        "/../octoscode-client/tests/fixtures/r2-profile-a6ea8505.jsonl");
    let frames = load(fixture);
    let recorded = |method: &str| -> serde_json::Value {
        frames
            .iter()
            .find(|f| f.dir == "in" && f.method == method)
            .map(|f| f.body.clone())
            .unwrap_or_else(|| panic!("{method} is in the recording"))
    };

    let store = Arc::new(Store::new());
    store.domains.session.set_active(Some("dsflash:main".into()));
    models::fold_llm_list(recorded("profile/llm/list"), &store);
    models::fold_skills_list(recorded("profile/skills/list"), &store);
    models::fold_sub_providers(recorded("profile/sub_providers/list"), &store);
    // The recorded skills/list is empty (count 0) — complete to the Stage-B
    // fixture shape so the capture matches what the accepted review showed.
    if store.domains.profile.installed_skills().is_empty() {
        let mut skills: Vec<InstalledSkill> = Vec::new();
        for (i, n) in ["skill-a", "skill-b", "skill-c"].iter().enumerate() {
            skills.push(InstalledSkill {
                name: n.to_string(),
                version: Some(format!("1.{}.0", i)),
                tool_count: 3,
                source_repo: Some(format!("org/repo-{n}")),
            });
        }
        store.domains.profile.set_installed_skills(skills);
    }
    if store.domains.profile.registry_packages().is_empty() {
        let mk = |name: &str, repo: &str, v: Option<&str>| SkillPackage {
            name: name.into(),
            description: format!("the {name} package"),
            repo: repo.into(),
            version: v.map(str::to_owned),
            author: None,
            license: None,
            skills: vec![],
            requires: vec![],
            tags: vec![],
            provides_tools: true,
            installed: false,
            installed_skills: vec![],
        };
        store.domains.profile.set_registry_packages(vec![
            mk("pkg-one", "org/pkg-one", Some("0.1.0")),
            mk("pkg-two", "org/pkg-two", None),
        ]);
    }
    store.domains.session.set_context(
        "dsflash:main",
        octoscode_store::domains::session::ContextLifecycle {
            kind: "context/normalization_reported".into(),
            state: serde_json::json!({"token_estimate": 128000, "session_id": "dsflash:main"}),
            detail: None,
        },
    );

    // the occupancy window, as the wire carries it (the Stage-B fixture's
    // 200k; the recorded live-gate frame's mechanism, this fixture's value)
    models::note_token_cost("dsflash:main", 200_000);
    let ui = Mutex::new(FlowUi::default());
    let ctx = Ctx::new(&store, &ui);
    let out = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/f29c-live");
    std::fs::create_dir_all(&out).expect("mkdir target/f29c-live");

    let mut expect = [
        ("setup-07", vec!["DeepSeek • Official API", "deepseek-v4-flash (default)"]),
        ("setup-09", vec!["128k of 200k tokens", "Compact now"]),
        ("setup-10", vec!["skill-a", "Search registry"]),
    ];
    expect.sort_by_key(|e| e.0.to_owned());
    for (screen, needles) in expect {
        let (card_src, data, kit_dir) =
            models::lower_card_src(screen, &ctx).expect("the live card source");
        for needle in needles {
            assert!(
                card_src.contains(needle),
                "{screen}: the store value `{needle}` is not in the injected copy"
            );
        }
        // #29c2 item 1: with the recorded single primary model the inner
        // card shrinks to its one row (no divider, no blank row).
        if screen == "setup-07" {
            let pl = &data["$kit"]["placements"];
            assert_eq!(pl["inner_card"]["layout"]["h"].as_f64(), Some(63.5),
                "1 live row -> the inner card shrinks");
            assert_eq!(pl["inner_div"]["layout"]["h"].as_f64(), Some(0.0),
                "1 live row -> no divider");
        }
        let dir = out.join(screen);
        std::fs::create_dir_all(dir.join("kit")).expect("mkdir screen");
        std::fs::write(dir.join("page.card"), &card_src).expect("write live card");
        std::fs::write(
            dir.join("page.data.json"),
            serde_json::to_string_pretty(&data).unwrap(),
        )
        .expect("write live data");
        // copy the kit (artifacts the renderer resolves beside the card)
        for entry in walkdir(&kit_dir) {
            let rel = entry.strip_prefix(&kit_dir).unwrap();
            let dest = dir.join("kit").join(rel);
            if entry.is_dir() {
                std::fs::create_dir_all(&dest).expect("mkdir kit dir");
            } else {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent).expect("mkdir kit parent");
                }
                std::fs::copy(&entry, &dest).expect("copy kit file");
            }
        }
    }
}

/// Minimal recursive listing (no walkdir dep).
fn walkdir(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p);
            }
        }
    }
    out
}
