//! Inbound notification routing: one handler per `UiNotification` variant,
//! plus the card #22 no-silent-drops guard.
//!
//! The transport decodes wire frames into `TransportEvent`s (and already logs
//! + ignores an unknown *wire* method). This registry is the second level.
//!
//! **Card #22 §2 (no silent drops).** A decoded notification must reach a
//! handler **or** an explicit [`Registry::ignore`] entry with a reason. Neither
//! is a silent drop: the first acts, the second is a *decision* recorded in the
//! store and in `docs/cards/22-ignored.csv`. Anything else (a method nobody has
//! claimed and nobody has deliberately ignored) is a bug we can see — it bumps a
//! per-kind `unhandled` counter in the store and logs **once per kind** at
//! `warn`. The registry never panics on it (RULES #6 / 8.8 condition 7: the peer
//! upgrades under us).
//!
//! Handlers are boxed as closures keyed by `H::METHOD`, so the trait keeps its
//! `const METHOD` (the fan-out recipe's single line) while the table stays a
//! plain `HashMap`.
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use octos_core::app_ui::AppUiBackendEvent as UiNotification;
use octoscode_store::Store;

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
    /// Card #22 §2: methods we deliberately do **not** act on, with the reason.
    /// An entry here is an explicit, reviewable decision — the opposite of a
    /// silent drop — and the source of `docs/cards/22-ignored.csv`.
    ignored: Vec<(&'static str, &'static str)>,
    /// Card #22 §2: the store whose per-kind `unhandled` counter records a
    /// method that reached neither a handler nor an ignore entry. `None` in a
    /// bare [`Registry::new`] (unit tests); [`crate::domains::register_all`]
    /// attaches it.
    store: Option<Arc<Store>>,
    /// Card #22 §2: methods already logged at `warn`, so the log fires once per
    /// kind, not once per frame.
    warned: HashSet<&'static str>,
    /// Methods seen that no handler claimed — surfaced for tests and for a
    /// "we are behind the peer" diagnostic. Never fatal.
    unknown: Vec<String>,
}

impl Registry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Attach the store the guard records into (card #22 §2). Called by
    /// [`crate::domains::register_all`].
    pub fn set_store(&mut self, store: Arc<Store>) {
        self.store = Some(store);
    }

    /// Register a handler for `H::METHOD`. Each notification method has exactly
    /// ONE owning domain: a second registration is a wiring bug (two domains
    /// would race to own the same state), so it panics instead of silently replacing.
    pub fn register<H: NotificationHandler + 'static>(&mut self, handler: H) {
        let method = H::METHOD;
        if self
            .handlers
            .insert(method, Box::new(move |n: &UiNotification| handler.handle(n)))
            .is_some()
        {
            panic!("duplicate notification handler for '{method}': exactly one domain may own it");
        }
    }

    /// Card #22 §2: declare that `method` is deliberately ignored, with `reason`.
    /// Ignoring is a decision, not a drop: the method is then "resolved" (the
    /// guard does not count it) and listed for review.
    pub fn ignore(&mut self, method: &'static str, reason: &'static str) {
        assert!(
            !self.handlers.contains_key(method),
            "method '{method}' has a handler; an ignore entry would be dead"
        );
        self.ignored.push((method, reason));
    }

    /// Card #22 §2: whether `method` has an explicit ignore entry.
    pub fn is_ignored(&self, method: &str) -> bool {
        self.ignored.iter().any(|(m, _)| *m == method)
    }

    /// Card #22 §2: the reason `method` is ignored, if it is.
    pub fn ignore_reason(&self, method: &str) -> Option<&'static str> {
        self.ignored
            .iter()
            .find(|(m, _)| *m == method)
            .map(|(_, r)| *r)
    }

    /// Card #22 §2: every explicitly ignored method with its reason, sorted by
    /// method — the machine-readable `docs/cards/22-ignored.csv` source.
    pub fn ignored_methods(&self) -> Vec<(&'static str, &'static str)> {
        let mut v = self.ignored.clone();
        v.sort_unstable_by_key(|(m, _)| *m);
        v
    }

    /// Route one decoded notification. Returns whether it was **resolved** — by
    /// a handler, or by an explicit ignore entry. A method with neither is the
    /// silent-drop case card #22 §2 catches: it bumps the store's per-kind
    /// `unhandled` counter, logs once per kind at `warn`, and returns `false`.
    pub fn dispatch(&mut self, notification: &UiNotification) -> bool {
        let method = notification.method();
        if let Some(handler) = self.handlers.get(method) {
            handler(notification);
            return true;
        }
        if self.is_ignored(method) {
            // An explicit decision (card #22 §2) — resolved, never counted.
            log::debug!("octoscode: notification '{method}' ignored by policy (declared)");
            return true;
        }
        // Card #22 §2: neither handled nor explicitly ignored — a silent drop
        // unless we make it loud. Count it, and log ONCE per kind.
        if let Some(store) = &self.store {
            store.note_unhandled(method);
        }
        if self.warned.insert(method) {
            log::warn!(
                "octoscode: unhandled notification '{method}' (no handler, no ignore entry) — \
                 add a handler or an explicit ignore with a reason"
            );
        }
        self.unknown.push(method.to_owned());
        false
    }

    /// Whether a handler is registered for `method`.
    pub fn handles(&self, method: &str) -> bool {
        self.handlers.contains_key(method)
    }

    /// The methods seen so far that no handler claimed, in arrival order.
    pub fn unknown_methods(&self) -> &[String] {
        &self.unknown
    }

    /// Card #22 §2: the kinds logged at `warn` so far — at most one entry per
    /// kind (the log fires once per kind, not once per frame). Test-visible.
    pub fn warned_kinds(&self) -> Vec<&'static str> {
        let mut v: Vec<_> = self.warned.iter().copied().collect();
        v.sort_unstable();
        v
    }

    /// The methods that currently have a handler.
    pub fn handled_methods(&self) -> Vec<&'static str> {
        let mut v: Vec<_> = self.handlers.keys().copied().collect();
        v.sort_unstable();
        v
    }
}
