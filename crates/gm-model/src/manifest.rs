//! What the content bundle holds (`manifest.gmc`, CONTENT.md 5.2): every entry by its key,
//! with the files, the icon keys and the hashes a client needs to draw the content it meets.
//! Written by `gm-tools content build`, read by the client in one call; `bitcode` because it
//! is the wire's codec and already in the client. A client looks everything up by key and
//! never by position, so a bundle older or newer than a zone's content resolves what it can.

use bitcode::{Decode, Encode};

/// The format's own version: a client refuses a manifest of another.
pub const MANIFEST_VERSION: u32 = 6;
/// The most entries any list may have: a bound before anything is allocated.
pub const MAX_ENTRIES: usize = 4096;

#[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
pub struct Manifest {
    pub format: u32,
    /// The content's own version (`assets/content/VERSION`).
    pub content_version: u32,
    pub templates: Vec<TemplateEntry>,
    pub materials: Vec<MaterialEntry>,
    pub abilities: Vec<AbilityEntry>,
    pub creatures: Vec<CreatureEntry>,
    pub builds: Vec<BuildEntry>,
    pub props: Vec<PropEntry>,
    /// The official avatars (CONTENT.md 3.2), one per frame and armour class at most.
    pub avatars: Vec<AvatarEntry>,
    /// The armour overlays (CONTENT.md 3.3): a template's model per frame it was made for.
    pub overlays: Vec<OverlayEntry>,
    /// The cloth patches (CONTENT.md 3.3, LOOK.md 6.6): a template's model per official
    /// avatar it was baked for.
    pub patches: Vec<PatchEntry>,
    /// Icon keys of the statuses, by status name (`burn`, `chill`, ...).
    pub statuses: Vec<IconEntry>,
    /// Portraits by frame name (`striker`).
    pub portraits: Vec<IconEntry>,
    /// The atlases, one per density (LOOK.md 2.2), thinnest first.
    pub atlases: Vec<AtlasEntry>,
}

/// One atlas of the bundle: `ui.gma` at one texel a dot, `ui2.gma` at two, ...
#[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
pub struct AtlasEntry {
    pub density: u8,
    pub file: String,
    pub sha256: [u8; 32],
    pub bytes: u32,
}

/// Which hand (CONTENT.md 3): `right`, `left`, `back`, `either` (the right, or the left
/// beside another).
pub const HELD_RIGHT: u8 = 0;
pub const HELD_LEFT: u8 = 1;
pub const HELD_BACK: u8 = 2;
pub const HELD_EITHER: u8 = 3;
/// Both hands (EQUIPMENT.md 3): worn, it takes the left hand too.
pub const HELD_TWO: u8 = 4;

#[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
pub struct TemplateEntry {
    pub key: String,
    /// `weapon` or `armour`.
    pub kind: String,
    /// The prop key of its model, when it has one.
    pub model: Option<String>,
    /// The icon's key in the atlas, when it has one (given or baked).
    pub icon: Option<String>,
    pub held: u8,
    /// Where the view model sits (LOOK.md 6.4): move (metres), turn (degrees), scale.
    pub fit_view: Option<[f32; 7]>,
    /// An armour: the standard bones whose part of the body its overlay covers
    /// (CONTENT.md 3.3), a bit per bone; the body hides those under it.
    pub covers: u32,
    /// An armour seen as a patch on the body's atlas (LOOK.md 6.6), not as an overlay:
    /// its model key names `patches`, not `overlays`.
    pub patch: bool,
    pub retired: bool,
}

/// A cloth patch (CONTENT.md 3.3): pixels of one official avatar's atlas, laid over the
/// body's own when the armour is worn, so the dressed body is one draw with one texture.
#[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
pub struct PatchEntry {
    /// The template's model key (`models/armour/<key>_<body>.png` in the sources).
    pub key: String,
    /// `rig::frame_index` of the body.
    pub frame: u8,
    /// The official avatar's key it was baked for (`AvatarEntry.key`): it fits no other.
    pub body: String,
    /// The file under the bundle: `armour/<key>_<body>.gmp`.
    pub file: String,
    pub sha256: [u8; 32],
    pub bytes: u32,
    /// Blocks of the atlas it touches, every level together.
    pub blocks: u32,
}

/// An armour overlay (CONTENT.md 3.3): a skinned piece on the standard rig, made for one
/// frame or for one official avatar, drawn over a body of that frame (or that look) with
/// the body's pose when the armour is worn.
#[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
pub struct OverlayEntry {
    /// The template's model key (`models/armour/<key>_<frame>.glb` in the sources, or
    /// `<key>_<avatar key>.glb` for one cut for an official avatar).
    pub key: String,
    /// `rig::frame_index`.
    pub frame: u8,
    /// The official avatar's key it was cut for (`AvatarEntry.key`, 2026-10-23): a body
    /// wearing that look wears this cut before the frame's; `None` for the frame's cut.
    pub body: Option<String>,
    /// The file under the bundle: `armour/<key>_<frame>.gmm`, or `<key>_<avatar key>.gmm`.
    pub file: String,
    pub sha256: [u8; 32],
    pub bytes: u32,
    pub triangles: u32,
    /// The ETC2 variant beside it, when the model has one.
    pub etc2: Option<Variant>,
}

#[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
pub struct MaterialEntry {
    pub key: String,
    pub layer: String,
    pub icon: Option<String>,
    pub tint: Option<[f32; 3]>,
}

#[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
pub struct AbilityEntry {
    pub key: String,
    pub icon: Option<String>,
    pub prop: Option<String>,
    pub sound: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
pub struct CreatureEntry {
    pub key: String,
    pub model: Option<String>,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
pub struct BuildEntry {
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
pub struct PropEntry {
    pub key: String,
    /// The file under the bundle: `props/<key>.gmm`.
    pub file: String,
    pub sha256: [u8; 32],
    pub bytes: u32,
    pub triangles: u32,
    /// The ETC2 variant beside it (`props/<key>.etc.gmm`), when the model has one.
    pub etc2: Option<Variant>,
}

/// A derived file beside a model's (MODELS.md 5.2): what a client without BC fetches
/// instead, named by the hash of its bytes so that the fetch is checked as any other.
#[derive(Clone, Copy, Debug, Default, PartialEq, Encode, Decode)]
pub struct Variant {
    pub sha256: [u8; 32],
    pub bytes: u32,
}

/// An official avatar: the look of a frame, worn by every character of that frame without
/// a model of its own (MODELS.md 1, CONTENT.md 3.2); or, since 2026-10-23, the look of a
/// body (BODIES.md 3.1), worn by every character of that body the same way, before the
/// frame's.
#[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
pub struct AvatarEntry {
    /// `rig::frame_index`.
    pub frame: u8,
    /// The model's key (`models/avatars/<key>.glb` in the sources).
    pub key: String,
    /// The body's key (`bodies.toml`) when this is a body's look; `None` for a frame's.
    pub body: Option<String>,
    /// The file under the bundle: `avatars/<frame>.gmm`, or `avatars/<body>.gmm`.
    pub file: String,
    pub sha256: [u8; 32],
    pub bytes: u32,
    pub triangles: u32,
    /// The ETC2 variant beside it, when the model has one.
    pub etc2: Option<Variant>,
}

#[derive(Clone, Debug, Default, PartialEq, Encode, Decode)]
pub struct IconEntry {
    pub key: String,
    pub icon: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ManifestError {
    #[error("not a manifest")]
    Decode,
    #[error("manifest format {0}; this client reads {MANIFEST_VERSION}")]
    Version(u32),
    #[error("a list is too long")]
    TooMany,
}

impl Manifest {
    pub fn encode(&self) -> Vec<u8> {
        bitcode::encode(self)
    }

    pub fn decode(bytes: &[u8]) -> Result<Manifest, ManifestError> {
        let m: Manifest = bitcode::decode(bytes).map_err(|_| ManifestError::Decode)?;
        if m.format != MANIFEST_VERSION {
            return Err(ManifestError::Version(m.format));
        }
        if [
            m.templates.len(),
            m.materials.len(),
            m.abilities.len(),
            m.creatures.len(),
            m.builds.len(),
            m.props.len(),
            m.avatars.len(),
            m.overlays.len(),
            m.patches.len(),
            m.statuses.len(),
            m.portraits.len(),
            m.atlases.len(),
        ]
        .iter()
        .any(|n| *n > MAX_ENTRIES)
        {
            return Err(ManifestError::TooMany);
        }
        Ok(m)
    }

    /// The atlas to draw with at `scale` pixels a dot: the one of that density, else the
    /// densest one under it, else the thinnest there is.
    pub fn atlas(&self, scale: u8) -> Option<&AtlasEntry> {
        self.atlases
            .iter()
            .filter(|a| a.density <= scale.max(1))
            .max_by_key(|a| a.density)
            .or_else(|| self.atlases.iter().min_by_key(|a| a.density))
    }

    pub fn template(&self, key: &str) -> Option<&TemplateEntry> {
        self.templates.iter().find(|t| t.key == key)
    }

    pub fn ability(&self, key: &str) -> Option<&AbilityEntry> {
        self.abilities.iter().find(|a| a.key == key)
    }

    pub fn material(&self, key: &str) -> Option<&MaterialEntry> {
        self.materials.iter().find(|m| m.key == key)
    }

    pub fn prop(&self, key: &str) -> Option<&PropEntry> {
        self.props.iter().find(|p| p.key == key)
    }

    /// The official avatar of a frame (`rig::frame_index`), CONTENT.md 3.2: worn by every
    /// body of the frame without a model of its own, whatever it wears (no armour class
    /// since GEAR.md 12, 2026-10-17).
    pub fn avatar(&self, frame: u8) -> Option<&AvatarEntry> {
        self.avatars
            .iter()
            .find(|a| a.body.is_none() && a.frame == frame)
    }

    /// The official avatar a body wears (BODIES.md 3.1, 2026-10-23): its body's look when
    /// the bundle has one, else its frame's.
    pub fn avatar_of(&self, body: Option<&str>, frame: u8) -> Option<&AvatarEntry> {
        body.and_then(|b| self.avatars.iter().find(|a| a.body.as_deref() == Some(b)))
            .or_else(|| self.avatar(frame))
    }

    /// An official avatar by its model key (`AvatarEntry.key`).
    pub fn avatar_by_key(&self, key: &str) -> Option<&AvatarEntry> {
        self.avatars.iter().find(|a| a.key == key)
    }

    /// The overlay of an armour's model for a frame (`rig::frame_index`): the frame's cut.
    pub fn overlay(&self, key: &str, frame: u8) -> Option<&OverlayEntry> {
        self.overlays
            .iter()
            .find(|o| o.key == key && o.body.is_none() && o.frame == frame)
    }

    /// The overlay of an armour's model on a body wearing an official avatar (its key):
    /// the cut for that look when the bundle has one, else the frame's.
    pub fn overlay_of(&self, key: &str, body: Option<&str>, frame: u8) -> Option<&OverlayEntry> {
        body.and_then(|b| {
            self.overlays
                .iter()
                .find(|o| o.key == key && o.body.as_deref() == Some(b))
        })
        .or_else(|| self.overlay(key, frame))
    }

    /// The patch of an armour's model for an official avatar (`AvatarEntry.key`).
    pub fn patch(&self, key: &str, body: &str) -> Option<&PatchEntry> {
        self.patches.iter().find(|p| p.key == key && p.body == body)
    }

    pub fn creature(&self, key: &str) -> Option<&CreatureEntry> {
        self.creatures.iter().find(|c| c.key == key)
    }

    pub fn portrait(&self, key: &str) -> Option<&str> {
        self.portraits
            .iter()
            .find(|p| p.key == key)
            .map(|p| p.icon.as_str())
    }

    pub fn status_icon(&self, key: &str) -> Option<&str> {
        self.statuses
            .iter()
            .find(|p| p.key == key)
            .map(|p| p.icon.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_the_version_is_checked() {
        let m = Manifest {
            format: MANIFEST_VERSION,
            content_version: 3,
            templates: vec![TemplateEntry {
                key: "sword".into(),
                kind: "weapon".into(),
                model: Some("sword_iron".into()),
                icon: Some("sword".into()),
                held: HELD_RIGHT,
                fit_view: None,
                covers: 0,
                patch: false,
                retired: false,
            }],
            patches: vec![PatchEntry {
                key: "trousers".into(),
                frame: 1,
                body: "striker_mail".into(),
                file: "armour/trousers_striker_mail.gmp".into(),
                sha256: [4; 32],
                bytes: 40_000,
                blocks: 3000,
            }],
            overlays: vec![OverlayEntry {
                key: "cuirass".into(),
                frame: 1,
                body: None,
                file: "armour/cuirass_striker.gmm".into(),
                sha256: [3; 32],
                bytes: 90_000,
                triangles: 1200,
                etc2: None,
            }],
            props: vec![PropEntry {
                key: "sword_iron".into(),
                file: "props/sword_iron.gmm".into(),
                sha256: [7; 32],
                bytes: 1234,
                triangles: 48,
                etc2: None,
            }],
            avatars: vec![AvatarEntry {
                frame: 1,
                key: "striker_mail".into(),
                body: None,
                file: "avatars/striker.gmm".into(),
                sha256: [9; 32],
                bytes: 415_970,
                triangles: 2946,
                etc2: None,
            }],
            ..Default::default()
        };
        let bytes = m.encode();
        assert_eq!(Manifest::decode(&bytes).unwrap(), m);
        assert_eq!(
            m.template("sword").unwrap().model.as_deref(),
            Some("sword_iron")
        );
        assert!(m.template("axe").is_none());
        assert_eq!(
            m.avatar(1).map(|a| a.file.as_str()),
            Some("avatars/striker.gmm")
        );
        assert!(m.avatar(2).is_none());
        assert_eq!(
            m.overlay("cuirass", 1).map(|o| o.file.as_str()),
            Some("armour/cuirass_striker.gmm")
        );
        assert!(m.overlay("cuirass", 0).is_none() && m.overlay("robe", 1).is_none());
        assert_eq!(
            m.patch("trousers", "striker_mail").map(|p| p.file.as_str()),
            Some("armour/trousers_striker_mail.gmp")
        );
        assert!(m.patch("trousers", "caster_cloth").is_none());
        let old = Manifest {
            format: 0,
            ..m.clone()
        };
        assert_eq!(
            Manifest::decode(&old.encode()),
            Err(ManifestError::Version(0))
        );
        assert_eq!(Manifest::decode(b"junk"), Err(ManifestError::Decode));
    }
}
