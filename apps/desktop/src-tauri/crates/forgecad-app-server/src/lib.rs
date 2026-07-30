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
// Historical Fal/OpenAI concept-image envelopes remain source-compatible only
// for migration fixtures. They are intentionally excluded from every product
// binary: Forge Studio's runtime AI boundary is DeepSeek authoring plus Qwen
// evidence/comparison, never a third concept-image provider.
#[cfg(test)]
mod concept_image_provider;
mod context;
mod e005_offline_harness;
mod e005_production_review;
mod e005_provider_runner;
mod e005_visual_review;
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
mod universal_author_context;
mod vision_evidence_provider;
#[cfg(test)]
mod visual_brief_director;
mod visual_program_runtime_v2;
mod visual_reference_comparison_provider;

pub use action_loop::*;
pub use cancellation::*;
#[cfg(test)]
pub use concept_image_provider::*;
pub use context::*;
pub use e005_offline_harness::*;
pub use e005_production_review::*;
pub use e005_provider_runner::*;
pub use e005_visual_review::*;
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
pub use universal_author_context::*;
pub use vision_evidence_provider::*;
#[cfg(test)]
pub use visual_brief_director::*;
pub use visual_program_runtime_v2::*;
pub use visual_reference_comparison_provider::*;
