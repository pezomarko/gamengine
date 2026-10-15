//! Handoffs to another zone (HUB.md 3.3).

use super::*;

impl Run {
    /// `ClientEvent::Travel`.
    pub(super) fn on_travel(&mut self, id: EntityId, to_zone: String) {
        let Some(link) = self.cfg.hub.clone() else {
            if let Some(s) = self.sessions.get(&id) {
                s.send_control(FromZone::TravelRefused("no hub".into()));
            }
            return;
        };
        let Some(slot) = self.hub_slots.get(&id) else {
            return;
        };
        if slot.ghost_since.is_some() {
            return;
        }
        let Some(state) = character_state(&self.zone, &link, id, slot) else {
            return;
        };
        // One handoff request per player at the hub, and no more than one a
        // second: the rest of a flood is dropped here.
        if !self
            .sessions
            .get_mut(&id)
            .is_some_and(|s| s.begin_travel_request())
        {
            return;
        }
        let character = slot.character;
        let web = self.sessions.get(&id).is_some_and(|s| s.conn.is_web());
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            let result = link.handoff(character, state, to_zone, web).await;
            let _ = tx.send(ClientEvent::TravelResult { id, result }).await;
        });
    }

    /// `ClientEvent::TravelResult`.
    pub(super) fn on_travel_result(&mut self, id: EntityId, result: Result<ZoneTicket, String>) {
        let Some(s) = self.sessions.get_mut(&id) else {
            return;
        };
        s.end_travel_request();
        match result {
            Ok(ticket) => {
                s.send_control(FromZone::TravelTicket {
                    zone: ticket.zone,
                    addr: ticket.addr.to_string(),
                    cert_der: ticket.cert_der,
                    token: bitcode::encode(&ticket.token),
                    web: ticket.web,
                });
                self.zone.set_ghost(id, true);
                self.director.human_left(&mut self.zone, id);
                if let Some(slot) = self.hub_slots.get_mut(&id) {
                    slot.ghost_since = Some(Instant::now());
                }
            }
            Err(e) => {
                s.send_control(FromZone::TravelRefused(e));
            }
        }
    }
}
