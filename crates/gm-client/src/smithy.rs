//! The smithy (ITEMS.md 6.6): the smith's shop and the forge. Two screens, each opened with
//! `E` standing at its place in the town, built as the bag's are (`bag.rs`): every list is
//! asked for when the screen opens and after something the person did; what is picked is
//! picked by what it is; nothing is said that the hub did not say. Buying and forging
//! are said to the zone, which knows where the body stands; what a craft would make,
//! and its fee, the hub says before anything is paid (`PlayerEcon::CraftView`).
//!
//! The **shop**: the smith's wares, a row a layer (cores, frames, catalysts, gems), each
//! part with its price under it; the picked one in words; **Buy**. The **forge**: the
//! templates the forge makes, the picked one's layers as slots, the parts of the
//! inventory as a grid beside them; a part clicked goes into the first free slot of its
//! layer, dragged it goes where it is dropped, dragged back out it is a part again; the
//! words of what the slots would make and the fee, as the hub reads them; **Forge**.

use gm_hub_proto::player::{PlayerEcon, PlayerEconReply, PlayerRequest, PlayerResponse};
use gm_hub_proto::protocol::{
    CharacterId, CraftTemplate, ItemSummary, PLACE_NONE, SessionId, Ware,
};
use web_time::Instant;

use crate::bag::{Asked, Bag, ItemLooks, coin_row, material, name, tooltip_lines};
use crate::font::ADVANCE;
use crate::front::PANEL_UNITS;
use crate::hub::{Answer, HubApi, Pending};
use crate::ui::{self, Canvas, Column, Key, ListEvent, NONE, Rect, SlotThing, Ui};

/// The shop's layers, in the order of the rows, and what each row is called.
const SHELVES: [(&str, &str); 4] = [
    ("core", "cores"),
    ("frame", "frames"),
    ("catalyst", "catalysts"),
    ("gem", "gems"),
];
/// A shelf's cells: the content's widest layer (six catalysts).
const SHELF_CELLS: usize = 6;
/// The forge's two panels, in units (EQUIPMENT.md 4's rule: side by side).
pub const FORGE_UNITS: f32 = 300.0;
pub const PARTS_UNITS: f32 = 300.0;
/// The slots of a craft: a template's layers, with a gem twice (two gems at most).
const MAX_SLOTS: usize = 5;
const INVENTORY_SLOTS: usize = 24;
/// What a row has room for of a price under a cell.
const UNDER_CHARS: usize = 9;

#[derive(Clone, Debug, PartialEq)]
pub enum SmithyAction {
    None,
    Close,
    /// Said to the zone (`FromClient::SmithBuy`).
    Buy {
        material: String,
        price: i64,
    },
    /// Said to the zone (`FromClient::Infuse`): the Essence into the weapon in hand.
    Infuse {
        fee: i64,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub enum ForgeAction {
    None,
    Close,
    /// Said to the zone (`FromClient::Forge`).
    Forge {
        template: String,
        components: Vec<i64>,
        fee: i64,
    },
}

/// What the hub was asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ask {
    Wares,
    Inventory,
    /// A preview, by its number: an older one's answer is not taken.
    Preview(u32),
}

/// The asks of a screen and their answers: the hub's part of both screens.
struct Asks {
    session: SessionId,
    character: CharacterId,
    asks: Vec<(Ask, Pending<Answer>)>,
    serial: u32,
}

impl Asks {
    fn ask(&mut self, hub: &dyn HubApi, what: Ask, op: PlayerEcon) {
        let req = PlayerRequest::Econ {
            session: self.session,
            character: self.character,
            op,
        };
        self.asks.push((what, hub.call(req)));
    }

    /// The answers that have come, each with what it answers.
    fn take(&mut self) -> Vec<(Ask, Answer)> {
        let mut out = Vec::new();
        let mut i = 0;
        while i < self.asks.len() {
            match self.asks[i].1.take() {
                Some(answer) => {
                    let (what, _) = self.asks.remove(i);
                    out.push((what, answer));
                }
                None => i += 1,
            }
        }
        out
    }

    fn waiting(&self) -> bool {
        !self.asks.is_empty()
    }
}

/// The layer of a material's id (`core/iron` is a core).
fn layer_of(material: &str) -> &str {
    material.split('/').next().unwrap_or("")
}

/// The part's layer, from its one component.
fn part_layer(item: &ItemSummary) -> &str {
    item.components
        .first()
        .map_or("", |(layer, _)| layer.as_str())
}

/// The smith's shop (ITEMS.md 6.6).
pub struct Smithy {
    hub: Asks,
    /// `None` until the hub has said.
    wares: Option<Vec<Ware>>,
    /// The picked ware, by its place in the list.
    picked: Option<usize>,
    coin: Option<i64>,
    notice: String,
    bad: bool,
    buying: Asked,
    now: Instant,
}

impl Smithy {
    pub fn open(
        hub: &dyn HubApi,
        session: SessionId,
        character: CharacterId,
        now: Instant,
    ) -> Smithy {
        let mut s = Smithy {
            hub: Asks {
                session,
                character,
                asks: Vec::new(),
                serial: 0,
            },
            wares: None,
            picked: None,
            coin: None,
            notice: String::new(),
            bad: false,
            buying: Asked::default(),
            now,
        };
        s.hub.ask(hub, Ask::Wares, PlayerEcon::Smithy);
        s.hub.ask(hub, Ask::Inventory, PlayerEcon::Inventory);
        s
    }

    pub fn name(&self) -> &'static str {
        "smithy"
    }

    fn say(&mut self, text: impl Into<String>, bad: bool) {
        self.notice = text.into();
        self.bad = bad;
    }

    /// The zone answered a buy of `material`. `Some`: this screen was not waiting for
    /// it; here is what to say of it where the game says such things.
    pub fn bought(
        &mut self,
        hub: &dyn HubApi,
        material: &str,
        result: Result<(), String>,
    ) -> Option<String> {
        let about = self
            .wares
            .as_ref()
            .and_then(|w| w.iter().position(|w| w.material == material))
            .map_or(-1, |i| i as i64);
        if !self.buying.answered(about) {
            return Some(match result {
                Ok(()) => format!("bought {}: it is in the inventory", material_name(material)),
                Err(why) => format!("not bought: {why}"),
            });
        }
        match result {
            Ok(()) => self.say(
                format!("bought {}: it is in the inventory", material_name(material)),
                false,
            ),
            Err(why) => self.say(why, true),
        }
        self.hub.ask(hub, Ask::Inventory, PlayerEcon::Inventory);
        None
    }

    fn answers(&mut self) {
        for (what, answer) in self.hub.take() {
            match (what, answer) {
                (Ask::Wares, Ok(PlayerResponse::Econ(PlayerEconReply::Smithy { wares, .. }))) => {
                    if let Some(i) = self.picked
                        && i >= wares.len()
                    {
                        self.picked = None;
                    }
                    self.wares = Some(wares);
                }
                (
                    Ask::Inventory,
                    Ok(PlayerResponse::Econ(PlayerEconReply::Holder { coin, .. })),
                ) => {
                    self.coin = Some(coin);
                }
                (_, Err(e)) => self.say(Bag::words(&e), true),
                (_, Ok(_)) => {}
            }
        }
    }

    /// One frame. `ui` was begun for `self.name()`.
    pub fn frame<C: Canvas>(
        &mut self,
        ui: &mut Ui<'_, C>,
        hub: &dyn HubApi,
        looks: ItemLooks<'_>,
        essence: Option<(String, bool)>,
        now: Instant,
    ) -> SmithyAction {
        let _ = hub;
        self.now = now;
        if self.buying.gave_up(now) {
            self.say("the zone did not answer: try again", true);
        }
        let s = ui.scale;
        let gap = 5.0 * s;
        let line = ui.line();
        let h = ui.button_height();
        let side = ui.slot_side();
        let under = line;
        let shelf_h = side + under;
        let inner =
            line + gap + SHELVES.len() as f32 * (shelf_h + gap) + 2.0 * line + gap + line + gap + h;
        let panel = Rect::centred(ui.size(), PANEL_UNITS * s, ui.panel_height(inner, true));
        let inner = ui.panel(panel, "the smith's wares");
        let mut col = Column::new(inner, gap);
        let wares = self.wares.clone().unwrap_or_default();
        let picked_ware = self.picked.and_then(|i| wares.get(i));
        // The purse and the picked thing's price on one line.
        let top = col.take(line);
        Bag::purse(ui, top, "you have", self.coin);
        if let Some(w) = picked_ware {
            let half = Rect::new(top.x + top.w * 0.5, top.y, top.w * 0.5, top.h);
            Bag::purse(ui, half, "price", Some(w.price));
        }
        // A shelf a layer: its name down the left, its parts as cells with the price
        // under each. Picked by the ware's place in the list, across the shelves.
        let label_w = ui.text_width("catalysts") + 2.0 * ADVANCE * s;
        let tip = |t: &SlotThing| {
            wares
                .get(t.id as usize)
                .map(|w| {
                    let mut lines = tooltip_lines(&w.item, None);
                    lines.push((format!("price: {}", coin_row(w.price, 40)), ui::FOCUS));
                    lines
                })
                .unwrap_or_default()
        };
        let known = self.wares.is_some();
        let mut now_picked = self.picked;
        for (layer, shelf) in SHELVES {
            let row = col.take(shelf_h);
            ui.label(
                row.x,
                row.y + (side - line) * 0.5,
                label_w,
                ui::FAINT,
                shelf,
            );
            let things: Vec<SlotThing> = wares
                .iter()
                .enumerate()
                .filter(|(_, w)| layer_of(&w.material) == layer)
                .map(|(i, w)| SlotThing {
                    id: i as i64,
                    said: format!(
                        "{}  {}  {}",
                        material_name(&w.material),
                        w.item.does.first().map_or("", |d| d.as_str()),
                        coin_row(w.price, 14)
                    ),
                    under: coin_row(w.price, UNDER_CHARS),
                    under_colour: ui::FOCUS,
                    fixed: true,
                    ..looks.thing(&w.item)
                })
                .collect();
            let grid = Rect::new(row.x + label_w, row.y, row.w - label_w, row.h);
            let mut sel = self
                .picked
                .filter(|i| {
                    wares
                        .get(*i)
                        .is_some_and(|w| layer_of(&w.material) == layer)
                })
                .map(|i| i as i64);
            // A shelf with no parts yet (the hub has not said) draws its wells.
            let capacity = if known {
                things.len().max(1)
            } else {
                SHELF_CELLS
            };
            if ui.grid(
                grid,
                shelf,
                SHELF_CELLS,
                capacity,
                &things,
                &mut sel,
                under,
                &tip,
            ) == ListEvent::Picked
            {
                now_picked = sel.map(|i| i as usize);
            }
        }
        self.picked = now_picked;
        // The picked ware in words: what it is, and what it does in a craft.
        let said = col.take(2.0 * line);
        match picked_ware {
            Some(w) => {
                ui.label(
                    said.x,
                    said.y,
                    said.w,
                    ui::TEXT,
                    &format!("{}  {}", material_name(&w.material), w.item.what),
                );
                let words: Vec<&str> = w.item.does.iter().take(2).map(|d| d.as_str()).collect();
                ui.label(said.x, said.y + line, said.w, ui::TEXT, &words.join("  "));
            }
            None => {
                let nothing = match (known, wares.is_empty()) {
                    (false, _) => "",
                    (true, true) => "the smith has nothing for sale",
                    (true, false) => "pick a part to see it",
                };
                ui.label(said.x, said.y, said.w, ui::FAINT, nothing);
            }
        }
        // The notice: what is waited for, or the last thing said.
        let note = col.take(line);
        let (text, colour) = if self.buying.waiting() {
            ("asking the smith", ui::FAINT)
        } else if self.hub.waiting() {
            ("asking the hub", ui::FAINT)
        } else if self.bad {
            (self.notice.as_str(), ui::WARN)
        } else {
            (self.notice.as_str(), ui::TEXT)
        };
        if !text.is_empty() {
            ui.label(note.x, note.y, note.w, colour, text);
        }
        let row = ui.buttons(col.take(h), &["Buy", "Essence", "Close"]);
        let free = self.buying.free(now) && !self.hub.waiting();
        let short = picked_ware
            .zip(self.coin)
            .is_some_and(|(w, coin)| coin < w.price);
        let off = match (picked_ware, free, short) {
            (None, _, _) => Some(""),
            (_, false, _) => Some(""),
            (_, _, true) => Some("not enough coin"),
            _ => None,
        };
        let pressed = ui.button_or(row[0], "Buy", off);
        // The Essence (SKILLS.md 4): into the weapon in hand, for the fee.
        let fee = gm_hub_proto::protocol::ESSENCE_FEE;
        let essence_off = match &essence {
            None => Some("the rite wakes the Essence"),
            Some((_, true)) => Some("the Essence is in the weapon in hand"),
            Some(_) if !free => Some(""),
            Some(_) if self.coin.is_some_and(|c| c < fee) => Some("not enough coin"),
            Some(_) => None,
        };
        if ui.button_or(row[1], "Essence", essence_off) {
            self.notice = format!(
                "the Essence into the weapon in hand, {fee} silver: {}",
                essence.map_or(String::new(), |(name, _)| name)
            );
            return SmithyAction::Infuse { fee };
        }
        if ui.button(row[2], "Close") || ui.key(Key::Escape) {
            return SmithyAction::Close;
        }
        self.answers();
        match (pressed, picked_ware, self.picked) {
            (true, Some(w), Some(i)) => {
                self.buying.begin(i as i64, now);
                self.notice.clear();
                SmithyAction::Buy {
                    material: w.material.clone(),
                    price: w.price,
                }
            }
            _ => SmithyAction::None,
        }
    }
}

/// A material's name as a person reads it (`core/iron` is iron).
fn material_name(id: &str) -> String {
    material(id)
}

/// The forge (ITEMS.md 6.6).
pub struct Forge {
    hub: Asks,
    /// `None` until the hub has said.
    templates: Option<Vec<CraftTemplate>>,
    template: usize,
    /// The parts of the inventory (and the purse); `None` until the hub has said.
    items: Option<Vec<ItemSummary>>,
    coin: Option<i64>,
    picked_part: usize,
    /// The slots of the picked template: a layer each, and the part in it.
    slots: Vec<(String, Option<i64>)>,
    /// What the slots would make, as the hub last said: the item and the fee; or why
    /// not, in the hub's words.
    preview: Option<(ItemSummary, i64)>,
    why_not: Option<String>,
    /// The slots changed since the hub was last asked what they make.
    dirty: bool,
    notice: String,
    bad: bool,
    forging: Asked,
    now: Instant,
}

impl Forge {
    pub fn open(
        hub: &dyn HubApi,
        session: SessionId,
        character: CharacterId,
        now: Instant,
    ) -> Forge {
        let mut f = Forge {
            hub: Asks {
                session,
                character,
                asks: Vec::new(),
                serial: 0,
            },
            templates: None,
            template: NONE,
            items: None,
            coin: None,
            picked_part: NONE,
            slots: Vec::new(),
            preview: None,
            why_not: None,
            dirty: false,
            notice: String::new(),
            bad: false,
            forging: Asked::default(),
            now,
        };
        f.hub.ask(hub, Ask::Wares, PlayerEcon::Smithy);
        f.hub.ask(hub, Ask::Inventory, PlayerEcon::Inventory);
        f
    }

    pub fn name(&self) -> &'static str {
        "forge"
    }

    fn say(&mut self, text: impl Into<String>, bad: bool) {
        self.notice = text.into();
        self.bad = bad;
    }

    /// The parts of the inventory: loose components, worn by nobody.
    fn parts(&self) -> Vec<ItemSummary> {
        self.items
            .as_ref()
            .map(|items| {
                items
                    .iter()
                    .filter(|i| i.template == "component" && i.place == PLACE_NONE)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The template picked, when the hub has said and one is.
    fn picked_template(&self) -> Option<&CraftTemplate> {
        self.templates.as_ref()?.get(self.template)
    }

    /// The slots a template has: its layers in the content's order, the gem twice.
    fn slots_of(t: &CraftTemplate) -> Vec<(String, Option<i64>)> {
        let mut slots = Vec::new();
        for layer in &t.layers {
            slots.push((layer.clone(), None));
            if layer == "gem" {
                slots.push((layer.clone(), None));
            }
        }
        slots.truncate(MAX_SLOTS);
        slots
    }

    /// The parts in the slots, in the slots' order.
    fn components(&self) -> Vec<i64> {
        self.slots.iter().filter_map(|(_, id)| *id).collect()
    }

    /// Another template: the slots are its, emptied.
    fn pick_template(&mut self, at: usize) {
        self.template = at;
        self.slots = self
            .picked_template()
            .map(Self::slots_of)
            .unwrap_or_default();
        self.preview = None;
        self.why_not = None;
        self.dirty = true;
    }

    /// The part `id` into the first free slot of its layer; `Err` with why not.
    fn place(&mut self, id: i64, layer: &str) -> Result<(), String> {
        if self.slots.iter().any(|(_, in_it)| *in_it == Some(id)) {
            return Ok(());
        }
        let what = self
            .picked_template()
            .map_or("a craft", |t| t.template.as_str())
            .to_string();
        if !self.slots.iter().any(|(l, _)| l == layer) {
            return Err(format!("a {what} takes no {layer}"));
        }
        match self
            .slots
            .iter_mut()
            .find(|(l, in_it)| l == layer && in_it.is_none())
        {
            Some((_, in_it)) => {
                *in_it = Some(id);
                self.dirty = true;
                Ok(())
            }
            None => Err(format!("a {what} has no room for another {layer}")),
        }
    }

    /// The part `id` into slot `at`, out of wherever it was; `Err` with why not.
    fn place_in(&mut self, id: i64, layer: &str, at: usize) -> Result<(), String> {
        let Some((want, _)) = self.slots.get(at) else {
            return Ok(());
        };
        if want != layer {
            return Err(format!("a {layer} does not go in the {want}'s slot"));
        }
        self.take_out(id);
        self.slots[at].1 = Some(id);
        self.dirty = true;
        Ok(())
    }

    /// The part `id` out of its slot, a part of the inventory again.
    fn take_out(&mut self, id: i64) {
        for (_, in_it) in &mut self.slots {
            if *in_it == Some(id) {
                *in_it = None;
                self.dirty = true;
            }
        }
    }

    fn clear(&mut self) {
        for (_, in_it) in &mut self.slots {
            *in_it = None;
        }
        self.preview = None;
        self.why_not = None;
        self.dirty = true;
    }

    /// The zone answered a forge. `Some`: this screen was not waiting for it; here is
    /// what to say of it where the game says such things.
    pub fn forged(&mut self, hub: &dyn HubApi, result: Result<i64, String>) -> Option<String> {
        if !self.forging.answered(0) {
            return Some(match result {
                Ok(_) => "forged: it is in the inventory".to_string(),
                Err(why) => format!("not forged: {why}"),
            });
        }
        match result {
            Ok(_) => {
                let made = self
                    .picked_template()
                    .map_or("it".to_string(), |t| t.template.clone());
                self.say(format!("forged: the {made} is in the inventory"), false);
                self.clear();
                self.dirty = false;
            }
            Err(why) => self.say(why, true),
        }
        self.hub.ask(hub, Ask::Inventory, PlayerEcon::Inventory);
        None
    }

    /// The hub asked what the slots would make, when they changed and hold anything.
    fn preview_ask(&mut self, hub: &dyn HubApi) {
        if !std::mem::take(&mut self.dirty) {
            return;
        }
        let components = self.components();
        let Some(template) = self.picked_template().map(|t| t.template.clone()) else {
            return;
        };
        if components.is_empty() {
            self.preview = None;
            self.why_not = None;
            return;
        }
        self.hub.serial += 1;
        let n = self.hub.serial;
        self.hub.ask(
            hub,
            Ask::Preview(n),
            PlayerEcon::CraftView {
                template,
                components,
            },
        );
    }

    fn answers(&mut self) {
        let newest = self.hub.serial;
        for (what, answer) in self.hub.take() {
            match (what, answer) {
                (
                    Ask::Wares,
                    Ok(PlayerResponse::Econ(PlayerEconReply::Smithy { templates, .. })),
                ) => {
                    self.templates = Some(templates);
                    if self.template == NONE
                        && self.templates.as_ref().is_some_and(|t| !t.is_empty())
                    {
                        self.pick_template(0);
                    }
                }
                (
                    Ask::Inventory,
                    Ok(PlayerResponse::Econ(PlayerEconReply::Holder { coin, items })),
                ) => {
                    self.coin = Some(coin);
                    // A part that is gone (sold, stored, spent) is out of its slot too.
                    let ids: Vec<i64> = items.iter().map(|i| i.id).collect();
                    for (_, in_it) in &mut self.slots {
                        if in_it.is_some_and(|id| !ids.contains(&id)) {
                            *in_it = None;
                            self.dirty = true;
                        }
                    }
                    self.items = Some(items);
                }
                (Ask::Preview(n), answer) if n == newest => match answer {
                    Ok(PlayerResponse::Econ(PlayerEconReply::CraftView { item, fee })) => {
                        self.preview = Some((item, fee));
                        self.why_not = None;
                    }
                    Err(e) => {
                        self.preview = None;
                        self.why_not = Some(Bag::words(&e));
                    }
                    Ok(_) => {}
                },
                (Ask::Preview(_), _) => {}
                (_, Err(e)) => self.say(Bag::words(&e), true),
                (_, Ok(_)) => {}
            }
        }
    }

    /// One frame. `ui` was begun for `self.name()`.
    pub fn frame<C: Canvas>(
        &mut self,
        ui: &mut Ui<'_, C>,
        hub: &dyn HubApi,
        looks: ItemLooks<'_>,
        now: Instant,
    ) -> ForgeAction {
        self.now = now;
        if self.forging.gave_up(now) {
            self.say("the zone did not answer: try again", true);
        }
        let s = ui.scale;
        let gap = 5.0 * s;
        let line = ui.line();
        let h = ui.button_height();
        let side = ui.slot_side();
        let cols = 6usize;
        let list_h = ui.list_height(6);
        let slots_h = side + line;
        let grid_h = 4.0 * side + 3.0 * 2.0 * s;
        // Where the two panels stand: side by side in the middle of the frame.
        let (fw, fh) = ui.size();
        let forge_w = FORGE_UNITS * s;
        let parts_w = PARTS_UNITS * s;
        let between = 10.0 * s;
        let forge_inner =
            list_h + gap + slots_h + gap + 3.0 * line + gap + line + gap + line + gap + h;
        let forge_h = ui.panel_height(forge_inner, true);
        let parts_inner = line + gap + grid_h + gap + 2.0 * line + gap + line;
        let parts_h = ui.panel_height(parts_inner, true);
        let x0 = ((fw - (forge_w + between + parts_w)) * 0.5).round();
        let forge_rect = Rect::new(x0, ((fh - forge_h) * 0.5).round(), forge_w, forge_h);
        let parts_rect = Rect::new(
            x0 + forge_w + between,
            ((fh - parts_h) * 0.5).round(),
            parts_w,
            parts_h,
        );
        let parts = self.parts();
        let part_of = |id: i64| parts.iter().find(|p| p.id == id);
        let slotted: Vec<i64> = self.components();

        // The forge: the templates, the slots, what they would make, the fee, the buttons.
        let inner = ui.panel(forge_rect, "forge");
        let mut col = Column::new(inner, gap);
        let templates = self.templates.clone().unwrap_or_default();
        let rows: Vec<Vec<String>> = templates
            .iter()
            .map(|t| vec![t.template.replace('_', " "), t.kind.clone()])
            .collect();
        let mut sel = self.template;
        let list = col.take(list_h);
        ui.list(list, "templates", &[0.0, 0.62], &rows, &mut sel);
        if sel != self.template && sel != NONE && sel < templates.len() {
            self.pick_template(sel);
        }
        // The slots: a layer each, named for it, the part in it as a cell.
        let slots_row = col.take(slots_h);
        let n = self.slots.len().max(1) as f32;
        // The slots across the panel, each with its name under it: a cell wide at least,
        // and no wider than a cell and a name.
        let pitch = ((slots_row.w - (n - 1.0) * gap) / n).clamp(side, side + 2.0 * line);
        let mut dropped_on: Vec<(usize, Option<(String, i64)>)> = Vec::new();
        let gems = self.slots.iter().filter(|(l, _)| l == "gem").count();
        let mut gem_n = 0;
        let names: Vec<String> = self
            .slots
            .iter()
            .map(|(layer, _)| {
                if layer == "gem" && gems > 1 {
                    gem_n += 1;
                    format!("gem {gem_n}")
                } else {
                    layer.clone()
                }
            })
            .collect();
        let slot_tip = |t: &SlotThing| {
            part_of(t.id)
                .map(|p| tooltip_lines(p, None))
                .unwrap_or_default()
        };
        for (i, (_, in_it)) in self.slots.iter().enumerate() {
            let x = slots_row.x + i as f32 * (pitch + gap);
            let cell = Rect::new(x, slots_row.y, side, side);
            let thing = in_it.and_then(part_of).map(|p| looks.thing(p));
            let dropped = ui.drop_slot(cell, &names[i], thing.as_ref(), &slot_tip);
            dropped_on.push((i, dropped));
            ui.label(x, slots_row.y + side, pitch, ui::FAINT, &names[i]);
        }
        if self.slots.is_empty() {
            let nothing = if self.templates.is_some() {
                "pick what to make"
            } else {
                ""
            };
            ui.label(slots_row.x, slots_row.y, slots_row.w, ui::FAINT, nothing);
        }
        // What it would make: the hub's words, or why not, or what is still to put in.
        let said = col.take(3.0 * line);
        match (&self.preview, &self.why_not) {
            (Some((item, _)), _) => {
                ui.label(
                    said.x,
                    said.y,
                    said.w,
                    ui::TEXT,
                    &format!("makes: {}  {}", name(item), item.what),
                );
                let rest = Rect::new(said.x, said.y + line, said.w, 2.0 * line);
                ui.paragraph(rest, ui::TEXT, &item.does.join("  "));
            }
            (None, Some(why)) => {
                ui.paragraph(said, ui::WARN, why);
            }
            (None, None) => {
                let hint = if self.slots.is_empty() {
                    ""
                } else if slotted.is_empty() {
                    "put a core and a frame in, from the parts beside"
                } else {
                    "asking the hub what it makes"
                };
                ui.label(said.x, said.y, said.w, ui::FAINT, hint);
            }
        }
        // The fee and the purse on one line.
        let money = col.take(line);
        let fee = self.preview.as_ref().map(|(_, fee)| *fee);
        Bag::purse(ui, money, "fee", fee);
        let half = Rect::new(money.x + money.w * 0.5, money.y, money.w * 0.5, money.h);
        Bag::purse(ui, half, "you have", self.coin);
        // The notice.
        let note = col.take(line);
        let (text, colour) = if self.forging.waiting() {
            ("asking the forge", ui::FAINT)
        } else if self.bad {
            (self.notice.as_str(), ui::WARN)
        } else {
            (self.notice.as_str(), ui::TEXT)
        };
        if !text.is_empty() {
            ui.label(note.x, note.y, note.w, colour, text);
        }
        let row = ui.buttons(col.take(h), &["Forge", "Clear", "Close"]);
        let free = self.forging.free(now) && !self.hub.waiting() && !self.dirty;
        let short = fee.zip(self.coin).is_some_and(|(fee, coin)| coin < fee);
        let off = match (&self.preview, free, short) {
            (None, _, _) => Some(""),
            (_, false, _) => Some(""),
            (_, _, true) => Some("not enough coin for the fee"),
            _ => None,
        };
        let forge_pressed = ui.button_or(row[0], "Forge", off);
        let clear_pressed = ui.button_if(row[1], "Clear", !slotted.is_empty());
        let close = ui.button(row[2], "Close") || ui.key(Key::Escape);

        // The parts: the purse, the loose components of the inventory that are not in a
        // slot, the picked one in words, and how the slots are filled.
        let inner = ui.panel(parts_rect, "parts");
        let mut col = Column::new(inner, gap);
        let count = col.take(line);
        let shown: Vec<&ItemSummary> = parts.iter().filter(|p| !slotted.contains(&p.id)).collect();
        let counted = match &self.items {
            Some(_) => format!("{} parts in the inventory", parts.len()),
            None => String::new(),
        };
        ui.label(count.x, count.y, count.w, ui::FAINT, &counted);
        let things: Vec<SlotThing> = shown.iter().map(|p| looks.thing(p)).collect();
        let area = col.take(grid_h);
        let grid_w = cols as f32 * side + (cols - 1) as f32 * 2.0 * s;
        let grid = Rect::new(area.x, area.y, grid_w, area.h);
        let mut part_sel = shown.get(self.picked_part).map(|p| p.id);
        let tip = |t: &SlotThing| {
            part_of(t.id)
                .map(|p| tooltip_lines(p, None))
                .unwrap_or_default()
        };
        let event = ui.grid(
            grid,
            "parts",
            cols,
            INVENTORY_SLOTS,
            &things,
            &mut part_sel,
            0.0,
            &tip,
        );
        let dropped_in_grid = ui.dropped("parts");
        let said = col.take(2.0 * line);
        let picked = part_sel.and_then(part_of);
        match picked {
            Some(p) => {
                ui.label(
                    said.x,
                    said.y,
                    said.w,
                    ui::TEXT,
                    &format!("{}  {}", name(p), p.what),
                );
                let words: Vec<&str> = p.does.iter().take(2).map(|d| d.as_str()).collect();
                ui.label(said.x, said.y + line, said.w, ui::TEXT, &words.join("  "));
            }
            None => {
                let nothing = match (&self.items, shown.is_empty()) {
                    (None, _) => "",
                    (Some(_), true) if parts.is_empty() => "no parts: the smith sells them",
                    (Some(_), true) => "every part is in a slot",
                    (Some(_), false) => "click a part to put it in, drag it to a slot",
                };
                ui.label(said.x, said.y, said.w, ui::FAINT, nothing);
            }
        }
        let how = col.take(line);
        ui.label(
            how.x,
            how.y,
            how.w,
            ui::FAINT,
            "drag a part out of a slot to take it back",
        );

        // What the person did, in order: a part clicked goes into its layer's slot; one
        // dropped on a slot goes there; one dropped back on the grid comes out.
        if event == ListEvent::Picked
            && let Some(id) = part_sel
            && let Some(p) = part_of(id)
        {
            let layer = part_layer(p).to_string();
            match self.place(id, &layer) {
                Ok(()) => {
                    self.notice.clear();
                    self.bad = false;
                    part_sel = None;
                }
                Err(why) => self.say(why, true),
            }
        }
        for (at, dropped) in dropped_on {
            if let Some((_, id)) = dropped
                && let Some(p) = part_of(id)
            {
                let layer = part_layer(p).to_string();
                match self.place_in(id, &layer, at) {
                    Ok(()) => {
                        self.notice.clear();
                        self.bad = false;
                    }
                    Err(why) => self.say(why, true),
                }
            }
        }
        if let Some((from, id)) = dropped_in_grid
            && from != "parts"
        {
            self.take_out(id);
        }
        self.picked_part = part_sel
            .and_then(|id| shown.iter().position(|p| p.id == id))
            .unwrap_or(NONE);
        if clear_pressed {
            self.clear();
            self.notice.clear();
            self.bad = false;
        }
        if close {
            return ForgeAction::Close;
        }
        self.preview_ask(hub);
        self.answers();
        let would = self
            .preview
            .as_ref()
            .map(|(_, fee)| *fee)
            .zip(self.picked_template().map(|t| t.template.clone()));
        match (forge_pressed, would) {
            (true, Some((fee, template))) => {
                self.forging.begin(0, now);
                self.notice.clear();
                ForgeAction::Forge {
                    template,
                    components: self.components(),
                    fee,
                }
            }
            _ => ForgeAction::None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use gm_hub_proto::protocol::{HubError, PLACE_WEAPON};

    use super::*;
    use crate::hub::RpcError;
    use crate::ui::tests::{Recorder, SIZES, tidy};
    use crate::ui::{UiInput, UiState};

    const S: SessionId = SessionId([7; 16]);
    const ME: CharacterId = 41;

    fn part(id: i64, material: &str) -> ItemSummary {
        ItemSummary {
            id,
            template: "component".to_string(),
            components: vec![(layer_of(material).to_string(), material.to_string())],
            place: PLACE_NONE,
            edge: [0; 9],
            worn: false,
            worn_in: PLACE_NONE,
            off: false,
            quantity: 1,
            cap: 0,
            fits: true,
            hands: String::new(),
            both_hands: false,
            what: format!("a {}, for crafting", layer_of(material)),
            does: vec!["edge 30 of 250".to_string(), "the standard".to_string()],
        }
    }

    /// The hub as these screens know it: the wares, the templates, an inventory, and
    /// what a craft of iron on oak makes.
    struct Shop {
        coin: i64,
        items: Vec<ItemSummary>,
        asked: Vec<PlayerEcon>,
        refuse_preview: Option<String>,
    }

    struct Hub(RefCell<Shop>);

    impl HubApi for Hub {
        fn call(&self, req: PlayerRequest) -> Pending<Answer> {
            let PlayerRequest::Econ {
                session: S,
                character: ME,
                op,
            } = req
            else {
                panic!("not what these screens ask: {req:?}");
            };
            let mut shop = self.0.borrow_mut();
            shop.asked.push(op.clone());
            let reply = match op {
                PlayerEcon::Inventory => Ok(PlayerEconReply::Holder {
                    coin: shop.coin,
                    items: shop.items.clone(),
                }),
                PlayerEcon::Smithy => Ok(PlayerEconReply::Smithy {
                    wares: [
                        ("core/tin", 5),
                        ("core/iron", 15),
                        ("frame/oak", 5),
                        ("catalyst/ember", 100),
                        ("gem/opal", 300),
                    ]
                    .iter()
                    .map(|(m, p)| Ware {
                        material: m.to_string(),
                        price: *p,
                        item: part(0, m),
                    })
                    .collect(),
                    templates: vec![
                        CraftTemplate {
                            template: "sword".into(),
                            kind: "weapon".into(),
                            layers: ["core", "catalyst", "frame", "gem"]
                                .iter()
                                .map(|l| l.to_string())
                                .collect(),
                        },
                        CraftTemplate {
                            template: "cuirass".into(),
                            kind: "armour".into(),
                            layers: ["core", "frame", "gem"]
                                .iter()
                                .map(|l| l.to_string())
                                .collect(),
                        },
                    ],
                }),
                PlayerEcon::CraftView {
                    template,
                    components,
                } => match &shop.refuse_preview {
                    Some(why) => Err(HubError::Invalid(why.clone())),
                    None if components.len() < 2 => Err(HubError::Invalid(
                        "a craft needs exactly one core and one frame".into(),
                    )),
                    None => {
                        let mut item = part(0, "core/iron");
                        item.template = template;
                        item.place = PLACE_WEAPON;
                        item.what = "a weapon, 40 of 250".into();
                        item.does = vec!["slash +2.0%".into()];
                        Ok(PlayerEconReply::CraftView { item, fee: 20 })
                    }
                },
                other => panic!("not what these screens ask: {other:?}"),
            };
            Pending::ready(reply.map(PlayerResponse::Econ).map_err(RpcError::Refused))
        }
    }

    fn shop() -> Hub {
        Hub(RefCell::new(Shop {
            coin: 120,
            items: vec![
                part(1, "core/iron"),
                part(2, "frame/oak"),
                part(3, "gem/opal"),
            ],
            asked: Vec::new(),
            refuse_preview: None,
        }))
    }

    struct Run {
        st: UiState,
        t: Instant,
        size: (f32, f32),
    }

    impl Run {
        fn new() -> Run {
            Run {
                st: UiState::default(),
                t: Instant::now(),
                size: (1280.0, 720.0),
            }
        }

        fn shop_frame(&mut self, s: &mut Smithy, input: &UiInput, hub: &Hub) -> SmithyAction {
            let mut canvas = Recorder::new(self.size.0, self.size.1);
            let mut ui = Ui::begin(&mut canvas, &mut self.st, input, s.name(), PANEL_UNITS);
            let a = s.frame(&mut ui, hub, ItemLooks::default(), None, self.t);
            ui.end();
            tidy(&canvas, &self.st, PANEL_UNITS);
            a
        }

        fn forge_frame(&mut self, f: &mut Forge, input: &UiInput, hub: &Hub) -> ForgeAction {
            let mut canvas = Recorder::new(self.size.0, self.size.1);
            let mut ui = Ui::begin(&mut canvas, &mut self.st, input, f.name(), PANEL_UNITS);
            let a = f.frame(&mut ui, hub, ItemLooks::default(), self.t);
            ui.end();
            tidy(&canvas, &self.st, PANEL_UNITS);
            a
        }

        fn click(&self, text: &str) -> UiInput {
            let r = self
                .st
                .find(text)
                .unwrap_or_else(|| panic!("nothing says {text:?}: {:?}", self.st.seen));
            let (x, y) = r.rect.centre();
            UiInput {
                cursor: (x, y),
                pressed: true,
                released: true,
                down: false,
                ..Default::default()
            }
        }
    }

    #[test]
    fn the_shop_shows_the_wares_by_shelf_and_buys_the_picked_one() {
        let hub = shop();
        let mut run = Run::new();
        let mut s = Smithy::open(&hub, S, ME, run.t);
        // The first frame asks; the second shows.
        run.shop_frame(&mut s, &UiInput::default(), &hub);
        run.shop_frame(&mut s, &UiInput::default(), &hub);
        assert!(run.st.shows("1 g 20 s"), "{:?}", run.st.seen);
        assert!(run.st.shows("iron  edge 30 of 250  15 s"));
        assert!(run.st.shows("pick a part to see it"));
        // Nothing picked: no buy.
        assert!(
            run.st.find("Buy").is_none(),
            "Buy is off with nothing picked"
        );
        let press = run.click("iron  edge 30 of 250  15 s");
        run.shop_frame(&mut s, &press, &hub);
        run.shop_frame(&mut s, &UiInput::default(), &hub);
        assert!(run.st.shows("iron  a core, for crafting"));
        let buy = run.click("Buy");
        let action = run.shop_frame(&mut s, &buy, &hub);
        assert_eq!(
            action,
            SmithyAction::Buy {
                material: "core/iron".into(),
                price: 15
            }
        );
        // Waiting for the zone: another buy is not sent.
        let buy = run.click("Buy");
        let _ = buy;
        run.shop_frame(&mut s, &UiInput::default(), &hub);
        assert!(run.st.shows("asking the smith"));
        assert_eq!(s.bought(&hub, "core/iron", Ok(())), None);
        // The purse is asked again; the frame after the answer says what was done.
        run.shop_frame(&mut s, &UiInput::default(), &hub);
        run.shop_frame(&mut s, &UiInput::default(), &hub);
        assert!(
            run.st.shows("bought iron: it is in the inventory"),
            "{:?}",
            run.st.seen
        );
        // Too dear: the button says why.
        hub.0.borrow_mut().coin = 10;
        run.shop_frame(&mut s, &UiInput::default(), &hub);
        run.shop_frame(&mut s, &UiInput::default(), &hub);
        let press = run.click("opal  edge 30 of 250  3 g");
        run.shop_frame(&mut s, &press, &hub);
        run.shop_frame(&mut s, &UiInput::default(), &hub);
        assert!(run.st.find("Buy").is_none());
    }

    #[test]
    fn the_forge_fills_slots_by_click_and_asks_what_they_make() {
        let hub = shop();
        let mut run = Run::new();
        let mut f = Forge::open(&hub, S, ME, run.t);
        run.forge_frame(&mut f, &UiInput::default(), &hub);
        run.forge_frame(&mut f, &UiInput::default(), &hub);
        assert!(
            run.st.shows("3 parts in the inventory"),
            "{:?}",
            run.st.seen
        );
        assert!(run.st.shows("core: nothing"));
        assert!(run.st.shows("gem 2: nothing"));
        assert!(
            run.st
                .shows("put a core and a frame in, from the parts beside")
        );
        // A click puts the part in its layer's slot; one part is not a craft.
        let press = run.click("iron  edge 30 of 250");
        run.forge_frame(&mut f, &press, &hub);
        run.forge_frame(&mut f, &UiInput::default(), &hub);
        run.forge_frame(&mut f, &UiInput::default(), &hub);
        assert!(run.st.shows("core: iron"), "{:?}", run.st.seen);
        assert!(run.st.shows("a craft needs exactly one core and one frame"));
        assert!(run.st.find("Forge").is_none());
        let press = run.click("oak  edge 30 of 250");
        run.forge_frame(&mut f, &press, &hub);
        run.forge_frame(&mut f, &UiInput::default(), &hub);
        run.forge_frame(&mut f, &UiInput::default(), &hub);
        assert!(run.st.shows("frame: oak"));
        assert!(
            run.st.shows("makes: sword  a weapon, 40 of 250"),
            "{:?}",
            run.st.seen
        );
        assert!(run.st.shows("20 s"));
        let forge = run.click("Forge");
        let action = run.forge_frame(&mut f, &forge, &hub);
        assert_eq!(
            action,
            ForgeAction::Forge {
                template: "sword".into(),
                components: vec![1, 2],
                fee: 20
            }
        );
        run.forge_frame(&mut f, &UiInput::default(), &hub);
        assert!(run.st.shows("asking the forge"));
        hub.0.borrow_mut().items = vec![part(3, "gem/opal")];
        assert_eq!(f.forged(&hub, Ok(99)), None);
        run.forge_frame(&mut f, &UiInput::default(), &hub);
        run.forge_frame(&mut f, &UiInput::default(), &hub);
        assert!(run.st.shows("forged: the sword is in the inventory"));
        assert!(run.st.shows("core: nothing"));
        assert!(run.st.shows("1 parts in the inventory"));
        // Another template empties the slots and offers its own.
        let press = run.click("cuirass");
        run.forge_frame(&mut f, &press, &hub);
        run.forge_frame(&mut f, &UiInput::default(), &hub);
        assert!(run.st.find("catalyst: nothing").is_none());
        assert!(run.st.shows("gem 1: nothing"));
    }

    #[test]
    fn both_screens_are_whole_at_every_size() {
        for size in SIZES {
            let hub = shop();
            let mut run = Run::new();
            run.size = size;
            let mut s = Smithy::open(&hub, S, ME, run.t);
            run.shop_frame(&mut s, &UiInput::default(), &hub);
            run.shop_frame(&mut s, &UiInput::default(), &hub);
            let press = run.click("iron  edge 30 of 250  15 s");
            run.shop_frame(&mut s, &press, &hub);
            run.shop_frame(&mut s, &UiInput::default(), &hub);
            assert!(run.st.clipped.is_empty(), "{size:?}: {:?}", run.st.clipped);
            let mut run = Run::new();
            run.size = size;
            let mut f = Forge::open(&hub, S, ME, run.t);
            run.forge_frame(&mut f, &UiInput::default(), &hub);
            run.forge_frame(&mut f, &UiInput::default(), &hub);
            let press = run.click("iron  edge 30 of 250");
            run.forge_frame(&mut f, &press, &hub);
            let press = run.click("oak  edge 30 of 250");
            run.forge_frame(&mut f, &press, &hub);
            run.forge_frame(&mut f, &UiInput::default(), &hub);
            run.forge_frame(&mut f, &UiInput::default(), &hub);
            assert!(
                run.st.shows("makes: sword  a weapon, 40 of 250"),
                "{size:?}"
            );
            assert!(run.st.clipped.is_empty(), "{size:?}: {:?}", run.st.clipped);
        }
    }
}
