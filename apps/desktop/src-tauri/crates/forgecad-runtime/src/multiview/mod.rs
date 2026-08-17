//! Runtime-owned joint multi-view authoring primitives.
//!
//! The modules here contain only typed context, camera-rig validation and
//! objective helpers.  Geometry compilation, CAS, Jobs and promotion remain
//! in the Runtime/optimization boundary.

pub(crate) mod camera_rig;
pub(crate) mod evaluation;
pub(crate) mod objective;
pub(crate) mod reference_context;
