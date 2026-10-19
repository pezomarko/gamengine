//! The tick (PROTOCOL.md 2): its stages in order, each a method, timed into `Phases`.

use super::*;

impl Run {
    /// One tick: what came in, the stages of the simulation, what goes out. `false` when
    /// `max_ticks` is reached.
    pub(super) fn tick(&mut self, deadline: Instant) -> bool {
        let started = Instant::now();
        if let Some(behind) = self.scheduler.resync_if_behind(started) {
            warn!(
                behind_ms = behind.as_millis(),
                "fell behind; resynchronising"
            );
        }
        // Network events since the last tick, and the once-a-second bookkeeping.
        let t_events = Instant::now();
        while let Ok(ev) = self.rx.try_recv() {
            self.handle_event(ev);
        }
        self.settle_parties();
        self.kick_idle();
        self.hub_bookkeeping();
        self.expire_hires();
        self.refresh_lag();
        self.phases.events += t_events.elapsed();
        self.simulate();
        let events: Vec<ZoneEvent> = self.zone.events.drain(..).collect();
        let recorded: Vec<gm_replay::Event> = match &self.recorder {
            Some(_) => events.iter().filter_map(replay_event).collect(),
            None => Vec::new(),
        };
        self.direct(&events);
        self.outcomes(events);
        self.snapshots();
        self.record(recorded);
        self.phases.ticks += 1;
        self.metrics.record(
            started.saturating_duration_since(deadline),
            started.elapsed(),
        );
        self.scheduler.advance();
        if self.last_report.elapsed() >= self.cfg.report_every {
            self.report_if_due();
        }
        !self
            .cfg
            .max_ticks
            .is_some_and(|n| self.scheduler.tick() >= n)
    }

    /// The party a body fights under (PARTY.md 2), made what the hub says it is when no fight holds it.
    pub(super) fn settle_parties(&mut self) {
        for (&id, slot) in &self.hub_slots {
            let want = self.parties.number(slot.character, id);
            if self.zone.player(id).is_none_or(|p| p.party == want) {
                self.party_waits.remove(&id);
                continue;
            }
            let wait = if self.director.engaged(id) {
                Some("the party changes for you when this fight is over")
            } else if want != id && self.director.party_engaged(want) {
                Some("your party is in a fight: you are of it when the fight is over")
            } else {
                None
            };
            match wait {
                Some(why) => {
                    if self.party_waits.insert(id)
                        && let Some(s) = self.sessions.get(&id)
                    {
                        zone_line(s, why);
                    }
                }
                None => {
                    self.director.set_party(&mut self.zone, id, want);
                    // Somebody who was told to wait is told when the wait is over.
                    if self.party_waits.remove(&id)
                        && let Some(s) = self.sessions.get(&id)
                    {
                        zone_line(
                            s,
                            if want == id {
                                "the fight is over: you are on your own now"
                            } else {
                                "the fight is over: you are of the party now"
                            },
                        );
                    }
                }
            }
        }
    }

    /// Once a second: a client that sent no input leaves (`INPUT_IDLE_SECS`, `FIRST_INPUT_SECS`).
    pub(super) fn kick_idle(&mut self) {
        if self.scheduler.tick().is_multiple_of(self.rate.hz() as u64) {
            let hz = self.rate.hz() as u64;
            for (id, s) in &self.sessions {
                let secs = if s.had_input {
                    crate::session::INPUT_IDLE_SECS
                } else {
                    crate::session::FIRST_INPUT_SECS
                };
                let ghost = self
                    .hub_slots
                    .get(id)
                    .is_some_and(|h| h.ghost_since.is_some());
                let idle = self.scheduler.tick().saturating_sub(s.last_input_at);
                if !ghost && (secs * hz..(secs + 1) * hz).contains(&idle) {
                    info!(entity = *id, name = %s.name, "kicked: no input");
                    s.kick(&format!("no input for {secs} seconds"), 5);
                }
            }
        }
    }

    /// Once a second under a hub: the periodic saves (staggered) and the ghosts that timed out.
    pub(super) fn hub_bookkeeping(&mut self) {
        if let Some(link) = &self.cfg.hub
            && self.scheduler.tick().is_multiple_of(self.rate.hz() as u64)
        {
            let now = Instant::now();
            let mut expired_ghosts: Vec<EntityId> = Vec::new();
            for (&id, slot) in self.hub_slots.iter_mut() {
                if let Some(since) = slot.ghost_since
                    && now.duration_since(since) >= GHOST_TIMEOUT
                {
                    slot.ghost_since = None;
                    match self.sessions.get(&id) {
                        Some(s) => {
                            // The other zone never claimed: the player plays on here. The
                            // hub still has the character on its way, and learns that it
                            // is here again from a save: at once (the block below), not in
                            // up to half a minute.
                            slot.last_save = now - SAVE_EVERY;
                            self.zone.set_ghost(id, false);
                            s.send_control(FromZone::TravelRefused(
                                "the other zone never claimed you".into(),
                            ));
                        }
                        // The client is gone too: the body leaves and the character goes
                        // offline (the hub accepts the origin's leaving save of a transit).
                        None => expired_ghosts.push(id),
                    }
                }
                if slot.ghost_since.is_none()
                    && now.duration_since(slot.last_save) >= SAVE_EVERY
                    && let Some(state) = character_state(&self.zone, link, id, slot)
                {
                    slot.last_save = now;
                    let link = link.clone();
                    let character = slot.character;
                    let tx = self.event_tx.clone();
                    tokio::spawn(async move {
                        let said = match link.save(character, state, false).await {
                            // What the hub holds of its party, by number: the zone looks
                            // whether it is behind (PARTY.md 3.2).
                            Ok(seq) => ClientEvent::PartySeq { character, seq },
                            Err(()) => ClientEvent::SaveRefused { character },
                        };
                        let _ = tx.send(said).await;
                    });
                }
            }
            for id in expired_ghosts {
                let slot = self.hub_slots.remove(&id);
                if let Some(slot) = &slot {
                    self.parties.left(slot.character, Instant::now());
                    if self.director.engaged(id) {
                        self.left_fights.insert(slot.character, id);
                    }
                }
                self.party_waits.remove(&id);
                self.trade_asks
                    .retain(|asker, (asked, _)| *asker != id && *asked != id);
                depart(
                    &mut self.recorder,
                    Some(link),
                    &mut self.recent_left,
                    id,
                    slot.as_ref().map(|s| s.character),
                );
                if let Some(slot) = slot
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
                self.director.human_left(&mut self.zone, id);
                self.zone.remove_player(id);
                for s in self.sessions.values() {
                    s.send_control(FromZone::PlayerLeft(id));
                }
                self.report.leaves += 1;
            }
        }
    }

    /// Hires that ran out or ended early: the companion leaves once out of its encounter (COMPANIONS.md 3.3).
    pub(super) fn expire_hires(&mut self) {
        if self.scheduler.tick().is_multiple_of(self.rate.hz() as u64)
            && !self.hire_expiry.is_empty()
        {
            let now = now_secs();
            let due: Vec<EntityId> = self
                .hire_expiry
                .iter()
                .filter(|(id, at)| **at <= now && !self.director.in_encounter(**id))
                .map(|(id, _)| *id)
                .collect();
            for id in due {
                self.hire_expiry.remove(&id);
                self.director.remove_companion(&mut self.zone, id);
            }
        }
    }

    /// Latency-bounded lag compensation (PROTOCOL.md 4), refreshed once a second.
    pub(super) fn refresh_lag(&mut self) {
        if self.scheduler.tick().is_multiple_of(self.rate.hz() as u64) {
            for s in self.sessions.values() {
                let half_rtt = s.conn.rtt() / 2;
                let ticks = half_rtt.as_secs_f64() / self.rate.period().as_secs_f64();
                self.zone.set_half_rtt_ticks(s.id, ticks.ceil() as u32);
            }
        }
    }

    /// The minds' pre-step, the simulation step, and the looks that follow it: the gun hand (MODES.md 3.7) and the sheath (LOOK.md 6.7).
    pub(super) fn simulate(&mut self) {
        let t_minds = Instant::now();
        self.director.pre_step(&mut self.zone, &self.world.bsp);
        self.phases.minds += t_minds.elapsed();
        let t_sim = Instant::now();
        self.zone.step(&self.world.bsp);
        self.phases.sim += t_sim.elapsed();
        // The weapon switched to in the gun mode (MODES.md 3.7) is what the hand shows
        // (LOOK.md 6.2): everyone is told when it changes.
        for s in self.sessions.values() {
            let Some(p) = self.zone.player(s.id) else {
                continue;
            };
            if p.sheet.kit.mode != gm_core::vocab::Mode::Gun {
                continue;
            }
            let held = p.mover.held;
            if self.hands_seen.insert(s.id, held) != Some(held) {
                let look = look_of(
                    &self.cfg.looks,
                    &self.zone,
                    &self.hub_slots,
                    &self.sheaths,
                    s.id,
                    None,
                );
                for other in self.sessions.values() {
                    other.send_control(FromZone::Look { id: s.id, look });
                }
            }
        }
        // The sheath (LOOK.md 6.7, EQUIPMENT.md 5.3): an RPG body's weapon rests on its
        // back once it has neither fought nor swung for the fight lock's ten seconds, and
        // is in its fist again the tick it does either; everyone is told when that
        // changes and the body holds something. (A gun build's hand is its own, above.)
        let fight = self
            .rate
            .ms_to_ticks(self.cfg.gear_after_fight.as_millis() as u32);
        for s in self.sessions.values() {
            let Some(p) = self.zone.player(s.id) else {
                continue;
            };
            if p.sheet.kit.mode == gm_core::vocab::Mode::Gun || !p.alive || p.ghost {
                continue;
            }
            let now = sheathed_now(
                p,
                self.armed.entry(s.id).or_default(),
                self.zone.tick,
                fight,
            );
            if self.sheaths.insert(s.id, now) != Some(now) {
                let look = look_of(
                    &self.cfg.looks,
                    &self.zone,
                    &self.hub_slots,
                    &self.sheaths,
                    s.id,
                    None,
                );
                if look.held == Look::NONE {
                    continue;
                }
                for other in self.sessions.values() {
                    other.send_control(FromZone::Look { id: s.id, look });
                }
            }
        }
    }

    /// The recorder's tick (ANTICHEAT.md 3): the frame, the reactions in sight, the aim analysis; files written off the loop.
    pub(super) fn record(&mut self, mut recorded: Vec<gm_replay::Event>) {
        let tick = self.zone.tick;
        if let Some(rec) = &mut self.recorder {
            let t_sight = Instant::now();
            self.sight.tick(
                &self.zone,
                &self.world.bsp,
                !self.cfg.wild,
                &|id| self.sessions.contains_key(&id),
                &mut recorded,
            );
            self.phases.sight += t_sight.elapsed();
            let t_record = Instant::now();
            let describe = |id: EntityId| -> Option<RosterEntry> {
                let p = self.zone.player(id)?;
                let (name, kind) = match self.sessions.get(&id) {
                    Some(s) => (s.name.clone(), BodyKind::Human),
                    None => self
                        .director
                        .driven()
                        .into_iter()
                        .find(|(body, _, _)| *body == id)
                        .map(|(_, name, kind)| (name, wire_kind(kind)))?,
                };
                Some(RosterEntry {
                    id,
                    name,
                    team: p.team(),
                    kind,
                    party: p.party,
                    build: self
                        .cfg
                        .content
                        .builds
                        .iter()
                        .find(|b| b.build == p.sheet.build)
                        .map_or_else(|| "custom".to_string(), |b| b.name.clone()),
                    character: self.hub_slots.get(&id).map_or(0, |h| h.character),
                    mode: p.sheet.build.mode as u8,
                })
            };
            // How far behind this tick each client's frames say they look.
            let view_lag = |id: EntityId| -> Option<u8> {
                self.sessions.contains_key(&id).then_some(())?;
                let p = self.zone.player(id)?;
                Some(tick.wrapping_sub(p.view_claimed).min(MAX_CLAIMED_VIEW_LAG) as u8)
            };
            let done = rec.tick(
                tick,
                &self.table,
                std::mem::take(&mut recorded),
                &describe,
                &view_lag,
            );
            self.phases.record += t_record.elapsed();
            for finished in done {
                let tx = self.written_tx.clone();
                // Compressing and writing a file is not the tick loop's work.
                self.writing.retain(|t| !t.is_finished());
                self.writing.push(tokio::task::spawn_blocking(move || {
                    match finished.write() {
                        Ok(w) => {
                            let _ = tx.send(w);
                        }
                        Err(e) => warn!("writing a replay: {e}"),
                    }
                }));
            }
        }
        while let Ok(w) = self.written_rx.try_recv() {
            replay_written(&w);
            if let Some(link) = &self.cfg.hub {
                let link = link.clone();
                tokio::spawn(async move { link.replay(w).await });
            }
        }
    }

    /// The zone report, every `report_every` (and once at the end).
    pub(super) fn report_if_due(&mut self) {
        let secs = self.last_report.elapsed().as_secs_f64();
        fill_report(
            &mut self.report,
            &mut self.sessions,
            &self.zone,
            &self.metrics,
            secs,
            self.scheduler.tick(),
        );
        self.phases.fill(&mut self.report);
        self.report.minds = self.director.minds();
        if let Some(rec) = &self.recorder {
            self.report.replay_frames = rec.stats.frames;
            self.report.replay_frame_bytes = rec.stats.frame_bytes;
            self.report.replays_written = rec.stats.fights_written + rec.stats.reports_written;
            self.report.replay_ring_bytes = rec.stats.ring_bytes;
        }
        self.phases = Phases::default();
        *self.hub_stats.lock().unwrap() =
            (self.report.players as u32, self.report.tick_mean_us as f32);
        info!(
            tick = self.report.tick,
            players = self.report.players,
            tx_bps = format_args!("{:.0}", self.report.tx_bytes_per_player_s),
            rx_bps = format_args!("{:.0}", self.report.rx_bytes_per_player_s),
            snap_bps = format_args!("{:.0}", self.report.snapshot_payload_per_player_s),
            tick_us_mean = format_args!("{:.1}", self.report.tick_mean_us),
            tick_us_p99 = format_args!("{:.1}", self.report.tick_p99_us),
            tick_us_max = format_args!("{:.1}", self.report.tick_max_us),
            late_us_max = format_args!("{:.1}", self.report.late_max_us),
            overruns = self.report.overruns,
            events_us = format_args!("{:.0}", self.report.events_us_mean),
            sim_us = format_args!("{:.0}", self.report.sim_us_mean),
            snapshot_us = format_args!("{:.0}", self.report.snapshot_us_mean),
            send_us = format_args!("{:.0}", self.report.send_us_mean),
            minds = self.report.minds,
            minds_us = format_args!("{:.0}", self.report.minds_us_mean),
            record_us = format_args!("{:.0}", self.report.record_us_mean),
            sight_us = format_args!("{:.0}", self.report.sight_us_mean),
            replay_ring_bytes = self.report.replay_ring_bytes,
            replay_frame_bytes = self.report.replay_frame_bytes,
            replays = self.report.replays_written,
            starved = self.report.starved_ticks + self.report.live_starved_ticks,
            executed = self.report.executed_frames + self.report.live_executed_frames,
            hits = self.report.hits_melee + self.report.hits_projectile + self.report.hits_area,
            kills = self.report.kills,
            "zone report"
        );
        if let Some(w) = &self.cfg.report_tx {
            let _ = w.send(self.report.clone());
        }
        self.metrics.reset_window();
        self.last_report = Instant::now();
    }
}
