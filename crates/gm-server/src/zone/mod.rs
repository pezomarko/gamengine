//! The zone process: tick loop over `gm_core::sim::Zone`, sessions, snapshots and reports.

mod arrivals;
mod conduct;
mod events;
mod gear;
mod market;
mod minds;
mod outcomes;
mod run;
mod smithy;
mod snapshot;
mod social;
mod step;
mod travel;

use run::Run;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use glam::Vec3;
use gm_ai::director::{CompanionSpec, CreatureSpawn, Director, DirectorEvent, EncounterState};
use gm_core::build::{Build, ContentPack};
use gm_core::sim::{HitKind, MAX_CLAIMED_VIEW_LAG, Zone, ZoneEvent};
use gm_core::tick::TickRate;
use gm_core::trace::{CollisionWorld, Contents, Hull};
use gm_core::tuning::Tuning;
use gm_core::vocab::EntityId;
use gm_net::control::{
    self, BodyKind, BuildChoice, FromZone, GmNews, GmOp, Look, PlayerEntry, SquadEntry, StallEntry,
    stall_in_reach, trade_in_reach,
};
use rayon::prelude::*;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::Instant;
use tracing::{info, warn};

use crate::hub_link::HubLink;
use crate::net::{ClientEvent, EVENT_CHANNEL, HubJoin, JoinInfo, NetConfig, PartyAsk, accept_loop};
use crate::party::ZoneParties;
use crate::recorder::{Recorder, RecorderConfig, Written};
use crate::session::{PvsCache, Session, TickTable};
use gm_hub_proto::protocol::{
    CharacterId, CharacterState, GearReading, ModelId, ModelRef, PartyNews, PartyReading,
    PartyReply, SayTo, StackReading, StallSummary, ZonePartyOp, ZoneTicket, now_secs,
};
use gm_net::input::InputDatagram;
use gm_net::link::Link;
use gm_replay::RosterEntry;

/// Zones save every character this often (HUB.md 3.2).
pub const SAVE_EVERY: Duration = Duration::from_secs(30);
/// A ghost waits this long for the other zone's claim (HUB.md 3.3).
pub const GHOST_TIMEOUT: Duration = Duration::from_secs(10);
/// A revoked model is remembered this long, so a claim that raced the takedown cannot bring
/// it back (MODELS.md 7).
pub const REVOKED_MEMORY: Duration = Duration::from_secs(60);
use crate::tick::{TickMetrics, TickScheduler};
use crate::world::ZoneWorld;

pub struct ZoneConfig {
    pub rate: TickRate,
    pub open: bool,
    pub seed: u64,
    pub max_players: usize,
    pub report_every: Duration,
    /// Stop after this many ticks (tests and benchmarks).
    pub max_ticks: Option<u64>,
    /// Latest report, for tests and dashboards.
    pub report_tx: Option<watch::Sender<ZoneReport>>,
    /// Abilities and preset builds (MATRIX.md 10).
    pub content: ContentPack,
    /// The looks of the content (CONTENT.md 6): the prop keys a `Look` indexes, and what
    /// each template and primary is held as. Empty: nobody holds anything.
    pub looks: gm_content::looks::Looks,
    /// Preset given to clients that ask for none.
    pub default_build: String,
    /// The hub this zone runs under (HUB.md); `None` = open development zone.
    pub hub: Option<Arc<HubLink>>,
    /// A wild zone (COMPANIONS.md 3.1): every human is team 1 and the map's creatures stand
    /// on their posts. Otherwise a team zone, as the arena.
    pub wild: bool,
    /// Companions come along with their commanders (COMPANIONS.md 3.2).
    pub squads: bool,
    /// Preset builds the zone lends to fill squad slots hires left empty (tutorial zones).
    pub recruits: Vec<String>,
    /// Every arrival starts at the map's spawns; a saved position is not resumed. For
    /// dungeons: nobody logs out past the gate and comes back at the boss's feet.
    pub arrive_at_entry: bool,
    /// Record fights between players and reports as replays (ANTICHEAT.md 3); the live aim
    /// statistics run with it.
    pub replay: Option<ReplayConfig>,
    /// What a body wears does not change until it has neither dealt nor taken damage for
    /// this long (ITEMS.md 5).
    pub gear_after_fight: Duration,
    /// Characters that are game masters here whatever the hub says (GM.md 1): an open
    /// zone's way, and a hand for tests.
    pub gm_names: Vec<String>,
    /// Where the game masters' tuning is read from at the start and written after every
    /// change (GM.md 3); none: a tuning lives as long as the zone.
    pub tuning_file: Option<std::path::PathBuf>,
}

/// Where replays go and how much of them an hour may hold.
#[derive(Clone, Debug)]
pub struct ReplayConfig {
    pub dir: std::path::PathBuf,
    /// Bytes of fight files per hour, before compression; reports are always written.
    pub bytes_per_hour: u64,
    /// The zone's name in file names and headers.
    pub zone: String,
}

impl Default for ZoneConfig {
    fn default() -> Self {
        ZoneConfig {
            rate: TickRate::COMBAT,
            open: true,
            seed: 1,
            max_players: 64,
            report_every: Duration::from_secs(5),
            max_ticks: None,
            report_tx: None,
            content: gm_core::sim::test_content::pack(TickRate::COMBAT),
            looks: Default::default(),
            default_build: "blade".into(),
            hub: None,
            wild: false,
            squads: false,
            recruits: Vec::new(),
            arrive_at_entry: false,
            replay: None,
            gear_after_fight: GEAR_AFTER_FIGHT,
            gm_names: Vec::new(),
            tuning_file: None,
        }
    }
}

/// A body that left can still be reported for this long (ANTICHEAT.md 5).
const REPORTABLE_AFTER_LEAVE: Duration = Duration::from_secs(120);
/// Chat lines a zone relays: this many a second over time, this many at once
/// (PROTOCOL.md 8). Past it lines are dropped.
const ZONE_CHAT_PER_SEC: f32 = 10.0;
const ZONE_CHAT_BURST: f32 = 30.0;

/// What a replay keeps of a simulation event (ANTICHEAT.md 3.1).
fn replay_event(ev: &ZoneEvent) -> Option<gm_replay::Event> {
    use gm_replay::{Event, Hit};
    Some(match *ev {
        ZoneEvent::Hit {
            attacker,
            target,
            amount,
            kind,
            absorbed,
            ..
        } => Event::Hit {
            attacker,
            target,
            amount,
            kind: match kind {
                HitKind::Melee => Hit::Melee,
                HitKind::Projectile => Hit::Projectile,
                HitKind::Area => Hit::Area,
                HitKind::Dot => Hit::Dot,
            },
            absorbed,
        },
        ZoneEvent::Killed { victim, killer } => Event::Killed { victim, killer },
        ZoneEvent::Parried { defender, attacker } => Event::Parried { defender, attacker },
        ZoneEvent::GuardBroken(id) => Event::GuardBroken(id),
        ZoneEvent::Respawned(id) => Event::Respawned(id),
        ZoneEvent::ProjectileSpawned {
            id,
            owner,
            view_lag: lag,
            lag: honoured,
            speed,
            gravity,
            lifetime,
            origin,
            ..
        } => Event::Shot {
            projectile: id,
            owner,
            origin: origin.into(),
            speed,
            gravity,
            lifetime,
            lag,
            honoured,
        },
        _ => return None,
    })
}

/// Say what a replay file holds: one line for the file, one per participant.
/// A client's body leaves the zone for good (its session ended, another zone claimed it,
/// its ghost ran out, it joined again): its aim numbers are logged and go to the hub
/// (ANTICHEAT.md 4.3), and it can still be reported for a while.
fn depart(
    recorder: &mut Option<Recorder>,
    hub: Option<&Arc<HubLink>>,
    recent_left: &mut Vec<(EntityId, CharacterId, Instant)>,
    id: EntityId,
    character: Option<CharacterId>,
) {
    recent_left.retain(|(_, _, at)| at.elapsed() < REPORTABLE_AFTER_LEAVE);
    recent_left.push((id, character.unwrap_or(0), Instant::now()));
    let Some(rec) = recorder else {
        return;
    };
    let name = rec.entry(id).map_or_else(String::new, |e| e.name.clone());
    let aim = rec.analyser.take(id);
    info!(entity = id, name = %name, "aim at leave: {}", aim.line());
    if let (Some(link), Some(character)) = (hub, character)
        && !aim.is_empty()
    {
        let link = link.clone();
        tokio::spawn(async move { link.aim(character, aim).await });
    }
}

fn replay_written(w: &Written) {
    info!(
        path = %w.path.display(),
        reason = ?w.header.reason,
        bytes = w.bytes.len(),
        seconds = format_args!("{:.1}", w.seconds),
        kills = w.kills,
        damage = w.damage,
        participants = w.participants.len(),
        "replay written"
    );
    for (who, aim) in &w.participants {
        info!(file = %w.path.display(), name = %who.name, "replay aim: {}", aim.line());
    }
}

/// What a body holds (LOOK.md 6.1): the model of the weapon it wears, else the prop of
/// its primary ability, else nothing; as an index into the pack's prop list. In the gun
/// mode (MODES.md 3.7) the hand is the weapon switched to, `1 2 3`, whatever is worn:
/// the prop of the ability in hand.
/// The stacks of a reading, as the simulation takes them (MODES.md 11.2).
fn stacks_of(reading: &GearReading) -> Vec<(String, u32, Option<i32>)> {
    reading
        .stacks
        .iter()
        .map(|s| (s.template.clone(), s.quantity, s.heals))
        .collect()
}

/// The body `id` spent `quantity` of its stacks that `which` picks (MODES.md 11.2), the
/// first first: the zone's copy is lowered at once (the next spend picks the right rows),
/// and the hub is told once per row; each reading comes back as a `Worn` nobody waits for.
/// The simulation's reserve is the sum of the stacks, so a spend may span two of them;
/// asked of one row for more than it holds, the hub would refuse and its reading would
/// hand the rounds back.
fn consume_stack(
    cfg: &ZoneConfig,
    hub_slots: &mut BTreeMap<EntityId, HubSlot>,
    event_tx: &mpsc::Sender<ClientEvent>,
    id: EntityId,
    which: impl Fn(&StackReading) -> bool,
    quantity: u32,
) {
    let Some(link) = cfg.hub.clone() else {
        return;
    };
    let Some(slot) = hub_slots.get_mut(&id) else {
        return;
    };
    let character = slot.character;
    let mut left = quantity;
    for stack in slot
        .stacks
        .iter_mut()
        .filter(|s| s.quantity > 0 && which(s))
    {
        if left == 0 {
            break;
        }
        let take = left.min(stack.quantity);
        let item = stack.item;
        stack.quantity -= take;
        left -= take;
        let link = link.clone();
        let tx = event_tx.clone();
        tokio::spawn(async move {
            let result = link.consume(character, item, take).await;
            if let Err(why) = &result {
                warn!(character, item, quantity = take, %why, "consume");
            }
            let _ = tx
                .send(ClientEvent::Worn {
                    id,
                    character,
                    item: 0,
                    result,
                    tell: false,
                })
                .await;
        });
    }
    if left > 0 {
        warn!(
            character,
            quantity, left, "consume: more spent than the stacks carried"
        );
    }
}

fn look_of(
    looks: &gm_content::looks::Looks,
    zone: &Zone,
    hub_slots: &std::collections::BTreeMap<u32, HubSlot>,
    sheaths: &HashMap<EntityId, bool>,
    body: u32,
    wears: Option<&[String; 3]>,
) -> Look {
    let player = zone.player(body);
    if let Some(p) = player
        && p.sheet.kit.mode == gm_core::vocab::Mode::Gun
    {
        let in_hand = p
            .mover
            .in_hand(&p.sheet.kit)
            .and_then(|slot| p.sheet.kit.abilities.get(slot as usize))
            .map(|a| a.id.0.saturating_sub(1) as usize);
        return Look {
            held: looks.held(None, in_hand),
            worn: [Look::NONE; gm_net::control::BODY_PLACES],
            off: Look::NONE,
            sheathed: false,
        };
    }
    // Somebody of the town nothing hurts (the smith, the trainer: ITEMS.md 6.6) wears
    // nothing of the hub's and holds the prop of its primary, so the smith has his hammer.
    if let Some(p) = player
        && p.unhurt
        && !hub_slots.contains_key(&body)
    {
        let primary = p
            .sheet
            .kit
            .abilities
            .first()
            .map(|a| a.id.0.saturating_sub(1) as usize);
        let wear = |i: usize| wears.map(|w| w[i].as_str()).filter(|t| !t.is_empty());
        return Look {
            held: looks.held(None, primary),
            worn: std::array::from_fn(|i| looks.worn(wear(i))),
            off: Look::NONE,
            sheathed: false,
        };
    }
    let worn = |place: usize| {
        hub_slots
            .get(&body)
            .map(|s| s.worn[place].as_str())
            .filter(|t| !t.is_empty())
    };
    // The hands hold what is worn in them and nothing else (LOOK.md 6.1, 6.5): no sword
    // in the inventory, no sword in the hand. The body (LOOK.md 6.6, EQUIPMENT.md 5.2):
    // an overlay a place, in the order the wire lists them.
    // Out of a fight the weapon rests on the back (LOOK.md 6.7): the sheath pass of the
    // tick loop keeps the state; a body it has not judged yet stands as it entered, armed.
    Look {
        held: looks.held(worn(0), None),
        worn: std::array::from_fn(|i| looks.worn(worn(gm_hub_proto::protocol::BODY_INDICES[i]))),
        off: looks.off(worn(1)),
        sheathed: sheaths.get(&body).copied().unwrap_or(false),
    }
}

/// Whether a body's weapon rests on its back (LOOK.md 6.7, EQUIPMENT.md 5.3): it has
/// neither dealt nor taken damage nor begun a swing or a cast for the fight lock's ten
/// seconds (`GEAR_AFTER_FIGHT`, ITEMS.md 5: the one timer the body keeps). `armed_at` is
/// the tick it last did either, as the pass remembers it; a dead body keeps its state.
fn sheathed_now(
    p: &gm_core::sim::Player,
    armed_at: &mut Option<gm_core::tick::Tick>,
    now: gm_core::tick::Tick,
    fight: gm_core::tick::Tick,
) -> bool {
    if p.acting != 0 {
        *armed_at = Some(now);
    }
    if let Some(at) = p.fought_at
        && armed_at.is_none_or(|a| gm_core::sim::tick_delta(at, a) > 0)
    {
        *armed_at = Some(at);
    }
    !armed_at.is_some_and(|at| (gm_core::sim::tick_delta(now, at).max(0) as u32) < fight)
}

fn wire_kind(kind: gm_ai::BodyKind) -> BodyKind {
    match kind {
        gm_ai::BodyKind::Human => BodyKind::Human,
        gm_ai::BodyKind::Companion { owner } => BodyKind::Companion { owner },
        gm_ai::BodyKind::Creature { def } => BodyKind::Creature { def },
    }
}

fn wire_order(order: gm_ai::Order) -> control::Order {
    match order {
        gm_ai::Order::Follow => control::Order::Follow,
        gm_ai::Order::Hold => control::Order::Hold,
        gm_ai::Order::MoveTo(p) => control::Order::MoveTo(p.into()),
        gm_ai::Order::Attack(id) => control::Order::Attack(id),
    }
}

fn ai_order(order: control::Order) -> gm_ai::Order {
    match order {
        control::Order::Follow => gm_ai::Order::Follow,
        control::Order::Hold => gm_ai::Order::Hold,
        control::Order::MoveTo(p) => gm_ai::Order::MoveTo(p.into()),
        control::Order::Attack(id) => gm_ai::Order::Attack(id),
    }
}

/// A commander's squad as its client is told.
fn squad_entries(director: &Director, zone: &Zone, commander: EntityId) -> Vec<SquadEntry> {
    director
        .squad(commander)
        .iter()
        .map(|m| SquadEntry {
            id: m.id,
            name: m.name.clone(),
            role: m.role() as u8,
            order: wire_order(m.mind.order()),
            max_health: zone
                .player(m.id)
                .map_or(0, |p| p.max_health().clamp(0, u16::MAX as i32) as u16),
            recruit: m.recruit,
        })
        .collect()
}

/// A recruit's name: the preset's, capitalised.
fn recruit_name(preset: &str) -> String {
    let mut c = preset.chars();
    match c.next() {
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
        None => "Recruit".into(),
    }
}

/// Hub bookkeeping of one session.
struct HubSlot {
    character: CharacterId,
    joined: Instant,
    play_seconds_before: u32,
    last_save: Instant,
    /// Set when a travel ticket went out; the body is a ghost until the claim or the timeout.
    ghost_since: Option<Instant>,
    /// Another zone claimed the character: no save on leave.
    claimed_elsewhere: bool,
    /// The number of the hub's reading of its gear that the body has (ITEMS.md 3.3): a
    /// reading with a smaller number that arrives later changes nothing.
    gear_seq: u64,
    /// The templates worn by place (`PLACE_NAMES`), from the same reading (LOOK.md 6.2):
    /// what it holds and wears.
    worn: [String; gm_hub_proto::protocol::PLACES],
    /// The stacks it carries, from the same reading (MODES.md 11.2): the firearms'
    /// reserves and the kits, and which item row each is, for the hub's books.
    stacks: Vec<StackReading>,
    /// Its item bar, from the same reading (LOOK.md 3.2): the template on each cell.
    bar: Vec<Option<String>>,
}

/// An invitation for a character that has no body here yet is kept this long (the
/// invitation's own life, PARTY.md 3.2).
const INVITE_KEPT: Duration = Duration::from_secs(gm_hub_proto::protocol::INVITE_SECS);

/// A reading of gear for a character that has no body here at the moment is kept this
/// long: an answer to the body it had, arriving while it joins again.
const GEAR_KEPT: Duration = Duration::from_secs(120);
/// How long the zone waits for the hub on a player's buy or wear before it says so and
/// lets the player ask again.
const HUB_PATIENCE: Duration = Duration::from_secs(10);

/// What a body wears does not change in a fight (ITEMS.md 5): not until it has neither
/// dealt nor taken damage for this long.
pub const GEAR_AFTER_FIGHT: Duration = Duration::from_secs(10);
/// What a player is told when the hub did not answer in `HUB_PATIENCE`.
const HUB_SILENT: &str = "the hub did not answer: try again";
use gm_net::control::TRAINER_REACH;

/// A line of the zone itself to one client (a refusal, a notice): it is not lost.
/// What a body of the director's wears for its look (ITEMS.md 6.6): a creature's
/// `wears`; nothing for anybody else.
fn npc_wears(pack: &ContentPack, kind: gm_ai::BodyKind) -> Option<&[String; 3]> {
    match kind {
        gm_ai::BodyKind::Creature { def } => pack.creatures.get(def as usize).map(|d| &d.wears),
        _ => None,
    }
}

fn zone_line(session: &Session, text: impl Into<String>) {
    session.send_control(FromZone::ChatFrom {
        from: 0,
        text: text.into(),
    });
}

/// The session of the body a character has here.
fn session_of<'a>(
    hub_slots: &BTreeMap<EntityId, HubSlot>,
    sessions: &'a BTreeMap<EntityId, Session>,
    character: CharacterId,
) -> Option<&'a Session> {
    let (id, _) = hub_slots.iter().find(|(_, s)| s.character == character)?;
    sessions.get(id)
}

/// The hub's news of a party (PARTY.md 3.2), from an answer or a notice: every character
/// here that it changes something for is told what its party is now, and, when it is in
/// one or out of one by this, in a line. (The number its body fights under follows when
/// the body is in no fight.)
fn party_news(
    parties: &mut ZoneParties,
    news: &PartyNews,
    hub_slots: &BTreeMap<EntityId, HubSlot>,
    sessions: &BTreeMap<EntityId, Session>,
) {
    let was: Vec<(CharacterId, bool)> = news
        .party
        .members
        .iter()
        .map(|(c, _)| *c)
        .chain(news.left.iter().copied())
        .map(|c| (c, parties.of(c).is_some()))
        .collect();
    for character in parties.news(news, Instant::now()) {
        let Some(s) = session_of(hub_slots, sessions, character) else {
            continue;
        };
        let names = parties.names(character);
        let before = was.iter().any(|(c, in_one)| *c == character && *in_one);
        match (before, names.is_empty()) {
            (false, false) => zone_line(s, format!("you are in a party: {}", names.join(", "))),
            (true, true) if news.party.members.is_empty() => zone_line(s, "the party is no more"),
            (true, true) => zone_line(s, "you are out of the party"),
            _ => {}
        }
        s.send_control(FromZone::Party(names));
    }
}

/// A stall as clients see it: the tile's centre, the keeper's looks. `None` when the tile is
/// not on this map (a stall left over from another build of it).
fn stall_entry(
    world: &ZoneWorld,
    s: &StallSummary,
    revoked: &[(ModelId, Instant)],
) -> Option<StallEntry> {
    let (grid, centre) = world
        .stall_grids
        .iter()
        .find_map(|g| Some((g, g.centre(s.tile_x, s.tile_y)?)))?;
    Some(StallEntry {
        id: s.id,
        pos: centre.into(),
        yaw: grid.yaw,
        owner: s.owner_name.clone(),
        frame: s.frame,
        model: s
            .model
            .filter(|m| m.frame == s.frame && !revoked.iter().any(|(id, _)| *id == m.id))
            .map(|m| m.id),
    })
}

fn character_state(
    zone: &Zone,
    link: &HubLink,
    id: EntityId,
    slot: &HubSlot,
) -> Option<CharacterState> {
    let p = zone.player(id)?;
    // The build the character chose (MATRIX.md 9.1): in a team zone one it asked for is
    // worn at the next respawn, and is what it wears when it next enters anywhere.
    let build = p
        .pending_build
        .clone()
        .unwrap_or_else(|| p.sheet.build.clone());
    Some(CharacterState {
        build,
        zone: Some(link.zone.clone()),
        position: p.mover.mv.origin.into(),
        yaw: p.mover.yaw,
        viewport: 0,
        play_seconds: slot.play_seconds_before + slot.joined.elapsed().as_secs() as u32,
    })
}

/// Counters over one report window plus running totals.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ZoneReport {
    pub tick: u64,
    pub players: usize,
    pub window_secs: f64,
    /// UDP bytes per connected player per second, from QUIC's own counters.
    pub tx_bytes_per_player_s: f64,
    pub rx_bytes_per_player_s: f64,
    pub snapshot_payload_per_player_s: f64,
    pub tick_mean_us: f64,
    pub tick_p99_us: f64,
    pub tick_max_us: f64,
    pub late_max_us: f64,
    pub overruns: u64,
    /// Where the tick went, mean microseconds over the window: network events, the
    /// simulation step, snapshot building, datagram sends.
    pub events_us_mean: f64,
    pub sim_us_mean: f64,
    pub snapshot_us_mean: f64,
    pub send_us_mean: f64,
    // Totals since start.
    pub joins: u64,
    pub leaves: u64,
    pub executed_frames: u64,
    pub starved_ticks: u64,
    pub dropped_frames: u64,
    pub oversize_drops: u64,
    pub send_failures: u64,
    pub malformed: u64,
    pub hits_melee: u64,
    pub hits_projectile: u64,
    pub hits_area: u64,
    pub hits_dot: u64,
    pub parries: u64,
    pub guard_breaks: u64,
    pub staggers: u64,
    pub statuses_applied: u64,
    pub kills: u64,
    /// Kills per team (index 0 = team 0 / none, 1, 2).
    pub team_kills: [u64; 3],
    pub max_tx_bytes_per_player_s: f64,
    pub max_rx_bytes_per_player_s: f64,
    /// Counters of the players connected right now (not yet folded into the totals).
    pub live_executed_frames: u64,
    pub live_starved_ticks: u64,
    /// Minds the zone runs (companions and creatures) and what they cost, mean microseconds
    /// per tick over the window (COMPANIONS.md 14).
    pub minds: usize,
    pub minds_us_mean: f64,
    /// Encounters, totals since start.
    pub encounters_engaged: u64,
    pub encounters_reset: u64,
    pub encounters_cleared: u64,
    /// The time the last cleared encounter took, seconds.
    pub last_clear_secs: u32,
    pub loot_items: u64,
    pub trials_passed: u64,
    pub orders: u64,
    pub orders_refused: u64,
    /// The recorder and the aim analysis (ANTICHEAT.md 10): mean microseconds per tick over
    /// the window, frames and their bytes since start, files written, what the ring holds.
    /// Chat lines relayed, and lines dropped by the zone's ceiling.
    pub chat_lines: u64,
    pub chat_dropped: u64,
    pub record_us_mean: f64,
    /// The part of it that is the line-of-sight sweep (ANTICHEAT.md 4.2).
    pub sight_us_mean: f64,
    pub replay_frames: u64,
    pub replay_frame_bytes: u64,
    pub replays_written: u64,
    pub replay_ring_bytes: usize,
}

/// Run the zone until `shutdown` resolves or `max_ticks` is reached. The endpoint must already
/// be bound; it is closed on exit.
pub async fn run(
    cfg: ZoneConfig,
    world: Arc<ZoneWorld>,
    endpoint: quinn::Endpoint,
    shutdown: impl Future<Output = ()>,
) -> anyhow::Result<ZoneReport> {
    run_with_web(cfg, world, endpoint, None, shutdown).await
}

/// `run`, with a WebTransport listener beside the QUIC endpoint (WEB.md 2.1).
pub async fn run_with_web(
    cfg: ZoneConfig,
    world: Arc<ZoneWorld>,
    endpoint: quinn::Endpoint,
    web: Option<crate::net::WebListener>,
    shutdown: impl Future<Output = ()>,
) -> anyhow::Result<ZoneReport> {
    let mut run = Run::start(cfg, world, endpoint, web);
    let shutdown = std::pin::pin!(shutdown);
    let mut shutdown = shutdown.fuse_once();
    info!(
        hz = run.rate.hz(),
        map = %run.world.name,
        spawns = run.world.spawns.len(),
        "zone running"
    );
    loop {
        let deadline = run.scheduler.next_deadline();
        tokio::select! {
            _ = &mut shutdown => {
                info!("shutdown requested");
                break;
            }
            _ = tokio::time::sleep_until(deadline) => {}
        }
        if !run.tick(deadline) {
            break;
        }
    }
    Ok(run.finish().await)
}

/// The build a joiner or a respec asks for (MATRIX.md 9.1): a preset of the content, the
/// zone's default, or a custom one.
fn resolve_build(
    cfg: &ZoneConfig,
    zone: &Zone,
    choice: Option<&BuildChoice>,
) -> Result<Build, String> {
    match choice {
        None => zone
            .content
            .build(&cfg.default_build)
            .cloned()
            .ok_or_else(|| format!("no default build {:?}", cfg.default_build)),
        Some(BuildChoice::Preset(name)) => zone
            .content
            .build(name)
            .cloned()
            .ok_or_else(|| format!("unknown preset {name:?}")),
        Some(BuildChoice::Custom(b)) => Ok(b.clone()),
    }
}

fn fill_report(
    report: &mut ZoneReport,
    sessions: &mut BTreeMap<EntityId, Session>,
    zone: &Zone,
    metrics: &TickMetrics,
    secs: f64,
    tick: u64,
) {
    let mut tx = 0u64;
    let mut rx = 0u64;
    let mut payload = 0u64;
    for s in sessions.values_mut() {
        let st = s.conn.stats();
        tx += st.udp_tx.bytes.saturating_sub(s.last_udp_tx);
        rx += st.udp_rx.bytes.saturating_sub(s.last_udp_rx);
        s.last_udp_tx = st.udp_tx.bytes;
        s.last_udp_rx = st.udp_rx.bytes;
        payload += s.snapshot_bytes;
        s.snapshot_bytes = 0;
    }
    let n = sessions.len().max(1) as f64;
    let secs = secs.max(1e-3);
    let sm = metrics.summary();
    report.tick = tick;
    report.players = sessions.len();
    report.window_secs = secs;
    report.tx_bytes_per_player_s = tx as f64 / n / secs;
    report.rx_bytes_per_player_s = rx as f64 / n / secs;
    report.snapshot_payload_per_player_s = payload as f64 / n / secs;
    if !sessions.is_empty() {
        report.max_tx_bytes_per_player_s = report
            .max_tx_bytes_per_player_s
            .max(report.tx_bytes_per_player_s);
        report.max_rx_bytes_per_player_s = report
            .max_rx_bytes_per_player_s
            .max(report.rx_bytes_per_player_s);
    }
    report.tick_mean_us = sm.mean_us;
    report.tick_p99_us = sm.p99_us;
    report.tick_max_us = sm.max_us;
    report.late_max_us = sm.late_max_us;
    report.overruns = sm.overruns;
    // Live players' counters (folded into the totals when they leave).
    report.live_executed_frames = zone.players().map(|p| p.executed_frames).sum();
    report.live_starved_ticks = zone.players().map(|p| p.starved_ticks).sum();
}

/// Where the ticks of a report window went.
#[derive(Default)]
struct Phases {
    ticks: u64,
    events: Duration,
    minds: Duration,
    sim: Duration,
    snapshot: Duration,
    send: Duration,
    record: Duration,
    sight: Duration,
}

impl Phases {
    fn fill(&self, report: &mut ZoneReport) {
        let n = self.ticks.max(1) as f64;
        report.events_us_mean = self.events.as_secs_f64() * 1e6 / n;
        report.sim_us_mean = self.sim.as_secs_f64() * 1e6 / n;
        report.snapshot_us_mean = self.snapshot.as_secs_f64() * 1e6 / n;
        report.send_us_mean = self.send.as_secs_f64() * 1e6 / n;
        report.minds_us_mean = self.minds.as_secs_f64() * 1e6 / n;
        report.record_us_mean = (self.record + self.sight).as_secs_f64() * 1e6 / n;
        report.sight_us_mean = self.sight.as_secs_f64() * 1e6 / n;
    }
}

/// A future that stays pending after it first resolved, so `select!` can poll it repeatedly.
trait FuseOnce: Future<Output = ()> + Sized {
    fn fuse_once(self) -> Fused<Self> {
        Fused { inner: Some(self) }
    }
}

impl<F: Future<Output = ()>> FuseOnce for F {}

struct Fused<F> {
    inner: Option<F>,
}

impl<F: Future<Output = ()> + Unpin> Future for Fused<F> {
    type Output = ();
    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<()> {
        match &mut self.inner {
            Some(f) => match std::pin::Pin::new(f).poll(cx) {
                std::task::Poll::Ready(()) => {
                    self.inner = None;
                    std::task::Poll::Ready(())
                }
                std::task::Poll::Pending => std::task::Poll::Pending,
            },
            None => std::task::Poll::Pending,
        }
    }
}
