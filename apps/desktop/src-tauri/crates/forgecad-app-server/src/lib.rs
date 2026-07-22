//! Bounded, transport-neutral ForgeCAD app-server core.
//!
//! K001 owns protocol lifecycle and delivery mechanics. K002 adds the native
//! Agent lifecycle, bounded Provider/tool orchestration, and redacted runtime
//! evidence. Persistence and product-core execution remain abstract ports
//! until K003; this crate never owns geometry execution or arbitrary code.

#![recursion_limit = "512"]

mod action_loop;
mod cancellation;
mod canonical;
pub mod compatibility;
mod context;
mod event_queue;
mod handler;
mod lifecycle;
mod native_runtime;
mod product_tools;
mod provider;
mod server;
mod trace;

pub use action_loop::*;
pub use cancellation::*;
pub use context::*;
pub use event_queue::*;
pub use handler::*;
pub use lifecycle::*;
pub use native_runtime::*;
pub use product_tools::*;
pub use provider::*;
pub use server::*;
pub use trace::*;
