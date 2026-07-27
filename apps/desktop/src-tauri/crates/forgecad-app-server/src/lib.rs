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
mod concept_image_provider;
mod context;
mod event_queue;
mod handler;
mod lifecycle;
mod multimodal_action_context;
mod native_runtime;
mod neural_visual_provider;
mod product_tools;
mod provider;
mod server;
mod trace;
mod vision_evidence_provider;
mod visual_brief_director;
mod visual_reference_comparison_provider;

pub use action_loop::*;
pub use cancellation::*;
pub use concept_image_provider::*;
pub use context::*;
pub use event_queue::*;
pub use handler::*;
pub use lifecycle::*;
pub use multimodal_action_context::*;
pub use native_runtime::*;
pub use neural_visual_provider::*;
pub use product_tools::*;
pub use provider::*;
pub use server::*;
pub use trace::*;
pub use vision_evidence_provider::*;
pub use visual_brief_director::*;
pub use visual_reference_comparison_provider::*;
