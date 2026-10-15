//! The market mirror (ECONOMY.md 7): stalls, buys, trades between two bodies here.

use super::*;

impl Run {
    /// `ClientEvent::StallsLoaded`.
    pub(super) fn on_stalls_loaded(&mut self, list: Vec<StallSummary>) {
        self.stalls = list.into_iter().map(|s| (s.id, s)).collect();
        let entries: Vec<StallEntry> = self
            .stalls
            .values()
            .filter_map(|s| stall_entry(&self.world, s, &self.revoked))
            .collect();
        info!(count = entries.len(), "market loaded");
        for s in self.sessions.values() {
            s.send_control(FromZone::Stalls(entries.clone()));
        }
    }

    /// `ClientEvent::StallOpen`.
    pub(super) fn on_stall_open(&mut self, id: EntityId) {
        let Some(session) = self.sessions.get_mut(&id) else {
            return;
        };
        if !session.begin_stall_request() {
            return;
        }
        // Only the zone knows where the player stands: it must be alive, on a
        // tile of the market, and the tile must be free (ECONOMY.md 7).
        let request = (|| {
            let link = self.cfg.hub.clone().ok_or("this zone has no market")?;
            let slot = self.hub_slots.get(&id).ok_or("this zone has no market")?;
            let p = self
                .zone
                .player(id)
                .filter(|p| p.alive && !p.ghost)
                .ok_or("you cannot open a stall now")?;
            let (x, y) = self
                .world
                .stall_grids
                .iter()
                .find_map(|g| g.tile_at(p.mover.mv.origin))
                .ok_or("stand on a market tile to open a stall")?;
            // One stall per character (ECONOMY.md 7): a keeper back after a
            // restart finds its stall standing, and is told so here rather
            // than by the hub's constraint on every ask.
            if self.stalls.values().any(|s| s.owner == slot.character) {
                return Err("you already have a stall");
            }
            if self.stalls.values().any(|s| (s.tile_x, s.tile_y) == (x, y)) {
                return Err("that tile is taken");
            }
            Ok((link, slot.character, x, y))
        })();
        match request {
            Ok((link, character, x, y)) => {
                let tx = self.event_tx.clone();
                tokio::spawn(async move {
                    let result = link.stall_open(character, x, y).await;
                    let _ = tx.send(ClientEvent::StallOpened { id, result }).await;
                });
            }
            Err(why) => {
                session.end_stall_request();
                session.send_control(FromZone::StallResult(Err(why.to_string())));
            }
        }
    }

    /// `ClientEvent::StallOpened`.
    pub(super) fn on_stall_opened(&mut self, id: EntityId, result: Result<StallSummary, String>) {
        let answer = match result {
            Ok(stall) => {
                if let Some(entry) = stall_entry(&self.world, &stall, &self.revoked) {
                    for s in self.sessions.values() {
                        s.send_control(FromZone::StallOpened(entry.clone()));
                    }
                }
                self.stalls.insert(stall.id, stall);
                Ok(())
            }
            Err(e) => Err(e),
        };
        if let Some(s) = self.sessions.get_mut(&id) {
            s.end_stall_request();
            s.send_control(FromZone::StallResult(answer));
        }
    }

    /// `ClientEvent::StallClose`.
    pub(super) fn on_stall_close(&mut self, id: EntityId) {
        let Some(session) = self.sessions.get_mut(&id) else {
            return;
        };
        if !session.begin_stall_request() {
            return;
        }
        match (self.cfg.hub.clone(), self.hub_slots.get(&id)) {
            (Some(link), Some(slot)) => {
                let character = slot.character;
                let tx = self.event_tx.clone();
                tokio::spawn(async move {
                    let result = link.stall_close(character).await;
                    let _ = tx.send(ClientEvent::StallCloseResult { id, result }).await;
                });
            }
            _ => {
                session.end_stall_request();
                session.send_control(FromZone::StallResult(Err("this zone has no market".into())));
            }
        }
    }

    /// `ClientEvent::StallCloseResult`.
    pub(super) fn on_stall_close_result(&mut self, id: EntityId, result: Result<(), String>) {
        // The stall itself goes when the hub's notice arrives.
        if let Some(s) = self.sessions.get_mut(&id) {
            s.end_stall_request();
            s.send_control(FromZone::StallResult(result));
        }
    }

    /// `ClientEvent::HubStallClosed`.
    pub(super) fn on_hub_stall_closed(&mut self, stall: i64) {
        if self.stalls.remove(&stall).is_some() {
            for s in self.sessions.values() {
                s.send_control(FromZone::StallClosed(stall));
            }
        }
    }

    /// `ClientEvent::StallBuy`.
    pub(super) fn on_stall_buy(&mut self, id: EntityId, stall: i64, listing: i64, price: i64) {
        let Some(session) = self.sessions.get_mut(&id) else {
            return;
        };
        // A buy is always answered: somebody is looking at a screen.
        let refuse = |session: &Session, why: &str| {
            session.send_control(FromZone::BuyResult {
                listing,
                result: Err(why.to_string()),
            });
        };
        if !session.begin_stall_request() {
            refuse(session, "one thing at a time: try again in a moment");
            return;
        }
        // Only the zone knows where the buyer stands: alive, in person, and at
        // that stall (ITEMS.md 5). The rest is the hub's to check.
        let request = (|| {
            let link = self.cfg.hub.clone().ok_or("this zone has no market")?;
            let slot = self.hub_slots.get(&id).ok_or("this zone has no market")?;
            let p = self
                .zone
                .player(id)
                .filter(|p| p.alive && !p.ghost)
                .ok_or("you cannot buy now")?;
            let at = self
                .stalls
                .get(&stall)
                .and_then(|s| {
                    self.world
                        .stall_grids
                        .iter()
                        .find_map(|g| g.centre(s.tile_x, s.tile_y))
                })
                .ok_or("that stall has closed")?;
            let feet = p.mover.mv.origin + Vec3::Z * Hull::Player.mins().z;
            if !stall_in_reach(at.into(), feet.into()) {
                return Err("walk up to the stall to buy");
            }
            Ok((link, slot.character))
        })();
        match request {
            Ok((link, character)) => {
                let tx = self.event_tx.clone();
                tokio::spawn(async move {
                    let asked = link.stall_buy(character, stall, listing, price);
                    let result = tokio::time::timeout(HUB_PATIENCE, asked)
                        .await
                        .unwrap_or_else(|_| Err(HUB_SILENT.to_string()));
                    let done = ClientEvent::StallBought {
                        id,
                        listing,
                        result: result.as_ref().map(|_| ()).map_err(Clone::clone),
                    };
                    let _ = tx.send(done).await;
                    // What it carries now (MODES.md 11.2): a stack bought is a
                    // reserve filled, taken as any reading of its gear is.
                    if let Ok(reading) = result {
                        let carried = ClientEvent::Worn {
                            id,
                            character,
                            item: 0,
                            result: Ok(reading),
                            tell: false,
                        };
                        let _ = tx.send(carried).await;
                    }
                });
            }
            Err(why) => {
                // The hub was not asked: this does not count against the next.
                session.cancel_stall_request();
                refuse(session, why);
            }
        }
    }

    /// `ClientEvent::StallBought`.
    pub(super) fn on_stall_bought(
        &mut self,
        id: EntityId,
        listing: i64,
        result: Result<(), String>,
    ) {
        if let Some(s) = self.sessions.get_mut(&id)
            && s.end_stall_request()
        {
            s.send_control(FromZone::BuyResult { listing, result });
        }
    }

    /// `ClientEvent::TradeAsk`: the body `id` asks `with` for a trade, or answers its asking
    /// (PARTY.md 6). Both asked: the hub opens the trade. The first to ask: the other is
    /// told, once for as long as the request waits.
    pub(super) fn on_trade_ask(&mut self, id: EntityId, with: EntityId) {
        let other_here = self.sessions.contains_key(&with);
        let Some(session) = self.sessions.get_mut(&id) else {
            return;
        };
        if !session.begin_party_request() {
            zone_line(session, "one thing at a time: try again in a moment");
            return;
        }
        let now = Instant::now();
        let lapse = Duration::from_secs(control::TRADE_ASK_SECS);
        self.trade_asks
            .retain(|_, (_, at)| now.duration_since(*at) < lapse);
        let (link, mine, theirs) = match self.trade_request(id, with, other_here) {
            Ok(r) => r,
            Err(why) => {
                if let Some(s) = self.sessions.get_mut(&id) {
                    s.cancel_party_request();
                    zone_line(s, why);
                }
                return;
            }
        };
        if self
            .trade_asks
            .get(&with)
            .is_some_and(|(whom, _)| *whom == id)
        {
            self.open_trade(id, with, link, mine, theirs);
            return;
        }
        // The first to ask. (The gate keeps its second: this reached somebody.)
        if let Some(s) = self.sessions.get_mut(&id) {
            s.end_party_request();
        }
        self.trade_asks.insert(id, (with, now));
        let name = |body: EntityId| {
            self.sessions
                .get(&body)
                .map_or_else(String::new, |s| s.name.clone())
        };
        if let Some(s) = self.sessions.get(&id) {
            zone_line(s, format!("you asked {} to trade", name(with)));
        }
        // The other is told once per pair in the time a request waits: asking A, then B,
        // then A again tells nobody twice.
        self.trade_told
            .retain(|_, at| now.duration_since(*at) < lapse);
        if !self.trade_told.contains_key(&(id, with))
            && let Some(other) = self.sessions.get(&with)
        {
            self.trade_told.insert((id, with), now);
            other.send_control(FromZone::TradeAsked { from: id });
        }
    }

    /// The zone's own part of a request to trade: two bodies standing together, alive and
    /// in person (PARTY.md 6). The rest is the hub's to check.
    fn trade_request(
        &self,
        id: EntityId,
        with: EntityId,
        other_here: bool,
    ) -> Result<(Arc<HubLink>, CharacterId, CharacterId), &'static str> {
        let link = self.cfg.hub.clone().ok_or("this zone has no market")?;
        let mine = self
            .hub_slots
            .get(&id)
            .ok_or("this zone has no market")?
            .character;
        if with == id {
            return Err("that is you");
        }
        let theirs = self
            .hub_slots
            .get(&with)
            .filter(|_| other_here)
            .ok_or("there is nobody there to trade with")?
            .character;
        let feet = |body: EntityId| {
            self.zone
                .player(body)
                .filter(|p| p.alive && !p.ghost)
                .map(|p| p.mover.mv.origin + Vec3::Z * Hull::Player.mins().z)
        };
        let (Some(a), Some(b)) = (feet(id), feet(with)) else {
            return Err("not now");
        };
        if !trade_in_reach(a.into(), b.into()) {
            return Err("walk up to them to trade");
        }
        Ok((link, mine, theirs))
    }

    /// Both asked: the hub opens the trade; its answer is a `TradeOpened`. (The pair may
    /// ask each other again once this is open.)
    fn open_trade(
        &mut self,
        id: EntityId,
        with: EntityId,
        link: Arc<HubLink>,
        mine: CharacterId,
        theirs: CharacterId,
    ) {
        self.trade_asks.remove(&with);
        self.trade_asks.remove(&id);
        self.trade_told.remove(&(id, with));
        self.trade_told.remove(&(with, id));
        let tx = self.event_tx.clone();
        let named = |body: EntityId| {
            self.sessions
                .get(&body)
                .map_or_else(String::new, |s| s.name.clone())
        };
        let names = (named(with), named(id));
        tokio::spawn(async move {
            let opened = |result, tell| ClientEvent::TradeOpened {
                a: with,
                b: id,
                names: names.clone(),
                result,
                tell,
            };
            let asked = link.trade_open(theirs, mine);
            tokio::pin!(asked);
            match tokio::time::timeout(HUB_PATIENCE, &mut asked).await {
                Ok(result) => {
                    let _ = tx.send(opened(result, true)).await;
                }
                Err(_) => {
                    // Told, and a trade the hub opened in the end is still theirs: its
                    // window comes up late (and the asker's gate, freed by the first
                    // answer, is not freed again by the second).
                    let silent = Err(HUB_SILENT.to_string());
                    let _ = tx.send(opened(silent, true)).await;
                    if let Ok(trade) = asked.await {
                        let _ = tx.send(opened(Ok(trade), false)).await;
                    }
                }
            }
        });
    }

    /// `ClientEvent::TradeOpened`.
    pub(super) fn on_trade_opened(
        &mut self,
        a: EntityId,
        b: EntityId,
        names: (String, String),
        result: Result<i64, String>,
        tell: bool,
    ) {
        if tell && let Some(s) = self.sessions.get_mut(&b) {
            s.end_party_request();
        }
        match result {
            Ok(trade) => {
                // (Whoever is still here is told, with the other's name as it
                // was: a trade the hub holds open must be seen to be closed.)
                for (to, with) in [(a, names.1), (b, names.0)] {
                    if let Some(s) = self.sessions.get(&to) {
                        s.send_control(FromZone::TradeOpened { trade, with });
                    }
                }
            }
            Err(why) => {
                for to in [a, b] {
                    if let Some(s) = self.sessions.get(&to) {
                        zone_line(s, format!("the trade did not open: {why}"));
                    }
                }
            }
        }
    }
}
