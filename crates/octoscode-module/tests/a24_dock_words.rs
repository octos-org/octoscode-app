//! A24 (parity row 117) — the sidebar's peer dock and the Fleet name every
//! peer status with ONE word, in every interface language.
//!
//! The web's dock reads the Fleet's own model (`PeerDock.tsx` renders
//! `fleetStatusWord`), so a peer the Fleet calls "Waiting for your approval"
//! is never "Blocked" or "needs you" in the sidebar. Here both surfaces are
//! lowered from one store holding a peer in EVERY status — the Fleet pane
//! (`b3_fleet_row_<i>_status`, its chip "<glyph> <word>") and the dock
//! (`pd_row_<i>_status`) — and what each DRAWS is compared row by row, in
//! English and in Chinese.

use std::sync::Mutex;

use octoscode_module::i18n::{self, Lang};
use octoscode_module::screens::board3::fleetview::{self, Status};
use octoscode_module::screens::board3::host::{self as b3, Dialog};
use octoscode_module::screens::peer_dock::{self as dock, Seat};
use octoscode_module::screens::{fleet_driver, peers};
use octoscode_store::domains::peer::{Activity, Disclosure, FleetInventory, InventoryOp, Origin, Outcome, PeerRow, RequestKind, RowStatus};
use octoscode_store::Store;

const SESSION: &str = "dsflash:main";
const DESKTOP: Seat = Seat { compact: false, width: 260.0, room: 600.0 };

/// Every word the Fleet's model has (`FleetStatusWord`).
const ALL: [Status; 10] = [
    Status::Requested,
    Status::Starting,
    Status::StillStarting,
    Status::Working,
    Status::WaitingApproval,
    Status::WaitingAnswer,
    Status::Finished,
    Status::Stopped,
    Status::Failed,
    Status::Unknown,
];

/// The board-3 state, the dock's state and the interface language are
/// process statics: one test at a time.
fn serial() -> std::sync::MutexGuard<'static, ()> {
    static L: Mutex<()> = Mutex::new(());
    L.lock().unwrap_or_else(|e| e.into_inner())
}

fn fresh() {
    b3::reset();
    fleet_driver::reset_seat();
    dock::reset();
    i18n::set_language(Lang::En);
    std::env::set_var("OCTOSCODE_DRIVER_ID_PATH", std::env::temp_dir().join("a24-dock-words-driver-id"));
}

/// A started peer with a model and an accepted operation.
fn peer(slug: &str, now: u64) -> PeerRow {
    let mut r = PeerRow::opening(&format!("m#peer-{slug}"), slug, Origin::Dispatch, "00000000-0000-4000-8000-0000000000d9", now);
    r.status = RowStatus::Started;
    r.operation_id = Some(format!("op-{slug}"));
    r.model = Some("glm-4.6".into());
    r.accepted_at_ms = Some(now - 60_000);
    r
}

/// One store whose Fleet holds a peer in every status: the inventory's
/// accepted operation the roster has not seen yet (Requested), then the
/// roster's rows through their lifecycle, blocks and outcomes.
fn every_status(now: u64) -> Store {
    let store = Store::new();
    store.set_active(Some(SESSION.into()));
    let p = &store.domains.peer;
    p.set_inventory(Some(FleetInventory::Complete {
        session_id: SESSION.into(),
        snapshot: "snap-1".into(),
        observed_revision: "1".into(),
        operations: vec![InventoryOp {
            operation_id: "op-requested".into(),
            slug: "requested".into(),
            lifecycle: "accepted".into(),
            adopted_session_id: "s-requested".into(),
            adopted_turn_id: "t-requested".into(),
            workspace_root: String::new(),
            model: "glm-4.6".into(),
            model_lane: String::new(),
            goal_id: None,
            accepted_at_ms: now - 1_000,
        }],
        disclosure: Disclosure { mode: "internal".into(), recovery: "none".into(), binding: None },
        completed_at_ms: now,
    }));
    let mut starting = peer("starting", now);
    starting.status = RowStatus::Opening;
    starting.opening_since_ms = now - 1_000;
    let mut slow = peer("slow", now);
    slow.status = RowStatus::Opening;
    slow.opening_since_ms = now - 60_000;
    let blocked = |slug: &str, kind: RequestKind| {
        let mut r = peer(slug, now);
        r.activity = Activity::Blocked;
        r.request_kind = Some(kind);
        r.request_id = Some(format!("req-{slug}"));
        r
    };
    let ended = |slug: &str, outcome: Outcome| {
        let mut r = peer(slug, now);
        r.activity = Activity::Done;
        r.outcome = Some(outcome);
        r.finished_at_ms = Some(now - 1_000);
        r
    };
    let mut unknown = peer("unknown", now);
    unknown.status = RowStatus::Unknown;
    for r in [
        starting,
        slow,
        peer("working", now),
        blocked("approval", RequestKind::Approval),
        blocked("answer", RequestKind::Question),
        ended("finished", Outcome::Finished),
        ended("stopped", Outcome::Stopped),
        ended("failed", Outcome::Failed),
        unknown,
    ] {
        assert!(p.stage_row(r, false));
    }
    store
}

/// The text a lowered `Label` named `id` draws (`text: "<lit>"`).
fn label_text(dsl: &str, id: &str) -> Option<String> {
    let head = format!("{id} := Label {{\n");
    let at = dsl.find(&head)? + head.len();
    let props = &dsl[at..];
    let start = props.find("text: \"")? + "text: \"".len();
    let mut out = String::new();
    let mut chars = props[start..].chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                other => out.push(other),
            },
            c => out.push(c),
        }
    }
    None
}

/// The Fleet pane as the app mounts it, its ended rows unfolded.
fn fleet_dsl(store: &Store) -> String {
    b3::reset();
    let _ = b3::open(Dialog::Fleet);
    let _ = b3::perform("b3.fleet.finished", 0, store);
    b3::lower_open(store).expect("the Fleet is open").dsl
}

/// Per row (the dock numbers its rows in the Fleet's order): the status, the
/// Fleet chip's text and the dock's word.
fn drawn(store: &Store, now: u64) -> Vec<(Status, String, String)> {
    let rows = fleetview::rows(store, now);
    let fleet = fleet_dsl(store);
    let docked = dock::lower(store, now, DESKTOP).expect("the dock shows");
    rows.iter()
        .enumerate()
        .map(|(i, r)| {
            let chip = label_text(&fleet, &format!("b3_fleet_row_{i}_status")).unwrap_or_else(|| panic!("the Fleet draws row {i}'s status"));
            let word = label_text(&docked.dsl, &format!("pd_row_{i}_status")).unwrap_or_else(|| panic!("the dock draws row {i}'s status"));
            (r.status, chip, word)
        })
        .collect()
}

/// The dock draws, for EVERY status, the very word the Fleet's chip draws
/// (after the chip's glyph) — in English, the web's `FleetStatusWord`; in
/// Chinese, the web's zh for it (never English, never a second wording).
#[test]
fn the_dock_draws_the_fleets_word_for_every_status_in_en_and_zh() {
    let _s = serial();
    fresh();
    let now = peers::now_ms();
    let store = every_status(now);
    let statuses: Vec<Status> = fleetview::rows(&store, now).iter().map(|r| r.status).collect();
    for s in ALL {
        assert_eq!(statuses.iter().filter(|x| **x == s).count(), 1, "one row is {s:?}: {statuses:?}");
    }
    for lang in [Lang::En, Lang::Zh] {
        i18n::set_language(lang);
        let rows = drawn(&store, now);
        i18n::set_language(Lang::En);
        assert_eq!(rows.len(), ALL.len());
        for (status, chip, word) in rows {
            assert_eq!(chip, format!("{} {word}", status.glyph()), "{lang:?} {status:?}: the dock's word is the Fleet chip's");
            match lang {
                Lang::En => assert_eq!(word, status.word(), "{status:?}: the web's FleetStatusWord"),
                Lang::Zh => {
                    assert_ne!(word, status.word(), "{status:?} reads Chinese in the dock and the Fleet");
                    assert!(word.chars().any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c)), "{status:?}: {word}");
                }
            }
        }
    }
}

/// The words themselves: each status has its own word in each language (no
/// two statuses collapse into one wording), and the shared function is the
/// Fleet's `t(FleetStatusWord)` the dock now reads.
#[test]
fn every_status_has_its_own_word_in_each_language() {
    let _s = serial();
    fresh();
    for lang in [Lang::En, Lang::Zh] {
        i18n::set_language(lang);
        let words: Vec<String> = ALL.iter().map(|s| fleetview::status_word(*s)).collect();
        i18n::set_language(Lang::En);
        for (i, w) in words.iter().enumerate() {
            assert!(!w.trim().is_empty(), "{lang:?} {:?}", ALL[i]);
            assert_eq!(words.iter().filter(|x| *x == w).count(), 1, "{lang:?}: {w} names one status");
        }
    }
}
