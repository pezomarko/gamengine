//! Windowed client: winit event loop, fixed 64 Hz simulation with interpolated rendering,
//! mouse look, PVS-culled drawing, optional benchmark mode. Offline it runs the local
//! simulation; with `--connect` it predicts the own entity, reconciles against the zone and
//! interpolates everyone else (PROTOCOL.md 7). Two viewports share the simulation
//! (VOCABULARY.md 9): first person aims from the eyes, third person aims the camera ray at a
//! world point and re-aims it from the eyes ("camera-to-muzzle re-aim").

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use web_time::Instant;

use glam::{Mat4, Vec2, Vec3};
use gm_bsp::Bsp;
use gm_core::build::{ContentPack, Sheet};
use gm_core::collide::{Aabb, sweep_boxes};
use gm_core::movement::{MoveInput, MoveVars, PlayerState, player_move, yaw_vectors};
use gm_core::sim::{Input as SimInput, buttons, view_dir};
use gm_core::tick::TickRate;
use gm_core::trace::{CollisionWorld, Hull};
use gm_hub_proto::names;
use gm_hub_proto::player::PlayerRequest;
use gm_model::ModelId;
use gm_net::client::ClientState;
use gm_net::control::{
    BODY_PLACES, BodyKind, BuildChoice, EncounterState, FromClient, FromZone, GmNews, Look, Order,
    SquadEntry, StallEntry,
};
use gm_net::snapshot::{EntityKind, SpawnInfo};
use gm_net::transport::fnv1a64;
use winit::application::ApplicationHandler;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
#[cfg(not(target_arch = "wasm32"))]
use winit::window::CursorGrabMode;
use winit::window::{CursorIcon, Window, WindowId};

use crate::avatars::{Avatars, Body, DOLL, OWN, stall_boxes, stall_keeper};
use crate::bag::{Bag, BagAction};
use crate::character::{CharacterAction, CharacterPage, CharacterView};
use crate::characters::CharacterDraw;
use crate::content::Content;
use crate::front::{Action, Auto, Front, PANEL_UNITS};
use crate::gm::{GmAction, GmPage, GmView};
use crate::hub::{Account, Hub, HubApi, ticket_addr};
use crate::hud::{self, Hud};
use crate::menu::{Chat, GameMenu, MenuAction, Offers, Said};
use crate::net::{NetClient, NetEvent, ZoneAddr};
use crate::people::{Here, People, PeopleAction, Social};
use crate::render::{EntityDraw, Gpu, Renderer, view_proj};
use crate::script::UiScript;
use crate::settings::Settings;
use crate::stats::FrameStats;
#[cfg(not(target_arch = "wasm32"))]
use crate::stats::{print_bench, print_bench_avatars};
use crate::touch::{self, Button as TouchButton, Event as TouchEvent, Fingers, Zone};
use crate::ui::{self, Key, Ui, UiInput, UiState};
use crate::world::{self, WorldMesh};
use crate::{Error, Options};
use gm_net::control::TRAINER_REACH;

mod frame;
mod hud_view;
mod input;
mod looks;
mod online;
mod report;
mod screens;
mod start;
mod view;
mod window;

use frame::*;
use hud_view::*;
pub(crate) use input::BAR_KEYS;
use input::*;
use looks::*;
use online::*;
#[cfg(target_arch = "wasm32")]
use start::fetch_map;
#[cfg(target_arch = "wasm32")]
pub use start::run_web;
#[cfg(not(target_arch = "wasm32"))]
pub use start::{open_replay, run};
use view::*;
pub(crate) use view::{Sim, third_person_camera};
use window::*;

const MAX_STEPS_PER_FRAME: u32 = 8;

/// Behind a screen with no zone being played, the camera turns this fast (CLIENT.md 2).
const BACKDROP_DEG_PER_S: f32 = 4.0;

/// How far the selector's camera stands from the body it shows whole (CLIENT.md 4.2).
const DOLL_OPEN_DISTANCE: f32 = 46.0;

/// While a zone is played the hub is asked something this often, so that the session is
/// still there when the zone is left (it ends after a day of silence).
const SESSION_TOUCH_SECS: u64 = 600;

/// Two presses this close in time and place are a double click.
const DOUBLE_CLICK_SECS: f32 = 0.4;

const DOUBLE_CLICK_PIXELS: f32 = 6.0;

const BENCH_YAW_DEG_PER_S: f32 = 20.0;

/// With a crowd the bench camera swings across it instead of turning away from it.
const BENCH_CROWD_SWING_DEG: f32 = 22.0;

/// Third-person camera: behind, slightly right and above the eyes (VOCABULARY.md 9).
/// A blow within this many seconds of the last keeps the combo counter going (MODES.md 4.6).
const COMBO_SECS: f32 = 2.0;

/// The recoil's punch on the view falls to a third in this long (MODES.md 3.3).
const PUNCH_DECAY_SECS: f32 = 0.06;

const CAMERA_BACK: f32 = 110.0;

const CAMERA_RIGHT: f32 = 24.0;

const CAMERA_UP: f32 = 12.0;

/// How far the camera ray is resolved for the re-aim.
const AIM_REACH: f32 = 4096.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Viewport {
    First,
    Third,
}

/// The view (CLIENT.md 2): the viewport, the camera's aim, the view model's motion
/// (LOOK.md 6.4), the recoil's punch (MODES.md 3.3), the scope's zoom, the crouch's
/// drop, and the yaw each body was drawn facing (LOOK.md 13.9).
struct View {
    /// The view model's kick (1 at a launch, decaying) and stride phase (LOOK.md 6.4).
    kick: f32,
    swing: f32,
    stride: f32,
    model: Option<ViewModel>,
    viewport: Viewport,
    /// The aim resolved by the last third-person tick, for the HUD.
    aim: (f32, f32),
    /// The recoil's punch on the view (MODES.md 3.3), (yaw, pitch) degrees, decaying.
    punch: (f32, f32),
    /// How far the eye has sunk into a crouch (MODES.md 3.4), units, eased.
    eye_drop: f32,
    /// The scope's zoom this frame (1 without one): the field of view is divided by it
    /// and so is the mouse.
    zoom: f32,
    bench_yaw0: f32,
    /// The last frame's view-projection and the window's size: what a click is
    /// unprojected through.
    last_vp: Option<(glam::Mat4, (f32, f32))>,
    /// The yaw each body was drawn facing last frame (LOOK.md 13.9), by its key.
    facings: HashMap<u32, f32>,
}

/// The fitting rooms of the command line (CONTENT.md 9), and where the own body was at
/// the last offline frame (its ground travel for the steps).
struct Offline {
    /// `--prop FILE` or `--prop KEY` offline: the own body holds it, once it is on the GPU.
    prop: Option<usize>,
    /// `--worn KEY`: the overlay the own body and the crowd wear offline, and its cover.
    worn: Vec<(usize, u32)>,
    /// And the patches (LOOK.md 6.6), as ids of the avatars' registry.
    patches: Vec<usize>,
    /// `--off KEY`: the prop in the own body's left hand, to look at.
    off: Option<usize>,
    /// `--sheathed`: the own prop rests on the back (LOOK.md 6.7).
    sheathed: bool,
    sound_from: Option<Vec3>,
}

/// What a frame draws of the zone: the entities and the bodies, the fight's effects
/// (LOOK.md 13), the names and the numbers over the bodies, and what the HUD shows of
/// the squad, the party and the creature fought.
struct Scene {
    entities: Vec<EntityDraw>,
    /// Bodies to draw this frame (players, the own body in third person).
    bodies: Vec<Body>,
    /// Models the zone revoked since the last frame.
    revoked: Vec<ModelId>,
    /// The fight's effects between frames, and the triangles they made this frame.
    effects: crate::fx::Effects,
    fx: crate::fx::FxMesh,
    /// The names over the bodies (LOOK.md 13).
    tags: Vec<Tag>,
    /// The numbers over the bodies this frame (LOOK.md 13.8).
    pops: Vec<crate::fx::Pop>,
    /// The combo counter (MODES.md 4.6): blows the own hand landed within two seconds of
    /// each other, and when the last landed.
    combo: (u32, Option<Instant>),
    /// Per squad slot: the companion's health as last sent, and whether it lives.
    squad_view: Vec<(Option<u16>, bool)>,
    /// The creature being fought: name, health, maximum.
    target_view: Option<(String, u16, u16)>,
    /// The party's other members: `None` when no body of that name is here, else the
    /// health the wire carries for it, when it does.
    party_view: Vec<(String, Option<Option<u16>>)>,
    /// The script's aim (`--script fight`): the nearest living enemy as of the last frame,
    /// its id, position and velocity, and when it was seen.
    script_target: Option<(u32, Vec3, Vec3, Instant)>,
}

/// The map besides its BSP: its mesh until the renderer takes it, its palette, a map to
/// switch to, and the hash a zone's `Welcome` is compared with.
struct World {
    mesh: Option<WorldMesh>,
    palette: world::Palette,
    /// A map to switch to before the next frame (a zone change), with its hash.
    pending_map: Option<(Bsp, u64)>,
    faces_total: usize,
    /// The hash of the map that is loaded: what a zone's `Welcome` is compared with.
    map_hash: u64,
}

/// The screens (CLIENT.md 4 to 6) and the toolkit's memory.
struct Screens {
    /// The screens before the game (CLIENT.md 4.1 to 4.3), and whether one is up.
    front: Option<Front>,
    front_up: bool,
    /// A person started the client and there is no hub to talk to: the screen that says
    /// so is up (it names the settings file, and what is wrong if a hub was named).
    title: Option<crate::menu::Title>,
    /// The game menu when it is open, and the chat (CLIENT.md 4.4, 5).
    menu: Option<GameMenu>,
    chat: Chat,
    /// The inventory or a stall, when one is open (ITEMS.md 6).
    bag: Option<Bag>,
    /// The page of people, a trade or the tavern (PARTY.md 8).
    people: Option<People>,
    gm_page: Option<GmPage>,
    /// The character's page (MATRIX.md 9.1): `K`, or the menu.
    character_page: Option<CharacterPage>,
    /// The toolkit's memory, and what happened since the last frame for it.
    ui: UiState,
    ui_input: UiInput,
    /// Somebody plays the person (CLIENT.md 9).
    ui_script: Option<UiScript>,
    /// Whether a screen was up in the last frame: the pointer follows the change.
    was_up: bool,
    /// The last frame drew the screens: a UI script acts on what was drawn, so it waits
    /// through frames that drew nothing (a window that is covered, a surface being made).
    ui_drawn: bool,
}

/// The pointer, the modifiers, the fingers (MODES.md 5.6) and what they are doing.
struct Pointer {
    /// The pointer, in pixels, and the last press (for the double click).
    cursor: (f32, f32),
    last_press: Option<(Instant, (f32, f32))>,
    shift: bool,
    /// Ctrl (or the command key) is held and Alt is not: what a shortcut is made with. (Ctrl
    /// with Alt is how some keyboards type `@`.)
    command: bool,
    /// A paste on its way: the clipboard is read on a thread of its own (its owner may
    /// take seconds to answer), and what it held is typed by the frame that finds it here.
    #[cfg(not(target_arch = "wasm32"))]
    pasting: Option<std::sync::mpsc::Receiver<String>>,
    grabbed: bool,
    /// When the pointer was last asked for (a browser gives and takes it by itself,
    /// WEB.md 3.4).
    grab_asked: Option<Instant>,
    /// The cursor the window shows (MODES.md 5.5): set again only when it changes.
    cursor_icon: CursorIcon,
    /// The fingers on the screen (MODES.md 5.6), the controls drawn for them this frame
    /// (where a finger may land on one), and a tap's primary, held a moment.
    fingers: Fingers,
    touch_buttons: Vec<(TouchButton, ui::Rect)>,
    tap_fire: Option<Instant>,
    /// A right press in the RPG mode: when and where, and the camera's yaw and pitch
    /// then, to tell a tap on the target (the secondary) from a drag of the orbit.
    rpg_right: Option<(Instant, (f32, f32), f32, f32)>,
}

/// The hub this client talks to, if it talks to one (CLIENT.md 2), and the session.
struct Session {
    hub: Option<Hub>,
    account: Option<Account>,
    character: Option<gm_hub_proto::protocol::CharacterId>,
    /// When the hub last heard from this client while it plays: a session lives while it
    /// is used (HUB.md 3.1), and a zone is played without a word to the hub.
    hub_touched: Instant,
    /// Zones being left: each says its goodbye on its own thread while the frames go on;
    /// kept until it has, or for a second.
    leaving: Vec<(NetClient, Instant)>,
    /// The zone's connection ended and a person is at the client: the reason, for the
    /// characters screen.
    zone_ended: Option<String>,
}

/// The frames counted and timed, and the run's end.
struct Clock {
    stats: FrameStats,
    last_frame: Instant,
    last_title: Instant,
    started: Instant,
    title_frame: usize,
    last_report: Instant,
    report_frame: usize,
    acquire_timeouts: u32,
    adapter_info: Option<wgpu::AdapterInfo>,
    exit_requested: bool,
}

/// What the settings file holds (CLIENT.md 8): a change is written within a second.
struct Prefs {
    kept: Settings,
    #[cfg(not(target_arch = "wasm32"))]
    path: Option<std::path::PathBuf>,
}

/// What only a browser has (WEB.md 3): the device and the map its tasks deliver, the
/// first frame's time, and what the page was last told.
#[cfg(target_arch = "wasm32")]
struct Web {
    /// The GPU device and a fetched map arrive here from their tasks.
    pending_gpu: PendingSlot<(wgpu::Surface<'static>, Gpu)>,
    pending_fetch: PendingSlot<(Bsp, u64)>,
    /// Milliseconds from navigation to the first frame presented (WEB.md 9).
    first_frame_ms: f64,
    /// Whether the browser had given the pointer as of the last frame.
    was_locked: bool,
    /// What the page was last told about the screens (CLIENT.md 4.1).
    told: String,
    /// What the page was last told of the text field with the keyboard (WEB.md 3.6): its
    /// value, or `None` for no field; and where the fields were, in CSS pixels.
    told_field: Option<String>,
    told_fields: Vec<f32>,
}

/// The client: what it was started with, the map, the window and the renderer on it,
/// the local walk or the zone, and the parts of a frame by concern.
struct App {
    opts: Options,
    /// The content bundle: the manifest, the props by key (CONTENT.md 6).
    content: Content,
    /// The window, from its creation: the renderer on it (`active`) may come later.
    window: Option<Arc<Window>>,
    bsp: Bsp,
    active: Option<Active>,
    sim: Sim,
    online: Option<Online>,
    input: Input,
    /// What is heard (SOUND.md).
    sound: crate::sound::Sound,
    /// The RPG mode's target, walk and waiting action (MODES.md 5).
    rpg: crate::rpg::Rpg,
    settings: Settings,
    /// A replay being watched instead of a game being played (ANTICHEAT.md 3.4).
    #[cfg(not(target_arch = "wasm32"))]
    playback: Option<crate::playback::Playback>,
    view: View,
    offline: Offline,
    scene: Scene,
    world: World,
    screens: Screens,
    pointer: Pointer,
    session: Session,
    clock: Clock,
    prefs: Prefs,
    #[cfg(target_arch = "wasm32")]
    web: Web,
}

/// A value a browser task delivers to the frame that waits for it.
#[cfg(target_arch = "wasm32")]
type PendingSlot<T> = std::rc::Rc<std::cell::RefCell<Option<Result<T, String>>>>;

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.active.is_some() || self.window.is_some() {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        let attrs = Window::default_attributes()
            .with_title("gamengine")
            .with_inner_size(winit::dpi::PhysicalSize::new(
                self.opts.width,
                self.opts.height,
            ))
            // The smallest frame the screens are made for (CLIENT.md 3).
            .with_min_inner_size(winit::dpi::PhysicalSize::new(640, 360));
        // The page owns the canvas and its size (WEB.md 5).
        #[cfg(target_arch = "wasm32")]
        let attrs = {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowAttributesExtWebSys;
            let canvas = web_sys::window()
                .and_then(|w| w.document())
                .and_then(|d| d.get_element_by_id("gm-canvas"))
                .and_then(|e| e.dyn_into::<web_sys::HtmlCanvasElement>().ok());
            Window::default_attributes()
                .with_title("gamengine")
                .with_canvas(canvas)
                .with_prevent_default(true)
                .with_focusable(true)
        };
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                self.fail(event_loop, &format!("window creation failed: {e}"));
                return;
            }
        };
        self.window = Some(window.clone());
        #[cfg(not(target_arch = "wasm32"))]
        {
            let instance = wgpu::Instance::new(
                wgpu::InstanceDescriptor::new_without_display_handle_from_env(),
            );
            let result = instance
                .create_surface(window.clone())
                .map_err(Error::from)
                .and_then(|surface| {
                    let gpu = Gpu::new(&instance, Some(&surface), self.opts.software)?;
                    self.activate(window.clone(), surface, gpu)
                });
            if let Err(e) = result {
                self.fail(event_loop, &format!("renderer setup failed: {e}"));
            }
        }
        // A browser hands out its GPU device asynchronously: the frame picks it up.
        #[cfg(target_arch = "wasm32")]
        {
            let slot = self.web.pending_gpu.clone();
            let software = self.opts.software;
            wasm_bindgen_futures::spawn_local(async move {
                let made = async {
                    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
                    // Each build draws through one API (WEB.md 3.2).
                    desc.backends = if cfg!(feature = "webgl") {
                        wgpu::Backends::GL
                    } else {
                        wgpu::Backends::BROWSER_WEBGPU
                    };
                    let instance = wgpu::Instance::new(desc);
                    let surface = instance
                        .create_surface(window.clone())
                        .map_err(|e| e.to_string())?;
                    let gpu = Gpu::request(&instance, Some(&surface), software)
                        .await
                        .map_err(|e| e.to_string())?;
                    Ok((surface, gpu))
                }
                .await;
                *slot.borrow_mut() = Some(made);
                window.request_redraw();
            });
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.window_event(event_loop, event);
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _id: DeviceId, event: DeviceEvent) {
        self.device_event(event);
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(w) = &self.window
            && !self.clock.exit_requested
        {
            w.request_redraw();
        }
    }
}
