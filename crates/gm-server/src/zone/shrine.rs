//! The shrine (SKILLS.md 5): `E` beside a `gm_shrine` post, by the stall's rule of reach,
//! washes every harmful status off a living body. Always answered with a line.

use super::*;

impl Run {
    /// `ClientEvent::Shrine`.
    pub(super) fn on_shrine(&mut self, id: EntityId) {
        let shrines = &self.world.shrines;
        let text = match self.zone.player_mut(id).filter(|p| p.alive && !p.ghost) {
            None => "not now",
            Some(p) => {
                let feet = p.mover.mv.origin + Vec3::Z * Hull::Player.mins().z;
                if !shrines
                    .iter()
                    .any(|&s| stall_in_reach(s.into(), feet.into()))
                {
                    "walk up to the shrine first"
                } else if p.mover.statuses.cleanse() {
                    "the shrine washes it away"
                } else {
                    "nothing ails you"
                }
            }
        };
        if let Some(session) = self.sessions.get(&id) {
            session.send_control(FromZone::ChatFrom {
                from: 0,
                text: text.to_string(),
            });
        }
    }
}
