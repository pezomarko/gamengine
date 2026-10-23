//! What the director did this tick (COMPANIONS.md): rosters, squads, encounters, loot, trials.

use super::*;

impl Run {
    /// The director's own step after the simulation's, and what it did.
    pub(super) fn direct(&mut self, events: &[ZoneEvent]) {
        let t_minds = Instant::now();
        self.director
            .post_step(&mut self.zone, &self.world.bsp, events);
        self.phases.minds += t_minds.elapsed();
        for ev in std::mem::take(&mut self.director.events) {
            match ev {
                DirectorEvent::Spawned { id, name, kind } => self.directed_spawned(id, name, kind),
                DirectorEvent::Removed { id } => self.directed_removed(id),
                DirectorEvent::Squad { commander } => self.directed_squad(commander),
                DirectorEvent::Encounter { name, state, tell } => {
                    self.directed_encounter(name, state, tell)
                }
                DirectorEvent::Loot {
                    encounter,
                    kill,
                    grants,
                } => self.directed_loot(encounter, kill, grants),
                DirectorEvent::Trial {
                    human,
                    key,
                    name,
                    verdict,
                    standing,
                    secs,
                } => self.directed_trial(human, key, name, verdict, standing, secs),
            }
        }
        // Squads that changed: their commanders are told, once, what the squad is now.
        for commander in self.squads_dirty.drain(..) {
            let entries = squad_entries(&self.director, &self.zone, commander);
            if let Some(s) = self.sessions.get_mut(&commander)
                && s.squad_told != entries
            {
                s.squad_told = entries.clone();
                s.send_control(FromZone::Squad(entries));
            }
        }
    }

    /// `DirectorEvent::Spawned`.
    pub(super) fn directed_spawned(&mut self, id: EntityId, name: String, kind: gm_ai::BodyKind) {
        let team = self.zone.player(id).map_or(0, |p| p.team());
        let model = self.companion_models.get(&id).map(|m| m.id);
        let look = look_of(
            &self.cfg.looks,
            &self.zone,
            &self.hub_slots,
            &self.sheaths,
            id,
            npc_wears(&self.pack, kind),
        );
        let body = self.zone.player(id).and_then(|p| p.body());
        for s in self.sessions.values() {
            s.send_control(FromZone::PlayerInfo {
                id,
                name: name.clone(),
                team,
                model,
                kind: wire_kind(kind),
                look,
                body,
            });
        }
    }

    /// `DirectorEvent::Removed`.
    pub(super) fn directed_removed(&mut self, id: EntityId) {
        self.companion_models.remove(&id);
        self.hire_expiry.remove(&id);
        for s in self.sessions.values() {
            s.send_control(FromZone::PlayerLeft(id));
        }
    }

    /// `DirectorEvent::Squad`.
    pub(super) fn directed_squad(&mut self, commander: EntityId) {
        if !self.squads_dirty.contains(&commander) {
            self.squads_dirty.push(commander);
        }
    }

    /// `DirectorEvent::Encounter`.
    pub(super) fn directed_encounter(
        &mut self,
        name: String,
        state: EncounterState,
        tell: Vec<EntityId>,
    ) {
        let wire = match state {
            EncounterState::Engaged => {
                self.report.encounters_engaged += 1;
                control::EncounterState::Engaged
            }
            EncounterState::Reset => {
                self.report.encounters_reset += 1;
                control::EncounterState::Reset
            }
            EncounterState::Cleared { secs } => {
                self.report.encounters_cleared += 1;
                self.report.last_clear_secs = secs;
                control::EncounterState::Cleared { secs }
            }
        };
        info!(encounter = %name, ?state, "encounter");
        // A fight that has trials: they are for one player and a squad, and a
        // party of people is told so when it begins, not after the kill.
        let trials = state == EncounterState::Engaged
            && self
                .zone
                .content
                .trials
                .iter()
                .any(|t| t.map == self.world.name && t.encounter == name && t.max_humans == 1);
        for id in tell {
            if let Some(s) = self.sessions.get(&id) {
                s.send_control(FromZone::Encounter {
                    name: name.clone(),
                    state: wire,
                });
                let of_a_party = self
                    .hub_slots
                    .get(&id)
                    .is_some_and(|slot| self.parties.of(slot.character).is_some());
                if trials && of_a_party {
                    zone_line(
                        s,
                        "a party of people passes no trial: those are for one player and a squad",
                    );
                }
            }
        }
    }

    /// `DirectorEvent::Loot`.
    pub(super) fn directed_loot(
        &mut self,
        encounter: String,
        kill: u64,
        grants: Vec<gm_ai::director::LootGrant>,
    ) {
        // The kill's reference: unique across restarts of this zone process.
        let reference = ((self.started_unix & 0xffff_ffff) << 24) as i64 | kill as i64;
        let mut items: Vec<(CharacterId, String)> = Vec::new();
        let mut coins: Vec<(CharacterId, i64)> = Vec::new();
        for g in &grants {
            self.report.loot_items += g.items.len() as u64;
            info!(encounter = %encounter, human = g.human, items = ?g.items, coin = g.coin, "loot");
            if let Some(s) = self.sessions.get(&g.human) {
                s.send_control(FromZone::Loot {
                    encounter: encounter.clone(),
                    items: g.items.clone(),
                    coin: g.coin,
                });
            }
            if let Some(slot) = self.hub_slots.get(&g.human) {
                items.extend(g.items.iter().map(|m| (slot.character, m.clone())));
                if g.coin > 0 {
                    coins.push((slot.character, g.coin as i64));
                }
            }
        }
        // One report per kill, repeated until the hub answers; the hub pays a
        // reference once (ECONOMY.md 9).
        if let Some(link) = self.cfg.hub.clone()
            && (!items.is_empty() || !coins.is_empty())
        {
            tokio::spawn(async move {
                match link.grant_kill(reference, items, coins).await {
                    Ok(true) => {}
                    Ok(false) => info!(reference, "the kill was already paid"),
                    Err(e) => warn!(reference, "the drop of a kill is lost: {e}"),
                }
            });
        }
    }

    /// `DirectorEvent::Trial`.
    pub(super) fn directed_trial(
        &mut self,
        human: EntityId,
        key: String,
        name: String,
        verdict: Result<(), gm_core::trial::Failure>,
        standing: gm_core::trial::Standing,
        secs: u32,
    ) {
        let passed = verdict.is_ok();
        info!(human, trial = %key, passed, %standing, "trial");
        if passed {
            self.report.trials_passed += 1;
        }
        if let Some(s) = self.sessions.get(&human) {
            s.send_control(FromZone::Trial {
                key: key.clone(),
                name,
                passed,
                // Why not; or, for a pass, what the ledger said.
                detail: verdict
                    .err()
                    .map_or_else(|| standing.to_string(), |f| f.to_string()),
                secs,
            });
        }
        if passed
            && let (Some(link), Some(slot)) = (self.cfg.hub.clone(), self.hub_slots.get(&human))
        {
            let character = slot.character;
            tokio::spawn(async move {
                if let Err(e) = link.trial(character, key, secs).await {
                    warn!(character, "recording a trial: {e}");
                }
            });
        }
    }
}
