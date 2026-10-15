//! What came in since the last tick: every `ClientEvent`, one method each, by family
//! (arrivals, gear, market, social, travel, conduct); the smallest here.

use super::arrivals::Arrival;
use super::*;

impl Run {
    /// One event of the network, the hub or an earlier task of this zone.
    pub(super) fn handle_event(&mut self, ev: ClientEvent) {
        match ev {
            ClientEvent::Join {
                name,
                build,
                team,
                hub,
                conn,
                control,
                reply,
            } => self.on_join(Arrival {
                name,
                build,
                team,
                hub,
                conn,
                control,
                reply,
            }),
            ClientEvent::Order { id, slots, order } => self.on_order(id, slots, order),
            ClientEvent::HubHireEnded { hirer, hire } => self.on_hub_hire_ended(hirer, hire),
            ClientEvent::Respec { id, build } => self.on_respec(id, build),
            ClientEvent::RespecSaved { id, result } => self.on_respec_saved(id, result),
            ClientEvent::Gm { id, op } => self.on_gm(id, op),
            ClientEvent::Travel { id, zone: to_zone } => self.on_travel(id, to_zone),
            ClientEvent::TravelResult { id, result } => self.on_travel_result(id, result),
            ClientEvent::HubClaimed { character } => self.on_hub_claimed(character),
            ClientEvent::HubKick { character, reason } => self.on_hub_kick(character, reason),
            ClientEvent::LeftSaved { character } => self.on_left_saved(character),
            ClientEvent::SaveRefused { character } => self.on_save_refused(character),
            ClientEvent::HubModelRevoked { model } => self.on_hub_model_revoked(model),
            ClientEvent::StallsLoaded(list) => self.on_stalls_loaded(list),
            ClientEvent::StallOpen { id } => self.on_stall_open(id),
            ClientEvent::StallOpened { id, result } => self.on_stall_opened(id, result),
            ClientEvent::StallClose { id } => self.on_stall_close(id),
            ClientEvent::StallCloseResult { id, result } => self.on_stall_close_result(id, result),
            ClientEvent::HubStallClosed { stall } => self.on_hub_stall_closed(stall),
            ClientEvent::StallBuy {
                id,
                stall,
                listing,
                price,
            } => self.on_stall_buy(id, stall, listing, price),
            ClientEvent::StallBought {
                id,
                listing,
                result,
            } => self.on_stall_bought(id, listing, result),
            ClientEvent::Wear {
                id,
                item,
                on,
                place,
            } => self.on_wear(id, item, on, place),
            ClientEvent::Worn {
                id,
                character,
                item,
                result,
                tell,
            } => self.on_worn(id, character, item, result, tell),
            ClientEvent::Party { id, ask } => self.on_party(id, ask),
            ClientEvent::PartyAnswered {
                id,
                ask,
                result,
                tell,
            } => self.on_party_answered(id, ask, result, tell),
            ClientEvent::HubParty(news) => self.on_hub_party(news),
            ClientEvent::PartySeq { character, seq } => self.on_party_seq(character, seq),
            ClientEvent::PartyRead { character, reading } => self.on_party_read(character, reading),
            ClientEvent::HubInvited { to, from } => self.on_hub_invited(to, from),
            ClientEvent::HubDeclined { to, by } => self.on_hub_declined(to, by),
            ClientEvent::HubHeard {
                to,
                channel,
                from,
                text,
            } => self.on_hub_heard(to, channel, from, text),
            ClientEvent::Say { id, to, text } => self.on_say(id, to, text),
            ClientEvent::Said {
                id,
                whisper,
                text,
                result,
            } => self.on_said(id, whisper, text, result),
            ClientEvent::TradeAsk { id, with } => self.on_trade_ask(id, with),
            ClientEvent::TradeOpened {
                a,
                b,
                names,
                result,
                tell,
            } => self.on_trade_opened(a, b, names, result, tell),
            ClientEvent::Input { id, datagram } => self.on_input(id, datagram),
            ClientEvent::Malformed { id } => self.on_malformed(id),
            ClientEvent::Chat { id, text } => self.on_chat(id, text),
            ClientEvent::Report { id, target, reason } => self.on_report(id, target, reason),
            ClientEvent::ReportOpened { id, result } => self.on_report_opened(id, result),
            ClientEvent::Leave { id } => self.on_leave(id),
        }
    }

    /// `ClientEvent::Order`.
    pub(super) fn on_order(&mut self, id: EntityId, slots: u8, order: control::Order) {
        let Some(session) = self.sessions.get(&id) else {
            return;
        };
        let visible = |target: EntityId| session.sees(target);
        match self
            .director
            .order(&self.zone, id, slots, ai_order(order), &visible)
        {
            Ok(()) => self.report.orders += 1,
            Err(why) => {
                self.report.orders_refused += 1;
                session.send_control(FromZone::OrderRefused(why));
            }
        }
    }

    /// `ClientEvent::HubHireEnded`.
    pub(super) fn on_hub_hire_ended(&mut self, hirer: CharacterId, hire: i64) {
        // The companion leaves now, or when its encounter ends (COMPANIONS.md 3.3).
        if let Some(commander) = self.body_of(hirer)
            && let Some(m) = self
                .director
                .squad(commander)
                .iter()
                .find(|m| m.hire == Some(hire))
        {
            self.hire_expiry.insert(m.id, 0);
        }
    }

    /// `ClientEvent::Input`.
    pub(super) fn on_input(&mut self, id: EntityId, datagram: InputDatagram) {
        if let Some(s) = self.sessions.get_mut(&id) {
            s.last_input_at = self.scheduler.tick();
            s.had_input = true;
            s.on_ack(datagram.ack_tick);
            for (t, frame) in datagram.frames() {
                self.zone
                    .queue_input(id, t, frame.to_sim(), datagram.view_tick);
            }
        }
    }
}
