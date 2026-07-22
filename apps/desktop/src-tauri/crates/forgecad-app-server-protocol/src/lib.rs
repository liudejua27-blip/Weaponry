//! Rust-owned wire contract for the ForgeCAD desktop app-server.
//!
//! This crate intentionally contains no transport, persistence, Provider, or
//! Tauri code.  Both the native bridge and the restricted development bridge
//! must serialize these exact types.

mod compat;
mod contract_validation;
mod cursor;
mod error;
mod initialize;
mod jsonrpc;
mod lifecycle;
mod native_lifecycle;
mod native_notification;
mod notification;
mod ownership;
mod persistence;
mod product_tool;
mod provider;

#[cfg(test)]
mod k002_fixture;

pub use compat::*;
pub use cursor::*;
pub use error::*;
pub use initialize::*;
pub use jsonrpc::*;
pub use lifecycle::*;
pub use native_lifecycle::*;
pub use native_notification::*;
pub use notification::*;
pub use ownership::*;
pub use persistence::*;
pub use product_tool::*;
pub use provider::*;

pub const METHOD_INITIALIZE: &str = "initialize";
pub const METHOD_INITIALIZED: &str = "initialized";
pub const METHOD_REQUEST_CANCEL: &str = "request/cancel";
pub const METHOD_NOTIFICATION_ACK: &str = "notification/ack";
pub const METHOD_EVENTS_REPLAY: &str = "thread/events/replay";
pub const METHOD_COMPAT_HTTP: &str = "compat/http";
pub const METHOD_COMPAT_SUBSCRIBE: &str = "compat/subscribe";
pub const METHOD_COMPAT_SSE: &str = "compat/sse";
pub const METHOD_COMPAT_UNSUBSCRIBE: &str = "compat/unsubscribe";
pub const METHOD_THREAD_CREATE: &str = "thread/create";
pub const METHOD_THREAD_LIST: &str = "thread/list";
pub const METHOD_THREAD_READ: &str = "thread/read";
pub const METHOD_THREAD_ARCHIVE: &str = "thread/archive";
pub const METHOD_TURN_START: &str = "turn/start";
pub const METHOD_TURN_READ: &str = "turn/read";
pub const METHOD_TURN_CANCEL: &str = "turn/cancel";
pub const METHOD_ITEM_LIST: &str = "item/list";
pub const METHOD_ITEM_READ: &str = "item/read";
pub const METHOD_APPROVAL_CREATE: &str = "approval/create";
pub const METHOD_APPROVAL_READ: &str = "approval/read";
pub const METHOD_APPROVAL_RESOLVE: &str = "approval/resolve";
pub const METHOD_PROVIDER_PREFLIGHT: &str = "provider/preflight";
pub const METHOD_PROVIDER_CHECK: &str = "provider/check";
pub const METHOD_PROVIDER_CANCEL: &str = "provider/cancel";
pub const METHOD_PRODUCT_TOOLS_LIST: &str = "product-tools/list";
pub const METHOD_PRODUCT_TOOLS_EXECUTE: &str = "product-tools/execute";
pub const METHOD_LIFECYCLE_PERSISTENCE_EXECUTE: &str = "lifecycle-persistence/execute";
pub const METHOD_MIGRATION_OWNERSHIP_READ: &str = "migration/ownership/read";

pub const NOTIFICATION_THREAD_CREATED: &str = "thread/created";
pub const NOTIFICATION_THREAD_UPDATED: &str = "thread/updated";
pub const NOTIFICATION_THREAD_ARCHIVED: &str = "thread/archived";
pub const NOTIFICATION_TURN_STARTED: &str = "turn/started";
pub const NOTIFICATION_ITEM_UPDATED: &str = "item/updated";
pub const NOTIFICATION_APPROVAL_CREATED: &str = "approval/created";
pub const NOTIFICATION_APPROVAL_RESOLVED: &str = "approval/resolved";
pub const NOTIFICATION_TURN_COMPLETED: &str = "turn/completed";
pub const NOTIFICATION_TURN_FAILED: &str = "turn/failed";
pub const NOTIFICATION_TURN_CANCELLED: &str = "turn/cancelled";
