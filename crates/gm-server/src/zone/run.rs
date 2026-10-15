//! The zone's run: everything the tick loop carries from one tick to the next, and the
//! run's start and finish.

use super::*;

/// The loop's state (one zone process; `run_with_web` drives it).
pub(super) struct Run {
    pub(super) cfg: ZoneConfig,
    pub(super) world: Arc<ZoneWorld>,
    pub(super) rate: TickRate,
    pub(super) rx: mpsc::Receiver<ClientEvent>,
    pub(super) event_tx: mpsc::Sender<ClientEvent>,
    pub(super) endpoint: quinn::Endpoint,
    pub(super) acceptor: tokio::task::JoinHandle<()>,
    pub(super) web_acceptor: Option<tokio::task::JoinHandle<()>>,
    pub(super) hub_stats: Arc<std::sync::Mutex<(u32, f32)>>,
    /// Hub bookkeeping per body.
    pub(super) hub_slots: BTreeMap<EntityId, HubSlot>,
    /// The gun hand each player was last seen with (MODES.md 3.7): a change is a `Look`.
    pub(super) hands_seen: HashMap<EntityId, u8>,
    /// Whether each player's weapon rests on its back (LOOK.md 6.7), and the tick it last
    /// fought or swung: a change of the first is a `Look`.
    pub(super) sheaths: HashMap<EntityId, bool>,
    pub(super) armed: HashMap<EntityId, Option<gm_core::tick::Tick>>,
    pub(super) revoked: Vec<(ModelId, Instant)>,
    /// The market: open stalls by id (the hub is their owner; this is its mirror).
    pub(super) stalls: BTreeMap<i64, StallSummary>,
    /// Readings of gear for characters that have no body here at the moment.
    pub(super) gear_kept: HashMap<CharacterId, (GearReading, Instant)>,
    /// The hub's parties as this zone has them (PARTY.md 2), the humans whose body does
    /// not carry its party's number yet because it is in a fight, and the requests to
    /// trade that wait for the other's: who asked whom, and when.
    pub(super) parties: ZoneParties,
    pub(super) party_waits: HashSet<EntityId>,
    pub(super) trade_asks: HashMap<EntityId, (EntityId, Instant)>,
    /// Whom a request to trade was told to, and when: once per pair in the time it waits.
    pub(super) trade_told: HashMap<(EntityId, EntityId), Instant>,
    /// Invitations for characters that are on their way here: told when the body is.
    pub(super) invites_kept: Vec<(CharacterId, String, Instant)>,
    /// Characters whose body left while it was in a fight, and that body: nobody who left
    /// a fight comes back into it (PARTY.md 2).
    pub(super) left_fights: HashMap<CharacterId, EntityId>,
    /// The content as loaded, and as a game master tuned it (GM.md 3).
    pub(super) tuning: Tuning,
    pub(super) pack: Arc<ContentPack>,
    pub(super) zone: Zone,
    pub(super) director: Director,
    /// The avatar models hired companions wear, and when their hires run out.
    pub(super) companion_models: BTreeMap<EntityId, ModelRef>,
    pub(super) hire_expiry: BTreeMap<EntityId, u64>,
    pub(super) squads_dirty: Vec<EntityId>,
    pub(super) started_unix: u64,
    pub(super) sessions: BTreeMap<EntityId, Session>,
    pub(super) pvs: PvsCache,
    pub(super) table: TickTable,
    pub(super) scheduler: TickScheduler,
    pub(super) metrics: TickMetrics,
    pub(super) phases: Phases,
    /// The recorder (ANTICHEAT.md 3) and what it measures as it records.
    pub(super) recorder: Option<Recorder>,
    pub(super) sight: crate::sight::Sight,
    pub(super) written_tx: mpsc::UnboundedSender<Written>,
    pub(super) written_rx: mpsc::UnboundedReceiver<Written>,
    pub(super) writing: Vec<tokio::task::JoinHandle<()>>,
    /// Characters whose leaving save is on its way to the hub.
    pub(super) leaving_saves: HashSet<CharacterId>,
    /// The zone's chat ceiling: lines it may still relay, and as of when.
    pub(super) chat_lines: f32,
    pub(super) chat_at: Instant,
    /// Bodies that left a moment ago can still be reported: entity, character, when.
    pub(super) recent_left: Vec<(EntityId, CharacterId, Instant)>,
    pub(super) report: ZoneReport,
    pub(super) last_report: Instant,
}

impl Run {
    /// The zone's setup: the listeners, the hub link, the content and its tuning, the
    /// minds, the recorder. The endpoint must already be bound.
    pub(super) fn start(
        cfg: ZoneConfig,
        world: Arc<ZoneWorld>,
        endpoint: quinn::Endpoint,
        web: Option<crate::net::WebListener>,
    ) -> Run {
        let rate = cfg.rate;
        let (tx, rx) = mpsc::channel::<ClientEvent>(EVENT_CHANNEL);
        let net_cfg = Arc::new(NetConfig {
            hz: rate.hz() as u16,
            map_name: world.name.clone(),
            map_hash: world.hash,
            open: cfg.open,
            props: Arc::new(cfg.looks.props.clone()),
            hub: cfg.hub.clone(),
            chat: Default::default(),
        });
        let web_acceptor =
            web.map(|w| tokio::spawn(crate::net::accept_loop_web(w, tx.clone(), net_cfg.clone())));
        let acceptor = tokio::spawn(accept_loop(endpoint.clone(), tx.clone(), net_cfg));
        let hub_stats: Arc<std::sync::Mutex<(u32, f32)>> =
            Arc::new(std::sync::Mutex::new((0, 0.0)));
        if let Some(link) = &cfg.hub {
            link.spawn_heartbeat(hub_stats.clone());
            link.spawn_notice_reader(tx.clone());
        }
        let event_tx = tx.clone();
        drop(tx);
        load_stalls(&cfg, &world, &event_tx);
        let (tuning, pack) = tuned_content(&cfg, rate);
        let mut zone = Zone::new(rate, cfg.seed, world.spawns.clone(), (*pack).clone());
        let director = minds(&cfg, &world, &mut zone);
        let recorder = recorder_of(&cfg, &world, rate);
        let (written_tx, written_rx) = mpsc::unbounded_channel::<Written>();
        upload_leftovers(&cfg);
        let now = Instant::now();
        Run {
            cfg,
            world,
            rate,
            rx,
            event_tx,
            endpoint,
            acceptor,
            web_acceptor,
            hub_stats,
            hub_slots: BTreeMap::new(),
            hands_seen: HashMap::new(),
            sheaths: HashMap::new(),
            armed: HashMap::new(),
            revoked: Vec::new(),
            stalls: BTreeMap::new(),
            gear_kept: HashMap::new(),
            parties: ZoneParties::default(),
            party_waits: HashSet::new(),
            trade_asks: HashMap::new(),
            trade_told: HashMap::new(),
            invites_kept: Vec::new(),
            left_fights: HashMap::new(),
            tuning,
            pack,
            zone,
            director,
            companion_models: BTreeMap::new(),
            hire_expiry: BTreeMap::new(),
            squads_dirty: Vec::new(),
            started_unix: now_secs(),
            sessions: BTreeMap::new(),
            pvs: PvsCache::default(),
            table: TickTable::default(),
            scheduler: TickScheduler::new(rate, now),
            metrics: TickMetrics::new(rate.period()),
            phases: Phases::default(),
            recorder,
            sight: crate::sight::Sight::default(),
            written_tx,
            written_rx,
            writing: Vec::new(),
            leaving_saves: HashSet::new(),
            chat_lines: ZONE_CHAT_BURST,
            chat_at: now,
            recent_left: Vec::new(),
            report: ZoneReport::default(),
            last_report: now,
        }
    }

    /// The zone stops: the last report, everyone saved (HUB.md 3.2), the replays written
    /// and sent, the sessions closed, the listeners gone.
    pub(super) async fn finish(mut self) -> ZoneReport {
        let secs = self.last_report.elapsed().as_secs_f64().max(1e-3);
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
        self.save_everyone().await;
        self.last_replays().await;
        for s in self.sessions.values() {
            s.send_control(FromZone::Kick("zone stopped".into()));
            s.conn.close(0, b"zone stopped");
        }
        // Fold the live sessions' counters into the totals.
        for p in self.zone.players() {
            self.report.executed_frames += p.executed_frames;
            self.report.starved_ticks += p.starved_ticks;
            self.report.dropped_frames += p.dropped_frames;
        }
        for s in self.sessions.values() {
            self.report.oversize_drops += s.oversize_drops;
            self.report.send_failures += s.send_failures;
        }
        if let Some(w) = &self.cfg.report_tx {
            let _ = w.send(self.report.clone());
        }
        self.endpoint.close(0u32.into(), b"zone stopped");
        self.acceptor.abort();
        // The web listener closes with its task: the sessions were closed one by one above.
        if let Some(task) = &self.web_acceptor {
            task.abort();
        }
        info!(ticks = self.report.tick, "zone stopped");
        self.report
    }

    /// Save everyone before the lights go out (HUB.md 3.2).
    async fn save_everyone(&self) {
        let Some(link) = &self.cfg.hub else {
            return;
        };
        for (&id, slot) in &self.hub_slots {
            if !slot.claimed_elsewhere
                && let Some(state) = character_state(&self.zone, link, id, slot)
            {
                let _ = link.save(slot.character, state, true).await;
            }
        }
    }

    /// What the recorder still holds is written before the zone goes, and everybody still
    /// here gets the line a leaver gets. The hub gets one try at each: a zone that stops
    /// does not wait an hour for a hub that is away, and what it could not send stays on
    /// disk for the next zone.
    async fn last_replays(&mut self) {
        let Some(rec) = &mut self.recorder else {
            return;
        };
        // Files still being written when the loop ended.
        for task in self.writing.drain(..) {
            let _ = task.await;
        }
        let mut last: Vec<Written> = Vec::new();
        while let Ok(w) = self.written_rx.try_recv() {
            last.push(w);
        }
        for finished in rec.flush() {
            match finished.write() {
                Ok(w) => last.push(w),
                Err(e) => warn!("writing a replay: {e}"),
            }
        }
        // Sent side by side: each has its own five seconds, and the zone waits for the
        // slowest of them, not for their sum.
        let mut last_words = Vec::new();
        for w in last {
            replay_written(&w);
            if let Some(link) = &self.cfg.hub {
                let link = link.clone();
                last_words.push(tokio::spawn(async move { link.replay_once(w).await }));
            }
        }
        // Everybody still here, and the bodies waiting for another zone to claim them.
        let present: std::collections::BTreeSet<EntityId> = self
            .sessions
            .keys()
            .chain(self.hub_slots.keys())
            .copied()
            .collect();
        for id in present {
            let name = rec.entry(id).map_or_else(String::new, |e| e.name.clone());
            let aim = rec.analyser.take(id);
            info!(entity = id, name = %name, "aim at leave: {}", aim.line());
            if let (Some(link), Some(slot)) = (&self.cfg.hub, self.hub_slots.get(&id))
                && !aim.is_empty()
            {
                let (link, character) = (link.clone(), slot.character);
                last_words.push(tokio::spawn(
                    async move { link.aim_once(character, aim).await },
                ));
            }
        }
        for task in last_words {
            let _ = task.await;
        }
    }

    /// The body a character has here.
    pub(super) fn body_of(&self, character: CharacterId) -> Option<EntityId> {
        self.hub_slots
            .iter()
            .find(|(_, s)| s.character == character)
            .map(|(id, _)| *id)
    }
}

/// The stalls of a market zone, asked of the hub once; they arrive as a `StallsLoaded`.
fn load_stalls(cfg: &ZoneConfig, world: &ZoneWorld, event_tx: &mpsc::Sender<ClientEvent>) {
    let Some(link) = cfg.hub.clone().filter(|_| !world.stall_grids.is_empty()) else {
        return;
    };
    let tx = event_tx.clone();
    tokio::spawn(async move {
        match link.stalls().await {
            Ok(list) => {
                let _ = tx.send(ClientEvent::StallsLoaded(list)).await;
            }
            Err(e) => warn!("loading the zone's stalls: {e}"),
        }
    });
}

/// The content as loaded, and as a game master tuned it (GM.md 3): the tuning of the last
/// run is read back; one that content refuses is dropped with a warning.
fn tuned_content(cfg: &ZoneConfig, rate: TickRate) -> (Tuning, Arc<ContentPack>) {
    let mut tuning = match &cfg.tuning_file {
        Some(path) => match gm_content::tuning::read(path) {
            Ok(t) => t,
            Err(e) => {
                warn!(path = %path.display(), "tuning not read: {e}");
                Tuning::default()
            }
        },
        None => Tuning::default(),
    };
    let mut pack = Arc::new(tuning.apply(&cfg.content, rate));
    if let Err(e) = pack.validate(rate) {
        warn!("the tuning read is not valid content, dropped: {e}");
        tuning = Tuning::default();
        pack = Arc::new(cfg.content.clone());
    }
    if !tuning.is_default() {
        info!(
            tempo = tuning.tempo,
            abilities = tuning.abilities.len(),
            "tuned content"
        );
    }
    (tuning, pack)
}

/// Minds (COMPANIONS.md): the nav grid, the creatures on their posts, the squads.
fn minds(cfg: &ZoneConfig, world: &ZoneWorld, zone: &mut Zone) -> Director {
    let seeds: Vec<glam::Vec3> = world.spawns.iter().map(|s| s.origin).collect();
    let posts: Vec<CreatureSpawn> = if cfg.wild {
        world
            .creature_posts
            .iter()
            .map(|p| CreatureSpawn {
                creature: p.creature.clone(),
                encounter: p.encounter.clone(),
                origin: p.origin,
                yaw: p.yaw,
            })
            .collect()
    } else {
        Vec::new()
    };
    let minded = cfg.squads || !posts.is_empty();
    let nav_started = std::time::Instant::now();
    let mut director = Director::new(
        zone,
        &world.bsp,
        &world.name,
        if minded { &seeds } else { &[] },
        &posts,
        cfg.seed,
    );
    if minded {
        info!(
            nav_nodes = director.nav.len(),
            nav_ms = format_args!("{:.1}", nav_started.elapsed().as_secs_f64() * 1000.0),
            creatures = director.minds(),
            "minds ready"
        );
    }
    director.events.clear();
    director
}

/// The recorder (ANTICHEAT.md 3), when the zone keeps replays.
fn recorder_of(cfg: &ZoneConfig, world: &ZoneWorld, rate: TickRate) -> Option<Recorder> {
    cfg.replay.as_ref().map(|r| {
        Recorder::new(RecorderConfig {
            dir: r.dir.clone(),
            zone: r.zone.clone(),
            map: world.name.clone(),
            map_hash: world.hash,
            hz: rate.hz() as u16,
            teams: !cfg.wild,
            content_hash: gm_net::fnv1a64(&bitcode::encode(&cfg.content)),
            bytes_per_hour: r.bytes_per_hour,
        })
    })
}

/// What an earlier zone left in the replay directory goes to the hub now: listed before
/// this zone writes anything of its own, read and sent one at a time.
fn upload_leftovers(cfg: &ZoneConfig) {
    let (Some(link), Some(r)) = (&cfg.hub, &cfg.replay) else {
        return;
    };
    let (link, left) = (link.clone(), Written::leftovers(&r.dir));
    if left.is_empty() {
        return;
    }
    tokio::spawn(async move {
        for path in left {
            match tokio::task::spawn_blocking(move || Written::read(path)).await {
                Ok(Ok(w)) => {
                    info!(file = %w.path.display(), "a replay left by an earlier zone");
                    link.replay(w).await;
                }
                Ok(Err(e)) => warn!("a leftover replay is not uploaded: {e}"),
                Err(_) => {}
            }
        }
    });
}
