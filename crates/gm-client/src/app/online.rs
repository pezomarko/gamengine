use super::*;

/// Networked play: the zone connection plus the shared prediction state.
pub(crate) struct Online {
    pub(super) net: NetClient,
    /// `Welcome` arrived; the client state is built when `Content` follows.
    pub(super) welcome: Option<(u32, u16)>,
    pub(super) client: Option<ClientState>,
    pub(super) pack: Option<ContentPack>,
    pub(super) team: u8,
    pub(super) build_name: String,
    pub(super) accumulator: f32,
    pub(super) prev_origin: Vec3,
    pub(super) curr_origin: Vec3,
    pub(super) last_snapshot: Instant,
    pub(super) rate: TickRate,
    /// Everyone in the zone: name, team, avatar model.
    pub(super) names: HashMap<u32, (String, u8, Option<ModelId>)>,
    /// What each body holds (LOOK.md 6.2), by the pack's prop list.
    pub(super) looks: HashMap<u32, Look>,
    /// The prop keys the zone's content names: what a `Look` indexes.
    pub(super) props: Vec<String>,
    /// Who drives each body (COMPANIONS.md 2.1), as the zone announced it.
    pub(super) kinds: HashMap<u32, BodyKind>,
    /// The own squad, in slot order, as the zone last told it.
    pub(super) squad: Vec<SquadEntry>,
    /// Encounter, loot, trial and order messages: when, what, in which colour.
    pub(super) messages: VecDeque<(Instant, String, [f32; 4])>,
    /// The market: open stalls and their keepers (ECONOMY.md 7).
    pub(super) stalls: Vec<StallEntry>,
    /// The zone granted the game master's page (GM.md 1), and its tuning as last told.
    pub(super) gm: bool,
    pub(super) tuning: gm_core::tuning::Tuning,
    pub(super) gm_note: String,
    /// The party as the hub has it, and who asked what (PARTY.md 4).
    pub(super) social: Social,
    pub(super) kills: u32,
    pub(super) deaths: u32,
    /// Blows the own hand landed, as the zone said (target, amount, absorbed): the
    /// numbers the next frame floats over the bodies it finds (LOOK.md 13.8).
    /// And what the own hand's Regen gave other bodies back (target, amount).
    pub(super) heals: Vec<(u32, u16)>,
    /// The hash of the map this connection plays on: the one loaded when it began, and
    /// from the zone's `Welcome` on the zone's (once that map is here).
    pub(super) map_hash: u64,
    pub(super) respec_note: String,
    /// Why the last press of an item key used nothing, and when (MODES.md 11.3): the HUD
    /// says it in yellow for `KIT_NOTE_SECS` over the item cells.
    pub(super) kit_note: Option<(Instant, &'static str)>,
    /// The item bar (LOOK.md 3.2): the template on each cell, as the hub holds it; asked
    /// when the zone's content arrives and whenever the inventory changed it.
    pub(super) bar: Vec<Option<String>>,
    pub(super) bar_wanted: bool,
    pub(super) bar_ask: Option<crate::hub::Pending<crate::hub::Answer>>,
    /// A travel ticket to act on: reconnect to another zone, reloading its map.
    pub(super) pending_travel: Option<(String, ZoneAddr, Vec<u8>)>,
    pub(super) zone_name: String,
    /// Events taken from the connection and not handled yet: while the zone's map is
    /// still on its way (a browser fetches it), everything after `Welcome` waits here.
    pub(super) backlog: VecDeque<NetEvent>,
    /// The zone's map is being fetched (a browser): its hash. It is the connection's
    /// map only once it has arrived.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(super) awaiting_map: Option<u64>,
}

/// How long a message stays on the HUD, and how many are kept.
pub(super) const MESSAGE_SECS: f32 = 9.0;

pub(super) const MESSAGES_KEPT: usize = 6;

impl Online {
    pub(super) fn say(&mut self, text: String, colour: [f32; 4]) {
        log::info!("{text}");
        self.messages.push_back((Instant::now(), text, colour));
        while self.messages.len() > MESSAGES_KEPT {
            self.messages.pop_front();
        }
    }

    /// Somebody nothing hurts stands in this zone: a trainer, where a build is worn at
    /// once (MATRIX.md 9.1). The client is not told whether a zone is of the world: one
    /// without a trainer (the arena) wears a build at the next respawn; the dungeon has
    /// neither, and the zone says so when asked.
    pub(super) fn has_trainer(&self) -> bool {
        let Some(pack) = &self.pack else {
            return false;
        };
        self.kinds.values().any(|k| {
            matches!(k, BodyKind::Creature { def }
                if pack.creatures.get(*def as usize).is_some_and(|d| d.npc))
        })
    }

    pub(super) fn build_name_of(
        &self,
        pack: &ContentPack,
        build: &gm_core::build::Build,
    ) -> String {
        pack.builds
            .iter()
            .find(|b| &b.build == build)
            .map_or_else(|| "custom".to_string(), |b| b.name.clone())
    }
}

/// How the client reaches its zone: directly, or with a ticket from the hub.
pub(super) struct Entry {
    pub(super) zone: ZoneAddr,
    pub(super) token: Vec<u8>,
    pub(super) zone_name: String,
}

pub(super) fn online(
    opts: &Options,
    sim: &Sim,
    map_hash: u64,
    entry: Entry,
) -> Result<Online, Error> {
    log::info!("connecting to the zone as {}", opts.name);
    Ok(Online {
        net: NetClient::connect(
            entry.zone,
            opts.name.clone(),
            opts.build.clone(),
            opts.team,
            entry.token,
        )?,
        welcome: None,
        client: None,
        pack: None,
        team: 0,
        build_name: String::new(),
        accumulator: 0.0,
        prev_origin: sim.curr.origin,
        curr_origin: sim.curr.origin,
        last_snapshot: Instant::now(),
        rate: TickRate::COMBAT,
        names: HashMap::new(),
        looks: HashMap::new(),
        props: Vec::new(),
        kinds: HashMap::new(),
        squad: Vec::new(),
        social: Social::default(),
        messages: VecDeque::new(),
        stalls: Vec::new(),
        gm: false,
        tuning: Default::default(),
        gm_note: String::new(),
        kills: 0,
        deaths: 0,
        heals: Vec::new(),
        map_hash,
        respec_note: String::new(),
        kit_note: None,
        bar: vec![None; gm_core::sim::BAR_CELLS],
        bar_wanted: false,
        bar_ask: None,
        pending_travel: None,
        zone_name: entry.zone_name,
        backlog: VecDeque::new(),
        awaiting_map: None,
    })
}

/// How a step of the online frame ends when the frame cannot go on: with the camera
/// where the local walk left it (the zone is between maps, or gone and the screens come
/// back), or with nothing (the program is ending).
enum Cut {
    Here,
    End,
}

/// What the prediction of a frame found (`App::predict`): the own body's actions by its
/// own ticks and the ground it covered (for the sound and the view model), the mode, the
/// other bodies as the RPG mode reads them, and the tick's length.
struct Predicted {
    own_actions: Vec<gm_core::sim::Action>,
    own_travel: f32,
    rpg: bool,
    rpg_bodies: Vec<crate::rpg::Body>,
    scripted: bool,
    dt: f32,
}

/// Where the frame is seen from (`App::seen`): the own hull's drawn origin, the eye over
/// it, the camera of the viewport, the interpolation tick and the other entities at it.
struct Seen {
    centre: Vec3,
    eye: Vec3,
    camera: Vec3,
    t: f32,
    others: Vec<gm_net::client::RenderEntity>,
    my_team: u8,
    viewport: Viewport,
}

impl App {
    /// The zone is left (the person chose to, or its connection ended): the characters are
    /// shown again, with the reason if there is one.
    pub(super) fn leave_zone(&mut self, why: &str) {
        self.hang_up();
        self.sound.quiet();
        self.screens.menu = None;
        self.screens.bag = None;
        self.screens.people = None;
        self.screens.gm_page = None;
        self.session.character = None;
        self.screens.chat.clear();
        self.scene.entities.clear();
        self.scene.bodies.clear();
        self.scene.squad_view.clear();
        self.scene.party_view.clear();
        self.scene.target_view = None;
        if let Some(front) = &mut self.screens.front {
            front.back_to_characters(why);
            self.screens.front_up = true;
        }
    }

    /// Hang up on the zone without waiting for it: the goodbye is said on the connection's
    /// own thread while the frames go on.
    pub(super) fn hang_up(&mut self) {
        // (The map that is loaded is whatever `switch_map` last loaded: a zone that was
        // left while its map was still on the way has not changed it.)
        if let Some(mut o) = self.online.take() {
            o.net.hang_up();
            self.session.leaving.push((o.net, Instant::now()));
        }
        // A map still being fetched for that zone arrives in a slot nobody reads.
        #[cfg(target_arch = "wasm32")]
        {
            self.web.pending_fetch = Default::default();
        }
    }

    /// The hub gave a ticket: connect to its zone as the character it is for.
    pub(super) fn enter_zone(
        &mut self,
        ticket: gm_hub_proto::protocol::ZoneTicket,
        name: String,
        event_loop: &ActiveEventLoop,
    ) {
        log::info!("hub ticket for zone {} at {}", ticket.zone, ticket.addr);
        let reachable = if cfg!(target_arch = "wasm32") {
            ticket.web.is_some()
        } else {
            true
        };
        let entered = if reachable {
            self.opts.name = name.clone();
            let entry = Entry {
                zone: ticket_addr(&ticket),
                token: bitcode::encode(&ticket.token),
                zone_name: ticket.zone.clone(),
            };
            // What will be loaded when the zone answers: a map already on its way in.
            let loaded = self
                .world
                .pending_map
                .as_ref()
                .map_or(self.world.map_hash, |m| m.1);
            online(&self.opts, &self.sim, loaded, entry).map_err(|e| e.to_string())
        } else {
            Err(format!("zone {} has no web listener", ticket.zone))
        };
        match entered {
            Ok(online) => {
                self.online = Some(online);
                self.session.character = Some(ticket.token.payload.character);
                // Remembered for a person; a command line that named it leaves no trace.
                if self.returns_to_screens() {
                    self.settings.character = name;
                }
            }
            Err(why) if self.returns_to_screens() => {
                if let Some(front) = &mut self.screens.front {
                    front.back_to_characters(&why);
                }
            }
            Err(why) => self.fail(event_loop, &why),
        }
    }

    /// F1–F4 ask the zone for the preset builds, applied at the next respawn.
    pub(super) fn respec_hotkeys(&mut self) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        let Some(pack) = &o.pack else { return };
        // T asks to travel to the zone named on the command line; a scripted run asks by
        // itself, once, after `travel_after` seconds.
        let timed = self.opts.travel_after > 0.0
            && self.clock.started.elapsed().as_secs_f32() >= self.opts.travel_after;
        if (self.input.just_pressed.remove(&KeyCode::KeyT) || timed)
            && let Some(target) = self.opts.travel_to.clone()
        {
            if timed {
                self.opts.travel_after = 0.0;
            }
            o.net.send_control(FromClient::Travel(target.clone()));
            o.respec_note = format!("travel to {target} requested");
            log::info!("{}", o.respec_note);
        }
        // F9 reports the player nearest the crosshair (ANTICHEAT.md 5): the zone keeps the
        // last half minute and the next ten seconds for a moderator.
        if self.input.just_pressed.remove(&KeyCode::F9)
            && let Some(c) = &o.client
        {
            let eye = c.mover.eye();
            let view = view_dir(self.sim.yaw, self.sim.pitch);
            let t = c.render_tick(0.0);
            let aimed = c
                .others_at(t)
                .into_iter()
                .filter(|e| e.kind == EntityKind::Player)
                .filter(|e| matches!(o.kinds.get(&e.id), Some(BodyKind::Human) | None))
                .map(|e| (e.id, view.dot((e.pos - eye).normalize_or_zero())))
                .filter(|(_, facing)| *facing > 0.94)
                .max_by(|a, b| a.1.total_cmp(&b.1));
            match aimed {
                Some((target, _)) => {
                    o.net.send_control(FromClient::Report {
                        target,
                        reason: gm_net::control::ReportReason::Other,
                    });
                    o.respec_note = "report sent".into();
                }
                None => o.respec_note = "look at the player to report, then F9".into(),
            }
        }
        // B opens a stall on the market tile underfoot, N closes the own stall.
        if self.input.just_pressed.remove(&KeyCode::KeyB) {
            o.net.send_control(FromClient::StallOpen);
            o.respec_note = "stall requested".into();
        }
        if self.input.just_pressed.remove(&KeyCode::KeyN) {
            o.net.send_control(FromClient::StallClose);
            o.respec_note = "closing the stall".into();
        }
        let keys = [KeyCode::F1, KeyCode::F2, KeyCode::F3, KeyCode::F4];
        for (i, k) in keys.iter().enumerate() {
            if self.input.just_pressed.remove(k)
                && let Some(b) = pack.builds.get(i)
            {
                o.net
                    .send_control(FromClient::Respec(BuildChoice::Preset(b.name.clone())));
                o.respec_note = format!("respec {} requested", b.name);
            }
        }
    }

    /// Network events, prediction ticks and the entity list for this frame, as named
    /// steps (CLIENT.md 2.1). Returns the camera `(position, yaw, pitch)`; `None` when the
    /// program is ending.
    pub(super) fn online_frame(
        &mut self,
        frame_dt: f32,
        event_loop: &ActiveEventLoop,
    ) -> Option<(Vec3, f32, f32)> {
        self.respec_hotkeys();
        self.viewport_of_mode();
        let o = self.online.as_mut()?;
        o.backlog.extend(o.net.poll());
        #[cfg(target_arch = "wasm32")]
        if let Err(cut) = self.await_zone_map(event_loop) {
            return self.cut(cut);
        }
        if let Err(cut) = self.zone_events(event_loop) {
            return self.cut(cut);
        }
        if self.take_travel() {
            return Some(self.here());
        }
        let Some(predicted) = self.predict(frame_dt) else {
            return Some(self.here());
        };
        let seen = self.seen(frame_dt, &predicted);
        self.script_target(&predicted, &seen);
        self.hud_views(&seen);
        self.others_in_sight(frame_dt, &predicted, &seen);
        self.hear(frame_dt, &predicted, &seen);
        self.stalls_in_sight(&seen);
        let own_anim = self.own_stance(&predicted, &seen);
        self.own_launches(&predicted, &seen);
        self.marks(&predicted, &seen);
        Some(self.own_view(frame_dt, &predicted, &seen, own_anim))
    }

    /// The camera where the local walk left it: what a frame between zones shows.
    fn here(&self) -> (Vec3, f32, f32) {
        (self.sim.eye(), self.sim.yaw, self.sim.pitch)
    }

    /// What a cut step leaves the frame with.
    fn cut(&self, cut: Cut) -> Option<(Vec3, f32, f32)> {
        match cut {
            Cut::Here => Some(self.here()),
            Cut::End => None,
        }
    }

    /// The camera is the character's mode's (MODES.md 2).
    fn viewport_of_mode(&mut self) {
        if let Some(c) = self.online.as_ref().and_then(|o| o.client.as_ref()) {
            self.view.viewport = if c.sheet.kit.mode.third_person() {
                Viewport::Third
            } else {
                Viewport::First
            };
        }
    }

    /// The zone's map is still being fetched (a browser): what the zone sent after its
    /// `Welcome` waits, except snapshots, which would be stale by then anyway.
    #[cfg(target_arch = "wasm32")]
    fn await_zone_map(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Cut> {
        let returns = self.returns_to_screens();
        let Some(o) = self.online.as_mut() else {
            return Ok(());
        };
        let Some(wanted) = o.awaiting_map else {
            return Ok(());
        };
        // The zone hung up meanwhile: say so now, not after a map nobody needs.
        if let Some(NetEvent::Disconnected(reason)) = o
            .backlog
            .iter()
            .find(|e| matches!(e, NetEvent::Disconnected(_)))
        {
            let why = format!("disconnected: {reason}");
            if returns {
                self.session.zone_ended = Some(why);
                return Err(Cut::Here);
            }
            self.fail(event_loop, &why);
            return Err(Cut::End);
        }
        match self.web.pending_fetch.borrow_mut().take() {
            None => {
                o.backlog.retain(|e| !matches!(e, NetEvent::Snapshot(_)));
                Err(Cut::Here)
            }
            Some(Ok((map, hash))) if hash == wanted => {
                o.awaiting_map = None;
                o.map_hash = hash;
                crate::web::tell_page("status", "");
                self.world.pending_map = Some((map, hash));
                // The world is replaced at the start of the next frame.
                Err(Cut::Here)
            }
            Some(result) => {
                let why = match result {
                    Err(e) => e,
                    Ok(_) => "this site's copy of the zone's map is another build".into(),
                };
                log::error!("{why}");
                if returns {
                    self.session.zone_ended = Some(why);
                    return Err(Cut::Here);
                }
                crate::web::tell_page("error", &why);
                self.clock.exit_requested = true;
                event_loop.exit();
                Err(Cut::End)
            }
        }
    }

    /// Everything the zone sent since the last frame, in order.
    fn zone_events(&mut self, event_loop: &ActiveEventLoop) -> Result<(), Cut> {
        let returns = self.returns_to_screens();
        while let Some(ev) = self.online.as_mut().and_then(|o| o.backlog.pop_front()) {
            match ev {
                NetEvent::Welcome {
                    entity,
                    hz,
                    map,
                    map_hash,
                } => self.on_welcome(entity, hz, map, map_hash, returns, event_loop)?,
                NetEvent::Snapshot(bytes) => self.on_snapshot(&bytes),
                NetEvent::Control(msg) => self.on_zone_message(msg),
                NetEvent::Disconnected(reason) => {
                    if returns {
                        log::info!("disconnected: {reason}");
                        self.session.zone_ended = Some(reason);
                        return Err(Cut::Here);
                    }
                    log::error!("disconnected: {reason}");
                    #[cfg(target_arch = "wasm32")]
                    crate::web::tell_page("error", &format!("disconnected: {reason}"));
                    self.clock.exit_requested = true;
                    event_loop.exit();
                    return Err(Cut::End);
                }
            }
        }
        Ok(())
    }

    /// The zone took the character: its tick rate, its map (loaded or fetched when it is
    /// another than the one here), its air.
    fn on_welcome(
        &mut self,
        entity: u32,
        hz: u16,
        map: String,
        map_hash: u64,
        returns: bool,
        event_loop: &ActiveEventLoop,
    ) -> Result<(), Cut> {
        let Some(o) = self.online.as_mut() else {
            return Ok(());
        };
        o.rate = TickRate::new(hz as u32);
        o.welcome = Some((entity, hz));
        log::info!("joined as entity {entity} on {map} at {hz} Hz");
        // The air of the map, and nothing read from the last zone's frame. A map already
        // loaded says its own air now; another map says it as it is switched to
        // (`switch_map`).
        self.sound.quiet();
        self.sound.set_rate(o.rate.dt());
        self.sound.air(&map);
        if map_hash == o.map_hash
            && let Some(air) = ambience_of(&self.bsp)
        {
            self.sound.air_named(air);
        }
        #[cfg(target_arch = "wasm32")]
        crate::web::tell_page("status", "");
        // The name becomes a file name or a URL: a zone does not choose paths.
        if !valid_map_name(&map) {
            let why = format!("the zone named a map this client will not load: {map:?}");
            if returns {
                self.session.zone_ended = Some(why);
                return Err(Cut::Here);
            }
            self.fail(event_loop, &why);
            return Err(Cut::End);
        }
        if map_hash == o.map_hash {
            return Ok(());
        }
        // Another map: load it (a zone change through the hub lands here).
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = self.opts.maps_dir.join(format!("{map}.bsp"));
            let loaded = std::fs::read(&path)
                .ok()
                .filter(|bytes| fnv1a64(bytes) == map_hash)
                .and_then(|_| Bsp::load(&path).ok());
            match loaded {
                Some(bsp) => {
                    log::info!("zone runs map {map}; loading {}", path.display());
                    self.world.pending_map = Some((bsp, map_hash));
                    o.map_hash = map_hash;
                    Ok(())
                }
                None => {
                    log::error!(
                        "zone runs map {map} with hash {map_hash:016x}; ours is {:016x} and {} does not match (wrong map build)",
                        o.map_hash,
                        path.display()
                    );
                    if returns {
                        self.session.zone_ended =
                            Some(format!("this client does not have the zone's map ({map})"));
                        return Err(Cut::Here);
                    }
                    self.clock.exit_requested = true;
                    event_loop.exit();
                    Err(Cut::End)
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            log::info!("zone runs map {map}; fetching it");
            crate::web::tell_page("status", "loading the zone's map");
            o.awaiting_map = Some(map_hash);
            let (slot, assets) = (self.web.pending_fetch.clone(), self.opts.assets.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let fetched = fetch_map(&assets, &map, Some(map_hash)).await;
                *slot.borrow_mut() = Some(fetched);
            });
            Err(Cut::Here)
        }
    }

    /// A snapshot of the zone's: the prediction reconciles with it.
    fn on_snapshot(&mut self, bytes: &[u8]) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        if let Some(c) = &mut o.client {
            match c.on_snapshot(&self.bsp, bytes) {
                Ok(()) => o.last_snapshot = Instant::now(),
                Err(e) => log::debug!("snapshot dropped: {e}"),
            }
        }
    }

    /// A message of the zone's on the control stream, by what it is about.
    fn on_zone_message(&mut self, msg: FromZone) {
        match msg {
            FromZone::Content {
                pack,
                own,
                team,
                props,
            } => self.on_content(pack, own, team, props),
            FromZone::Gm(news) => self.on_gm_news(news),
            FromZone::BuildApplied(build) => self.on_build_applied(build),
            FromZone::RespecResult(result) => self.on_respec_result(result),
            FromZone::TravelTicket {
                zone,
                addr,
                cert_der,
                token,
                web,
            } => self.on_travel_ticket(zone, addr, cert_der, token, web),
            FromZone::TravelRefused(reason) => self.zone_noted(format!("travel refused: {reason}")),
            FromZone::Roster(players) => self.on_roster(players),
            FromZone::PlayerInfo {
                id,
                name,
                team,
                model,
                kind,
                look,
            } => self.on_player_info(id, name, team, model, kind, look),
            FromZone::Look { id, look } => {
                if let Some(o) = self.online.as_mut() {
                    o.looks.insert(id, look);
                }
            }
            FromZone::Squad(entries) => {
                if let Some(o) = self.online.as_mut() {
                    o.squad = entries;
                }
            }
            FromZone::OrderRefused(why) => {
                self.zone_said(format!("order refused: {why}"), hud::ORANGE)
            }
            FromZone::Encounter { name, state } => self.on_encounter(name, state),
            FromZone::Loot {
                encounter,
                items,
                coin,
            } => self.on_loot(encounter, items, coin),
            FromZone::Trial {
                name,
                passed,
                detail,
                ..
            } => self.on_trial(name, passed, detail),
            FromZone::ModelRevoked(id) => self.on_model_revoked(id),
            FromZone::Stalls(stalls) => {
                if let Some(o) = self.online.as_mut() {
                    o.stalls = stalls;
                }
            }
            FromZone::StallOpened(stall) => self.on_stall_opened(stall),
            FromZone::StallClosed(id) => {
                if let Some(o) = self.online.as_mut() {
                    o.stalls.retain(|s| s.id != id);
                }
            }
            FromZone::StallResult(result) => self.on_stall_result(result),
            FromZone::BuyResult { listing, result } => self.on_buy_result(listing, result),
            FromZone::WearResult { item, result } => self.on_wear_result(item, result),
            FromZone::PlayerLeft(id) => {
                if let Some(o) = self.online.as_mut() {
                    o.names.remove(&id);
                    o.kinds.remove(&id);
                }
            }
            FromZone::Party(names) => self.on_party(names),
            FromZone::Invited { from } => self.on_invited(from),
            FromZone::Heard {
                channel,
                from,
                text,
            } => self.on_heard(channel, from, text),
            FromZone::TradeAsked { from } => self.on_trade_asked(from),
            FromZone::TradeOpened { trade, with } => self.on_trade_opened(trade, with),
            FromZone::Hit {
                target,
                amount,
                absorbed,
                at,
            } => self.on_hit(target, amount, absorbed, at),
            FromZone::Healed { target, amount } => {
                if let Some(o) = self.online.as_mut() {
                    o.heals.push((target, amount));
                }
            }
            FromZone::Impact { at, normal } => {
                self.scene
                    .effects
                    .impact(Vec3::from_array(at), Vec3::from_array(normal));
            }
            FromZone::Killed { victim, killer } => self.on_killed(victim, killer),
            FromZone::ReportResult(result) => self.on_report_result(result),
            FromZone::ChatFrom { from, text } => self.on_chat_from(from, text),
            // (Not printed: the words for every message of the zone's would be
            // carried by every browser for this one line.)
            _ => log::debug!("a message of the zone's this client has no use for"),
        }
    }

    /// A line of the zone's on the HUD (COMPANIONS.md 6).
    fn zone_said(&mut self, text: String, colour: [f32; 4]) {
        if let Some(o) = self.online.as_mut() {
            o.say(text, colour);
        }
    }

    /// A refusal of the zone's: the window's note, the log, and the HUD in orange.
    fn zone_noted(&mut self, text: String) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        o.respec_note = text;
        log::info!("{}", o.respec_note);
        o.say(o.respec_note.clone(), hud::ORANGE);
    }

    /// Everyone in the zone, as the zone lists them at the start.
    fn on_roster(&mut self, players: Vec<gm_net::control::PlayerEntry>) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        o.names.clear();
        o.kinds.clear();
        o.looks.clear();
        for p in players {
            o.kinds.insert(p.id, p.kind);
            o.names.insert(p.id, (p.name, p.team, p.model));
            o.looks.insert(p.id, p.look);
        }
    }

    /// Somebody came in, or changed.
    fn on_player_info(
        &mut self,
        id: u32,
        name: String,
        team: u8,
        model: Option<ModelId>,
        kind: BodyKind,
        look: Look,
    ) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        o.kinds.insert(id, kind);
        o.names.insert(id, (name, team, model));
        o.looks.insert(id, look);
    }

    fn on_encounter(&mut self, name: String, state: EncounterState) {
        let (text, colour) = match state {
            EncounterState::Engaged => (format!("{name}: engaged"), hud::WHITE),
            EncounterState::Reset => (format!("{name}: reset"), hud::ORANGE),
            EncounterState::Cleared { secs } => {
                (format!("{name}: cleared in {secs} s"), hud::GREEN)
            }
        };
        self.zone_said(text, colour);
    }

    fn on_loot(&mut self, encounter: String, items: Vec<String>, coin: u32) {
        let mut what = items.join(", ");
        if coin > 0 {
            if !what.is_empty() {
                what.push_str(", ");
            }
            what.push_str(&format!("{coin} silver"));
        }
        self.zone_said(format!("loot ({encounter}): {what}"), hud::YELLOW);
    }

    fn on_trial(&mut self, name: String, passed: bool, detail: String) {
        if passed {
            self.zone_said(format!("trial passed: {name}"), hud::GREEN);
            self.zone_said(detail, hud::DIM);
        } else {
            self.zone_said(format!("trial not passed: {name}: {detail}"), hud::DIM);
        }
    }

    /// The hub revoked a model: nobody wears it from here on, and the renderer lets it
    /// go at the next frame.
    fn on_model_revoked(&mut self, id: ModelId) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        for entry in o.names.values_mut() {
            if entry.2 == Some(id) {
                entry.2 = None;
            }
        }
        for stall in &mut o.stalls {
            if stall.model == Some(id) {
                stall.model = None;
            }
        }
        self.scene.revoked.push(id);
    }

    fn on_stall_opened(&mut self, stall: StallEntry) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        log::info!("{} opened a stall", stall.owner);
        o.stalls.retain(|s| s.id != stall.id);
        o.stalls.push(stall);
    }

    fn on_stall_result(&mut self, result: Result<(), String>) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        o.respec_note = match result {
            Ok(()) => "stall: done".into(),
            Err(e) => format!("stall refused: {e}"),
        };
        log::info!("{}", o.respec_note);
    }

    fn on_report_result(&mut self, result: Result<(), String>) {
        let (text, colour) = match result {
            Ok(()) => (
                "report taken: the fight is kept for a moderator".to_string(),
                hud::GREEN,
            ),
            Err(why) => (format!("report refused: {why}"), hud::ORANGE),
        };
        self.zone_said(text, colour);
    }

    /// The zone's content: the pack, the own build and the props a `Look` indexes; the
    /// client state is built from it, or tuned under the zone (GM.md 3).
    fn on_content(
        &mut self,
        pack: ContentPack,
        own: gm_core::build::Build,
        team: u8,
        props: Vec<String>,
    ) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        let Some((entity, _)) = o.welcome else {
            log::error!("content before welcome");
            return;
        };
        o.team = team;
        o.props = props;
        o.build_name = o.build_name_of(&pack, &own);
        match &mut o.client {
            // Content tuned under the zone (GM.md 3): the same body on the new numbers;
            // the prediction goes on.
            Some(c) => {
                log::info!("content tuned: {} abilities", pack.abilities.len());
                c.set_sheet(Sheet::new(own, &pack, team));
                o.gm_note = "tuned".into();
            }
            None => {
                log::info!(
                    "content: {} abilities, {} presets; playing {} on team {team}",
                    pack.abilities.len(),
                    pack.builds.len(),
                    o.build_name
                );
                o.client = Some(ClientState::new(
                    entity,
                    o.rate,
                    Sheet::new(own, &pack, team),
                ));
            }
        }
        o.pack = Some(pack);
        o.bar_wanted = true;
    }

    /// What the zone says to its game master (GM.md).
    fn on_gm_news(&mut self, news: GmNews) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        match news {
            GmNews::Granted => {
                o.gm = true;
                o.say("game master here: G opens the page".into(), hud::DIM);
                log::info!("game master");
            }
            GmNews::Tuning(t) => {
                o.gm_note = if t.is_default() {
                    "as authored".into()
                } else {
                    format!("tempo x{:.2}, {} set", t.tempo, t.abilities.len())
                };
                o.tuning = t;
            }
            GmNews::Refused(why) => {
                log::info!("gm refused: {why}");
                o.gm_note = format!("refused: {why}");
            }
        }
    }

    /// The zone applied a build (a respec, at a respawn or the trainer).
    fn on_build_applied(&mut self, build: gm_core::build::Build) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        let changed = o.client.as_ref().is_some_and(|c| c.sheet.build != build);
        if changed && let Some(pack) = &o.pack {
            let name = o.build_name_of(pack, &build);
            let sheet = Sheet::new(build, pack, o.team);
            if let Some(c) = &mut o.client {
                c.set_sheet(sheet);
            }
            o.build_name = name;
            o.respec_note = format!("now {}", o.build_name);
            log::info!("build applied: {}", o.build_name);
        }
    }

    /// The zone's word on a respec is the page's note and a line on the HUD, so that it
    /// is read with the page up or closed (MATRIX.md 9.1).
    fn on_respec_result(&mut self, result: Result<(), String>) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        let (note, colour) = match result {
            Ok(()) if o.has_trainer() => ("worn, and saved".to_string(), hud::GREEN),
            Ok(()) => ("saved; worn at the next respawn".to_string(), hud::GREEN),
            Err(e) => (e, hud::ORANGE),
        };
        o.respec_note = note.clone();
        o.say(note, colour);
    }

    /// A travel ticket: kept for the frame to act on (`take_travel`). The connection here
    /// is given up only for one that can be opened: a browser needs the zone's web
    /// listener, a native client its address.
    fn on_travel_ticket(
        &mut self,
        zone: String,
        addr: String,
        cert_der: Vec<u8>,
        token: Vec<u8>,
        web: Option<gm_net::control::WebAddr>,
    ) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        log::info!("travel ticket for {zone} at {addr}");
        let to = ZoneAddr {
            addr: addr.parse().ok(),
            cert_der,
            web,
        };
        let reachable = if cfg!(target_arch = "wasm32") {
            to.web.is_some()
        } else {
            to.addr.is_some()
        };
        if reachable {
            o.pending_travel = Some((zone, to, token));
        } else if cfg!(target_arch = "wasm32") {
            o.say(format!("{zone} has no web listener"), hud::ORANGE);
        } else {
            o.say(format!("{zone}: bad address {addr:?}"), hud::ORANGE);
        }
    }

    /// The answers the inventory and the stall wait for (ITEMS.md 5). One that no open
    /// screen is waiting for (the screen was closed, or it is the answer to an earlier
    /// request) is said where the game says things, and moves nothing on a screen.
    fn on_buy_result(&mut self, listing: i64, result: Result<(), String>) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        log::info!("buy of listing {listing}: {result:?}");
        let good = result.is_ok();
        let elsewhere = match (&mut self.screens.bag, &self.session.hub) {
            (Some(bag), Some(hub)) => bag.bought(hub, listing, result),
            _ => Some(match result {
                Ok(()) => "bought: it is in the inventory".to_string(),
                Err(e) => format!("not bought: {e}"),
            }),
        };
        if let Some(text) = elsewhere {
            o.say(text, if good { hud::DIM } else { hud::ORANGE });
        }
    }

    fn on_wear_result(&mut self, item: i64, result: Result<(), String>) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        log::info!("wear of item {item}: {result:?}");
        let good = result.is_ok();
        let elsewhere = match (&mut self.screens.bag, &self.session.hub) {
            (Some(bag), Some(hub)) => bag.worn(hub, item, result),
            _ => Some(match result {
                Ok(()) => "what is worn changed".to_string(),
                Err(e) => format!("what is worn did not change: {e}"),
            }),
        };
        if let Some(text) = elsewhere {
            o.say(text, if good { hud::DIM } else { hud::ORANGE });
        }
    }

    /// People together (PARTY.md 4). What the party is is the hub's word, shown as it
    /// is said.
    fn on_party(&mut self, names: Vec<String>) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        log::info!("party: {}", names.join(", "));
        if !names.is_empty() {
            // In a party: whatever invitations waited are moot.
            o.social.asks.retain(|a| a.trade.is_some());
        }
        o.social.party = names;
    }

    /// Somebody who is not heard is not heard here either: the invitation is declined,
    /// so that it holds none of the places.
    fn on_invited(&mut self, from: String) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        let unheard = self
            .settings
            .ignored
            .iter()
            .any(|i| names::skeleton(i) == names::skeleton(&from));
        if unheard {
            // (The zone takes one such request a second, as the page's.)
            o.social.asked = Some(Instant::now());
            o.net
                .send_control(FromClient::PartyAnswer { from, join: false });
        } else if o.social.invited(from.clone(), Instant::now()) {
            log::info!("{from} invites to a party");
            self.sound.play(crate::sound::synth::Cue::Chime);
            let line = format!("{from} invites you to a party: P");
            self.screens.chat.heard(None, line, &[]);
        }
    }

    /// Heard as it is shown: not an ignored name's, not the own whisper going out
    /// (SOUND.md 3). (Not logged: a whisper is between two people.)
    fn on_heard(&mut self, channel: u8, from: String, text: String) {
        log::debug!("<{from} on {channel}> {text}");
        let shown = self
            .screens
            .chat
            .heard_on(channel, from, text, &self.settings.ignored);
        if shown && channel != gm_net::control::CHANNEL_WHISPERED {
            self.sound.play(crate::sound::synth::Cue::Blip);
        }
    }

    fn on_trade_asked(&mut self, from: u32) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        let Some(name) = o.names.get(&from).map(|n| n.0.clone()) else {
            return;
        };
        let unheard = self
            .settings
            .ignored
            .iter()
            .any(|i| names::skeleton(i) == names::skeleton(&name));
        if !unheard && o.social.trade_asked(from, name.clone(), Instant::now()) {
            log::info!("{name} asks to trade");
            self.sound.play(crate::sound::synth::Cue::Chime);
            let line = format!("{name} asks to trade: P");
            self.screens.chat.heard(None, line, &[]);
        }
    }

    /// The hub opened a trade both asked for: its window comes up over whatever screen
    /// was up (said twice, it is up already).
    fn on_trade_opened(&mut self, trade: i64, with: String) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        log::info!("trade {trade} with {with}");
        o.social
            .asks
            .retain(|a| a.trade.is_none() || a.from != with);
        let up = self
            .screens
            .people
            .as_ref()
            .is_some_and(|p| p.trading(trade));
        if !up
            && let (Some(hub), Some(account), Some(character)) = (
                &self.session.hub,
                &self.session.account,
                self.session.character,
            )
        {
            let now = Instant::now();
            let session = account.session;
            self.screens.people = Some(People::trade(hub, session, character, trade, with, now));
            self.screens.menu = None;
            self.screens.bag = None;
            self.screens.chat.drop_line();
            self.input.keys.clear();
            self.input.just_pressed.clear();
            self.input.mouse.clear();
        }
    }

    /// A blow the own hand landed: the number where it landed (LOOK.md 13.11), not over
    /// the head (a headshot reads as one), and the combo counter (MODES.md 4.6).
    fn on_hit(&mut self, target: u32, amount: u16, absorbed: u16, at: [f32; 3]) {
        let _ = target;
        self.scene
            .effects
            .hit(Vec3::from_array(at), amount as u32, absorbed as u32);
        let now = Instant::now();
        self.scene.combo = match self.scene.combo {
            (n, Some(last)) if now.duration_since(last).as_secs_f32() < COMBO_SECS => {
                (n + 1, Some(now))
            }
            _ => (1, Some(now)),
        };
    }

    fn on_killed(&mut self, victim: u32, killer: u32) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        let me = o.client.as_ref().map(|c| c.my_id);
        if Some(killer) == me && victim != killer {
            o.kills += 1;
        }
        if Some(victim) == me {
            o.deaths += 1;
        }
        let name = |id: u32| {
            o.names
                .get(&id)
                .map(|(n, _, _)| n.clone())
                .unwrap_or_else(|| format!("#{id}"))
        };
        log::info!("{} killed {}", name(killer), name(victim));
    }

    /// A line in the zone's chat. From nobody: the zone itself (a refusal, a notice).
    fn on_chat_from(&mut self, from: u32, text: String) {
        let Some(o) = self.online.as_mut() else {
            return;
        };
        let who = (from != 0).then(|| {
            o.names
                .get(&from)
                .map_or_else(|| format!("#{from}"), |n| n.0.clone())
        });
        log::info!("<{}> {text}", who.as_deref().unwrap_or("zone"));
        let mine = o.client.as_ref().is_some_and(|c| c.my_id == from);
        let shown = self.screens.chat.heard(who, text, &self.settings.ignored);
        if shown && !mine && from != 0 {
            self.sound.play(crate::sound::synth::Cue::Blip);
        }
    }

    /// A travel ticket: say goodbye here, connect there (the body stays as a ghost until
    /// the other zone claims it, HUB.md 3.3). `true` when the frame ends on it.
    fn take_travel(&mut self) -> bool {
        let Some(o) = self.online.as_mut() else {
            return false;
        };
        let Some((zone, to, token)) = o.pending_travel.take() else {
            return false;
        };
        o.net.hang_up();
        o.backlog.clear();
        match NetClient::connect(to, self.opts.name.clone(), None, self.opts.team, token) {
            Ok(net) => {
                let old = std::mem::replace(&mut o.net, net);
                self.session.leaving.push((old, Instant::now()));
                o.welcome = None;
                o.client = None;
                o.pack = None;
                o.names.clear();
                o.kinds.clear();
                o.squad.clear();
                o.stalls.clear();
                o.zone_name = zone;
                o.respec_note = format!("travelling to {}", o.zone_name);
                // The next zone says what the party is; who asked what stays here.
                o.social = Social::default();
                self.screens.bag = None;
                self.screens.people = None;
                self.screens.gm_page = None;
                self.scene.entities.clear();
                self.scene.bodies.clear();
            }
            // The connection that was hung up says so in a moment, and that ends it.
            Err(e) => log::error!("travel failed: {e}"),
        }
        true
    }

    /// The prediction's ticks for this frame: the own inputs sent, the own actions and
    /// the ground covered kept for the sound and the view model. `None` before the zone's
    /// content made a client state.
    fn predict(&mut self, frame_dt: f32) -> Option<Predicted> {
        let viewport = self.view.viewport;
        let scripted = self.opts.script.as_deref() == Some("fight");
        let bsp = &self.bsp;
        let o = self.online.as_mut()?;
        let c = o.client.as_mut()?;
        let dt = o.rate.dt();
        o.accumulator += frame_dt.min(0.25);
        let mut steps = 0;
        let bodies = c.latest_boxes();
        // What the own body did this frame, for the sound: its predicted actions and
        // the ground it covered by its own ticks (SOUND.md 3).
        let mut own_actions: Vec<gm_core::sim::Action> = Vec::new();
        let mut own_travel = 0.0_f32;
        // Space dodges in the action mode (MODES.md 4.6): the kit's dash, while ready.
        let dodge = (c.sheet.kit.mode == gm_core::vocab::Mode::Action)
            .then(|| dodge_slot(&c.sheet.kit, &c.mover, c.tick))
            .flatten();
        let gun = c.sheet.kit.mode == gm_core::vocab::Mode::Gun;
        let rpg = c.sheet.kit.mode == gm_core::vocab::Mode::Rpg;
        // The RPG mode's keys (MODES.md 5.3): with a target they ask for a target-action;
        // without one they press as the action mode does.
        let rpg_bodies = if rpg {
            rpg_bodies_of(c, o.team)
        } else {
            Vec::new()
        };
        let (mut rpg_pressed, mut rpg_ability) = if rpg {
            Self::rpg_keys(&mut self.rpg, &mut self.input, &rpg_bodies)
        } else {
            (0, 0)
        };
        while o.accumulator >= dt && steps < MAX_STEPS_PER_FRAME {
            let (yaw, pitch) = match viewport {
                Viewport::First => (self.sim.yaw, self.sim.pitch),
                // The RPG body faces where it goes or its target; its bolts without a
                // target fly level (MODES.md 5.3).
                Viewport::Third if rpg => (self.sim.yaw, 0.0),
                Viewport::Third => {
                    let eye = c.mover.eye();
                    let camera = third_person_camera(bsp, eye, self.sim.yaw, self.sim.pitch);
                    re_aim(bsp, &bodies, camera, self.sim.yaw, self.sim.pitch, eye)
                }
            };
            self.view.aim = (yaw, pitch);
            let rpg_frame = rpg.then(|| {
                let mut f = self.rpg.frame(
                    bsp,
                    &c.sheet.kit,
                    &c.mover,
                    &rpg_bodies,
                    self.input.axes(),
                    yaw,
                    c.tick,
                    o.rate.hz(),
                );
                f.buttons |= std::mem::take(&mut rpg_pressed);
                if f.ability == 0 {
                    f.ability = std::mem::take(&mut rpg_ability);
                }
                f
            });
            let input = if scripted {
                let target = self
                    .scene
                    .script_target
                    .map(|(_, at, velocity, _)| (at, velocity));
                let input = fight_input(
                    c.mover.eye(),
                    target,
                    &mut self.sim,
                    &mut self.scene.script_stuck,
                    c.tick,
                );
                self.view.aim = (input.yaw, input.pitch);
                input
            } else {
                self.input.sim_input(yaw, pitch, dodge, gun, rpg_frame)
            };
            // A press of an item key that begins no use is said why (MODES.md 11.3):
            // what the predicted body knows (an empty cell, the air, the hands, a
            // stagger), else the health, which the zone refuses the same tick.
            if input.buttons & buttons::USE != 0
                && let Some(cell) = gm_core::sim::bar_cell(input.use_slot)
            {
                let m = &c.mover;
                let staggered = m.statuses.staggered() || m.statuses.downed();
                let why = gm_core::sim::item_refusal(m, cell, c.tick, staggered)
                    .map(gm_core::sim::ItemRefusal::word)
                    .or_else(|| (c.own_health >= c.sheet.derived.health).then_some(KIT_FULL));
                if let Some(why) = why {
                    log::info!("item {}: {why}", BAR_KEYS[cell]);
                    o.kit_note = Some((Instant::now(), why));
                }
            }
            let before = c.mover.mv.origin;
            let datagram = c.local_tick(bsp, input);
            own_actions.extend(c.actions.iter().copied());
            if c.mover.mv.on_ground {
                own_travel += (c.mover.mv.origin - before).truncate().length();
            }
            o.net.send_input(datagram.encode());
            o.prev_origin = o.curr_origin;
            o.curr_origin = c.mover.mv.origin;
            o.accumulator -= dt;
            steps += 1;
        }
        if steps == MAX_STEPS_PER_FRAME {
            o.accumulator = 0.0;
        }
        if !c.synced() {
            // Until the first snapshot lands, show the map from the local spawn.
            o.prev_origin = c.mover.mv.origin;
            o.curr_origin = c.mover.mv.origin;
        }
        Some(Predicted {
            own_actions,
            own_travel,
            rpg,
            rpg_bodies,
            scripted,
            dt,
        })
    }

    /// The RPG mode's keys (MODES.md 5.3): with a target they ask for a target-action;
    /// without one they press as the action mode does. Returns the buttons and the
    /// active pressed without a target.
    fn rpg_keys(
        rpg: &mut crate::rpg::Rpg,
        input: &mut Input,
        rpg_bodies: &[crate::rpg::Body],
    ) -> (u16, u8) {
        use crate::rpg::Act;
        let present = |id: u32| rpg_bodies.iter().any(|b| b.id == id);
        rpg.lost(present);
        let mut pressed = 0u16;
        let mut ability = 0u8;
        let asks = [
            (KeyCode::Digit1, Act::Primary),
            (KeyCode::Digit2, Act::Secondary),
            (KeyCode::Digit3, Act::Active(1)),
            (KeyCode::Digit4, Act::Active(2)),
            (KeyCode::Digit5, Act::Active(3)),
            (KeyCode::Digit6, Act::Active(4)),
        ];
        for (key, act) in asks {
            if !input.just_pressed.remove(&key) {
                continue;
            }
            if rpg.target.is_some() {
                rpg.ask(act);
            } else {
                match act {
                    Act::Primary => pressed |= buttons::PRIMARY,
                    Act::Secondary => pressed |= buttons::SECONDARY,
                    Act::Active(n) => ability = n,
                }
            }
        }
        (pressed, ability)
    }

    /// Where the frame is seen from: the own hull's drawn origin and the eye over it
    /// (the crouch's drop eased, the recoil's punch decaying), the camera of the
    /// viewport, and the other entities at the interpolation time (PROTOCOL.md 7.3).
    fn seen(&mut self, frame_dt: f32, predicted: &Predicted) -> Seen {
        let scope = self.gun_scope();
        let viewport = self.view.viewport;
        let bsp = &self.bsp;
        let o = self.online.as_mut().expect("predicted");
        let c = o.client.as_mut().expect("predicted");
        let dt = predicted.dt;
        let extra = o.last_snapshot.elapsed().as_secs_f32() / dt;
        let t = c.render_tick(extra);
        self.scene.entities.clear();
        self.scene.bodies.clear();
        self.scene.tags.clear();
        self.scene.effects.begin(frame_dt);
        // The crouch lowers the eye (MODES.md 3.4): the predicted mover's drop, eased over
        // about a tenth of a second so the view does not jump.
        let drop = if c.mover.crouched {
            gm_core::sim::CROUCH_DROP
        } else {
            0.0
        };
        self.view.eye_drop += (drop - self.view.eye_drop) * (frame_dt * 12.0).min(1.0);
        // The own hull origin as drawn, and the eye over it less the drop: the body
        // itself is lowered by its squat (the animator's), not by the eye.
        let centre = {
            let alpha = (o.accumulator / o.rate.dt()).clamp(0.0, 1.0);
            o.prev_origin.lerp(o.curr_origin, alpha)
        };
        let eye = centre + Vec3::Z * (c.mover.mv.hull.eye_height() - self.view.eye_drop);
        // The recoil's punch (MODES.md 3.3): the firearm's kick for this shot lands on the
        // view at once and decays over 150 ms; the frames sent carry the mouse's aim,
        // the zone kicks the bolt by the same pair.
        for a in &predicted.own_actions {
            if let gm_core::sim::Action::Fire { kick, .. } = a
                && (kick.0 != 0.0 || kick.1 != 0.0)
            {
                self.view.punch = *kick;
            }
        }
        let decay = (-frame_dt / PUNCH_DECAY_SECS).exp();
        self.view.punch = (self.view.punch.0 * decay, self.view.punch.1 * decay);
        self.view.zoom = if self.input.scoped {
            scope.max(1) as f32
        } else {
            1.0
        };
        let camera = match viewport {
            Viewport::First => eye,
            Viewport::Third if predicted.rpg => {
                crate::rpg::orbit_camera(bsp, centre, self.sim.yaw, self.sim.pitch, self.rpg.dist)
            }
            Viewport::Third => third_person_camera(bsp, eye, self.sim.yaw, self.sim.pitch),
        };
        let others = c.others_at(t);
        Seen {
            centre,
            eye,
            camera,
            t,
            others,
            my_team: o.team,
            viewport,
        }
    }

    /// `--script fight`: the nearest living enemy, its velocity from where it stood a
    /// frame ago if it is the same body.
    fn script_target(&mut self, predicted: &Predicted, seen: &Seen) {
        if !predicted.scripted {
            return;
        }
        let my_team = seen.my_team;
        let centre = seen.centre;
        let nearest = seen
            .others
            .iter()
            .filter(|e| e.kind == EntityKind::Player && e.alive())
            .filter(|e| match e.spawn {
                SpawnInfo::Player { team, .. } => my_team == 0 || team != my_team,
                _ => false,
            })
            .min_by(|a, b| {
                (a.pos - centre)
                    .length_squared()
                    .total_cmp(&(b.pos - centre).length_squared())
            })
            .map(|e| (e.id, e.pos));
        let now = Instant::now();
        self.scene.script_target = nearest.map(|(id, at)| {
            let velocity = match self.scene.script_target {
                Some((was, before, _, then)) if was == id => {
                    let dt = (now - then).as_secs_f32();
                    if dt > 1.0e-3 {
                        ((at - before) / dt).clamp_length_max(600.0)
                    } else {
                        Vec3::ZERO
                    }
                }
                _ => Vec3::ZERO,
            };
            (id, at, velocity, now)
        });
    }

    /// What the HUD shows of the squad, of the party and of the creature being fought:
    /// the one the squad is ordered onto, or else the nearest one that is hurt; in the
    /// RPG mode the target's frame (MODES.md 5.2).
    fn hud_views(&mut self, seen: &Seen) {
        let Some(o) = self.online.as_ref() else {
            return;
        };
        let Some(c) = o.client.as_ref() else { return };
        let others = &seen.others;
        let centre = seen.centre;
        self.scene.squad_view = o
            .squad
            .iter()
            .map(|m| {
                let e = others.iter().find(|e| e.id == m.id);
                (e.and_then(|e| e.health), e.is_some_and(|e| e.alive()))
            })
            .collect();
        // And of the party: the other members, each with the health the wire carries for
        // its body when that is here (the bodies that are people: a hired copy of
        // somebody bears that name too).
        let me = Some(c.my_id);
        self.scene.party_view =
            o.social
                .party
                .iter()
                .filter_map(|member| {
                    let body = o.names.iter().find(|(id, n)| {
                        n.0 == *member && o.kinds.get(id) == Some(&BodyKind::Human)
                    });
                    match body {
                        Some((id, _)) if Some(*id) == me => None,
                        Some((id, _)) => {
                            let health = others.iter().find(|e| e.id == *id).and_then(|e| e.health);
                            Some((member.clone(), Some(health)))
                        }
                        None => Some((member.clone(), None)),
                    }
                })
                .collect();
        self.scene.target_view = None;
        // The RPG mode's target frame (MODES.md 5.2): its name and the health the zone
        // sends for a targeted body; a stranger's whole is read as the band's top.
        let rpg = c.sheet.kit.mode == gm_core::vocab::Mode::Rpg;
        if rpg && let Some(target) = self.rpg.target {
            if let Some(e) = others.iter().find(|e| e.id == target) {
                let name = o
                    .names
                    .get(&target)
                    .map_or_else(|| "?".to_string(), |n| n.0.clone());
                let max = match (o.kinds.get(&target), &o.pack) {
                    (Some(BodyKind::Creature { def }), Some(pack)) => {
                        pack.creatures.get(*def as usize).map_or(1500, |d| d.health)
                    }
                    _ => 1500,
                };
                self.scene.target_view = Some((name, e.health.unwrap_or(0), max));
            }
        } else if let Some(pack) = &o.pack {
            let mut best: Option<(f32, String, u16, u16)> = None;
            for e in others.iter().filter(|e| e.alive()) {
                let (Some(BodyKind::Creature { def }), Some(health)) =
                    (o.kinds.get(&e.id), e.health)
                else {
                    continue;
                };
                let Some(d) = pack.creatures.get(*def as usize) else {
                    continue;
                };
                let ordered = o
                    .squad
                    .iter()
                    .any(|m| matches!(m.order, Order::Attack(id) if id == e.id));
                if !ordered && health >= d.health {
                    continue;
                }
                let rank = (e.pos - centre).length() - if ordered { 1.0e6 } else { 0.0 };
                if best.as_ref().is_none_or(|b| rank < b.0) {
                    best = Some((rank, d.name.clone(), health, d.health));
                }
            }
            self.scene.target_view = best.map(|(_, name, health, max)| (name, health, max));
        }
    }

    /// Every other entity at the interpolation time: the bodies with their looks and
    /// their names over them, the bolts and the areas as effects.
    fn others_in_sight(&mut self, frame_dt: f32, predicted: &Predicted, seen: &Seen) {
        for e in seen.others.iter().copied() {
            match e.kind {
                EntityKind::Player => self.other_body(e, frame_dt, predicted, seen),
                EntityKind::Projectile => self.other_projectile(e, seen),
                EntityKind::Area => self.other_area(e),
            }
        }
    }

    /// Another body: whose side it is on, what it does drawn where it lands (LOOK.md
    /// 13: the wedge of the swing it winds up, the slash when it comes, a spark when it
    /// is hit), the own hand's numbers over its head, its name when the head is in sight
    /// and near, its facing, and the body itself with what it holds and wears.
    fn other_body(
        &mut self,
        e: gm_net::client::RenderEntity,
        frame_dt: f32,
        predicted: &Predicted,
        seen: &Seen,
    ) {
        let SpawnInfo::Player {
            frame,
            team,
            aspects,
            armour,
        } = e.spawn
        else {
            return;
        };
        let bsp = &self.bsp;
        let Some(o) = self.online.as_mut() else {
            return;
        };
        let camera = seen.camera;
        let my_team = seen.my_team;
        let feet_under = Vec3::Z * Hull::Player.mins().z;
        // Health the zone sends (the own party's and creatures') under the name.
        let mate = o.squad.iter().position(|m| m.id == e.id);
        let max_health = match (mate, o.kinds.get(&e.id), &o.pack) {
            (Some(i), _, _) => Some(o.squad[i].max_health),
            (None, Some(BodyKind::Creature { def }), Some(pack)) => {
                pack.creatures.get(*def as usize).map(|d| d.health)
            }
            _ => None,
        };
        // Whose side it is on: the own team's, the own squad's and the own party's are
        // friends; a creature and another team's are foes.
        let name = o.names.get(&e.id).map(|n| n.0.as_str()).unwrap_or("");
        let creature = matches!(o.kinds.get(&e.id), Some(BodyKind::Creature { .. }));
        let friend = (my_team != 0 && team == my_team)
            || mate.is_some()
            || (!name.is_empty() && self.scene.party_view.iter().any(|(n, _)| n == name));
        let foe = !friend && (creature || (my_team != 0 && team != 0));
        if e.alive() {
            let swing = o
                .pack
                .as_ref()
                .zip((e.acting as usize).checked_sub(1))
                .and_then(|(p, i)| p.abilities.get(i))
                .and_then(|d| swing_of(&d.ability, predicted.dt));
            self.scene.effects.fighter(
                &crate::fx::Fighter {
                    key: e.id,
                    feet: e.pos + feet_under,
                    chest: crate::fx::chest_of(e.flags & gm_net::snapshot::flags::CROUCHED != 0),
                    yaw: e.yaw,
                    anim: e.anim,
                    swing,
                    side: if friend {
                        crate::fx::Side::Friend
                    } else {
                        crate::fx::Side::Foe
                    },
                    health: e.health,
                },
                &mut self.scene.fx,
            );
            // What the own hand did to it, as the zone said: a number over its head
            // (LOOK.md 13.8).
            let over_head = e.pos + feet_under + Vec3::Z * 72.0;
            o.heals.retain(|&(target, amount)| {
                if target != e.id {
                    return true;
                }
                self.scene.effects.healed(over_head, amount as u32);
                false
            });
            Self::other_tag(
                bsp,
                &mut self.scene.tags,
                &e,
                name,
                (friend, creature, foe),
                max_health,
                camera,
            );
        }
        let yaw = facing(
            self.view.facings.get(&e.id).copied(),
            e.yaw,
            e.vel,
            e.anim,
            e.flags & gm_net::snapshot::flags::RPG != 0,
            frame_dt,
        );
        self.view.facings.insert(e.id, yaw);
        let look = o.looks.get(&e.id).copied().unwrap_or_default();
        let worn = worn_overlay(
            &mut self.content,
            self.active.as_mut(),
            &o.props,
            look,
            frame,
            armour,
        );
        self.scene.bodies.push(Body {
            key: e.id,
            origin: e.pos,
            yaw,
            pitch: e.pitch,
            anim: e.anim,
            crouched: e.flags & gm_net::snapshot::flags::CROUCHED != 0,
            frame,
            armour,
            aspects,
            status: e.status,
            model: o.names.get(&e.id).and_then(|n| n.2),
            distance: (e.pos - camera).length(),
            lit: self.scene.effects.flash(e.id),
            prop: held_prop(&mut self.content, self.active.as_mut(), &o.props, look),
            off: off_prop(&mut self.content, self.active.as_mut(), &o.props, look),
            worn: worn.0,
            hide: worn.1,
            patches: worn.2,
            sheathed: look.sheathed,
        });
    }

    /// Its name over its head, when the head is in sight and near: in the colour of
    /// its side (`friend`, `creature`, `foe`), with its health under it when known.
    fn other_tag(
        bsp: &Bsp,
        tags: &mut Vec<Tag>,
        e: &gm_net::client::RenderEntity,
        name: &str,
        side: (bool, bool, bool),
        max_health: Option<u16>,
        camera: Vec3,
    ) {
        let (friend, creature, foe) = side;
        let head = e.pos + Vec3::Z * 40.0;
        if name.is_empty()
            || (head - camera).length() >= 900.0
            || bsp.trace(Hull::Point, camera, head).fraction < 1.0
        {
            return;
        }
        tags.push(Tag {
            at: head,
            name: name.to_string(),
            ink: if friend {
                [0.55, 0.80, 1.0, 1.0]
            } else if creature {
                [1.0, 0.62, 0.30, 1.0]
            } else if foe {
                [1.0, 0.42, 0.36, 1.0]
            } else {
                hud::WHITE
            },
            health: e
                .health
                .zip(max_health)
                .map(|(h, max)| h as f32 / max.max(1) as f32),
        });
    }

    /// A bolt: a bright head on a streak along its way, round a small solid core. The
    /// own bolts are known by their ability (its size, its damage's colour); another's
    /// is a bolt.
    fn other_projectile(&mut self, e: gm_net::client::RenderEntity, seen: &Seen) {
        self.scene.entities.push(EntityDraw {
            mins: e.pos - Vec3::splat(1.2),
            maxs: e.pos + Vec3::splat(1.2),
            color: [1.0, 0.95, 0.6, 1.0],
        });
        let c = self.online.as_ref().and_then(|o| o.client.as_ref());
        let known = match (e.spawn, c) {
            (SpawnInfo::Projectile { owner, def, .. }, Some(c)) if owner == c.my_id => {
                c.sheet.kit.abilities.get(def as usize).and_then(|a| {
                    a.steps.iter().find_map(|s| match &s.verb {
                        gm_core::vocab::Verb::Projectile(p) => {
                            Some((p.radius, damage_ink(&p.damage)))
                        }
                        _ => None,
                    })
                })
            }
            _ => None,
        };
        let (radius, ink) = known.unwrap_or((3.0, crate::fx::BOLT));
        self.scene.effects.projectile(
            e.id,
            e.pos,
            e.vel,
            radius,
            ink,
            seen.camera,
            &mut self.scene.fx,
        );
    }

    /// An area: a disc of its own size on the floor, and a burst when it appears. What
    /// hurts is orange, what helps is green (PROTOCOL.md 5).
    fn other_area(&mut self, e: gm_net::client::RenderEntity) {
        let (r, harmful) = match e.spawn {
            SpawnInfo::Area {
                radius, harmful, ..
            } => (radius as f32, harmful),
            _ => (32.0, true),
        };
        self.scene
            .effects
            .area(e.id, e.pos, r, harmful, &mut self.scene.fx);
    }

    /// Heard (SOUND.md 3): every new sample of every entity up to the render tick, the
    /// own body as it predicted itself and as the zone said, from the own body's place
    /// with the camera's facing. Then the tracks older than the render tick are let go.
    fn hear(&mut self, frame_dt: f32, predicted: &Predicted, seen: &Seen) {
        let bsp = &self.bsp;
        let Some(o) = self.online.as_mut() else {
            return;
        };
        let Some(c) = o.client.as_mut() else { return };
        let world = |from: Vec3, to: Vec3| {
            use gm_core::trace::CollisionWorld;
            bsp.trace(Hull::Point, from, to).fraction < 1.0
        };
        self.sound.feed(c.tracks(), seen.t, &world);
        let anims = std::mem::take(&mut c.own_anims);
        if c.synced() {
            let me = crate::sound::cues::OwnNow {
                id: c.my_id,
                pos: seen.centre,
                on_ground: c.mover.mv.on_ground,
                travel: predicted.own_travel,
                health: c.own_health,
                alive: c.own_alive,
            };
            self.sound.own(
                self.clock.started.elapsed().as_secs_f32(),
                me,
                &anims,
                &predicted.own_actions,
            );
        }
        let yaw = self.sim.yaw;
        let listener = crate::sound::Listener {
            pos: seen.eye,
            yaw: yaw.to_radians(),
        };
        self.sound.end_frame(frame_dt, listener);
        c.prune(seen.t);
    }

    /// The market: every stall with its keeper, who never moves and costs no snapshot.
    /// While somebody stands behind the counter (the owner, usually), that body is the
    /// keeper; the stand-in is drawn when the tile is empty.
    fn stalls_in_sight(&mut self, seen: &Seen) {
        let Some(o) = self.online.as_ref() else {
            return;
        };
        for stall in &o.stalls {
            stall_boxes(stall, &mut self.scene.entities);
            let at = Vec3::from(stall.pos);
            let attended = self
                .scene
                .bodies
                .iter()
                .any(|b| (b.origin - at).truncate().length() < 40.0)
                || (o.curr_origin - at).truncate().length() < 40.0;
            if !attended {
                self.scene.bodies.push(stall_keeper(stall, seen.camera));
            }
        }
    }

    /// The own body's stance (LOOK.md 13): what the zone says of it, except that its own
    /// script is shown from the prediction, a round trip sooner; and so is the place its
    /// swing lands, which is what a hand aims with. Returns the stance.
    fn own_stance(&mut self, predicted: &Predicted, seen: &Seen) -> u8 {
        let Some(o) = self.online.as_ref() else {
            return gm_core::sim::anim::IDLE;
        };
        let Some(c) = o.client.as_ref() else {
            return gm_core::sim::anim::IDLE;
        };
        let feet_under = Vec3::Z * Hull::Player.mins().z;
        let own_script = c.mover.script.and_then(|s| {
            let ability = c.sheet.kit.abilities.get(s.ability as usize)?;
            let elapsed = gm_core::sim::tick_delta(c.tick, s.started).max(0) as u32;
            Some((ability, elapsed))
        });
        let own_anim = {
            use gm_core::sim::anim;
            let said = c.own_anim;
            if !c.synced() {
                anim::IDLE
            } else if !c.own_alive
                || !(anim::acts(said) || matches!(said, anim::IDLE | anim::RUN | anim::AIR))
            {
                // Dead, staggered, commanding, dashing, guarding: the zone's word.
                said
            } else if let Some((ability, elapsed)) = own_script {
                gm_core::sim::script_anim(ability, elapsed)
            } else {
                // The feet are predicted here, so their stance is too: a run that waited
                // for the zone's word slid for a round trip before the legs moved (and
                // the script over here and not yet there is the same case).
                if !c.mover.mv.on_ground {
                    anim::AIR
                } else if c.mover.ground_speed() > 10.0 {
                    anim::RUN
                } else {
                    anim::IDLE
                }
            }
        };
        if c.synced() {
            self.scene.effects.fighter(
                &crate::fx::Fighter {
                    key: OWN,
                    feet: seen.centre + feet_under,
                    // Under the eye in the first person, so the slash crosses the view.
                    chest: crate::fx::chest_of(c.mover.crouched),
                    yaw: c.mover.yaw,
                    anim: own_anim,
                    swing: own_script.and_then(|(ability, _)| swing_of(ability, predicted.dt)),
                    side: crate::fx::Side::Own,
                    health: Some(c.own_health.clamp(0, u16::MAX as i32) as u16),
                },
                &mut self.scene.fx,
            );
        }
        own_anim
    }

    /// A bolt the own body let go this frame flies from the hand at once (the zone's
    /// bolt shows a round trip later, well on its way).
    fn own_launches(&mut self, predicted: &Predicted, seen: &Seen) {
        let bsp = &self.bsp;
        let Some(o) = self.online.as_ref() else {
            return;
        };
        let Some(c) = o.client.as_ref() else { return };
        let eye = seen.eye;
        for action in &predicted.own_actions {
            if let gm_core::sim::Action::Fire {
                ability,
                step,
                target,
                ..
            } = action
                && let Some(ab) = c.sheet.kit.abilities.get(*ability as usize)
                && let Some(gm_core::vocab::Verb::Projectile(p)) =
                    ab.steps.get(*step as usize).map(|s| &s.verb)
            {
                // From where the zone spawns it (`resolve_origin`): the weapon's offset in
                // the body's frame, else the eyes. Not the camera: in the third person
                // the eye of the body is well in front of it.
                let from = match p.spawn {
                    gm_core::vocab::Origin::Weapon { offset } => {
                        let (fwd, right) = gm_core::movement::yaw_vectors(c.mover.yaw);
                        eye + fwd * offset[0] + right * offset[1] + Vec3::Z * offset[2]
                    }
                    _ => eye,
                };
                // Where the zone sends it (MODES.md 5.3): led to its target when that is
                // within the ability's range and in sight, as the zone leads it; else
                // along the body's look. (Until 2026-10-08 the tracer always flew the
                // look's way, which in the RPG mode is the camera's: the director saw
                // the shard leave toward the camera's horizon while the zone's bolt
                // went for the dummy.)
                let led = predicted
                    .rpg_bodies
                    .iter()
                    .find(|b| *target != 0 && b.id == *target)
                    .filter(|b| {
                        let centre = b.centre();
                        ab.range > 0.0
                            && (centre - c.mover.mv.origin).truncate().length() <= ab.range
                            && gm_core::sim::sees(bsp, c.mover.eye(), centre)
                    })
                    .map(|b| {
                        let vel = seen
                            .others
                            .iter()
                            .find(|e| e.id == b.id)
                            .map_or(Vec3::ZERO, |e| e.vel);
                        let point = gm_core::sim::aim::lead(
                            from,
                            b.centre(),
                            vel,
                            p.speed,
                            p.gravity_scale,
                        );
                        (point - from).normalize_or_zero()
                    })
                    .filter(|d| d.length_squared() > 0.5);
                let dir = led.unwrap_or_else(|| gm_core::sim::view_dir(c.mover.yaw, c.mover.pitch));
                self.scene
                    .effects
                    .launch(from, dir * p.speed, p.radius, damage_ink(&p.damage));
            }
        }
    }

    /// The frame's effects drawn, the RPG mode's marks (MODES.md 5.5: where the body is
    /// going, and a ring under its target), the numbers over the bodies, and the facings
    /// of the bodies no longer in sight forgotten.
    fn marks(&mut self, predicted: &Predicted, seen: &Seen) {
        if let Some(o) = self.online.as_mut() {
            // A blow on a body the frame did not find (gone, or never in sight) says nothing.
            o.heals.clear();
        }
        self.scene.effects.draw(seen.camera, &mut self.scene.fx);
        if predicted.rpg {
            if let Some(goal) = self.rpg.walk {
                let feet = goal + Vec3::Z * Hull::Player.mins().z;
                self.scene.fx.wall(
                    feet,
                    10.0,
                    6.0,
                    [1.0, 0.85, 0.3, 0.6],
                    [1.0, 0.85, 0.3, 0.0],
                );
            }
            if let Some(t) = self.rpg.target
                && let Some(e) = seen.others.iter().find(|e| e.id == t)
            {
                let feet = e.pos + Vec3::Z * Hull::Player.mins().z;
                self.scene
                    .fx
                    .wall(feet, 20.0, 4.0, [1.0, 0.3, 0.2, 0.7], [1.0, 0.3, 0.2, 0.0]);
            }
        }
        self.scene.effects.end();
        self.scene.pops = self.scene.effects.numbers();
        // (The own body's facing is kept across a frame in the first person, where it
        // is not drawn: a switch of the viewport finds it where it was.)
        let drawn: HashSet<u32> = self
            .scene
            .bodies
            .iter()
            .map(|b| b.key)
            .chain([OWN])
            .collect();
        self.view.facings.retain(|k, _| drawn.contains(k));
    }

    /// The own body as the viewport shows it: in the first person the view model
    /// (LOOK.md 6.4) and the camera with the recoil's punch; otherwise the own body
    /// posed by the server's animation state. Returns the camera.
    fn own_view(
        &mut self,
        frame_dt: f32,
        predicted: &Predicted,
        seen: &Seen,
        own_anim: u8,
    ) -> (Vec3, f32, f32) {
        match seen.viewport {
            Viewport::First => self.own_view_model(frame_dt, predicted, seen, own_anim),
            _ => self.own_body(frame_dt, seen, own_anim),
        }
    }

    /// The view model (LOOK.md 6.4): the held prop in the frame's corner, with the
    /// stride's bob and a kick on a launch; a swing is carried across the view by the
    /// own stance. Returns the camera with the recoil's punch (MODES.md 3.3).
    fn own_view_model(
        &mut self,
        frame_dt: f32,
        predicted: &Predicted,
        seen: &Seen,
        own_anim: u8,
    ) -> (Vec3, f32, f32) {
        let eye = seen.eye;
        let fired = predicted
            .own_actions
            .iter()
            .any(|a| matches!(a, gm_core::sim::Action::Fire { .. }));
        let want = match own_anim {
            gm_core::sim::anim::WINDUP => -1.0,
            gm_core::sim::anim::SWING => 1.0,
            gm_core::sim::anim::RECOVER => 0.55,
            _ => 0.0,
        };
        let rate = if want == 0.0 { 9.0 } else { 34.0 };
        self.view.swing += (want - self.view.swing) * (frame_dt * rate).min(1.0);
        if fired {
            self.view.kick = 1.0;
        }
        self.view.kick = (self.view.kick - frame_dt * 6.0).max(0.0);
        self.view.stride += predicted.own_travel / 64.0;
        let bsp = &self.bsp;
        let Some(o) = self.online.as_ref() else {
            return (eye, self.sim.yaw, self.sim.pitch);
        };
        let Some(c) = o.client.as_ref() else {
            return (eye, self.sim.yaw, self.sim.pitch);
        };
        // The gun mode's hand is known here before the zone says so. A weapon on the
        // back (LOOK.md 6.7) is not in the view: the hand is empty.
        let own_look = o.looks.get(&c.my_id).copied().unwrap_or_default();
        let key = match gun_hand_prop(&self.content, o.pack.as_ref(), c) {
            Some(key) => Some(key),
            None if own_look.sheathed => None,
            None => o.props.get(own_look.held as usize).cloned(),
        };
        let fit = key
            .as_deref()
            .map_or(Mat4::IDENTITY, |k| view_fit_of(&self.content, k));
        let prop = key.and_then(|key| {
            let active = self.active.as_mut()?;
            let (gpu, characters) = (&active.gpu, &mut active.renderer.characters);
            self.content
                .prop(&key, |model| Some(characters.add_model(gpu, model)))
        });
        let reload = reload_progress(c);
        // (Not while the scope is up, MODES.md 3.2.)
        self.view.model = prop.filter(|_| !self.input.scoped).map(|slot| ViewModel {
            slot,
            fit,
            eye,
            yaw: self.sim.yaw + self.view.punch.0,
            pitch: self.sim.pitch - self.view.punch.1,
            stride: if c.mover.mv.on_ground {
                self.view.stride
            } else {
                0.0
            },
            kick: self.view.kick,
            swing: self.view.swing,
            reload,
            light: crate::avatars::light_at(bsp, eye),
        });
        (
            eye,
            self.sim.yaw + self.view.punch.0,
            (self.sim.pitch - self.view.punch.1).clamp(-89.0, 89.0),
        )
    }

    /// The own body, posed by the server's animation state. It faces where the mover
    /// does while a turn holds it (toward its target for a target-action, MODES.md 5.3;
    /// the magnet's turn, 4.2) and the camera's way otherwise; an RPG body is free of the
    /// camera between actions (MODES.md 5.1): it stands the way it last went or was
    /// turned, where until 2026-10-08 it swung round with the orbit (the director).
    fn own_body(&mut self, frame_dt: f32, seen: &Seen, own_anim: u8) -> (Vec3, f32, f32) {
        let camera = (seen.camera, self.sim.yaw, self.sim.pitch);
        let Some(o) = self.online.as_ref() else {
            return camera;
        };
        let Some(c) = o.client.as_ref() else {
            return camera;
        };
        let build = &c.sheet.build;
        let locked = c.mover.lock_yaw.is_some();
        let look = if locked { c.mover.yaw } else { self.sim.yaw };
        let rpg = c.sheet.kit.mode == gm_core::vocab::Mode::Rpg;
        let yaw = facing(
            self.view.facings.get(&OWN).copied(),
            look,
            c.mover.mv.velocity,
            own_anim,
            rpg && !locked,
            frame_dt,
        );
        self.view.facings.insert(OWN, yaw);
        let worn = worn_overlay(
            &mut self.content,
            self.active.as_mut(),
            &o.props,
            o.looks.get(&c.my_id).copied().unwrap_or_default(),
            gm_model::rig::frame_index(build.frame),
            build.armour as u8,
        );
        self.scene.bodies.push(Body {
            key: OWN,
            origin: seen.centre,
            yaw,
            pitch: self.sim.pitch,
            anim: own_anim,
            crouched: c.mover.crouched,
            frame: gm_model::rig::frame_index(build.frame),
            armour: build.armour as u8,
            aspects: build.aspects.0,
            status: c.mover.statuses.mask(),
            model: o.names.get(&c.my_id).and_then(|n| n.2),
            distance: 0.0,
            lit: self.scene.effects.flash(OWN),
            prop: held_prop(
                &mut self.content,
                self.active.as_mut(),
                &o.props,
                o.looks.get(&c.my_id).copied().unwrap_or_default(),
            ),
            off: off_prop(
                &mut self.content,
                self.active.as_mut(),
                &o.props,
                o.looks.get(&c.my_id).copied().unwrap_or_default(),
            ),
            worn: worn.0,
            hide: worn.1,
            patches: worn.2,
            sheathed: o.looks.get(&c.my_id).is_some_and(|l| l.sheathed),
        });
        camera
    }

    /// The other bodies as the RPG mode reads them (MODES.md 5): where they stand now,
    /// their frame, and whether they are enemies (another team; in the wild, everyone).
    pub(super) fn rpg_bodies(&self) -> Vec<crate::rpg::Body> {
        let Some(o) = &self.online else {
            return Vec::new();
        };
        let Some(c) = &o.client else {
            return Vec::new();
        };
        rpg_bodies_of(c, o.team)
    }

    /// The item bar (LOOK.md 3.2): asked of the hub once the zone's content is here, and
    /// taken from the inventory screen while it is up (a drag there arranges it).
    pub(super) fn poll_bar(&mut self) {
        let wanted = self.online.as_ref().is_some_and(|o| o.bar_wanted);
        if wanted && let Some((hub, session, character)) = self.owner() {
            let ask = hub.call(PlayerRequest::Econ {
                session,
                character,
                op: gm_hub_proto::player::PlayerEcon::Bar,
            });
            if let Some(o) = &mut self.online {
                o.bar_wanted = false;
                o.bar_ask = Some(ask);
            }
        }
        let Some(o) = &mut self.online else { return };
        if let Some(answer) = o.bar_ask.as_ref().and_then(|p| p.take()) {
            o.bar_ask = None;
            match answer {
                Ok(gm_hub_proto::player::PlayerResponse::Econ(
                    gm_hub_proto::player::PlayerEconReply::Bar(cells),
                )) => o.bar = cells,
                other => log::warn!("the item bar was not read: {other:?}"),
            }
        }
        if let Some(cells) = self.screens.bag.as_ref().and_then(|b| b.bar())
            && cells != o.bar.as_slice()
        {
            o.bar = cells.to_vec();
        }
    }
}
