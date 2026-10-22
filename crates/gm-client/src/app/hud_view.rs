use super::*;

pub(super) fn role_name(role: u8) -> &'static str {
    match role {
        0 => "heal",
        1 => "tank",
        2 => "scout",
        _ => "dps",
    }
}

pub(super) fn order_name(order: &Order) -> &'static str {
    match order {
        Order::Follow => "follow",
        Order::Hold => "hold",
        Order::MoveTo(_) => "move",
        Order::Attack(_) => "attack",
    }
}

/// The HUD of one frame (COMPANIONS.md 6): the own bars, the squad panel, the creature
/// being fought and the messages in every viewport.
/// What the HUD draws besides the own bars: the squad's health, the creature being
/// fought, and the scale it is all drawn at.
pub(crate) struct HudView<'a> {
    /// The names over the bodies in sight (LOOK.md 13).
    pub tags: &'a [Tag],
    /// The numbers floating over the bodies (LOOK.md 13.8).
    pub pops: &'a [crate::fx::Pop],
    /// The frame is seen from the own eyes: the own numbers have no head to float over
    /// and sit under the aim instead.
    pub first_person: bool,
    /// The own body was just hurt: 1 running down to 0, the frame's red edge.
    pub hurt: f32,
    pub squad: &'a [(Option<u16>, bool)],
    /// The other members of the party: name; `None` when no body of that name is here,
    /// else the health the wire carries for it, when it does.
    pub party: &'a [(String, Option<Option<u16>>)],
    pub target: Option<&'a (String, u16, u16)>,
    pub scale: f32,
    /// The bundle's manifest: where the icons' keys come from (LOOK.md 3).
    pub manifest: Option<&'a gm_model::manifest::Manifest>,
    /// Seconds since the client started: the hotbar's flashes run on it.
    pub time: f32,
    /// The own name, for the portrait frame.
    pub own_name: &'a str,
    /// The combo counter (MODES.md 4.6): hits in the chain, and seconds since the last.
    pub combo: (u32, f32),
    /// The gun mode (MODES.md 3.8): whether the body crouches (the cone's base), and the
    /// scope's zoom this frame (1 without).
    pub crouched: bool,
    pub zoom: f32,
    /// The scope is up: the cone the crosshair shows shrinks with it (MODES.md 10.2).
    pub scoped: bool,
    /// A finger's controls (MODES.md 5.6): the buttons and which are held, and the
    /// stick's centre and knob while one holds it.
    pub touch: TouchHud<'a>,
}

#[derive(Default)]
pub(crate) struct TouchHud<'a> {
    pub buttons: &'a [(TouchButton, ui::Rect)],
    pub held: Vec<TouchButton>,
    pub stick: Option<((f32, f32), (f32, f32))>,
}

/// A name over a body (LOOK.md 13): where its head is, what it is called, the colour of
/// its side, and its health as a part of the whole when the frame knows both.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Tag {
    pub at: Vec3,
    pub name: String,
    pub ink: [f32; 4],
    pub health: Option<f32>,
}

/// One cell of the hotbar (LOOK.md 3.2), as `--report` and the gate read it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct HotbarCell {
    pub key: &'static str,
    pub ability: String,
    /// `ready`, `cooling`, `unaffordable`, `silenced`, `active`.
    pub state: &'static str,
    /// Of the cooldown, 0 (just used) to 1 (ready).
    pub ready: f32,
    /// Seconds until ready, while cooling.
    pub left_secs: f32,
    /// The stage a chain's window has reached (MODES.md 4.3): 0 outside one, else 2, 3...
    pub stage: u8,
    /// An item cell (LOOK.md 3.2): how many of its stack are carried; `None` for an
    /// ability's cell.
    pub count: Option<u16>,
}

/// An item cell of the hotbar (LOOK.md 3.2): its key, what is set on it, how many are
/// carried, and its state: `using` (the sweep runs), `ready`, `empty` (nothing set, or
/// none carried: the cell is drawn bare).
pub(super) fn item_cells(o: &Online, c: &ClientState) -> Vec<HotbarCell> {
    let using = c.mover.using_item(c.tick);
    let progress = c.mover.use_progress(c.tick, c.rate.dt()).unwrap_or(1.0);
    (0..gm_core::sim::BAR_CELLS)
        .map(|i| {
            let template = o.bar.get(i).cloned().flatten();
            let count = c.mover.bar[i];
            let (state, ready) = if using == Some(i as u8) {
                ("using", progress)
            } else if template.is_some() && count > 0 {
                ("ready", 1.0)
            } else {
                ("empty", 1.0)
            };
            HotbarCell {
                key: BAR_KEYS[i],
                ability: template.unwrap_or_else(|| "-".to_string()),
                state,
                ready,
                left_secs: 0.0,
                stage: 0,
                count: Some(count),
            }
        })
        .collect()
}

/// The hotbar's cells for the own body: what each key does and its state now. Every
/// cell is read from the kit (BODIES.md 7.4): the worn weapon's blow, the body skill
/// and the weapon's spell are in it, and not in the build's own lists.
pub(crate) fn hotbar(o: &Online) -> Vec<HotbarCell> {
    let (Some(c), Some(pack)) = (&o.client, &o.pack) else {
        return Vec::new();
    };
    // Every cell is the kit's slot (the body skill stands first among the actives,
    // BODIES.md 3.4, and is no row of the build's: read from the build, the cells after
    // it were each the next one's, so `3` showed the skill after it and did the skill,
    // 2026-10-19).
    let kit = &c.sheet.kit;
    let mut cells = Vec::new();
    let mut cell = |key: &'static str, slot: Option<u8>| {
        cells.extend(hotbar_cell(c, pack, key, slot));
    };
    if kit.mode == gm_core::vocab::Mode::Fps {
        // The FPS mode (MODES.md 3.7): the three weapons, the one in hand lit, then the
        // actives on 4 to 7.
        cell("1", kit.primary);
        cell("2", kit.secondary);
        cell("3", kit.knife);
        for (i, key) in ["4", "5", "6", "7"].into_iter().enumerate() {
            cell(key, kit.actives.get(i).copied().flatten());
        }
        if let Some(held) = cells.get_mut(c.mover.held as usize)
            && held.state == "ready"
        {
            held.state = "active";
        }
        cells.extend(item_cells(o, c));
        return cells;
    }
    // The RPG mode's keys (MODES.md 5.5): the kit on 1 to 6, the guard on Shift.
    let rpg = kit.mode == gm_core::vocab::Mode::Rpg;
    let keys: [&'static str; 7] = if rpg {
        ["1", "2", "Shift", "3", "4", "5", "6"]
    } else {
        ["LMB", "RMB", "X", "1", "2", "3", "4"]
    };
    cell(keys[0], kit.primary);
    cell(keys[1], kit.secondary);
    cell(keys[2], kit.guard);
    for (i, key) in keys[3..].iter().enumerate() {
        cell(key, kit.actives.get(i).copied().flatten());
    }
    cells.extend(item_cells(o, c));
    cells
}

/// One cell of an ability: the key, the ability on the slot (the next stage's in a
/// chain's window, MODES.md 4.3), and its state from the predicted mover. Nothing for
/// an empty slot. The ability's definition is the slot's own (`AbilityId` is the pack's
/// index plus one).
fn hotbar_cell(
    c: &ClientState,
    pack: &ContentPack,
    key: &'static str,
    slot: Option<u8>,
) -> Option<HotbarCell> {
    let kit = &c.sheet.kit;
    let now = c.tick;
    // The pack's row of the kit's ability: its id is its index and one (gm-content).
    let mut slot = slot? as usize;
    let mut def = kit.abilities.get(slot)?.id.0.saturating_sub(1) as usize;
    // A chain's window (MODES.md 4.3): the cell is the next stage's.
    let mut stage = 0;
    if let Some((from, next, _)) = c.mover.chain
        && from as usize == slot
    {
        let mut at = from;
        let mut n = 1u8;
        while at != next && n < 12 {
            match kit.chain_next.get(at as usize).copied().flatten() {
                Some(k) => {
                    at = k;
                    n += 1;
                }
                None => break,
            }
        }
        stage = n;
        slot = next as usize;
        def = kit.abilities[slot].id.0.saturating_sub(1) as usize;
    }
    let (ab, d) = (kit.abilities.get(slot)?, pack.abilities.get(def)?);
    let left = gm_core::sim::tick_delta(c.mover.cooldowns[slot], now).max(0) as f32;
    let whole = ab.cooldown.ticks.max(1) as f32;
    let ready = (1.0 - left / whole).clamp(0.0, 1.0);
    let elemental = kit.elemental.get(slot).copied().unwrap_or(false);
    let running = c
        .mover
        .script
        .as_ref()
        .is_some_and(|s| s.ability as usize == slot)
        || (key == "X" && c.mover.guard != gm_core::sim::GuardState::None);
    let state = if running {
        "active"
    } else if elemental && c.mover.statuses.silenced() {
        "silenced"
    } else if left > 0.0 {
        "cooling"
    } else if c.mover.stamina < ab.cost.stamina as f32 || c.mover.focus < ab.cost.focus as f32 {
        "unaffordable"
    } else {
        "ready"
    };
    Some(HotbarCell {
        key,
        ability: d.key.clone(),
        state,
        ready,
        left_secs: left * c.rate.dt(),
        stage,
        count: None,
    })
}

/// The hotbar's cells (LOOK.md 3.2): `n` squares of 40 dots, 3 apart, bottom centre, the
/// last `items` of them (the item cells) a wider gap after the abilities.
pub(crate) fn hotbar_rects(size: (f32, f32), s: f32, n: usize, items: usize) -> Vec<ui::Rect> {
    let side = 40.0 * s;
    let gap = 3.0 * s;
    let wide = 14.0 * s;
    let abilities = n.saturating_sub(items);
    let mut total = n as f32 * (side + gap) - gap;
    if abilities > 0 && items > 0 {
        total += wide;
    }
    let x0 = ((size.0 - total) * 0.5).round();
    let y0 = size.1 - 16.0 - side;
    (0..n)
        .map(|i| {
            let shift = if i >= abilities && abilities > 0 {
                wide
            } else {
                0.0
            };
            ui::Rect::new(x0 + i as f32 * (side + gap) + shift, y0, side, side)
        })
        .collect()
}

/// The controls a finger may land on (MODES.md 5.6): the menu top right; in the action
/// and gun modes the jump and the secondary bottom right, over the hotbar's line; and
/// the hotbar's cells while a kit is played; and `E` above them while something is in
/// reach to use (`use_in_reach`). Nothing without a finger seen.
pub(crate) fn touch_controls(
    size: (f32, f32),
    s: f32,
    online: Option<&Online>,
    rpg: bool,
    use_in_reach: bool,
) -> Vec<(TouchButton, ui::Rect)> {
    let (w, h) = size;
    let mut out = Vec::new();
    let menu = 36.0 * s;
    out.push((
        TouchButton::Menu,
        ui::Rect::new(w - 16.0 - menu, 16.0, menu, menu),
    ));
    let side = 60.0 * s;
    let gap = 12.0 * s;
    let y = h - 16.0 - side;
    if rpg {
        out.push((
            TouchButton::Secondary,
            ui::Rect::new(w - 16.0 - side, y, side, side),
        ));
    } else {
        out.push((
            TouchButton::Jump,
            ui::Rect::new(w - 16.0 - side, y, side, side),
        ));
        out.push((
            TouchButton::Secondary,
            ui::Rect::new(w - 16.0 - side - gap - side, y, side, side),
        ));
    }
    if use_in_reach {
        out.push((
            TouchButton::Use,
            ui::Rect::new(w - 16.0 - side, y - gap - side, side, side),
        ));
    }
    if let Some(o) = online {
        let cells = hotbar(o);
        let items = cells.iter().filter(|c| c.count.is_some()).count();
        for (i, r) in hotbar_rects(size, s, cells.len(), items)
            .into_iter()
            .enumerate()
        {
            out.push((TouchButton::Hot(i as u8), r));
        }
    }
    out
}

/// Where a world point is on a screen of `size` pixels; `None` behind the camera.
pub(crate) fn project(view_proj: glam::Mat4, size: (f32, f32), point: Vec3) -> Option<(f32, f32)> {
    let clip = view_proj * point.extend(1.0);
    if clip.w <= 1.0 {
        return None;
    }
    let (x, y) = (clip.x / clip.w, clip.y / clip.w);
    Some(((x + 1.0) * 0.5 * size.0, (1.0 - y) * 0.5 * size.1))
}

/// The HUD of one frame (COMPANIONS.md 6, LOOK.md 3): a finger's controls, the hurt
/// edge, the names and the numbers over the bodies, the aim and the combo; then, while
/// a zone is played, the gun's crosshair and magazine, the own portrait and bars, the
/// hotbar with the statuses over it, the squad and the party, the creature fought and
/// the messages. All on the first layer, in call order: a screen's plates (the same
/// layer, drawn after) cover it, as they covered the bars before there were layers.
pub(crate) fn build_hud(hud: &mut Hud, online: Option<&Online>, vp: glam::Mat4, view: HudView<'_>) {
    let HudView {
        tags,
        pops,
        first_person,
        hurt,
        squad: squad_view,
        party,
        target,
        scale: s,
        manifest,
        time,
        own_name,
        combo,
        crouched,
        zoom,
        scoped,
        touch,
    } = view;
    let (w, h) = hud.size;
    // The HUD's words: the text face of the bundle, the small one without it.
    let cap = hud.cap();
    let line = (cap + 5.0) * s;
    let skinned = hud.skinned;
    hud_touch(hud, &touch, s, cap);
    hud.set_layer(ui::LAYER_PLATES);
    hud_hurt(hud, hurt, w, h);
    hud_tags(hud, vp, tags, s, cap);
    hud_pops(hud, vp, pops, s, first_person);
    hud_aim(hud, w, h, s, combo);
    let Some(o) = online else { return };
    let Some(c) = &o.client else { return };
    hud_gun(hud, c, s, cap, line, crouched, scoped, zoom);
    let mut y = hud_own(hud, o, c, s, cap, line, skinned, manifest, own_name);
    if skinned {
        hud_hotbar(hud, o, c, s, cap, line, manifest);
    }
    let _ = time;
    y = hud_squad(hud, o, squad_view, s, line, y);
    hud_party(hud, party, s, line, skinned, y);
    hud_target_and_messages(hud, o, target, s, line, w);
}

/// As much of `text` as fits in `room` pixels.
fn fit(hud: &Hud, s: f32, text: &str, room: f32) -> String {
    let mut out = String::new();
    for c in text.chars() {
        out.push(c);
        if hud.width(s, &out) > room {
            out.pop();
            break;
        }
    }
    out
}

/// A finger's controls (MODES.md 5.6): the stick where the finger landed, and the
/// buttons in the corners, lit while held. The hotbar's cells are their own buttons.
fn hud_touch(hud: &mut Hud, touch: &TouchHud<'_>, s: f32, cap: f32) {
    if let Some((centre, knob)) = touch.stick {
        let reach = touch::STICK_DOTS * s;
        let c = Vec2::new(centre.0, centre.1);
        hud.wedge(c, reach, 0.0, 1.0, [1.0, 1.0, 1.0, 0.12]);
        let len = (knob.0 * knob.0 + knob.1 * knob.1).sqrt();
        let k = if len > reach {
            Vec2::new(knob.0 / len * reach, knob.1 / len * reach)
        } else {
            Vec2::new(knob.0, knob.1)
        };
        hud.wedge(c + k, reach * 0.4, 0.0, 1.0, [1.0, 1.0, 1.0, 0.35]);
    }
    for (button, r) in touch.buttons {
        let word = match button {
            TouchButton::Menu => "menu",
            TouchButton::Jump => "jump",
            TouchButton::Secondary => "2",
            TouchButton::Use => "E",
            TouchButton::Hot(_) => continue,
        };
        let held = touch.held.contains(button);
        let fill = if held {
            [1.0, 0.85, 0.3, 0.55]
        } else {
            [0.0, 0.0, 0.0, 0.35]
        };
        if !hud.frame(r.x, r.y, r.w, r.h, "hotbar_cell", s, hud::PLAIN) {
            hud.rect(r.x, r.y, r.w, r.h, [0.1, 0.1, 0.12, 0.6]);
        }
        hud.rect(
            r.x + 2.0 * s,
            r.y + 2.0 * s,
            r.w - 4.0 * s,
            r.h - 4.0 * s,
            fill,
        );
        let tw = hud.width(s, word);
        hud.print(
            r.x + (r.w - tw) * 0.5,
            r.y + (r.h - cap * s) * 0.5,
            s,
            hud::WHITE,
            word,
        );
    }
}

/// Hurt: the frame's edge goes red for a moment (LOOK.md 13).
fn hud_hurt(hud: &mut Hud, hurt: f32, w: f32, h: f32) {
    if hurt <= 0.0 {
        return;
    }
    for (k, part) in [0.035_f32, 0.07, 0.11].into_iter().enumerate() {
        let (tx, ty) = (w * part, h * part);
        let red = [0.85, 0.05, 0.03, 0.22 * hurt / (k as f32 + 1.0)];
        hud.rect(0.0, 0.0, w, ty, red);
        hud.rect(0.0, h - ty, w, ty, red);
        hud.rect(0.0, ty, tx, h - 2.0 * ty, red);
        hud.rect(w - tx, ty, tx, h - 2.0 * ty, red);
    }
}

/// The names over the bodies, under everything else of the HUD: small print, a shade
/// behind it, the health under it where it is known.
fn hud_tags(hud: &mut Hud, vp: glam::Mat4, tags: &[Tag], s: f32, cap: f32) {
    let print = (s * 0.5).max(1.0);
    for tag in tags {
        let Some((x, y)) = project(vp, hud.size, tag.at) else {
            continue;
        };
        let tw = hud.width(print, &tag.name);
        let (tx, ty) = ((x - tw * 0.5).round(), (y - (cap + 3.0) * print).round());
        hud.print(tx + 1.0, ty + 1.0, print, [0.0, 0.0, 0.0, 0.75], &tag.name);
        hud.print(tx, ty, print, tag.ink, &tag.name);
        if let Some(frac) = tag.health {
            let bw = (40.0 * print).max(tw * 0.8);
            hud.bar(
                (x - bw * 0.5).round(),
                y.round(),
                bw,
                3.0 * print,
                frac,
                tag.ink,
            );
        }
    }
}

/// The numbers (LOOK.md 13.8): in the title face over the head they belong to, what the
/// own hand dealt the largest, what others dealt and healed the smallest (13.13); each
/// floats up as it fades, with a shade behind. The own hurts in the first person have no
/// head in the frame: they fall from under the aim instead, where the eyes are.
fn hud_pops(hud: &mut Hud, vp: glam::Mat4, pops: &[crate::fx::Pop], s: f32, first_person: bool) {
    use crate::fx::Blow;
    let (w, h) = hud.size;
    let face = if hud.has_face(hud::FaceId::Title) {
        hud::FaceId::Title
    } else {
        hud.words()
    };
    let (_, ascent) = hud.metrics(face);
    for pop in pops {
        let own = matches!(pop.blow, Blow::Taken | Blow::Healed);
        let (x, y) = if own && first_person {
            (w * 0.5, h * 0.5 + 100.0 * s - pop.lift * 40.0 * s)
        } else {
            let Some((x, y)) = project(vp, hud.size, pop.at) else {
                continue;
            };
            (x, y - pop.lift * 44.0 * s)
        };
        let print = match pop.blow {
            Blow::Dealt | Blow::Mended => s * 1.4,
            Blow::Taken | Blow::Healed => s,
            Blow::Blocked => (s * 0.6).max(1.0),
            Blow::Seen | Blow::SeenHealed => (s * 0.7).max(1.0),
        };
        let tw = hud.width_in(face, print, &pop.text);
        let (tx, ty) = ((x - tw * 0.5).round(), (y - ascent * print).round());
        let shade = [0.0, 0.0, 0.0, 0.8 * pop.ink[3]];
        hud.text_in(face, tx + 1.0, ty + 1.0, print, shade, &pop.text);
        hud.text_in(face, tx, ty, print, pop.ink, &pop.text);
    }
}

/// The aim: a dot in the middle. The combo counter (MODES.md 4.6), right of the aim:
/// two hits or more within two seconds of each other, fading over the two seconds
/// after the last.
fn hud_aim(hud: &mut Hud, w: f32, h: f32, s: f32, combo: (u32, f32)) {
    hud.rect(w * 0.5 - 2.0, h * 0.5 - 2.0, 4.0, 4.0, hud::SHADE);
    hud.rect(w * 0.5 - 1.0, h * 0.5 - 1.0, 2.0, 2.0, hud::WHITE);
    if combo.0 >= 2 && combo.1 < COMBO_SECS {
        let fade = 1.0 - combo.1 / COMBO_SECS;
        let text = format!("{} hits", combo.0);
        let print = s * 1.2;
        let ink = [1.0, 0.85, 0.3, fade];
        hud.text(
            w * 0.5 + 18.0 * s + 1.0,
            h * 0.5 - 8.0 * s + 1.0,
            print,
            [0.0, 0.0, 0.0, 0.8 * fade],
            &text,
        );
        hud.text(w * 0.5 + 18.0 * s, h * 0.5 - 8.0 * s, print, ink, &text);
    }
}

/// The gun mode (MODES.md 3.8): the crosshair opens with the cone, four lines whose gap
/// is the cone's angle on the screen; the magazine over the reserve bottom right, in
/// red below half a magazine, "reloading" while the hands are at it; the scope's mask
/// and its lines.
#[allow(clippy::too_many_arguments)]
fn hud_gun(
    hud: &mut Hud,
    c: &ClientState,
    s: f32,
    cap: f32,
    line: f32,
    crouched: bool,
    scoped: bool,
    zoom: f32,
) {
    let Some((f, g)) = c.mover.gun_in_hand(&c.sheet.kit) else {
        return;
    };
    let (w, h) = hud.size;
    let now = c.tick;
    let cone = gm_core::sim::cone_deg(
        f,
        c.sheet.derived.max_speed,
        &c.mover,
        g,
        crouched,
        scoped,
        g.spray,
        now,
    );
    let per_deg = h / (crate::render::fov_y_deg() / zoom.max(1.0));
    let gap = (cone * per_deg).max(4.0 * s);
    let len = 6.0 * s;
    let (cx, cy) = (w * 0.5, h * 0.5);
    for (dx, dy, lw, lh) in [
        (-gap - len, -s * 0.5, len, s),
        (gap, -s * 0.5, len, s),
        (-s * 0.5, -gap - len, s, len),
        (-s * 0.5, gap, s, len),
    ] {
        hud.rect(cx + dx - 1.0, cy + dy - 1.0, lw + 2.0, lh + 2.0, hud::SHADE);
        hud.rect(cx + dx, cy + dy, lw, lh, hud::WHITE);
    }
    if zoom > 1.0 {
        let r = h * 0.42;
        let mask = [0.0, 0.0, 0.0, 0.92];
        hud.rect(0.0, 0.0, w, cy - r, mask);
        hud.rect(0.0, cy + r, w, h - cy - r, mask);
        hud.rect(0.0, cy - r, cx - r, 2.0 * r, mask);
        hud.rect(cx + r, cy - r, w - cx - r, 2.0 * r, mask);
        hud.rect(cx - r, cy - s * 0.5, 2.0 * r, s, [0.0, 0.0, 0.0, 0.6]);
        hud.rect(cx - s * 0.5, cy - r, s, 2.0 * r, [0.0, 0.0, 0.0, 0.6]);
    }
    let low = (g.magazine as u32) * 2 < f.magazine as u32 || g.magazine == 0;
    let ink = if low { hud::RED } else { hud::WHITE };
    let text = format!("{} / {}", g.magazine, g.reserve);
    let print = s * 2.0;
    let tw = hud.width(print, &text);
    // (Above the corner's "Esc menu" line.)
    let (x, y) = (w - 16.0 - tw, h - 16.0 - line - cap * print);
    hud.print(x + 1.0, y + 1.0, print, hud::SHADE, &text);
    hud.print(x, y, print, ink, &text);
    if c.mover.reloading(now) {
        let word = "reloading";
        let tw = hud.width(s, word);
        hud.label(w - 16.0 - tw, y - line, s, hud::YELLOW, word);
    }
}

/// The own body, top left (LOOK.md 3.1): a portrait in its frame, the name, the three
/// bars; without a skin, the bars alone as before, bottom left. Returns where the
/// squad's lines begin under it.
#[allow(clippy::too_many_arguments)]
fn hud_own(
    hud: &mut Hud,
    o: &Online,
    c: &ClientState,
    s: f32,
    cap: f32,
    line: f32,
    skinned: bool,
    manifest: Option<&gm_model::manifest::Manifest>,
    own_name: &str,
) -> f32 {
    let (_, h) = hud.size;
    let max_health = c.sheet.derived.health.max(1) as f32;
    let rows: [(&str, f32, f32, [f32; 4]); 3] = [
        ("HP", c.own_health.max(0) as f32, max_health, hud::RED),
        (
            "ST",
            c.mover.stamina,
            c.sheet.derived.stamina.max(1.0),
            hud::YELLOW,
        ),
        (
            "MP",
            c.mover.focus,
            c.sheet.derived.focus.max(1.0),
            hud::BLUE,
        ),
    ];
    let mut y = if skinned {
        16.0
    } else {
        h - 16.0 - line * rows.len() as f32
    };
    if c.mover.commanding(c.tick) {
        let at = if skinned {
            h - 16.0 - 60.0 * s
        } else {
            y - line
        };
        hud.label(16.0, at, s, hud::YELLOW, "command stance");
    }
    if !skinned {
        for (name, have, max, colour) in rows {
            hud.label(16.0, y, s, hud::WHITE, name);
            let x = 16.0 + hud.width(s, "HP ") + 4.0 * s;
            hud.bar(x, y + s, 180.0, cap * s - 2.0 * s, have / max, colour);
            hud.label(
                x + 188.0,
                y,
                s,
                hud::DIM,
                &format!("{:.0}/{:.0}", have, max),
            );
            y += line;
        }
        return 16.0;
    }
    let side = 48.0 * s;
    let build = &c.sheet.build;
    // The frame's portrait (CONTENT.md 5.3): the official avatar's where the bundle has
    // one, else the mannequin's.
    let portrait = manifest.and_then(|m| {
        m.portrait(gm_model::rig::frame_name(build.frame))
            .map(str::to_string)
    });
    hud.frame(16.0, y, side, side, "portrait_frame", s, hud::PLAIN);
    if let Some(key) = &portrait {
        hud.icon(
            16.0 + 6.0 * s,
            y + 6.0 * s,
            side - 12.0 * s,
            key,
            hud::PLAIN,
        );
    }
    let x = 16.0 + side + 6.0 * s;
    let (lh, _) = hud.metrics(hud::FaceId::Text);
    // The name the zone announced the own body by, else what the command line said.
    let name = o
        .names
        .get(&c.my_id)
        .map(|n| n.0.as_str())
        .filter(|n| !n.is_empty())
        .unwrap_or(own_name);
    hud.text_in(hud::FaceId::Text, x, y, s, hud::WHITE, name);
    let mut by = y + lh * s + 2.0 * s;
    let bh = 10.0 * s;
    for (_, have, max, colour) in rows {
        hud.frame(x, by, 150.0 * s, bh + 4.0 * s, "bar_frame", s, hud::PLAIN);
        let inner_w = (150.0 * s - 6.0 * s) * (have / max).clamp(0.0, 1.0);
        if inner_w > 0.0 {
            hud.image(x + 3.0 * s, by + 2.0 * s, inner_w, bh, "bar_fill", colour);
        }
        // The numbers, with a shade under them: white on a yellow bar is not read.
        let text = format!("{:.0}/{:.0}", have, max);
        let tw = hud.width(s, &text);
        let (tx, ty) = (
            x + 150.0 * s - tw - 4.0 * s,
            by + 2.0 * s + (bh - cap * s) * 0.5,
        );
        hud.print(tx + 1.0, ty + 1.0, s, [0.0, 0.0, 0.0, 0.7], &text);
        hud.print(tx, ty, s, hud::WHITE, &text);
        by += bh + 6.0 * s;
    }
    by + 4.0 * s
}

/// The hotbar, bottom centre (LOOK.md 3.2): a cell per ability of the kit, its icon or
/// its glyph, its key, and its state from the predicted mover; the item cells after
/// them; why the last item key used nothing; the own statuses above it.
fn hud_hotbar(
    hud: &mut Hud,
    o: &Online,
    c: &ClientState,
    s: f32,
    cap: f32,
    line: f32,
    manifest: Option<&gm_model::manifest::Manifest>,
) {
    let (w, h) = hud.size;
    let cells = hotbar(o);
    let items = cells.iter().filter(|c| c.count.is_some()).count();
    let rects = hotbar_rects((w, h), s, cells.len(), items);
    let side = 40.0 * s;
    let (x0, y0) = rects.first().map_or((0.0, h - 16.0 - side), |r| (r.x, r.y));
    // The cells without a picture show their ability's name (LOOK.md 3.4), whole and
    // all in one size: small print when any of them is wider than a cell.
    let name_room = side - 8.0 * s;
    let name_print = if cells.iter().all(|c| hud.width(s, &c.ability) <= name_room) {
        s
    } else {
        (s * 0.5).max(1.0)
    };
    for (i, cell) in cells.iter().enumerate() {
        let x = rects.get(i).map_or(x0, |r| r.x);
        hud.frame(x, y0, side, side, "hotbar_cell", s, hud::PLAIN);
        if cell.count.is_some() {
            hud_item_cell(hud, cell, manifest, x, y0, side, s, cap);
        } else {
            hud_ability_cell(hud, cell, manifest, x, y0, side, s, cap, name_print);
        }
        // The key, in its tab at the top left of the cell.
        let kw = hud.width(s, cell.key) + 4.0 * s;
        hud.frame(
            x - s,
            y0 - 4.0 * s,
            kw,
            (cap + 4.0) * s,
            "hotbar_key",
            s,
            hud::PLAIN,
        );
        hud.print(x + s, y0 - 2.0 * s, s, hud::WHITE, cell.key);
    }
    // Why the last press of an item key used nothing (MODES.md 11.3): in yellow over
    // the item cells for a second.
    if let Some(why) = kit_note_of(o.kit_note)
        && let Some(first) = rects.get(cells.len().saturating_sub(items))
    {
        let last = rects.last().unwrap_or(first);
        let mid = (first.x + last.x + last.w) * 0.5;
        let tw = hud.width(s, why);
        hud.label(
            (mid - tw * 0.5).round(),
            y0 - (cap + 4.0) * s - line,
            s,
            hud::YELLOW,
            why,
        );
    }
    hud_statuses(hud, c, manifest, s, cap, x0, y0 - 30.0 * s);
}

/// An item cell (LOOK.md 3.2): its stack's icon or name and the count; bare when
/// nothing is set on it or none is carried, and the sweep while it is used.
#[allow(clippy::too_many_arguments)]
fn hud_item_cell(
    hud: &mut Hud,
    cell: &HotbarCell,
    manifest: Option<&gm_model::manifest::Manifest>,
    x: f32,
    y0: f32,
    side: f32,
    s: f32,
    cap: f32,
) {
    let Some(count) = cell.count else { return };
    if cell.state == "empty" {
        return;
    }
    let icon = manifest
        .and_then(|m| m.template(&cell.ability))
        .and_then(|t| t.icon.clone());
    let inner = side - 8.0 * s;
    let drawn = icon
        .as_deref()
        .is_some_and(|k| hud.icon(x + 4.0 * s, y0 + 4.0 * s, inner, k, hud::PLAIN));
    if !drawn {
        let print = (s * 0.5).max(1.0);
        let mut short = cell.ability.clone();
        while hud.width(print, &short) > inner && short.pop().is_some() {}
        let tw = hud.width(print, &short);
        hud.print(
            x + (side - tw) * 0.5,
            y0 + (side - cap * print) * 0.5 - 3.0 * s,
            print,
            hud::WHITE,
            &short,
        );
    }
    let n = format!("×{count}");
    let tw = hud.width(s, &n);
    hud.print(
        x + side - tw - 3.0 * s + 1.0,
        y0 + side - (cap + 2.0) * s + 1.0,
        s,
        hud::SHADE,
        &n,
    );
    hud.print(
        x + side - tw - 3.0 * s,
        y0 + side - (cap + 2.0) * s,
        s,
        hud::WHITE,
        &n,
    );
    if cell.state == "using" {
        let centre = Vec2::new(x + side * 0.5, y0 + side * 0.5);
        hud.wedge(centre, side * 0.72, cell.ready, 1.0, [0.0, 0.0, 0.0, 0.62]);
    }
}

/// An ability's cell: its icon or its name, dim when it cannot be afforded or is
/// silenced; the cooldown's sweep with the seconds, the silence's bar, the active's
/// rim; a chain's stage (MODES.md 4.3) bottom right.
#[allow(clippy::too_many_arguments)]
fn hud_ability_cell(
    hud: &mut Hud,
    cell: &HotbarCell,
    manifest: Option<&gm_model::manifest::Manifest>,
    x: f32,
    y0: f32,
    side: f32,
    s: f32,
    cap: f32,
    name_print: f32,
) {
    let icon = manifest
        .and_then(|m| m.ability(&cell.ability))
        .and_then(|a| a.icon.clone());
    let inner = side - 8.0 * s;
    let tint = match cell.state {
        "unaffordable" | "silenced" => [0.45, 0.45, 0.5, 1.0],
        _ => hud::PLAIN,
    };
    let drawn = icon
        .as_deref()
        .is_some_and(|k| hud.icon(x + 4.0 * s, y0 + 4.0 * s, inner, k, tint));
    if !drawn {
        let print = name_print;
        let mut short = cell.ability.clone();
        while hud.width(print, &short) > inner && short.pop().is_some() {}
        let tw = hud.width(print, &short);
        hud.print(
            x + (side - tw) * 0.5,
            y0 + (side - cap * print) * 0.5,
            print,
            if tint == hud::PLAIN {
                hud::WHITE
            } else {
                hud::DIM
            },
            &short,
        );
    }
    match cell.state {
        "cooling" => {
            // The dark sweep, clockwise from twelve, over what is left; the seconds
            // when more than one.
            let centre = Vec2::new(x + side * 0.5, y0 + side * 0.5);
            hud.wedge(centre, side * 0.72, cell.ready, 1.0, [0.0, 0.0, 0.0, 0.62]);
            if cell.left_secs >= 1.0 {
                let t = format!("{:.0}", cell.left_secs.ceil());
                let tw = hud.width(s, &t);
                hud.print(
                    x + (side - tw) * 0.5,
                    y0 + (side - cap * s) * 0.5,
                    s,
                    hud::WHITE,
                    &t,
                );
            }
        }
        "silenced" => {
            hud.rect(
                x + 4.0 * s,
                y0 + side * 0.5 - s,
                side - 8.0 * s,
                2.0 * s,
                hud::RED,
            );
        }
        "active" => {
            let rim = [1.0, 0.85, 0.3, 1.0];
            hud.rect(x, y0, side, s, rim);
            hud.rect(x, y0 + side - s, side, s, rim);
            hud.rect(x, y0, s, side, rim);
            hud.rect(x + side - s, y0, s, side, rim);
        }
        _ => {}
    }
    if cell.stage >= 2 {
        let numeral = ["", "I", "II", "III", "IV", "V"]
            .get(cell.stage as usize)
            .copied()
            .unwrap_or("V+");
        let tw = hud.width(s, numeral);
        hud.print(
            x + side - tw - 3.0 * s,
            y0 + side - (cap + 2.0) * s,
            s,
            [1.0, 0.85, 0.3, 1.0],
            numeral,
        );
    }
}

/// The statuses (LOOK.md 3.3), above the hotbar: an icon or the status's name, a ring
/// of the time left as a wedge, the stacks.
fn hud_statuses(
    hud: &mut Hud,
    c: &ClientState,
    manifest: Option<&gm_model::manifest::Manifest>,
    s: f32,
    cap: f32,
    x0: f32,
    sy: f32,
) {
    let now = c.tick;
    let mut sx = x0;
    for slot in c.mover.statuses.active() {
        let Some(status) = slot.status else { continue };
        let left = gm_core::sim::tick_delta(slot.until, now).max(0) as f32;
        let side = 24.0 * s;
        let harmful = !matches!(
            status,
            gm_core::vocab::Status::Regen
                | gm_core::vocab::Status::Haste
                | gm_core::vocab::Status::Fortify
                | gm_core::vocab::Status::Stealth
        );
        hud.rect(
            sx,
            sy,
            side,
            side,
            if harmful {
                [0.35, 0.08, 0.06, 0.85]
            } else {
                [0.08, 0.3, 0.1, 0.85]
            },
        );
        let icon = manifest.and_then(|m| m.status_icon(status.name()).map(str::to_string));
        let drawn = icon
            .as_deref()
            .is_some_and(|k| hud.icon(sx + 2.0 * s, sy + 2.0 * s, side - 4.0 * s, k, hud::PLAIN));
        if !drawn {
            let short = fit(hud, s, status.name(), side - 4.0 * s);
            hud.print(sx + 2.0 * s, sy + 2.0 * s, s, hud::WHITE, &short);
        }
        // The time left, as a sweep that empties: a status of 5 s is nearly whole at 4.
        let frac = (left / 320.0).clamp(0.0, 1.0);
        hud.wedge(
            Vec2::new(sx + side * 0.5, sy + side * 0.5),
            side * 0.6,
            frac,
            1.0,
            [0.0, 0.0, 0.0, 0.5],
        );
        if slot.stacks > 1 {
            let t = slot.stacks.to_string();
            let tw = hud.width(s, &t);
            hud.print(
                sx + side - tw - s,
                sy + side - cap * s - s,
                s,
                hud::YELLOW,
                &t,
            );
        }
        sx += side + 3.0 * s;
    }
}

/// The squad, top left under the own body: slot, name, role, order, and its health.
/// Returns where the next line goes.
fn hud_squad(
    hud: &mut Hud,
    o: &Online,
    squad_view: &[(Option<u16>, bool)],
    s: f32,
    line: f32,
    mut y: f32,
) -> f32 {
    for (i, m) in o.squad.iter().enumerate() {
        let (health, alive) = squad_view.get(i).copied().unwrap_or((None, false));
        let colour = if alive { hud::WHITE } else { hud::RED };
        let text = format!(
            "{} {}  {}  {}",
            i + 1,
            m.name,
            role_name(m.role),
            if alive { order_name(&m.order) } else { "down" }
        );
        hud.label(16.0, y, s, colour, &text);
        y += line;
        let frac = health.map_or(0.0, |v| v as f32 / m.max_health.max(1) as f32);
        hud.bar(16.0, y - 2.0 * s, 150.0, 3.0 * s, frac, hud::GREEN);
        y += 6.0 * s;
    }
    y
}

/// The party, under the squad (PARTY.md 8.2, LOOK.md 3.1): each member's health when
/// the wire carries it (its body is here, in sight, and of the party in this fight), its
/// name alone when its body is here and the wire does not, `away` when it is not here;
/// with a skin, each in a small frame with a bar.
fn hud_party(
    hud: &mut Hud,
    party: &[(String, Option<Option<u16>>)],
    s: f32,
    line: f32,
    skinned: bool,
    mut y: f32,
) {
    for (name, here) in party {
        let (text, colour) = match here {
            Some(Some(0)) => (format!("{name}  down"), hud::RED),
            Some(Some(health)) => (format!("{name}  {health}"), hud::WHITE),
            Some(None) => (name.clone(), hud::WHITE),
            None => (format!("{name}  away"), hud::DIM),
        };
        if skinned {
            let fw = 150.0 * s;
            let fh = 22.0 * s;
            hud.frame(16.0, y, fw, fh, "well", s, hud::PLAIN);
            if let Some(Some(health)) = here {
                let max = 400.0f32;
                let frac = (*health as f32 / max).clamp(0.0, 1.0);
                hud.image(
                    16.0 + 3.0 * s,
                    y + fh - 7.0 * s,
                    (fw - 6.0 * s) * frac,
                    4.0 * s,
                    "bar_fill",
                    hud::GREEN,
                );
            }
            hud.print(16.0 + 4.0 * s, y + 3.0 * s, s, colour, &text);
            y += fh + 3.0 * s;
        } else {
            hud.label(16.0, y, s, colour, &text);
            y += line;
        }
    }
}

/// The creature being fought, top middle; messages under it.
fn hud_target_and_messages(
    hud: &mut Hud,
    o: &Online,
    target: Option<&(String, u16, u16)>,
    s: f32,
    line: f32,
    w: f32,
) {
    let mut y = 16.0;
    if let Some((name, health, max)) = target {
        let tw = hud.width(s, name);
        hud.label((w - tw) * 0.5, y, s, hud::WHITE, name);
        y += line;
        let bw = (w * 0.34).min(520.0);
        hud.bar(
            (w - bw) * 0.5,
            y,
            bw,
            5.0 * s,
            *health as f32 / (*max).max(1) as f32,
            hud::RED,
        );
        let count = format!("{health}/{max}");
        let small = (s * 0.5).max(1.0);
        hud.print(
            (w - hud.width(small, &count)) * 0.5,
            y + 0.75 * s,
            small,
            hud::WHITE,
            &count,
        );
        y += 5.0 * s + 10.0;
    }
    for (at, text, colour) in &o.messages {
        if at.elapsed().as_secs_f32() > MESSAGE_SECS {
            continue;
        }
        // A long line is cut to the screen: the log has the whole of it.
        let shown = fit(hud, s, text, w - 40.0);
        let tw = hud.width(s, &shown);
        hud.label((w - tw) * 0.5, y, s, *colour, &shown);
        y += line;
    }
}

#[cfg(test)]
mod kit_tests {
    use super::*;
    use gm_core::sim::buttons;

    /// MODES.md 11.3, LOOK.md 3.2: `F` uses a kit in every mode (the first item cell's
    /// key), and `8`, `9`, `0` the other cells. The press is sampled per tick from the
    /// keys pressed since the last one; the RPG mode builds its frame another way and
    /// must still read them.
    #[test]
    fn f_is_the_kit_in_every_mode() {
        let modes: [(&str, bool, Option<crate::rpg::RpgFrame>); 3] = [
            ("action", false, None),
            ("fps", true, None),
            ("rpg", false, Some(crate::rpg::RpgFrame::default())),
        ];
        for (mode, gun, rpg) in modes {
            for (i, key) in BAR_KEY_CODES.iter().enumerate() {
                let mut input = Input::default();
                input.keys.insert(*key);
                input.just_pressed.insert(*key);
                let first = input.sim_input(0.0, 0.0, None, gun, rpg);
                assert!(
                    first.buttons & buttons::USE != 0,
                    "{mode}: {} just pressed is USE",
                    BAR_KEYS[i]
                );
                assert_eq!(
                    first.use_slot,
                    i as u8 + 1,
                    "{mode}: the cell of {}",
                    BAR_KEYS[i]
                );
                // Held on, it is pressed once: the next tick carries it no more.
                let second = input.sim_input(0.0, 0.0, None, gun, rpg);
                assert!(
                    second.buttons & buttons::USE == 0 && second.use_slot == 0,
                    "{mode}: {} held is not pressed again",
                    BAR_KEYS[i]
                );
            }
        }
    }

    /// MODES.md 11.3: the item cells say for a second why a press used nothing.
    #[test]
    fn the_hud_says_why_a_kit_was_refused_for_a_second() {
        let now = Instant::now();
        let ago = now - std::time::Duration::from_secs_f32(KIT_NOTE_SECS + 0.1);
        assert_eq!(kit_note_of(None), None);
        for why in [
            KIT_FULL,
            gm_core::sim::ItemRefusal::InTheAir.word(),
            gm_core::sim::ItemRefusal::HandsBusy.word(),
            gm_core::sim::ItemRefusal::Empty.word(),
        ] {
            assert_eq!(kit_note_of(Some((now, why))), Some(why));
            assert_eq!(kit_note_of(Some((ago, why))), None);
        }
        assert_eq!(KIT_FULL, "at full health");
    }

    /// LOOK.md 3.2: the item cells sit after the abilities, a wider gap between, and the
    /// bar as a whole stays centred.
    #[test]
    fn the_item_cells_follow_the_abilities_a_gap_apart() {
        let s = 2.0;
        let rects = hotbar_rects((1280.0, 720.0), s, 7 + 4, 4);
        assert_eq!(rects.len(), 11);
        let step = rects[1].x - rects[0].x;
        assert_eq!(step, 43.0 * s);
        assert_eq!(
            rects[7].x - rects[6].x,
            step + 14.0 * s,
            "the gap before the items"
        );
        assert_eq!(rects[8].x - rects[7].x, step);
        let left = rects[0].x;
        let right = 1280.0 - (rects[10].x + rects[10].w);
        assert!((left - right).abs() <= 1.0, "{left} vs {right}");
        // Without items, as before.
        let plain = hotbar_rects((1280.0, 720.0), s, 7, 0);
        assert_eq!(plain[6].x - plain[5].x, step);
    }
}
