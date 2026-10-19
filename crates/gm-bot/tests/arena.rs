//! Phase 3 acceptance (PLAN.md 11.8, MATRIX.md 11): a build that dominates can be countered
//! by re-speccing. Eight against eight in the arena map, identical duelist brains on both
//! sides, driven straight through `gm_core::sim::Zone` (no network, no interpolation delay,
//! so neither side has a latency edge). Kills per team decide.

use std::path::Path;
use std::sync::Arc;

use glam::Vec3;
use gm_bot::{Behaviour, Brain, View};
use gm_bsp::Bsp;
use gm_core::build::Worn;
use gm_core::sim::{Input, Spawn, Zone, ZoneEvent};
use gm_core::tick::TickRate;
use gm_core::vocab::EntityId;
use gm_net::client::RenderEntity;
use gm_net::snapshot::{SpawnInfo, flags};

const MAP: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/maps/built/arena.bsp"
);
const CONTENT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/content");
const RATE: TickRate = TickRate::COMBAT;

struct Outcome {
    kills: [u32; 3],
    deaths: [u32; 3],
}

fn spawns(bsp: &Bsp) -> Vec<Spawn> {
    bsp.entities
        .iter()
        .filter(|e| e.classname() == "gm_spawn")
        .filter_map(|e| {
            Some(Spawn {
                origin: e.origin()?,
                yaw: e.f32("angle").unwrap_or(0.0),
                team: e.f32("team").unwrap_or(0.0) as u8,
            })
        })
        .collect()
}

/// Play `secs` seconds of team 1 (`build_a`) against team 2 (`build_b`), 8 v 8.
fn play(bsp: &Arc<Bsp>, build_a: &str, build_b: &str, seed: u64, secs: u32) -> Outcome {
    play_worn(
        bsp,
        (build_a, &Worn::NONE),
        (build_b, &Worn::NONE),
        seed,
        secs,
    )
}

/// A match of eight against eight, each side a preset in what it wears (GEAR.md 2 and 3:
/// `Worn::NONE` is the preset in nothing a hub dresses, its build's class and kit).
fn play_worn(bsp: &Arc<Bsp>, a: (&str, &Worn), b: (&str, &Worn), seed: u64, secs: u32) -> Outcome {
    let content = std::env::var("ARENA_CONTENT").unwrap_or_else(|_| CONTENT.to_string());
    let pack = gm_content::load_dir(Path::new(&content), RATE).expect("content");
    let mut zone = Zone::new(RATE, seed, spawns(bsp), pack.clone());
    // A gun is issued with its magazine only, and a bow with nothing; the rounds and the
    // arrows a body carries are stacks the hub tells the zone of (MODES.md 11, GEAR.md 7).
    // Here every bot carries plenty of each.
    let ammo: Vec<(String, u32, Option<i32>)> = pack
        .abilities
        .iter()
        .filter_map(|a| a.ability.ammo_stack())
        .map(|key| (key.to_string(), 500, None))
        .collect();
    let mut bots: Vec<(EntityId, Brain, u8)> = Vec::new();
    for i in 0..16 {
        let (team, (name, worn)) = if i % 2 == 0 { (1u8, a) } else { (2u8, b) };
        let build = pack.build(name).expect(name).clone();
        let id = zone
            .add_player(bsp.as_ref(), build, team)
            .expect("preset validates");
        zone.set_worn(id, worn.clone());
        zone.set_stacks(id, &ammo, &[]);
        bots.push((
            id,
            Brain::new(seed * 100 + i as u64, Behaviour::Duelist),
            team,
        ));
    }
    let ticks = secs * RATE.hz();
    let mut kills = [0u32; 3];
    let mut deaths = [0u32; 3];
    for t in 1..=ticks {
        // Everyone sees everyone at this tick (the arena has no PVS edge for either side).
        let others: Vec<RenderEntity> = zone
            .players()
            .map(|p| RenderEntity {
                id: p.id,
                kind: gm_net::snapshot::EntityKind::Player,
                spawn: SpawnInfo::Player {
                    frame: 0,
                    team: p.team(),
                    aspects: p.sheet.build.aspects.0,
                },
                pos: p.mover.mv.origin,
                yaw: p.mover.yaw,
                pitch: p.mover.pitch,
                anim: p.anim,
                acting: 0,
                flags: if p.alive { flags::ALIVE } else { 0 },
                status: p.mover.statuses.mask(),
                health: None,
                vel: p.mover.mv.velocity,
            })
            .collect();
        let inputs: Vec<(EntityId, Input)> = bots
            .iter_mut()
            .map(|(id, brain, team)| {
                let p = zone.player(*id).expect("player");
                let visible: Vec<RenderEntity> =
                    others.iter().filter(|e| e.id != *id).copied().collect();
                let input = brain.think(&View {
                    me: &p.mover,
                    kit: &p.sheet.kit,
                    frame: p.sheet.build.frame,
                    team: *team,
                    alive: p.alive,
                    health: p.health,
                    others: &visible,
                    tick: t,
                });
                (*id, input)
            })
            .collect();
        for (id, input) in inputs {
            zone.queue_input(id, t, input, 0);
        }
        zone.step(bsp.as_ref());
        let events: Vec<ZoneEvent> = zone.events.drain(..).collect();
        for ev in events {
            if let ZoneEvent::Killed { victim, killer } = ev {
                let kt = zone.player(killer).map_or(0, |p| p.team()) as usize;
                let vt = zone.player(victim).map_or(0, |p| p.team()) as usize;
                if kt != vt {
                    kills[kt] += 1;
                }
                deaths[vt] += 1;
            }
        }
    }
    Outcome { kills, deaths }
}

fn ratio(o: &Outcome) -> f64 {
    o.kills[1] as f64 / o.kills[2].max(1) as f64
}

#[test]
fn ironclad_beats_blade_and_frostweaver_beats_ironclad() {
    let bsp = Arc::new(Bsp::load(Path::new(MAP)).expect("arena built"));
    let secs = 60;
    let seeds = [1u64, 2, 3];
    let mut a_total = [0u32; 3];
    let mut b_total = [0u32; 3];
    let mut c_total = [0u32; 3];
    for &seed in &seeds {
        // The cycle closes: the frostweaver's Water is halved by the blade's Water (MATRIX.md 5).
        let c = play(&bsp, "blade", "frostweaver", seed, secs);
        println!(
            "seed {seed}: blade {} : {} frostweaver (deaths {:?}) ratio {:.2}",
            c.kills[1],
            c.kills[2],
            &c.deaths[1..],
            ratio(&c)
        );
        for (t, k) in c_total.iter_mut().zip(c.kills) {
            *t += k;
        }
        let a = play(&bsp, "ironclad", "blade", seed, secs);
        println!(
            "seed {seed}: ironclad {} : {} blade (deaths {:?}) ratio {:.2}",
            a.kills[1],
            a.kills[2],
            &a.deaths[1..],
            ratio(&a)
        );
        let b = play(&bsp, "ironclad", "frostweaver", seed, secs);
        println!(
            "seed {seed}: ironclad {} : {} frostweaver (deaths {:?}) ratio {:.2}",
            b.kills[1],
            b.kills[2],
            &b.deaths[1..],
            ratio(&b)
        );
        for ((ta, tb), (ka, kb)) in a_total
            .iter_mut()
            .zip(b_total.iter_mut())
            .zip(a.kills.into_iter().zip(b.kills))
        {
            *ta += ka;
            *tb += kb;
        }
    }
    let a_ratio = a_total[1] as f64 / a_total[2].max(1) as f64;
    let b_ratio = b_total[2] as f64 / b_total[1].max(1) as f64;
    let c_ratio = c_total[1] as f64 / c_total[2].max(1) as f64;
    println!(
        "totals over {} seeds x {secs} s: ironclad:blade {}:{} ({a_ratio:.2}); frostweaver:ironclad {}:{} ({b_ratio:.2}); blade:frostweaver {}:{} ({c_ratio:.2})",
        seeds.len(),
        a_total[1],
        a_total[2],
        b_total[2],
        b_total[1],
        c_total[1],
        c_total[2]
    );
    // The blade's edge over the frostweaver was the triangle's (slash 1.25 on cloth,
    // MATRIX.md 4 until 2026-10-17); without an armour class the leg is near even (CI
    // 19:18, the reference machine 25:18), so this leg asks for no domination either way
    // until the director re-tunes it (MATRIX.md 18). With the matrix at 0.75 / 1.25
    // (MATRIX.md 19, 2026-10-19) the two decided legs narrowed (ironclad over blade 2.6,
    // frostweaver over ironclad 1.6 on the reference machine; 3.0–3.6 before): the gate
    // asks that each still points the same way, 1.2 and more, and the mirror is even.
    // With the frost nova as the staff's second move (BODIES.md 3.6, the same day) the
    // frostweaver has it in nothing and uses it as a second move, in reach, every eight
    // seconds: the third leg went to the frostweaver (13:25 on the reference machine;
    // ironclad over blade 3.1, frostweaver over ironclad 2.1). The leg is the director's
    // to tune (MATRIX.md 19); this asks only that the blade is not wiped out.
    assert!(
        c_ratio >= 0.4,
        "frostweaver wipes out the blade: {}:{}",
        c_total[1],
        c_total[2]
    );
    assert!(
        a_total[1] + a_total[2] >= 30,
        "too few kills to decide: {a_total:?}"
    );
    assert!(
        a_ratio >= 1.2,
        "ironclad does not dominate blade: {}:{}",
        a_total[1],
        a_total[2]
    );
    assert!(
        b_ratio >= 1.2,
        "frostweaver does not counter ironclad: {}:{}",
        b_total[2],
        b_total[1]
    );
}

#[test]
fn mirror_match_is_even() {
    // Same build both sides: the map and the brains favour neither team (within noise).
    let bsp = Arc::new(Bsp::load(Path::new(MAP)).expect("arena built"));
    let mut total = [0u32; 3];
    for seed in [11u64, 12, 13, 14] {
        let o = play(&bsp, "blade", "blade", seed, 45);
        println!("seed {seed}: blade {} : {} blade", o.kills[1], o.kills[2]);
        for (t, k) in total.iter_mut().zip(o.kills) {
            *t += k;
        }
    }
    let r = total[1] as f64 / total[2].max(1) as f64;
    println!("mirror totals {}:{} ratio {r:.2}", total[1], total[2]);
    assert!(total[1] + total[2] >= 30, "too few kills: {total:?}");
    assert!(
        (0.7..=1.43).contains(&r),
        "the map or the brains are lopsided: {r:.2}"
    );
    let _ = Vec3::ZERO;
}

/// Every preset against every other, both sides, so the table is free of the team-1 edge
/// (`ARENA_SEEDS` seeds × `ARENA_SECS` s each, default 2 × 60). Prints the kill table and
/// the share of kills each build took against each other; run on demand:
/// `ARENA_ROUND_ROBIN=1 cargo test -p gm-bot --release --test arena round_robin -- --ignored --nocapture`.
#[test]
#[ignore]
fn round_robin() {
    let bsp = Arc::new(Bsp::load(Path::new(MAP)).expect("arena built"));
    let pack = gm_content::load_dir(Path::new(CONTENT), RATE).expect("content");
    let mut names: Vec<String> = pack.builds.iter().map(|b| b.name.clone()).collect();
    names.sort();
    let seeds: u64 = std::env::var("ARENA_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2);
    let secs: u32 = std::env::var("ARENA_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let n = names.len();
    // kills[a][b] = kills build a scored against build b, summed over both orientations.
    let mut kills = vec![vec![0u32; n]; n];
    for a in 0..n {
        for b in a..n {
            for seed in 1..=seeds {
                let o = play(&bsp, &names[a], &names[b], seed, secs);
                kills[a][b] += o.kills[1];
                kills[b][a] += o.kills[2];
                if a != b {
                    let o = play(&bsp, &names[b], &names[a], seed, secs);
                    kills[b][a] += o.kills[1];
                    kills[a][b] += o.kills[2];
                }
            }
            eprintln!(
                "{} v {}: {} : {}",
                names[a], names[b], kills[a][b], kills[b][a]
            );
        }
    }
    println!("\nkills (row scored on column), {seeds} seeds x {secs} s, both sides:");
    print!("| build |");
    for b in &names {
        print!(" {b} |");
    }
    println!(" total |");
    println!("|---|{}---|", "---|".repeat(n));
    for a in 0..n {
        print!("| {} |", names[a]);
        let mut for_ = 0;
        let mut against = 0;
        for (b, row) in kills.iter().enumerate() {
            print!(" {}:{} |", kills[a][b], row[a]);
            if a != b {
                for_ += kills[a][b];
                against += row[a];
            }
        }
        println!(" {for_}:{against} |");
    }
    println!("\nshare of kills (row's kills / all kills in the matchup):");
    print!("| build |");
    for b in &names {
        print!(" {b} |");
    }
    println!(" mean |");
    println!("|---|{}---|", "---|".repeat(n));
    for a in 0..n {
        print!("| {} |", names[a]);
        let mut sum = 0.0;
        for (b, row) in kills.iter().enumerate() {
            let tot = (kills[a][b] + row[a]).max(1) as f64;
            let share = kills[a][b] as f64 / tot;
            if a == b {
                print!(" - |");
            } else {
                print!(" {:.0}% |", share * 100.0);
                sum += share;
            }
        }
        println!(" {:.0}% |", sum / (n - 1) as f64 * 100.0);
    }
}

/// The open hands (GEAR.md 9, step 7): each preset in its own gear against the odd
/// things the open hands allow (a blade with a warhammer, a frostweaver in plate, an
/// ironclad with a bow...), both sides, `ARENA_SEEDS` × `ARENA_SECS` (default 1 × 60).
/// What a body wears is read as the hub would read it (`gm_content::worn::worn_of` over
/// the six template keys), so the table measures the kit and the class the gear brings,
/// not an item's edge. Prints the share of kills each odd case took against each preset;
/// run on demand:
/// `cargo test -p gm-bot --release --test arena open_hands -- --ignored --nocapture`.
#[test]
#[ignore]
fn open_hands() {
    let bsp = Arc::new(Bsp::load(Path::new(MAP)).expect("arena built"));
    let pack = gm_content::load_dir(Path::new(CONTENT), RATE).expect("content");
    let looks = gm_content::looks::Looks::load_dir(Path::new(CONTENT)).expect("looks");
    let seeds: u64 = std::env::var("ARENA_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let secs: u32 = std::env::var("ARENA_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);
    let worn = |preset: &str, right: &str, left: &str, chest: &str| -> Worn {
        let six = [right, left, chest, "", "", ""].map(str::to_string);
        gm_content::worn::worn_of(&looks, &pack, pack.build(preset).unwrap(), &six)
    };
    // Each preset in its own gear: its weapon, its shield where it has one, the chest piece
    // of its class.
    let own: Vec<(&str, Worn)> = vec![
        ("blade", worn("blade", "sword", "", "mail_shirt")),
        (
            "ironclad",
            worn("ironclad", "greatsword", "shield", "plate"),
        ),
        ("frostweaver", worn("frostweaver", "staff", "", "robe")),
        ("shade", worn("shade", "dagger", "", "cuirass")),
        ("mender", worn("mender", "staff", "", "robe")),
        ("musketeer", worn("musketeer", "musket", "", "cuirass")),
        ("shaman", worn("shaman", "hammer", "", "cuirass")),
    ];
    let odd: Vec<(&str, &str, Worn)> = vec![
        (
            "blade with a warhammer",
            "blade",
            worn("blade", "warhammer", "", "mail_shirt"),
        ),
        (
            "blade with a tower shield",
            "blade",
            worn("blade", "sword", "tower_shield", "mail_shirt"),
        ),
        (
            "frostweaver in plate",
            "frostweaver",
            worn("frostweaver", "staff", "", "plate"),
        ),
        (
            "ironclad with a bow",
            "ironclad",
            worn("ironclad", "bow", "", "plate"),
        ),
        (
            "blade with a bow",
            "blade",
            worn("blade", "bow", "", "mail_shirt"),
        ),
        (
            "shade with a greataxe",
            "shade",
            worn("shade", "greataxe", "", "cuirass"),
        ),
        ("blade in cloth", "blade", worn("blade", "sword", "", "")),
    ];
    println!(
        "\nshare of kills (the odd case against each preset in its own gear), {seeds} seed(s) x {secs} s, both sides:"
    );
    print!("| case |");
    for (name, _) in &own {
        print!(" {name} |");
    }
    println!(" mean |");
    println!("|---|{}---|", "---|".repeat(own.len()));
    for (label, preset, w) in &odd {
        print!("| {label} |");
        let mut sum = 0.0;
        for (name, theirs) in &own {
            let (mut for_, mut against) = (0u32, 0u32);
            for seed in 1..=seeds {
                let o = play_worn(&bsp, (preset, w), (name, theirs), seed, secs);
                for_ += o.kills[1];
                against += o.kills[2];
                let o = play_worn(&bsp, (name, theirs), (preset, w), seed, secs);
                for_ += o.kills[2];
                against += o.kills[1];
            }
            let share = for_ as f64 / (for_ + against).max(1) as f64;
            sum += share;
            print!(" {:.0}% ({for_}:{against}) |", share * 100.0);
        }
        println!(" {:.0}% |", sum / own.len() as f64 * 100.0);
    }
}
