//! Reports and malformed datagrams (ANTICHEAT.md 5).

use super::*;

impl Run {
    /// `ClientEvent::Malformed`.
    pub(super) fn on_malformed(&mut self, id: EntityId) {
        if let Some(s) = self.sessions.get_mut(&id) {
            s.malformed += 1;
            self.report.malformed += 1;
            if s.malformed > 100 {
                s.conn.close(2, b"malformed datagrams");
            }
        }
    }

    /// `ClientEvent::Report`.
    pub(super) fn on_report(
        &mut self,
        id: EntityId,
        target: EntityId,
        reason: control::ReportReason,
    ) {
        let Some(s) = self.sessions.get_mut(&id) else {
            return;
        };
        let refuse = |s: &Session, why: &str| {
            s.send_control(FromZone::ReportResult(Err(why.to_string())));
        };
        let Some(rec) = &mut self.recorder else {
            refuse(s, "this zone keeps no replays");
            return;
        };
        // A client's body here now, or one that left within two minutes.
        self.recent_left
            .retain(|(_, _, at)| at.elapsed() < REPORTABLE_AFTER_LEAVE);
        let target_character = rec
            .entry(target)
            .filter(|e| e.human() && e.id != id)
            .map(|e| e.character)
            .or_else(|| {
                self.recent_left
                    .iter()
                    .find(|(body, _, _)| *body == target && *body != id)
                    .map(|(_, character, _)| *character)
            });
        if target_character.is_none() {
            refuse(s, "nobody to report by that id");
            return;
        }
        if !s.may_report() {
            refuse(s, "one report in thirty seconds");
            return;
        }
        if !rec.may_report() {
            refuse(s, "too many reports in this zone this hour");
            return;
        }
        match (&self.cfg.hub, target_character, self.hub_slots.get(&id)) {
            (None, Some(_), _) => {
                rec.report(self.zone.tick, None);
                info!(reporter = id, target, reason = reason.name(), "report");
                s.send_control(FromZone::ReportResult(Ok(())));
            }
            // The hub opens the report first (its limits are the hub's); the
            // replay is written when it has.
            (Some(link), Some(target_character), Some(slot)) => {
                let (link, reporter, tx) = (link.clone(), slot.character, self.event_tx.clone());
                tokio::spawn(async move {
                    let result = link.report(reporter, target_character, reason).await;
                    let _ = tx.send(ClientEvent::ReportOpened { id, result }).await;
                });
            }
            _ => refuse(s, "nobody to report by that id"),
        }
    }

    /// `ClientEvent::ReportOpened`.
    pub(super) fn on_report_opened(&mut self, id: EntityId, result: Result<i64, String>) {
        let answer = match (result, &mut self.recorder) {
            (Ok(opened), Some(rec)) => {
                rec.report(self.zone.tick, Some(opened));
                info!(reporter = id, opened, "report opened");
                Ok(())
            }
            (Ok(_), None) => Err("this zone keeps no replays".to_string()),
            (Err(why), _) => Err(why),
        };
        if let Some(s) = self.sessions.get(&id) {
            s.send_control(FromZone::ReportResult(answer));
        }
    }
}
