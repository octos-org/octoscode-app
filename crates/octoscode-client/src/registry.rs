//! Inbound notification routing: one handler per `UiNotification` variant.
//!
//! The transport decodes wire frames into `TransportEvent`s (and already logs
//! + ignores an unknown *wire* method). This registry is the second level:
//! a **decoded** notification whose variant no domain has claimed yet goes to
//! ONE explicit arm that logs `debug!` with the method name and moves on.
//! Never fatal — the peer upgrades under us (RULES #6, 8.8 condition 7).
//!
//! Handlers are boxed as closures keyed by `H::METHOD`, so the trait keeps its
//! `const METHOD` (the fan-out recipe's single line) while the table stays a
//! plain `HashMap`.
use std::collections::HashMap;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;

/// One domain's handler for one notification method.
pub trait NotificationHandler: Send + Sync {
    /// The exact wire method, e.g. `"message/delta"` (`UiNotification::method`).
    const METHOD: &'static str;
    fn handle(&self, notification: &UiNotification);
}

/// Routes decoded notifications by method to the domain that owns them.
#[derive(Default)]
pub struct Registry {
    handlers: HashMap<&'static str, Box<dyn Fn(&UiNotification) + Send + Sync>>,
    /// Methods seen that no handler claimed — surfaced for tests and for a
    /// "we are behind the peer" diagnostic. Never fatal.
    unknown: Vec<String>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler for `H::METHOD`. Re-registering replaces.
    pub fn register<H: NotificationHandler + 'static>(&mut self, handler: H) {
        let method = H::METHOD;
        self.handlers
            .insert(method, Box::new(move |n: &UiNotification| handler.handle(n)));
    }

    /// Route one decoded notification. Returns whether a handler claimed it.
    pub fn dispatch(&mut self, notification: &UiNotification) -> bool {
        let method = notification.method();
        match self.handlers.get(method) {
            Some(handler) => {
                handler(notification);
                true
            }
            None => {
                // The one tolerated-unknown arm: named, logged, ignored.
                log::debug!(
                    "octoscode: unhandled notification method '{method}' (decoded, ignored)"
                );
                self.unknown.push(method.to_owned());
                false
            }
        }
    }

    /// Whether a handler is registered for `method`.
    pub fn handles(&self, method: &str) -> bool {
        self.handlers.contains_key(method)
    }

    /// The methods seen so far that no handler claimed, in arrival order.
    pub fn unknown_methods(&self) -> &[String] {
        &self.unknown
    }

    /// The methods that currently have a handler.
    pub fn handled_methods(&self) -> Vec<&'static str> {
        let mut v: Vec<_> = self.handlers.keys().copied().collect();
        v.sort_unstable();
        v
    }
}
