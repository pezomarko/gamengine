//! What a body wears and runs on: a team zone's respec, wearing, the game masters' tuning (GM.md).

use super::*;

impl Run {
    /// `ClientEvent::Respec`: a build chosen in a team zone (the arena, the practice
    /// ground: the arena's keys, a bot's counter-pick) is worn at the next respawn. In
    /// the world a build is the character's from its creation (MATRIX.md 20, 2026-10-20:
    /// no trainer, nothing redone) and the ask is refused in words.
    pub(super) fn on_respec(&mut self, id: EntityId, build: BuildChoice) {
        let result = resolve_build(&self.cfg, &self.zone, Some(&build)).and_then(|b| {
            // (A body is for keeps in the world, BODIES.md 3.5, where every respec is
            // refused; a team zone's presets are bodies since BODIES.md 7.5, and a bot's
            // counter-pick from one to another is the body changing.)
            if self.cfg.wild {
                return Err("a build is set at creation: there is no trainer".into());
            }
            self.zone.request_respec(id, b).map_err(|e| e.to_string())
        });
        // Under a hub the build it chose is saved at once, and the answer is
        // the save's: a build worn here that the hub does not hold would be
        // the old one again at the next claim. (The periodic save carries it
        // too, as it carries a pending one.)
        let saving = result.is_ok()
            && self
                .cfg
                .hub
                .as_ref()
                .zip(self.hub_slots.get_mut(&id))
                .and_then(|(link, slot)| {
                    let state = character_state(&self.zone, link, id, slot)?;
                    slot.last_save = Instant::now();
                    let (link, tx, character) =
                        (link.clone(), self.event_tx.clone(), slot.character);
                    tokio::spawn(async move {
                        let saved = link.save_told(character, state).await;
                        let result = match saved {
                            Ok(seq) => {
                                let _ = tx.send(ClientEvent::PartySeq { character, seq }).await;
                                Ok(())
                            }
                            Err(why) => Err(why),
                        };
                        let _ = tx.send(ClientEvent::RespecSaved { id, result }).await;
                    });
                    Some(())
                })
                .is_some();
        if !saving && let Some(s) = self.sessions.get(&id) {
            s.send_control(FromZone::RespecResult(result));
        }
    }

    /// `ClientEvent::RespecSaved`.
    pub(super) fn on_respec_saved(&mut self, id: EntityId, result: Result<(), String>) {
        if let Err(why) = &result {
            warn!(entity = id, "a respec was worn but not saved: {why}");
        }
        if let Some(s) = self.sessions.get(&id) {
            s.send_control(FromZone::RespecResult(
                result.map_err(|why| format!("worn here, but not saved: {why}")),
            ));
        }
    }

    /// `ClientEvent::Gm`: a game master's act (GM.md 2); anyone else's is refused. A change
    /// of the tuning is tried on the content as loaded; what content would refuse is
    /// refused here, and the tuning stands as it was.
    pub(super) fn on_gm(&mut self, id: EntityId, op: GmOp) {
        let Some(s) = self.sessions.get(&id) else {
            return;
        };
        if !s.gm {
            s.send_control(FromZone::Gm(GmNews::Refused("not a game master".into())));
            return;
        }
        let name = s.name.clone();
        let (result, tuned) = self.gm_act(id, &name, op);
        let result = match tuned {
            Some(next) if result.is_ok() => self.retune(next, &name),
            _ => result,
        };
        if let (Err(e), Some(s)) = (result, self.sessions.get(&id)) {
            s.send_control(FromZone::Gm(GmNews::Refused(e)));
        }
    }

    /// The act: a heal or a respec is done here; a change of the tuning is only made up
    /// (`retune` makes it stand).
    fn gm_act(
        &mut self,
        id: EntityId,
        name: &str,
        op: GmOp,
    ) -> (Result<(), String>, Option<Tuning>) {
        let mut tuned = None;
        let result: Result<(), String> = match op {
            GmOp::Tempo(t) => {
                let (lo, hi) = gm_core::tuning::TEMPO_RANGE;
                if !(lo..=hi).contains(&t) {
                    Err(format!("the tempo is between {lo} and {hi}"))
                } else {
                    let mut next = self.tuning.clone();
                    next.tempo = t;
                    tuned = Some(next);
                    Ok(())
                }
            }
            GmOp::Ability(a) => {
                if !self.cfg.content.abilities.iter().any(|d| d.key == a.key) {
                    Err(format!("no ability {}", a.key))
                } else {
                    let mut next = self.tuning.clone();
                    next.set(a);
                    tuned = Some(next);
                    Ok(())
                }
            }
            GmOp::ResetTuning => {
                tuned = Some(Tuning::default());
                Ok(())
            }
            GmOp::Heal { everyone } => {
                let ids: Vec<EntityId> = if everyone {
                    self.zone.players().map(|p| p.id).collect()
                } else {
                    vec![id]
                };
                for who in ids {
                    self.zone.make_whole(who);
                }
                info!(%name, everyone, "gm: healed");
                Ok(())
            }
            GmOp::Respec(build) => {
                let r = self.zone.respec_now(id, build).map_err(|e| e.to_string());
                if r.is_ok() {
                    info!(%name, "gm: respec now");
                }
                r
            }
        };
        (result, tuned)
    }

    /// A tuning content accepts stands: the zone runs the new numbers, the file keeps
    /// them, everyone gets a new `Content`, and the game masters the tuning as it is.
    fn retune(&mut self, next: Tuning, name: &str) -> Result<(), String> {
        let next_pack = next.apply(&self.cfg.content, self.rate);
        if let Err(e) = next_pack.validate(self.rate) {
            return Err(format!("content refuses that: {e}"));
        }
        self.tuning = next;
        self.pack = Arc::new(next_pack);
        self.zone.retune((*self.pack).clone());
        info!(
            %name,
            tempo = self.tuning.tempo,
            abilities = self.tuning.abilities.len(),
            "gm: content tuned"
        );
        if let Some(path) = &self.cfg.tuning_file
            && let Err(e) = gm_content::tuning::write(path, &self.tuning)
        {
            warn!(path = %path.display(), "tuning not written: {e}");
        }
        for (sid, s) in &self.sessions {
            let Some(p) = self.zone.player(*sid) else {
                continue;
            };
            s.send_control(FromZone::Content {
                pack: (*self.pack).clone(),
                own: p.sheet.build.clone(),
                team: p.team(),
                props: self.cfg.looks.props.clone(),
                worn: p.worn.clone(),
            });
            if s.gm {
                s.send_control(FromZone::Gm(GmNews::Tuning(self.tuning.clone())));
            }
        }
        Ok(())
    }

    /// `ClientEvent::Wear`.
    pub(super) fn on_wear(&mut self, id: EntityId, item: i64, on: bool, place: Option<u8>) {
        let Some(session) = self.sessions.get_mut(&id) else {
            return;
        };
        let refuse = |session: &Session, why: &str| {
            session.send_control(FromZone::WearResult {
                item,
                result: Err(why.to_string()),
            });
        };
        if !session.begin_gear_request() {
            refuse(session, "one thing at a time: try again in a moment");
            return;
        }
        // What a body wears changes while it stands alive and in person, and
        // not in a fight (ITEMS.md 5): only the zone knows either.
        let request = (|| {
            let link = self.cfg.hub.clone().ok_or("nothing is worn in this zone")?;
            let slot = self
                .hub_slots
                .get(&id)
                .ok_or("nothing is worn in this zone")?;
            let p = self
                .zone
                .player(id)
                .filter(|p| p.alive && !p.ghost)
                .ok_or("not now")?;
            let fight = self
                .zone
                .rate
                .ms_to_ticks(self.cfg.gear_after_fight.as_millis() as u32);
            if p.fought_within(self.zone.tick, fight) {
                return Err("not in a fight: wait a moment");
            }
            Ok((link, slot.character))
        })();
        match request {
            Ok((link, character)) => {
                let tx = self.event_tx.clone();
                tokio::spawn(async move {
                    let worn = |result, tell| ClientEvent::Worn {
                        id,
                        character,
                        item,
                        result,
                        tell,
                    };
                    let asked = link.wear(character, item, on, place);
                    tokio::pin!(asked);
                    match tokio::time::timeout(HUB_PATIENCE, &mut asked).await {
                        Ok(result) => {
                            let _ = tx.send(worn(result, true)).await;
                        }
                        Err(_) => {
                            // The player is told and may ask again; what the
                            // hub answers in the end is still what the
                            // character wears, and is applied when it comes.
                            let silent = Err(HUB_SILENT.to_string());
                            let _ = tx.send(worn(silent, true)).await;
                            let late = asked.await;
                            if late.is_ok() {
                                let _ = tx.send(worn(late, false)).await;
                            }
                        }
                    }
                });
            }
            Err(why) => {
                session.cancel_gear_request();
                refuse(session, why);
            }
        }
    }

    /// What a body's worn things bring to its kit (GEAR.md 2), from the hub's six template
    /// keys: into the body at once (`Zone::set_worn`, which rearms it), and told to its
    /// client when it changed, so that both compile the one sheet. Asked at the claim,
    /// after every reading, and after a respawn applied a build (the staff's bolt is the
    /// build's choice).
    pub(super) fn refit(&mut self, body: EntityId, templates: &[String]) {
        let Some(p) = self.zone.player(body) else {
            return;
        };
        let mut worn =
            gm_content::worn::worn_of(&self.cfg.looks, &self.pack, &p.sheet.build, templates);
        // The ladder's locks and the Essence (SKILLS.md 4, 5) are the hub's reading.
        if let Some(slot) = self.hub_slots.get(&body) {
            worn.locked = slot.locked;
            if let Some(e) = &slot.essence {
                gm_content::worn::essence_of(
                    &mut worn,
                    &self.cfg.looks,
                    &self.pack,
                    templates,
                    &e.form,
                    e.in_hand,
                );
            }
        }
        // The frames' stamina (GEAR.md 6) is the hub's reading of the materials, set into
        // the body before this is asked; the client predicts with it, so it rides in `Worn`.
        worn.stamina_blows = p.gear.stamina_blows;
        worn.stamina_block = p.gear.stamina_block;
        if self.zone.set_worn(body, worn.clone()) {
            info!(body, primary = ?worn.primary, guard = ?worn.guard, "worn");
            if let Some(s) = self.sessions.get(&body) {
                s.send_control(FromZone::Worn(worn));
            }
        }
    }

    /// `ClientEvent::Worn`.
    pub(super) fn on_worn(
        &mut self,
        id: EntityId,
        character: CharacterId,
        item: i64,
        result: Result<GearReading, String>,
        tell: bool,
    ) {
        // What the hub holds is what the body has, at once: a build waits for
        // the respawn because the client predicts with it; gear it does not.
        // It goes to the body the *character* has now (it may have joined
        // again since it asked), and only if it is newer than what that has.
        if let Ok(reading) = &result {
            let here = self
                .hub_slots
                .iter()
                .find(|(_, slot)| slot.character == character)
                .map(|(body, slot)| (*body, slot.gear_seq));
            match here {
                Some((body, gear_seq)) => {
                    if reading.seq > gear_seq {
                        // What it held before the reading, then the reading
                        // (Gemini's review: measured after, the look never
                        // changed and nobody was told).
                        let before = look_of(
                            &self.cfg.looks,
                            &self.zone,
                            &self.hub_slots,
                            &self.sheaths,
                            body,
                            None,
                        );
                        if let Some(slot) = self.hub_slots.get_mut(&body) {
                            slot.gear_seq = reading.seq;
                            slot.worn = reading.templates.clone();
                            slot.stacks = reading.stacks.clone();
                            slot.bar = reading.bar.clone();
                            if slot.essence.is_none()
                                && let Some(e) = &reading.essence
                                && let Some(s) = self.sessions.get(&body)
                            {
                                let name = self.pack.find(&e.form).map_or(e.form.as_str(), |i| {
                                    self.pack.abilities[i as usize].ability.name.as_str()
                                });
                                s.send_control(FromZone::ChatFrom {
                                    from: 0,
                                    text: format!("The Essence wakes in your weapon: {name}."),
                                });
                            }
                            slot.essence = reading.essence.clone();
                            let opened = slot.locked & !reading.locked;
                            slot.locked = reading.locked;
                            if opened != 0
                                && let Some(s) = self.sessions.get(&body)
                            {
                                s.send_control(FromZone::ChatFrom {
                                    from: 0,
                                    text: opened_words(opened).into(),
                                });
                            }
                        }
                        self.zone.set_gear(body, reading.gear);
                        self.zone
                            .set_stacks(body, &stacks_of(reading), &reading.bar);
                        self.refit(body, &reading.templates);
                        info!(
                            character,
                            dealt = ?reading.gear.dealt,
                            taken = ?reading.gear.taken,
                            "gear"
                        );
                        // What it holds may have changed with it (LOOK.md 6.2).
                        let look = look_of(
                            &self.cfg.looks,
                            &self.zone,
                            &self.hub_slots,
                            &self.sheaths,
                            body,
                            None,
                        );
                        if look != before {
                            info!(character, body, held = look.held, "look");
                            for s in self.sessions.values() {
                                s.send_control(FromZone::Look { id: body, look });
                            }
                        }
                    }
                }
                None => {
                    // No body at the moment: kept for the one a join in
                    // flight is about to make.
                    self.gear_kept.retain(|_, (_, at)| at.elapsed() < GEAR_KEPT);
                    let newer = self
                        .gear_kept
                        .get(&character)
                        .is_none_or(|(known, _)| reading.seq > known.seq);
                    if newer {
                        self.gear_kept
                            .insert(character, (reading.clone(), Instant::now()));
                    }
                }
            }
        }
        if tell
            && let Some(s) = self.sessions.get_mut(&id)
            && s.end_gear_request()
        {
            s.send_control(FromZone::WearResult {
                item,
                result: result.map(|_| ()),
            });
        }
    }
}

/// What a step of the ladder opened (SKILLS.md 5), in words.
fn opened_words(cells: u8) -> &'static str {
    use gm_core::build::locked;
    if cells & locked::SPELL != 0 {
        "You have earned your weapon's spell: its key is next to your trick's."
    } else if cells & locked::TRICK != 0 {
        "You have earned your body's trick: the first key of the actives."
    } else {
        "You have earned your second move: the right button, or 2."
    }
}
