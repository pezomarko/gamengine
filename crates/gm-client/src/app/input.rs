use super::*;

#[derive(Default)]
pub(super) struct Input {
    pub(super) keys: HashSet<KeyCode>,
    /// Keys pressed since the last simulation tick (consumed by the next tick).
    pub(super) just_pressed: HashSet<KeyCode>,
    pub(super) mouse: HashSet<MouseButton>,
    pub(super) mouse_dx: f32,
    pub(super) mouse_dy: f32,
    /// A finger's stick (MODES.md 5.6): forward and side, while one holds it; the keys
    /// are taken first.
    pub(super) stick: Option<(f32, f32)>,
    /// The gun mode (MODES.md 3.7): the weapon in hand (0 the gun, 1 the pistol, 2 the
    /// knife) and whether the scope is up (the secondary button toggles it).
    pub(super) held: u8,
    pub(super) scoped: bool,
}

impl Input {
    pub(super) fn down(&self, k: KeyCode) -> bool {
        self.keys.contains(&k)
    }

    pub(super) fn axes(&self) -> (f32, f32) {
        let axis = |neg, pos| (self.down(pos) as i32 - self.down(neg) as i32) as f32;
        let keys = (
            axis(KeyCode::KeyS, KeyCode::KeyW) + axis(KeyCode::ArrowDown, KeyCode::ArrowUp),
            axis(KeyCode::KeyA, KeyCode::KeyD) + axis(KeyCode::ArrowLeft, KeyCode::ArrowRight),
        );
        match self.stick {
            Some(stick) if keys == (0.0, 0.0) => stick,
            _ => keys,
        }
    }

    pub(super) fn move_input(&self, yaw: f32) -> MoveInput {
        let (forward, side) = self.axes();
        MoveInput {
            yaw,
            forward,
            side,
            jump: self.down(KeyCode::Space),
        }
    }

    /// The frame's input. `dodge` is the active slot (1-based) Space plays instead of a
    /// jump (MODES.md 4.6): the kit's dash, while it is ready. In the gun mode (MODES.md
    /// 3.7) `1 2 3` take the gun, the pistol and the knife in hand, `4`–`7` are the
    /// actives, Ctrl crouches, Shift walks, `R` reloads and the secondary button is the
    /// scope.
    pub(super) fn sim_input(
        &mut self,
        yaw: f32,
        pitch: f32,
        dodge: Option<u8>,
        gun: bool,
        rpg: Option<crate::rpg::RpgFrame>,
    ) -> SimInput {
        // The item bar's keys (LOOK.md 3.2, MODES.md 11.3) use a cell in every mode: read
        // before the modes part. `F` is the first cell's, the kit's by default.
        let (kit, use_slot) = match BAR_KEY_CODES
            .iter()
            .position(|k| self.just_pressed.contains(k))
        {
            Some(i) => (buttons::USE, i as u8 + 1),
            None => (0, 0),
        };
        // The RPG mode (MODES.md 5.5): `1` the primary, `2` the secondary, `3`–`6` the
        // actives, Shift guards, Space jumps; the axes are the walk's when none is held.
        if let Some(r) = rpg {
            let mut b = r.buttons | kit;
            if self.down(KeyCode::Space) {
                b |= buttons::JUMP;
            }
            if self.down(KeyCode::ShiftLeft) || self.down(KeyCode::ShiftRight) {
                b |= buttons::GUARD;
            }
            self.just_pressed.clear();
            return SimInput {
                buttons: b,
                yaw,
                pitch,
                forward: r.forward,
                side: r.side,
                ability: r.ability,
                held: 0,
                target: r.target,
                use_slot,
            };
        }
        let (mut forward, mut side) = self.axes();
        let mut b = kit;
        let dodged = dodge.is_some() && self.just_pressed.contains(&KeyCode::Space);
        if self.down(KeyCode::Space) && dodge.is_none() {
            b |= buttons::JUMP;
        }
        if self.just_pressed.contains(&KeyCode::KeyR) {
            b |= buttons::RELOAD;
        }
        if self.mouse.contains(&MouseButton::Left) {
            b |= buttons::PRIMARY;
        }
        let ctrl = self.down(KeyCode::ControlLeft)
            || self.down(KeyCode::ControlRight)
            || self.down(KeyCode::KeyC);
        let shift = self.down(KeyCode::ShiftLeft) || self.down(KeyCode::ShiftRight);
        let mut ability;
        if gun {
            for (i, k) in [KeyCode::Digit1, KeyCode::Digit2, KeyCode::Digit3]
                .iter()
                .enumerate()
            {
                if self.just_pressed.contains(k) {
                    self.held = i as u8;
                    self.scoped = false;
                }
            }
            ability = [
                KeyCode::Digit4,
                KeyCode::Digit5,
                KeyCode::Digit6,
                KeyCode::Digit7,
            ]
            .iter()
            .position(|k| self.just_pressed.contains(k))
            .map_or(0, |i| i as u8 + 1);
            if ctrl {
                b |= buttons::CROUCH;
            }
            if shift {
                forward *= 0.5;
                side *= 0.5;
            }
            if self.scoped {
                b |= buttons::SCOPE;
            }
        } else {
            if self.mouse.contains(&MouseButton::Right) {
                b |= buttons::SECONDARY;
            }
            // Guard is on C as well: a browser keeps Ctrl+W for itself (WEB.md 3.4).
            if ctrl {
                b |= buttons::GUARD;
            }
            if shift {
                b |= buttons::ABILITY1;
            }
            ability = [
                KeyCode::Digit1,
                KeyCode::Digit2,
                KeyCode::Digit3,
                KeyCode::Digit4,
            ]
            .iter()
            .position(|k| self.just_pressed.contains(k))
            .map_or(0, |i| i as u8 + 1);
        }
        if dodged && let Some(d) = dodge {
            ability = d;
        }
        self.just_pressed.clear();
        SimInput {
            buttons: b,
            yaw,
            pitch,
            forward,
            side,
            ability,
            use_slot,
            held: if gun { self.held } else { 0 },
            target: 0,
        }
    }
}

/// How long the HUD says why a press of an item key used nothing (MODES.md 11.3).
pub(super) const KIT_NOTE_SECS: f32 = 1.0;

/// The zone's own refusal of a kit (MODES.md 11.3), in the HUD's words.
pub(super) const KIT_FULL: &str = "at full health";

/// The item bar's keys (LOOK.md 3.2), a cell each: free in every mode (the gun's weapons
/// take `1`–`3` and its actives `4`–`7`, the RPG's kit `1`–`6`). `F` is the first cell's,
/// so `F` uses a kit as it always did, the kit sitting there unless moved.
pub(crate) const BAR_KEYS: [&str; gm_core::sim::BAR_CELLS] = ["F", "8", "9", "0"];

pub(super) const BAR_KEY_CODES: [KeyCode; gm_core::sim::BAR_CELLS] = [
    KeyCode::KeyF,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::Digit0,
];

/// The refusal the item cells show (MODES.md 11.3): the last press's words, within
/// `KIT_NOTE_SECS` of it.
pub(super) fn kit_note_of(note: Option<(Instant, &'static str)>) -> Option<&'static str> {
    note.filter(|(at, _)| at.elapsed().as_secs_f32() < KIT_NOTE_SECS)
        .map(|(_, why)| why)
}

impl App {
    /// Keys held when the toolkit takes the keyboard are not held for the game any more.
    pub(super) fn release_keys(&mut self) {
        self.input.keys.clear();
        self.input.just_pressed.clear();
        self.input.mouse.clear();
    }

    /// A left click in the RPG mode (MODES.md 5.2, 5.5): a target, the primary at the
    /// target, or a walk.
    pub(super) fn rpg_click(&mut self) {
        let Some((vp, size)) = self.last_vp else {
            return;
        };
        let Some((from, dir)) = crate::rpg::Rpg::ray(vp, size, self.cursor) else {
            return;
        };
        let bodies = self.rpg_bodies();
        self.rpg.click(&self.bsp, from, dir, &bodies);
    }

    /// The body under a pixel in the RPG mode, by the last frame's view.
    pub(super) fn rpg_under(&self, cursor: (f32, f32)) -> Option<u32> {
        let (vp, size) = self.last_vp?;
        let (from, dir) = crate::rpg::Rpg::ray(vp, size, cursor)?;
        crate::rpg::Rpg::pick(&self.bsp, from, dir, &self.rpg_bodies())
    }

    /// The right button let go in the RPG mode: a tap on the target (no drag of the
    /// orbit, quick) is the secondary at it (MODES.md 5.5); a drag was the camera's.
    pub(super) fn rpg_right_release(&mut self) {
        let Some((at, cursor, yaw, pitch)) = self.rpg_right.take() else {
            return;
        };
        let quick = at.elapsed().as_secs_f32() < DOUBLE_CLICK_SECS;
        let turned = (self.sim.yaw - yaw).abs() > 1.0 || (self.sim.pitch - pitch).abs() > 1.0;
        if quick && !turned && self.rpg.hover(self.rpg_under(cursor)) == crate::rpg::Hover::Attack {
            self.rpg.ask(crate::rpg::Act::Secondary);
        }
    }

    /// What a finger landing at `at` is for (MODES.md 5.6): the pointer while a screen
    /// is up; a control it lands on; the world in the RPG mode; else the stick on the
    /// left of the frame and the look on the right.
    pub(super) fn touch_zone(&self, at: (f32, f32)) -> Zone {
        if self.screen_up() {
            return Zone::Ui;
        }
        if let Some((b, _)) = self.touch_buttons.iter().find(|(_, r)| r.contains(at)) {
            return Zone::Button(*b);
        }
        if self.rpg_mode() {
            return Zone::World;
        }
        let w = self.active.as_ref().map_or(0.0, |a| a.config.width as f32);
        if at.0 < w * touch::STICK_SHARE {
            Zone::Stick
        } else {
            Zone::Look
        }
    }

    /// A mouse button pressed in the game with the pointer held: the secondary toggles
    /// the scope of a scoped firearm (MODES.md 3.2).
    pub(super) fn press_in_game(&mut self, button: MouseButton) {
        if self.input.mouse.insert(button) && button == MouseButton::Right && self.gun_scope() > 1 {
            self.input.scoped = !self.input.scoped;
        }
    }

    /// A key held by a finger (a control, a cell of the hotbar).
    pub(super) fn hold_key(&mut self, code: KeyCode, down: bool) {
        if down {
            if self.input.keys.insert(code) {
                self.input.just_pressed.insert(code);
            }
        } else {
            self.input.keys.remove(&code);
        }
    }

    /// A control pressed or let go by a finger (MODES.md 5.6).
    pub(super) fn touch_button(&mut self, button: TouchButton, down: bool) {
        match button {
            TouchButton::Menu => {
                if down {
                    self.ui_key(Key::Escape);
                }
            }
            TouchButton::Jump => self.hold_key(KeyCode::Space, down),
            TouchButton::Secondary => {
                if self.rpg_mode() {
                    if down {
                        self.rpg.ask(crate::rpg::Act::Secondary);
                    }
                } else if down {
                    self.press_in_game(MouseButton::Right);
                } else {
                    self.input.mouse.remove(&MouseButton::Right);
                }
            }
            TouchButton::Hot(i) => {
                let key = self
                    .online
                    .as_ref()
                    .and_then(|o| hotbar(o).get(i as usize).map(|c| c.key));
                match key {
                    Some("Shift") => self.hold_key(KeyCode::ShiftLeft, down),
                    Some("C") => self.hold_key(KeyCode::KeyC, down),
                    Some("LMB") if down => self.press_in_game(MouseButton::Left),
                    Some("LMB") => {
                        self.input.mouse.remove(&MouseButton::Left);
                    }
                    Some("RMB") if down => self.press_in_game(MouseButton::Right),
                    Some("RMB") => {
                        self.input.mouse.remove(&MouseButton::Right);
                    }
                    // An item cell (LOOK.md 3.2): its key.
                    Some(k) if BAR_KEYS.contains(&k) => {
                        if let Some(i) = BAR_KEYS.iter().position(|b| *b == k) {
                            self.hold_key(BAR_KEY_CODES[i], down);
                        }
                    }
                    Some(k) => {
                        let digits = [
                            KeyCode::Digit1,
                            KeyCode::Digit2,
                            KeyCode::Digit3,
                            KeyCode::Digit4,
                            KeyCode::Digit5,
                            KeyCode::Digit6,
                            KeyCode::Digit7,
                            KeyCode::Digit8,
                        ];
                        if let Some(n) = k.parse::<usize>().ok().filter(|n| (1..=8).contains(n)) {
                            self.hold_key(digits[n - 1], down);
                        }
                    }
                    None => {}
                }
            }
        }
    }

    /// What the fingers did since the last frame (MODES.md 5.6), as the mouse and the
    /// keys would have done it: before the look, which turns by a finger's drag.
    pub(super) fn fingers_frame(&mut self, up: bool) {
        if let Some(at) = self.tap_fire
            && at.elapsed().as_secs_f32() > touch::TAP_HOLD_SECS
        {
            self.tap_fire = None;
            self.input.mouse.remove(&MouseButton::Left);
        }
        if !self.fingers.seen {
            return;
        }
        let dpr = self
            .active
            .as_ref()
            .map_or(1.0, |a| a.window.scale_factor() as f32)
            .max(0.5);
        let reach = touch::STICK_DOTS * self.ui_scale();
        self.input.stick = self.fingers.stick(reach);
        for event in self.fingers.drain() {
            match event {
                TouchEvent::Press(Zone::Ui, at) => {
                    let now = Instant::now();
                    let double = self.last_press.is_some_and(|(when, where_)| {
                        now.duration_since(when).as_secs_f32() < DOUBLE_CLICK_SECS
                            && (where_.0 - at.0).abs() < DOUBLE_CLICK_PIXELS * dpr
                            && (where_.1 - at.1).abs() < DOUBLE_CLICK_PIXELS * dpr
                    });
                    self.last_press = (!double).then_some((now, at));
                    self.ui_press(at, double);
                }
                TouchEvent::Move(Zone::Ui, at) => self.cursor = at,
                TouchEvent::Release(Zone::Ui, at) => {
                    self.cursor = at;
                    if self.ui_input.down {
                        self.ui_release();
                    }
                }
                TouchEvent::Press(Zone::Button(b), _) => self.touch_button(b, true),
                TouchEvent::Release(Zone::Button(b), _) => self.touch_button(b, false),
                TouchEvent::Press(..) | TouchEvent::Move(..) | TouchEvent::Release(..) => {}
                TouchEvent::Tap(Zone::World, at) if !up => {
                    self.cursor = at;
                    self.rpg_click();
                }
                TouchEvent::Tap(Zone::Look, _) if !up => {
                    self.input.mouse.insert(MouseButton::Left);
                    self.tap_fire = Some(Instant::now());
                }
                TouchEvent::Tap(..) => {}
                TouchEvent::LongPress(Zone::World, at) if !up => {
                    if self.rpg.hover(self.rpg_under(at)) == crate::rpg::Hover::Attack {
                        self.rpg.ask(crate::rpg::Act::Secondary);
                    }
                }
                TouchEvent::LongPress(..) => {}
                TouchEvent::Drag(dx, dy) => {
                    let gain = touch::LOOK_GAIN / dpr;
                    self.input.mouse_dx += dx * gain;
                    self.input.mouse_dy += dy * gain;
                }
                TouchEvent::Pinch(ratio) => {
                    if self.rpg_mode() && ratio > 0.0 {
                        self.rpg.dist = (self.rpg.dist / ratio)
                            .clamp(crate::rpg::DIST_MIN, crate::rpg::DIST_MAX);
                    }
                }
            }
        }
    }

    /// The cursor the pointer shows (MODES.md 5.5): in the RPG mode with the pointer
    /// free, a crosshair over the target (a click attacks), a hand over another body (a
    /// click targets), the arrow elsewhere; the arrow everywhere else.
    pub(super) fn show_cursor(&mut self) {
        let want = if self.rpg_mode() && !self.grabbed && !self.screen_up() {
            match self.rpg.hover(self.rpg_under(self.cursor)) {
                crate::rpg::Hover::Attack => CursorIcon::Crosshair,
                crate::rpg::Hover::Body => CursorIcon::Pointer,
                crate::rpg::Hover::Ground => CursorIcon::Default,
            }
        } else {
            CursorIcon::Default
        };
        if want != self.cursor_icon {
            self.cursor_icon = want;
            if let Some(a) = self.active.as_ref() {
                a.window.set_cursor(want);
            }
        }
    }

    /// Tab in the RPG mode (MODES.md 5.2): the nearest enemy in sight not yet cycled.
    pub(super) fn rpg_cycle(&mut self) {
        let Some(c) = self.online.as_ref().and_then(|o| o.client.as_ref()) else {
            return;
        };
        let eye = c.mover.eye();
        let me = c.mover.mv.origin;
        let candidates: Vec<(u32, f32)> = self
            .rpg_bodies()
            .iter()
            .filter(|b| b.enemy)
            .filter(|b| gm_core::sim::sees(&self.bsp, eye, b.capsule().center()))
            .map(|b| (b.id, (b.origin - me).length()))
            .collect();
        self.rpg.cycle(&candidates);
    }

    /// The scope of the firearm in hand (MODES.md 3.2): its zoom, 0 without one.
    pub(super) fn gun_scope(&self) -> u8 {
        let Some(c) = self.online.as_ref().and_then(|o| o.client.as_ref()) else {
            return 0;
        };
        if self.input.held > 1 {
            return 0;
        }
        c.mover
            .in_hand(&c.sheet.kit)
            .and_then(|i| c.sheet.kit.abilities[i as usize].firearm.as_ref())
            .map_or(0, |f| f.scope)
    }

    /// An event of the window's (CLIENT.md 3): the keys, the mouse, the fingers, the focus.
    pub(super) fn window_event(&mut self, event_loop: &ActiveEventLoop, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(_) => self.configure_surface(),
            WindowEvent::Focused(false) => {
                self.input.keys.clear();
                self.input.mouse.clear();
                self.fingers.clear();
                self.set_grab(false);
            }
            WindowEvent::ModifiersChanged(held) => {
                let held = held.state();
                self.shift = held.shift_key();
                self.command = (held.control_key() || held.super_key()) && !held.alt_key();
            }
            WindowEvent::Focused(true) => {
                if cfg!(not(target_arch = "wasm32")) && self.wants_pointer() {
                    self.set_grab(true);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as f32, position.y as f32);
            }
            WindowEvent::Touch(t) => {
                let at = (t.location.x as f32, t.location.y as f32);
                let phase = match t.phase {
                    TouchPhase::Started => touch::Phase::Started,
                    TouchPhase::Moved => touch::Phase::Moved,
                    TouchPhase::Ended => touch::Phase::Ended,
                    TouchPhase::Cancelled => touch::Phase::Cancelled,
                };
                let zone = self.touch_zone(at);
                let slop = touch::SLOP_DOTS * self.ui_scale();
                self.fingers
                    .touch(t.id, phase, at, Instant::now(), slop, |_| zone);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let turn = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                if self.screen_up() {
                    self.ui_input.wheel += turn;
                } else if self.rpg_mode() {
                    // The orbit camera's distance (MODES.md 5.5).
                    self.rpg.dist = (self.rpg.dist - turn * 20.0)
                        .clamp(crate::rpg::DIST_MIN, crate::rpg::DIST_MAX);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => match state {
                ElementState::Pressed => {
                    if self.screen_up() {
                        // The pointer is the screen's (CLIENT.md 3): a click is a widget's.
                        if button == MouseButton::Left {
                            let now = Instant::now();
                            let double = self.last_press.is_some_and(|(at, where_)| {
                                now.duration_since(at).as_secs_f32() < DOUBLE_CLICK_SECS
                                    && (where_.0 - self.cursor.0).abs() < DOUBLE_CLICK_PIXELS
                                    && (where_.1 - self.cursor.1).abs() < DOUBLE_CLICK_PIXELS
                            });
                            self.last_press = (!double).then_some((now, self.cursor));
                            self.ui_press(self.cursor, double);
                        }
                    } else if self.rpg_mode() {
                        // The RPG mode (MODES.md 5.5): the left button picks a body or a
                        // place on the ground, the right one held turns the camera.
                        match button {
                            MouseButton::Left => self.rpg_click(),
                            MouseButton::Right => {
                                self.rpg_right = Some((
                                    Instant::now(),
                                    self.cursor,
                                    self.sim.yaw,
                                    self.sim.pitch,
                                ));
                                self.input.mouse.insert(button);
                                self.set_grab(true);
                            }
                            _ => {}
                        }
                    } else if !self.grabbed && self.opts.bench_frames.is_none() {
                        self.set_grab(true);
                    } else {
                        self.press_in_game(button);
                    }
                }
                ElementState::Released => {
                    self.input.mouse.remove(&button);
                    if button == MouseButton::Left && self.ui_input.down {
                        self.ui_release();
                    }
                    if button == MouseButton::Right && self.rpg_mode() && self.grabbed {
                        self.set_grab(false);
                        self.rpg_right_release();
                    }
                }
            },
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                let code = match event.physical_key {
                    PhysicalKey::Code(code) => Some(code),
                    PhysicalKey::Unidentified(_) => None,
                };
                // The keys the screens act on (CLIENT.md 6).
                let key = match code {
                    Some(KeyCode::Enter | KeyCode::NumpadEnter) => Some(Key::Enter),
                    Some(KeyCode::Escape) => Some(Key::Escape),
                    Some(KeyCode::Tab) if self.shift => Some(Key::BackTab),
                    Some(KeyCode::Tab) => Some(Key::Tab),
                    Some(KeyCode::Backspace) => Some(Key::Backspace),
                    Some(KeyCode::Delete) => Some(Key::Delete),
                    Some(KeyCode::ArrowLeft) => Some(Key::Left),
                    Some(KeyCode::ArrowRight) => Some(Key::Right),
                    Some(KeyCode::ArrowUp) => Some(Key::Up),
                    Some(KeyCode::ArrowDown) => Some(Key::Down),
                    Some(KeyCode::Home) => Some(Key::Home),
                    Some(KeyCode::End) => Some(Key::End),
                    Some(KeyCode::PageUp) => Some(Key::PageUp),
                    Some(KeyCode::PageDown) => Some(Key::PageDown),
                    _ => None,
                };
                if self.capturing() {
                    // The toolkit has the keyboard: the game gets nothing.
                    if !pressed {
                        return;
                    }
                    // Ctrl+V and Shift+Insert paste (the key that types `v`, whatever
                    // the layout; the V key where the layout has no such letter).
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        let v = match event.logical_key.as_ref() {
                            winit::keyboard::Key::Character(c) if c.is_ascii() => {
                                c.eq_ignore_ascii_case("v")
                            }
                            _ => code == Some(KeyCode::KeyV),
                        };
                        if (self.command && v) || (self.shift && code == Some(KeyCode::Insert)) {
                            self.paste();
                            return;
                        }
                    }
                    match (key, &event.text) {
                        // A key held down does not press Enter or Escape again: the
                        // line it opened would close, the login it sent be sent again.
                        (Some(Key::Enter | Key::Escape), _) if event.repeat => {}
                        (Some(key), _) => self.ui_key(key),
                        // With Ctrl held a key is a shortcut, not a letter (and a key
                        // that has no place on this keyboard's map still has its text:
                        // a program that types for a person sends such).
                        (None, Some(text)) if !self.command => self.ui_text(text),
                        _ => {}
                    }
                    return;
                }
                let Some(code) = code else { return };
                match event.state {
                    ElementState::Pressed => {
                        if self.input.keys.insert(code) {
                            self.input.just_pressed.insert(code);
                        }
                        match (code, key) {
                            // Enter opens the chat line, Escape the menu.
                            (_, Some(key @ (Key::Enter | Key::Escape))) if !event.repeat => {
                                self.ui_key(key)
                            }
                            _ => {}
                        }
                        match code {
                            // The inventory, and the stall the body stands at.
                            KeyCode::KeyI if !event.repeat => self.ui_key(Key::Inventory),
                            KeyCode::KeyO if !event.repeat => self.ui_key(Key::Equipment),
                            KeyCode::KeyE if !event.repeat => self.ui_key(Key::Use),
                            KeyCode::KeyP if !event.repeat => self.ui_key(Key::People),
                            KeyCode::KeyG if !event.repeat => self.ui_key(Key::Gm),
                            KeyCode::KeyK if !event.repeat => self.ui_key(Key::Character),
                            KeyCode::Tab if !event.repeat => self.ui_key(Key::Tab),
                            // Outside a zone only (the fitting room, a replay): in one
                            // the character's mode is the camera (MODES.md 2).
                            KeyCode::KeyV if !event.repeat && self.online.is_none() => {
                                self.viewport = match self.viewport {
                                    Viewport::First => Viewport::Third,
                                    Viewport::Third => Viewport::First,
                                };
                                log::info!("viewport: {:?}", self.viewport);
                            }
                            _ => {}
                        }
                    }
                    ElementState::Released => {
                        self.input.keys.remove(&code);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                self.show_cursor();
                self.frame(event_loop);
            }
            _ => {}
        }
    }

    /// The mouse moved while the pointer is held: the look turns.
    pub(super) fn device_event(&mut self, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event
            && self.grabbed
        {
            self.input.mouse_dx += delta.0 as f32;
            self.input.mouse_dy += delta.1 as f32;
        }
    }
}
