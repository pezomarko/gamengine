//! Parties, invitations, whispers and chat (PARTY.md).

use super::*;

impl Run {
    /// `ClientEvent::Party`.
    pub(super) fn on_party(&mut self, id: EntityId, ask: PartyAsk) {
        let Some(session) = self.sessions.get_mut(&id) else {
            return;
        };
        if !session.begin_party_request() {
            zone_line(session, "one thing at a time: try again in a moment");
            return;
        }
        // The zone's own part (PARTY.md 4): the asker is a body in person,
        // and a name is one a character can have. The rest is the hub's.
        let named = |name: &str| gm_hub_proto::names::character_name(name).is_ok();
        let request = (|| {
            let link = self.cfg.hub.clone().ok_or("this zone has no parties")?;
            let character = self
                .hub_slots
                .get(&id)
                .ok_or("this zone has no parties")?
                .character;
            if self.zone.player(id).is_none_or(|p| p.ghost) {
                return Err("not now");
            }
            let op = match &ask {
                PartyAsk::Invite(name) | PartyAsk::Remove(name) if !named(name) => {
                    return Err("nobody can be called that");
                }
                PartyAsk::Answer { from, .. } if !named(from) => {
                    return Err("nobody can be called that");
                }
                PartyAsk::Invite(name) => ZonePartyOp::Invite {
                    from: character,
                    to: name.clone(),
                },
                PartyAsk::Answer { from, join } => ZonePartyOp::Answer {
                    character,
                    from: from.clone(),
                    join: *join,
                },
                PartyAsk::Leave => ZonePartyOp::Leave { character },
                PartyAsk::Remove(name) => ZonePartyOp::Remove {
                    leader: character,
                    name: name.clone(),
                },
            };
            Ok((link, op))
        })();
        match request {
            Ok((link, op)) => {
                let tx = self.event_tx.clone();
                tokio::spawn(async move {
                    let answered = |result, tell| ClientEvent::PartyAnswered {
                        id,
                        ask: ask.clone(),
                        result,
                        tell,
                    };
                    let asked = link.party(op);
                    tokio::pin!(asked);
                    match tokio::time::timeout(HUB_PATIENCE, &mut asked).await {
                        Ok(result) => {
                            let _ = tx.send(answered(result, true)).await;
                        }
                        Err(_) => {
                            // As with gear: the player is told and may ask
                            // again, and what the hub did in the end is
                            // still what the party is (this zone is not
                            // told of its own change in any other way).
                            let silent = Err(HUB_SILENT.to_string());
                            let _ = tx.send(answered(silent, true)).await;
                            let late = asked.await;
                            if late.is_ok() {
                                let _ = tx.send(answered(late, false)).await;
                            }
                        }
                    }
                });
            }
            Err(why) => {
                session.cancel_party_request();
                zone_line(session, why);
            }
        }
    }

    /// `ClientEvent::PartyAnswered`.
    pub(super) fn on_party_answered(
        &mut self,
        id: EntityId,
        ask: PartyAsk,
        result: Result<PartyReply, String>,
        tell: bool,
    ) {
        if let Ok(PartyReply::News(news)) = &result {
            party_news(&mut self.parties, news, &self.hub_slots, &self.sessions);
        }
        if tell
            && let Some(s) = self.sessions.get_mut(&id)
            && s.end_party_request()
        {
            let line = match (ask, result) {
                (_, Err(why)) => why,
                (_, Ok(PartyReply::Invited { name })) => {
                    format!("{name} was asked to join")
                }
                (_, Ok(PartyReply::Declined)) => "declined".to_string(),
                (PartyAsk::Remove(name), Ok(_)) => {
                    format!("{name} is out of the party")
                }
                // The news said the rest to everybody it concerns.
                (_, Ok(_)) => return,
            };
            zone_line(s, line);
        }
    }

    /// `ClientEvent::HubParty`.
    pub(super) fn on_hub_party(&mut self, news: PartyNews) {
        party_news(&mut self.parties, &news, &self.hub_slots, &self.sessions);
    }

    /// `ClientEvent::PartySeq`.
    pub(super) fn on_party_seq(&mut self, character: CharacterId, seq: u64) {
        // A save's answer: the zone is behind (a notice was lost) when the
        // hub's number is larger than its own. It asks, once.
        let Some(link) = self.cfg.hub.clone() else {
            return;
        };
        if self.parties.behind(character, seq) {
            let tx = self.event_tx.clone();
            tokio::spawn(async move {
                let op = ZonePartyOp::Read { character };
                if let Ok(PartyReply::Reading(reading)) = link.party(op).await {
                    let _ = tx.send(ClientEvent::PartyRead { character, reading }).await;
                }
            });
        }
    }

    /// `ClientEvent::PartyRead`.
    pub(super) fn on_party_read(&mut self, character: CharacterId, reading: PartyReading) {
        if self.parties.read(character, reading)
            && let Some(s) = session_of(&self.hub_slots, &self.sessions, character)
        {
            warn!(character, "a party's news had been missed: mended");
            s.send_control(FromZone::Party(self.parties.names(character)));
        }
    }

    /// `ClientEvent::HubInvited`.
    pub(super) fn on_hub_invited(&mut self, to: CharacterId, from: String) {
        match session_of(&self.hub_slots, &self.sessions, to) {
            Some(s) => {
                s.send_control(FromZone::Invited { from });
            }
            // On its way here: kept for the body (a minute, as the
            // invitation itself; a few hundred at most).
            None => {
                self.invites_kept
                    .retain(|(_, _, at)| at.elapsed() < INVITE_KEPT);
                if self.invites_kept.len() < 512 {
                    self.invites_kept.push((to, from, Instant::now()));
                }
            }
        }
    }

    /// `ClientEvent::HubDeclined`.
    pub(super) fn on_hub_declined(&mut self, to: CharacterId, by: String) {
        if let Some(s) = session_of(&self.hub_slots, &self.sessions, to) {
            zone_line(s, format!("{by} declined"));
        }
    }

    /// `ClientEvent::HubHeard`.
    pub(super) fn on_hub_heard(
        &mut self,
        to: Vec<CharacterId>,
        channel: u8,
        from: String,
        text: String,
    ) {
        for character in to {
            if let Some(s) = session_of(&self.hub_slots, &self.sessions, character) {
                // A line may be lost; nobody is disconnected over one.
                s.send_droppable(FromZone::Heard {
                    channel,
                    from: from.clone(),
                    text: text.clone(),
                });
            }
        }
    }

    /// `ClientEvent::Say`.
    pub(super) fn on_say(&mut self, id: EntityId, to: SayTo, text: String) {
        // The connection checked the line and its sender's rate (`net.rs`).
        // A party's line and a whisper go through the hub, whoever hears
        // them (PARTY.md 5).
        let Some(session) = self.sessions.get(&id) else {
            return;
        };
        let (Some(link), Some(slot)) = (self.cfg.hub.clone(), self.hub_slots.get(&id)) else {
            session.send_droppable(FromZone::ChatFrom {
                from: 0,
                text: "this zone has no parties".into(),
            });
            return;
        };
        let (tx, from) = (self.event_tx.clone(), slot.character);
        tokio::spawn(async move {
            let whisper = matches!(to, SayTo::Whisper(_));
            let op = ZonePartyOp::Say {
                from,
                to,
                text: text.clone(),
            };
            let result = match tokio::time::timeout(HUB_PATIENCE, link.party(op)).await {
                Ok(Ok(PartyReply::Said { to })) => Ok(to),
                Ok(Ok(_)) => Err("the hub could not be asked: try again".to_string()),
                Ok(Err(why)) => Err(why),
                Err(_) => Err(HUB_SILENT.to_string()),
            };
            let said = ClientEvent::Said {
                id,
                whisper,
                text,
                result,
            };
            let _ = tx.send(said).await;
        });
    }

    /// `ClientEvent::Said`.
    pub(super) fn on_said(
        &mut self,
        id: EntityId,
        whisper: bool,
        text: String,
        result: Result<String, String>,
    ) {
        let Some(s) = self.sessions.get(&id) else {
            return;
        };
        match result {
            // A whisper is answered to the whisperer as it went out; a
            // party's line comes back with everybody else's.
            Ok(to) if whisper => {
                self.report.chat_lines += 1;
                s.send_droppable(FromZone::Heard {
                    channel: control::CHANNEL_WHISPERED,
                    from: to,
                    text,
                });
            }
            Ok(_) => self.report.chat_lines += 1,
            Err(why) => {
                s.send_droppable(FromZone::ChatFrom { from: 0, text: why });
            }
        }
    }

    /// `ClientEvent::Chat`.
    pub(super) fn on_chat(&mut self, id: EntityId, text: String) {
        // The connection checked the line and its sender's rate (`net.rs`);
        // here is the zone's own ceiling: everybody hears everybody, and a
        // crowd of talkers must not become everybody's traffic.
        if !self.sessions.contains_key(&id) {
            return;
        }
        let now = Instant::now();
        let back = now.duration_since(self.chat_at).as_secs_f32() * ZONE_CHAT_PER_SEC;
        self.chat_lines = (self.chat_lines + back).min(ZONE_CHAT_BURST);
        self.chat_at = now;
        if self.chat_lines < 1.0 {
            self.report.chat_dropped += 1;
            // Its sender is told (their own bucket bounds how often).
            if let Some(s) = self.sessions.get(&id) {
                s.send_droppable(FromZone::ChatFrom {
                    from: 0,
                    text: "too many are talking: that line was not heard".into(),
                });
            }
            return;
        }
        self.chat_lines -= 1.0;
        self.report.chat_lines += 1;
        for s in self.sessions.values() {
            // A chat line may be lost; nobody is disconnected over one.
            s.send_droppable(FromZone::ChatFrom {
                from: id,
                text: text.clone(),
            });
        }
    }
}
