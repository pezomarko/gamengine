use super::*;

/// Offline simulation: the local player against the map, no server.
pub struct Sim {
    pub rate: TickRate,
    pub accumulator: f32,
    pub prev: PlayerState,
    pub curr: PlayerState,
    pub yaw: f32,
    pub pitch: f32,
    pub vars: MoveVars,
}

impl Sim {
    pub fn new(bsp: &Bsp) -> Sim {
        Sim::at(bsp, None)
    }

    /// Start at `start` (`x, y, z, yaw`), or at the map's start.
    pub fn at(bsp: &Bsp, start: Option<[f32; 4]>) -> Sim {
        let (origin, yaw) = match start {
            Some([x, y, z, yaw]) => (Vec3::new(x, y, z), yaw),
            None => bsp
                .player_start()
                .unwrap_or((Vec3::new(0.0, 0.0, 64.0), 0.0)),
        };
        let st = PlayerState::new(origin);
        Sim {
            rate: TickRate::COMBAT,
            accumulator: 0.0,
            prev: st,
            curr: st,
            yaw,
            pitch: 0.0,
            vars: MoveVars::QUAKE,
        }
    }

    /// Advance the simulation by `frame_dt` seconds of wall time in fixed ticks. Returns the
    /// number of ticks run.
    pub fn advance(&mut self, bsp: &Bsp, input: &MoveInput, frame_dt: f32) -> u32 {
        let dt = self.rate.dt();
        self.accumulator += frame_dt.min(0.25);
        let mut steps = 0;
        while self.accumulator >= dt && steps < MAX_STEPS_PER_FRAME {
            self.prev = self.curr;
            player_move(bsp, &self.vars, &mut self.curr, input, dt);
            self.accumulator -= dt;
            steps += 1;
        }
        if steps == MAX_STEPS_PER_FRAME {
            self.accumulator = 0.0;
        }
        steps
    }

    /// Eye position interpolated between the last two ticks.
    pub fn eye(&self) -> Vec3 {
        self.prev.origin.lerp(self.curr.origin, self.alpha())
            + Vec3::new(0.0, 0.0, self.curr.hull.eye_height())
    }

    pub fn origin(&self) -> Vec3 {
        self.prev.origin.lerp(self.curr.origin, self.alpha())
    }

    pub(super) fn alpha(&self) -> f32 {
        (self.accumulator / self.rate.dt()).clamp(0.0, 1.0)
    }
}

/// Third-person camera position: pulled in by a point trace so it never enters a wall.
pub(crate) fn third_person_camera(
    world: &dyn CollisionWorld,
    eye: Vec3,
    yaw: f32,
    pitch: f32,
) -> Vec3 {
    let fwd = view_dir(yaw, pitch);
    let (_, right) = yaw_vectors(yaw);
    let desired = eye - fwd * CAMERA_BACK + right * CAMERA_RIGHT + Vec3::Z * CAMERA_UP;
    let tr = world.trace(Hull::Point, eye, desired);
    if tr.fraction < 1.0 {
        let back = (eye - tr.end).normalize_or_zero();
        tr.end + back * 6.0
    } else {
        desired
    }
}

/// Camera-to-muzzle re-aim (VOCABULARY.md 9): resolve the camera ray to the first world or
/// player hit, then aim at that point from the eyes. Returns `(yaw, pitch)` for the input.
pub(super) fn re_aim(
    world: &dyn CollisionWorld,
    bodies: &[Aabb],
    camera: Vec3,
    cam_yaw: f32,
    cam_pitch: f32,
    eye: Vec3,
) -> (f32, f32) {
    let dir = view_dir(cam_yaw, cam_pitch);
    let far = camera + dir * AIM_REACH;
    let tr = world.trace(Hull::Point, camera, far);
    let mut point = if tr.fraction < 1.0 { tr.end } else { far };
    let bt = sweep_boxes(Hull::Point, camera, point, bodies);
    if bt.fraction < 1.0 && !bt.start_solid {
        point = bt.end;
    }
    let to = point - eye;
    // A hit between the camera and the body: aim straight along the camera.
    let aim = if to.dot(dir) < 16.0 {
        dir
    } else {
        to.normalize_or_zero()
    };
    let yaw = aim.y.atan2(aim.x).to_degrees().rem_euclid(360.0);
    let pitch = (-aim.z).clamp(-1.0, 1.0).asin().to_degrees();
    (yaw, pitch)
}

/// The faces visible from any of `leaves`, each listed once.
pub(super) fn visible_from(bsp: &Bsp, leaves: &[usize]) -> Vec<u32> {
    let mut seen = vec![false; bsp.faces.len()];
    let mut out = Vec::new();
    for &leaf in leaves {
        for f in bsp.visible_faces(leaf) {
            if !seen[f as usize] {
                seen[f as usize] = true;
                out.push(f);
            }
        }
    }
    out
}

/// What a map says its air is (SOUND.md 3): the worldspawn's `gm_ambience`, when it has one.
pub(super) fn ambience_of(bsp: &Bsp) -> Option<&str> {
    bsp.entities
        .iter()
        .find(|e| e.classname() == "worldspawn")
        .and_then(|e| e.get("gm_ambience"))
}

/// A map's name as a zone may send it: letters, digits, `_` and `-`, at most 64. It is joined
/// to a directory natively and put into a URL in a browser.
pub(super) fn valid_map_name(name: &str) -> bool {
    (1..=64).contains(&name.len())
        && name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// What the script assumes of the weapons it holds (the v1 content's are close to it): a
/// bolt's speed and the wind-up before it leaves, a melee weapon's reach from the eye, and
/// how far the first active (an area around the caster for most builds) reaches.
pub(super) const SCRIPT_BOLT_SPEED: f32 = 1400.0;

pub(super) const SCRIPT_BOLT_WINDUP: f32 = 0.15;

pub(super) const SCRIPT_MELEE_REACH: f32 = 84.0;

pub(super) const SCRIPT_AREA_REACH: f32 = 150.0;

/// The script walks straight at its enemy, and a wall between them stops it there for
/// the run (the WebGL fight on a software GPU, 2026-10-16: no damage either way in 40 s).
/// Every `SCRIPT_STUCK_TICKS` the eye is compared with where it was: under
/// `SCRIPT_STUCK_MOVE` units on, the body side-steps for `SCRIPT_SIDESTEP_TICKS`, the
/// other way than last time, with a jump at the start, and slides along the wall.
pub(super) const SCRIPT_STUCK_TICKS: u32 = 32;

pub(super) const SCRIPT_STUCK_MOVE: f32 = 24.0;

pub(super) const SCRIPT_SIDESTEP_TICKS: u32 = 48;

/// The script's memory of its walk: where the eye was at the last check and the tick of
/// it, the tick a side-step lasts until, and which way it goes.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct ScriptStuck {
    checked: Option<(Vec3, u32)>,
    until: u32,
    side: f32,
}

/// `--script fight` (WEB.md 8): face the nearest enemy and walk at it; in reach strike and
/// use the first active (an area for most builds, which a guard does not turn away as it
/// does a swing); further off loose the ranged secondary where the enemy will be. With
/// nobody in sight, walk and turn. `target` is the enemy's position and velocity; a walk
/// that gets nowhere side-steps (`ScriptStuck`).
pub(super) fn fight_input(
    eye: Vec3,
    target: Option<(Vec3, Vec3)>,
    sim: &mut Sim,
    stuck: &mut ScriptStuck,
    tick: u32,
) -> SimInput {
    let mut input = SimInput {
        buttons: 0,
        yaw: sim.yaw,
        pitch: 0.0,
        forward: 1.0,
        side: 0.0,
        ability: 0,
        held: 0,
        target: 0,
        use_slot: 0,
    };
    match target {
        Some((at, velocity)) => {
            let distance = (at - eye).length().max(1.0);
            let melee = distance < SCRIPT_MELEE_REACH;
            // A bolt is aimed where the enemy will be when it arrives.
            let aim_at = if melee {
                at
            } else {
                at + velocity * (SCRIPT_BOLT_WINDUP + distance / SCRIPT_BOLT_SPEED)
            };
            let to = aim_at - eye;
            let reach = to.length().max(1.0);
            sim.yaw = to.y.atan2(to.x).to_degrees().rem_euclid(360.0);
            sim.pitch = (-to.z / reach).clamp(-1.0, 1.0).asin().to_degrees();
            input.yaw = sim.yaw;
            input.pitch = sim.pitch;
            if melee {
                input.buttons |= buttons::PRIMARY;
                if distance < 48.0 {
                    input.forward = 0.0;
                }
            } else if distance < 900.0 {
                input.buttons |= buttons::SECONDARY;
            }
            // The first active as often as it comes back, the second now and then.
            if distance < SCRIPT_AREA_REACH && tick.is_multiple_of(32) {
                input.ability = if tick.is_multiple_of(256) { 2 } else { 1 };
            }
        }
        None => {
            sim.yaw = (sim.yaw + 0.6).rem_euclid(360.0);
            sim.pitch = 0.0;
            input.yaw = sim.yaw;
        }
    }
    if input.forward > 0.0 {
        if tick < stuck.until {
            input.side = stuck.side;
            if tick + SCRIPT_SIDESTEP_TICKS == stuck.until {
                input.buttons |= buttons::JUMP;
            }
        } else if let Some((was, then)) = stuck.checked {
            if tick.wrapping_sub(then) >= SCRIPT_STUCK_TICKS {
                if (eye - was).length() < SCRIPT_STUCK_MOVE {
                    stuck.side = if stuck.side > 0.0 { -1.0 } else { 1.0 };
                    stuck.until = tick + SCRIPT_SIDESTEP_TICKS;
                }
                stuck.checked = Some((eye, tick));
            }
        } else {
            stuck.checked = Some((eye, tick));
        }
    }
    input
}

/// A body turns toward its way of travel this fast when it is drawn (LOOK.md 13.9).
pub(super) const FACING_TURN_DEG_PER_SEC: f32 = 720.0;

/// A body runs facing its travel only while the travel is within this of where it
/// looks: a side-step is a run to the side; backing off is a backpedal, the eyes on
/// whom it backs off from.
pub(super) const FACING_BACKPEDAL_DEG: f32 = 100.0;

/// Where a body is drawn facing (LOOK.md 13.9): running or in the air, its way of
/// travel, so that a side-step is a run to the side and not a slide of a body running
/// ahead; in every other stance where it looks, which is where its blow lands. `drawn` is
/// the yaw it was drawn at last frame: the turn is quick, not a snap.
///
/// An RPG body (`free`, MODES.md 5.1) does not turn with its camera: it runs facing its
/// travel whichever way that is (no backpedal: `S` walks it toward the camera), turns to
/// where it looks only for an action (the zone fires there, or at its target while the
/// turn holds it, in which case the caller passes that yaw and `free` false), and in
/// every other stance stands as it was left.
pub(crate) fn facing(
    drawn: Option<f32>,
    view_yaw: f32,
    vel: Vec3,
    anim: u8,
    free: bool,
    dt: f32,
) -> f32 {
    use gm_core::sim::anim;
    let flat = vel.truncate();
    let mut target = view_yaw;
    if matches!(anim, anim::RUN | anim::AIR) && flat.length() > 10.0 {
        let travel = flat.y.atan2(flat.x).to_degrees();
        let off = (travel - view_yaw + 180.0).rem_euclid(360.0) - 180.0;
        if free || off.abs() <= FACING_BACKPEDAL_DEG {
            target = travel;
        }
    } else if free
        && !anim::acts(anim)
        && let Some(drawn) = drawn
    {
        return drawn;
    }
    let Some(drawn) = drawn else {
        return target.rem_euclid(360.0);
    };
    let delta = (target - drawn + 180.0).rem_euclid(360.0) - 180.0;
    let step = FACING_TURN_DEG_PER_SEC * dt.max(0.0);
    (drawn + delta.clamp(-step, step)).rem_euclid(360.0)
}

/// The wedge and the times of an ability's swing, when its script begins with one: what
/// the effects draw (`fx::Swing`), from the numbers the zone resolves the hit with.
pub(crate) fn swing_of(ability: &gm_core::vocab::Ability, dt: f32) -> Option<crate::fx::Swing> {
    match ability.steps.first() {
        Some(gm_core::vocab::Step {
            verb: gm_core::vocab::Verb::MeleeArc(arc),
            at,
        }) => Some(crate::fx::Swing {
            reach: arc.reach,
            arc_deg: arc.arc_deg,
            windup: (at + arc.timing.windup) as f32 * dt,
            active: arc.timing.active as f32 * dt,
        }),
        _ => None,
    }
}

/// The colour of what deals a damage: its element's (the aspects' colours), or steel's
/// for a blow.
pub(crate) fn damage_ink(damage: &gm_core::vocab::DamagePacket) -> [f32; 4] {
    match damage.dtype.element() {
        Some(e) => {
            let c = crate::avatars::ASPECT_COLOURS[e as usize];
            [c[0], c[1], c[2], 1.0]
        }
        None => [0.92, 0.94, 1.0, 1.0],
    }
}

/// The other bodies as the RPG mode reads them (MODES.md 5): where they stand now, their
/// frame, and whether they are enemies (another team; in the wild, everyone).
pub(super) fn rpg_bodies_of(c: &ClientState, my_team: u8) -> Vec<crate::rpg::Body> {
    c.others_at(c.render_tick(0.0))
        .iter()
        .filter(|e| e.kind == EntityKind::Player && e.alive())
        .filter_map(|e| match e.spawn {
            SpawnInfo::Player { frame, team, .. } => Some(crate::rpg::Body {
                id: e.id,
                origin: e.pos,
                frame: crate::rpg::frame_of(frame),
                crouched: e.flags & gm_net::snapshot::flags::CROUCHED != 0,
                enemy: team != my_team || team == gm_core::sim::TEAM_WILD,
            }),
            _ => None,
        })
        .collect()
}

/// The active slot (1-based) that is a dash with an untouchable window (MODES.md 4.4),
/// while it is ready: what Space plays in the action mode.
pub(crate) fn dodge_slot(
    kit: &gm_core::build::Kit,
    mover: &gm_core::sim::Mover,
    now: u32,
) -> Option<u8> {
    kit.actives.iter().enumerate().find_map(|(i, slot)| {
        let slot = (*slot)? as usize;
        let ab = kit.abilities.get(slot)?;
        let dash = matches!(
            ab.steps.first().map(|s| &s.verb),
            Some(gm_core::vocab::Verb::MoveSelf(ms))
                if matches!(ms.kind, gm_core::vocab::MoveKind::Dash { .. }) && ms.iframes > 0
        );
        (dash && gm_core::sim::tick_delta(now, mover.cooldowns[slot]) >= 0).then_some(i as u8 + 1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gm_core::collide::BoxWorld;

    #[test]
    fn a_running_body_faces_its_travel_and_an_acting_one_its_aim() {
        use gm_core::sim::anim;
        let side = Vec3::new(0.0, 200.0, 0.0);
        // Looking along +x and stepping to the left (+y): drawn running that way, turned
        // to it over a few frames rather than at once.
        assert_eq!(facing(None, 0.0, side, anim::RUN, false, 0.016), 90.0);
        let turned = facing(Some(0.0), 0.0, side, anim::RUN, false, 0.05);
        assert!(turned > 30.0 && turned < 40.0, "{turned}");
        let there = facing(Some(turned), 0.0, side, anim::RUN, false, 0.25);
        assert_eq!(there, 90.0);
        // Winding up a blow: back to the aim, where the blow lands. In the air, the travel.
        assert_eq!(
            facing(Some(90.0), 0.0, side, anim::WINDUP, false, 0.25),
            0.0
        );
        assert_eq!(facing(Some(90.0), 0.0, side, anim::AIR, false, 0.25), 90.0);
        // Standing still, or guarding: the aim. Backing off: a backpedal, the aim kept.
        assert_eq!(
            facing(None, 30.0, Vec3::ZERO, anim::RUN, false, 0.016),
            30.0
        );
        assert_eq!(facing(None, 30.0, side, anim::GUARD, false, 0.016), 30.0);
        let back = Vec3::new(-200.0, 50.0, 0.0);
        assert_eq!(facing(None, 0.0, back, anim::RUN, false, 0.016), 0.0);
        // The turn takes the short way round.
        let near = facing(
            Some(350.0),
            0.0,
            Vec3::new(200.0, 60.0, 0.0),
            anim::RUN,
            false,
            0.01,
        );
        assert!(!(10.0..=350.0).contains(&near), "{near}");
    }

    #[test]
    fn an_rpg_body_stands_as_it_was_left_and_turns_only_for_an_action() {
        use gm_core::sim::anim;
        let still = Vec3::ZERO;
        // Standing, the camera orbits (the look turns): the body stays where it was drawn.
        assert_eq!(facing(Some(90.0), 0.0, still, anim::IDLE, true, 0.25), 90.0);
        assert_eq!(
            facing(Some(90.0), 180.0, still, anim::GUARD, true, 0.25),
            90.0
        );
        // Never drawn yet: the look, once.
        assert_eq!(facing(None, 30.0, still, anim::IDLE, true, 0.016), 30.0);
        // Walking toward the camera (S): a run that way, not a backpedal.
        let back = Vec3::new(-200.0, 0.0, 0.0);
        assert_eq!(facing(Some(180.0), 0.0, back, anim::RUN, true, 0.25), 180.0);
        assert_eq!(facing(Some(180.0), 0.0, back, anim::RUN, false, 0.25), 0.0);
        // An action without a target fires the camera's way: the body turns to it.
        assert_eq!(facing(Some(90.0), 0.0, still, anim::CAST, true, 0.25), 0.0);
        // A target-action's turn: the caller passes the mover's yaw and no freedom.
        assert_eq!(
            facing(Some(90.0), 45.0, still, anim::IDLE, false, 0.25),
            45.0
        );
    }

    #[test]
    fn third_person_camera_stays_out_of_walls() {
        let mut world = BoxWorld::floor();
        // A wall right behind the player (yaw 0 looks +x, the camera goes -x).
        world.push(
            Vec3::new(-60.0, -256.0, 0.0),
            Vec3::new(-40.0, 256.0, 200.0),
        );
        let eye = Vec3::new(0.0, 0.0, 46.0);
        let cam = third_person_camera(&world, eye, 0.0, 0.0);
        assert!(cam.x > -40.0, "camera inside the wall: {cam:?}");
        assert!(cam.x < -20.0, "camera not pulled back at all: {cam:?}");
        let open = third_person_camera(&BoxWorld::floor(), eye, 0.0, 0.0);
        assert!((open.x + CAMERA_BACK).abs() < 1e-3);
    }

    #[test]
    fn re_aim_targets_the_point_under_the_crosshair() {
        let world = BoxWorld::floor();
        let eye = Vec3::new(0.0, 0.0, 46.0);
        let cam = third_person_camera(&world, eye, 0.0, 0.0);
        // A body 300 u ahead on the camera ray (the camera sits CAMERA_RIGHT to the right,
        // which is -y at yaw 0): the ray hits it and the eye aims at the hit point, a few
        // degrees to the right of straight ahead.
        let body = Aabb::around(Vec3::new(300.0, -CAMERA_RIGHT, 40.0), Hull::Player);
        let (yaw, pitch) = re_aim(&world, &[body], cam, 0.0, 0.0, eye);
        assert!(yaw > 350.0 && yaw < 359.0, "yaw {yaw}");
        assert!(pitch.abs() < 5.0, "pitch {pitch}");
        // With nothing to hit, the eye ray converges on the far point: nearly parallel.
        let (yaw, _) = re_aim(&world, &[], cam, 0.0, 0.0, eye);
        assert!(!(1.0..=359.0).contains(&yaw), "yaw {yaw}");
    }
}
