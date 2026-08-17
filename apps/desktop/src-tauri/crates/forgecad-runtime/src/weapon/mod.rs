//! Weapon-specific authoring contracts.
//!
//! This module is intentionally small and declarative.  It does not own
//! Runtime state and it never creates a candidate or writes CAS.  The Runtime
//! remains the only writer; these helpers only validate the bounded coordinate
//! frame used by weapon reference and optimization requests.

pub(crate) mod coordinate_frame;
pub(crate) mod profile;
