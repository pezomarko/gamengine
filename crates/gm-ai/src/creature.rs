//! A creature's mind (COMPANIONS.md 8.2): a post, a threat table, a leash, and the shared
//! fighter underneath. Nothing taunts: a creature fights whoever hurts it most, whoever
//! soaks its blows, and whoever is in its face.

use glam::Vec3;
use gm_core::build::Sheet;
use gm_core::sim::{Input, tick_delta};
use gm_core::tick::Tick;
use gm_core::vocab::EntityId;

use crate::fighter::{Engage, Fighter};
use crate::kit::{KitPlan, Use};
use crate::sense::{Body, Senses};

/// Threat a body gets for being seen: enough to be attacked, nothing against real threat.
const SIGHT_THREAT: f32 = 1.0;
/// Threat decays by this share each second.
const DECAY_PER_S: f32 = 0.05;
/// A new target must have this much more threat than the current one; less when nearer.
const SWITCH: f32 = 1.3;
const SWITCH_NEARER: f32 = 1.1;
/// What is in its face weighs more: threat counts this many times over for a body within
/// reach, falling to once at `FAR`.
const PROXIMITY: f32 = 3.0;
const FAR: f32 = 400.0;
const PICK_EVERY: Tick = 8;
/// A creature chases to this far short of its leash.
pub const CHASE_MARGIN: f32 = 256.0;
/// A shooting creature with no swing keeps this distance.
const RANGE: f32 = 400.0;

pub struct Creature {
    pub home: Vec3,
    pub home_yaw: f32,
    pub sight: f32,
    pub leash: f32,
    fighter: Fighter,
    threat: Vec<(EntityId, f32)>,
    target: Option<EntityId>,
    taunted_by: Option<EntityId>,
    next_pick: Tick,
}

impl Creature {
    pub fn new(
        seed: u64,
        sheet: &Sheet,
        home: Vec3,
        home_yaw: f32,
        sight: f32,
        leash: f32,
    ) -> Creature {
        Creature {
            home,
            home_yaw,
            sight,
            leash,
            fighter: Fighter::new(seed, KitPlan::of(&sheet.kit), home_yaw),
            threat: Vec::new(),
            target: None,
            taunted_by: None,
            next_pick: 0,
        }
    }

    fn add(&mut self, id: EntityId, amount: f32) {
        match self.threat.iter_mut().find(|(e, _)| *e == id) {
            Some((_, t)) => *t += amount,
            None => self.threat.push((id, amount)),
        }
    }

    /// `attacker` dealt this much damage to the creature.
    pub fn hurt_by(&mut self, attacker: EntityId, amount: f32) {
        self.add(attacker, amount);
    }

    /// A blow of the creature lost this much to `body`'s block: a shield holds the eye.
    pub fn blocked_by(&mut self, body: EntityId, absorbed: f32) {
        self.add(body, absorbed * 0.5);
    }

    /// `healer` restored this much health to somebody the creature is fighting.
    pub fn healing_by(&mut self, healer: EntityId, amount: f32) {
        self.add(healer, amount * 0.5);
    }

    /// Another creature of the encounter perceived `body`: pull one, fight all.
    pub fn alert(&mut self, body: EntityId) {
        if !self.knows(body) {
            self.add(body, SIGHT_THREAT);
        }
    }

    pub fn knows(&self, body: EntityId) -> bool {
        self.threat.iter().any(|(e, _)| *e == body)
    }

    /// Everybody on the threat table, highest first.
    pub fn table(&self) -> Vec<(EntityId, f32)> {
        let mut t = self.threat.clone();
        t.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        t
    }

    pub fn target(&self) -> Option<EntityId> {
        self.target
    }

    pub fn engaged(&self) -> bool {
        !self.threat.is_empty()
    }

    /// Forget everything (the encounter reset the creature).
    pub fn reset(&mut self) {
        self.threat.clear();
        self.target = None;
        self.taunted_by = None;
        self.fighter.nav.clear();
        self.fighter.yaw = self.home_yaw;
        self.fighter.pitch = 0.0;
    }

    /// Drop one body from the table (it left the zone, or its party went out).
    pub fn forget(&mut self, body: EntityId) {
        self.threat.retain(|(e, _)| *e != body);
        if self.target == Some(body) {
            self.target = None;
        }
    }

    /// One frame.
    pub fn think(&mut self, s: &Senses<'_>) -> Input {
        let me = s.pos();
        // Threat fades; the dead and the gone leave the table.
        let keep = 1.0 - DECAY_PER_S * s.dt();
        for (_, t) in &mut self.threat {
            *t *= keep;
        }
        self.threat
            .retain(|(id, _)| s.body(*id).is_some_and(|b| b.alive));

        if tick_delta(s.tick, self.next_pick) >= 0 {
            self.next_pick = s.tick.wrapping_add(PICK_EVERY);
            // New faces: anything that is not a creature, in sight, nearest first; two
            // traces a decision at most.
            let mut strangers: Vec<(f32, &Body)> = s
                .bodies
                .iter()
                .filter(|b| b.alive && b.creature.is_none() && !self.knows(b.id))
                .map(|b| (s.dist(b), b))
                .filter(|(d, _)| *d <= self.sight)
                .collect();
            strangers.sort_by(|a, b| a.0.total_cmp(&b.0));
            let seen: Vec<EntityId> = strangers
                .iter()
                .take(2)
                .filter(|(_, b)| s.sees(b))
                .map(|(_, b)| b.id)
                .collect();
            for id in seen {
                self.add(id, SIGHT_THREAT);
            }
            // The highest threat, weighted by how near the body is (what a creature can hit
            // holds its eye), with a margin so two equals do not swap every decision.
            let reach = self.fighter.plan.reach().max(64.0);
            let dist_of = |id: EntityId| s.body(id).map_or(f32::MAX, |b| s.dist(b));
            let weigh = |(id, t): (EntityId, f32)| {
                // (A reach of 400 u or more would divide by nothing.)
                let near = ((FAR - dist_of(id)) / (FAR - reach).max(1.0)).clamp(0.0, 1.0);
                (id, t * (1.0 + (PROXIMITY - 1.0) * near))
            };
            let current = self
                .target
                .and_then(|id| self.threat.iter().find(|(e, _)| *e == id).copied())
                .map(weigh);
            let best = self
                .threat
                .iter()
                .copied()
                .map(weigh)
                .max_by(|a, b| a.1.total_cmp(&b.1).then(b.0.cmp(&a.0)));
            self.target = match (current, best) {
                (Some((cur, ct)), Some((new, nt))) if new != cur => {
                    let margin = if dist_of(new) < dist_of(cur) {
                        SWITCH_NEARER
                    } else {
                        SWITCH
                    };
                    Some(if nt > ct * margin { new } else { cur })
                }
                (Some((cur, _)), _) => Some(cur),
                (None, Some((new, _))) => Some(new),
                (None, None) => None,
            };
        }

        // Taunted (MATRIX.md 8): the taunter is fought while it lasts and tops the table
        // after, by the switch margin, set once when the taunt lands.
        let taunted_by = s.taunted_by();
        if let Some(by) = taunted_by {
            if self.taunted_by != Some(by) {
                let top = self.threat.iter().map(|(_, t)| *t).fold(0.0, f32::max);
                let mine = self
                    .threat
                    .iter()
                    .find(|(e, _)| *e == by)
                    .map_or(0.0, |(_, t)| *t);
                self.add(by, (top * SWITCH - mine).max(SIGHT_THREAT));
            }
            self.target = Some(by);
        }
        self.taunted_by = taunted_by;
        let Some(t) = self.target.and_then(|id| s.body(id)).filter(|b| b.alive) else {
            // Nothing to fight: back to the post (a reset puts the body there at once; this
            // is for a creature that merely strayed).
            self.target = None;
            return if (me - self.home).truncate().length() > 24.0 {
                self.fighter.go(s, self.home, 16.0).0
            } else {
                let (f, _) = gm_core::movement::yaw_vectors(self.home_yaw);
                self.fighter.idle(s, Some(s.eye() + f * 200.0))
            };
        };
        // Far-reaching areas go to the body farthest away that it knows and can see: the
        // back line gets to dance too.
        let artillery = if self
            .fighter
            .plan
            .actives
            .iter()
            .flatten()
            .any(|(slot, u)| matches!(u, Use::Aimed { harmful: true, .. }) && s.ready(*slot))
        {
            let mut known: Vec<(f32, &Body)> = self
                .threat
                .iter()
                .filter_map(|(id, _)| s.body(*id))
                .filter(|b| b.alive)
                .map(|b| (s.dist(b), b))
                .collect();
            known.sort_by(|a, b| b.0.total_cmp(&a.0));
            known
                .into_iter()
                .take(2)
                .find(|(_, b)| s.sees(b))
                .map(|(_, b)| b)
        } else {
            None
        };
        let plan = &self.fighter.plan;
        let range = if plan.shoots() && plan.reach() == 0.0 {
            RANGE
        } else {
            0.0
        };
        let my_id = s.id;
        let ally = move |b: &Body| b.creature.is_some() && b.id != my_id;
        let hostile = |b: &Body| b.creature.is_none();
        self.fighter.fight(
            s,
            &Engage {
                target: t,
                // It stops short of the leash's end, so whoever fights it within reach of its
                // own weapons is still inside the arena (COMPANIONS.md 9).
                tether: Some((self.home, (self.leash - CHASE_MARGIN).max(self.leash * 0.5))),
                flank: false,
                range,
                artillery,
                ally: &ally,
                hostile: &hostile,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nav::NavGrid;
    use crate::view::ZoneView;
    use gm_core::collide::BoxWorld;
    use gm_core::sim::{Zone, test_content};
    use gm_core::tick::TickRate;
    use gm_core::vocab::{ApplyStatus, StackRule, Status, StatusTarget};

    #[test]
    fn a_taunt_tops_the_table_by_the_margin_once() {
        let rate = TickRate::COMBAT;
        let world = BoxWorld::floor();
        let mut zone = Zone::new(rate, 1, vec![], test_content::pack(rate));
        let build = zone.content.build("blade").unwrap().clone();
        let at = |x: f32| Vec3::new(x, 0.0, 24.0);
        let me = zone.add_player_at(build.clone(), 1, at(0.0), 0.0);
        let a = zone.add_player_at(build.clone(), 0, at(100.0), 180.0);
        let b = zone.add_player_at(build, 0, at(-100.0), 0.0);
        let sheet = zone.player(me).unwrap().sheet.clone();
        let mut mind = Creature::new(1, &sheet, at(0.0), 0.0, 800.0, 2000.0);
        mind.hurt_by(a, 30.0);
        mind.hurt_by(b, 5.0);
        let taunt = ApplyStatus {
            status: Status::Taunt,
            duration: rate.ms_to_ticks(4000),
            magnitude: 1.0,
            max_stacks: 1,
            stacking: StackRule::Refresh,
            target: StatusTarget::Area,
            dispellable: true,
        };
        zone.apply_status(me, b, &taunt);
        let nav = NavGrid::build(&world, &[at(0.0)]);
        let creature_of = |id| (id == me).then_some(0);
        for _ in 0..64 {
            let view = ZoneView::of(&zone, &creature_of);
            mind.think(&view.senses(&zone, me, &world, &nav).unwrap());
        }
        let threat = |id| mind.table().into_iter().find(|e| e.0 == id).unwrap().1;
        let ratio = threat(b) / threat(a);
        assert!((ratio - SWITCH).abs() < 0.01, "{ratio}");
        assert_eq!(mind.target(), Some(b));
    }
}
