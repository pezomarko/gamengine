use super::*;

/// The prop a look names, on the GPU (LOOK.md 6): loaded from the bundle when first seen;
/// nothing while it loads, for a look the pack does not name, or without a renderer.
pub(super) fn held_prop(
    content: &mut Content,
    active: Option<&mut Active>,
    props: &[String],
    look: Look,
) -> Option<usize> {
    prop_slot(content, active, props, look.held)
}

/// What a body wears (LOOK.md 6.6): the overlay of its armour's model for its frame, as a
/// slot of the character renderer, and the bones the overlay covers; nothing and 0 while
/// it loads, for a frame the bundle has no overlay for, or without a renderer.
pub(super) fn worn_overlay(
    content: &mut Content,
    active: Option<&mut Active>,
    props: &[String],
    look: Look,
    frame: u8,
) -> ([Option<usize>; BODY_PLACES], u32) {
    let mut worn = [None; BODY_PLACES];
    let mut hide = 0;
    let Some(active) = active else {
        return (worn, 0);
    };
    let (gpu, characters) = (&active.gpu, &mut active.renderer.characters);
    for (i, index) in look.worn.iter().enumerate() {
        let Some(key) = props.get(*index as usize) else {
            continue;
        };
        if let Some(slot) =
            content.overlay(key, frame, |model| Some(characters.add_model(gpu, model)))
        {
            worn[i] = Some(slot);
            hide |= content.covers(key);
        }
    }
    (worn, hide)
}

/// The off hand's prop (LOOK.md 6.5), the same way.
pub(super) fn off_prop(
    content: &mut Content,
    active: Option<&mut Active>,
    props: &[String],
    look: Look,
) -> Option<usize> {
    prop_slot(content, active, props, look.off)
}

/// The prop of a template a character wears, on the selector (CLIENT.md 4.2: no zone has
/// sent a prop list yet, the hub's list names the templates): the manifest's model for
/// the template's key, as a slot of the character renderer, when the bundle has it;
/// nothing for an empty place or a template the bundle does not know.
pub(super) fn template_model(content: &Content, template: Option<&str>) -> Option<String> {
    let key = template.filter(|t| !t.is_empty())?;
    content
        .manifest
        .as_ref()?
        .templates
        .iter()
        .find(|t| t.key == key)?
        .model
        .clone()
}

pub(super) fn template_prop(
    content: &mut Content,
    active: Option<&mut Active>,
    template: Option<&str>,
) -> Option<usize> {
    let model = template_model(content, template)?;
    let active = active?;
    let (gpu, characters) = (&active.gpu, &mut active.renderer.characters);
    let slot = content.prop(&model, |m| Some(characters.add_model(gpu, m)))?;
    if content.is_shield(&model) {
        active.avatars.flat.insert(slot);
    }
    Some(slot)
}

/// The overlays of the armour a character wears, on the selector (LOOK.md 6.6,
/// EQUIPMENT.md 5.2): one a body place from the hub's list of what is worn, and the
/// bones they cover between them.
pub(super) fn template_overlay(
    content: &mut Content,
    active: Option<&mut Active>,
    worn: Option<&[String; gm_hub_proto::protocol::PLACES]>,
    frame: u8,
) -> ([Option<usize>; BODY_PLACES], u32) {
    let mut out = [None; BODY_PLACES];
    let mut hide = 0;
    let (Some(worn), Some(active)) = (worn, active) else {
        return (out, 0);
    };
    let (gpu, characters) = (&active.gpu, &mut active.renderer.characters);
    for (i, index) in gm_hub_proto::protocol::BODY_INDICES.iter().enumerate() {
        let Some(model) = template_model(content, Some(worn[*index].as_str())) else {
            continue;
        };
        if let Some(slot) = content.overlay(&model, frame, |m| Some(characters.add_model(gpu, m))) {
            out[i] = Some(slot);
            hide |= content.covers(&model);
        }
    }
    (out, hide)
}

pub(super) fn prop_slot(
    content: &mut Content,
    active: Option<&mut Active>,
    props: &[String],
    index: u16,
) -> Option<usize> {
    let key = props.get(index as usize)?;
    let active = active?;
    let (gpu, characters) = (&active.gpu, &mut active.renderer.characters);
    let slot = content.prop(key, |model| Some(characters.add_model(gpu, model)))?;
    if content.is_shield(key) {
        active.avatars.flat.insert(slot);
    }
    Some(slot)
}

/// Where the view model of a prop sits (LOOK.md 6.4): the `fit_view` of the template
/// whose model it is, as a matrix in model space; identity for a prop no template fits.
pub(super) fn view_fit_of(content: &Content, key: &str) -> Mat4 {
    content
        .manifest
        .as_ref()
        .and_then(|m| {
            m.templates
                .iter()
                .find(|t| t.model.as_deref() == Some(key))
                .and_then(|t| t.fit_view.as_ref())
        })
        .map_or(Mat4::IDENTITY, gm_model::pose::view_fit)
}

/// The prop of the weapon in the own hand in the gun mode (MODES.md 3.7): the ability
/// switched to, by its key in the pack, through the manifest; `None` in another mode or
/// for a hand with nothing. The zone says the same with a `Look`, a round trip later.
pub(super) fn gun_hand_prop(
    content: &Content,
    pack: Option<&ContentPack>,
    c: &gm_net::client::ClientState,
) -> Option<String> {
    let kit = &c.sheet.kit;
    if kit.mode != gm_core::vocab::Mode::Gun {
        return None;
    }
    let slot = c.mover.in_hand(kit)? as usize;
    let def = kit.abilities.get(slot)?.id.0.checked_sub(1)? as usize;
    let key = &pack?.abilities.get(def)?.key;
    content
        .manifest
        .as_ref()?
        .abilities
        .iter()
        .find(|a| &a.key == key)?
        .prop
        .clone()
}

/// How far along the reload of the firearm in the own hand is (MODES.md 3.2): 0 when
/// none is under way, rising to 1 as it ends.
pub(super) fn reload_progress(c: &gm_net::client::ClientState) -> f32 {
    let Some((f, g)) = c.mover.gun_in_hand(&c.sheet.kit) else {
        return kit_progress(c);
    };
    let Some(until) = g.reload_until else {
        return kit_progress(c);
    };
    let left = gm_core::sim::tick_delta(until, c.tick).max(0) as f32;
    (1.0 - left / f.reload.max(1) as f32).clamp(0.0, 1.0)
}

/// How far along an item's use is (MODES.md 11.3), 0 when none: the view model is
/// lowered and worked as for a reload.
pub(super) fn kit_progress(c: &gm_net::client::ClientState) -> f32 {
    c.mover.use_progress(c.tick, c.rate.dt()).unwrap_or(0.0)
}
