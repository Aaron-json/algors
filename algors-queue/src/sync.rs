/// This module is used to switch between implementations of `sync` primitives.
/// It is internal to the crate and not part of the public API.

#[cfg(loom)]
pub(crate) use loom::sync::{Arc, atomic};

#[cfg(not(loom))]
pub(crate) use {alloc::sync::Arc, core::sync::atomic};
