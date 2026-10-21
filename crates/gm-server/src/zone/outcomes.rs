//! What the simulation reported this tick (`ZoneEvent`): counters, what clients are told,
//! the hub's books.

use super::*;

impl Run {
    /// Every event of the tick's simulation step.
    pub(super) fn outcomes(&mut self, events: Vec<ZoneEvent>) {
        for ev in events {
            match ev {
                ZoneEvent::Hit {
                    kind,
                    attacker,
                    target,
                    amount,
                    absorbed,
                    at,
                } => self.happened_hit(kind, attacker, target, amount, absorbed, at),
                ZoneEvent::Impact { at, normal } => self.happened_impact(at, normal),
                ZoneEvent::Killed { victim, killer } => self.happened_killed(victim, killer),
                ZoneEvent::Respawned(id) => self.happened_respawned(id),
                ZoneEvent::Parried { .. } => self.report.parries += 1,
                ZoneEvent::GuardBroken(_) => self.report.guard_breaks += 1,
                ZoneEvent::Staggered(_) => self.report.staggers += 1,
                ZoneEvent::StatusApplied { .. } => self.report.statuses_applied += 1,
                ZoneEvent::RoundsLoaded { id, hand, rounds } => {
                    self.happened_rounds_loaded(id, hand, rounds)
                }
                ZoneEvent::ItemUsed { id, cell } => self.happened_item_used(id, cell),
                ZoneEvent::Healed {
                    target,
                    source,
                    amount,
                } => self.happened_healed(target, source, amount),
                ZoneEvent::ProjectileSpawned { .. }
                | ZoneEvent::ProjectileRemoved(_)
                | ZoneEvent::AreaSpawned { .. }
                | ZoneEvent::AreaRemoved(_) => {}
            }
        }
    }

    /// `ZoneEvent::Hit`.
    pub(super) fn happened_hit(
        &mut self,
        kind: HitKind,
        attacker: EntityId,
        target: EntityId,
        amount: i32,
        absorbed: i32,
        at: Vec3,
    ) {
        match kind {
            HitKind::Melee => self.report.hits_melee += 1,
            HitKind::Projectile => self.report.hits_projectile += 1,
            HitKind::Area => self.report.hits_area += 1,
            HitKind::Dot => self.report.hits_dot += 1,
        }
        let amount = amount.max(0) as u64;
        if attacker != target
            && let Some(s) = self.sessions.get_mut(&attacker)
        {
            s.damage_dealt += amount;
            s.hits_landed += 1;
        }
        if let Some(s) = self.sessions.get_mut(&target) {
            s.damage_taken += amount;
        }
        // Everyone is told what landed and whose hand it was: the attacker's number
        // large over the body it hit, the others' small, the target's not at all
        // (LOOK.md 13.8, 13.13).
        for s in self.sessions.values() {
            s.send_control(FromZone::Hit {
                attacker,
                target,
                amount: amount.min(u16::MAX as u64) as u16,
                absorbed: absorbed.clamp(0, u16::MAX as i32) as u16,
                at: at.to_array(),
            });
        }
    }

    /// A bullet's mark (MODES.md 10.2): everyone in the zone may see the wall.
    /// `ZoneEvent::Impact`.
    pub(super) fn happened_impact(&mut self, at: Vec3, normal: Vec3) {
        for s in self.sessions.values() {
            s.send_control(FromZone::Impact {
                at: at.to_array(),
                normal: normal.to_array(),
            });
        }
    }

    /// `ZoneEvent::Killed`.
    pub(super) fn happened_killed(&mut self, victim: EntityId, killer: EntityId) {
        self.report.kills += 1;
        let team = self.zone.player(killer).map_or(0, |p| p.team()) as usize;
        self.report.team_kills[team.min(2)] += 1;
        for s in self.sessions.values() {
            s.send_control(FromZone::Killed { victim, killer });
        }
    }

    /// `ZoneEvent::Respawned`.
    pub(super) fn happened_respawned(&mut self, id: EntityId) {
        // The client's prediction switches to whatever build the respawn applied.
        let mut wears = None;
        if let (Some(s), Some(p)) = (self.sessions.get_mut(&id), self.zone.player(id)) {
            s.send_control(FromZone::BuildApplied(p.sheet.build.clone()));
            // A respec to another frame shows the mannequin; back, the model.
            let now = s.wears(p.frame());
            if now != s.announced {
                s.announced = now;
                wears = Some((s.name.clone(), p.team(), now));
            }
        }
        // A respec may hold another weapon (the primary's prop, LOOK.md 6.1):
        // everyone is told either way, in one message or the other.
        let look = look_of(
            &self.cfg.looks,
            &self.zone,
            &self.hub_slots,
            &self.sheaths,
            id,
            None,
        );
        if let Some((name, team, model)) = wears {
            for s in self.sessions.values() {
                s.send_control(FromZone::PlayerInfo {
                    id,
                    name: name.clone(),
                    team,
                    model,
                    kind: BodyKind::Human,
                    look,
                });
            }
        } else {
            for s in self.sessions.values() {
                s.send_control(FromZone::Look { id, look });
            }
        }
        // The build the respawn applied may choose otherwise among its weapon's
        // primaries (GEAR.md 2): the kit is compiled again with what is worn.
        let templates = self
            .hub_slots
            .get(&id)
            .map(|h| h.worn.clone())
            .unwrap_or_default();
        self.refit(id, &templates);
    }

    /// Rounds loaded and kits used (MODES.md 11.2, 11.3): the simulation spent
    /// them; the hub's books follow, and its reading after is adopted as any.
    /// `ZoneEvent::RoundsLoaded`.
    pub(super) fn happened_rounds_loaded(&mut self, id: EntityId, hand: u8, rounds: u16) {
        let spent = self.zone.player(id).and_then(|p| {
            let kit = &p.sheet.kit;
            let slot = if hand == 0 {
                kit.primary
            } else {
                kit.secondary
            };
            kit.abilities[slot? as usize]
                .ammo_stack()
                .map(str::to_string)
        });
        if let Some(template) = spent {
            consume_stack(
                &self.cfg,
                &mut self.hub_slots,
                &self.event_tx,
                id,
                |s| s.template == template,
                rounds as u32,
            );
        }
    }

    /// `ZoneEvent::ItemUsed`.
    pub(super) fn happened_item_used(&mut self, id: EntityId, cell: u8) {
        // One of the cell's stack (LOOK.md 3.2), by the bar the slot holds.
        // Said in the log (MODES.md 11.3): a reload is not, an item is rarer
        // and what a player asks about.
        let Some(template) = self
            .hub_slots
            .get(&id)
            .and_then(|s| s.bar.get(cell as usize).cloned().flatten())
        else {
            warn!(
                body = id,
                cell, "item used from a cell the bar has nothing on"
            );
            return;
        };
        let left = self.hub_slots.get(&id).map_or(0, |s| {
            s.stacks
                .iter()
                .filter(|s| s.template == template)
                .map(|s| s.quantity)
                .sum::<u32>()
        });
        info!(
            body = id,
            character = self.hub_slots.get(&id).map(|s| s.character),
            cell = cell + 1,
            template,
            left = left.saturating_sub(1),
            "item used"
        );
        consume_stack(
            &self.cfg,
            &mut self.hub_slots,
            &self.event_tx,
            id,
            |s| s.template == template,
            1,
        );
    }

    /// `ZoneEvent::Healed`.
    pub(super) fn happened_healed(&mut self, target: EntityId, source: EntityId, amount: i32) {
        // Everyone is told what the Regen gave and whose it was: the healer's green
        // number large over the body, the others' small, the healed body's not at
        // all, since its own health says it (LOOK.md 13.8, 13.13). A Regen on the own
        // body is still told: the others see it.
        if source == target && self.sessions.len() <= 1 {
            return;
        }
        for s in self.sessions.values() {
            s.send_control(FromZone::Healed {
                source,
                target,
                amount: amount.clamp(0, u16::MAX as i32) as u16,
            });
        }
    }
}
