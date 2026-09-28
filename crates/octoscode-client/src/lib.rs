//! octoscode-client — the typed UI-Protocol client for octoscode-app.
//!
//! One [`Method`] per RPC, in a file per domain under [`domains`], plus one
//! [`Registry`] that routes inbound notifications to their domain handler.
//! This is the shape every later fan-out lane follows: **one file per
//! domain**, and the only shared lines are the domain's `register()` call in
//! [`domains::register_all`] and its module declaration.
//!
//! ## How to add a method (the fan-out recipe)
//! 1. Add a unit struct in your domain file implementing [`Method`]:
//!    `NAME`, `Params`, `Result`.
//! 2. Call it through [`Client::call`]. Nothing else changes.
//!
//! ## How to add a notification (the fan-out recipe)
//! 1. Add a struct implementing [`NotificationHandler`] in your domain file.
//! 2. Export it from that file's `register(reg: &mut Registry)`.
//! The registry routes by the handler's `METHOD`; an unregistered method
//! falls through to the built-in tolerated-unknown arm (logged, never fatal).
mod method;
pub mod domains;
pub mod registry;

pub use method::Method;
pub use registry::{NotificationHandler, Registry};

use octos_core::ui_protocol::RpcError;
use octos_app_transport::OutboundCommand;
use serde::Serialize;
use serde_json::Value;
use tokio::sync::{mpsc, oneshot};

/// Why a [`Client::call`] did not produce its typed result.
#[derive(Debug)]
pub enum ClientError {
    /// The server answered with a JSON-RPC error.
    Rpc { method: String, error: RpcError },
    /// The server's `result` did not match the method's `Result` type.
    Decode { method: String, reason: String },
    /// The transport channel is closed (connection gone).
    Transport { method: String, reason: String },
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rpc { method, error } => {
                write!(f, "{method}: rpc error {} ({})", error.code, error.message)
            }
            Self::Decode { method, reason } => write!(f, "{method}: bad result: {reason}"),
            Self::Transport { method, reason } => write!(f, "{method}: transport: {reason}"),
        }
    }
}

impl std::error::Error for ClientError {}

/// A typed client over the transport's outbound command channel.
///
/// Every call rides the transport's ONE generic `OutboundCommand::Request`
/// (D10a): the same serialize + registry path the typed lifecycle commands
/// use, so there is a single wire implementation.
#[derive(Clone)]
pub struct Client {
    commands: mpsc::Sender<OutboundCommand>,
}

impl Client {
    pub fn new(commands: mpsc::Sender<OutboundCommand>) -> Self {
        Self { commands }
    }

    /// Issue `M` and decode its typed result.
    pub async fn call<M: Method>(&self, params: M::Params) -> Result<M::Result, ClientError> {
        let method = M::NAME;
        let params = serde_json::to_value(&params).map_err(|e| ClientError::Decode {
            method: method.to_owned(),
            reason: format!("params do not serialize: {e}"),
        })?;
        let value = self.request(method, params).await?;
        serde_json::from_value::<M::Result>(value).map_err(|e| ClientError::Decode {
            method: method.to_owned(),
            reason: e.to_string(),
        })
    }

    /// The untyped half: any method, any params, the raw `result` value.
    /// Mirrors the web client's one generic `request`
    /// (`packages/client/src/client.ts:426`).
    pub async fn request(&self, method: &str, params: Value) -> Result<Value, ClientError> {
        let (reply, rx) = oneshot::channel();
        self.commands
            .send(OutboundCommand::Request {
                method: method.to_owned(),
                params,
                reply,
            })
            .await
            .map_err(|e| ClientError::Transport {
                method: method.to_owned(),
                reason: e.to_string(),
            })?;
        match rx.await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(error)) => Err(ClientError::Rpc {
                method: method.to_owned(),
                error,
            }),
            Err(_) => Err(ClientError::Transport {
                method: method.to_owned(),
                reason: "reply channel dropped".to_owned(),
            }),
        }
    }
}

/// Serialize params for tests and callers that build a request by hand.
pub fn to_params<T: Serialize>(params: &T) -> Value {
    serde_json::to_value(params).unwrap_or(Value::Null)
}
