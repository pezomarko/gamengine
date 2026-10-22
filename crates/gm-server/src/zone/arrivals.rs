//! Bodies that come and go: a join, a leave, a claim by another zone, the hub's kicks.

use super::*;

/// A client's `Join` as the network delivered it (`ClientEvent::Join`).
pub(super) struct Arrival {
    pub name: String,
    pub build: Option<BuildChoice>,
    pub team: u8,
    pub hub: Option<HubJoin>,
    pub conn: Link,
    pub control: mpsc::Sender<FromZone>,
    pub reply: oneshot::Sender<Result<JoinInfo, String>>,
}

impl Run {
    /// `ClientEvent::Join`: room made for the body, the body placed, the hub's reading
    /// adopted, the session admitted and announced, its squad raised, the joiner told what
    /// is here.
    pub(super) fn on_join(&mut self, a: Arrival) {
        let Arrival {
            name,
            build,
            team,
            hub,
            conn,
            control,
            reply,
        } = a;
        let placed = match self.make_room(hub.as_ref()) {
            Ok(()) => self.place_arrival(build.as_ref(), team, hub.as_ref()),
            Err(why) => Err(why),
        };
        let (id, build, team) = match placed {
            Ok(p) => p,
            Err(why) => {
                let _ = reply.send(Err(why));
                return;
            }
        };
        self.revoked.retain(|(_, at)| at.elapsed() < REVOKED_MEMORY);
        let model = hub
            .as_ref()
            .and_then(|h| h.model)
            .filter(|m| !self.revoked.iter().any(|(id, _)| *id == m.id));
        let hired = hub.as_ref().map(|h| h.squad.clone()).unwrap_or_default();
        let hub_gm = hub.as_ref().is_some_and(|h| h.gm);
        if let Some(h) = hub {
            self.adopt_reading(id, h);
        }
        let worn = self
            .zone
            .player(id)
            .map(|p| p.worn.clone())
            .unwrap_or_default();
        let _ = reply.send(Ok(JoinInfo {
            entity: id,
            server_tick: self.zone.tick,
            build,
            team,
            worn,
            pack: self.pack.clone(),
        }));
        let session = Session::new(id, name, conn, control);
        self.admit(session, team, model, hub_gm);
        self.raise_squad(id, hired);
        self.welcome(id);
        self.report.joins += 1;
    }

    /// Room for a character's new body: its old body here goes (a reconnect the hub
    /// allowed), unless a fight holds it or the fight it left is not over; and the zone
    /// is not full.
    fn make_room(&mut self, hub: Option<&HubJoin>) -> Result<(), String> {
        // The same character again: drop the old body first, so that it does not count
        // against the room it frees. Not while that body is in a fight (PARTY.md 2): a
        // new body would be alive where it is dead and held, so the fight goes on with
        // the body it has (its own connection, or nobody's until it is kicked), and the
        // newcomer is turned away.
        if let Some(h) = hub
            && let Some(old) = self.body_of(h.character)
        {
            if self.director.engaged(old) {
                return Err("your body here is still in a fight: come back when it is over".into());
            }
            self.drop_rejoined(old, h.character);
        }
        // Nobody who left a fight comes back into it (PARTY.md 2): a new body would be
        // alive where the old one was dead and held.
        self.left_fights
            .retain(|_, old| self.director.in_encounter(*old));
        if let Some(h) = hub
            && self.left_fights.contains_key(&h.character)
        {
            return Err("the fight you left here is not over: come back when it is".into());
        }
        if self.sessions.len() >= self.cfg.max_players {
            return Err("zone full".into());
        }
        Ok(())
    }

    /// The old body of a character that joins again: what it wore is kept for the new
    /// one (the claim's reading may be the older of the two), and it leaves.
    fn drop_rejoined(&mut self, old: EntityId, character: CharacterId) {
        if let (Some(slot), Some(p)) = (self.hub_slots.get(&old), self.zone.player(old)) {
            let reading = GearReading {
                seq: slot.gear_seq,
                gear: p.gear,
                templates: slot.worn.clone(),
                stacks: slot.stacks.clone(),
                bar: slot.bar.clone(),
                essence: slot.essence.clone(),
                locked: slot.locked,
            };
            self.gear_kept.insert(character, (reading, Instant::now()));
        }
        // (Its party's reading is kept the same way, by `parties`.)
        self.parties.left(character, Instant::now());
        self.director.human_left(&mut self.zone, old);
        self.zone.remove_player(old);
        if let Some(s) = self.sessions.remove(&old) {
            s.conn.close(4, b"character joined again");
        }
        self.forget_body(old);
        // (The old session's own leave finds nothing left to announce.)
        for s in self.sessions.values() {
            s.send_control(FromZone::PlayerLeft(old));
        }
        depart(
            &mut self.recorder,
            self.cfg.hub.as_ref(),
            &mut self.recent_left,
            old,
            Some(character),
        );
    }

    /// The zone's own books of a body that is gone: its hub slot, its looks, what it
    /// waited for.
    pub(super) fn forget_body(&mut self, id: EntityId) {
        self.hub_slots.remove(&id);
        self.hands_seen.remove(&id);
        self.sheaths.remove(&id);
        self.armed.remove(&id);
        self.party_waits.remove(&id);
        self.trade_asks
            .retain(|asker, (asked, _)| *asker != id && *asked != id);
    }

    /// The build resolved, the team, and the body placed: at its saved position when that
    /// is still a place to stand (the map may have been rebuilt since), else at a spawn.
    fn place_arrival(
        &mut self,
        build: Option<&BuildChoice>,
        team: u8,
        hub: Option<&HubJoin>,
    ) -> Result<(EntityId, Build, u8), String> {
        let build = resolve_build(&self.cfg, &self.zone, build)?;
        let team = if self.cfg.wild {
            1
        } else if team == 0 || team > 2 {
            self.zone.smallest_team()
        } else {
            team
        };
        let resume = hub.and_then(|h| h.origin).filter(|(origin, _)| {
            !self.cfg.arrive_at_entry
                && origin.is_finite()
                && self.world.bsp.point_contents(Hull::Player, *origin) == Contents::Empty
        });
        let id = match resume {
            Some((origin, yaw)) => {
                build
                    .validate(&self.zone.content)
                    .map_err(|e| format!("invalid build: {e}"))?;
                self.zone.add_player_at(build.clone(), team, origin, yaw)
            }
            None => self
                .zone
                .add_player(&self.world.bsp, build.clone(), team)
                .map_err(|e| format!("invalid build: {e}"))?,
        };
        Ok((id, build, team))
    }

    /// What it wears (ITEMS.md 3.3): the hub's reading at the claim, or a later one this
    /// zone already has for the character (the answer to a change its last body asked
    /// for); from here on it changes only through this zone. Its party (PARTY.md 4): the
    /// claim's reading, or a newer one the zone was told of before the body was here; the
    /// number its body carries follows before this tick is simulated, unless its party is
    /// in a fight here. The slot keeps the hub's books from here on.
    fn adopt_reading(&mut self, id: EntityId, h: HubJoin) {
        let mut reading = h.gear.clone();
        if let Some((kept, _)) = self.gear_kept.remove(&h.character)
            && kept.seq > reading.seq
        {
            reading = kept;
        }
        self.zone.set_gear(id, reading.gear);
        self.zone.set_stacks(id, &stacks_of(&reading), &reading.bar);
        self.parties.joined(h.character, h.party);
        let templates = reading.templates.clone();
        self.hub_slots.insert(
            id,
            HubSlot {
                gear_seq: reading.seq,
                worn: reading.templates.clone(),
                stacks: reading.stacks.clone(),
                bar: reading.bar.clone(),
                essence: reading.essence.clone(),
                locked: reading.locked,
                character: h.character,
                joined: Instant::now(),
                play_seconds_before: h.play_seconds,
                // Staggered so a zone full of players never saves at once.
                last_save: Instant::now()
                    - Duration::from_secs((id % SAVE_EVERY.as_secs() as u32) as u64),
                ghost_since: None,
                claimed_elsewhere: false,
            },
        );
        // The kit its worn things bring (GEAR.md 2), before the client is greeted with it.
        self.refit(id, &templates);
    }

    /// The session admitted: a game master's word (GM.md 1, the hub's or the zone's own
    /// list), the model it wears, and everybody told of the new body.
    fn admit(&mut self, mut session: Session, team: u8, model: Option<ModelRef>, hub_gm: bool) {
        let id = session.id;
        session.last_input_at = self.scheduler.tick();
        session.gm = hub_gm || self.cfg.gm_names.contains(&session.name);
        if session.gm {
            info!(entity = id, name = %session.name, "game master");
            session.send_control(FromZone::Gm(GmNews::Granted));
            if !self.tuning.is_default() {
                session.send_control(FromZone::Gm(GmNews::Tuning(self.tuning.clone())));
            }
        }
        session.model = model;
        session.announced = self.zone.player(id).and_then(|p| session.wears(p.frame()));
        let look = look_of(
            &self.cfg.looks,
            &self.zone,
            &self.hub_slots,
            &self.sheaths,
            id,
            None,
        );
        for s in self.sessions.values() {
            s.send_control(FromZone::PlayerInfo {
                id,
                name: session.name.clone(),
                team,
                model: session.announced,
                kind: BodyKind::Human,
                look,
            });
        }
        self.sessions.insert(id, session);
    }

    /// The squad (COMPANIONS.md 3.2): hired avatars first, then the zone's recruits for
    /// the slots left.
    fn raise_squad(&mut self, id: EntityId, hired: Vec<gm_hub_proto::protocol::HiredAvatar>) {
        if !self.cfg.squads {
            return;
        }
        for h in hired {
            let spec = CompanionSpec {
                name: h.name.clone(),
                build: h.build.clone(),
                recruit: false,
                hire: Some(h.hire),
            };
            if let Some(c) = self
                .director
                .add_companion(&mut self.zone, &self.world.bsp, id, spec)
            {
                self.hire_expiry.insert(c, h.expires_at);
                if let Some(m) = h.model.filter(|m| {
                    m.frame == crate::session::frame_index(h.build.frame)
                        && !self.revoked.iter().any(|(r, _)| *r == m.id)
                }) {
                    self.companion_models.insert(c, m);
                }
            }
        }
        for preset in &self.cfg.recruits {
            let Some(build) = self.zone.content.build(preset).cloned() else {
                continue;
            };
            let spec = CompanionSpec {
                name: recruit_name(preset),
                build,
                recruit: true,
                hire: None,
            };
            self.director
                .add_companion(&mut self.zone, &self.world.bsp, id, spec);
        }
    }

    /// The joiner learns everyone, itself included, in one message (a town of hundreds
    /// must not overflow its control queue); then its party, whoever invited it while it
    /// was on its way, and the market.
    fn welcome(&mut self, id: EntityId) {
        let mut roster: Vec<PlayerEntry> = self
            .sessions
            .values()
            .map(|s| PlayerEntry {
                id: s.id,
                name: s.name.clone(),
                team: self.zone.player(s.id).map_or(0, |p| p.team()),
                model: s.announced,
                kind: BodyKind::Human,
                look: look_of(
                    &self.cfg.looks,
                    &self.zone,
                    &self.hub_slots,
                    &self.sheaths,
                    s.id,
                    None,
                ),
            })
            .collect();
        roster.extend(
            self.director
                .driven()
                .into_iter()
                .map(|(body, name, kind)| PlayerEntry {
                    id: body,
                    name,
                    team: self.zone.player(body).map_or(0, |p| p.team()),
                    model: self.companion_models.get(&body).map(|m| m.id),
                    kind: wire_kind(kind),
                    look: look_of(
                        &self.cfg.looks,
                        &self.zone,
                        &self.hub_slots,
                        &self.sheaths,
                        body,
                        npc_wears(&self.pack, kind),
                    ),
                }),
        );
        self.sessions[&id].send_control(FromZone::Roster(roster));
        if let Some(slot) = self.hub_slots.get(&id) {
            if self.parties.of(slot.character).is_some() {
                let names = self.parties.names(slot.character);
                self.sessions[&id].send_control(FromZone::Party(names));
            }
            let character = slot.character;
            self.invites_kept.retain(|(to, from, at)| {
                let waits = at.elapsed() < INVITE_KEPT;
                if waits && *to == character {
                    self.sessions[&id].send_control(FromZone::Invited { from: from.clone() });
                }
                waits && *to != character
            });
        }
        if !self.stalls.is_empty() {
            self.sessions[&id].send_control(FromZone::Stalls(
                self.stalls
                    .values()
                    .filter_map(|s| stall_entry(&self.world, s, &self.revoked))
                    .collect(),
            ));
        }
    }

    /// `ClientEvent::HubClaimed`.
    pub(super) fn on_hub_claimed(&mut self, character: CharacterId) {
        if let Some(id) = self.body_of(character) {
            if let Some(slot) = self.hub_slots.get_mut(&id) {
                slot.claimed_elsewhere = true;
            }
            if self.director.engaged(id) {
                self.left_fights.insert(character, id);
            }
            self.director.human_left(&mut self.zone, id);
            self.zone.remove_player(id);
            if let Some(s) = self.sessions.remove(&id) {
                s.send_control(FromZone::Kick("claimed by another zone".into()));
                s.conn.close(0, b"travelled");
            }
            self.hub_slots.remove(&id);
            self.hands_seen.remove(&id);
            self.sheaths.remove(&id);
            self.armed.remove(&id);
            self.parties.left(character, Instant::now());
            self.party_waits.remove(&id);
            self.trade_asks
                .retain(|asker, (asked, _)| *asker != id && *asked != id);
            depart(
                &mut self.recorder,
                self.cfg.hub.as_ref(),
                &mut self.recent_left,
                id,
                Some(character),
            );
            // The body leaves here: said once, counted once (the `Leave` that
            // follows the closed connection finds nothing left to announce).
            for s in self.sessions.values() {
                s.send_control(FromZone::PlayerLeft(id));
            }
            self.report.leaves += 1;
        }
    }

    /// `ClientEvent::HubKick`.
    pub(super) fn on_hub_kick(&mut self, character: CharacterId, reason: String) {
        let here = self
            .hub_slots
            .iter()
            .find(|(_, s)| s.character == character)
            .and_then(|(id, _)| self.sessions.get(id));
        match here {
            Some(s) => s.kick(&reason, 3),
            // The hub has the character here and the zone has no body for
            // it. If it has just left, the save it left with is on its way
            // and says so itself (a logout races its own leaving save, and
            // that save must not find the character gone). Otherwise the hub
            // is told, or the character would stay "here" until this zone
            // restarts. (A client of its that is still on the way in gets a
            // body all the same; the hub refuses its first save, and that
            // ends it: `SaveRefused`.)
            None if self.leaving_saves.contains(&character) => {}
            None => {
                if let Some(link) = self.cfg.hub.clone() {
                    tokio::spawn(async move { link.release(character).await });
                }
            }
        }
    }

    /// `ClientEvent::LeftSaved`.
    pub(super) fn on_left_saved(&mut self, character: CharacterId) {
        self.leaving_saves.remove(&character);
    }

    /// `ClientEvent::SaveRefused`.
    pub(super) fn on_save_refused(&mut self, character: CharacterId) {
        // The hub does not have this character here (it was logged out or
        // banned while its client was on the way in, or the hub lost track):
        // what the hub does not know of does not play on. Not a traveller on
        // its way out, whose character is the other zone's by now.
        let here = self
            .hub_slots
            .iter()
            .find(|(_, s)| {
                s.character == character && s.ghost_since.is_none() && !s.claimed_elsewhere
            })
            .and_then(|(id, _)| self.sessions.get(id));
        if let Some(s) = here {
            warn!(
                character,
                "the hub does not have this character here: it leaves"
            );
            s.kick("the hub no longer has this character here", 3);
        }
    }

    /// `ClientEvent::HubModelRevoked`.
    pub(super) fn on_hub_model_revoked(&mut self, model: ModelId) {
        self.revoked.retain(|(_, at)| at.elapsed() < REVOKED_MEMORY);
        self.revoked.push((model, Instant::now()));
        for s in self.sessions.values_mut() {
            if s.model.is_some_and(|m| m.id == model) {
                s.model = None;
                s.announced = None;
            }
        }
        for stall in self.stalls.values_mut() {
            if stall.model.is_some_and(|m| m.id == model) {
                stall.model = None;
            }
        }
        // Everyone forgets it, whoever wore it.
        for s in self.sessions.values() {
            s.send_control(FromZone::ModelRevoked(model));
        }
    }

    /// `ClientEvent::Leave`.
    pub(super) fn on_leave(&mut self, id: EntityId) {
        let leaving_character = self.hub_slots.get(&id).map(|s| s.character);
        // A ghost's connection ends with its `Bye` on the way to another zone: the
        // body and the hub's transit stay until the claim or the timeout (HUB.md 3.3).
        if self
            .hub_slots
            .get(&id)
            .is_some_and(|s| s.ghost_since.is_some())
        {
            if let Some(s) = self.sessions.remove(&id) {
                self.report.oversize_drops += s.oversize_drops;
                self.report.send_failures += s.send_failures;
            }
            // The body is still here: it leaves at the claim or the timeout.
            return;
        }
        if let (Some(link), Some(slot)) = (&self.cfg.hub, self.hub_slots.remove(&id))
            && !slot.claimed_elsewhere
            && let Some(state) = character_state(&self.zone, link, id, &slot)
        {
            let (link, tx) = (link.clone(), self.event_tx.clone());
            let character = slot.character;
            self.leaving_saves.insert(character);
            tokio::spawn(async move {
                let _ = link.save(character, state, true).await;
                let _ = tx.send(ClientEvent::LeftSaved { character }).await;
            });
        }
        if let Some(character) = leaving_character {
            self.parties.left(character, Instant::now());
            if self.director.engaged(id) {
                self.left_fights.insert(character, id);
            }
        }
        self.party_waits.remove(&id);
        self.trade_asks
            .retain(|asker, (asked, _)| *asker != id && *asked != id);
        self.director.human_left(&mut self.zone, id);
        let body = self.zone.remove_player(id);
        if body.is_some() {
            depart(
                &mut self.recorder,
                self.cfg.hub.as_ref(),
                &mut self.recent_left,
                id,
                leaving_character,
            );
        }
        if let Some(p) = &body {
            info!(
                entity = id,
                executed = p.executed_frames,
                starved = p.starved_ticks,
                dropped = p.dropped_frames,
                queued = p.queued_frames(),
                kills = p.kills,
                deaths = p.deaths,
                "player stats at leave"
            );
            self.report.executed_frames += p.executed_frames;
            self.report.starved_ticks += p.starved_ticks;
            self.report.dropped_frames += p.dropped_frames;
        }
        let session = self.sessions.remove(&id);
        if let Some(s) = &session {
            self.report.oversize_drops += s.oversize_drops;
            self.report.send_failures += s.send_failures;
            let st = s.conn.stats();
            info!(
                entity = id,
                name = %s.name,
                web = s.conn.is_web(),
                damage_dealt = s.damage_dealt,
                damage_taken = s.damage_taken,
                hits_landed = s.hits_landed,
                udp_tx_bytes = st.udp_tx.bytes,
                udp_rx_bytes = st.udp_rx.bytes,
                rtt_ms = format_args!("{:.1}", s.conn.rtt().as_secs_f64() * 1000.0),
                "session at leave"
            );
        }
        // Claimed by another zone a moment ago: that said and counted it.
        if body.is_none() && session.is_none() {
            return;
        }
        for s in self.sessions.values() {
            s.send_control(FromZone::PlayerLeft(id));
        }
        self.report.leaves += 1;
    }
}
