//! Host-port transport: inside a shell that runs its own octos kernel
//! (OctoSense, ADR 0003 "An app that is an octos client"), the app does not
//! dial a server or hold a token. The shell hands it a port, and frames go
//! through the shell, which holds them to the app's scope.
//!
//! Lifecycle: `Idle → Dialing → Handshaking (awaiting session/open) → Live`,
//! as over a socket. When the shell's kernel restarts, the port says
//! `Reset`: requests still waiting fail with a clear error, the transport
//! emits `Reconnecting` and opens every session it had opened again from its
//! replay cursor. A closed port is `Failed`.
//!
//! The port itself is the embedder's (Makepad's `OctosUiPort` in the
//! OctoSense module): this crate sees it as a [`HostPort`], so it stays free
//! of Makepad.

use std::sync::Arc;

use octos_core::ui_protocol::RpcError;
use tokio::sync::mpsc;

use crate::proto::{
    build_outbound, build_reopens, emit, fail_all_pending, handle_inbound_text, Outbound,
    SharedState, CHANNEL_BUFFER,
};
use crate::{ConnectionState, OutboundCommand, TransportConfig, TransportEvent};

/// JSON-RPC error code for requests the restarted kernel never answered.
pub const KERNEL_RESTARTED: i64 = -32099;

/// What the host says on a port.
#[derive(Debug, Clone, PartialEq)]
pub enum HostPortEvent {
    /// A kernel frame for the app.
    Frame(String),
    /// The host's kernel restarted; the port goes on.
    Reset(String),
    /// The port ended.
    Closed(String),
}

/// The app's end of a port to its host's kernel.
pub struct HostPort {
    /// Send one JSON-RPC frame. `Err` once the port is closed.
    pub send: Box<dyn Fn(String) -> Result<(), String> + Send + Sync>,
    /// Wait for the host's next event. It runs on a thread of its own and is
    /// not called again after [`HostPortEvent::Closed`].
    pub recv: Box<dyn FnMut() -> HostPortEvent + Send>,
}

/// Like the socket transport's `spawn_with_waker`, over `port`. `cfg`
/// supplies the replay cursor and its file; its URL and bearer are unused.
pub fn spawn_with_waker(
    cfg: TransportConfig,
    port: HostPort,
    waker: Option<Arc<dyn Fn() + Send + Sync>>,
) -> (mpsc::Sender<OutboundCommand>, mpsc::Receiver<TransportEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::channel::<OutboundCommand>(CHANNEL_BUFFER);
    let (evt_tx, evt_rx) = mpsc::channel::<TransportEvent>(CHANNEL_BUFFER);
    match waker {
        None => {
            tokio::spawn(run(cfg, port, cmd_rx, evt_tx));
        }
        Some(wake) => {
            let (inner_tx, mut inner_rx) = mpsc::channel::<TransportEvent>(CHANNEL_BUFFER);
            tokio::spawn(run(cfg, port, cmd_rx, inner_tx));
            tokio::spawn(async move {
                while let Some(evt) = inner_rx.recv().await {
                    if evt_tx.send(evt).await.is_err() {
                        break;
                    }
                    wake();
                }
            });
        }
    }
    (cmd_tx, evt_rx)
}

async fn run(
    cfg: TransportConfig,
    port: HostPort,
    mut commands: mpsc::Receiver<OutboundCommand>,
    events: mpsc::Sender<TransportEvent>,
) {
    let HostPort { send, mut recv } = port;
    // The host's events, from its blocking receive onto this task.
    let (in_tx, mut inbound) = mpsc::unbounded_channel::<HostPortEvent>();
    let reader = std::thread::Builder::new().name("octoscode-host-port".into()).spawn(move || loop {
        let event = recv();
        let closed = matches!(event, HostPortEvent::Closed(_));
        if in_tx.send(event).is_err() || closed {
            break;
        }
    });
    emit(&events, TransportEvent::ConnectionState(ConnectionState::Idle)).await;
    if let Err(e) = reader {
        log::error!("host port: reader thread: {e}");
        emit(&events, TransportEvent::ConnectionState(ConnectionState::Failed)).await;
        return;
    }
    emit(&events, TransportEvent::ConnectionState(ConnectionState::Dialing)).await;
    let persist = cfg.cursor_file.clone().map(|p| {
        Arc::new(crate::cursor::FileCursorPersist::new(p)) as Arc<dyn crate::cursor::CursorPersist>
    });
    let mut shared = SharedState::new(cfg.cursor.clone(), persist);
    let mut state = ConnectionState::Handshaking;
    emit(&events, TransportEvent::ConnectionState(ConnectionState::Handshaking)).await;
    let mut attempt: u32 = 0;
    loop {
        tokio::select! {
            biased;
            cmd = commands.recv() => {
                let Some(cmd) = cmd else { break };
                match build_outbound(cmd, &mut shared) {
                    Outbound::Disconnect => break,
                    Outbound::Skip => {}
                    Outbound::Send { id, frame, pending } => {
                        if let Err(e) = send(frame) {
                            log::warn!("host port: {e}");
                            emit(&events, TransportEvent::ConnectionState(ConnectionState::Failed)).await;
                            break;
                        }
                        if let Some(p) = pending {
                            shared.pending.insert(id, p);
                        }
                    }
                }
            }
            event = inbound.recv() => match event {
                Some(HostPortEvent::Frame(text)) => {
                    if text.trim().is_empty() {
                        continue;
                    }
                    if let Some(t) = handle_inbound_text(&text, &mut shared, &events, &mut state).await {
                        emit(&events, TransportEvent::ConnectionState(t)).await;
                    }
                }
                Some(HostPortEvent::Reset(reason)) => {
                    log::info!("host port: {reason}; opening the sessions again");
                    fail_all_pending(
                        &mut shared,
                        &events,
                        RpcError::new(KERNEL_RESTARTED, "the octos kernel restarted; try again"),
                    )
                    .await;
                    attempt += 1;
                    emit(&events, TransportEvent::ConnectionState(ConnectionState::Reconnecting { attempt })).await;
                    state = ConnectionState::Handshaking;
                    for (id, frame, pending) in build_reopens(&mut shared) {
                        if send(frame).is_err() {
                            break;
                        }
                        shared.pending.insert(id, pending);
                    }
                }
                Some(HostPortEvent::Closed(reason)) => {
                    log::warn!("host port closed: {reason}");
                    emit(&events, TransportEvent::ConnectionState(ConnectionState::Failed)).await;
                    break;
                }
                None => {
                    log::warn!("host port: its reader stopped");
                    emit(&events, TransportEvent::ConnectionState(ConnectionState::Failed)).await;
                    break;
                }
            },
        }
    }
    shared.registry.cancel_all();
    shared.pending.clear();
    while commands.try_recv().is_ok() {}
}
