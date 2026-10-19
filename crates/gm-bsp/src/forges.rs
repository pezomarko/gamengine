//! Forge posts (ITEMS.md 6.6): where a map stands a forge, the hearth and the anvil a craft
//! is made at. A `gm_forge` entity marks the mouth of the hearth; the zone decides a
//! craft's reach from it, and the client draws the fire there. The zone, the tools and the
//! client read the same entity.

use glam::Vec3;

use crate::Bsp;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ForgePost {
    /// The mouth of the hearth, on the ground.
    pub origin: Vec3,
    /// The way the hearth opens.
    pub yaw: f32,
}

/// The most forges one map may post.
pub const MAX_FORGE_POSTS: usize = 32;

impl Bsp {
    /// The `gm_forge`s of the map, in entity order; one without an origin is left out.
    pub fn forge_posts(&self) -> Vec<ForgePost> {
        self.entities
            .iter()
            .filter(|e| e.classname() == "gm_forge")
            .filter_map(|e| {
                Some(ForgePost {
                    origin: e.origin()?,
                    yaw: e.f32("angle").unwrap_or(0.0),
                })
            })
            .take(MAX_FORGE_POSTS)
            .collect()
    }
}
