//! A12 — the conversation's transport link.
//!
//! The rest of the app holds ONE stable command sender (the `Conversation`'s
//! `cmd_tx`, cloned into its `Client`) and reads ONE stable event receiver.
//! This module relays both to whichever WS transport is current, so a
//! transport that gave up (its 5-minute reconnect budget) or a "Retry now"
//! is REPLACED under the same `Conversation` — store, timeline, composer and
//! Session survive — exactly as the web's `#reconnect` replaces its transport
//! and keeps the retained Session (`active-session-runtime.ts:978-1000`,
//! `#replaceTransport` `:860-879`). The web never stops retrying an opened
//! Session (`#scheduleReconnect` `:951-976`, `#failCurrent(.., true)`); only a
//! voluntary disconnect does (`disconnect()` `:769-784`).
//!
//! The relay also keeps requests honest across an outage:
//! - while an up socket is gone (the outage), a request fails AT ONCE with a
//!   named error instead of parking in a dead socket's queue (the transport
//!   reads commands only while a socket is up, `ws/mod.rs` `run_live`), so no
//!   caller waits forever and nothing stale is replayed onto the next socket;
//! - a request whose socket drops before its reply fails then (its reply
//!   channel is dropped: the client reports a transport failure, which the
//!   turn controller treats as an UNCONFIRMED start, never as a rejection).
use std::sync::{Arc, Mutex};

use octos_app_transport::host::{self, HostPort};
use octos_app_transport::{ws, ConnectionState, OutboundCommand, TransportConfig, TransportEvent};
use octos_core::ui_protocol::RpcError;
use tokio::sync::{mpsc, oneshot, watch};

/// The code of a request the link refused while disconnected (JSON-RPC's
/// implementation-defined server-error range; nothing went on the wire).
pub const NOT_CONNECTED_CODE: i64 = -32000;

/// The message of that refusal (shown by surfaces that print a request's
/// error).
pub const NOT_CONNECTED: &str = "Not connected: Octos is reconnecting. Nothing was sent.";

/// What one transport state means for the conversation (the flow acts on it).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Transition {
    /// The very first socket of this link is up (the opener's own
    /// `session/open` is already queued).
    FirstSocket,
    /// A socket is up AGAIN after an earlier one: the active Session must be
    /// re-opened on it (the WS transport does not replay opens).
    Redial,
    /// The up socket is gone; the transport is backing off / re-dialing.
    Dropped,
    /// `Live` (a `session/open` answered on the current socket).
    Live,
    /// The current transport exited (`Failed`).
    GaveUp,
    /// Anything else (`Idle`, `Dialing`, a re-dial attempt while already
    /// dropped, `ReplayApplying`).
    Other,
}

#[derive(Default)]
struct State {
    /// The current transport's command sender; `None` once closed / dead.
    current: Option<mpsc::Sender<OutboundCommand>>,
    /// The current transport's event pump (aborted when it is replaced).
    pump: Option<tokio::task::JoinHandle<()>>,
    /// Which transport is current (bumped by every replace / close).
    generation: u64,
    /// A socket is up (Handshaking / Live / ReplayApplying).
    socket_up: bool,
    /// A socket was up at least once on this link.
    ever_up: bool,
    /// A voluntary disconnect: never re-dial again.
    closed: bool,
    /// How many times the transport was replaced.
    respawns: u32,
}

/// The link. Cheap to share (`Arc`).
pub struct Link {
    /// A19b — the config every (re)spawned transport dials with; its profile
    /// header can change ([`Link::carry_profile`]).
    cfg: Mutex<TransportConfig>,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
    handle: tokio::runtime::Handle,
    /// The stable event channel's sender; taken when the link closes or
    /// dies, so the conversation's drain ends (as it did when its one
    /// transport exited) instead of parking forever.
    events: Mutex<Option<mpsc::Sender<TransportEvent>>>,
    state: Mutex<State>,
    /// Bumped with `State::generation` (the relay follows a replace).
    gen_tx: watch::Sender<u64>,
    /// Bumped whenever an UP socket drops (in-flight requests fail on it).
    epoch_tx: watch::Sender<u64>,
    /// Inside a shell that hands the app a port to its own kernel
    /// ([`Link::start_host`]): the port, until the first transport takes it.
    /// Nothing replaces that transport: the port outlives a kernel restart,
    /// so a port that closed means the kernel is gone or the app is closing.
    host: Mutex<Option<HostPort>>,
    hosted: bool,
}

impl Link {
    /// Spawn the first transport plus the relay. Must run inside a tokio
    /// runtime (as `ws::spawn` does). Returns the link, the stable command
    /// sender and the stable event receiver.
    pub fn start(
        cfg: TransportConfig,
        waker: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> (Arc<Self>, mpsc::Sender<OutboundCommand>, mpsc::Receiver<TransportEvent>) {
        Self::start_with(cfg, waker, None)
    }

    /// [`Link::start`] over the shell's port instead of a socket.
    pub fn start_host(
        cfg: TransportConfig,
        port: HostPort,
        waker: Option<Arc<dyn Fn() + Send + Sync>>,
    ) -> (Arc<Self>, mpsc::Sender<OutboundCommand>, mpsc::Receiver<TransportEvent>) {
        Self::start_with(cfg, waker, Some(port))
    }

    fn start_with(
        cfg: TransportConfig,
        waker: Option<Arc<dyn Fn() + Send + Sync>>,
        port: Option<HostPort>,
    ) -> (Arc<Self>, mpsc::Sender<OutboundCommand>, mpsc::Receiver<TransportEvent>) {
        let (events, evt_rx) = mpsc::channel::<TransportEvent>(64);
        let (cmd_tx, cmd_rx) = mpsc::channel::<OutboundCommand>(64);
        let (gen_tx, _) = watch::channel(0u64);
        let (epoch_tx, _) = watch::channel(0u64);
        let link = Arc::new(Self {
            cfg: Mutex::new(cfg),
            waker,
            handle: tokio::runtime::Handle::current(),
            events: Mutex::new(Some(events)),
            state: Mutex::new(State::default()),
            gen_tx,
            epoch_tx,
            hosted: port.is_some(),
            host: Mutex::new(port),
        });
        link.spawn_transport();
        let relay = link.clone();
        link.handle.spawn(async move { relay.relay(cmd_rx).await });
        (link, cmd_tx, evt_rx)
    }

    /// Start a fresh transport and make it current (the previous one, if
    /// any, loses its command sender — it exits at its next socket — and its
    /// event pump, so nothing it says reaches the conversation any more).
    fn spawn_transport(self: &Arc<Self>) {
        let (t_tx, t_rx) = {
            let _guard = self.handle.enter();
            let cfg = self.cfg.lock().unwrap().clone();
            match self.host.lock().unwrap().take() {
                Some(port) => host::spawn_with_waker(cfg, port, None),
                None => ws::spawn(cfg),
            }
        };
        let mut st = self.state.lock().unwrap();
        if let Some(old) = st.pump.take() {
            old.abort();
        }
        st.generation += 1;
        let generation = st.generation;
        st.current = Some(t_tx);
        st.socket_up = false;
        let me = Arc::downgrade(self);
        st.pump = Some(self.handle.spawn(pump(me, generation, t_rx)));
        drop(st);
        let _ = self.gen_tx.send(generation);
    }

    /// A12 — replace the transport now (the banner's "Retry now", or the
    /// current one gave up). Refused after a voluntary disconnect.
    pub fn respawn(self: &Arc<Self>) -> bool {
        if self.hosted {
            ::log::info!("octoscode: link — the shell's port is not replaced");
            return false;
        }
        {
            let mut st = self.state.lock().unwrap();
            if st.closed {
                return false;
            }
            st.respawns += 1;
        }
        ::log::info!("octoscode: link — starting a fresh transport ({} so far)", self.respawns());
        self.spawn_transport();
        true
    }

    /// A19b — the profile the socket names in its `X-Profile-Id` header.
    pub fn header_profile(&self) -> String {
        self.cfg.lock().unwrap().profile_id.0.clone()
    }

    /// A19b — make the connection carry `profile` (its `X-Profile-Id`): Core
    /// resolves a Session id that does not embed its profile (`<profile>:main`
    /// — octos-core `SessionKey::profile_id` reads only `profile:channel:chat`)
    /// from the connection's routed profile, so without it a `session/hydrate`
    /// answers "unknown session". The header is fixed at the upgrade, so the
    /// transport is replaced (the A12 re-dial re-opens the active Session on
    /// the new socket). False when it already carries it, or after a
    /// voluntary disconnect.
    pub fn carry_profile(self: &Arc<Self>, profile: &str) -> bool {
        // A shell's port carries no header: the shell decides the profile.
        if self.hosted {
            return false;
        }
        {
            let mut cfg = self.cfg.lock().unwrap();
            if cfg.profile_id.0 == profile {
                return false;
            }
            cfg.profile_id = octos_app_transport::ProfileId::new(profile.to_owned());
        }
        self.respawn()
    }

    /// Close the link for good (a voluntary disconnect): no transport is
    /// current any more and none is started again.
    pub fn close(&self) {
        let mut st = self.state.lock().unwrap();
        if st.closed {
            return;
        }
        st.closed = true;
        st.current = None;
        st.socket_up = false;
        if let Some(p) = st.pump.take() {
            p.abort();
        }
        st.generation += 1;
        let g = st.generation;
        drop(st);
        self.events.lock().unwrap().take();
        let _ = self.gen_tx.send(g);
    }

    /// The current transport exited and nothing will replace it (a first
    /// connect that never came up): queued commands are dropped, so their
    /// callers fail instead of waiting.
    pub fn mark_dead(&self) {
        let mut st = self.state.lock().unwrap();
        st.current = None;
        st.socket_up = false;
        st.generation += 1;
        let g = st.generation;
        drop(st);
        self.events.lock().unwrap().take();
        let _ = self.gen_tx.send(g);
    }

    pub fn is_closed(&self) -> bool {
        self.state.lock().unwrap().closed
    }

    /// A socket was up at least once.
    pub fn ever_up(&self) -> bool {
        self.state.lock().unwrap().ever_up
    }

    /// An up socket dropped and no new one is up yet (requests refuse).
    pub fn in_outage(&self) -> bool {
        let st = self.state.lock().unwrap();
        st.ever_up && !st.socket_up && !st.closed
    }

    pub fn respawns(&self) -> u32 {
        self.state.lock().unwrap().respawns
    }

    /// Fold one transport state (called in order by the flow's drain).
    pub fn note_state(&self, s: &ConnectionState) -> Transition {
        let mut st = self.state.lock().unwrap();
        match s {
            ConnectionState::Handshaking | ConnectionState::ReplayApplying if st.socket_up => Transition::Other,
            ConnectionState::Handshaking | ConnectionState::ReplayApplying => {
                st.socket_up = true;
                let first = !st.ever_up;
                st.ever_up = true;
                if first {
                    Transition::FirstSocket
                } else {
                    Transition::Redial
                }
            }
            ConnectionState::Live => {
                st.socket_up = true;
                st.ever_up = true;
                Transition::Live
            }
            ConnectionState::Reconnecting { .. } if st.socket_up => {
                st.socket_up = false;
                drop(st);
                self.epoch_tx.send_modify(|e| *e += 1);
                Transition::Dropped
            }
            ConnectionState::Failed => {
                let was_up = st.socket_up;
                st.socket_up = false;
                drop(st);
                if was_up {
                    self.epoch_tx.send_modify(|e| *e += 1);
                }
                Transition::GaveUp
            }
            _ => Transition::Other,
        }
    }

    fn current(&self) -> Option<mpsc::Sender<OutboundCommand>> {
        self.state.lock().unwrap().current.clone()
    }

    /// The relay: every command from the app, in order, to the current
    /// transport (following a replace while one waits for room).
    async fn relay(self: Arc<Self>, mut rx: mpsc::Receiver<OutboundCommand>) {
        let mut gens = self.gen_tx.subscribe();
        while let Some(cmd) = rx.recv().await {
            if matches!(cmd, OutboundCommand::Disconnect) {
                // Best effort to the socket (a live transport closes it), then
                // the link closes: a re-dialing transport loses its sender and
                // exits at its next socket instead of reconnecting.
                if let Some(tx) = self.current() {
                    let _ = tx.try_send(OutboundCommand::Disconnect);
                }
                self.close();
                continue;
            }
            let Some(cmd) = self.admit(cmd) else { continue };
            let mut cmd = Some(cmd);
            while let Some(c) = cmd.take() {
                let Some(tx) = self.current() else {
                    // Closed / dead: the command is dropped (a request's
                    // caller gets "reply channel dropped").
                    break;
                };
                gens.borrow_and_update();
                tokio::select! {
                    permit = tx.reserve() => match permit {
                        Ok(p) => p.send(c),
                        Err(_) => {
                            // That transport exited: wait for its replacement
                            // (or the close) and try again.
                            cmd = Some(c);
                            if gens.changed().await.is_err() {
                                break;
                            }
                        }
                    },
                    r = gens.changed() => {
                        cmd = Some(c);
                        if r.is_err() {
                            break;
                        }
                    }
                }
            }
        }
        // Every sender is gone (the Conversation was dropped).
        self.close();
    }

    /// Admission at the relay: during an outage a request is refused at once
    /// (nothing is written anywhere); otherwise its reply is watched so a
    /// socket drop before the answer fails it.
    fn admit(&self, cmd: OutboundCommand) -> Option<OutboundCommand> {
        let outage = self.in_outage();
        match cmd {
            OutboundCommand::Request { method, params, reply } => {
                if outage {
                    ::log::info!("octoscode: link — {method} refused while reconnecting");
                    let _ = reply.send(Err(RpcError {
                        code: NOT_CONNECTED_CODE,
                        message: NOT_CONNECTED.to_owned(),
                        data: None,
                    }));
                    return None;
                }
                let (tx, rx) = oneshot::channel();
                let mut epochs = self.epoch_tx.subscribe();
                epochs.borrow_and_update();
                let m = method.clone();
                self.handle.spawn(async move {
                    tokio::select! {
                        r = rx => {
                            if let Ok(r) = r {
                                let _ = reply.send(r);
                            }
                            // Dropped by the transport: drop ours too.
                        }
                        _ = epochs.changed() => {
                            ::log::info!("octoscode: link — {m}: the socket dropped before its reply");
                            drop(reply);
                        }
                    }
                });
                Some(OutboundCommand::Request { method, params, reply: tx })
            }
            other if outage => {
                ::log::info!("octoscode: link — {} dropped while reconnecting", command_name(&other));
                None
            }
            other => Some(other),
        }
    }
}

/// Forward one transport's events to the conversation's stable channel,
/// then wake the UI thread (makepad's `SignalToUI`).
async fn pump(link: std::sync::Weak<Link>, generation: u64, mut rx: mpsc::Receiver<TransportEvent>) {
    while let Some(evt) = rx.recv().await {
        let Some(link) = link.upgrade() else { return };
        if link.state.lock().unwrap().generation != generation {
            return; // replaced: a retired transport never speaks again
        }
        let Some(tx) = link.events.lock().unwrap().clone() else { return };
        if tx.send(evt).await.is_err() {
            return;
        }
        if let Some(w) = &link.waker {
            w();
        }
    }
}

fn command_name(c: &OutboundCommand) -> &'static str {
    match c {
        OutboundCommand::OpenSession(_) | OutboundCommand::OpenSessionFresh(_) => "session/open",
        OutboundCommand::ListSessions => "session/list",
        OutboundCommand::HydrateSession { .. } => "session/hydrate",
        OutboundCommand::StartTurn(_) => "turn/start",
        OutboundCommand::InterruptTurn(_) => "turn/interrupt",
        OutboundCommand::SendApprovalResponse { .. } => "approval/respond",
        OutboundCommand::FetchDiffPreview { .. } => "diff/preview/get",
        OutboundCommand::RequestTaskOutput { .. } => "task/output/read",
        OutboundCommand::Request { .. } => "request",
        OutboundCommand::Disconnect => "disconnect",
    }
}
