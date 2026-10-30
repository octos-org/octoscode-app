//! Exercise hosted session opens through the production transport and flow.
//! Only the shell's port is scripted; paths use a real temporary filesystem.

mod a20_common;

use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc, Arc,
};
use std::time::Duration;

use octos_app_transport::host::{HostPort, HostPortEvent};
use octoscode_module::flow::{Conversation, FlowEvent};
use octoscode_module::screens::{board1, recents, sidebar, workspace};
use serde_json::{json, Value};

// The navigation cache is process-wide, just as in the running app.
static NAVIGATION: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct NavigationState {
    endpoint: String,
    storage: Arc<dyn recents::Storage>,
}

impl NavigationState {
    fn isolated() -> Self {
        let previous = Self {
            endpoint: recents::endpoint(),
            storage: recents::store(),
        };
        recents::set_connected_endpoint("https://navigation.example");
        recents::set_store(Arc::new(recents::MemoryStore::new()));
        previous
    }
}

impl Drop for NavigationState {
    fn drop(&mut self) {
        recents::set_connected_endpoint(&self.endpoint);
        recents::set_store(self.storage.clone());
    }
}

struct Workspace(PathBuf);

impl Workspace {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "octoscode-host-workspace-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        std::fs::create_dir(path.join("canonical")).unwrap();
        std::fs::create_dir(path.join("other")).unwrap();
        Self(path)
    }

    #[cfg(unix)]
    fn symlink(&self) {
        std::os::unix::fs::symlink(self.0.join("canonical"), self.0.join("alias")).unwrap();
    }

    fn path(&self, name: &str) -> String {
        self.0.join(name).to_str().unwrap().to_owned()
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}

struct ClosePort(mpsc::Sender<HostPortEvent>);

impl Drop for ClosePort {
    fn drop(&mut self) {
        let _ = self.0.send(HostPortEvent::Closed("test finished".into()));
    }
}

#[derive(Clone, Copy)]
enum Entry {
    Direct,
    Workspace,
    Picker,
}

async fn open(requested: String, returned: String) -> (bool, Option<String>) {
    let (accepted, conv) = open_via(requested, returned, Entry::Direct).await;
    (
        accepted,
        conv.store
            .domains
            .session
            .workspace_root(&conv.session_id()),
    )
}

async fn open_via(requested: String, returned: String, entry: Entry) -> (bool, Arc<Conversation>) {
    let (tx, rx) = mpsc::channel();
    let _close = ClosePort(tx.clone());
    let root = returned.clone();
    let port = HostPort {
        send: Box::new(move |text| {
            let request: Value = serde_json::from_str(&text).unwrap();
            let session = &request["params"]["session_id"];
            let result = match request["method"].as_str().unwrap() {
                "session/open" => json!({"opened": {
                    "session_id": session, "active_profile_id": "_main",
                    "workspace_root": root,
                    "cursor": {"stream": session, "seq": 1},
                    "capabilities": {
                        "version": {"protocol": "octos-ui/v1alpha1", "schema_version": 1, "jsonrpc": "2.0"},
                        "capabilities_schema_version": 2,
                        "supported_methods": ["session/open", "session/hydrate"],
                        "supported_notifications": [],
                        "supported_features": ["state.session_hydrate.v1"]
                    }
                }}),
                "session/hydrate" => {
                    json!({"session_id": session, "cursor": {"stream": session, "seq": 2}, "messages": []})
                }
                _ => json!({}),
            };
            tx.send(HostPortEvent::Frame(
                json!({
                    "jsonrpc": "2.0", "id": request["id"], "result": result
                })
                .to_string(),
            ))
            .map_err(|e| e.to_string())
        }),
        recv: Box::new(move || {
            rx.recv()
                .unwrap_or(HostPortEvent::Closed("test ended".into()))
        }),
    };
    let (conv, mut events) = Conversation::connect_host(port, "_main", None, None).unwrap();
    let conv = Arc::new(conv);
    match entry {
        Entry::Direct => {
            conv.open_workspace(Some(requested)).await.unwrap();
        }
        Entry::Workspace => workspace::apply(workspace::Effect::Open(requested), &conv)
            .await
            .unwrap(),
        Entry::Picker => board1::execute(
            board1::Work::NewSession { cwd: requested },
            Some(conv.clone()),
        )
        .await
        .unwrap(),
    }
    if !matches!(entry, Entry::Direct) {
        assert_eq!(
            recents::load_recent_workspaces(&*recents::store(), &recents::endpoint()).len(),
            3,
            "sending an open must not reconcile navigation before the reply is accepted"
        );
    }
    let accepted = tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = events.recv().await {
            match conv.on_event(event) {
                FlowEvent::WorkspaceOpened(_) => return true,
                FlowEvent::Other(reason) if reason == "session/open-workspace-mismatch" => {
                    return false
                }
                _ => {}
            }
        }
        panic!("host port closed before session/open settled");
    })
    .await
    .expect("session/open reply");
    (accepted, conv)
}

#[cfg(unix)]
#[tokio::test]
async fn hosted_open_accepts_a_symlink_to_the_returned_workspace() {
    let _navigation = NAVIGATION.lock().await;
    let _state = NavigationState::isolated();
    let workspace = Workspace::new();
    workspace.symlink();
    let canonical = std::fs::canonicalize(workspace.path("canonical"))
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let (accepted, root) = open(workspace.path("alias"), canonical.clone()).await;
    assert!(
        accepted,
        "the shell canonicalized the requested workspace, not another directory"
    );
    assert_eq!(root, Some(canonical));
}

#[tokio::test]
async fn hosted_open_still_rejects_a_different_directory() {
    let _navigation = NAVIGATION.lock().await;
    let _state = NavigationState::isolated();
    let workspace = Workspace::new();
    let (accepted, root) = open(workspace.path("canonical"), workspace.path("other")).await;
    assert!(!accepted);
    assert_eq!(root, None);
}

#[tokio::test]
async fn hosted_open_does_not_equate_two_unresolvable_paths() {
    let _navigation = NAVIGATION.lock().await;
    let _state = NavigationState::isolated();
    let workspace = Workspace::new();
    let (accepted, root) = open(workspace.path("missing-one"), workspace.path("missing-two")).await;
    assert!(!accepted);
    assert_eq!(root, None);
}

#[tokio::test]
async fn remote_open_does_not_use_local_path_equivalence_as_server_identity() {
    let _navigation = NAVIGATION.lock().await;
    let _state = NavigationState::isolated();
    let workspace = Workspace::new();
    let canonical = std::fs::canonicalize(workspace.path("canonical"))
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let server = a20_common::Server::start(a20_common::Script {
        open_root: Some((a20_common::MAIN.to_owned(), canonical)),
        ..Default::default()
    })
    .await;
    let (conv, mut events) =
        Conversation::connect(&server.base, "dummy", a20_common::PROFILE, None, None).unwrap();
    conv.open_workspace_as(a20_common::MAIN, Some(workspace.path("canonical/.")))
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while let Some(event) = events.recv().await {
            match conv.on_event(event) {
                FlowEvent::Other(reason) if reason == "session/open-workspace-mismatch" => return,
                FlowEvent::WorkspaceOpened(_) => {
                    panic!("local path equivalence must not authorize a remote workspace")
                }
                _ => {}
            }
        }
        panic!("no workspace refusal");
    })
    .await
    .unwrap();
    assert_eq!(
        conv.store.domains.session.workspace_root(a20_common::MAIN),
        None
    );
}

#[cfg(unix)]
#[tokio::test]
async fn hosted_picker_open_unifies_recents_history_and_sidebar_after_acceptance() {
    let _navigation = NAVIGATION.lock().await;
    let _state = NavigationState::isolated();
    let directory = Workspace::new();
    directory.symlink();
    let alias = directory.path("alias");
    let unrelated = directory.path("other");
    let canonical = std::fs::canonicalize(directory.path("canonical"))
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    for entry in [Entry::Workspace, Entry::Picker] {
        let storage = Arc::new(recents::MemoryStore::new());
        recents::set_store(storage.clone());
        let endpoint = &recents::endpoint();
        // Simulate an older cache containing both spellings, without migrating
        // any unrelated directory or needing a real kernel/model request.
        recents::remember_workspace(&*storage, endpoint, &unrelated, 0);
        recents::remember_workspace(&*storage, endpoint, &canonical, 1);
        recents::remember_workspace(&*storage, endpoint, &alias, 2);
        let (accepted, conv) = open_via(alias.clone(), canonical.clone(), entry).await;
        assert!(
            accepted,
            "hosted alias must be accepted before navigation is reconciled"
        );
        assert_eq!(
            conv.store
                .domains
                .session
                .workspace_root(&conv.session_id()),
            Some(canonical.clone())
        );
        let projection = sidebar::project(&conv.store, recents::now_ms());
        assert_eq!(
            projection.groups.len(),
            2,
            "the opened directory must have one group alongside the unrelated workspace"
        );
        assert_eq!(projection.groups[0].key, canonical);
        assert_eq!(projection.groups[1].key, unrelated);
        let rows = recents::load_recent_workspaces(&*storage, endpoint);
        assert_eq!(
            rows.iter().map(|r| &r.path).collect::<Vec<_>>(),
            vec![&canonical, &unrelated]
        );
        assert_eq!(
            recents::history_workspaces(&*storage, endpoint),
            vec![canonical.clone(), unrelated.clone()]
        );
        assert_eq!(
            board1::picker()
                .recents
                .iter()
                .map(|(_, path)| path)
                .collect::<Vec<_>>(),
            vec![&canonical, &unrelated]
        );
    }
}

#[tokio::test]
async fn hosted_open_preserves_exact_match_without_local_resolution() {
    let _navigation = NAVIGATION.lock().await;
    let _state = NavigationState::isolated();
    let workspace = Workspace::new();
    let missing = workspace.path("missing");
    let (accepted, root) = open(missing.clone(), missing.clone()).await;
    assert!(
        accepted,
        "the existing exact-match rule does not resolve the path locally"
    );
    assert_eq!(root, Some(missing));
}

#[cfg(unix)]
#[tokio::test]
async fn hosted_open_uses_the_navigation_endpoint_for_all_workspace_indexes() {
    let _navigation = NAVIGATION.lock().await;
    let _state = NavigationState::isolated();
    let directory = Workspace::new();
    directory.symlink();
    let canonical = std::fs::canonicalize(directory.path("canonical"))
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let (accepted, conv) =
        open_via(directory.path("alias"), canonical.clone(), Entry::Direct).await;
    assert!(accepted);
    let endpoint = recents::endpoint();
    assert_ne!(
        endpoint,
        conv.http_base(),
        "the navigation key must deliberately differ from the transport origin"
    );
    let storage = recents::store();
    assert!(
        recents::load_recent_workspaces(&*storage, &conv.http_base()).is_empty(),
        "the accepted open must not write navigation under the transport origin"
    );
    assert!(recents::history_workspaces(&*storage, &conv.http_base()).is_empty());
    assert_eq!(
        recents::load_recent_workspaces(&*storage, &endpoint)
            .iter()
            .map(|r| &r.path)
            .collect::<Vec<_>>(),
        vec![&canonical]
    );
    assert_eq!(
        recents::history_workspaces(&*storage, &endpoint),
        vec![canonical.clone()]
    );
    let projection = sidebar::project(&conv.store, recents::now_ms());
    assert_eq!(projection.groups.len(), 1);
    assert_eq!(projection.groups[0].key, canonical);
}
