//! The content bundle as the client has it (CONTENT.md 6): the manifest and the atlas read
//! at start from `<assets>/built/content/`, props loaded by key when first needed, from the
//! bundle and never from the hub. Everything is looked up by key; what the bundle lacks is
//! drawn as nothing (principle 4: never wait for art), and a bundle that is missing or
//! broken leaves the client drawing exactly as it did before Phase 14.

use std::collections::HashMap;

use gm_model::atlas::Atlas;
use gm_model::format::TextureFormat;
use gm_model::manifest::{Manifest, Variant};
use gm_model::{Model, Patch};

/// Where the bundle's files are, under the assets: `<assets>/built/content`.
pub const DIR: &str = "built/content";
/// What an official avatar's fetch is filed under in the browser's inbox, before
/// `<frame>_<armour>`.
#[cfg(target_arch = "wasm32")]
const AVATAR_PREFIX: &str = "avatar:";
/// And an armour overlay's, before `<frame>:<key>`.
#[cfg(target_arch = "wasm32")]
const OVERLAY_PREFIX: &str = "overlay:";
/// And a cloth patch's, before `<body>:<key>`.
#[cfg(target_arch = "wasm32")]
const PATCH_PREFIX: &str = "patch:";
/// Where the browser's fetches land, by key: the file's bytes, or `None` for a 404.
#[cfg(target_arch = "wasm32")]
type Inbox<K> = std::rc::Rc<std::cell::RefCell<Vec<(K, Option<Vec<u8>>)>>>;

/// A prop as the client has it.
#[derive(Debug)]
pub enum PropState {
    /// Asked for and not yet here (a fetch in flight in the browser).
    #[allow(dead_code)]
    Loading,
    /// On the GPU, in this slot of the character renderer.
    Loaded(usize),
    /// The bundle has no such file, or the file is not a prop: said once, never asked again.
    Missing,
}

pub struct Content {
    pub manifest: Option<Manifest>,
    /// An atlas that arrived, until the HUD takes it (`take_atlas`).
    atlas: Option<Atlas>,
    /// The density of the atlas last asked for (`want_atlas`): 0 before any.
    atlas_asked: u8,
    /// Atlases the browser fetched, by density, until `want_atlas` reads them: every one
    /// that landed since the last frame (two may, when the scale changed twice while the
    /// page was hidden, and the one asked for last must not be lost to the other).
    #[cfg(target_arch = "wasm32")]
    atlas_inbox: Inbox<u8>,
    pub props: HashMap<String, PropState>,
    /// The official avatars by frame and armour class (CONTENT.md 3.2): `None` until
    /// asked for, then as a prop.
    pub avatars: [[Option<PropState>; 4]; 4],
    /// The armour overlays (CONTENT.md 3.3) by model key and frame, as props.
    pub overlays: HashMap<(String, u8), PropState>,
    /// The cloth patches (CONTENT.md 3.3, LOOK.md 6.6) by model key and the official
    /// avatar's key they were baked for, the same way; the slot is the avatars' registry's.
    pub patches: HashMap<(String, String), PropState>,
    /// Where props are read from: a directory natively, a URL prefix in the browser.
    base: String,
    /// Files that arrived (the browser's fetches land here; natively a read is immediate).
    #[cfg(target_arch = "wasm32")]
    inbox: Inbox<String>,
    /// What was said about the bundle, for the report.
    pub note: String,
    /// The atlas encoding this client fetches (MODELS.md 5.2): the manifest names a
    /// variant beside a model, and `prefer` says which is read.
    wanted: TextureFormat,
}

impl Content {
    /// A client without a bundle.
    pub fn none(why: &str) -> Content {
        Content {
            manifest: None,
            atlas: None,
            atlas_asked: 0,
            #[cfg(target_arch = "wasm32")]
            atlas_inbox: Default::default(),
            props: HashMap::new(),
            avatars: Default::default(),
            overlays: HashMap::new(),
            patches: HashMap::new(),
            base: String::new(),
            #[cfg(target_arch = "wasm32")]
            inbox: Default::default(),
            note: why.to_string(),
            wanted: TextureFormat::Bc1Srgb,
        }
    }

    /// Fetch the ETC2 variants (or the BC1 files) from here on: what the GPU samples.
    pub fn prefer(&mut self, wanted: TextureFormat) {
        self.wanted = wanted;
    }

    /// The file to read for a model and the hash it must have: the variant in the wanted
    /// format when the manifest names one, the original otherwise.
    fn pick(&self, file: &str, sha: [u8; 32], etc2: Option<Variant>) -> (String, [u8; 32]) {
        match (self.wanted, etc2) {
            (TextureFormat::Etc2Srgb, Some(v)) => (self.wanted.file_of(file), v.sha256),
            _ => (file.to_string(), sha),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn pick_sha(&self, sha: [u8; 32], etc2: Option<Variant>) -> [u8; 32] {
        match (self.wanted, etc2) {
            (TextureFormat::Etc2Srgb, Some(v)) => v.sha256,
            _ => sha,
        }
    }

    /// From the bundle's two files (read by whoever has them) and where its props are.
    pub fn from_bytes(base: String, manifest: Option<Vec<u8>>, atlas: Option<Vec<u8>>) -> Content {
        let mut c = Content::none("");
        c.base = base;
        let mut notes = Vec::new();
        match manifest.as_deref().map(Manifest::decode) {
            Some(Ok(m)) => {
                notes.push(format!(
                    "content version {} ({} templates, {} props, {} abilities)",
                    m.content_version,
                    m.templates.len(),
                    m.props.len(),
                    m.abilities.len()
                ));
                c.manifest = Some(m);
            }
            Some(Err(e)) => {
                log::error!("the content manifest: {e}");
                notes.push(format!("manifest refused: {e}"));
            }
            None => notes.push("no content bundle".into()),
        }
        match atlas.as_deref().map(Atlas::decode) {
            Some(Ok(a)) => {
                if let Some(m) = &c.manifest
                    && m.atlases
                        .iter()
                        .all(|e| e.sha256 != gm_model::model_id(atlas.as_deref().unwrap_or(&[])))
                {
                    log::warn!("the atlas is not one the manifest names");
                }
                notes.push(format!(
                    "atlas {}x{} ({} pieces, {} faces, {} icons)",
                    a.w,
                    a.h,
                    a.pieces.len(),
                    a.faces.len(),
                    a.icons.len()
                ));
                c.atlas_asked = a.density;
                c.atlas = Some(a);
            }
            Some(Err(e)) => {
                log::error!("the content atlas: {e}");
                notes.push(format!("atlas refused: {e}"));
            }
            None => {}
        }
        c.note = notes.join("; ");
        c
    }

    /// Read the bundle from `<assets>/built/content` on the desktop.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(assets: &std::path::Path) -> Content {
        let dir = assets.join(DIR);
        let read = |name: &str| std::fs::read(dir.join(name)).ok();
        let c = Content::from_bytes(
            dir.to_string_lossy().into_owned(),
            read("manifest.gmc"),
            read("ui.gma"),
        );
        log::info!("content: {}", c.note);
        c
    }

    /// Fetch the bundle from `<assets>/built/content` on the page; a site without one
    /// (404) is a client without a bundle, not an error.
    #[cfg(target_arch = "wasm32")]
    pub async fn fetch(assets: &str) -> Content {
        let base = format!("{assets}/{DIR}");
        let get = |name: &str| {
            let url = format!("{base}/{name}");
            async move {
                match crate::web::fetch_bytes(&url).await {
                    Ok(bytes) => bytes,
                    Err(e) => {
                        log::warn!("{e}");
                        None
                    }
                }
            }
        };
        let manifest = get("manifest.gmc").await;
        let atlas = get("ui.gma").await;
        let c = Content::from_bytes(base, manifest, atlas);
        log::info!("content: {}", c.note);
        c
    }

    /// An atlas that arrived, once: the HUD uploads it.
    pub fn take_atlas(&mut self) -> Option<Atlas> {
        self.atlas.take()
    }

    /// Ask for the atlas to draw with at `scale` pixels a dot (LOOK.md 2.2): the bundle's
    /// atlas of that density, or the nearest thinner one it has. Called every frame; a
    /// change of scale reads (in the browser: fetches) the other file once, and until it
    /// is here the HUD goes on drawing with the atlas it has. A file that is missing or
    /// refused is said once and what is in use stays.
    pub fn want_atlas(&mut self, scale: u8) {
        #[cfg(target_arch = "wasm32")]
        {
            let arrived = std::mem::take(&mut *self.atlas_inbox.borrow_mut());
            for (density, bytes) in arrived {
                match bytes {
                    Some(bytes) => self.atlas_arrived(density, &bytes),
                    None => log::warn!("the atlas at {density} texels a dot: not on the site"),
                }
            }
        }
        let Some(entry) = self.manifest.as_ref().and_then(|m| m.atlas(scale)) else {
            return;
        };
        if entry.density == self.atlas_asked {
            return;
        }
        let (density, file) = (entry.density, entry.file.clone());
        self.atlas_asked = density;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = std::path::Path::new(&self.base).join(&file);
            match std::fs::read(&path) {
                Ok(bytes) => self.atlas_arrived(density, &bytes),
                Err(e) => log::warn!("the atlas {}: {e}", path.display()),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            let url = format!("{}/{file}", self.base);
            let inbox = self.atlas_inbox.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let bytes = match crate::web::fetch_bytes(&url).await {
                    Ok(b) => b,
                    Err(e) => {
                        log::warn!("{e}");
                        None
                    }
                };
                inbox.borrow_mut().push((density, bytes));
            });
        }
    }

    /// An atlas's bytes are here: checked against the manifest, decoded, kept for the HUD
    /// unless another density has been asked for since.
    fn atlas_arrived(&mut self, density: u8, bytes: &[u8]) {
        let named = self
            .manifest
            .as_ref()
            .and_then(|m| m.atlases.iter().find(|e| e.density == density))
            .is_some_and(|e| e.sha256 == gm_model::model_id(bytes));
        if !named {
            log::warn!("the atlas at {density} texels a dot is not the one the manifest names");
            return;
        }
        match Atlas::decode(bytes) {
            Ok(a) if a.density == density => {
                log::info!("atlas {}x{} at {density} texels a dot", a.w, a.h);
                if density == self.atlas_asked {
                    self.atlas = Some(a);
                }
            }
            Ok(a) => log::warn!(
                "the atlas file of density {density} says it is of density {}",
                a.density
            ),
            Err(e) => log::error!("the atlas at {density} texels a dot: {e}"),
        }
    }

    /// Ask for a prop by key: `Some(slot)` when it is on the GPU. The first ask starts the
    /// load; a prop the bundle has not is `None` for good. `load` puts a decoded model on
    /// the GPU and returns its slot.
    pub fn prop(
        &mut self,
        key: &str,
        mut load: impl FnMut(&Model) -> Option<usize>,
    ) -> Option<usize> {
        if let Some(state) = self.props.get(key) {
            return match state {
                PropState::Loaded(slot) => Some(*slot),
                _ => None,
            };
        }
        let Some(entry) = self.manifest.as_ref().and_then(|m| m.prop(key)) else {
            self.props.insert(key.to_string(), PropState::Missing);
            return None;
        };
        let (file, sha) = self.pick(&entry.file, entry.sha256, entry.etc2);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = std::path::Path::new(&self.base).join(&file);
            let state = match std::fs::read(&path) {
                Ok(bytes) => Self::decoded(key, &bytes, sha, &mut load),
                Err(e) => {
                    log::warn!("prop `{key}`: {}: {e}", path.display());
                    PropState::Missing
                }
            };
            let out = match &state {
                PropState::Loaded(slot) => Some(*slot),
                _ => None,
            };
            self.props.insert(key.to_string(), state);
            out
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (sha, &mut load);
            let url = format!("{}/{file}", self.base);
            let inbox = self.inbox.clone();
            let key_owned = key.to_string();
            wasm_bindgen_futures::spawn_local(async move {
                let bytes = match crate::web::fetch_bytes(&url).await {
                    Ok(b) => b,
                    Err(e) => {
                        log::warn!("{e}");
                        None
                    }
                };
                inbox.borrow_mut().push((key_owned, bytes));
            });
            self.props.insert(key.to_string(), PropState::Loading);
            None
        }
    }

    /// The official avatar of a frame (`rig::frame_index`) in an armour class (its index),
    /// CONTENT.md 3.2, by the same road as a prop: `Some(slot)` when it is on the GPU, the
    /// first ask starts the load, and `None` for good when the bundle has none. A class
    /// without a row of its own wears the frame's nearest (`Manifest::avatar_near`): the
    /// ask is filed under that row's own pair, so the body is loaded once, not once a
    /// class.
    pub fn avatar(
        &mut self,
        frame: u8,
        armour: u8,
        mut load: impl FnMut(&Model) -> Option<usize>,
    ) -> Option<usize> {
        if frame >= 4 || armour >= 4 {
            return None;
        }
        let (frame, armour) = self
            .manifest
            .as_ref()
            .and_then(|m| m.avatar_near(frame, armour))
            .map_or((frame, armour), |e| (e.frame, e.armour));
        let (f, a) = (frame as usize, armour as usize);
        if f >= 4 || a >= 4 {
            return None;
        }
        if let Some(state) = &self.avatars[f][a] {
            return match state {
                PropState::Loaded(slot) => Some(*slot),
                _ => None,
            };
        }
        let Some(entry) = self.manifest.as_ref().and_then(|m| m.avatar(frame, armour)) else {
            self.avatars[f][a] = Some(PropState::Missing);
            return None;
        };
        let (file, sha) = self.pick(&entry.file, entry.sha256, entry.etc2);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = std::path::Path::new(&self.base).join(&file);
            let state = match std::fs::read(&path) {
                Ok(bytes) => Self::decoded_body(frame, armour, &bytes, sha, &mut load),
                Err(e) => {
                    log::warn!("avatar {frame}/{armour}: {}: {e}", path.display());
                    PropState::Missing
                }
            };
            let out = match &state {
                PropState::Loaded(slot) => Some(*slot),
                _ => None,
            };
            self.avatars[f][a] = Some(state);
            out
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (sha, &mut load);
            let url = format!("{}/{file}", self.base);
            let inbox = self.inbox.clone();
            let key = format!("{AVATAR_PREFIX}{frame}_{armour}");
            wasm_bindgen_futures::spawn_local(async move {
                let bytes = match crate::web::fetch_bytes(&url).await {
                    Ok(b) => b,
                    Err(e) => {
                        log::warn!("{e}");
                        None
                    }
                };
                inbox.borrow_mut().push((key, bytes));
            });
            self.avatars[f][a] = Some(PropState::Loading);
            None
        }
    }

    /// The overlay of an armour's model for a frame (CONTENT.md 3.3), by the same road as
    /// a prop: `Some(slot)` when it is on the GPU, the first ask starts the load, and
    /// `None` for good when the bundle has none for that frame.
    pub fn overlay(
        &mut self,
        key: &str,
        frame: u8,
        mut load: impl FnMut(&Model) -> Option<usize>,
    ) -> Option<usize> {
        let at = (key.to_string(), frame);
        if let Some(state) = self.overlays.get(&at) {
            return match state {
                PropState::Loaded(slot) => Some(*slot),
                _ => None,
            };
        }
        let Some(entry) = self.manifest.as_ref().and_then(|m| m.overlay(key, frame)) else {
            self.overlays.insert(at, PropState::Missing);
            return None;
        };
        let (file, sha) = self.pick(&entry.file, entry.sha256, entry.etc2);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = std::path::Path::new(&self.base).join(&file);
            let state = match std::fs::read(&path) {
                Ok(bytes) => Self::decoded_overlay(key, frame, &bytes, sha, &mut load),
                Err(e) => {
                    log::warn!("overlay `{key}` for frame {frame}: {}: {e}", path.display());
                    PropState::Missing
                }
            };
            let out = match &state {
                PropState::Loaded(slot) => Some(*slot),
                _ => None,
            };
            self.overlays.insert(at, state);
            out
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (sha, &mut load);
            let url = format!("{}/{file}", self.base);
            let inbox = self.inbox.clone();
            let filed = format!("{OVERLAY_PREFIX}{frame}:{key}");
            wasm_bindgen_futures::spawn_local(async move {
                let bytes = match crate::web::fetch_bytes(&url).await {
                    Ok(b) => b,
                    Err(e) => {
                        log::warn!("{e}");
                        None
                    }
                };
                inbox.borrow_mut().push((filed, bytes));
            });
            self.overlays.insert(at, PropState::Loading);
            None
        }
    }

    /// Whether an armour of this model key is seen as a patch on the body's atlas
    /// (CONTENT.md 3.3), by the manifest's template of that model.
    pub fn is_patch(&self, key: &str) -> bool {
        self.manifest.as_ref().is_some_and(|m| {
            m.templates
                .iter()
                .any(|t| t.model.as_deref() == Some(key) && t.patch)
        })
    }

    /// The official avatar's key of a frame in an armour class (CONTENT.md 3.2), when the
    /// bundle has one for the frame (the class's row, else the frame's nearest, as
    /// `avatar` wears it): the body a patch must have been baked for.
    pub fn official_key(&self, frame: u8, armour: u8) -> Option<String> {
        self.manifest
            .as_ref()?
            .avatar_near(frame, armour)
            .map(|a| a.key.clone())
    }

    /// The patch of an armour's model for the official avatar of a frame in an armour
    /// class (LOOK.md 6.6), by the same road as an overlay: `Some(id)` when the avatars'
    /// registry has it (`load` puts a decoded patch there and returns its id), the first
    /// ask starts the load, and `None` for good when the bundle has none for that body.
    pub fn patch(
        &mut self,
        key: &str,
        frame: u8,
        armour: u8,
        mut load: impl FnMut(&Patch) -> Option<usize>,
    ) -> Option<usize> {
        let body = self.official_key(frame, armour)?;
        let at = (key.to_string(), body.clone());
        if let Some(state) = self.patches.get(&at) {
            return match state {
                PropState::Loaded(slot) => Some(*slot),
                _ => None,
            };
        }
        let Some(entry) = self.manifest.as_ref().and_then(|m| m.patch(key, &body)) else {
            self.patches.insert(at, PropState::Missing);
            return None;
        };
        let (file, sha) = (entry.file.clone(), entry.sha256);
        #[cfg(not(target_arch = "wasm32"))]
        {
            let path = std::path::Path::new(&self.base).join(&file);
            let state = match std::fs::read(&path) {
                Ok(bytes) => Self::decoded_patch(key, &body, &bytes, sha, &mut load),
                Err(e) => {
                    log::warn!("patch `{key}` for {body}: {}: {e}", path.display());
                    PropState::Missing
                }
            };
            let out = match &state {
                PropState::Loaded(slot) => Some(*slot),
                _ => None,
            };
            self.patches.insert(at, state);
            out
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (sha, &mut load);
            let url = format!("{}/{file}", self.base);
            let inbox = self.inbox.clone();
            let filed = format!("{PATCH_PREFIX}{body}:{key}");
            wasm_bindgen_futures::spawn_local(async move {
                let bytes = match crate::web::fetch_bytes(&url).await {
                    Ok(b) => b,
                    Err(e) => {
                        log::warn!("{e}");
                        None
                    }
                };
                inbox.borrow_mut().push((filed, bytes));
            });
            self.patches.insert(at, PropState::Loading);
            None
        }
    }

    /// A patch's bytes: the file the manifest names, a patch baked for that body.
    fn decoded_patch(
        key: &str,
        body: &str,
        bytes: &[u8],
        sha: [u8; 32],
        load: &mut impl FnMut(&Patch) -> Option<usize>,
    ) -> PropState {
        if gm_model::model_id(bytes) != sha {
            log::warn!("patch `{key}`: the file is not the one the manifest names");
            return PropState::Missing;
        }
        match Patch::decode(bytes) {
            Ok(p) if p.body == body => match load(&p) {
                Some(slot) => {
                    log::info!(
                        "patch `{key}` for {body}: {} blocks, {:.1}% of the atlas",
                        p.blocks(),
                        p.coverage() * 100.0
                    );
                    PropState::Loaded(slot)
                }
                None => PropState::Missing,
            },
            Ok(_) => {
                log::warn!("patch `{key}`: the file was baked for another body than {body}");
                PropState::Missing
            }
            Err(e) => {
                log::warn!("patch `{key}`: {e}");
                PropState::Missing
            }
        }
    }

    /// What an armour's overlay covers (CONTENT.md 3.3): the standard bones whose part of
    /// the body is hidden under it, from the template whose model it is; nothing for a
    /// key no template names.
    /// Whether the prop of this key is a shield's (LOOK.md 6.7: sheathed, it hangs flat
    /// on the back), by the manifest's template of that model.
    pub fn is_shield(&self, key: &str) -> bool {
        self.manifest.as_ref().is_some_and(|m| {
            m.templates
                .iter()
                .any(|t| t.model.as_deref() == Some(key) && t.kind == "shield")
        })
    }

    pub fn covers(&self, key: &str) -> u32 {
        self.manifest
            .as_ref()
            .and_then(|m| m.templates.iter().find(|t| t.model.as_deref() == Some(key)))
            .map_or(0, |t| t.covers)
    }

    /// Fetches that landed since the last frame, decoded and put on the GPU (one a frame,
    /// like avatars, WEB.md 4).
    #[cfg(target_arch = "wasm32")]
    pub fn poll(
        &mut self,
        mut load: impl FnMut(&Model) -> Option<usize>,
        mut load_patch: impl FnMut(&Patch) -> Option<usize>,
    ) {
        let arrived = {
            let mut inbox = self.inbox.borrow_mut();
            if inbox.is_empty() {
                return;
            }
            inbox.remove(0)
        };
        let (key, bytes) = arrived;
        if let Some(rest) = key.strip_prefix(PATCH_PREFIX) {
            let Some((body, key)) = rest.split_once(':') else {
                return;
            };
            let (body, key) = (body.to_string(), key.to_string());
            let sha = self
                .manifest
                .as_ref()
                .and_then(|m| m.patch(&key, &body))
                .map(|e| e.sha256)
                .unwrap_or([0; 32]);
            let state = match bytes {
                Some(bytes) => Self::decoded_patch(&key, &body, &bytes, sha, &mut load_patch),
                None => {
                    log::warn!("patch `{key}` for {body}: not on the site");
                    PropState::Missing
                }
            };
            self.patches.insert((key, body), state);
            return;
        }
        if let Some(rest) = key.strip_prefix(OVERLAY_PREFIX) {
            let (frame, key) = match rest.split_once(':') {
                Some((f, k)) => (f.parse::<u8>().unwrap_or(255), k.to_string()),
                None => return,
            };
            let sha = self
                .manifest
                .as_ref()
                .and_then(|m| m.overlay(&key, frame))
                .map(|e| self.pick_sha(e.sha256, e.etc2))
                .unwrap_or([0; 32]);
            let state = match bytes {
                Some(bytes) => Self::decoded_overlay(&key, frame, &bytes, sha, &mut load),
                None => {
                    log::warn!("overlay `{key}` for frame {frame}: not on the site");
                    PropState::Missing
                }
            };
            self.overlays.insert((key, frame), state);
            return;
        }
        if let Some(rest) = key.strip_prefix(AVATAR_PREFIX) {
            let mut it = rest.split('_').map(|s| s.parse::<u8>().unwrap_or(255));
            let (frame, armour) = (it.next().unwrap_or(255), it.next().unwrap_or(255));
            let (f, a) = (frame as usize, armour as usize);
            if f >= 4 || a >= 4 {
                return;
            }
            let sha = self
                .manifest
                .as_ref()
                .and_then(|m| m.avatar(frame, armour))
                .map(|e| self.pick_sha(e.sha256, e.etc2))
                .unwrap_or([0; 32]);
            let state = match bytes {
                Some(bytes) => Self::decoded_body(frame, armour, &bytes, sha, &mut load),
                None => {
                    log::warn!("avatar {frame}/{armour}: not on the site");
                    PropState::Missing
                }
            };
            self.avatars[f][a] = Some(state);
            return;
        }
        let sha = self
            .manifest
            .as_ref()
            .and_then(|m| m.prop(&key))
            .map(|p| self.pick_sha(p.sha256, p.etc2))
            .unwrap_or([0; 32]);
        let state = match bytes {
            Some(bytes) => Self::decoded(&key, &bytes, sha, &mut load),
            None => {
                log::warn!("prop `{key}`: not on the site");
                PropState::Missing
            }
        };
        self.props.insert(key, state);
    }

    /// An overlay's bytes: the file the manifest names, an overlay made for that frame.
    fn decoded_overlay(
        key: &str,
        frame: u8,
        bytes: &[u8],
        sha: [u8; 32],
        load: &mut impl FnMut(&Model) -> Option<usize>,
    ) -> PropState {
        if gm_model::model_id(bytes) != sha {
            log::warn!("overlay `{key}`: the file is not the one the manifest names");
            return PropState::Missing;
        }
        match Model::decode(bytes) {
            Ok(m) if m.is_overlay() && m.frame == frame => match load(&m) {
                Some(slot) => {
                    log::info!(
                        "overlay `{key}` for frame {frame}: {} triangles, {}x{} atlas",
                        m.triangles(),
                        m.tex_w,
                        m.tex_h
                    );
                    PropState::Loaded(slot)
                }
                None => PropState::Missing,
            },
            Ok(_) => {
                log::warn!("overlay `{key}`: the file is not an overlay for frame {frame}");
                PropState::Missing
            }
            Err(e) => {
                log::warn!("overlay `{key}`: {e}");
                PropState::Missing
            }
        }
    }

    /// An official avatar's bytes: the file the manifest names, a body of that frame.
    fn decoded_body(
        frame: u8,
        armour: u8,
        bytes: &[u8],
        sha: [u8; 32],
        load: &mut impl FnMut(&Model) -> Option<usize>,
    ) -> PropState {
        if gm_model::model_id(bytes) != sha {
            log::warn!("avatar {frame}/{armour}: the file is not the one the manifest names");
            return PropState::Missing;
        }
        match Model::decode(bytes) {
            Ok(m) if !m.is_prop() && m.frame == frame => match load(&m) {
                Some(slot) => {
                    log::info!(
                        "avatar {frame}/{armour}: {} triangles, {}x{} atlas",
                        m.triangles(),
                        m.tex_w,
                        m.tex_h
                    );
                    PropState::Loaded(slot)
                }
                None => PropState::Missing,
            },
            Ok(_) => {
                log::warn!("avatar {frame}/{armour}: the file is not a body of that frame");
                PropState::Missing
            }
            Err(e) => {
                log::warn!("avatar {frame}/{armour}: {e}");
                PropState::Missing
            }
        }
    }

    fn decoded(
        key: &str,
        bytes: &[u8],
        sha: [u8; 32],
        load: &mut impl FnMut(&Model) -> Option<usize>,
    ) -> PropState {
        if gm_model::model_id(bytes) != sha {
            log::warn!("prop `{key}`: the file is not the one the manifest names");
            return PropState::Missing;
        }
        match Model::decode(bytes) {
            Ok(m) if m.is_prop() => match load(&m) {
                Some(slot) => PropState::Loaded(slot),
                None => PropState::Missing,
            },
            Ok(_) => {
                log::warn!("prop `{key}`: the file is a body, not a prop");
                PropState::Missing
            }
            Err(e) => {
                log::warn!("prop `{key}`: {e}");
                PropState::Missing
            }
        }
    }
}
