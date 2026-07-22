use serde::Serialize;

use crate::{semantic_sha256, CoreResult};

#[allow(dead_code)]
pub(crate) fn stable_id(prefix: &str, value: &impl Serialize) -> CoreResult<String> {
    Ok(format!("{prefix}_{}", semantic_sha256(value)?))
}
