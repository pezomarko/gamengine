//! The smithy (ITEMS.md 6.6): the smith's shop and the forge. The zone vouches for the
//! place alone, as it does for a stall (ITEMS.md 5): the body is alive, in person, and
//! stands by the smith (within `TRAINER_REACH` of a body of the `smith` role) or at a
//! forge (within `FORGE_REACH` of a `gm_forge` post); the hub checks the price, the fee,
//! the coin, the room and the parts. A request is always answered: somebody is looking
//! at a screen.

use super::*;

impl Run {
    /// A body nothing hurts, of `role`, stands within `reach` of `at`: the trainer
    /// (MATRIX.md 9.1) or the smith (ITEMS.md 6.6), as the content names them.
    pub(super) fn npc_near(&self, at: Vec3, role: &str, reach: f32) -> bool {
        self.zone.players().any(|t| {
            t.alive
                && t.unhurt
                && (t.mover.mv.origin - at).length() <= reach
                && matches!(self.director.kind_of(t.id), gm_ai::BodyKind::Creature { def }
                    if self.pack.creatures.get(def as usize).is_some_and(|d| d.role == role))
        })
    }

    /// A forge of the map (ITEMS.md 6.6) is in the body's reach, by the stall's rule.
    pub(super) fn forge_near(&self, feet: Vec3) -> bool {
        self.world
            .forges
            .iter()
            .any(|f| stall_in_reach(f.origin.into(), feet.into()))
    }

    /// `ClientEvent::SmithBuy`.
    pub(super) fn on_smith_buy(&mut self, id: EntityId, material: String, price: i64) {
        let request = (|| {
            let link = self.cfg.hub.clone().ok_or("this zone has no smith")?;
            let slot = self.hub_slots.get(&id).ok_or("this zone has no smith")?;
            let p = self
                .zone
                .player(id)
                .filter(|p| p.alive && !p.ghost)
                .ok_or("you cannot buy now")?;
            if !self.npc_near(p.mover.mv.origin, "smith", TRAINER_REACH) {
                return Err("walk up to the smith to buy");
            }
            Ok((link, slot.character))
        })();
        let Some(session) = self.sessions.get_mut(&id) else {
            return;
        };
        let refuse = |session: &Session, material: &str, why: &str| {
            session.send_control(FromZone::SmithResult {
                material: material.to_string(),
                result: Err(why.to_string()),
            });
        };
        if !session.begin_stall_request() {
            refuse(
                session,
                &material,
                "one thing at a time: try again in a moment",
            );
            return;
        }
        match request {
            Ok((link, character)) => {
                let tx = self.event_tx.clone();
                tokio::spawn(async move {
                    let asked = link.smith_buy(character, &material, price);
                    let result = tokio::time::timeout(HUB_PATIENCE, asked)
                        .await
                        .unwrap_or_else(|_| Err(HUB_SILENT.to_string()));
                    let _ = tx
                        .send(ClientEvent::SmithBought {
                            id,
                            material,
                            result: result.map(|_| ()),
                        })
                        .await;
                });
            }
            Err(why) => {
                session.cancel_stall_request();
                refuse(session, &material, why);
            }
        }
    }

    /// `ClientEvent::SmithBought`.
    pub(super) fn on_smith_bought(
        &mut self,
        id: EntityId,
        material: String,
        result: Result<(), String>,
    ) {
        if let Some(s) = self.sessions.get_mut(&id)
            && s.end_stall_request()
        {
            s.send_control(FromZone::SmithResult { material, result });
        }
    }

    /// `ClientEvent::Forge`.
    pub(super) fn on_forge(
        &mut self,
        id: EntityId,
        template: String,
        components: Vec<i64>,
        fee: i64,
    ) {
        let request = (|| {
            let link = self.cfg.hub.clone().ok_or("this zone has no forge")?;
            let slot = self.hub_slots.get(&id).ok_or("this zone has no forge")?;
            let p = self
                .zone
                .player(id)
                .filter(|p| p.alive && !p.ghost)
                .ok_or("you cannot craft now")?;
            let feet = p.mover.mv.origin + Vec3::Z * Hull::Player.mins().z;
            if !self.forge_near(feet) {
                return Err("stand at the forge to craft");
            }
            Ok((link, slot.character))
        })();
        let Some(session) = self.sessions.get_mut(&id) else {
            return;
        };
        let refuse = |session: &Session, why: &str| {
            session.send_control(FromZone::ForgeResult(Err(why.to_string())));
        };
        if !session.begin_stall_request() {
            refuse(session, "one thing at a time: try again in a moment");
            return;
        }
        match request {
            Ok((link, character)) => {
                let tx = self.event_tx.clone();
                tokio::spawn(async move {
                    let asked = link.forge(character, &template, &components, fee);
                    let result = tokio::time::timeout(HUB_PATIENCE, asked)
                        .await
                        .unwrap_or_else(|_| Err(HUB_SILENT.to_string()));
                    let _ = tx.send(ClientEvent::Forged { id, result }).await;
                });
            }
            Err(why) => {
                session.cancel_stall_request();
                refuse(session, why);
            }
        }
    }

    /// `ClientEvent::Forged`.
    pub(super) fn on_forged(&mut self, id: EntityId, result: Result<i64, String>) {
        if let Some(s) = self.sessions.get_mut(&id)
            && s.end_stall_request()
        {
            s.send_control(FromZone::ForgeResult(result));
        }
    }
}
