//! The selector (CLIENT.md 4.2 and 4.3): one character across the whole frame, over the
//! map turning behind the screens. The body, with what it holds, stands in the middle;
//! its name over it; the attributes and what they buy on the left, the kit on the right
//! with the words of the ability picked; arrows on both sides of the body go to the
//! previous and the next one. The screens that use it (the account's characters, the
//! archetypes of a new one) put their own fields and buttons in the strip at the bottom.

use gm_core::build::{Build, ContentPack, Slot};
use gm_core::tick::TickRate;

use crate::character::words_of;
use crate::front::frame_name;
use crate::ui::{self, Canvas, Rect, Ui};

/// A side panel is this wide, in units, where the frame allows; narrower frames give it
/// a share of the width down to `SIDE_LEAST`.
const SIDE_UNITS: f32 = 270.0;
const SIDE_LEAST: f32 = 150.0;
/// The arrows beside the body, in units.
const ARROW_W: f32 = 26.0;
const ARROW_H: f32 = 44.0;
/// Lines under the kit for the words of the ability picked, at least.
const ABILITY_LINES: f32 = 3.0;

/// What the selector shows this frame.
pub struct Shown<'a> {
    pub name: &'a str,
    pub build: &'a Build,
    /// The hub's content, once it is here: without it the kit is names the client
    /// does not have yet, and the panel says so.
    pub pack: Option<&'a ContentPack>,
    pub rate: TickRate,
    /// Under the body: what the archetype is (its blurb), or where a character is and
    /// how long it was played.
    pub words: &'a str,
    /// Which of how many, for the arrows and the line that counts them.
    pub index: usize,
    pub count: usize,
}

/// What the selector gave back.
pub struct Picked {
    /// The arrows: -1, 0 or 1 (the keys are the caller's, after its own fields).
    pub step: i32,
    /// A tab clicked, by index.
    pub tab: Option<usize>,
    /// The room inside the strip at the bottom, for the caller's fields and buttons.
    pub strip: Rect,
}

/// A row of buttons whose first is the one to press (Play, Create): half as wide again
/// as the others' share, so that it leads the row.
pub fn lead_buttons<C: Canvas>(ui: &Ui<'_, C>, area: Rect, labels: &[&str]) -> Vec<Rect> {
    let plain = ui.buttons(area, labels);
    if plain.len() < 2 {
        return plain;
    }
    let s = ui.scale;
    let gap = 6.0 * s;
    let n = labels.len() as f32;
    let room = area.w - gap * (n - 1.0);
    let unit = room / (n + 0.5);
    let mut x = area.x;
    (0..labels.len())
        .map(|i| {
            let w = if i == 0 { unit * 1.5 } else { unit };
            let r = Rect::new(x.round(), area.y, w.floor(), area.h);
            x += w + gap;
            r
        })
        .collect()
}

/// The names of a build's slots, in kit order.
fn slot_word(slot: Slot) -> &'static str {
    match slot {
        Slot::Primary => "weapon",
        Slot::Secondary => "secondary",
        Slot::Guard => "guard",
        Slot::Active => "active",
        Slot::Extra => "with it",
    }
}

/// A preset's name as a class is called (EQUIPMENT.md 2): its first letter up.
pub fn capitalised(name: &str) -> String {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) => c.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// What a build is called (EQUIPMENT.md 2, BODIES.md 3.3): its body's name when it has a
/// body (`Brute`); else the preset it is, by its class name; `Custom` otherwise, and
/// `Custom (a Blade)` when every ability of a preset's kit is in it. The content names
/// the bodies and the presets; without it nothing is said.
pub fn class_name(pack: Option<&ContentPack>, b: &Build) -> String {
    let Some(pack) = pack else {
        return String::new();
    };
    // A preset by its name first (the arena's bots are presets with bodies, BODIES.md
    // 7.5: an Ironclad, not a Brute); a player's build by its body.
    if let Some(p) = pack.builds.iter().find(|p| &p.build == b) {
        return capitalised(&p.name);
    }
    if let Some(body) = b.body_def(pack) {
        return body.name.clone();
    }
    let kit: Vec<u16> = b.slots().into_iter().map(|(i, _)| i).collect();
    let near = pack.builds.iter().find(|p| {
        p.build.frame == b.frame && p.build.slots().into_iter().all(|(i, _)| kit.contains(&i))
    });
    match near {
        Some(p) => {
            let name = capitalised(&p.name);
            let article = match name.chars().next() {
                Some('A' | 'E' | 'I' | 'O' | 'U') => "an",
                _ => "a",
            };
            format!("Custom ({article} {name})")
        }
        None => "Custom".to_string(),
    }
}

/// `Class  |  frame`, the equipment's line under the name (EQUIPMENT.md 4.1): the ribbon
/// without the aspects and the mode, which the selector says. (`frame in armour` until
/// GEAR.md 12, 2026-10-17: there is no armour class to say.)
pub fn short_ribbon(class: &str, b: &Build) -> String {
    let body = frame_name(b.frame).to_string();
    if class.is_empty() {
        body
    } else {
        format!("{class}  |  {body}")
    }
}

/// `Class  |  frame  |  group  |  aspects  |  mode`, the line under the name; the class
/// leads it when the content is here to name one, and the group (BODIES.md 7) is said
/// when the pack is here to read it. (The armour left the line on 2026-10-16: it is the
/// chest piece's, GEAR.md 3, worn in the world and not chosen here.)
pub fn ribbon(class: &str, pack: Option<&ContentPack>, b: &Build) -> String {
    let aspects: Vec<&str> = b.aspects.iter().map(|e| e.name()).collect();
    let aspects = if aspects.is_empty() {
        "no aspect".to_string()
    } else {
        aspects.join(" and ")
    };
    let lead = if class.is_empty() {
        String::new()
    } else {
        format!("{class}  |  ")
    };
    let group = match pack {
        Some(pack) => format!("{}  |  ", b.group(pack).name()),
        None => String::new(),
    };
    format!(
        "{lead}{}  |  {group}{}  |  {} mode",
        frame_name(b.frame),
        aspects,
        b.mode.name()
    )
}

/// Draw the selector. `shown` is `None` while what to show is still being asked for:
/// then `waiting` is said in the middle. `tabs` are names to put in a row under the
/// ribbon, the `index` one lit (the archetypes); without them the line counts
/// (`3 of 10`). `ability_at` is the kit's row picked, whose words are read under the kit.
/// `strip_inner` is how much room the caller wants in the strip at the bottom.
pub fn selector<C: Canvas>(
    ui: &mut Ui<'_, C>,
    shown: Option<&Shown<'_>>,
    waiting: &str,
    tabs: Option<&[String]>,
    ability_at: &mut usize,
    strip_inner: f32,
) -> Picked {
    let s = ui.scale;
    let (w, h) = ui.size();
    let m = 8.0 * s;
    let gap = 6.0 * s;
    let line = ui.line();
    let bh = ui.button_height();
    let mut picked = Picked {
        step: 0,
        tab: None,
        strip: Rect::new(0.0, 0.0, 0.0, 0.0),
    };
    // The strip at the bottom first: its plate is the first drawn, and what the
    // caller puts in it (a field with the keyboard) must have the keys before the arrows.
    let strip_h = ui.panel_height(strip_inner, false);
    let strip = Rect::new(m, h - m - strip_h, w - 2.0 * m, strip_h);
    picked.strip = ui.panel(strip, "");

    // The name over everything, the ribbon under it, then the tabs or the count.
    let top = m + 2.0 * s;
    let head = Rect::new(m, top, w - 2.0 * m, ui.title_line());
    let (name, ribbon_text) = match shown {
        Some(sh) => (
            sh.name.to_string(),
            ribbon(&class_name(sh.pack, sh.build), sh.pack, sh.build),
        ),
        None => (String::new(), String::new()),
    };
    let mut y = top;
    if !name.is_empty() {
        y += ui.heading(head, ui::FOCUS, &name);
    } else {
        y += ui.title_line();
    }
    y += 2.0 * s;
    if !ribbon_text.is_empty() {
        let tw = ui.text_width(&ribbon_text).min(head.w);
        ui.label(
            (head.x + (head.w - tw) * 0.5).round(),
            y,
            head.w,
            ui::FAINT,
            &ribbon_text,
        );
    }
    y += line + 2.0 * s;
    // What it is (the blurb; where a character is), under the ribbon: a line in the
    // middle where it fits, else wrapped across the frame, two lines at most.
    if let Some(sh) = shown
        && !sh.words.is_empty()
    {
        let tw = ui.text_width(sh.words);
        if tw <= head.w {
            ui.label(
                (head.x + (head.w - tw) * 0.5).round(),
                y,
                head.w,
                ui::TEXT,
                sh.words,
            );
            y += line;
        } else {
            let wide = (head.w * 0.8).max(tw.min(head.w) * 0.55).min(head.w);
            let lines = ui::wrap_measured(sh.words, wide, |t| ui.text_width(t)).len() as f32;
            let r = Rect::new(
                (head.x + (head.w - wide) * 0.5).round(),
                y,
                wide,
                2.0 * line,
            );
            ui.paragraph(r, ui::TEXT, sh.words);
            y += lines.min(2.0) * line;
        }
    } else {
        y += line;
    }
    y += gap;
    match tabs {
        Some(names) if !names.is_empty() => {
            // A row of names, as wide as they need, in the middle; the one shown lit.
            let labels: Vec<&str> = names.iter().map(String::as_str).collect();
            let need: f32 = labels
                .iter()
                .map(|l| ui.text_width(l) + 16.0 * s)
                .sum::<f32>()
                + 6.0 * s * labels.len().saturating_sub(1) as f32;
            let row_w = need.min(head.w);
            let row = Rect::new((head.x + (head.w - row_w) * 0.5).round(), y, row_w, bh);
            let at = shown.map_or(usize::MAX, |sh| sh.index);
            for (i, r) in ui.buttons(row, &labels).into_iter().enumerate() {
                if ui.button(r, labels[i]) {
                    picked.tab = Some(i);
                }
                if i == at {
                    ui.mark(r);
                }
            }
            y += bh + gap;
        }
        _ => {
            if let Some(sh) = shown
                && sh.count > 1
            {
                let count = format!("{} of {}", sh.index + 1, sh.count);
                let tw = ui.text_width(&count);
                ui.label(
                    (head.x + (head.w - tw) * 0.5).round(),
                    y,
                    head.w,
                    ui::FAINT,
                    &count,
                );
            }
            y += line + gap;
        }
    }

    // The middle: two side panels and the body between them.
    let middle = Rect::new(m, y, w - 2.0 * m, (strip.y - gap - y).max(0.0));
    let side_w = (middle.w * 0.26)
        .clamp(SIDE_LEAST * s, SIDE_UNITS * s)
        .min(middle.w * 0.4);
    // Each side panel as tall as what it holds, under the room there is: the attributes
    // (five rows, the points, seven derived, the kit's cost); the kit's rows and the
    // words under them.
    let row_h = line + 2.0 * s;
    let left_h = ui
        .panel_height(5.0 * row_h + gap + 4.0 * row_h, true)
        .min(middle.h);
    let kit_rows = shown.map_or(1, |sh| {
        sh.pack
            .map_or_else(
                || sh.build.slots().len(),
                |p| sh.build.kit_slots_in(&gm_core::build::Worn::NONE, p).len(),
            )
            .max(1)
    });
    // The words of the ability picked, in as many lines as they wrap to at this width
    // (three at least), so that nothing of them is cut where there is room.
    let picked_words = shown
        .and_then(|sh| {
            let pack = sh.pack?;
            let (i, _) = sh
                .build
                .kit_slots_in(&gm_core::build::Worn::NONE, pack)
                .get(*ability_at)
                .copied()?;
            pack.abilities.get(i as usize).map(|d| words_of(d, sh.rate))
        })
        .unwrap_or_default();
    let (numbers, effect) = split_words(&picked_words);
    let count = |t: &str| ui::wrap_measured(t, side_w - 16.0 * s, |t| ui.text_width(t)).len();
    let words_lines = (count(numbers) + count(effect)) as f32;
    let about_lines = words_lines.max(ABILITY_LINES);
    let right_h = ui
        .panel_height(ui.list_height(kit_rows) + gap + about_lines * line, true)
        .min(middle.h);
    let left = Rect::new(middle.x, middle.y, side_w, left_h);
    let right = Rect::new(middle.x + middle.w - side_w, middle.y, side_w, right_h);
    let centre = Rect::new(
        left.x + left.w + gap,
        middle.y,
        (right.x - gap - (left.x + left.w + gap)).max(0.0),
        middle.h,
    );
    let Some(sh) = shown else {
        ui.panel(left, "attributes");
        ui.panel(right, "abilities");
        let tw = ui.text_width(waiting).min(centre.w);
        ui.label(
            (centre.x + (centre.w - tw) * 0.5).round(),
            (centre.y + (centre.h - line) * 0.5).round(),
            centre.w,
            ui::FAINT,
            waiting,
        );
        return picked;
    };
    let b = sh.build;

    // The body, with the arrows at its sides.
    let doll_h = centre.h.max(0.0);
    let doll_w = (centre.w - 2.0 * (ARROW_W * s + gap))
        .min(doll_h * 1.1)
        .max(0.0);
    let doll = Rect::new(
        (centre.x + (centre.w - doll_w) * 0.5).round(),
        centre.y,
        doll_w.round(),
        doll_h.round(),
    );
    ui.paperdoll_open(doll);
    let arrow_y = (doll.y + (doll.h - ARROW_H * s) * 0.5).round();
    let prev = Rect::new(
        doll.x - gap - ARROW_W * s,
        arrow_y,
        ARROW_W * s,
        ARROW_H * s,
    );
    let next = Rect::new(doll.x + doll.w + gap, arrow_y, ARROW_W * s, ARROW_H * s);
    let several = sh.count > 1;
    if ui.button_if(prev, "<", several) {
        picked.step = -1;
    }
    if ui.button_if(next, ">", several) {
        picked.step = 1;
    }
    // Left: the five attributes with what each buys, then what else they come to.
    let inner = ui.panel(left, "attributes");
    let d = sh.pack.map_or_else(|| b.derived(), |p| b.derived_in(p));
    let attrs = b.attributes.as_array();
    const NAMES: [&str; 5] = ["STR", "AGI", "CON", "INT", "SPR"];
    let buys = [
        format!("blows x{:.2}", d.physical_mult),
        format!("speed {:.0} u/s", d.max_speed),
        format!("health {}", d.health),
        format!("spells x{:.2}", d.elemental_mult),
        format!("ward {:.0}%", d.ward * 100.0),
    ];
    let row_h = line + 2.0 * s;
    let (name_w, value_w) = (34.0 * s, 26.0 * s);
    let mut ry = inner.y;
    for i in 0..5 {
        ui.label(inner.x, ry, name_w, ui::TEXT, NAMES[i]);
        let value = attrs[i].to_string();
        let vw = ui.text_width(&value);
        ui.label(
            inner.x + name_w + (value_w - vw) * 0.5,
            ry,
            value_w,
            ui::TEXT,
            &value,
        );
        let wx = inner.x + name_w + value_w + gap;
        ui.label(wx, ry, inner.x + inner.w - wx, ui::FAINT, &buys[i]);
        ry += row_h;
    }
    // A body at its floors has its thirty points in hand (BODIES.md 6.7): said, so the
    // numbers above read as the floor they are.
    if let Some(p) = sh.pack
        && b.body.is_some()
    {
        let left = gm_core::matrix::Attributes::FREE_POINTS.saturating_sub(b.points_spent(p));
        if left > 0 {
            ui.label(
                inner.x,
                ry,
                inner.w,
                ui::FOCUS,
                &format!("+ {left} points to place"),
            );
            ry += row_h;
        }
    }
    ry += gap;
    // What else the body comes to (health, speed and ward are said above), name on the
    // left, number on the right; as many as there is room for.
    let derived: [(&str, String); 4] = [
        (
            "stamina",
            format!("{:.0} (+{:.0}/s)", d.stamina, d.stamina_regen),
        ),
        ("focus", format!("{:.0} (+{:.0}/s)", d.focus, d.focus_regen)),
        ("armour", format!("{:.0}%", d.armour * 100.0)),
        ("evasion", format!("{:.0}%", d.evasion * 100.0)),
    ];
    for (name, value) in derived {
        if ry + line > inner.y + inner.h {
            break;
        }
        ui.label(inner.x, ry, inner.w * 0.5, ui::FAINT, name);
        let vw = ui.text_width(&value);
        ui.label(
            (inner.x + inner.w - vw).max(inner.x + inner.w * 0.5),
            ry,
            inner.w * 0.5,
            ui::TEXT,
            &value,
        );
        ry += row_h;
    }
    // Right: the kit, slot by slot, and the words of the row picked under it.
    let inner = ui.panel(right, "abilities");
    let Some(pack) = sh.pack else {
        ui.label(inner.x, inner.y, inner.w, ui::FAINT, waiting);
        return picked;
    };
    let rows: Vec<Vec<String>> = b
        .kit_slots_in(&gm_core::build::Worn::NONE, pack)
        .iter()
        .map(|(i, slot)| {
            let name = pack
                .abilities
                .get(*i as usize)
                .map_or_else(|| "?".to_string(), |a| a.ability.name.clone());
            let word = if *slot == Slot::Active && b.body_skill(pack) == Some(*i) {
                "body skill"
            } else {
                slot_word(*slot)
            };
            vec![name, word.to_string()]
        })
        .collect();
    let about_h = about_lines * line;
    let list_h = ui
        .list_height(rows.len().max(1))
        .min((inner.h - about_h - gap).max(ui.list_height(1)));
    let list = Rect::new(inner.x, inner.y, inner.w, list_h);
    if *ability_at >= rows.len() {
        *ability_at = 0;
    }
    ui.list(list, "abilities", &[0.0, 0.6], &rows, ability_at);
    let about = Rect::new(
        inner.x,
        list.y + list.h + gap,
        inner.w,
        (inner.y + inner.h - (list.y + list.h + gap)).max(0.0),
    );
    if !picked_words.is_empty() {
        // The numbers faint on their own line(s), what it does under them.
        let (numbers, effect) = split_words(&picked_words);
        let used = ui.paragraph(about, ui::FAINT, numbers);
        ui.paragraph(
            Rect::new(about.x, about.y + used, about.w, (about.h - used).max(0.0)),
            ui::TEXT,
            effect,
        );
    }
    picked
}

/// An ability's words (`words_of`) in two: `Name: 2 pts, 1.1 s cooldown.` and what it
/// does.
fn split_words(words: &str) -> (&str, &str) {
    match words.find(". ") {
        Some(i) => (&words[..i + 1], words[i + 2..].trim_start()),
        None => (words, ""),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ribbon_reads_the_build() {
        let pack = gm_core::sim::test_content::pack(TickRate::COMBAT);
        let ironclad = &pack.builds[0].build;
        assert_eq!(
            ribbon("", None, ironclad),
            "colossus  |  ground  |  action mode"
        );
        assert_eq!(
            ribbon(&class_name(Some(&pack), ironclad), Some(&pack), ironclad),
            "Ironclad  |  colossus  |  melee  |  ground  |  action mode"
        );
        assert_eq!(short_ribbon("Ironclad", ironclad), "Ironclad  |  colossus");
        let mut neutral = ironclad.clone();
        neutral.aspects = gm_core::matrix::Aspects::default();
        assert!(ribbon("", None, &neutral).contains("|  no aspect  |"));
    }

    /// EQUIPMENT.md 2: a preset is its class; a build with a preset's whole kit in it
    /// is a custom build of that class; anything else is custom; nothing without content.
    #[test]
    fn a_build_is_named_by_the_preset_it_is_or_contains() {
        let pack = gm_core::sim::test_content::pack(TickRate::COMBAT);
        let ironclad = pack.builds[0].build.clone();
        assert_eq!(class_name(Some(&pack), &ironclad), "Ironclad");
        assert_eq!(class_name(None, &ironclad), "");
        // A point moved: no preset, so the body's name (BODIES.md 7.5); without a body,
        // the preset the kit contains, else custom.
        let mut heavier = ironclad.clone();
        heavier.attributes.str_ = heavier.attributes.str_.saturating_sub(1);
        heavier.attributes.con += 1;
        assert_eq!(class_name(Some(&pack), &heavier), "Brute");
        heavier.body = None;
        assert_eq!(class_name(Some(&pack), &heavier), "Custom (an Ironclad)");
        let mut other = ironclad.clone();
        other.body = None;
        other.frame = gm_core::vocab::ArchetypeFrame::Caster;
        assert_eq!(class_name(Some(&pack), &other), "Custom");
    }
}
