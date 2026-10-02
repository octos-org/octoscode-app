//! A29 — parity row 6: the `/btw` aside on this conversation, owned by the
//! Session that asked.
//!
//! The web's chain, natively: the composer's command layer
//! (`intent.ts:96-104`, `App.tsx:1199-1203`) admits `/btw <question>` through
//! the Session's controller (`use-octos-session.ts:3503-3522` →
//! `LazyBtwController.ask`, `lazy-btw-controller.ts:85-105`), which captures
//! the asking Session BEFORE the call (`:89-95`); `session/btw
//! {session_id, question}` (`packages/client/src/btw.ts:77-81`) answers in one
//! reply; the reply settles only the request that still answers in THAT
//! Session (`#owns` / `#current`, `:128-177`). The state lives in the store's
//! `btw` domain (`octoscode_store::domains::btw`), so the panel above the
//! composer (`screens::btw`) and the sidebar marker read the same per-Session
//! record.
//!
//! A child module of `flow` (like `flow_controller.rs`), so it reads the
//! conversation's private plumbing without widening its API.
use std::time::Duration;

use octoscode_client::ClientError;
use octoscode_store::domains::btw::{Admission, Gate, Reply, Settled, Ticket};
use serde_json::json;

use super::{Conversation, Direction};

/// The `session/btw` reply budget: the web client's default request timeout
/// (`packages/client/src/client.ts:199` `requestTimeoutMs: 30_000`), after
/// which the aside fails ("The aside could not be answered. Try again.").
/// `OCTOSCODE_BTW_TIMEOUT_MS` overrides it for a fixture walk.
pub fn btw_timeout() -> Duration {
    let ms = std::env::var("OCTOSCODE_BTW_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30_000);
    Duration::from_millis(ms)
}

/// The web's usage hint for `/btw` without a question (`intent.ts:99-104`).
pub const USAGE_HINT: &str = "Use /btw <question> for a temporary side answer. Nothing was sent to the model.";

impl Conversation {
    /// The Session whose composer is on screen — the Session an aside typed
    /// there belongs to (the window's active Session; the flow's own id
    /// before any is active).
    pub fn composer_session(&self) -> String {
        self.store
            .active_session()
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| self.session_id())
    }

    /// The admission gate's facts now: `session/btw` advertised as a METHOD
    /// (`supportsMethod`, `btw.ts:66` / `interaction.ts:14-22` — octos lists
    /// it in `supported_methods`) and the Session's connection live and
    /// healthy (`#current`: connected, no recovery in progress).
    pub fn btw_gate(&self) -> Gate {
        Gate {
            advertised: crate::screens::dialog::advertises(&self.store, "session/btw"),
            connected: self.store.is_live() && !self.in_outage(),
        }
    }

    /// `askBtw` (`use-octos-session.ts:3503-3522`): admit `question` for the
    /// composer's Session, captured NOW. Nothing is sent unless accepted.
    pub fn admit_btw(&self, question: &str) -> Admission {
        let session = self.composer_session();
        self.store.domains.btw.ask(&session, question, self.btw_gate())
    }

    /// `session/btw` for an admitted ticket, then its settle into the ASKING
    /// Session (`#run`, `lazy-btw-controller.ts:157-198`). The ticket carries
    /// the Session captured at admission — the call never re-reads which
    /// Session is on screen, so a switch while it runs changes nothing.
    pub async fn run_btw(&self, ticket: Ticket) -> Settled {
        self.run_btw_detailed(ticket).await.0
    }

    /// [`Conversation::run_btw`] with why the call failed, when it did (the
    /// screens table's `apply` reports it).
    pub async fn run_btw_detailed(&self, ticket: Ticket) -> (Settled, Option<String>) {
        let params = json!({"session_id": ticket.session, "question": ticket.question});
        self.trace.record(
            self.started,
            Direction::Out,
            "session/btw",
            None,
            Some(format!("session={} request={}", ticket.session, ticket.request_id)),
        );
        let reply = tokio::time::timeout(btw_timeout(), self.client.request("session/btw", params)).await;
        let (outcome, why) = match reply {
            Ok(Ok(v)) => {
                // The reply as it arrived (card #13's frame trace carries the
                // outbound half; the RPC result is not a notification, so the
                // event pump never records it).
                self.client.trace().result("session/btw", None, &v);
                match crate::screens::sessions::parse_aside_reply(&v, &ticket.session) {
                    Ok((answer, model)) => (Reply::Answer { answer, model }, String::new()),
                    Err(e) => (Reply::Error, e),
                }
            }
            Ok(Err(e)) => (Reply::Error, e.to_string()),
            Err(_) => (Reply::Error, format!("no reply within {} ms", btw_timeout().as_millis())),
        };
        // `#current` at reply time: a reply on a connection that is no longer
        // live settles stale, whatever it says.
        let connected = self.store.is_live() && !self.in_outage();
        let settled = self.store.domains.btw.settle(&ticket, outcome, connected);
        makepad_widgets::log!(
            "[octoscode] aside {} #{} -> {settled:?}{}",
            ticket.session,
            ticket.request_id,
            if why.is_empty() { String::new() } else { format!(" ({why})") }
        );
        makepad_widgets::SignalToUI::set_ui_signal();
        (settled, (!why.is_empty()).then_some(why))
    }

    /// Run an admitted ticket's call on the runtime without holding the
    /// caller (the composer returns at once, as the web's `void #run`).
    fn spawn_btw(&self, ticket: Ticket) -> Option<Ticket> {
        match self.shared() {
            Some(me) => {
                tokio::spawn(async move {
                    me.run_btw(ticket).await;
                });
                None
            }
            None => Some(ticket),
        }
    }

    /// `/btw <question>` (alias `/aside`) from the composer: the web's
    /// command layer (`intent.ts:72-104`) — the method's availability first
    /// ("/btw is unavailable"), then the question ("Use /btw <question> …"),
    /// then the Session's admission (`App.tsx:1199-1203`): an accepted aside
    /// consumes the draft; a refused one (busy, not live) keeps it.
    pub async fn btw_command(&self, args: &str) -> Admission {
        let session = self.composer_session();
        let gate = self.btw_gate();
        if !gate.advertised {
            self.btw_receipt(&session, crate::i18n::tr("Not supported by this server"));
            self.consume_draft(&session);
            return Admission::Unavailable;
        }
        if args.trim().is_empty() {
            self.btw_receipt(&session, crate::i18n::tr(USAGE_HINT));
            self.consume_draft(&session);
            return Admission::Empty;
        }
        let admission = self.store.domains.btw.ask(&session, args, gate);
        match &admission {
            Admission::Accepted(ticket) => {
                self.consume_draft(&session);
                makepad_widgets::log!(
                    "[octoscode] aside asked in {session} (#{}): answering",
                    ticket.request_id
                );
                makepad_widgets::SignalToUI::set_ui_signal();
                if let Some(ticket) = self.spawn_btw(ticket.clone()) {
                    self.run_btw(ticket).await;
                }
            }
            other => {
                makepad_widgets::log!("[octoscode] aside refused in {session}: {other:?} (draft kept)");
            }
        }
        admission
    }

    /// The composer's own submit (Enter, the send control): an EMPTY submit
    /// dismisses this Session's aside (`App.tsx:1170-1173`), then the
    /// ordinary path. Programmatic sends (Resume chat) call `submit_draft`
    /// directly and never dismiss, as the web's `resumeChat` never calls
    /// `submit`.
    pub async fn submit_composer(&self) -> Result<String, ClientError> {
        if self.ui.lock().unwrap().draft().trim().is_empty() {
            let session = self.composer_session();
            if self.store.domains.btw.dismiss(&session) {
                makepad_widgets::log!("[octoscode] empty submit: the aside of {session} dismissed");
                makepad_widgets::SignalToUI::set_ui_signal();
            }
        }
        self.submit_draft().await
    }

    /// The local report of a `/btw` that sent nothing
    /// (`local-report.ts:112-119`: "/btw is unavailable" over the reason),
    /// one receipt row in the asking Session's transcript.
    fn btw_receipt(&self, session: &str, reason: &str) {
        let title = crate::i18n::tr_with("/{command} is unavailable", &[("command", "btw")]);
        // The receipt renders as Markdown: `<question>` must stay text.
        let text = format!("**{title}**\n\n{}", reason.replace('<', "\\<"));
        self.store.domains.session.timeline.append(
            session,
            Some(crate::screens::palette::next_receipt_turn()),
            crate::screens::palette::REPORT_KIND,
            text,
        );
        makepad_widgets::SignalToUI::set_ui_signal();
        makepad_widgets::log!("[octoscode] command /btw: reported ({reason}), nothing sent");
    }

    /// The command was taken: the composer clears and so does the Session's
    /// saved draft (A8 "sent drafts stay cleared").
    fn consume_draft(&self, session: &str) {
        self.ui.lock().unwrap().set_draft_inner(String::new());
        crate::drafts::save(session, "");
    }
}
