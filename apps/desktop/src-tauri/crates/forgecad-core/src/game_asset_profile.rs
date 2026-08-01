use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::{CoreError, CoreResult};

pub const GAME_ASSET_PROFILE_SCHEMA_VERSION: &str = "GameAssetProfile@1";
pub const GAME_ASSET_DELIVERY_REQUEST_SCHEMA_VERSION: &str = "GameAssetDeliveryRequest@1";

/// User intent for a game-ready delivery.  It deliberately excludes part IDs
/// and sockets: those are derived by Rust from the exact executable source so
/// a Provider or WebView cannot fabricate bindings for a future asset.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GameAssetDeliveryRequest {
    pub schema_version: String,
    pub profile_id: String,
    pub lod_triangle_budgets: [u32; 3],
    pub target_texel_density_pixels_per_meter: u16,
}

impl GameAssetDeliveryRequest {
    pub fn validate(&self) -> CoreResult<()> {
        let profile_id_valid = !self.profile_id.is_empty()
            && self.profile_id.len() <= 120
            && self.profile_id.is_ascii()
            && self
                .profile_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'));
        if self.schema_version != GAME_ASSET_DELIVERY_REQUEST_SCHEMA_VERSION
            || !profile_id_valid
            || self.lod_triangle_budgets[2] == 0
            || self.lod_triangle_budgets[0] > 150_000
            || self.lod_triangle_budgets[0] < self.lod_triangle_budgets[1]
            || self.lod_triangle_budgets[1] < self.lod_triangle_budgets[2]
            || !(128..=2048).contains(&self.target_texel_density_pixels_per_meter)
        {
            return Err(CoreError::invalid_data(
                "GAME_ASSET_DELIVERY_REQUEST_INVALID",
                "Game asset delivery intent requires bounded LODs and texel density.",
            ));
        }
        Ok(())
    }

    /// Convert user intent into a complete profile only after executable
    /// source part IDs have been derived by Rust. Sockets intentionally start
    /// empty and can be added later through an exact asset edit.
    pub fn derive_profile(&self, collision_proxy_part_ids: Vec<String>) -> CoreResult<GameAssetProfile> {
        self.validate()?;
        let profile = GameAssetProfile {
            schema_version: GAME_ASSET_PROFILE_SCHEMA_VERSION.into(),
            profile_id: self.profile_id.clone(),
            lod_triangle_budgets: self.lod_triangle_budgets,
            collision_proxy_part_ids,
            sockets: Vec::new(),
            target_texel_density_pixels_per_meter: self.target_texel_density_pixels_per_meter,
        };
        profile.validate()?;
        Ok(profile)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameAssetProfile {
    pub schema_version: String,
    pub profile_id: String,
    pub lod_triangle_budgets: [u32; 3],
    pub collision_proxy_part_ids: Vec<String>,
    pub sockets: Vec<GameAssetSocket>,
    /// A delivery target, not a measured fact. The compiler may only promote
    /// this to an exportable asset after GLB readback has derived and checked
    /// the effective density from its actual UVs and surface area.
    pub target_texel_density_pixels_per_meter: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GameAssetSocket {
    pub socket_id: String,
    pub part_id: String,
    pub pivot_meters: [f32; 3],
    pub forward: [f32; 3],
}

impl GameAssetProfile {
    pub fn validate(&self) -> CoreResult<()> {
        let collision_part_ids = self
            .collision_proxy_part_ids
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let socket_ids = self
            .sockets
            .iter()
            .map(|socket| socket.socket_id.as_str())
            .collect::<BTreeSet<_>>();
        if self.schema_version != GAME_ASSET_PROFILE_SCHEMA_VERSION
            || self.profile_id.is_empty()
            || self.lod_triangle_budgets[2] == 0
            || self.lod_triangle_budgets[0] > 150_000
            || self.lod_triangle_budgets[0] < self.lod_triangle_budgets[1]
            || self.lod_triangle_budgets[1] < self.lod_triangle_budgets[2]
            || self.collision_proxy_part_ids.is_empty()
            || collision_part_ids.len() != self.collision_proxy_part_ids.len()
            || socket_ids.len() != self.sockets.len()
            || self.sockets.iter().any(|socket| !socket.valid())
            || !(128..=2048).contains(&self.target_texel_density_pixels_per_meter)
        {
            return Err(CoreError::invalid_data(
                "GAME_ASSET_PROFILE_INVALID",
                "Game asset profile requires bounded LODs, collision proxies and texel density.",
            ));
        }
        Ok(())
    }
}

impl GameAssetSocket {
    fn valid(&self) -> bool {
        !self.socket_id.is_empty()
            && !self.part_id.is_empty()
            && self
                .pivot_meters
                .iter()
                .all(|value| value.is_finite() && value.abs() <= 100.0)
            && self
                .forward
                .iter()
                .all(|value| value.is_finite() && value.abs() <= 1.0)
            && self.forward.iter().map(|value| value * value).sum::<f32>() > 0.25
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivery_request_derives_only_code_owned_collision_bindings() {
        let request = GameAssetDeliveryRequest {
            schema_version: GAME_ASSET_DELIVERY_REQUEST_SCHEMA_VERSION.into(),
            profile_id: "game_prop_standard".into(),
            lod_triangle_budgets: [150_000, 60_000, 12_000],
            target_texel_density_pixels_per_meter: 1024,
        };
        let profile = request
            .derive_profile(vec!["part_body".into()])
            .expect("bounded request should derive a profile");
        assert_eq!(profile.collision_proxy_part_ids, vec!["part_body"]);
        assert!(profile.sockets.is_empty());
    }

    #[test]
    fn delivery_request_rejects_unbounded_profile_identity() {
        let request = GameAssetDeliveryRequest {
            schema_version: GAME_ASSET_DELIVERY_REQUEST_SCHEMA_VERSION.into(),
            profile_id: "game prop standard".into(),
            lod_triangle_budgets: [150_000, 60_000, 12_000],
            target_texel_density_pixels_per_meter: 1024,
        };
        assert_eq!(
            request.validate().unwrap_err().code(),
            "GAME_ASSET_DELIVERY_REQUEST_INVALID"
        );
    }

    #[test]
    fn game_asset_profile_rejects_non_directional_socket() {
        let profile = GameAssetProfile {
            schema_version: GAME_ASSET_PROFILE_SCHEMA_VERSION.into(),
            profile_id: "game_hard_surface".into(),
            lod_triangle_budgets: [90_000, 36_000, 8_000],
            collision_proxy_part_ids: vec!["part_body".into()],
            sockets: vec![GameAssetSocket {
                socket_id: "socket_vfx".into(),
                part_id: "part_body".into(),
                pivot_meters: [0.0; 3],
                forward: [0.0; 3],
            }],
            target_texel_density_pixels_per_meter: 1024,
        };
        assert!(profile.validate().is_err());
    }

    #[test]
    fn game_asset_profile_rejects_duplicate_delivery_ids() {
        let profile = GameAssetProfile {
            schema_version: GAME_ASSET_PROFILE_SCHEMA_VERSION.into(),
            profile_id: "game_hard_surface".into(),
            lod_triangle_budgets: [90_000, 36_000, 8_000],
            collision_proxy_part_ids: vec!["part_body".into(), "part_body".into()],
            sockets: vec![
                GameAssetSocket {
                    socket_id: "socket_vfx".into(),
                    part_id: "part_body".into(),
                    pivot_meters: [0.0; 3],
                    forward: [0.0, 0.0, 1.0],
                },
                GameAssetSocket {
                    socket_id: "socket_vfx".into(),
                    part_id: "part_body".into(),
                    pivot_meters: [0.0; 3],
                    forward: [0.0, 1.0, 0.0],
                },
            ],
            target_texel_density_pixels_per_meter: 1024,
        };
        assert_eq!(
            profile.validate().unwrap_err().code(),
            "GAME_ASSET_PROFILE_INVALID"
        );
    }
}
