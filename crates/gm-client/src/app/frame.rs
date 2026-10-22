use super::*;

/// The view model to draw this frame (LOOK.md 6.4), decided by the online frame and
/// pushed once the avatars' frame has begun.
#[derive(Clone, Copy)]
pub(super) struct ViewModel {
    pub(super) slot: usize,
    /// The template's `fit_view` for the prop (LOOK.md 6.4), identity without one.
    pub(super) fit: Mat4,
    pub(super) eye: Vec3,
    pub(super) yaw: f32,
    pub(super) pitch: f32,
    pub(super) stride: f32,
    pub(super) kick: f32,
    /// −1 drawn back in a windup, +1 across at the end of a cut, 0 at rest.
    pub(super) swing: f32,
    /// How far along the reload in hand is, 0 when none (MODES.md 3.2).
    pub(super) reload: f32,
    pub(super) light: [f32; 3],
}

/// What the screens, the menu and the chat line asked for in a frame (`App::screens_frame`).
struct Actions {
    front: [Action; 2],
    menu: MenuAction,
    said: Option<Said>,
    bag: BagAction,
    smithy: SmithyAction,
    forge: ForgeAction,
    people: PeopleAction,
    gm: GmAction,
    character: CharacterAction,
}

impl Default for Actions {
    fn default() -> Self {
        Actions {
            front: [Action::None, Action::None],
            menu: MenuAction::None,
            said: None,
            bag: BagAction::None,
            smithy: SmithyAction::None,
            forge: ForgeAction::None,
            people: PeopleAction::None,
            gm: GmAction::None,
            character: CharacterAction::None,
        }
    }
}

/// The paperdoll of a frame (LOOK.md 5): the panel's rectangle, its camera, its bodies.
struct Doll {
    rect: ui::Rect,
    vp: Mat4,
    draws: Vec<CharacterDraw>,
}

impl App {
    /// A frame of a replay (ANTICHEAT.md 3.4): its keys, its time, its scene, and the camera
    /// in the followed body's eyes, behind it, or above it.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn playback_frame(&mut self, frame_dt: f32) -> (Vec3, f32, f32) {
        let Some(p) = &mut self.playback else {
            return (self.sim.eye(), self.sim.yaw, self.sim.pitch);
        };
        let pressed = |k: KeyCode, input: &mut Input| input.just_pressed.remove(&k);
        if pressed(KeyCode::BracketLeft, &mut self.input) {
            p.cycle(-1);
        }
        if pressed(KeyCode::BracketRight, &mut self.input) {
            p.cycle(1);
        }
        if pressed(KeyCode::Space, &mut self.input) {
            p.paused = !p.paused;
        }
        if p.paused {
            if pressed(KeyCode::Comma, &mut self.input) {
                p.step(-1);
            }
            if pressed(KeyCode::Period, &mut self.input) {
                p.step(1);
            }
        }
        if pressed(KeyCode::ArrowLeft, &mut self.input) {
            p.seek(p.time - 5.0);
        }
        if pressed(KeyCode::ArrowRight, &mut self.input) {
            p.seek(p.time + 5.0);
        }
        for (key, speed) in [
            (KeyCode::Digit1, 0.25),
            (KeyCode::Digit2, 0.5),
            (KeyCode::Digit3, 1.0),
            (KeyCode::Digit4, 2.0),
        ] {
            if pressed(key, &mut self.input) {
                p.speed = speed;
            }
        }
        self.input.just_pressed.clear();
        p.advance(frame_dt);
        self.scene.entities.clear();
        self.scene.bodies.clear();
        let outside = self.view.viewport == Viewport::Third;
        let (eye, yaw, pitch) = p.scene(outside, &mut self.scene.bodies, &mut self.scene.entities);
        if self.view.viewport == Viewport::Third {
            (third_person_camera(&self.bsp, eye, yaw, pitch), yaw, pitch)
        } else {
            (eye, yaw, pitch)
        }
    }

    /// One frame (CLIENT.md 2.1), as named steps: what arrived since the last, the
    /// pointer and the look, the scene of the zone, the replay or the local walk, then
    /// the draw: the world, the bodies, the HUD, the screens, the paperdoll, the present.
    pub(super) fn frame(&mut self, event_loop: &ActiveEventLoop) {
        let Some((now, frame_dt)) = self.frame_begin(event_loop) else {
            return;
        };
        let bench = self.opts.bench_frames.is_some();
        let returns = self.returns_to_screens();
        let up = self.pointer_frame();
        self.look_frame(frame_dt, up, bench);
        #[cfg(not(target_arch = "wasm32"))]
        let watching = self.playback.is_some();
        #[cfg(target_arch = "wasm32")]
        let watching = false;
        let Some((camera, cam_yaw, cam_pitch)) =
            self.scene_frame(frame_dt, event_loop, watching, bench)
        else {
            return;
        };
        // The UI's scale this frame: read before the renderer is borrowed for the frame.
        let chosen = self.ui_scale_choice();
        self.touch_frame(watching, bench);
        let Some(frame) = self.acquire(camera, event_loop) else {
            return;
        };
        let vp = self.stage_scene(frame_dt, camera, cam_yaw, cam_pitch);
        self.hud_frame(vp, chosen, watching, bench);
        let actions = if bench {
            Actions::default()
        } else {
            self.screens_frame(chosen, returns)
        };
        let doll = self.doll_frame(frame_dt);
        if !self.present(frame, vp, doll, bench) {
            return;
        }
        self.clock_frame(now, bench);
        self.title_frame();
        self.run_end(event_loop);
        self.take_actions(actions, event_loop);
    }

    /// What arrived since the last frame: the browser's GPU device, a map to switch to,
    /// a paste, the script's events, the page's form, the goodbyes said; and the hub
    /// told now and then that the session is in use (HUB.md 3.1). Returns the frame's
    /// start and its length in seconds; `None` when the frame ends here (the script
    /// failed, the renderer could not be made).
    fn frame_begin(&mut self, event_loop: &ActiveEventLoop) -> Option<(Instant, f32)> {
        self.poll_bar();
        #[cfg(target_arch = "wasm32")]
        if self.active.is_none() {
            let made = self.web.pending_gpu.borrow_mut().take();
            match (made, self.window.clone()) {
                (Some(Ok((surface, gpu))), Some(window)) => {
                    if let Err(e) = self.activate(window, surface, gpu) {
                        self.fail(event_loop, &format!("renderer setup failed: {e}"));
                        return None;
                    }
                }
                (Some(Err(e)), _) => {
                    self.fail(event_loop, &format!("renderer setup failed: {e}"));
                    return None;
                }
                _ => {}
            }
        }
        if let Some((bsp, hash)) = self.world.pending_map.take() {
            self.switch_map(bsp, hash);
        }
        let now = Instant::now();
        let frame_dt = (now - self.clock.last_frame).as_secs_f32();
        self.clock.last_frame = now;
        #[cfg(not(target_arch = "wasm32"))]
        self.pasted();
        if !self.run_script(event_loop) {
            return None;
        }
        #[cfg(target_arch = "wasm32")]
        self.page_frame();
        self.session
            .leaving
            .retain(|(net, since)| !net.gone() && since.elapsed().as_secs_f32() < 1.0);
        // A session lives while it is used, and a zone is played without a word to the hub:
        // now and then the client says one (HUB.md 3.1).
        if self.online.is_some()
            && self.session.hub_touched.elapsed().as_secs() >= SESSION_TOUCH_SECS
            && let (Some(hub), Some(account)) = (&self.session.hub, &self.session.account)
        {
            self.session.hub_touched = Instant::now();
            let session = account.session;
            // Nobody waits for the answer.
            let _ = hub.call(PlayerRequest::ListZones { session });
        }
        Some((now, frame_dt))
    }

    /// The pointer follows the screens: free while one is up, the game's again after.
    /// Not for a finger (WEB.md 3.5): a phone's browser gives the pointer lock to the tap
    /// that put the screen down and takes it back at the next touch, and losing it is
    /// Escape, so every tap in the game opened the menu. Then the fingers' events.
    /// Returns whether a screen is up.
    fn pointer_frame(&mut self) -> bool {
        let up = self.screen_up();
        if up != self.screens.was_up {
            self.screens.was_up = up;
            if up {
                self.release_keys();
                self.set_grab(false);
            } else if self.wants_pointer() && self.focused() && !self.pointer.fingers.seen {
                self.set_grab(true);
            }
        }
        if self.capturing() {
            // Whatever was held is not held for the game while the toolkit has the keys.
            self.release_keys();
        }
        self.fingers_frame(up);
        up
    }

    /// Mouse look is applied per frame for responsiveness; movement uses it at tick
    /// time. A bench turns by itself; a taunt (MATRIX.md 8) holds the view on the
    /// taunter with the body, and the mouse moves nothing until the taunt is out.
    fn look_frame(&mut self, frame_dt: f32, up: bool, bench: bool) {
        if bench && self.opts.crowd > 0 {
            let t = self.clock.started.elapsed().as_secs_f32();
            self.sim.yaw = self.view.bench_yaw0 + BENCH_CROWD_SWING_DEG * (t * 0.7).sin();
        } else if bench {
            self.sim.yaw += BENCH_YAW_DEG_PER_S * frame_dt;
        } else if (self.pointer.grabbed || self.pointer.fingers.turning() || self.pointer.dragged)
            && !up
        {
            let turn = self.settings.sensitivity / self.view.zoom.max(1.0);
            let tilt = if self.settings.invert { -turn } else { turn };
            self.sim.yaw -= self.input.mouse_dx * turn;
            let (low, high) = if self.rpg_mode() {
                (5.0, 80.0)
            } else {
                (-89.0, 89.0)
            };
            self.sim.pitch = (self.sim.pitch + self.input.mouse_dy * tilt).clamp(low, high);
        }
        if let Some(c) = self.online.as_ref().and_then(|o| o.client.as_ref())
            && c.mover.statuses.has(gm_core::vocab::Status::Taunt)
            && c.mover.lock_yaw.is_some()
        {
            self.sim.yaw = c.mover.yaw;
        }
        self.sim.yaw = self.sim.yaw.rem_euclid(360.0);
        self.input.mouse_dx = 0.0;
        self.input.mouse_dy = 0.0;
        self.pointer.dragged = false;
    }

    /// The scene of this frame and the camera `(position, yaw, pitch)`: a replay being
    /// watched, the zone being played, the map turning slowly behind a screen with no
    /// zone (CLIENT.md 2), or the local walk. `None` when the frame ends here.
    fn scene_frame(
        &mut self,
        frame_dt: f32,
        event_loop: &ActiveEventLoop,
        watching: bool,
        bench: bool,
    ) -> Option<(Vec3, f32, f32)> {
        if watching {
            #[cfg(not(target_arch = "wasm32"))]
            {
                return Some(self.playback_frame(frame_dt));
            }
            #[cfg(target_arch = "wasm32")]
            unreachable!()
        }
        if self.online.is_some() {
            let camera = self.online_frame(frame_dt, event_loop)?;
            // The zone has the character: the screens step aside.
            if self.screens.front_up && self.online.as_ref().is_some_and(|o| o.client.is_some()) {
                self.screens.front_up = false;
                self.screens.chat.clear();
            }
            // Its connection ended and a person is here: back to the characters.
            if let Some(why) = self.session.zone_ended.take() {
                self.leave_zone(&why);
            }
            return Some(camera);
        }
        if self.screens.front_up || self.screens.title.is_some() {
            self.scene.entities.clear();
            self.scene.bodies.clear();
            let yaw = self.view.bench_yaw0
                + BACKDROP_DEG_PER_S * self.clock.started.elapsed().as_secs_f32();
            return Some((self.sim.eye(), yaw.rem_euclid(360.0), 0.0));
        }
        Some(self.offline_frame(frame_dt, bench))
    }

    /// The local walk (no zone): the own body against the map, its steps heard, the own
    /// body or the view model to look at (the fitting rooms of CONTENT.md 9).
    fn offline_frame(&mut self, frame_dt: f32, bench: bool) -> (Vec3, f32, f32) {
        let walk = self.opts.script.as_deref() == Some("walk");
        let input = if bench {
            MoveInput {
                yaw: self.sim.yaw,
                ..Default::default()
            }
        } else if walk {
            // A second standing, five seconds forward, standing again (SOUND.md 7):
            // what the sound gate renders to a file.
            let t = self.clock.started.elapsed().as_secs_f32();
            MoveInput {
                yaw: self.sim.yaw,
                forward: if (1.0..6.0).contains(&t) { 1.0 } else { 0.0 },
                ..Default::default()
            }
        } else {
            self.input.move_input(self.sim.yaw)
        };
        self.input.just_pressed.clear();
        self.sim.advance(&self.bsp, &input, frame_dt);
        self.offline_sound(frame_dt);
        self.scene.entities.clear();
        self.scene.bodies.clear();
        match self.view.viewport {
            Viewport::First => self.offline_view_model(frame_dt),
            _ => self.offline_body(),
        }
    }

    /// Heard offline: the own body's steps and landings (its animation is the local
    /// mover's; nobody else is here).
    fn offline_sound(&mut self, frame_dt: f32) {
        let v = self.sim.curr.velocity;
        let anim = if !self.sim.curr.on_ground {
            gm_core::sim::anim::AIR
        } else if v.truncate().length() > 20.0 {
            gm_core::sim::anim::RUN
        } else {
            gm_core::sim::anim::IDLE
        };
        let here = self.sim.origin();
        let travel = match (self.sim.curr.on_ground, self.offline.sound_from) {
            (true, Some(from)) => (here - from).truncate().length(),
            _ => 0.0,
        };
        self.offline.sound_from = Some(here);
        let me = crate::sound::cues::OwnNow {
            id: 0,
            pos: self.sim.origin(),
            on_ground: self.sim.curr.on_ground,
            travel,
            health: 0,
            alive: true,
        };
        let now = self.clock.started.elapsed().as_secs_f32();
        // (No zone here: the word's tick is the frame clock at 64 Hz.)
        self.sound.own(now, me, &[((now * 64.0) as u32, anim)], &[]);
        let listener = crate::sound::Listener {
            pos: self.sim.eye(),
            yaw: self.sim.yaw.to_radians(),
        };
        self.sound.end_frame(frame_dt, listener);
    }

    /// The fitting room of the view model (`--prop KEY`, LOOK.md 6.4): the prop in the
    /// view with its stride's bob; R held works a reload over and over, to see it.
    fn offline_view_model(&mut self, frame_dt: f32) -> (Vec3, f32, f32) {
        let eye = self.sim.eye();
        self.view.stride += self.sim.curr.velocity.truncate().length() * frame_dt / 64.0;
        let t = self.clock.started.elapsed().as_secs_f32();
        let reload = if self.input.down(KeyCode::KeyR) {
            (t / 2.4).fract()
        } else {
            0.0
        };
        self.view.model = self.offline.prop.map(|slot| ViewModel {
            slot,
            fit: self
                .opts
                .prop
                .as_deref()
                .map_or(Mat4::IDENTITY, |k| view_fit_of(&self.content, k)),
            eye,
            yaw: self.sim.yaw,
            pitch: self.sim.pitch,
            stride: if self.sim.curr.on_ground {
                self.view.stride
            } else {
                0.0
            },
            kick: 0.0,
            swing: 0.0,
            reload,
            light: crate::avatars::light_at(&self.bsp, eye),
        });
        (eye, self.sim.yaw, self.sim.pitch)
    }

    /// The own body offline, in the third person, with what the command line gave it to
    /// hold and wear (`--prop FILE`, `--off`, `--worn`, `--sheathed`, CONTENT.md 9).
    fn offline_body(&mut self) -> (Vec3, f32, f32) {
        let v = self.sim.curr.velocity;
        self.scene.bodies.push(Body {
            key: OWN,
            origin: self.sim.origin(),
            yaw: self.sim.yaw,
            pitch: self.sim.pitch,
            anim: if !self.sim.curr.on_ground {
                gm_core::sim::anim::AIR
            } else if v.truncate().length() > 20.0 {
                gm_core::sim::anim::RUN
            } else {
                gm_core::sim::anim::IDLE
            },
            // Offline there is no mover: the keys draw the squat, on the ground or off it.
            crouched: self.input.down(KeyCode::ControlLeft)
                || self.input.down(KeyCode::ControlRight)
                || self.input.down(KeyCode::KeyX),
            frame: 1,
            aspects: 0,
            status: 0,
            model: None,
            distance: 0.0,
            lit: 0.0,
            prop: self.offline.prop,
            off: self.offline.off,
            worn: crate::avatars::places(&self.offline.worn).0,
            hide: crate::avatars::places(&self.offline.worn).1,
            patches: {
                let mut p = [None; BODY_PLACES];
                for (o, id) in p.iter_mut().zip(&self.offline.patches) {
                    *o = Some(*id);
                }
                p
            },
            sheathed: self.offline.sheathed,
        });
        (
            third_person_camera(&self.bsp, self.sim.eye(), self.sim.yaw, self.sim.pitch),
            self.sim.yaw,
            self.sim.pitch,
        )
    }

    /// The controls a finger may land on this frame (MODES.md 5.6), which the HUD draws
    /// where they are hit.
    fn touch_frame(&mut self, watching: bool, bench: bool) {
        self.pointer.touch_buttons =
            if self.pointer.fingers.seen && !watching && !bench && !self.screen_up() {
                let size = self.active.as_ref().map_or((1.0, 1.0), |a| {
                    (a.config.width.max(1) as f32, a.config.height.max(1) as f32)
                });
                let use_in_reach = self.use_hint().is_some();
                touch_controls(
                    size,
                    self.ui_scale(),
                    self.online.as_ref(),
                    self.rpg_mode(),
                    use_in_reach,
                )
            } else {
                Vec::new()
            };
    }

    /// The world is drawn from the camera's leaf; then the frame's texture from the
    /// surface. `None` when there is no renderer yet, or the surface has no frame to
    /// give this time (reconfigured, occluded, or lost).
    fn acquire(
        &mut self,
        camera: Vec3,
        event_loop: &ActiveEventLoop,
    ) -> Option<wgpu::SurfaceTexture> {
        let Some(a) = &mut self.active else {
            return None;
        };
        let leaves = vec![self.bsp.leaf_for_point(camera)];
        if a.drawn_from != leaves {
            if leaves.is_empty() || leaves.contains(&0) {
                a.renderer.set_visible_faces(&a.gpu, None);
            } else {
                a.renderer
                    .set_visible_faces(&a.gpu, Some(&visible_from(&self.bsp, &leaves)));
            }
            a.drawn_from = leaves;
        }
        match a.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) => Some(f),
            wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
                log::debug!("surface suboptimal; reconfiguring");
                a.gpu.queue.present(f);
                self.configure_surface();
                None
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.clock.acquire_timeouts += 1;
                if self.clock.acquire_timeouts == 3 {
                    log::warn!(
                        "the display is not releasing frames (Fifo on a compositor, or a sleeping monitor); try --present mailbox"
                    );
                }
                None
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                log::debug!("surface occluded");
                None
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                log::debug!("surface outdated or lost; reconfiguring");
                self.configure_surface();
                None
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                log::error!("surface validation error");
                event_loop.exit();
                None
            }
        }
    }

    /// The scene staged for the renderer: the models revoked, the own avatar pinned, the
    /// props and the official avatars that arrived, the view model, the bodies with the
    /// rings of their aspects, the effects, the crowd. Returns the view-projection.
    fn stage_scene(&mut self, frame_dt: f32, camera: Vec3, cam_yaw: f32, cam_pitch: f32) -> Mat4 {
        let Some(a) = &mut self.active else {
            return Mat4::IDENTITY;
        };
        let aspect = a.config.width as f32 / a.config.height.max(1) as f32;
        for id in self.scene.revoked.drain(..) {
            a.avatars.revoke(&id, &mut a.renderer.characters);
        }
        // The own avatar is the last thing the disk cache lets go of (MODELS.md 8).
        if let Some(o) = &self.online
            && let Some(c) = &o.client
            && let Some(id) = o.names.get(&c.my_id).and_then(|n| n.2)
        {
            a.avatars.pin(&id);
        }
        // Props that arrived from the site since last frame go on the GPU, one a frame
        // (CONTENT.md 6; natively a prop is read when first asked for).
        #[cfg(target_arch = "wasm32")]
        {
            let (gpu, characters) = (&a.gpu, &mut a.renderer.characters);
            let avatars = &mut a.avatars;
            self.content.poll(
                |model| Some(characters.add_model(gpu, model)),
                |patch| Some(avatars.add_patch(patch.clone())),
            );
        }
        // The official avatars of the bundle (CONTENT.md 3.2) stand in for the mannequins
        // of their frame: natively a file is read the first time it is asked for, in a
        // browser it is fetched like a prop and lands through `poll`.
        {
            let (gpu, characters, avatars) = (&a.gpu, &mut a.renderer.characters, &mut a.avatars);
            for frame in 0..4u8 {
                let key = self.content.official_key(frame);
                let slot = self.content.avatar(frame, |model| {
                    let slot = characters.add_model(gpu, model);
                    // Its atlas stays on the CPU: the cloth patches are laid into a copy
                    // of it when the body wears them (LOOK.md 6.6).
                    avatars.keep_body(slot, key.as_deref().unwrap_or(""), model);
                    Some(slot)
                });
                avatars.set_official(frame, slot);
            }
        }
        a.avatars.begin_frame();
        if let Some(v) = self.view.model.take() {
            a.avatars.view_model(
                v.slot, v.fit, v.eye, v.yaw, v.pitch, v.stride, v.kick, v.swing, v.reload, v.light,
            );
        }
        for body in &self.scene.bodies {
            a.avatars.push(
                body,
                frame_dt,
                &self.bsp,
                &a.renderer.characters,
                &mut self.scene.entities,
            );
            // The ring of its aspects at its feet (MODELS.md 9, LOOK.md 13).
            if body.anim != gm_core::sim::anim::DEAD && body.aspects != 0 {
                crate::fx::aspect_ring(
                    &mut self.scene.fx,
                    body.origin + Vec3::Z * Hull::Player.mins().z,
                    body.yaw,
                    body.aspects,
                );
            }
        }
        // The frame's effects go to the renderer, which draws them after the bodies.
        a.renderer.fx.verts.append(&mut self.scene.fx.verts);
        a.avatars.push_crowd(
            self.clock.started.elapsed().as_secs_f32(),
            frame_dt,
            camera,
            &self.bsp,
            &a.renderer.characters,
            &mut self.scene.entities,
        );
        let vp =
            crate::render::view_proj_zoomed(camera, cam_yaw, cam_pitch, aspect, self.view.zoom);
        self.view.last_vp = Some((
            vp,
            (a.config.width.max(1) as f32, a.config.height.max(1) as f32),
        ));
        vp
    }

    /// One scale for the HUD and the screens: what the window gives, or what was chosen;
    /// and the atlas made for that scale (LOOK.md 2.2), from the frame it is here. Then
    /// the HUD of the game (COMPANIONS.md 6), unless a screen or a bench hides it.
    fn hud_frame(&mut self, vp: Mat4, chosen: u8, watching: bool, bench: bool) {
        let Some(a) = &mut self.active else {
            return;
        };
        let scale = ui::scale_for(
            (a.config.width.max(1) as f32, a.config.height.max(1) as f32),
            PANEL_UNITS,
            chosen,
        );
        self.content.want_atlas(scale as u8);
        if let Some(atlas) = self.content.take_atlas() {
            a.renderer.hud.set_atlas(&a.gpu, atlas);
        }
        a.renderer.hud.begin((a.config.width, a.config.height));
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(p) = &self.playback {
            p.hud(&mut a.renderer.hud);
        }
        if !watching && !bench && !self.screens.front_up && self.screens.title.is_none() {
            build_hud(
                &mut a.renderer.hud,
                self.online.as_ref(),
                vp,
                HudView {
                    tags: &self.scene.tags,
                    pops: &self.scene.pops,
                    first_person: self.view.viewport == Viewport::First,
                    hurt: self.scene.effects.own_hurt,
                    squad: &self.scene.squad_view,
                    party: &self.scene.party_view,
                    target: self.scene.target_view.as_ref(),
                    scale,
                    manifest: self.content.manifest.as_ref(),
                    time: self.clock.started.elapsed().as_secs_f32(),
                    own_name: &self.opts.character,
                    combo: (
                        self.scene.combo.0,
                        self.scene
                            .combo
                            .1
                            .map_or(f32::MAX, |t| t.elapsed().as_secs_f32()),
                    ),
                    // The body's posture, not the keys: a crouch takes the duck time, and
                    // under a low ceiling it lasts past the key (MODES.md 3.5).
                    crouched: self
                        .online
                        .as_ref()
                        .and_then(|o| o.client.as_ref())
                        .is_some_and(|c| c.mover.crouched),
                    zoom: self.view.zoom,
                    scoped: self.input.scoped,
                    touch: TouchHud {
                        buttons: &self.pointer.touch_buttons,
                        held: self
                            .pointer
                            .touch_buttons
                            .iter()
                            .map(|(b, _)| *b)
                            .filter(|b| self.pointer.fingers.holds(*b))
                            .collect(),
                        stick: self.pointer.fingers.stick_drawn(),
                    },
                },
            );
        }
    }

    /// The screens, the menu and the chat, over the HUD (CLIENT.md 4 to 6): drawn, and
    /// what they asked for this frame. The renderer and the toolkit's memory are taken
    /// out of `self` for the drawing, so that the screens can ask the client what they
    /// need, and put back after.
    fn screens_frame(&mut self, chosen: u8, returns: bool) -> Actions {
        let mut actions = Actions::default();
        let Some(mut a) = self.active.take() else {
            return actions;
        };
        let screen = self.screen_name();
        let mut state = std::mem::take(&mut self.screens.ui);
        let mut input = std::mem::take(&mut self.screens.ui_input);
        input.last_cursor = input.cursor;
        input.cursor = self.pointer.cursor;
        input.time = self.clock.started.elapsed().as_secs_f32();
        state.paperdolls.clear();
        let mut ui = Ui::begin_at(
            &mut a.renderer.hud,
            &mut state,
            &input,
            screen,
            PANEL_UNITS,
            chosen,
        );
        if let Some(title) = &self.screens.title {
            match crate::menu::title(&mut ui, title) {
                Some(true) => self.screens.title = None,
                Some(false) => actions.front[1] = Action::Quit,
                None => {}
            }
        } else if self.screens.front_up {
            if let Some(front) = &mut self.screens.front {
                actions.front = front.frame(&mut ui);
            }
        } else {
            self.game_screens(&mut ui, &mut actions, returns);
        }
        ui.end();
        if std::mem::take(&mut state.presses) > 0 {
            self.sound.play(crate::sound::synth::Cue::Click);
        }
        self.screens.ui_drawn = true;
        self.screens.ui = state;
        // What happened is used up; a button still held is still held.
        self.screens.ui_input = UiInput {
            down: input.down,
            // The pointer stays where it is: the paperdoll turns by how far it moved
            // since last frame, not by where it is.
            cursor: input.cursor,
            ..Default::default()
        };
        self.active = Some(a);
        actions
    }

    /// Which screen is up, by name (the UI scripts and the gates read it).
    fn screen_name(&self) -> &'static str {
        match (&self.screens.front, &self.screens.menu, &self.screens.bag) {
            _ if self.screens.title.is_some() => "title",
            (Some(front), _, _) if self.screens.front_up => front.screen.name(),
            (_, Some(menu), _) => menu.page.name(),
            (_, _, Some(bag)) => bag.name(),
            _ if self.screens.smithy.is_some() => {
                self.screens.smithy.as_ref().map_or("", |s| s.name())
            }
            _ if self.screens.forge.is_some() => {
                self.screens.forge.as_ref().map_or("", |f| f.name())
            }
            _ if self.screens.people.is_some() => {
                self.screens.people.as_ref().map_or("", |p| p.page.name())
            }
            _ if self.screens.gm_page.is_some() => {
                self.screens.gm_page.as_ref().map_or("", |g| g.name())
            }
            _ if self.screens.character_page.is_some() => self
                .screens
                .character_page
                .as_ref()
                .map_or("", |p| p.name()),
            _ if self.screens.chat.open => "chat",
            _ => "game",
        }
    }

    /// The screens of the game: the chat line, the inventory or the stall, the people,
    /// the game master's and the character's pages, the menu, or the corner's hints.
    fn game_screens<C: ui::Canvas>(
        &mut self,
        ui: &mut Ui<'_, C>,
        actions: &mut Actions,
        returns: bool,
    ) {
        let playing = self.online.as_ref().is_some_and(|o| o.client.is_some());
        if playing {
            actions.said = self.screens.chat.frame(ui, &mut self.settings.ignored);
        } else if self.screens.chat.open {
            // Between two zones nothing is drawn of it: nor does it keep the keys.
            self.screens.chat.drop_line();
        }
        // The game goes away under an open inventory as under the menu; a body that
        // stopped playing (a travel, the zone gone) has neither.
        if !playing {
            self.screens.bag = None;
            self.screens.smithy = None;
            self.screens.forge = None;
            self.screens.people = None;
            self.screens.gm_page = None;
        }
        let me = self.opts.name.clone();
        self.bag_screen(ui, actions, &me);
        self.smithy_screen(ui, actions);
        self.people_screen(ui, actions, &me);
        self.pages_screen(ui, actions);
        self.menu_screen(ui, actions, playing, returns);
    }

    /// The inventory or a stall (ITEMS.md 6), with who wears it for the equipment's
    /// ribbon (EQUIPMENT.md 4.1): the class the content names, and the build.
    fn bag_screen<C: ui::Canvas>(&mut self, ui: &mut Ui<'_, C>, actions: &mut Actions, me: &str) {
        let keeps_stall = self
            .online
            .as_ref()
            .is_some_and(|o| o.stalls.iter().any(|s| s.owner == me));
        let (class, ribbon) = self
            .online
            .as_ref()
            .and_then(|o| o.client.as_ref().map(|c| (&c.sheet, o.pack.as_ref())))
            .map(|(sheet, pack)| {
                let class = crate::showcase::class_name(pack, &sheet.build);
                let ribbon = crate::showcase::short_ribbon(&class, &sheet.build);
                (class, ribbon)
            })
            .unwrap_or_default();
        if let (Some(bag), Some(hub)) = (&mut self.screens.bag, &self.session.hub)
            && self.screens.menu.is_none()
        {
            let looks = crate::bag::ItemLooks {
                manifest: self.content.manifest.as_ref(),
            };
            let stats = self
                .online
                .as_ref()
                .and_then(|o| o.client.as_ref())
                .map(|c| crate::character::stat_rows(&c.sheet.build, &c.sheet.derived))
                .unwrap_or_default();
            let wearer = crate::bag::Wearer {
                name: me,
                ribbon: &ribbon,
                class: &class,
                stats: &stats,
            };
            actions.bag = bag.frame_with(ui, hub, keeps_stall, Instant::now(), looks, wearer);
        }
    }

    /// The smith's shop or the forge (ITEMS.md 6.6), when one is open and nothing is over it.
    fn smithy_screen<C: ui::Canvas>(&mut self, ui: &mut Ui<'_, C>, actions: &mut Actions) {
        if self.screens.menu.is_some() || self.screens.bag.is_some() {
            return;
        }
        let looks = crate::bag::ItemLooks {
            manifest: self.content.manifest.as_ref(),
        };
        let now = Instant::now();
        if let (Some(smithy), Some(hub)) = (&mut self.screens.smithy, &self.session.hub) {
            actions.smithy = smithy.frame(ui, hub, looks, now);
        } else if let (Some(forge), Some(hub)) = (&mut self.screens.forge, &self.session.hub) {
            actions.forge = forge.frame(ui, hub, looks, now);
        }
    }

    /// The people here and of the party, a trade, the tavern (PARTY.md 8). Who is here:
    /// the bodies that are people, and whether each stands near enough to trade with
    /// (the rule the zone decides with).
    fn people_screen<C: ui::Canvas>(
        &mut self,
        ui: &mut Ui<'_, C>,
        actions: &mut Actions,
        me: &str,
    ) {
        let (Some(people), Some(o), None, None) = (
            &mut self.screens.people,
            &mut self.online,
            &self.screens.menu,
            &self.screens.bag,
        ) else {
            return;
        };
        let Some(c) = &o.client else { return };
        let mine: [f32; 3] = c.mover.mv.origin.into();
        let others = c.others_at(c.render_tick(0.0));
        let mut here: Vec<Here> = o
            .names
            .iter()
            .filter(|(id, _)| **id != c.my_id && o.kinds.get(id) == Some(&BodyKind::Human))
            .map(|(id, (name, _, _))| Here {
                body: *id,
                name: name.clone(),
                near: others
                    .iter()
                    .find(|e| e.id == *id)
                    .is_some_and(|e| gm_net::control::trade_in_reach(mine, e.pos.into())),
            })
            .collect();
        here.sort_by(|a, b| a.name.cmp(&b.name));
        let hub = self
            .session
            .hub
            .as_ref()
            .filter(|_| self.session.account.is_some() && self.session.character.is_some())
            .map(|hub| hub as &dyn HubApi);
        let unix = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .map_or(0, |d| d.as_secs());
        let word = self
            .screens
            .chat
            .zone_said(std::time::Duration::from_secs(8));
        actions.people = people.frame_with(
            ui,
            hub,
            me,
            &mut o.social,
            &here,
            word,
            Instant::now(),
            unix,
            crate::bag::ItemLooks {
                manifest: self.content.manifest.as_ref(),
            },
        );
    }

    /// The game master's page (GM.md 4) and the character's page (CLIENT.md 4.5), each
    /// under nothing else.
    fn pages_screen<C: ui::Canvas>(&mut self, ui: &mut Ui<'_, C>, actions: &mut Actions) {
        if let (Some(page), Some(o), None, None, None) = (
            &mut self.screens.gm_page,
            &self.online,
            &self.screens.menu,
            &self.screens.bag,
            &self.screens.people,
        ) && let (Some(c), Some(pack)) = (&o.client, &o.pack)
        {
            let view = GmView {
                pack,
                own: &c.sheet.build,
                tuning: &o.tuning,
                rate: o.rate,
                note: &o.gm_note,
            };
            actions.gm = page.frame(ui, &view);
        }
        if let (Some(page), Some(o), None, None, None, None) = (
            &mut self.screens.character_page,
            &self.online,
            &self.screens.menu,
            &self.screens.bag,
            &self.screens.people,
            &self.screens.gm_page,
        ) && let (Some(c), Some(pack)) = (&o.client, &o.pack)
        {
            let view = CharacterView {
                pack,
                own: &c.sheet.build,
                worn: &o.worn,
                rate: o.rate,
            };
            actions.character = page.frame(ui, &view);
        }
    }

    /// The menu when it is open and nothing is over it; without one, in the game, the
    /// corner's hints: where this is, the two keys nothing else tells of, and the stall
    /// the body stands at, whose it is and the key.
    fn menu_screen<C: ui::Canvas>(
        &mut self,
        ui: &mut Ui<'_, C>,
        actions: &mut Actions,
        playing: bool,
        returns: bool,
    ) {
        match &mut self.screens.menu {
            Some(_) if self.screens.bag.is_some() => {}
            Some(menu) => {
                let hub = self
                    .session
                    .hub
                    .as_ref()
                    .zip(self.session.account.as_ref())
                    .map(|(hub, account)| (hub as &dyn HubApi, account.session));
                // A page's tab is closed and made fullscreen by the browser.
                let offers = Offers {
                    inventory: playing && hub.is_some() && self.session.character.is_some(),
                    equipment: playing && hub.is_some() && self.session.character.is_some(),
                    people: playing,
                    gm: playing && self.online.as_ref().is_some_and(|o| o.gm),
                    travel: playing && hub.is_some(),
                    leave: returns && self.online.is_some(),
                    fullscreen: cfg!(not(target_arch = "wasm32")),
                    #[cfg(target_arch = "wasm32")]
                    page_fullscreen: Some(crate::web::fullscreen_on()),
                    #[cfg(not(target_arch = "wasm32"))]
                    page_fullscreen: None,
                    quit: cfg!(not(target_arch = "wasm32")),
                };
                let here = self.online.as_ref().map_or("", |o| o.zone_name.as_str());
                actions.menu = menu.frame(ui, hub, offers, here, &mut self.settings);
            }
            None if self.screens.bag.is_some()
                || self.screens.smithy.is_some()
                || self.screens.forge.is_some()
                || self.screens.people.is_some()
                || self.screens.gm_page.is_some()
                || self.screens.character_page.is_some() => {}
            None if playing && !self.screens.chat.open => {
                let here = self.online.as_ref().map_or("", |o| o.zone_name.as_str());
                let hint = if here.is_empty() {
                    "Esc menu  Enter chat".to_string()
                } else {
                    format!("{here}  Esc menu  Enter chat")
                };
                let (w, h) = ui.size();
                ui.small(w - 16.0, h - 14.0 * ui.scale, ui::FAINT, &hint);
                if let Some(look) = self.use_hint() {
                    ui.small(w - 16.0, h - 24.0 * ui.scale, ui::TEXT, &look);
                }
            }
            None => {}
        }
    }

    /// The paperdoll (LOOK.md 5): the own body as the equip panel shows it, idle, turned
    /// by the drag across it, with what it holds, drawn into the panel's rectangle by a
    /// camera of its own; on the selector (CLIENT.md 4.2) the character or the archetype
    /// shown. Nothing without a panel this frame.
    fn doll_frame(&mut self, frame_dt: f32) -> Option<Doll> {
        let (rect, turn, open) = {
            let d = self.screens.ui.paperdolls.first()?;
            (d.rect, d.turn, d.open)
        };
        let shown = rect.w > 1.0 && rect.h > 1.0;
        let draws = if !shown {
            Vec::new()
        } else if self.screens.front_up {
            self.doll_on_selector(frame_dt, turn)
        } else {
            self.doll_of_own(frame_dt, turn)
        };
        let aspect = rect.w / rect.h.max(1.0);
        let vp = if open {
            // The whole body, head to feet, from a little further back.
            view_proj(
                Vec3::new(DOLL_OPEN_DISTANCE, 0.0, 34.0),
                180.0,
                -1.0,
                aspect,
            )
        } else {
            // In front of the body, at its chest, looking back at it.
            view_proj(Vec3::new(82.0, 0.0, 31.0), 180.0, -3.0, aspect)
        };
        Some(Doll { rect, vp, draws })
    }

    /// The selector's body (CLIENT.md 4.2): the character or the archetype shown, in the
    /// open, with what it wears (the hub's list names the templates), as the zone draws
    /// it; an archetype stands bare, as a new character enters. The selector shows the
    /// weapon in the hand: a showcase, not a body in the world.
    fn doll_on_selector(&mut self, frame_dt: f32, turn: f32) -> Vec<CharacterDraw> {
        let Some(a) = self.active.as_mut() else {
            return Vec::new();
        };
        let Some((build, model, worn)) = self.screens.front.as_ref().and_then(|f| f.shown()) else {
            return Vec::new();
        };
        let frame = gm_model::rig::frame_index(build.frame);
        let place = |i: usize| worn.map(|w| w[i].as_str());
        let prop = template_prop(&mut self.content, Some(a), place(0));
        let off = template_prop(&mut self.content, Some(a), place(1));
        let (overlay, hide, patches) = template_overlay(&mut self.content, Some(a), worn, frame);
        let body = Body {
            key: DOLL,
            origin: Vec3::new(0.0, 0.0, 24.0),
            yaw: 180.0 + turn * 360.0,
            pitch: 0.0,
            anim: gm_core::sim::anim::IDLE,
            crouched: false,
            frame,
            aspects: build.aspects.0,
            status: 0,
            model,
            distance: 0.0,
            lit: 0.0,
            prop,
            off,
            worn: overlay,
            hide,
            patches,
            sheathed: false,
        };
        a.avatars
            .doll(&body, frame_dt, &self.bsp, &a.renderer.characters)
    }

    /// The own body on the equip panel, with what it holds and wears in the zone.
    fn doll_of_own(&mut self, frame_dt: f32, turn: f32) -> Vec<CharacterDraw> {
        let Some(a) = self.active.as_mut() else {
            return Vec::new();
        };
        let Some(o) = self.online.as_ref() else {
            return Vec::new();
        };
        let Some(c) = o.client.as_ref() else {
            return Vec::new();
        };
        let build = &c.sheet.build;
        let look = o.looks.get(&c.my_id).copied().unwrap_or_default();
        let frame = gm_model::rig::frame_index(build.frame);
        let worn = worn_overlay(&mut self.content, Some(a), &o.props, look, frame);
        let body = Body {
            key: DOLL,
            origin: Vec3::new(0.0, 0.0, 24.0),
            yaw: 180.0 + turn * 360.0,
            pitch: 0.0,
            anim: gm_core::sim::anim::IDLE,
            crouched: false,
            frame,
            aspects: build.aspects.0,
            status: 0,
            model: o.names.get(&c.my_id).and_then(|n| n.2),
            distance: 0.0,
            lit: 0.0,
            prop: held_prop(&mut self.content, Some(a), &o.props, look),
            off: off_prop(&mut self.content, Some(a), &o.props, look),
            worn: worn.0,
            hide: worn.1,
            patches: worn.2,
            sheathed: look.sheathed,
        };
        a.avatars
            .doll(&body, frame_dt, &self.bsp, &a.renderer.characters)
    }

    /// The frame rendered and presented. `false` when a bench is still waiting for the
    /// models its crowd wears to be on the GPU: the frame is not counted.
    fn present(
        &mut self,
        frame: wgpu::SurfaceTexture,
        vp: Mat4,
        doll: Option<Doll>,
        bench: bool,
    ) -> bool {
        let Some(a) = &mut self.active else {
            return false;
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(a.view_format),
            ..Default::default()
        });
        let doll = doll.as_ref().map(|d| (d.rect, d.vp, d.draws.as_slice()));
        a.renderer.render_with_doll(
            &a.gpu,
            &view,
            vp,
            &self.scene.entities,
            &a.avatars.draws,
            doll,
        );
        a.avatars.end_frame(&a.gpu, &mut a.renderer.characters);
        a.window.pre_present_notify();
        a.gpu.queue.present(frame);
        self.clock.acquire_timeouts = 0;
        // A bench counts frames once every model its crowd wears is on the GPU.
        !(bench
            && a.avatars.cache.as_ref().is_some_and(|c| c.pending() > 0)
            && self.clock.started.elapsed().as_secs_f32() < 60.0)
    }

    /// The frame counted, the first one's time told (WEB.md 9), and the frame rate
    /// capped on the CPU so an uncapped present mode does not spin the GPU at 100%.
    fn clock_frame(&mut self, now: Instant, bench: bool) {
        self.clock.stats.frame();
        #[cfg(target_arch = "wasm32")]
        if self.web.first_frame_ms == 0.0 {
            self.web.first_frame_ms = web_sys::window()
                .and_then(|w| w.performance())
                .map_or(0.0, |p| p.now());
        }
        #[cfg(not(target_arch = "wasm32"))]
        if !bench && self.opts.max_fps > 0 {
            let budget = std::time::Duration::from_secs_f64(1.0 / self.opts.max_fps as f64);
            let spent = now.elapsed();
            if spent < budget {
                std::thread::sleep(budget - spent);
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = now;
        if bench && self.clock.stats.frames() > 0 && self.clock.stats.frames().is_multiple_of(60) {
            log::debug!("bench frame {}", self.clock.stats.frames());
        }
    }

    /// The window's title once a second: where and who, the bars, the frame rate; and
    /// the settings kept if they changed.
    fn title_frame(&mut self) {
        if self.clock.last_title.elapsed().as_secs_f32() < 1.0 {
            return;
        }
        let Some(a) = &self.active else { return };
        let r = self.clock.stats.report_since(self.clock.title_frame);
        let vp = match self
            .online
            .as_ref()
            .and_then(|o| o.client.as_ref())
            .map(|c| c.sheet.kit.mode)
        {
            Some(mode) => mode.name(),
            None => match self.view.viewport {
                Viewport::First => "1st",
                Viewport::Third => "3rd",
            },
        };
        let title = match &self.online {
            Some(o) => {
                let (hp, max_hp, st, fo, corr, delay) =
                    o.client.as_ref().map_or((0, 0, 0.0, 0.0, 0, 0), |c| {
                        (
                            c.own_health,
                            c.sheet.derived.health,
                            c.mover.stamina,
                            c.mover.focus,
                            c.stats.corrections,
                            c.delay_ticks,
                        )
                    });
                let models = a.avatars.stats();
                format!(
                    "gamengine [{}{} team {} {vp}]  hp {hp}/{max_hp}  st {st:.0}  fo {fo:.0}  k {} d {}  {} players  {} stalls  {} models  delay {delay}  corr {corr}  {:.0} fps  {}",
                    if o.zone_name.is_empty() {
                        String::new()
                    } else {
                        format!("{} ", o.zone_name)
                    },
                    o.build_name,
                    o.team,
                    o.kills,
                    o.deaths,
                    o.names.len(),
                    o.stalls.len(),
                    models.ready,
                    r.fps_avg,
                    o.respec_note
                )
            }
            None => format!(
                "gamengine [{vp}]  {:.0} fps  {:.2} ms  {} / {} faces",
                r.fps_avg, r.ms_avg, a.renderer.faces_drawn, self.world.faces_total
            ),
        };
        a.window.set_title(&title);
        self.clock.title_frame = self.clock.stats.frames();
        self.clock.last_title = Instant::now();
        self.keep_settings();
    }

    /// How a scripted run ends: after its frames (a bench), after its seconds, with the
    /// report line told once a second (`--report`, WEB.md 8) and whole at the end in a
    /// browser.
    fn run_end(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(n) = self.opts.bench_frames
            && self.clock.stats.frames() >= n as usize
        {
            event_loop.exit();
        }
        if self.opts.report && self.clock.last_report.elapsed().as_secs_f32() >= 1.0 {
            self.clock.last_report = Instant::now();
            let line = self.report_line();
            #[cfg(not(target_arch = "wasm32"))]
            println!("stats: {line}");
            #[cfg(target_arch = "wasm32")]
            crate::web::tell_page("stats", &line);
        }
        if self.opts.seconds > 0.0
            && self.clock.started.elapsed().as_secs_f32() >= self.opts.seconds
        {
            #[cfg(target_arch = "wasm32")]
            {
                // The whole run in one line, for whoever scripted it.
                self.clock.report_frame = 0;
                let line = self.report_line();
                if let Some(o) = &mut self.online {
                    o.net.close();
                }
                if let Some(hub) = &self.session.hub {
                    hub.logout();
                }
                crate::web::tell_page("done", &line);
            }
            event_loop.exit();
        }
    }

    /// What the screens, the menu and the chat line asked for in this frame, done.
    fn take_actions(&mut self, actions: Actions, event_loop: &ActiveEventLoop) {
        let [answered, clicked] = actions.front;
        self.bag_act(actions.bag);
        self.smithy_act(actions.smithy);
        self.forge_act(actions.forge);
        self.people_act(actions.people);
        self.gm_act(actions.gm);
        self.character_act(actions.character);
        self.act(answered, actions.menu, actions.said, event_loop);
        self.act(clicked, MenuAction::None, None, event_loop);
    }
}
