//! One UI-Protocol method: its name, its params, its result.
//!
//! Mirrors the web client's typed wrappers over one generic `request`
//! (`packages/client/src/client.ts:426`): the wire call is always the same,
//! only the method name and the two types differ. A domain file implements
//! this once per method; [`crate::Client::call`] does the rest.
use serde::{de::DeserializeOwned, Serialize};

pub trait Method {
    /// The exact wire method name, e.g. `"session/list"`.
    const NAME: &'static str;
    type Params: Serialize;
    type Result: DeserializeOwned;
}
