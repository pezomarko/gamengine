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
        self.entities.clear();
        self.bodies.clear();
        let outside = self.viewport == Viewport::Third;
        let (eye, yaw, pitch) = p.scene(outside, &mut self.bodies, &mut self.entities);
        if self.viewport == Viewport::Third {
            (third_person_camera(&self.bsp, eye, yaw, pitch), yaw, pitch)
        } else {
            (eye, yaw, pitch)
        }
    }

    pub(super) fn frame(&mut self, event_loop: &ActiveEventLoop) {
        self.poll_bar();
        #[cfg(target_arch = "wasm32")]
        if self.active.is_none() {
            let made = self.pending_gpu.borrow_mut().take();
            match (made, self.window.clone()) {
                (Some(Ok((surface, gpu))), Some(window)) => {
                    if let Err(e) = self.activate(window, surface, gpu) {
                        self.fail(event_loop, &format!("renderer setup failed: {e}"));
                        return;
                    }
                }
                (Some(Err(e)), _) => {
                    self.fail(event_loop, &format!("renderer setup failed: {e}"));
                    return;
                }
                _ => {}
            }
        }
        if let Some((bsp, hash)) = self.pending_map.take() {
            self.switch_map(bsp, hash);
        }
        let now = Instant::now();
        let frame_dt = (now - self.last_frame).as_secs_f32();
        self.last_frame = now;
        #[cfg(not(target_arch = "wasm32"))]
        self.pasted();
        if !self.run_script(event_loop) {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        self.page_frame();
        self.leaving
            .retain(|(net, since)| !net.gone() && since.elapsed().as_secs_f32() < 1.0);
        // A session lives while it is used, and a zone is played without a word to the hub:
        // now and then the client says one (HUB.md 3.1).
        if self.online.is_some()
            && self.hub_touched.elapsed().as_secs() >= SESSION_TOUCH_SECS
            && let (Some(hub), Some(account)) = (&self.hub, &self.account)
        {
            self.hub_touched = Instant::now();
            let session = account.session;
            // Nobody waits for the answer.
            let _ = hub.call(PlayerRequest::ListZones { session });
        }
        let returns = self.returns_to_screens();
        // The pointer follows the screens: free while one is up, the game's again after.
        // Not for a finger (WEB.md 3.5): a phone's browser gives the pointer lock to the
        // tap that put the screen down and takes it back at the next touch, and losing
        // it is Escape, so every tap in the game opened the menu.
        let up = self.screen_up();
        if up != self.was_up {
            self.was_up = up;
            if up {
                self.release_keys();
                self.set_grab(false);
            } else if self.wants_pointer() && self.focused() && !self.fingers.seen {
                self.set_grab(true);
            }
        }
        if self.capturing() {
            // Whatever was held is not held for the game while the toolkit has the keys.
            self.release_keys();
        }
        self.fingers_frame(up);

        // Mouse look is applied per frame for responsiveness; movement uses it at tick time.
        let bench = self.opts.bench_frames.is_some();
        if bench && self.opts.crowd > 0 {
            let t = self.started.elapsed().as_secs_f32();
            self.sim.yaw = self.bench_yaw0 + BENCH_CROWD_SWING_DEG * (t * 0.7).sin();
        } else if bench {
            self.sim.yaw += BENCH_YAW_DEG_PER_S * frame_dt;
        } else if (self.grabbed || self.fingers.turning()) && !up {
            let turn = self.settings.sensitivity / self.zoom.max(1.0);
            let tilt = if self.settings.invert { -turn } else { turn };
            self.sim.yaw -= self.input.mouse_dx * turn;
            let (low, high) = if self.rpg_mode() {
                (5.0, 80.0)
            } else {
                (-89.0, 89.0)
            };
            self.sim.pitch = (self.sim.pitch + self.input.mouse_dy * tilt).clamp(low, high);
        }
        // Taunted (MATRIX.md 8): the view is turned to the taunter with the body and held
        // there; the mouse moves nothing until the taunt is out.
        if let Some(c) = self.online.as_ref().and_then(|o| o.client.as_ref())
            && c.mover.statuses.has(gm_core::vocab::Status::Taunt)
            && c.mover.lock_yaw.is_some()
        {
            self.sim.yaw = c.mover.yaw;
        }
        self.sim.yaw = self.sim.yaw.rem_euclid(360.0);
        self.input.mouse_dx = 0.0;
        self.input.mouse_dy = 0.0;

        #[cfg(not(target_arch = "wasm32"))]
        let watching = self.playback.is_some();
        #[cfg(target_arch = "wasm32")]
        let watching = false;
        let (camera, cam_yaw, cam_pitch) = if watching {
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.playback_frame(frame_dt)
            }
            #[cfg(target_arch = "wasm32")]
            unreachable!()
        } else if self.online.is_some() {
            let camera = match self.online_frame(frame_dt, event_loop) {
                Some(cam) => cam,
                None => return,
            };
            // The zone has the character: the screens step aside.
            if self.front_up && self.online.as_ref().is_some_and(|o| o.client.is_some()) {
                self.front_up = false;
                self.chat.clear();
            }
            // Its connection ended and a person is here: back to the characters.
            if let Some(why) = self.zone_ended.take() {
                self.leave_zone(&why);
            }
            camera
        } else if self.front_up || self.title.is_some() {
            // Behind a screen with no zone: the map, turning slowly (CLIENT.md 2).
            self.entities.clear();
            self.bodies.clear();
            let yaw = self.bench_yaw0 + BACKDROP_DEG_PER_S * self.started.elapsed().as_secs_f32();
            (self.sim.eye(), yaw.rem_euclid(360.0), 0.0)
        } else {
            let walk = self.opts.script.as_deref() == Some("walk");
            let input = if bench {
                MoveInput {
                    yaw: self.sim.yaw,
                    ..Default::default()
                }
            } else if walk {
                // A second standing, five seconds forward, standing again (SOUND.md 7):
                // what the sound gate renders to a file.
                let t = self.started.elapsed().as_secs_f32();
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
            // Heard: the own body's steps and landings, offline too (its animation is
            // the local mover's; nobody else is here).
            {
                let v = self.sim.curr.velocity;
                let anim = if !self.sim.curr.on_ground {
                    gm_core::sim::anim::AIR
                } else if v.truncate().length() > 20.0 {
                    gm_core::sim::anim::RUN
                } else {
                    gm_core::sim::anim::IDLE
                };
                let here = self.sim.origin();
                let travel = match (self.sim.curr.on_ground, self.sound_from) {
                    (true, Some(from)) => (here - from).truncate().length(),
                    _ => 0.0,
                };
                self.sound_from = Some(here);
                let me = crate::sound::cues::OwnNow {
                    id: 0,
                    pos: self.sim.origin(),
                    on_ground: self.sim.curr.on_ground,
                    travel,
                    health: 0,
                    alive: true,
                };
                let now = self.started.elapsed().as_secs_f32();
                // (No zone here: the word's tick is the frame clock at 64 Hz.)
                self.sound.own(now, me, &[((now * 64.0) as u32, anim)], &[]);
                let listener = crate::sound::Listener {
                    pos: self.sim.eye(),
                    yaw: self.sim.yaw.to_radians(),
                };
                self.sound.end_frame(frame_dt, listener);
            }
            self.entities.clear();
            self.bodies.clear();
            match self.viewport {
                Viewport::First => {
                    // The fitting room of the view model (`--prop KEY`, LOOK.md 6.4):
                    // the prop in the view with its stride's bob; R held works a reload
                    // over and over, to see it.
                    let eye = self.sim.eye();
                    self.view_stride +=
                        self.sim.curr.velocity.truncate().length() * frame_dt / 64.0;
                    let t = self.started.elapsed().as_secs_f32();
                    let reload = if self.input.down(KeyCode::KeyR) {
                        (t / 2.4).fract()
                    } else {
                        0.0
                    };
                    self.view_model = self.offline_prop.map(|slot| ViewModel {
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
                            self.view_stride
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
                _ => {
                    let v = self.sim.curr.velocity;
                    self.bodies.push(Body {
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
                        crouched: self.sim.curr.on_ground
                            && (self.input.down(KeyCode::ControlLeft)
                                || self.input.down(KeyCode::ControlRight)
                                || self.input.down(KeyCode::KeyC)),
                        frame: 1,
                        armour: 0,
                        aspects: 0,
                        status: 0,
                        model: None,
                        distance: 0.0,
                        lit: 0.0,
                        // Offline (`--prop FILE`, CONTENT.md 9): the prop to look at.
                        prop: self.offline_prop,
                        off: self.offline_off,
                        worn: crate::avatars::places(&self.offline_worn).0,
                        hide: crate::avatars::places(&self.offline_worn).1,
                        sheathed: self.offline_sheathed,
                    });
                    (
                        third_person_camera(
                            &self.bsp,
                            self.sim.eye(),
                            self.sim.yaw,
                            self.sim.pitch,
                        ),
                        self.sim.yaw,
                        self.sim.pitch,
                    )
                }
            }
        };

        // Whether the character's page may wear its draft here, read before the renderer
        // is borrowed for the frame.
        let at_trainer = self.at_trainer();
        // The UI's scale this frame, and the controls a finger may land on (MODES.md
        // 5.6), which the HUD draws where they are hit: read before the renderer is
        // borrowed too.
        let chosen = self.ui_scale_choice();
        self.touch_buttons = if self.fingers.seen && !watching && !bench && !self.screen_up() {
            let size = self.active.as_ref().map_or((1.0, 1.0), |a| {
                (a.config.width.max(1) as f32, a.config.height.max(1) as f32)
            });
            touch_controls(size, self.ui_scale(), self.online.as_ref(), self.rpg_mode())
        } else {
            Vec::new()
        };
        let Some(a) = &mut self.active else { return };
        // The world is drawn from the camera's leaf.
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

        let frame = match a.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f) => f,
            wgpu::CurrentSurfaceTexture::Suboptimal(f) => {
                log::debug!("surface suboptimal; reconfiguring");
                a.gpu.queue.present(f);
                self.configure_surface();
                return;
            }
            wgpu::CurrentSurfaceTexture::Timeout => {
                self.acquire_timeouts += 1;
                if self.acquire_timeouts == 3 {
                    log::warn!(
                        "the display is not releasing frames (Fifo on a compositor, or a sleeping monitor); try --present mailbox"
                    );
                }
                return;
            }
            wgpu::CurrentSurfaceTexture::Occluded => {
                log::debug!("surface occluded");
                return;
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                log::debug!("surface outdated or lost; reconfiguring");
                self.configure_surface();
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                log::error!("surface validation error");
                event_loop.exit();
                return;
            }
        };
        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(a.view_format),
            ..Default::default()
        });
        let aspect = a.config.width as f32 / a.config.height.max(1) as f32;
        for id in self.revoked.drain(..) {
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
            self.content
                .poll(|model| Some(characters.add_model(gpu, model)));
        }
        // The official avatars of the bundle (CONTENT.md 3.2) stand in for the mannequins
        // of their frame and armour class: natively a file is read the first time it is
        // asked for, in a browser it is fetched like a prop and lands through `poll`.
        {
            let (gpu, characters) = (&a.gpu, &mut a.renderer.characters);
            for frame in 0..4u8 {
                for armour in 0..4u8 {
                    let slot = self.content.avatar(frame, armour, |model| {
                        Some(characters.add_model(gpu, model))
                    });
                    a.avatars.set_official(frame, armour, slot);
                }
            }
        }
        a.avatars.begin_frame();
        if let Some(v) = self.view_model.take() {
            a.avatars.view_model(
                v.slot, v.fit, v.eye, v.yaw, v.pitch, v.stride, v.kick, v.swing, v.reload, v.light,
            );
        }
        for body in &self.bodies {
            a.avatars.push(
                body,
                frame_dt,
                &self.bsp,
                &a.renderer.characters,
                &mut self.entities,
            );
            // The ring of its aspects at its feet (MODELS.md 9, LOOK.md 13).
            if body.anim != gm_core::sim::anim::DEAD && body.aspects != 0 {
                crate::fx::aspect_ring(
                    &mut self.fx,
                    body.origin + Vec3::Z * Hull::Player.mins().z,
                    body.yaw,
                    body.aspects,
                );
            }
        }
        // The frame's effects go to the renderer, which draws them after the bodies.
        a.renderer.fx.verts.append(&mut self.fx.verts);
        a.avatars.push_crowd(
            self.started.elapsed().as_secs_f32(),
            frame_dt,
            camera,
            &self.bsp,
            &a.renderer.characters,
            &mut self.entities,
        );
        let vp = crate::render::view_proj_zoomed(camera, cam_yaw, cam_pitch, aspect, self.zoom);
        self.last_vp = Some((
            vp,
            (a.config.width.max(1) as f32, a.config.height.max(1) as f32),
        ));
        // One scale for the HUD and the screens: what the window gives, or what was chosen;
        // and the atlas made for that scale (LOOK.md 2.2), from the frame it is here.
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
        if !watching && !bench && !self.front_up && self.title.is_none() {
            build_hud(
                &mut a.renderer.hud,
                self.online.as_ref(),
                vp,
                HudView {
                    tags: &self.tags,
                    pops: &self.pops,
                    first_person: self.viewport == Viewport::First,
                    hurt: self.effects.own_hurt,
                    squad: &self.squad_view,
                    party: &self.party_view,
                    target: self.target_view.as_ref(),
                    scale,
                    manifest: self.content.manifest.as_ref(),
                    time: self.started.elapsed().as_secs_f32(),
                    own_name: &self.opts.character,
                    combo: (
                        self.combo.0,
                        self.combo.1.map_or(f32::MAX, |t| t.elapsed().as_secs_f32()),
                    ),
                    crouched: self.input.down(KeyCode::ControlLeft)
                        || self.input.down(KeyCode::ControlRight)
                        || self.input.down(KeyCode::KeyC),
                    zoom: self.zoom,
                    scoped: self.input.scoped,
                    touch: TouchHud {
                        buttons: &self.touch_buttons,
                        held: self
                            .touch_buttons
                            .iter()
                            .map(|(b, _)| *b)
                            .filter(|b| self.fingers.holds(*b))
                            .collect(),
                        stick: self.fingers.stick_drawn(),
                    },
                },
            );
        }
        // The screens, the menu and the chat, over the HUD (CLIENT.md 4 to 6).
        let mut front_actions = [Action::None, Action::None];
        let (mut menu_action, mut said) = (MenuAction::None, None);
        let mut bag_action = BagAction::None;
        let mut people_action = PeopleAction::None;
        let mut gm_action = GmAction::None;
        let mut character_action = CharacterAction::None;
        if !bench {
            let playing = self.online.as_ref().is_some_and(|o| o.client.is_some());
            let screen = match (&self.front, &self.menu, &self.bag) {
                _ if self.title.is_some() => "title",
                (Some(front), _, _) if self.front_up => front.screen.name(),
                (_, Some(menu), _) => menu.page.name(),
                (_, _, Some(bag)) => bag.name(),
                _ if self.people.is_some() => self.people.as_ref().map_or("", |p| p.page.name()),
                _ if self.gm_page.is_some() => self.gm_page.as_ref().map_or("", |g| g.name()),
                _ if self.character_page.is_some() => {
                    self.character_page.as_ref().map_or("", |p| p.name())
                }
                _ if self.chat.open => "chat",
                _ => "game",
            };
            // Whose stall the body stands at, for the corner of the screen.
            let near = Self::stall_in_reach(self.online.as_ref())
                .filter(|_| playing && self.hub.is_some())
                .map(|s| s.owner.clone());
            let me = self.opts.name.clone();
            let keeps_stall = self
                .online
                .as_ref()
                .is_some_and(|o| o.stalls.iter().any(|s| s.owner == me));
            self.ui_input.last_cursor = self.ui_input.cursor;
            self.ui_input.cursor = self.cursor;
            self.ui_input.time = self.started.elapsed().as_secs_f32();
            self.ui.paperdolls.clear();
            let mut ui = Ui::begin_at(
                &mut a.renderer.hud,
                &mut self.ui,
                &self.ui_input,
                screen,
                PANEL_UNITS,
                chosen,
            );
            if let Some(title) = &self.title {
                match crate::menu::title(&mut ui, title) {
                    Some(true) => self.title = None,
                    Some(false) => front_actions[1] = Action::Quit,
                    None => {}
                }
            } else if self.front_up {
                if let Some(front) = &mut self.front {
                    front_actions = front.frame(&mut ui);
                }
            } else {
                if playing {
                    said = self.chat.frame(&mut ui, &mut self.settings.ignored);
                } else if self.chat.open {
                    // Between two zones nothing is drawn of it: nor does it keep the keys.
                    self.chat.drop_line();
                }
                // The game goes away under an open inventory as under the menu; a body
                // that stopped playing (a travel, the zone gone) has neither.
                if !playing {
                    self.bag = None;
                    self.people = None;
                    self.gm_page = None;
                }
                // Who wears it, for the equipment's ribbon (EQUIPMENT.md 4.1): the
                // class the content names, and the build.
                let (class, ribbon) = self
                    .online
                    .as_ref()
                    .and_then(|o| o.client.as_ref().map(|c| (&c.sheet.build, o.pack.as_ref())))
                    .map(|(build, pack)| {
                        let class = crate::showcase::class_name(pack, build);
                        let ribbon = crate::showcase::short_ribbon(&class, build);
                        (class, ribbon)
                    })
                    .unwrap_or_default();
                match (&mut self.bag, &self.hub) {
                    (Some(bag), Some(hub)) if self.menu.is_none() => {
                        let looks = crate::bag::ItemLooks {
                            manifest: self.content.manifest.as_ref(),
                        };
                        let wearer = crate::bag::Wearer {
                            name: &me,
                            ribbon: &ribbon,
                            class: &class,
                        };
                        bag_action = bag.frame_with(
                            &mut ui,
                            hub,
                            keeps_stall,
                            Instant::now(),
                            looks,
                            wearer,
                        );
                    }
                    _ => {}
                }
                // The people here and of the party, a trade, the tavern (PARTY.md 8).
                if let (Some(people), Some(o), None, None) =
                    (&mut self.people, &mut self.online, &self.menu, &self.bag)
                    && let Some(c) = &o.client
                {
                    // Who is here: the bodies that are people, and whether each stands
                    // near enough to trade with (the rule the zone decides with).
                    let mine: [f32; 3] = c.mover.mv.origin.into();
                    let others = c.others_at(c.render_tick(0.0));
                    let mut here: Vec<Here> = o
                        .names
                        .iter()
                        .filter(|(id, _)| {
                            **id != c.my_id && o.kinds.get(id) == Some(&BodyKind::Human)
                        })
                        .map(|(id, (name, _, _))| Here {
                            body: *id,
                            name: name.clone(),
                            near: others.iter().find(|e| e.id == *id).is_some_and(|e| {
                                gm_net::control::trade_in_reach(mine, e.pos.into())
                            }),
                        })
                        .collect();
                    here.sort_by(|a, b| a.name.cmp(&b.name));
                    let hub = self
                        .hub
                        .as_ref()
                        .filter(|_| self.account.is_some() && self.character.is_some())
                        .map(|hub| hub as &dyn HubApi);
                    let unix = web_time::SystemTime::now()
                        .duration_since(web_time::UNIX_EPOCH)
                        .map_or(0, |d| d.as_secs());
                    let word = self.chat.zone_said(std::time::Duration::from_secs(8));
                    people_action = people.frame_with(
                        &mut ui,
                        hub,
                        &me,
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
                // The game master's page (GM.md 4).
                if let (Some(page), Some(o), None, None, None) = (
                    &mut self.gm_page,
                    &self.online,
                    &self.menu,
                    &self.bag,
                    &self.people,
                ) && let (Some(c), Some(pack)) = (&o.client, &o.pack)
                {
                    let view = GmView {
                        pack,
                        own: &c.sheet.build,
                        tuning: &o.tuning,
                        rate: o.rate,
                        note: &o.gm_note,
                    };
                    gm_action = page.frame(&mut ui, &view);
                }
                // The character's page (MATRIX.md 9.1).
                if let (Some(page), Some(o), None, None, None, None) = (
                    &mut self.character_page,
                    &self.online,
                    &self.menu,
                    &self.bag,
                    &self.people,
                    &self.gm_page,
                ) && let (Some(c), Some(pack)) = (&o.client, &o.pack)
                {
                    let view = CharacterView {
                        pack,
                        own: &c.sheet.build,
                        rate: o.rate,
                        at_trainer,
                        note: &o.respec_note,
                    };
                    character_action = page.frame(&mut ui, &view);
                }
                match &mut self.menu {
                    Some(_) if self.bag.is_some() => {}
                    Some(menu) => {
                        let hub = self
                            .hub
                            .as_ref()
                            .zip(self.account.as_ref())
                            .map(|(hub, account)| (hub as &dyn HubApi, account.session));
                        // A page's tab is closed and made fullscreen by the browser.
                        let offers = Offers {
                            inventory: playing && hub.is_some() && self.character.is_some(),
                            equipment: playing && hub.is_some() && self.character.is_some(),
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
                        menu_action = menu.frame(&mut ui, hub, offers, here, &mut self.settings);
                    }
                    None if self.bag.is_some()
                        || self.people.is_some()
                        || self.gm_page.is_some()
                        || self.character_page.is_some() => {}
                    None if playing && !self.chat.open => {
                        // Where this is, and the two keys nothing else tells of.
                        let here = self.online.as_ref().map_or("", |o| o.zone_name.as_str());
                        let hint = if here.is_empty() {
                            "Esc menu  Enter chat".to_string()
                        } else {
                            format!("{here}  Esc menu  Enter chat")
                        };
                        let (w, h) = ui.size();
                        ui.small(w - 16.0, h - 14.0 * ui.scale, ui::FAINT, &hint);
                        // And the stall the body stands at, whose it is and the key.
                        if let Some(owner) = &near {
                            let look = format!("{owner}'s stall  E look");
                            ui.small(w - 16.0, h - 24.0 * ui.scale, ui::TEXT, &look);
                        }
                    }
                    None => {}
                }
            }
            ui.end();
            if std::mem::take(&mut self.ui.presses) > 0 {
                self.sound.play(crate::sound::synth::Cue::Click);
            }
            self.ui_drawn = true;
            // What happened is used up; a button still held is still held.
            self.ui_input = UiInput {
                down: self.ui_input.down,
                // The pointer stays where it is: the paperdoll turns by how far it moved
                // since last frame, not by where it is.
                cursor: self.ui_input.cursor,
                ..Default::default()
            };
        }
        // The paperdoll (LOOK.md 5): the own body as the equip panel shows it, idle,
        // turned by the drag across it, with what it holds, drawn into the panel's
        // rectangle by a camera of its own.
        let doll_draws: Vec<CharacterDraw> = match (self.ui.paperdolls.first(), &self.online) {
            // The selector's body (CLIENT.md 4.2): the character or the archetype shown,
            // in the open, with what it wears (the hub's list names the templates), as
            // the zone draws it; an archetype stands bare, as a new character enters.
            (Some(doll), _) if self.front_up && doll.rect.w > 1.0 && doll.rect.h > 1.0 => {
                match self.front.as_ref().and_then(|f| f.shown()) {
                    Some((build, model, worn)) => {
                        let frame = gm_model::rig::frame_index(build.frame);
                        let place = |i: usize| worn.map(|w| w[i].as_str());
                        let prop = template_prop(&mut self.content, Some(a), place(0));
                        let off = template_prop(&mut self.content, Some(a), place(1));
                        let (overlay, hide) =
                            template_overlay(&mut self.content, Some(a), worn, frame);
                        let body = Body {
                            key: DOLL,
                            origin: Vec3::new(0.0, 0.0, 24.0),
                            yaw: 180.0 + doll.turn * 360.0,
                            pitch: 0.0,
                            anim: gm_core::sim::anim::IDLE,
                            crouched: false,
                            frame,
                            armour: build.armour as u8,
                            aspects: build.aspects.0,
                            status: 0,
                            model,
                            distance: 0.0,
                            lit: 0.0,
                            prop,
                            off,
                            worn: overlay,
                            hide,
                            // The selector shows the weapon in the hand (CLIENT.md 4.2):
                            // a showcase, not a body in the world.
                            sheathed: false,
                        };
                        a.avatars
                            .doll(&body, frame_dt, &self.bsp, &a.renderer.characters)
                    }
                    None => Vec::new(),
                }
            }
            (Some(doll), Some(o)) if doll.rect.w > 1.0 && doll.rect.h > 1.0 => match &o.client {
                Some(c) => {
                    let build = &c.sheet.build;
                    let worn = worn_overlay(
                        &mut self.content,
                        Some(a),
                        &o.props,
                        o.looks.get(&c.my_id).copied().unwrap_or_default(),
                        gm_model::rig::frame_index(build.frame),
                    );
                    let body = Body {
                        key: DOLL,
                        origin: Vec3::new(0.0, 0.0, 24.0),
                        yaw: 180.0 + doll.turn * 360.0,
                        pitch: 0.0,
                        anim: gm_core::sim::anim::IDLE,
                        crouched: false,
                        frame: gm_model::rig::frame_index(build.frame),
                        armour: build.armour as u8,
                        aspects: build.aspects.0,
                        status: 0,
                        model: o.names.get(&c.my_id).and_then(|n| n.2),
                        distance: 0.0,
                        lit: 0.0,
                        prop: held_prop(
                            &mut self.content,
                            Some(a),
                            &o.props,
                            o.looks.get(&c.my_id).copied().unwrap_or_default(),
                        ),
                        off: off_prop(
                            &mut self.content,
                            Some(a),
                            &o.props,
                            o.looks.get(&c.my_id).copied().unwrap_or_default(),
                        ),
                        worn: worn.0,
                        hide: worn.1,
                        sheathed: o.looks.get(&c.my_id).is_some_and(|l| l.sheathed),
                    };
                    a.avatars
                        .doll(&body, frame_dt, &self.bsp, &a.renderer.characters)
                }
                None => Vec::new(),
            },
            _ => Vec::new(),
        };
        let doll = self.ui.paperdolls.first().map(|d| {
            let aspect = d.rect.w / d.rect.h.max(1.0);
            let vp = if d.open {
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
            (d.rect, vp, doll_draws.as_slice())
        });
        a.renderer
            .render_with_doll(&a.gpu, &view, vp, &self.entities, &a.avatars.draws, doll);
        a.avatars.end_frame(&a.gpu, &mut a.renderer.characters);
        a.window.pre_present_notify();
        a.gpu.queue.present(frame);
        self.acquire_timeouts = 0;
        // A bench counts frames once every model its crowd wears is on the GPU.
        if bench
            && a.avatars.cache.as_ref().is_some_and(|c| c.pending() > 0)
            && self.started.elapsed().as_secs_f32() < 60.0
        {
            return;
        }
        self.stats.frame();
        #[cfg(target_arch = "wasm32")]
        if self.first_frame_ms == 0.0 {
            self.first_frame_ms = web_sys::window()
                .and_then(|w| w.performance())
                .map_or(0.0, |p| p.now());
        }
        #[cfg(not(target_arch = "wasm32"))]
        if !bench && self.opts.max_fps > 0 {
            // Cheap CPU-side cap so an uncapped present mode does not spin the GPU at 100%.
            let budget = std::time::Duration::from_secs_f64(1.0 / self.opts.max_fps as f64);
            let spent = now.elapsed();
            if spent < budget {
                std::thread::sleep(budget - spent);
            }
        }
        if bench && self.stats.frames() > 0 && self.stats.frames().is_multiple_of(60) {
            log::debug!("bench frame {}", self.stats.frames());
        }

        if self.last_title.elapsed().as_secs_f32() >= 1.0 {
            let r = self.stats.report_since(self.title_frame);
            let vp = match self
                .online
                .as_ref()
                .and_then(|o| o.client.as_ref())
                .map(|c| c.sheet.kit.mode)
            {
                Some(mode) => mode.name(),
                None => match self.viewport {
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
                    r.fps_avg, r.ms_avg, a.renderer.faces_drawn, self.faces_total
                ),
            };
            a.window.set_title(&title);
            self.title_frame = self.stats.frames();
            self.last_title = Instant::now();
            self.keep_settings();
        }
        if let Some(n) = self.opts.bench_frames
            && self.stats.frames() >= n as usize
        {
            event_loop.exit();
        }
        if self.opts.report && self.last_report.elapsed().as_secs_f32() >= 1.0 {
            self.last_report = Instant::now();
            let line = self.report_line();
            #[cfg(not(target_arch = "wasm32"))]
            println!("stats: {line}");
            #[cfg(target_arch = "wasm32")]
            crate::web::tell_page("stats", &line);
        }
        if self.opts.seconds > 0.0 && self.started.elapsed().as_secs_f32() >= self.opts.seconds {
            #[cfg(target_arch = "wasm32")]
            {
                // The whole run in one line, for whoever scripted it.
                self.report_frame = 0;
                let line = self.report_line();
                if let Some(o) = &mut self.online {
                    o.net.close();
                }
                if let Some(hub) = &self.hub {
                    hub.logout();
                }
                crate::web::tell_page("done", &line);
            }
            event_loop.exit();
        }
        let [answered, clicked] = front_actions;
        self.bag_act(bag_action);
        self.people_act(people_action);
        self.gm_act(gm_action);
        self.character_act(character_action);
        self.act(answered, menu_action, said, event_loop);
        self.act(clicked, MenuAction::None, None, event_loop);
    }
}
