//! The town's crouch yard (MODES.md 12): a crawl hole only a crouch goes through and a stack
//! of crates only a crouch-jump reaches, on the committed map with the real movement and the
//! real crouched hull. Rebuild the map with `gm-tools map gen-town` and `map build` after
//! moving either.

use std::path::PathBuf;

use glam::Vec3;
use gm_bsp::Bsp;
use gm_core::movement::{MoveInput, MoveVars, player_move};
use gm_core::sim::{Mover, crouch_step};

const DT: f32 = 1.0 / 64.0;
const REST_Z: f32 = 24.0;

fn town() -> Bsp {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/maps/built/town.bsp");
    Bsp::load(&path).expect("town.bsp must exist; run gm-tools map build")
}

/// One frame of a body as `step_mover` moves it: the posture first, then the movement, at
/// half pace crouched.
fn frame(bsp: &Bsp, m: &mut Mover, yaw: f32, forward: f32, crouch: bool, jump: bool) {
    let jumps = jump && !m.mv.jump_held;
    crouch_step(bsp, m, crouch, jumps, DT);
    let pace = if m.crouched { 0.5 } else { 1.0 };
    let input = MoveInput {
        yaw,
        forward: forward * pace,
        side: 0.0,
        jump,
    };
    player_move(bsp, &MoveVars::QUAKE, &mut m.mv, &input, DT);
}

fn frames(bsp: &Bsp, m: &mut Mover, n: usize, forward: f32, crouch: bool) {
    for _ in 0..n {
        frame(bsp, m, 90.0, forward, crouch, false);
    }
}

#[test]
fn the_crawl_hole_lets_a_crouch_through_and_not_a_stand() {
    let bsp = town();
    // North up the street, at the hole (x -112..-48 in the wall at y 896..912).
    let mut m = Mover::new(Vec3::new(-80.0, 820.0, REST_Z + 8.0), 90.0);
    frames(&bsp, &mut m, 128, 1.0, false);
    assert!(m.mv.on_ground);
    assert!(
        (m.mv.origin.y - 880.0).abs() < 1.0,
        "standing it stops at the wall: {:?}",
        m.mv.origin
    );
    frames(&bsp, &mut m, 192, 1.0, true);
    assert!(m.crouched);
    assert!(
        m.mv.origin.y > 940.0,
        "crouched it went through: {:?}",
        m.mv.origin
    );
    // In the yard it stands up by itself.
    frames(&bsp, &mut m, 8, 0.0, false);
    assert!(!m.crouched);
}

/// Run north at the crates (x 32..128 from y 784, 52 u high) and jump where the face is
/// reached a third of a second into the jump, crouching in the air if `crouch`; where the
/// body ends.
fn jump_at_the_crates(crouch: bool) -> Vec3 {
    let bsp = town();
    let mut m = Mover::new(Vec3::new(80.0, 450.0, REST_Z + 8.0), 90.0);
    frames(&bsp, &mut m, 32, 1.0, false);
    let speed = m.mv.velocity.y;
    assert!(speed > 300.0, "the run up: {speed}");
    let take_off = 784.0 - 16.0 - speed * 0.34;
    while m.mv.origin.y + speed * DT < take_off {
        frame(&bsp, &mut m, 90.0, 1.0, false, false);
    }
    frame(&bsp, &mut m, 90.0, 1.0, crouch, true);
    frames(&bsp, &mut m, 64, 1.0, crouch);
    frames(&bsp, &mut m, 16, 0.0, crouch);
    m.mv.origin
}

#[test]
fn a_jump_falls_short_of_the_crates_and_a_crouch_jump_lands_on_them() {
    let plain = jump_at_the_crates(false);
    assert!(
        (plain.z - REST_Z).abs() < 0.5 && plain.y < 770.0,
        "a jump falls back off the face: {plain:?}"
    );
    let tucked = jump_at_the_crates(true);
    assert!(
        (tucked.z - (52.0 + REST_Z)).abs() < 0.5 && tucked.y > 784.0,
        "a crouch-jump lands on top: {tucked:?}"
    );
}
