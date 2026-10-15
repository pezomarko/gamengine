//! One bot: connect, handshake, receive the content, then a fixed-step loop of predicted
//! inputs and snapshots.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use gm_bsp::Bsp;
use gm_core::build::Sheet;
use gm_core::sim::Input;
use gm_core::tick::TickRate;
use gm_core::vocab::EntityId;
use gm_net::PROTOCOL_VERSION;
use gm_net::client::{ClientState, ClientStats, RenderEntity};
use gm_net::control::ControlError;
use gm_net::control::WebAddr;
use gm_net::control::{self, BodyKind, BuildChoice, EncounterState, FromClient, FromZone};
use gm_net::link::{Link, LinkError, RecvHalf, SendHalf, web_connect};
use gm_net::transport::{SERVER_NAME, web_transport_config};
use tokio::time::Instant;
use tracing::{debug, info};

use crate::brain::{Behaviour, Brain, View, counter_pick, dominant_enemy_aspects};
use crate::raid::Raid;

/// Counter-picking bots look at the enemy every this many seconds (after the first look).
pub const COUNTER_PICK_PERIOD_S: u32 = 10;

#[derive(Clone, Debug)]
pub struct BotConfig {
    pub name: String,
    pub seed: u64,
    pub behaviour: Behaviour,
    pub rate: TickRate,
    /// Stop after this many client ticks (0 = run until `shutdown`).
    pub run_ticks: u32,
    /// Preset build to ask for (`None`: the zone's default).
    pub build: Option<String>,
    /// Team to ask for (0 = let the zone balance).
    pub team: u8,
    /// Re-spec to the preset that counters the enemy's aspects (MATRIX.md 11).
    pub counter_pick: bool,
    /// Ask to travel to this zone after `travel_after_ticks` (hub flows only).
    pub travel_to: Option<String>,
    pub travel_after_ticks: u32,
    /// Walk to this tile of the map's market (counted across its grids) and open a stall
    /// there, or on the next free one (ECONOMY.md 7).
    pub stall_tile: Option<u32>,
    /// How the view moves (ANTICHEAT.md 10): the brain's wish at once, a hand, or a cheat.
    pub aim: crate::aim::AimModel,
    /// Report the first enemy seen after this many ticks (0 = never; ANTICHEAT.md 5).
    pub report_after_ticks: u32,
    /// Say this line in the zone's chat every so many seconds, at the zone's own tick
    /// rate (CLIENT.md 5).
    pub say: Option<(String, f32)>,
    /// What the bot does about other people (PARTY.md 9).
    pub social: Social,
}

/// A bot as a person among people: it can ask somebody into a party, join whoever asks
/// it, trade with whoever asks, and answer what is said to it.
#[derive(Clone, Debug, Default)]
pub struct Social {
    /// Ask the character of this name into a party, again every few seconds until the
    /// zone says the two are in one.
    pub invite: Option<String>,
    /// Join whoever invites, ask back whoever asks to trade, answer a party's line with
    /// a line and a whisper with a whisper.
    pub sociable: bool,
    /// Where the trades the zone says the hub opened are handed to (the bot's hub
    /// session sits elsewhere: `hub_flow`).
    pub trades: Option<tokio::sync::mpsc::UnboundedSender<i64>>,
}

/// Why the bot loop ended.
#[derive(Clone, Debug)]
pub enum BotExit {
    Done,
    /// The zone handed over a ticket for another zone.
    Travel(Box<gm_hub_proto::protocol::ZoneTicket>),
}

#[derive(Clone, Debug, Default)]
pub struct BotReport {
    pub name: String,
    pub entity: EntityId,
    pub team: u8,
    pub ticks: u32,
    pub secs: f64,
    pub client: ClientStats,
    pub udp_tx_bytes: u64,
    pub udp_rx_bytes: u64,
    pub rtt_ms: f64,
    pub kills_seen: u32,
    pub deaths_seen: u32,
    pub own_kills: u32,
    pub own_deaths: u32,
    pub final_health: i32,
    pub send_failures: u64,
    pub others_seen_max: usize,
    /// Re-specs requested and the build the bot ended with.
    pub respecs: u32,
    pub final_build: String,
    /// Avatar models the zone announced (MODELS.md 7): the most distinct ids worn at once,
    /// the own one at the end, the revocations heard, and whether anybody was still
    /// announced wearing a revoked model afterwards.
    pub models_seen: usize,
    pub own_model: Option<[u8; 32]>,
    pub revocations: u32,
    pub revoked_still_worn: bool,
    /// The most players the zone listed at once (roster and joins minus leaves).
    pub roster: usize,
    /// The bot opened a stall; and the stalls the zone showed when it left.
    pub stall_opened: bool,
    pub stalls_seen: usize,
    /// Ground covered, in world units (a bot that never moves is stuck).
    pub travelled: f32,
    /// A raid leader's run (COMPANIONS.md 14): the most companions it commanded at once,
    /// how many of them were hired rather than lent, the orders it gave and the ones the
    /// zone refused, the encounters it was told were cleared (name, seconds) and reset,
    /// what a kill gave it, the trials judged on it (key, passed, why not), and whether it
    /// finished the map.
    /// The party as the zone last said it (the hub's word), and the lines heard through
    /// the hub: channel, from whom, what.
    pub party: Vec<String>,
    pub lines: Vec<(u8, String, String)>,
    pub squad_max: usize,
    pub squad_hired: usize,
    pub orders: u32,
    pub orders_refused: u32,
    pub cleared: Vec<(String, u32)>,
    pub resets: u32,
    pub loot: Vec<String>,
    pub coin: u32,
    pub trials: Vec<(String, bool, String)>,
    pub raid_done: bool,
    /// The zone took this bot's report (ANTICHEAT.md 5).
    pub report_accepted: bool,
    /// Chat lines heard, the first 32 of them: the speaker's entity (0 = the zone) and
    /// the text.
    pub heard: Vec<(u32, String)>,
    /// The zone kicked the bot: why.
    pub kicked: Option<String>,
    /// Bodies the zone announced as creatures, and the most seen with their health.
    pub creatures_announced: usize,
    pub creature_health_seen: usize,
}

impl BotReport {
    pub fn tx_bytes_per_s(&self) -> f64 {
        self.udp_tx_bytes as f64 / self.secs.max(1e-3)
    }

    pub fn rx_bytes_per_s(&self) -> f64 {
        self.udp_rx_bytes as f64 / self.secs.max(1e-3)
    }
}

/// Connect to `server` through `endpoint` (which must trust the zone's certificate) and play.
pub async fn run_bot(
    endpoint: &quinn::Endpoint,
    server: SocketAddr,
    cfg: BotConfig,
    world: Arc<Bsp>,
    shutdown: impl Future<Output = ()>,
) -> anyhow::Result<BotReport> {
    let (report, _, _) = run_bot_with_token(
        endpoint,
        server,
        cfg,
        Vec::new(),
        move |_| Ok(world.clone()),
        shutdown,
    )
    .await?;
    Ok(report)
}

/// Like [`run_bot`] with a hub token; the map is loaded by name from `Welcome` through
/// `load_map`. Returns the report, why the loop ended and the map name.
pub async fn run_bot_with_token(
    endpoint: &quinn::Endpoint,
    server: SocketAddr,
    cfg: BotConfig,
    token: Vec<u8>,
    load_map: impl Fn(&str) -> anyhow::Result<Arc<Bsp>>,
    shutdown: impl Future<Output = ()>,
) -> anyhow::Result<(BotReport, BotExit, String)> {
    let conn = endpoint.connect(server, SERVER_NAME)?.await?;
    run_bot_on_link(Link::Quic(conn), cfg, token, load_map, shutdown).await
}

/// The same bot through a zone's WebTransport listener (WEB.md 7): what a browser's
/// session carries, without a browser.
pub async fn run_bot_web(
    web: &WebAddr,
    cfg: BotConfig,
    token: Vec<u8>,
    load_map: impl Fn(&str) -> anyhow::Result<Arc<Bsp>>,
    shutdown: impl Future<Output = ()>,
) -> anyhow::Result<(BotReport, BotExit, String)> {
    let (endpoint, conn) = web_connect(web, web_transport_config()).await?;
    let result = run_bot_on_link(conn, cfg, token, load_map, shutdown).await;
    endpoint.wait_idle().await;
    result
}

/// The bot on an established connection of either transport.
pub async fn run_bot_on_link(
    conn: Link,
    cfg: BotConfig,
    token: Vec<u8>,
    load_map: impl Fn(&str) -> anyhow::Result<Arc<Bsp>>,
    shutdown: impl Future<Output = ()>,
) -> anyhow::Result<(BotReport, BotExit, String)> {
    let mut run = BotRun::join(conn, cfg, token, load_map).await?;
    let mut shutdown = std::pin::pin!(shutdown);
    loop {
        let sleep = tokio::time::sleep_until(run.next);
        let wake = tokio::select! {
            _ = &mut shutdown => Wake::Stop,
            _ = sleep => Wake::Tick,
            dg = run.conn.read_datagram() => Wake::Datagram(dg),
            msg = control::recv(&mut run.recv) => Wake::Message(msg),
        };
        let flow = match wake {
            Wake::Stop => Flow::Stop,
            Wake::Tick => run.tick().await,
            Wake::Datagram(dg) => run.on_datagram(dg),
            Wake::Message(msg) => run.on_message(msg).await,
        };
        if flow == Flow::Stop {
            break;
        }
    }
    Ok(run.finish().await)
}

/// What woke the loop.
enum Wake {
    Stop,
    Tick,
    Datagram(Result<Bytes, LinkError>),
    Message(Result<Option<FromZone>, ControlError>),
}

/// Whether the loop goes on after what it handled.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Flow {
    Go,
    Stop,
}

/// A bot's run in one zone: everything the loop carries from one wake to the next.
struct BotRun {
    conn: Link,
    cfg: BotConfig,
    send: SendHalf,
    recv: RecvHalf,
    entity: EntityId,
    rate: TickRate,
    map_name: String,
    world: Arc<Bsp>,
    exit: BotExit,
    pack: gm_core::build::ContentPack,
    team: u8,
    /// What it says and how often, in this zone's ticks.
    say: Option<(String, u32)>,
    current_build: String,
    client: ClientState,
    brain: Brain,
    aimer: crate::aim::Aimer,
    reported: bool,
    /// A raid leader thinks with `gm_ai` instead: the nav grid is flooded here, once.
    raid: Option<Raid>,
    /// The tick the raid was finished at: the loop runs two seconds more, for the last
    /// kill's loot and verdicts to arrive.
    raid_done_at: Option<u32>,
    /// The market's tiles; a vendor walks to its tile, a taken tile sends it to the next.
    tiles: Vec<glam::Vec3>,
    stall_tile: Option<u32>,
    /// Where the stalls of the market stand, by id (the roster at the join, then each one
    /// opened or closed): a vendor whose stall stands from before walks to it, and one
    /// without a stall yet walks to the first tile nobody stands on, not to the first tile
    /// (2026-10-14, the director: "merchants are all trying to get to first slot and not
    /// their own"). Two tiles are the same spot within half the grid's pitch (`near`).
    stall_spots: std::collections::HashMap<i64, glam::Vec3>,
    near: f32,
    /// The tick of the last stall request, and whether the zone has answered it.
    stall_asked: Option<u32>,
    stall_answered: bool,
    stalls_seen: std::collections::HashSet<i64>,
    /// Who is here and what they wear, as the zone announces it.
    wearing: std::collections::HashMap<u32, Option<[u8; 32]>>,
    revoked: Vec<[u8; 32]>,
    own_model: Option<[u8; 32]>,
    next_pick: u32,
    report: BotReport,
    start: Instant,
    period: Duration,
    next: Instant,
    ticks: u32,
}

impl BotRun {
    /// The handshake: `Hello`, `Welcome`, `Content`; the map loaded; the brain, the aim and
    /// the market's tiles made ready.
    async fn join(
        conn: Link,
        cfg: BotConfig,
        token: Vec<u8>,
        load_map: impl Fn(&str) -> anyhow::Result<Arc<Bsp>>,
    ) -> anyhow::Result<BotRun> {
        let (mut send, mut recv) = conn.open_bi().await?;
        control::send(
            &mut send,
            &FromClient::Hello {
                version: PROTOCOL_VERSION as u16,
                name: cfg.name.clone(),
                token,
                build: cfg.build.clone().map(BuildChoice::Preset),
                team: cfg.team,
            },
        )
        .await?;
        let welcome = control::recv(&mut recv)
            .await?
            .ok_or_else(|| anyhow::anyhow!("server closed before Welcome"))?;
        let (entity, hz, map_name) = match welcome {
            FromZone::Welcome {
                entity, hz, map, ..
            } => (entity, hz, map),
            FromZone::Reject(reason) => anyhow::bail!("rejected: {reason}"),
            other => anyhow::bail!("unexpected handshake message {other:?}"),
        };
        let world = load_map(&map_name)?;
        let content = control::recv(&mut recv)
            .await?
            .ok_or_else(|| anyhow::anyhow!("server closed before Content"))?;
        let (pack, own, team) = match content {
            FromZone::Content {
                pack, own, team, ..
            } => (pack, own, team),
            other => anyhow::bail!("expected Content, got {other:?}"),
        };
        let rate = TickRate::new(hz as u32);
        info!(name = %cfg.name, entity, hz, team, "bot joined");
        let say = cfg
            .say
            .as_ref()
            .map(|(line, secs)| (line.clone(), ((secs * hz as f32).round() as u32).max(1)));
        let current_build = build_name(&pack, &own);
        let client = ClientState::new(entity, rate, Sheet::new(own, &pack, team));
        let mut brain = Brain::new(cfg.seed, cfg.behaviour);
        brain.hz = rate.hz();
        let aimer = crate::aim::Aimer::new(cfg.aim, cfg.seed);
        let raid = (cfg.behaviour == Behaviour::Raid).then(|| Raid::new(&world, cfg.seed));
        let tiles: Vec<glam::Vec3> = world
            .stall_grids()
            .iter()
            .flat_map(|g| {
                (0..g.rows).flat_map(move |row| {
                    (0..g.cols).filter_map(move |col| g.centre(g.base_x + col, g.base_y + row))
                })
            })
            .collect();
        let stall_tile = cfg.stall_tile.filter(|_| !tiles.is_empty());
        let near = tiles
            .get(1)
            .map_or(32.0, |t| (*t - tiles[0]).truncate().length() * 0.5);
        if let Some(t) = stall_tile {
            brain.goal = Some(tiles[t as usize % tiles.len()]);
        }
        let report = BotReport {
            name: cfg.name.clone(),
            entity,
            team,
            ..Default::default()
        };
        let start = Instant::now();
        Ok(BotRun {
            conn,
            send,
            recv,
            entity,
            rate,
            map_name,
            world,
            exit: BotExit::Done,
            pack,
            team,
            say,
            current_build,
            client,
            brain,
            aimer,
            reported: false,
            raid,
            raid_done_at: None,
            tiles,
            stall_tile,
            stall_spots: Default::default(),
            near,
            stall_asked: None,
            stall_answered: true,
            stalls_seen: Default::default(),
            wearing: Default::default(),
            revoked: Vec::new(),
            own_model: None,
            next_pick: COUNTER_PICK_PERIOD_S * rate.hz(),
            report,
            start,
            period: rate.period(),
            next: start,
            ticks: 0,
            cfg,
        })
    }

    /// One client tick: a thought, the view aimed, what it has to say and ask, the books,
    /// the step and its datagram.
    async fn tick(&mut self) -> Flow {
        self.ticks += 1;
        let t = self.client.render_tick(0.0);
        let others = self.client.others_at(t);
        self.report.others_seen_max = self.report.others_seen_max.max(
            others
                .iter()
                .filter(|e| e.kind == gm_net::snapshot::EntityKind::Player)
                .count(),
        );
        let input = self.think(&others).await;
        // The view goes where this bot's aim model takes it.
        let input = self.aimer.apply(
            input,
            &self.client,
            self.world.as_ref(),
            self.team,
            &others,
            self.client.tick.wrapping_add(1),
        );
        self.speak(&others).await;
        self.ask().await;
        self.account();
        self.step(t, input);
        if self.cfg.run_ticks != 0 && self.ticks >= self.cfg.run_ticks {
            return Flow::Stop;
        }
        if self
            .raid_done_at
            .is_some_and(|at| self.ticks >= at + 2 * self.rate.hz())
        {
            return Flow::Stop;
        }
        Flow::Go
    }

    /// What the body does this tick: the raid leader's mind, or the brain's.
    async fn think(&mut self, others: &[RenderEntity]) -> Input {
        let Some(raid) = self.raid.as_mut() else {
            return self.brain.think(&View {
                me: &self.client.mover,
                kit: &self.client.sheet.kit,
                frame: self.client.sheet.build.frame,
                team: self.team,
                alive: self.client.own_alive,
                health: self.client.own_health,
                others,
                tick: self.client.tick.wrapping_add(1),
            });
        };
        self.report.creature_health_seen = self.report.creature_health_seen.max(
            others
                .iter()
                .filter(|e| {
                    e.health.is_some()
                        && matches!(raid.kinds.get(&e.id), Some(BodyKind::Creature { .. }))
                })
                .count(),
        );
        let (input, order) = raid.think(&self.client, others, &self.world, &self.pack, self.team);
        if let Some(order) = order {
            self.report.orders += 1;
            let _ = control::send(&mut self.send, &order).await;
        }
        if raid.done() && self.raid_done_at.is_none() {
            info!(name = %self.cfg.name, ticks = self.ticks, "raid done");
            self.raid_done_at = Some(self.ticks);
            self.report.raid_done = true;
        }
        input
    }

    /// What it says to the zone this tick: its line, a report of the first enemy seen, the
    /// travel it was told to ask for, a counter-pick.
    async fn speak(&mut self, others: &[RenderEntity]) {
        if let Some((line, every)) = &self.say
            && self.ticks.is_multiple_of(*every)
        {
            let _ = control::send(&mut self.send, &FromClient::Chat(line.clone())).await;
        }
        if self.cfg.report_after_ticks > 0
            && !self.reported
            && self.ticks >= self.cfg.report_after_ticks
            && let Some(enemy) = others.iter().find(|e| {
                e.kind == gm_net::snapshot::EntityKind::Player
                    && e.alive()
                    && (self.team == 0 || e.team() != self.team)
            })
        {
            self.reported = true;
            info!(name = %self.cfg.name, target = enemy.id, "reporting");
            let report = FromClient::Report {
                target: enemy.id,
                reason: gm_net::control::ReportReason::Aim,
            };
            let _ = control::send(&mut self.send, &report).await;
        }
        if let Some(to) = &self.cfg.travel_to
            && self.cfg.travel_after_ticks > 0
            && self.ticks == self.cfg.travel_after_ticks
        {
            info!(name = %self.cfg.name, %to, "asking to travel");
            let _ = control::send(&mut self.send, &FromClient::Travel(to.clone())).await;
        }
        if self.cfg.counter_pick && self.ticks >= self.next_pick {
            self.next_pick = self.ticks + COUNTER_PICK_PERIOD_S * self.rate.hz();
            let enemy = dominant_enemy_aspects(self.team, others);
            if let Some(name) = counter_pick(&self.pack, &self.current_build, enemy) {
                info!(name = %self.cfg.name, from = %self.current_build, to = %name, "counter-pick");
                let _ = control::send(
                    &mut self.send,
                    &FromClient::Respec(BuildChoice::Preset(name)),
                )
                .await;
                self.report.respecs += 1;
            }
        }
    }

    /// What it asks for: a stall on its tile (the zone drops a request that comes within a
    /// second of the last one: after a refusal the next one waits that second out, and a
    /// request that got no answer is repeated after three), and somebody into a party,
    /// every five seconds until the zone says the two are in one.
    async fn ask(&mut self) {
        let hz = self.rate.hz();
        let wait = if self.stall_answered {
            hz + hz / 8
        } else {
            3 * hz
        };
        if let Some(goal) = self.brain.goal
            && !self.report.stall_opened
            && self.stall_asked.is_none_or(|t| self.ticks - t > wait)
            && self.client.own_alive
            && (self.client.mover.mv.origin - goal)
                .truncate()
                .abs()
                .max_element()
                < 56.0
        {
            self.stall_asked = Some(self.ticks);
            self.stall_answered = false;
            let _ = control::send(&mut self.send, &FromClient::StallOpen).await;
        }
        if let Some(who) = &self.cfg.social.invite
            && self.ticks % (5 * hz) == hz
            && !self.report.party.iter().any(|m| m == who)
        {
            let ask = FromClient::PartyInvite { name: who.clone() };
            let _ = control::send(&mut self.send, &ask).await;
        }
    }

    /// The books of what the zone has announced so far.
    fn account(&mut self) {
        self.report.roster = self.report.roster.max(self.wearing.len());
        if self.ticks.is_multiple_of(16) {
            let distinct: std::collections::HashSet<&[u8; 32]> =
                self.wearing.values().flatten().collect();
            self.report.models_seen = self.report.models_seen.max(distinct.len());
            self.report.revoked_still_worn |= self
                .wearing
                .values()
                .flatten()
                .any(|m| self.revoked.contains(m));
            self.own_model = self.wearing.get(&self.entity).copied().flatten();
        }
    }

    /// The local tick and its datagram; the next deadline (never catching up more than a
    /// second: the timeline is reset instead).
    fn step(&mut self, t: f32, input: Input) {
        let before = self.client.mover.mv.origin;
        let datagram = self.client.local_tick(self.world.as_ref(), input);
        let step = (self.client.mover.mv.origin - before).truncate().length();
        // A correction or a respawn is not walking.
        if step < 32.0 {
            self.report.travelled += step;
        }
        if self
            .conn
            .send_datagram(Bytes::from(datagram.encode()))
            .is_err()
        {
            self.report.send_failures += 1;
        }
        self.client.prune(t);
        self.next += self.period;
        let now = Instant::now();
        if now.saturating_duration_since(self.next) > Duration::from_secs(1) {
            self.next = now;
        }
    }

    /// A snapshot, or the connection's end.
    fn on_datagram(&mut self, dg: Result<Bytes, LinkError>) -> Flow {
        match dg {
            Ok(bytes) => {
                if let Err(e) = self.client.on_snapshot(self.world.as_ref(), &bytes) {
                    debug!(name = %self.cfg.name, "snapshot dropped: {e}");
                }
                Flow::Go
            }
            Err(e) => {
                info!(name = %self.cfg.name, "connection closed: {e}");
                Flow::Stop
            }
        }
    }

    /// A control message from the zone.
    async fn on_message(&mut self, msg: Result<Option<FromZone>, ControlError>) -> Flow {
        let msg = match msg {
            Ok(Some(msg)) => msg,
            Ok(None) | Err(_) => return Flow::Stop,
        };
        match msg {
            FromZone::Killed { victim, killer } => {
                self.report.kills_seen += 1;
                if victim == self.entity {
                    self.report.own_deaths += 1;
                }
                if killer == self.entity {
                    self.report.own_kills += 1;
                }
            }
            FromZone::BuildApplied(build) => {
                if build != self.client.sheet.build {
                    self.current_build = build_name(&self.pack, &build);
                    self.client
                        .set_sheet(Sheet::new(build, &self.pack, self.team));
                }
            }
            // Content tuned under the zone (GM.md 3): the same build on the new numbers;
            // the prediction goes on.
            FromZone::Content { pack, own, .. } => {
                self.pack = pack;
                self.current_build = build_name(&self.pack, &own);
                self.client
                    .set_sheet(Sheet::new(own, &self.pack, self.team));
            }
            FromZone::TravelTicket {
                zone,
                addr,
                cert_der,
                token,
                web,
            } => return self.on_travel_ticket(zone, addr, cert_der, token, web),
            FromZone::TravelRefused(reason) => {
                info!(name = %self.cfg.name, "travel refused: {reason}");
            }
            FromZone::Kick(reason) => {
                info!(name = %self.cfg.name, "kicked: {reason}");
                self.report.kicked = Some(reason);
                return Flow::Stop;
            }
            FromZone::StallResult(result) => self.on_stall_result(result),
            FromZone::Stalls(list) => self.on_stalls(list),
            FromZone::StallOpened(stall) => self.on_stall_opened(stall),
            FromZone::StallClosed(id) => {
                self.stalls_seen.remove(&id);
                self.stall_spots.remove(&id);
            }
            // People together (PARTY.md 4): the party as the hub has it, and what a
            // sociable bot does when it is asked something.
            FromZone::Party(names) => {
                info!(name = %self.cfg.name, party = %names.join(","), "party");
                self.report.party = names;
            }
            FromZone::Invited { from } => {
                info!(name = %self.cfg.name, %from, "invited");
                if self.cfg.social.sociable {
                    let join = FromClient::PartyAnswer { from, join: true };
                    let _ = control::send(&mut self.send, &join).await;
                }
            }
            FromZone::Heard {
                channel,
                from,
                text,
            } => self.on_heard(channel, from, text).await,
            FromZone::TradeAsked { from } => {
                info!(name = %self.cfg.name, from, "asked to trade");
                if self.cfg.social.sociable {
                    let back = FromClient::TradeAsk { with: from };
                    let _ = control::send(&mut self.send, &back).await;
                }
            }
            FromZone::TradeOpened { trade, with } => {
                info!(name = %self.cfg.name, trade, %with, "trade opened");
                if let Some(trades) = &self.cfg.social.trades {
                    let _ = trades.send(trade);
                }
            }
            FromZone::ReportResult(result) => {
                info!(name = %self.cfg.name, ?result, "report answered");
                self.report.report_accepted = result.is_ok();
            }
            FromZone::ChatFrom { from, text } => {
                if from != self.entity && self.report.heard.len() < 32 {
                    // Said as it is heard: whoever scripted the run need not wait for the
                    // bot's end to know.
                    info!(name = %self.cfg.name, from, %text, "heard");
                    self.report.heard.push((from, text));
                }
            }
            FromZone::Roster(players) => self.on_roster(players),
            FromZone::PlayerInfo {
                id, model, kind, ..
            } => {
                if matches!(kind, BodyKind::Creature { .. }) && !self.wearing.contains_key(&id) {
                    self.report.creatures_announced += 1;
                }
                if let Some(raid) = self.raid.as_mut() {
                    raid.kinds.insert(id, kind);
                }
                self.wearing.insert(id, model);
            }
            FromZone::PlayerLeft(id) => {
                if let Some(raid) = self.raid.as_mut() {
                    raid.kinds.remove(&id);
                }
                self.wearing.remove(&id);
            }
            FromZone::Squad(entries) => {
                self.report.squad_max = self.report.squad_max.max(entries.len());
                self.report.squad_hired = self
                    .report
                    .squad_hired
                    .max(entries.iter().filter(|e| !e.recruit).count());
                if let Some(raid) = self.raid.as_mut() {
                    raid.squad = entries;
                }
            }
            FromZone::OrderRefused(why) => {
                self.report.orders_refused += 1;
                debug!(name = %self.cfg.name, "order refused: {why}");
            }
            FromZone::Encounter { name, state } => {
                info!(name = %self.cfg.name, encounter = %name, ?state, "encounter");
                match state {
                    EncounterState::Cleared { secs } => self.report.cleared.push((name, secs)),
                    EncounterState::Reset => self.report.resets += 1,
                    EncounterState::Engaged => {}
                }
            }
            FromZone::Loot { items, coin, .. } => {
                info!(name = %self.cfg.name, ?items, coin, "loot");
                self.report.loot.extend(items);
                self.report.coin += coin;
            }
            FromZone::Trial {
                key,
                passed,
                detail,
                secs,
                ..
            } => {
                info!(name = %self.cfg.name, trial = %key, passed, secs, %detail, "trial");
                self.report.trials.push((key, passed, detail));
            }
            FromZone::ModelRevoked(model) => {
                self.report.revocations += 1;
                self.revoked.push(model);
                for m in self.wearing.values_mut() {
                    if *m == Some(model) {
                        *m = None;
                    }
                }
            }
            _ => {}
        }
        Flow::Go
    }

    /// A ticket to another zone: the run ends with it (`BotExit::Travel`); a ticket that
    /// cannot be read is ignored.
    fn on_travel_ticket(
        &mut self,
        zone: String,
        addr: String,
        cert_der: Vec<u8>,
        token: Vec<u8>,
        web: Option<WebAddr>,
    ) -> Flow {
        let addr: SocketAddr = match addr.parse() {
            Ok(a) => a,
            Err(e) => {
                info!(name = %self.cfg.name, "bad travel address {addr}: {e}");
                return Flow::Go;
            }
        };
        let Ok(token) = bitcode::decode(&token) else {
            info!(name = %self.cfg.name, "bad travel token");
            return Flow::Go;
        };
        info!(name = %self.cfg.name, %zone, %addr, "travel ticket");
        self.exit = BotExit::Travel(Box::new(gm_hub_proto::protocol::ZoneTicket {
            zone,
            addr,
            cert_der,
            token,
            web,
        }));
        Flow::Stop
    }

    /// The zone's answer to a stall request: opened, standing from before the restart (the
    /// bot keeps it rather than asking every second), or refused (somebody was faster: the
    /// next tile).
    fn on_stall_result(&mut self, result: Result<(), String>) {
        self.stall_answered = true;
        match result {
            Ok(()) => {
                self.report.stall_opened = true;
                info!(name = %self.cfg.name, "stall opened");
            }
            Err(why) if why.contains("already have a stall") => {
                self.report.stall_opened = true;
                info!(name = %self.cfg.name, "stall stands from before");
            }
            Err(why) => {
                debug!(name = %self.cfg.name, "stall refused: {why}");
                if let Some(t) = self.stall_tile.as_mut() {
                    *t += 1;
                    self.brain.goal = Some(self.tiles[*t as usize % self.tiles.len()]);
                }
            }
        }
    }

    /// The market as it stands at the join: the bot's own stall if it has one, else the
    /// first free tile.
    fn on_stalls(&mut self, list: Vec<control::StallEntry>) {
        self.stalls_seen = list.iter().map(|s| s.id).collect();
        self.stall_spots = list
            .iter()
            .map(|s| (s.id, glam::Vec3::from(s.pos)))
            .collect();
        if let Some(own) = list.iter().find(|s| s.owner == self.cfg.name) {
            self.brain.goal = Some(glam::Vec3::from(own.pos));
            self.report.stall_opened = true;
            info!(name = %self.cfg.name, stall = own.id, "stall stands from before");
        } else if let Some(t) = self.stall_tile.as_mut()
            && !self.report.stall_opened
            && let Some(free) = free_tile(&self.tiles, *t, &self.stall_spots, self.near)
        {
            *t = free;
            self.brain.goal = Some(self.tiles[free as usize]);
        }
    }

    /// A stall opened: somebody else's on the tile this bot was walking to sends it to the
    /// next free one; its own is where it stands, for whoever sends it a buyer.
    fn on_stall_opened(&mut self, stall: control::StallEntry) {
        self.stalls_seen.insert(stall.id);
        self.stall_spots
            .insert(stall.id, glam::Vec3::from(stall.pos));
        if stall.owner != self.cfg.name
            && !self.report.stall_opened
            && let Some(t) = self.stall_tile.as_mut()
            && self
                .brain
                .goal
                .is_some_and(|g| (g - glam::Vec3::from(stall.pos)).truncate().length() < self.near)
            && let Some(free) = free_tile(&self.tiles, *t, &self.stall_spots, self.near)
        {
            *t = free;
            self.brain.goal = Some(self.tiles[free as usize]);
        }
        if stall.owner == self.cfg.name {
            self.brain.goal = Some(glam::Vec3::from(stall.pos));
            info!(
                name = %self.cfg.name,
                stall = stall.id,
                x = stall.pos[0],
                y = stall.pos[1],
                z = stall.pos[2],
                yaw = stall.yaw,
                "stall stands"
            );
        }
    }

    /// A line through the hub: a sociable bot answers its party and a whisperer (its own
    /// "aye" comes back to it and is not answered); the first lines are kept for the report.
    async fn on_heard(&mut self, channel: u8, from: String, text: String) {
        info!(name = %self.cfg.name, channel, %from, %text, "heard through the hub");
        let mine = from == self.cfg.name;
        if self.cfg.social.sociable && !mine {
            let answer = match channel {
                control::CHANNEL_PARTY => Some(FromClient::PartySay("aye".into())),
                control::CHANNEL_WHISPER => Some(FromClient::Whisper {
                    to: from.replace(' ', ""),
                    text: "psst yourself".into(),
                }),
                _ => None,
            };
            if let Some(answer) = answer.filter(|_| text != "aye") {
                let _ = control::send(&mut self.send, &answer).await;
            }
        }
        if self.report.lines.len() < 64 {
            self.report.lines.push((channel, from, text));
        }
    }

    /// Everyone here, in one message: the creatures counted, the raid's kinds, who wears
    /// what.
    fn on_roster(&mut self, players: Vec<control::PlayerEntry>) {
        self.report.creatures_announced = players
            .iter()
            .filter(|p| matches!(p.kind, BodyKind::Creature { .. }))
            .count();
        if let Some(raid) = self.raid.as_mut() {
            raid.kinds = players.iter().map(|p| (p.id, p.kind)).collect();
        }
        self.wearing = players.into_iter().map(|p| (p.id, p.model)).collect();
    }

    /// The report filled from the run, the `Bye`, the connection closed.
    async fn finish(mut self) -> (BotReport, BotExit, String) {
        self.report.deaths_seen = self.report.kills_seen;
        let stats = self.conn.stats();
        self.report.ticks = self.ticks;
        self.report.secs = self.start.elapsed().as_secs_f64();
        self.report.client = self.client.stats;
        self.report.udp_tx_bytes = stats.udp_tx.bytes;
        self.report.udp_rx_bytes = stats.udp_rx.bytes;
        self.report.rtt_ms = self.conn.rtt().as_secs_f64() * 1000.0;
        self.report.final_health = self.client.own_health;
        self.report.final_build = self.current_build;
        self.report.stalls_seen = self.stalls_seen.len();
        self.report.own_model = self.own_model;
        let _ = control::send(&mut self.send, &FromClient::Bye).await;
        self.conn.close(0, b"done");
        // Give the Bye a moment to leave before the endpoint is dropped.
        tokio::time::sleep(Duration::from_millis(20)).await;
        (self.report, self.exit, self.map_name)
    }
}

/// The name of a preset build, or "custom".
fn build_name(pack: &gm_core::build::ContentPack, b: &gm_core::build::Build) -> String {
    pack.builds
        .iter()
        .find(|nb| &nb.build == b)
        .map_or_else(|| "custom".to_string(), |nb| nb.name.clone())
}

/// The first tile from `from` on (round the market) that no stall of `spots` stands on,
/// two spots being one within `near`; `None` when every tile is taken.
fn free_tile(
    tiles: &[glam::Vec3],
    from: u32,
    spots: &std::collections::HashMap<i64, glam::Vec3>,
    near: f32,
) -> Option<u32> {
    let n = tiles.len();
    (0..n)
        .map(|k| (from as usize + k) % n)
        .find(|&k| {
            !spots
                .values()
                .any(|s| (*s - tiles[k]).truncate().length() < near)
        })
        .map(|k| k as u32)
}
