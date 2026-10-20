//! The character's page (CLIENT.md 4.5): the body, its thirty points, its elements and
//! what its hands bring, read, not edited (MATRIX.md 20, 2026-10-20: a build is set at
//! creation and there is no trainer). The build editor here is the creation screen's
//! (BODIES.md 6.7) and the game master's (GM.md 4), which wears any build at once.

use gm_core::build::{AbilityDef, Build, BuildError, ContentPack, MAX_ACTIVES, Slot, Worn};
use gm_core::matrix::{Aspects, Attributes, Element};
use gm_core::tick::TickRate;
use gm_core::vocab::{ArchetypeFrame, Guard, Mode, MoveKind, Origin, Shape, Trigger, Verb};

use crate::showcase;
use crate::ui::{self, Canvas, Column, Key, ListEvent, Rect, RowMark, Ui};

/// `n` columns across `r` with `gap` between them.
pub fn columns(r: Rect, n: usize, gap: f32) -> Vec<Rect> {
    let w = (r.w - gap * (n as f32 - 1.0)) / n as f32;
    (0..n)
        .map(|i| Rect::new(r.x + i as f32 * (w + gap), r.y, w, r.h))
        .collect()
}

/// The editor's panel, in units: as wide as this where the frame allows (two columns:
/// the body on the left, the kit's lists on the right, every ability in sight), else
/// what the frame has less a margin.
pub const EDITOR_WIDE: f32 = 820.0;
/// The body's column, in units.
const BODY_WIDE: f32 = 296.0;
/// The presets' row has a label before the buttons from this width (units) up.
const PRESET_LABEL_AT: f32 = 700.0;
/// A kit column at least this wide (units) shows the actives in two lists side by
/// side; narrower, in one that scrolls.
const SPLIT_ACTIVES_AT: f32 = 420.0;
/// The attributes' rows: a line and a little, not a button's height (five of them),
/// and a little apart.
const ATTR_ROW_EXTRA: f32 = 2.0;
const ATTR_GAP: f32 = 2.0;

pub fn panel_width<C: Canvas>(ui: &Ui<'_, C>) -> f32 {
    (EDITOR_WIDE * ui.scale).min(ui.size().0 - 16.0 * ui.scale)
}

/// What the body's column takes under the presets: frame, armour, aspects, the
/// attributes' header and five rows.
fn body_height<C: Canvas>(ui: &Ui<'_, C>) -> f32 {
    let s = ui.scale;
    let gap = 5.0 * s;
    let (fh, line) = (ui.field_height(), ui.line());
    3.0 * (fh + gap) + (line + gap) + 5.0 * (line + ATTR_ROW_EXTRA * s) + 4.0 * ATTR_GAP * s
}

/// Rows the kit's tallest list would show with nothing hidden.
fn kit_rows(pack: &ContentPack, split: bool) -> usize {
    let count = |slot: Slot| {
        pack.abilities
            .iter()
            .filter(|d| d.slot == slot && !d.creature)
            .count()
    };
    let actives = count(Slot::Active);
    let actives = if split { actives.div_ceil(2) } else { actives };
    let middle = count(Slot::Secondary) + count(Slot::Guard) + 1;
    // The middle column stacks two lists with a label between: counted in rows.
    count(Slot::Primary).max(actives).max(middle + 2)
}

/// What the editor takes of a panel `w` wide when every list shows all its rows: the
/// presets' row, the taller of its two columns, and two lines that read an ability.
pub fn editor_height<C: Canvas>(ui: &Ui<'_, C>, pack: &ContentPack, w: f32) -> f32 {
    let s = ui.scale;
    let gap = 5.0 * s;
    let (h, line) = (ui.button_height(), ui.line());
    let split = w - BODY_WIDE * s - 2.0 * gap >= SPLIT_ACTIVES_AT * s;
    let kit = (line + gap) + ui.list_height(kit_rows(pack, split));
    let _ = h;
    (ui.field_height() + gap) + body_height(ui).max(kit) + gap + 2.0 * line
}

/// The build editor across `area`: a preset to start from over both columns; the body
/// on the left (frame, armour, aspects, the five attributes with what each buys); the
/// kit on the right (every list whole where the room allows); and under them a line
/// that reads the ability under the cursor. `b` is the draft; what the caller does
/// with it is the caller's business.
pub fn build_editor<C: Canvas>(
    ui: &mut Ui<'_, C>,
    area: Rect,
    pack: &ContentPack,
    rate: TickRate,
    b: &mut Build,
) {
    build_editor_wearing(ui, area, pack, rate, b, &Worn::NONE, true)
}

/// The creation screen's editor (BODIES.md 6.7): the body's attributes with the thirty
/// points to place, the element wheel and the mode; no lists of the hands (the gear's)
/// and no spells (the weapons').
pub fn creation_editor<C: Canvas>(
    ui: &mut Ui<'_, C>,
    area: Rect,
    pack: &ContentPack,
    rate: TickRate,
    b: &mut Build,
) {
    build_editor_wearing(ui, area, pack, rate, b, &Worn::NONE, false)
}

/// `build_editor` with what the hands bring (BODIES.md 3.8): a body's editor offers the
/// weapon's techniques where it holds more than one. Without `spells` it is the creation
/// screen's (BODIES.md 6.7): the element wheel in place of the kit.
pub fn build_editor_wearing<C: Canvas>(
    ui: &mut Ui<'_, C>,
    area: Rect,
    pack: &ContentPack,
    rate: TickRate,
    b: &mut Build,
    worn: &Worn,
    spells: bool,
) {
    let s = ui.scale;
    let gap = 5.0 * s;
    let (h, line) = (ui.button_height(), ui.line());
    let fh = ui.field_height();
    let mut col = Column::new(area, gap);
    if let Some(body) = b.body_def(pack) {
        // A body is for keeps (BODIES.md 3.5): the row says it, with its frame and its
        // skill, and offers the mode the character plays (MODES.md 2). The row is a
        // field's height: the mode's label over its three buttons.
        let row = col.take(fh);
        let skill = name_of(pack, body.skill);
        let said = format!(
            "{}  |  {}  |  body skill: {skill}",
            body.name,
            crate::front::frame_name(body.frame)
        );
        let label_w = (ui.text_width(&said) + 2.0 * gap).min(row.w * 0.6);
        ui.label(
            row.x,
            row.y + line + (h - line) * 0.5,
            label_w,
            ui::TEXT,
            &said,
        );
        const MODES: [Mode; 3] = [Mode::Action, Mode::Rpg, Mode::Fps];
        let mut mode = MODES.iter().position(|m| *m == b.mode).unwrap_or(0);
        let modes = Rect::new(row.x + label_w, row.y, row.w - label_w, row.h);
        if ui.choice(modes, "mode", &["action", "rpg", "fps"], &mut mode) {
            let (primary, secondary, guard) = pack.bare_hands(MODES[mode]);
            b.mode = MODES[mode];
            b.primary = primary;
            b.secondary = secondary;
            b.guard = guard;
        }
    } else {
        // A preset to start from: a row of them, the one the draft is lit, "custom" when
        // it is none; a word before them where the row is wide.
        let names: Vec<&str> = pack.builds.iter().map(|nb| nb.name.as_str()).collect();
        let preset = pack
            .builds
            .iter()
            .position(|nb| &nb.build == b)
            .unwrap_or(names.len());
        let mut options = names.clone();
        options.push("custom");
        let row = col.take(h);
        let buttons = if area.w >= PRESET_LABEL_AT * s {
            let label_w = ui.text_width("start from") + 2.0 * gap;
            ui.label(
                row.x,
                row.y + (row.h - line) * 0.5,
                label_w,
                ui::FAINT,
                "start from",
            );
            Rect::new(row.x + label_w, row.y, row.w - label_w, row.h)
        } else {
            row
        };
        for (i, r) in ui.buttons(buttons, &options).into_iter().enumerate() {
            if ui.button(r, options[i])
                && let Some(nb) = pack.builds.get(i)
            {
                *b = nb.build.clone();
            }
            if i == preset {
                ui.mark(r);
            }
        }
    }
    // The two columns, and under them what room is left (a line or two) to read an
    // ability in.
    let rest = col.rest();
    let kit_h = (line + gap)
        + ui.list_height(kit_rows(
            pack,
            rest.w - BODY_WIDE * s - 2.0 * gap >= SPLIT_ACTIVES_AT * s,
        ));
    let columns_h = body_height(ui).max(kit_h).min(rest.h - gap - line);
    let read_lines = (((rest.h - columns_h - gap) / line).floor() as usize).clamp(1, 2);
    let both = Rect::new(rest.x, rest.y, rest.w, columns_h);
    let read = Rect::new(
        rest.x,
        rest.y + rest.h - line * read_lines as f32,
        rest.w,
        line * read_lines as f32,
    );
    let body = Rect::new(both.x, both.y, BODY_WIDE * s, both.h);
    let kit = Rect::new(
        both.x + body.w + 2.0 * gap,
        both.y,
        both.w - body.w - 2.0 * gap,
        both.h,
    );
    body_column(ui, body, pack, b, !spells);
    let (over, why) = if spells {
        kit_columns(ui, kit, pack, b, worn)
    } else {
        creation_column(ui, kit, pack, b);
        (None, None)
    };
    // What the ability under the cursor does, in the lines there are (the last cut if
    // it must be), after why it cannot be taken where it cannot; else what the lines
    // are for.
    match over.and_then(|i| pack.abilities.get(i as usize)) {
        Some(def) => {
            let words = words_of(def, rate);
            let text = match &why {
                // `words_of` opens with the name: the reason goes between.
                Some(why) => {
                    let head = format!("{}: ", def.ability.name);
                    let rest = words.strip_prefix(&head).unwrap_or(&words);
                    format!("{head}{why}. {rest}")
                }
                None => words,
            };
            let lines = ui::wrap_measured(&text, read.w, |t| ui.text_width(t));
            for (n, l) in lines.iter().take(read_lines).enumerate() {
                let shown = if n + 1 == read_lines && lines.len() > read_lines {
                    ui.fit_text(read.w, &format!("{l} ..."))
                } else {
                    l.clone()
                };
                let ink = if why.is_some() { ui::WARN } else { ui::TEXT };
                ui.label(read.x, read.y + line * n as f32, 0.0, ink, &shown);
            }
        }
        None => ui.label(
            read.x,
            read.y,
            read.w,
            ui::FAINT,
            if spells {
                "point at an ability to read what it does"
            } else {
                "point at an element to read what it does, what it allows and what it is weak to"
            },
        ),
    }
}

/// The frame and the armour, the aspects, and the attributes: a name, `-`, the value,
/// `+`, and what the points buy, a row each (MATRIX.md 6).
fn body_column<C: Canvas>(
    ui: &mut Ui<'_, C>,
    area: Rect,
    pack: &ContentPack,
    b: &mut Build,
    creation: bool,
) {
    let s = ui.scale;
    let gap = 5.0 * s;
    let (fh, line) = (ui.field_height(), ui.line());
    let mut col = Column::new(area, gap);
    const FRAMES: [ArchetypeFrame; 4] = [
        ArchetypeFrame::Colossus,
        ArchetypeFrame::Striker,
        ArchetypeFrame::Caster,
        ArchetypeFrame::Infiltrator,
    ];
    let floors = b.floors(pack).as_array();
    if b.body.is_some() {
        // The frame is the body's (BODIES.md 3.2): said, not chosen.
        let row = col.take(fh);
        ui.label(
            row.x,
            row.y + (row.h - line) * 0.5,
            row.w,
            ui::FAINT,
            &format!("frame  {}  (the body's)", crate::front::frame_name(b.frame)),
        );
    } else {
        let mut frame = FRAMES.iter().position(|f| *f == b.frame).unwrap_or(0);
        if ui.choice(
            col.take(fh),
            "frame",
            &["colossus", "striker", "caster", "infiltrator"],
            &mut frame,
        ) {
            b.frame = FRAMES[frame];
        }
    }
    // Aspects: one or two of six (the second costs 10); on the creation screen they
    // are the wheel's (`creation_column`).
    if !creation {
        aspect_boxes(ui, col.take(fh), b);
    }
    // The attributes (up to 25, thirty points to spend above the floors: the body's,
    // or five each) under their budget.
    let spent = b.points_spent(pack);
    let head = col.take(line);
    // (A draft from elsewhere may be over the points: said so, in the warning's colour.)
    let (budget, ink) = match Attributes::FREE_POINTS.checked_sub(spent) {
        Some(left) => (
            format!(
                "attributes   {left} of {} points to place",
                Attributes::FREE_POINTS
            ),
            ui::FOCUS,
        ),
        None => (
            format!("attributes   {spent} of {} points", Attributes::FREE_POINTS),
            ui::WARN,
        ),
    };
    ui.label(head.x, head.y, head.w, ink, &budget);
    const NAMES: [&str; 5] = ["STR", "AGI", "CON", "INT", "SPR"];
    let mut attrs = b.attributes.as_array();
    let d = b.derived_in(pack);
    let buys = [
        format!("blows x{:.2}", d.physical_mult),
        format!("speed {:.0} u/s", d.max_speed),
        format!("health {}", d.health),
        format!("spells x{:.2}", d.elemental_mult),
        format!("ward {:.0}%", d.ward * 100.0),
    ];
    // A row a line and a little, less the little where the column is short (the game
    // master's page has a row of tabs over the editor). On the creation screen a row
    // of two buttons under them: a suggested spread, and back to the floors.
    let buttons_h = if creation { fh + gap } else { 0.0 };
    let short = col.rest().h - buttons_h - 4.0 * ATTR_GAP * s;
    let row_h = (line + ATTR_ROW_EXTRA * s).min(short / 5.0).max(line);
    let (name_w, button_w, value_w) = (34.0 * s, 20.0 * s, 26.0 * s);
    let rows = col.take(5.0 * row_h + 4.0 * ATTR_GAP * s);
    for i in 0..5 {
        let row = Rect::new(
            rows.x,
            rows.y + i as f32 * (row_h + ATTR_GAP * s),
            rows.w,
            row_h,
        );
        let text_y = row.y + (row.h - line) * 0.5;
        ui.label(row.x, text_y, name_w, ui::TEXT, NAMES[i]);
        let minus = Rect::new(row.x + name_w, row.y, button_w, row.h);
        let plus = Rect::new(minus.x + button_w + value_w, row.y, button_w, row.h);
        if ui.button_or(
            minus,
            "-",
            (attrs[i] <= floors[i]).then_some("the body's floor"),
        ) {
            attrs[i] -= 1;
        }
        let no_plus = if attrs[i] >= Attributes::MAX {
            Some("25 is the most")
        } else if spent >= Attributes::FREE_POINTS {
            Some("every point is placed: take one off another first")
        } else {
            None
        };
        if ui.button_or(plus, "+", no_plus) {
            attrs[i] += 1;
        }
        let value = attrs[i].to_string();
        let vw = ui.text_width(&value);
        ui.label(
            minus.x + button_w + (value_w - vw) * 0.5,
            text_y,
            value_w,
            ui::TEXT,
            &value,
        );
        let words_x = plus.x + button_w + 2.0 * gap;
        // What the points buy; with a body, the floor first where the row has room.
        let words = if b.body.is_some() && row.w >= 260.0 * s {
            format!("floor {}   {}", floors[i], buys[i])
        } else {
            buys[i].clone()
        };
        ui.label(words_x, text_y, row.x + row.w - words_x, ui::FAINT, &words);
    }
    if creation {
        let row = col.take(fh);
        let under = Rect::new(row.x, row.y + line, row.w, row.h - line);
        ui.label(row.x, row.y, row.w, ui::FAINT, "or take");
        let buttons = ui.buttons(under, &["Suggested spread", "Back to the floors"]);
        if ui.button(buttons[0], "Suggested spread") {
            attrs = suggested_spread(floors);
        }
        if ui.button_if(buttons[1], "Back to the floors", spent > 0) {
            attrs = floors;
        }
    }
    let [str_, agi, con, int, spr] = attrs;
    b.attributes = Attributes {
        str_,
        agi,
        con,
        int,
        spr,
    };
}

/// Thirty points laid along the body's floors (BODIES.md 6.7): each attribute gets its
/// share of the floors' sum, the remainder to the highest floors, nothing past the cap.
fn suggested_spread(floors: [u8; 5]) -> [u8; 5] {
    let total: u32 = floors.iter().map(|&f| f as u32).sum::<u32>().max(1);
    let mut out = floors;
    let mut given = 0;
    for (o, &f) in out.iter_mut().zip(&floors) {
        let add = ((Attributes::FREE_POINTS * f as u32 / total) as u8).min(Attributes::MAX - f);
        *o = f + add;
        given += add as u32;
    }
    let mut order: Vec<usize> = (0..5).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(floors[i]));
    let mut left = Attributes::FREE_POINTS.saturating_sub(given);
    while left > 0 {
        let before = left;
        for &i in &order {
            if left > 0 && out[i] < Attributes::MAX {
                out[i] += 1;
                left -= 1;
            }
        }
        if left == before {
            break;
        }
    }
    out
}

/// The six aspects as check boxes in a row (the K page and the game master's): one or
/// two of six, the second at a cost.
fn aspect_boxes<C: Canvas>(ui: &mut Ui<'_, C>, row: Rect, b: &mut Build) {
    let s = ui.scale;
    let line = ui.line();
    ui.label(
        row.x,
        row.y,
        row.w,
        ui::FAINT,
        "aspects  (none is neutral; the second costs 10)",
    );
    // Each box as wide as its word needs, the spare shared between them.
    let boxes = Rect::new(row.x, row.y + line, row.w, row.h - line);
    let names: Vec<&str> = Element::ALL.iter().map(|e| e.name()).collect();
    let side = ui.ascent() + 4.0 * s;
    let needs: Vec<f32> = names
        .iter()
        .map(|n| side + 6.0 * s + ui.text_width(n))
        .collect();
    let spare = ((boxes.w - needs.iter().sum::<f32>()) / (names.len() - 1) as f32).max(0.0);
    let mut x = boxes.x;
    let cells: Vec<Rect> = needs
        .iter()
        .map(|w| {
            let r = Rect::new(x.round(), boxes.y, w.ceil(), boxes.h);
            x += w + spare;
            r
        })
        .collect();
    for (e, r) in Element::ALL.iter().zip(cells) {
        let mut on = b.aspects.contains(*e);
        if ui.checkbox(r, names[*e as usize], &mut on) {
            let bit = Aspects::one(*e).0;
            b.aspects = Aspects(if on {
                b.aspects.0 | bit
            } else {
                b.aspects.0 & !bit
            });
        }
    }
}

/// The creation screen's right column (BODIES.md 6.7): the element wheel, and beside
/// it what the hands bring and where the spells are.
fn creation_column<C: Canvas>(ui: &mut Ui<'_, C>, area: Rect, pack: &ContentPack, b: &mut Build) {
    let s = ui.scale;
    let gap = 6.0 * s;
    let line = ui.line();
    // The wheel takes a square as tall as the column, on the left, a little under the
    // mode's row; the words the rest.
    let side = (area.h - gap).min(area.w * 0.62).max(0.0);
    let wheel = Rect::new(area.x, area.y + gap, side.round(), side.round());
    element_wheel(ui, wheel, pack, b);
    let words = Rect::new(
        wheel.x + wheel.w + 2.0 * gap,
        area.y,
        (area.w - wheel.w - 2.0 * gap).max(0.0),
        area.h,
    );
    ui.label(words.x, words.y, words.w, ui::FAINT, "hands and body");
    let hands = match b.guard {
        Some(g) => format!(
            "{}, {} and {}",
            name_of(pack, b.primary),
            name_of(pack, b.secondary),
            name_of(pack, g)
        ),
        None => format!(
            "{} and {}",
            name_of(pack, b.primary),
            name_of(pack, b.secondary)
        ),
    };
    let skill = b
        .body_skill(pack)
        .map(|i| name_of(pack, i))
        .unwrap_or_default();
    let text = format!(
        "Empty-handed: {hands}. A weapon in the right hand brings its blow and its second \
         move; a shield or a focus in the left brings the guard.\n\nBody skill: {skill}, \
         free, always on the first key.\n\nSpells come with the weapons that carry them: \
         the element picked here is what a staff casts, and what you resist."
    );
    let r = Rect::new(
        words.x,
        words.y + line + gap,
        words.w,
        (words.h - line - gap).max(0.0),
    );
    ui.paragraph(r, ui::TEXT, &text);
}

/// The six elements on a wheel (BODIES.md 6.7): a picture each, the one or two picked
/// lit, what the picks come to in the middle; the pointer on one reads what it does,
/// what it allows and what it is strong and weak against. A click takes an element or
/// lets it go; a third is refused in words. None is neutral.
fn element_wheel<C: Canvas>(ui: &mut Ui<'_, C>, area: Rect, pack: &ContentPack, b: &mut Build) {
    let s = ui.scale;
    let line = ui.line();
    // A cell is a picture and its word; the ring's radius leaves the cells inside the
    // square with a little air.
    let cell = (area.w.min(area.h) * 0.23).max(24.0 * s).round();
    let cell_h = cell + line;
    let radius = ((area.w.min(area.h) - cell_h) * 0.5 - 2.0 * s).max(0.0);
    let (cx, cy) = (
        area.x + area.w * 0.5,
        area.y + (area.h - line) * 0.5 + line * 0.5,
    );
    let picked = b.aspects;
    let full = picked.count() >= 2;
    for (i, e) in Element::ALL.iter().enumerate() {
        let angle = -std::f32::consts::FRAC_PI_2 + i as f32 * std::f32::consts::FRAC_PI_3;
        let x = (cx + radius * angle.cos() - cell * 0.5).round();
        let y = (cy + radius * angle.sin() - cell_h * 0.5).round();
        let r = Rect::new(x, y, cell, cell_h);
        let on = picked.contains(*e);
        let off = (!on && full).then_some("two at most: let one go first");
        let name = showcase::capitalised(e.name());
        let icon = format!("elem_{}", e.name());
        let tip = || element_words(pack, *e);
        if ui.picture_box(r, &icon, &name, on, off, &tip) {
            let bit = Aspects::one(*e).0;
            b.aspects = Aspects(if on {
                b.aspects.0 & !bit
            } else {
                b.aspects.0 | bit
            });
        }
    }
    // The middle: what is picked, and what it costs.
    let mut names: Vec<String> = b
        .aspects
        .iter()
        .map(|e| showcase::capitalised(e.name()))
        .collect();
    // (Short words: the middle is a cell and a half wide.)
    let (head, under, ink) = match names.len() {
        0 => ("neutral".to_string(), "none picked", ui::FAINT),
        1 => (names.remove(0), "free", ui::FOCUS),
        _ => (names.join(" + "), "+10 points", ui::FOCUS),
    };
    let room = (2.0 * radius - cell).max(cell);
    let head = ui.fit_text(room, &head);
    let hw = ui.text_width(&head);
    ui.label((cx - hw * 0.5).round(), cy - line, hw + s, ink, &head);
    let under = ui.fit_text(room, under);
    let uw = ui.text_width(&under);
    ui.label(
        (cx - uw * 0.5).round(),
        cy + 2.0 * s,
        uw + s,
        ui::FAINT,
        &under,
    );
}

/// What an element is to a player (MATRIX.md 5), for the pointer on its picture: a
/// line of what it does, what the pack lets it cast, and the matrix as strengths and
/// weaknesses, attacking and defending.
fn element_words(pack: &ContentPack, e: Element) -> ui::TipLines {
    use gm_core::matrix::{BEATS, RESISTED, element_mult};
    let name = showcase::capitalised(e.name());
    let flavour = match e {
        Element::Fire => "burning bolts and bursts: damage that keeps burning",
        Element::Water => "frost: bolts that slow, a nova that freezes",
        Element::Grass => "poison and thorns: briars that hold, clouds that sicken",
        Element::Electric => "sparks and thunder: quick bolts, a clap that staggers",
        Element::Ground => "stone and weight: a thrown rock, a stomp, a fortified stand",
        Element::Air => "shadow and wind: darts, a gust, a blink, a vanishing",
    };
    let list = |which: &dyn Fn(Element) -> bool| -> String {
        let names: Vec<String> = Element::ALL
            .iter()
            .filter(|o| which(**o))
            .map(|o| o.name().to_string())
            .collect();
        if names.is_empty() {
            "nothing".to_string()
        } else {
            names.join(", ")
        }
    };
    let mut spells: Vec<String> = pack
        .abilities
        .iter()
        .filter(|d| d.aspect == Some(e) && !d.creature && d.slot != Slot::Extra)
        .map(|d| d.ability.name.clone())
        .collect();
    spells.dedup();
    let spells = if spells.is_empty() {
        "nothing in this pack".to_string()
    } else {
        spells.join(", ")
    };
    vec![
        (name, ui::FOCUS),
        (flavour.to_string(), ui::TEXT),
        (format!("casts: {spells}"), ui::TEXT),
        (
            format!(
                "your {} spells hit harder on {}",
                e.name(),
                list(&|o| element_mult(e, o) >= BEATS)
            ),
            ui::TEXT,
        ),
        (
            format!("weaker on {}", list(&|o| element_mult(e, o) <= RESISTED)),
            ui::FAINT,
        ),
        (
            format!("you resist {}", list(&|o| element_mult(o, e) <= RESISTED)),
            ui::TEXT,
        ),
        (
            format!(
                "you take more from {}",
                list(&|o| element_mult(o, e) >= BEATS)
            ),
            ui::WARN,
        ),
    ]
}

/// The name of an ability, for what is said about a draft.
fn name_of(pack: &ContentPack, i: u16) -> String {
    pack.abilities
        .get(i as usize)
        .map_or_else(|| format!("ability {i}"), |a| a.ability.name.clone())
}

/// Why ability `i` cannot go into `slot` of the draft `b` as it stands (CLIENT.md 4.6):
/// the mode's rule on firearms and guards, the aspect it needs, an ability already in
/// the kit that it would double or share a cooldown with, the actives' count, the
/// budget. `None` where it can, or already is there. The rules are `Build::validate`'s
/// (MATRIX.md 9), asked one ability at a time.
pub fn why_not(pack: &ContentPack, b: &Build, slot: Slot, i: u16) -> Option<String> {
    let def = pack.abilities.get(i as usize)?;
    let there = match slot {
        Slot::Primary => b.primary == i,
        Slot::Secondary => b.secondary == i,
        Slot::Guard => b.guard == Some(i),
        Slot::Active => b.actives.contains(&i),
        Slot::Extra => false,
    };
    if there {
        return None;
    }
    if slot == Slot::Active && b.body_skill(pack) == Some(i) {
        return Some("the body's own: it is in the kit already".into());
    }
    let firearm = def.ability.firearm.is_some();
    match (b.mode, slot) {
        (Mode::Fps, Slot::Primary) if !firearm => {
            return Some("an FPS build's weapon is a firearm".into());
        }
        (Mode::Fps, Slot::Guard) => return Some("an FPS build has no guard".into()),
        (Mode::Action | Mode::Rpg, _) if firearm => {
            return Some("a firearm: only an FPS build holds one".into());
        }
        _ => {}
    }
    if let Some(needs) = def.aspect
        && !b.aspects.contains(needs)
    {
        return Some(format!("needs the {} aspect", needs.name()));
    }
    // The rest of the kit: everything slotted but what this one would replace.
    let others: Vec<u16> = b
        .kit_slots_in(&gm_core::build::Worn::NONE, pack)
        .into_iter()
        .filter(|(_, in_slot)| slot == Slot::Active || *in_slot != slot)
        .map(|(j, _)| j)
        .collect();
    if others.contains(&i) {
        return Some("already in the kit".into());
    }
    if let Some(g) = def.ability.cooldown.group
        && let Some(other) = others.iter().find(|j| {
            pack.abilities
                .get(**j as usize)
                .and_then(|a| a.ability.cooldown.group)
                == Some(g)
        })
    {
        return Some(format!("shares a cooldown with {}", name_of(pack, *other)));
    }
    let most = if b.body.is_some() {
        MAX_ACTIVES - 1
    } else {
        MAX_ACTIVES
    };
    if slot == Slot::Active && b.actives.len() >= most {
        return Some(format!("{most} spells already: take one out"));
    }
    None
}

/// The pack's refusal of a draft, with the abilities by name.
fn said(pack: &ContentPack, e: &BuildError) -> String {
    match e {
        BuildError::UnknownAbility(i) => format!("the pack has no ability {i}"),
        BuildError::UnknownBody(i) => format!("the pack has no body {i}"),
        BuildError::BodyFrame => "the frame is the body's".to_string(),
        BuildError::AttributeUnderFloor => "an attribute is under its body's floor".to_string(),
        BuildError::WrongSlot { index, expected } => {
            let slot = match expected {
                Slot::Primary => "weapon",
                Slot::Secondary => "secondary",
                Slot::Guard => "guard",
                Slot::Active => "active",
                Slot::Extra => "extra",
            };
            format!("{} does not fit the {slot} slot", name_of(pack, *index))
        }
        BuildError::DuplicateAbility(i) => format!("{} is in the kit twice", name_of(pack, *i)),
        BuildError::MissingAspect { index, needs } => {
            format!(
                "{} needs the {} aspect",
                name_of(pack, *index),
                needs.name()
            )
        }
        BuildError::CreatureOnly(i) => format!("{} is a creature's", name_of(pack, *i)),
        BuildError::NotSlottable(i) => {
            format!("{} comes with another ability", name_of(pack, *i))
        }
        BuildError::FirearmNeedsGun(i) => {
            format!(
                "{} is a firearm: only a gun build holds one",
                name_of(pack, *i)
            )
        }
        BuildError::SharedCooldownGroup(g) => {
            // The two that share it, by name.
            let shared: Vec<String> = pack
                .abilities
                .iter()
                .filter(|a| a.ability.cooldown.group == Some(*g))
                .map(|a| a.ability.name.clone())
                .collect();
            format!("one cooldown shared: {}", shared.join(", "))
        }
        other => other.to_string(),
    }
}

/// The kit's lists side by side under its budget: the weapons; the secondaries over
/// the guards (or none); the actives, in two lists where there is room. A click puts
/// a row in or takes it out; a row that cannot go in as the draft stands is off
/// (`why_not`). Returns the ability under the cursor and, if it is off, why.
fn kit_columns<C: Canvas>(
    ui: &mut Ui<'_, C>,
    area: Rect,
    pack: &ContentPack,
    b: &mut Build,
    worn: &Worn,
) -> (Option<u16>, Option<String>) {
    let s = ui.scale;
    let gap = 5.0 * s;
    let line = ui.line();
    let split = area.w >= SPLIT_ACTIVES_AT * s;
    let of = |slot: Slot| -> Vec<(u16, String, String)> {
        pack.abilities
            .iter()
            .enumerate()
            .filter(|(_, d)| d.slot == slot && !d.creature)
            .map(|(i, d)| (i as u16, d.ability.name.clone(), String::new()))
            .collect()
    };
    let (primaries, secondaries, guards, actives) = (
        of(Slot::Primary),
        of(Slot::Secondary),
        of(Slot::Guard),
        of(Slot::Active),
    );
    // With a body (BODIES.md 3.4) the hands are the gear's: one column says so and
    // names the body skill, the actives take the rest; without one, the weapon, the
    // secondary and the guard are lists of their own.
    let bodied = b.body.is_some();
    let first_active = if bodied { 1 } else { 2 };
    let most = if bodied { MAX_ACTIVES - 1 } else { MAX_ACTIVES };
    let cols = columns(area, first_active + if split { 2 } else { 1 }, gap);
    // The labels, and the budget over the actives.
    let head_y = area.y;
    if bodied {
        ui.label(cols[0].x, head_y, cols[0].w, ui::FAINT, "hands and body");
    } else {
        ui.label(cols[0].x, head_y, cols[0].w, ui::FAINT, "weapon");
        ui.label(cols[1].x, head_y, cols[1].w, ui::FAINT, "secondary");
    }
    // Over the actives, which are the kit's choice (nothing is priced, MATRIX.md 20).
    let what = if bodied { "spells" } else { "actives" };
    let budget = format!("{what}, up to {most}");
    let budget_w = area.x + area.w - cols[first_active].x;
    ui.label(cols[first_active].x, head_y, budget_w, ui::FOCUS, &budget);
    // The lists: as many rows as the column has room for, the column's own count at most.
    let lists_y = area.y + line + gap;
    let room = area.y + area.h - lists_y;
    let row_h = line + 2.0 * s;
    let fits = ((room - 2.0 * s) / row_h).floor().max(1.0) as usize;
    let row_height = |rows: usize| rows as f32 * row_h + 2.0 * s;
    let list_rect =
        move |c: Rect, rows: usize| Rect::new(c.x, lists_y, c.w, row_height(rows.min(fits).max(1)));
    // The name; the second column is empty since nothing is priced (MATRIX.md 20).
    const COLS: [f32; 2] = [0.0, 0.86];
    let rows_of = |list: &[(u16, String, String)], none: bool| -> Vec<Vec<String>> {
        let mut rows: Vec<Vec<String>> = list
            .iter()
            .map(|(_, n, c)| vec![n.clone(), c.clone()])
            .collect();
        if none {
            rows.push(vec!["none".to_string(), String::new()]);
        }
        rows
    };
    let mut over: Option<u16> = None;
    let mut why: Option<String> = None;
    // A list of one slot: the rows that cannot be taken are off, and the cursor over
    // one reads why.
    let mut pick = |ui: &mut Ui<'_, C>,
                    r: Rect,
                    name: &str,
                    slot: Slot,
                    list: &[(u16, String, String)],
                    current: Option<u16>,
                    none: bool,
                    b: &Build|
     -> Option<Option<u16>> {
        let rows = rows_of(list, none);
        let reasons: Vec<Option<String>> = list
            .iter()
            .map(|(i, ..)| why_not(pack, b, slot, *i))
            .collect();
        let marks: Vec<RowMark> = reasons
            .iter()
            .map(|r| {
                if r.is_some() {
                    RowMark::Off
                } else {
                    RowMark::Plain
                }
            })
            .collect();
        let mut at = match current {
            Some(c) => list.iter().position(|(i, ..)| *i == c).unwrap_or(ui::NONE),
            None if none => rows.len() - 1,
            None => ui::NONE,
        };
        let event = ui.list_marked(r, name, &COLS, &rows, &marks, &mut at);
        if let Some(i) = ui.row_over(name) {
            over = list.get(i).map(|(i, ..)| *i);
            why = reasons.get(i).cloned().flatten();
        }
        match event {
            ListEvent::Picked | ListEvent::Activated => Some(list.get(at).map(|(i, ..)| *i)),
            ListEvent::None => None,
        }
    };
    if let Some(skill) = b.body_skill(pack) {
        // What the hands bring (GEAR.md 2) and what the body does, in words; under them
        // the weapon's techniques (BODIES.md 3.8) where it has more than one.
        let text = format!(
            "The weapon in your right hand is your blow and your second move; the thing in \
             your left is your guard. Empty-handed: {}, {}{}.\n\nBody skill: {}, always \
             the first active, at no cost.",
            name_of(pack, b.primary),
            name_of(pack, b.secondary),
            b.guard
                .map(|g| format!(" and {}", name_of(pack, g)))
                .unwrap_or_default(),
            name_of(pack, skill)
        );
        let r = Rect::new(cols[0].x, lists_y, cols[0].w, room);
        let used = ui.paragraph(r, ui::TEXT, &text);
        if worn.techniques.len() > 1 {
            let label_y = lists_y + used + gap;
            ui.label(cols[0].x, label_y, cols[0].w, ui::FAINT, "technique");
            let techniques: Vec<(u16, String, String)> = worn
                .techniques
                .iter()
                .map(|&i| (i, name_of(pack, i), String::new()))
                .collect();
            let t_room = area.y + area.h - (label_y + line + gap);
            let t_fits = ((t_room - 2.0 * s) / row_h).floor().max(1.0) as usize;
            let list = Rect::new(
                cols[0].x,
                label_y + line + gap,
                cols[0].w,
                row_height(techniques.len().min(t_fits)),
            );
            let current = worn.primary.filter(|p| worn.techniques.contains(p));
            if let Some(Some(i)) = pick(
                ui,
                list,
                "technique",
                Slot::Primary,
                &techniques,
                current.or(Some(b.primary)),
                false,
                b,
            ) {
                b.primary = i;
            }
        }
    } else {
        if let Some(Some(i)) = pick(
            ui,
            list_rect(cols[0], primaries.len()),
            "primary",
            Slot::Primary,
            &primaries,
            Some(b.primary),
            false,
            b,
        ) {
            b.primary = i;
        }
        // The middle column: the secondaries, then the guards under their own label.
        let sec = list_rect(cols[1], secondaries.len());
        if let Some(Some(i)) = pick(
            ui,
            sec,
            "secondary",
            Slot::Secondary,
            &secondaries,
            Some(b.secondary),
            false,
            b,
        ) {
            b.secondary = i;
        }
        let guard_label_y = sec.y + sec.h + gap;
        ui.label(cols[1].x, guard_label_y, cols[1].w, ui::FAINT, "guard");
        let guard_rows = guards.len() + 1;
        let guard_room = area.y + area.h - (guard_label_y + line + gap);
        let guard_fits = ((guard_room - 2.0 * s) / row_h).floor().max(1.0) as usize;
        let guard = Rect::new(
            cols[1].x,
            guard_label_y + line + gap,
            cols[1].w,
            row_height(guard_rows.min(guard_fits)),
        );
        if let Some(g) = pick(ui, guard, "guard", Slot::Guard, &guards, b.guard, true, b) {
            b.guard = g;
        }
    }
    // Actives: the ones in the build are lit; a click toggles the row and the list goes
    // back to nothing selected, so the next click on the same row toggles it again.
    let halves: Vec<&[(u16, String, String)]> = if split {
        let (a, z) = actives.split_at(actives.len().div_ceil(2));
        vec![a, z]
    } else {
        vec![&actives[..]]
    };
    for (n, part) in halves.iter().enumerate() {
        let rows: Vec<Vec<String>> = part
            .iter()
            .map(|(_, name, cost)| vec![name.clone(), cost.clone()])
            .collect();
        let reasons: Vec<Option<String>> = part
            .iter()
            .map(|(i, ..)| why_not(pack, b, Slot::Active, *i))
            .collect();
        let marks: Vec<RowMark> = part
            .iter()
            .zip(&reasons)
            .map(|((i, ..), why)| {
                if b.actives.contains(i) {
                    RowMark::Picked
                } else if why.is_some() {
                    RowMark::Off
                } else {
                    RowMark::Plain
                }
            })
            .collect();
        let name = if n == 0 { "actives" } else { "actives b" };
        let mut at = ui::NONE;
        let r = list_rect(cols[first_active + n], part.len());
        if ui.list_marked(r, name, &COLS, &rows, &marks, &mut at) != ListEvent::None
            && let Some((i, ..)) = part.get(at)
        {
            if b.actives.contains(i) {
                b.actives.retain(|a| a != i);
            } else if b.actives.len() < most && b.body_skill(pack) != Some(*i) {
                b.actives.push(*i);
            }
        }
        if let Some(i) = ui.row_over(name) {
            over = part.get(i).map(|(i, ..)| *i);
            why = reasons.get(i).cloned().flatten();
        }
    }
    (over, why)
}

/// Seconds from ticks, short: `14 s`, `0.6 s`.
fn secs(rate: TickRate, ticks: u32) -> String {
    let ms = rate.ticks_to_ms(ticks);
    if ms.is_multiple_of(1000) {
        format!("{} s", ms / 1000)
    } else {
        format!("{:.1} s", ms as f32 / 1000.0)
    }
}

/// An ability in a line a player can read: its slot, what it costs to use, when it is
/// back, and what it does, from its script (VOCABULARY.md 5). Nothing here decides
/// anything; the numbers are the pack's.
pub fn words_of(def: &AbilityDef, rate: TickRate) -> String {
    let a = &def.ability;
    let slot = match def.slot {
        Slot::Primary => "a blow",
        Slot::Secondary => "a second move",
        Slot::Guard => "a guard",
        Slot::Active => "an active",
        Slot::Extra => "a stage",
    };
    let mut parts = vec![format!("{}: {slot}", a.name)];
    if a.cost.stamina > 0 {
        parts.push(format!("{} stamina", a.cost.stamina));
    }
    if a.cost.focus > 0 {
        parts.push(format!("{} focus", a.cost.focus));
    }
    if a.cooldown.ticks > 0 {
        parts.push(format!("{} cooldown", secs(rate, a.cooldown.ticks)));
    }
    if let Some(e) = def.aspect {
        parts.push(format!("needs {}", e.name()));
    }
    let damage = |d: &gm_core::vocab::DamagePacket| -> String {
        let mut w = format!("{} {}", d.amount, d.dtype.name());
        if d.stagger > 0 {
            w.push_str(", staggers");
        }
        w
    };
    let status = |st: &gm_core::vocab::ApplyStatus| -> String {
        format!("{} for {}", st.status.name(), secs(rate, st.duration))
    };
    let shape = |sh: &Shape| -> String {
        match sh {
            Shape::Sphere { radius } | Shape::Cylinder { radius, .. } => {
                format!("{radius:.0} u around")
            }
            Shape::Cone { length, .. } => format!("a cone of {length:.0} u"),
            Shape::Box { half_extents } => format!("{:.0} u across", half_extents[0] * 2.0),
        }
    };
    let origin = |o: &Origin| -> &'static str {
        match o {
            Origin::Aim { .. } => " where you aim",
            Origin::Impact => " where it lands",
            _ => " you",
        }
    };
    let mut does: Vec<String> = Vec::new();
    for step in &a.steps {
        match &step.verb {
            Verb::MeleeArc(m) => {
                let mut w = format!("a swing of {:.0} u: {}", m.reach, damage(&m.damage));
                if m.max_targets > 1 {
                    w.push_str(&format!(", up to {}", m.max_targets));
                }
                does.push(w);
            }
            Verb::Projectile(p) => {
                let mut w = if p.damage.amount > 0 {
                    format!("a bolt: {}", damage(&p.damage))
                } else {
                    "a dart".to_string()
                };
                for t in &p.on_hit {
                    match t {
                        Trigger::Status(st) => {
                            w.push_str(&format!(", {} on whom it hits", status(st)))
                        }
                        Trigger::Area(ar) => {
                            w.push_str(&format!(", then {}", shape(&ar.shape)));
                            if let Some(d) = &ar.damage {
                                w.push_str(&format!(": {}", damage(d)));
                            }
                            for st in &ar.effects {
                                w.push_str(&format!(", {}", status(st)));
                            }
                        }
                    }
                }
                does.push(w);
            }
            Verb::AreaEffect(ar) => {
                let mut w = format!("{}{}", shape(&ar.shape), origin(&ar.origin));
                if let Some(d) = &ar.damage {
                    w.push_str(&format!(": {}", damage(d)));
                }
                for st in &ar.effects {
                    w.push_str(&format!(", {}", status(st)));
                }
                if ar.duration > 0 {
                    w.push_str(&format!(", for {}", secs(rate, ar.duration)));
                }
                does.push(w);
            }
            Verb::ApplyStatus(st) => does.push(format!("{} on yourself", status(st))),
            Verb::MoveSelf(m) => does.push(match m.kind {
                MoveKind::Dash { speed, duration } => {
                    format!("a dash at {speed:.0} u/s for {}", secs(rate, duration))
                }
                MoveKind::Leap { .. } => "a leap".to_string(),
                MoveKind::Charge {
                    speed, duration, ..
                } => {
                    format!("a charge at {speed:.0} u/s for {}", secs(rate, duration))
                }
                MoveKind::Blink { distance } => format!("a blink of {distance:.0} u"),
            }),
            Verb::Guard(Guard::Block(bl)) => does.push(format!(
                "a block: {:.0}% less from the front, {} stamina a hit",
                bl.mitigation * 100.0,
                bl.stamina_per_hit
            )),
            Verb::Guard(Guard::Parry(pa)) => does.push(format!(
                "a parry: a window of {}, and the attacker pays for it",
                secs(rate, pa.window)
            )),
        }
    }
    let mut text = parts.join(", ");
    if !does.is_empty() {
        text.push_str(". ");
        text.push_str(&does.join("; "));
    }
    if a.move_scale == 0.0 {
        text.push_str("; you stand still for it");
    } else if a.move_scale < 0.5 {
        text.push_str(&format!("; you move at {:.0}%", a.move_scale * 100.0));
    }
    text
}

/// The pack's word on a draft: the points, the kit's cost, and what is wrong if anything.
pub fn verdict(pack: &ContentPack, b: &Build) -> (bool, String) {
    let points = b.points_spent(pack);
    match b.validate(pack) {
        Ok(()) => (
            true,
            format!("{points} of {} points", Attributes::FREE_POINTS),
        ),
        Err(e) => (false, said(pack, &e)),
    }
}

/// What the page asked for in a frame.
#[derive(Clone, Debug, PartialEq)]
pub enum CharacterAction {
    None,
    Close,
}

/// What the page is shown each frame.
pub struct CharacterView<'a> {
    pub pack: &'a ContentPack,
    /// The build worn now.
    pub own: &'a Build,
    /// What the hands bring now (GEAR.md 2).
    pub worn: &'a Worn,
    /// The zone's tick rate: the abilities' ticks read in seconds.
    pub rate: TickRate,
}

/// The character's page: the build as it is (MATRIX.md 20: set at creation, never
/// edited in play), the sheet it comes to, and what the hands bring.
#[derive(Default)]
pub struct CharacterPage;

impl CharacterPage {
    /// The name a UI script waits for (CLIENT.md 9).
    pub fn name(&self) -> &'static str {
        "character"
    }

    pub fn frame<C: Canvas>(
        &mut self,
        ui: &mut Ui<'_, C>,
        v: &CharacterView<'_>,
    ) -> CharacterAction {
        if ui.key(Key::Escape) {
            return CharacterAction::Close;
        }
        let s = ui.scale;
        let gap = 5.0 * s;
        let h = ui.button_height();
        let line = ui.line();
        let w = panel_width(ui);
        let want = sheet_height(ui) + gap + h;
        let most = ui::PANEL_HIGH * s - ui.panel_height(0.0, true);
        let inner = want.min(most);
        let panel = Rect::centred(ui.size(), w, ui.panel_height(inner, true));
        let inner = ui.panel(panel, "character");
        let rest = Rect::new(inner.x, inner.y, inner.w, inner.h - gap - h);
        let body = Rect::new(rest.x, rest.y, BODY_WIDE * s, rest.h);
        let hands = Rect::new(
            rest.x + body.w + 2.0 * gap,
            rest.y,
            rest.w - body.w - 2.0 * gap,
            rest.h,
        );
        sheet_column(ui, body, v.pack, v.own);
        hands_column(ui, hands, v.pack, v.own, v.worn, v.rate);
        let row = Rect::new(rest.x, rest.y + rest.h + gap, rest.w, h);
        let (_, words) = verdict(v.pack, v.own);
        ui.label(
            row.x + row.w * 0.42,
            row.y + (row.h - line) * 0.5,
            row.w * 0.58,
            ui::FAINT,
            &words,
        );
        let buttons = ui.buttons(Rect::new(row.x, row.y, row.w * 0.4, row.h), &["Close"]);
        if ui.button(buttons[0], "Close") {
            return CharacterAction::Close;
        }
        CharacterAction::None
    }
}

/// What the sheet's column takes: the body's line, the elements' line, the attributes'
/// header and five rows.
fn sheet_height<C: Canvas>(ui: &Ui<'_, C>) -> f32 {
    let s = ui.scale;
    let gap = 5.0 * s;
    let line = ui.line();
    3.0 * (line + gap) + 5.0 * (line + ATTR_ROW_EXTRA * s) + 4.0 * ATTR_GAP * s
}

/// The build, read: who the body is, its mode, its elements, and the five attributes
/// with their floors and what they buy (MATRIX.md 6).
fn sheet_column<C: Canvas>(ui: &mut Ui<'_, C>, area: Rect, pack: &ContentPack, b: &Build) {
    let s = ui.scale;
    let gap = 5.0 * s;
    let line = ui.line();
    let mut col = Column::new(area, gap);
    let who = match b.body_def(pack) {
        Some(body) => format!(
            "{}  |  {}  |  {} mode",
            body.name,
            crate::front::frame_name(body.frame),
            b.mode.name()
        ),
        None => format!(
            "{}  |  {} mode",
            crate::front::frame_name(b.frame),
            b.mode.name()
        ),
    };
    let r = col.take(line);
    ui.label(r.x, r.y, r.w, ui::TEXT, &who);
    let elements: Vec<&str> = Element::ALL
        .iter()
        .filter(|e| b.aspects.contains(**e))
        .map(|e| e.name())
        .collect();
    let elements = if elements.is_empty() {
        "no element".to_string()
    } else {
        elements.join(" + ")
    };
    let r = col.take(line);
    ui.label(r.x, r.y, r.w, ui::FAINT, &elements);
    let spent = b.points_spent(pack);
    let r = col.take(line);
    ui.label(
        r.x,
        r.y,
        r.w,
        ui::FOCUS,
        &format!("attributes   {spent} of {} points", Attributes::FREE_POINTS),
    );
    const NAMES: [&str; 5] = ["STR", "AGI", "CON", "INT", "SPR"];
    let attrs = b.attributes.as_array();
    let floors = b.floors(pack).as_array();
    let d = b.derived_in(pack);
    let buys = [
        format!("blows x{:.2}", d.physical_mult),
        format!("speed {:.0} u/s", d.max_speed),
        format!("health {}", d.health),
        format!("spells x{:.2}", d.elemental_mult),
        format!("ward {:.0}%", d.ward * 100.0),
    ];
    let row_h = line + ATTR_ROW_EXTRA * s;
    let (name_w, value_w) = (34.0 * s, 26.0 * s);
    let rows = col.take(5.0 * row_h + 4.0 * ATTR_GAP * s);
    for i in 0..5 {
        let y = rows.y + i as f32 * (row_h + ATTR_GAP * s) + (row_h - line) * 0.5;
        ui.label(rows.x, y, name_w, ui::TEXT, NAMES[i]);
        ui.label(rows.x + name_w, y, value_w, ui::TEXT, &attrs[i].to_string());
        let words_x = rows.x + name_w + value_w + 2.0 * gap;
        let words = if b.body.is_some() && rows.w >= 260.0 * s {
            format!("floor {}   {}", floors[i], buys[i])
        } else {
            buys[i].clone()
        };
        ui.label(words_x, y, rows.x + rows.w - words_x, ui::FAINT, &words);
    }
}

/// What the hands bring and what the body does (GEAR.md 2, BODIES.md 3.4): the kit as
/// it plays now, a line a slot, where each comes from and when it is back.
fn hands_column<C: Canvas>(
    ui: &mut Ui<'_, C>,
    area: Rect,
    pack: &ContentPack,
    b: &Build,
    worn: &Worn,
    rate: TickRate,
) {
    let gap = 5.0 * ui.scale;
    let line = ui.line();
    ui.label(area.x, area.y, area.w, ui::FAINT, "in hand and in the body");
    let mut y = area.y + line + gap;
    for (i, slot) in b.kit_slots_in(worn, pack) {
        let Some(def) = pack.abilities.get(i as usize) else {
            continue;
        };
        if y + line > area.y + area.h {
            break;
        }
        let from = match slot {
            Slot::Primary if worn.primary.is_some() => "the weapon",
            Slot::Primary => "empty hands",
            Slot::Secondary if worn.secondary.is_some() => "the hands' second move",
            Slot::Secondary => "the kick",
            Slot::Guard if worn.guard.is_some() => "the off hand",
            Slot::Guard => "the empty off hand",
            Slot::Active if b.body_skill(pack) == Some(i) => "the body",
            Slot::Active => "the build",
            Slot::Extra => continue,
        };
        let back = if def.ability.cooldown.ticks > 0 {
            format!(", {}", secs(rate, def.ability.cooldown.ticks))
        } else {
            String::new()
        };
        let text = ui.fit_text(area.w, &format!("{from}: {}{back}", def.ability.name));
        ui.label(area.x, y, area.w, ui::TEXT, &text);
        y += line + gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::tests::{Recorder, tidy};
    use crate::ui::{UiInput, UiState};
    use gm_core::sim::test_content;

    fn frame(
        page: &mut CharacterPage,
        st: &mut UiState,
        input: &UiInput,
        v: &CharacterView<'_>,
    ) -> CharacterAction {
        let mut canvas = Recorder::new(1280.0, 720.0);
        let mut ui = Ui::begin(&mut canvas, st, input, page.name(), 300.0);
        let action = page.frame(&mut ui, v);
        ui.end();
        action
    }

    fn click(
        page: &mut CharacterPage,
        st: &mut UiState,
        text: &str,
        v: &CharacterView<'_>,
    ) -> CharacterAction {
        frame(page, st, &UiInput::default(), v);
        let at = st
            .find(text)
            .unwrap_or_else(|| panic!("nothing says {text:?}"))
            .rect
            .centre();
        click_at(page, st, at, v)
    }

    fn click_at(
        page: &mut CharacterPage,
        st: &mut UiState,
        at: (f32, f32),
        v: &CharacterView<'_>,
    ) -> CharacterAction {
        let press = UiInput {
            cursor: at,
            pressed: true,
            down: true,
            ..Default::default()
        };
        frame(page, st, &press, v);
        let release = UiInput {
            cursor: at,
            released: true,
            ..Default::default()
        };
        frame(page, st, &release, v)
    }

    #[test]
    fn the_page_is_whole_at_every_size() {
        let pack = test_content::pack(TickRate::TOWN);
        let own = pack.builds[0].build.clone();
        let v = CharacterView {
            pack: &pack,
            own: &own,
            worn: &Worn::NONE,
            rate: TickRate::TOWN,
        };
        for size in crate::ui::tests::SIZES {
            let (mut page, mut st) = (CharacterPage, UiState::default());
            let mut canvas = Recorder::new(size.0, size.1);
            let input = UiInput::default();
            let mut ui = Ui::begin(&mut canvas, &mut st, &input, page.name(), 300.0);
            page.frame(&mut ui, &v);
            ui.end();
            tidy(&canvas, &st, 300.0);
            assert!(st.clipped.is_empty(), "at {size:?}: {:?}", st.clipped);
        }
    }

    /// The page reads the build and offers nothing to change (MATRIX.md 20): the
    /// points, the elements, the hands, and Close.
    #[test]
    fn the_page_reads_and_never_edits() {
        let pack = test_content::pack(TickRate::TOWN);
        let own = pack.builds[1].build.clone();
        let v = CharacterView {
            pack: &pack,
            own: &own,
            worn: &Worn::NONE,
            rate: TickRate::TOWN,
        };
        let (mut page, mut st) = (CharacterPage, UiState::default());
        frame(&mut page, &mut st, &UiInput::default(), &v);
        assert!(st.shows("30 of 30 points"), "{:?}", st.seen);
        assert!(st.shows("water"), "{:?}", st.seen);
        assert!(st.shows("STR"), "{:?}", st.seen);
        assert!(st.shows("empty hands: Sword"), "{:?}", st.seen);
        assert!(st.find("+").is_none(), "{:?}", st.seen);
        assert!(st.find("Wear it").is_none(), "{:?}", st.seen);
        assert_eq!(
            click(&mut page, &mut st, "Close", &v),
            CharacterAction::Close
        );
    }

    #[test]
    fn what_cannot_be_taken_is_off_and_says_why() {
        let pack = test_content::pack(TickRate::TOWN);
        let by = |key: &str| pack.find(key).unwrap_or_else(|| panic!("no {key}"));
        let ironclad = pack.build("ironclad").expect("ironclad").clone();
        let blade = pack.build("blade").expect("blade").clone();
        let musketeer = pack.build("musketeer").expect("musketeer").clone();
        // A firearm in an action build; a sword and a guard in an FPS build.
        assert_eq!(
            why_not(&pack, &blade, Slot::Primary, by("musket")).as_deref(),
            Some("a firearm: only an FPS build holds one")
        );
        assert_eq!(
            why_not(&pack, &musketeer, Slot::Primary, by("sword")).as_deref(),
            Some("an FPS build's weapon is a firearm")
        );
        assert_eq!(
            why_not(&pack, &musketeer, Slot::Guard, by("parry")).as_deref(),
            Some("an FPS build has no guard")
        );
        // An aspect the build lacks; what is in the kit already is fine. (The nova and
        // the stomp are the staff's second moves since BODIES.md 3.6: secondaries.)
        assert_eq!(
            why_not(&pack, &ironclad, Slot::Secondary, by("frost_nova")).as_deref(),
            Some("needs the water aspect")
        );
        assert_eq!(why_not(&pack, &ironclad, Slot::Active, by("fortify")), None);
        assert_eq!(
            why_not(&pack, &ironclad, Slot::Primary, ironclad.primary),
            None
        );
    }
}
