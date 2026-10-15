use super::*;

impl App {
    /// A screen has the pointer and the keys: one before the game, or the game menu
    /// (CLIENT.md 6).
    pub(super) fn screen_up(&self) -> bool {
        self.front_up
            || self.menu.is_some()
            || self.bag.is_some()
            || self.people.is_some()
            || self.gm_page.is_some()
            || self.character_page.is_some()
            || self.title.is_some()
    }

    /// The hub and the session and character it is asked about, while one is played.
    pub(super) fn owner(
        &self,
    ) -> Option<(
        &Hub,
        gm_hub_proto::protocol::SessionId,
        gm_hub_proto::protocol::CharacterId,
    )> {
        let playing = self.online.as_ref().is_some_and(|o| o.client.is_some());
        match (&self.hub, &self.account, self.character) {
            (Some(hub), Some(account), Some(character)) if playing => {
                Some((hub, account.session, character))
            }
            _ => None,
        }
    }

    /// The stall the body stands at (ITEMS.md 5): the nearest one in reach, by the rule
    /// the zone decides a purchase with.
    pub(super) fn stall_in_reach(online: Option<&Online>) -> Option<&StallEntry> {
        let o = online?;
        let c = o.client.as_ref()?;
        let feet = c.mover.mv.origin + Vec3::Z * Hull::Player.mins().z;
        let far = |s: &StallEntry| (Vec3::from(s.pos) - feet).truncate().length_squared();
        o.stalls
            .iter()
            .filter(|s| gm_net::control::stall_in_reach(s.pos, feet.into()))
            .min_by(|a, b| far(a).total_cmp(&far(b)))
    }

    /// `I`: the inventory of the character being played; with it up already, the bag's
    /// grid shown or hidden beside the equipment (EQUIPMENT.md 4).
    pub(super) fn open_inventory(&mut self) {
        if let Some(bag) = &mut self.bag
            && bag.page == crate::bag::Page::Inventory
        {
            if !bag.toggle_grid() {
                self.bag = None;
            }
            return;
        }
        match self.owner() {
            Some((hub, session, character)) => {
                self.bag = Some(Bag::inventory(hub, session, character, Instant::now()));
                self.menu = None;
                self.release_keys();
            }
            None => self.note("there is no inventory without a hub"),
        }
    }

    /// `O`: the equipment of the character being played (EQUIPMENT.md 4): what the body
    /// wears, with the places at its sides; with the inventory up already, shown or
    /// hidden beside the bag.
    pub(super) fn open_equipment(&mut self) {
        if let Some(bag) = &mut self.bag
            && bag.page == crate::bag::Page::Inventory
        {
            if !bag.toggle_equip() {
                self.bag = None;
            }
            return;
        }
        match self.owner() {
            Some((hub, session, character)) => {
                self.bag = Some(Bag::equipment(hub, session, character, Instant::now()));
                self.menu = None;
                self.release_keys();
            }
            None => self.note("there is no equipment without a hub"),
        }
    }

    /// `P`: the people here and of the party. (Without a hub there are no parties, and
    /// the zone says so to whoever asks.)
    pub(super) fn open_people(&mut self) {
        if !self.online.as_ref().is_some_and(|o| o.client.is_some()) {
            return;
        }
        // (Without a hub the page still says who is here; it asks the hub nothing.)
        let nobody = (gm_hub_proto::protocol::SessionId([0; 16]), 0);
        let (session, character) = self.owner().map_or(nobody, |(_, s, c)| (s, c));
        self.people = Some(People::here(session, character, Instant::now()));
        self.menu = None;
        self.bag = None;
        self.release_keys();
    }

    /// `K`: the character's page (MATRIX.md 9.1): the thirty points and the kit, worn at
    /// the trainer.
    pub(super) fn open_character(&mut self) {
        let Some(o) = &self.online else { return };
        if o.client.is_none() {
            return;
        }
        self.character_page = Some(CharacterPage::default());
        self.menu = None;
        self.bag = None;
        self.people = None;
        self.gm_page = None;
        self.release_keys();
    }

    /// What the character's page asked for: the zone is asked, and answers
    /// (`RespecResult`, shown as the page's note).
    pub(super) fn character_act(&mut self, action: CharacterAction) {
        match action {
            CharacterAction::None => {}
            CharacterAction::Close => self.character_page = None,
            CharacterAction::Wear(build) => {
                if let Some(o) = &mut self.online {
                    o.respec_note = "asked".into();
                    o.net
                        .send_control(FromClient::Respec(BuildChoice::Custom(build)));
                }
            }
        }
    }

    /// Standing by the trainer (TRAINER_REACH of a body nothing hurts), where a build is
    /// worn in the world; in a team zone (the arena) anywhere, at the next respawn.
    pub(super) fn at_trainer(&self) -> bool {
        let Some(o) = &self.online else { return false };
        let (Some(c), Some(pack)) = (&o.client, &o.pack) else {
            return false;
        };
        if !o.has_trainer() {
            return true;
        }
        let npc = |id: &u32| {
            matches!(o.kinds.get(id), Some(BodyKind::Creature { def })
                if pack.creatures.get(*def as usize).is_some_and(|d| d.npc))
        };
        let me = c.mover.mv.origin;
        c.others_at(c.render_tick(0.0))
            .iter()
            .any(|e| e.alive() && npc(&e.id) && (e.pos - me).length() <= TRAINER_REACH)
    }

    /// `G`: the game master's page (GM.md 4), for a character the zone granted it to.
    pub(super) fn open_gm(&mut self) {
        let Some(o) = &self.online else { return };
        if o.client.is_none() {
            return;
        }
        if !o.gm {
            self.note("the zone did not make this character a game master");
            return;
        }
        self.gm_page = Some(GmPage::default());
        self.menu = None;
        self.bag = None;
        self.people = None;
        self.character_page = None;
        self.release_keys();
    }

    /// What the game master's page asked for: the zone is asked, and answers.
    pub(super) fn gm_act(&mut self, action: GmAction) {
        match action {
            GmAction::None => {}
            GmAction::Close => self.gm_page = None,
            GmAction::Send(op) => {
                if let Some(o) = &mut self.online {
                    o.gm_note = "asked".into();
                    o.net.send_control(FromClient::Gm(op));
                }
            }
        }
    }

    /// What the page of people asked for.
    pub(super) fn people_act(&mut self, action: PeopleAction) {
        match action {
            PeopleAction::None => {}
            PeopleAction::Close => self.people = None,
            PeopleAction::Zone(say) => {
                if let Some(o) = &mut self.online {
                    o.net.send_control(say);
                }
            }
            PeopleAction::Whisper(name) => {
                self.people = None;
                self.chat.open_with(format!("/w {name} "));
            }
        }
    }

    /// `E`: what the stall the body stands at has for sale.
    pub(super) fn open_stall(&mut self) {
        let near = Self::stall_in_reach(self.online.as_ref());
        let Some((id, owner)) = near.map(|s| (s.id, s.owner.clone())) else {
            return;
        };
        match self.owner() {
            Some((hub, session, character)) => {
                let now = Instant::now();
                self.bag = Some(Bag::stall(hub, session, character, id, &owner, now));
                self.menu = None;
                self.release_keys();
            }
            None => self.note("there is nothing to look at without a hub"),
        }
    }

    /// A word in the window's title and the log, as the stall keys say theirs.
    pub(super) fn note(&mut self, text: &str) {
        log::info!("{text}");
        if let Some(o) = &mut self.online {
            o.respec_note = text.to_string();
        }
    }

    /// The keys are the toolkit's: a screen is up, or the chat line is open.
    pub(super) fn capturing(&self) -> bool {
        self.screen_up() || self.chat.open
    }

    /// Whether the game would hold the pointer for mouse look if it had the window.
    /// The character plays the RPG mode (MODES.md 5): the pointer is free, the camera
    /// turns while the secondary button is held.
    pub(super) fn rpg_mode(&self) -> bool {
        self.online
            .as_ref()
            .and_then(|o| o.client.as_ref())
            .is_some_and(|c| c.sheet.kit.mode == gm_core::vocab::Mode::Rpg)
    }

    pub(super) fn wants_pointer(&self) -> bool {
        !self.rpg_mode()
            && !self.screen_up()
            && self.opts.bench_frames.is_none()
            && self.opts.script.is_none()
            && self.opts.replay.is_none()
    }

    /// A person is at the client: when a zone's connection ends, the characters are shown
    /// again. A client on autopilot or joined directly ends with its connection.
    pub(super) fn returns_to_screens(&self) -> bool {
        self.front.as_ref().is_some_and(|f| !f.on_autopilot())
    }

    /// A key the screens act on was pressed, by a person or by a script (CLIENT.md 6).
    pub(super) fn ui_key(&mut self, key: Key) {
        if self.capturing() {
            self.ui_input.keys.push(key);
            return;
        }
        // In the game: Enter opens the chat line, Escape the menu (a replay has the menu too:
        // it is where Quit is).
        match key {
            Key::Enter if self.online.as_ref().is_some_and(|o| o.client.is_some()) => {
                self.chat.open = true;
                self.release_keys();
            }
            // In the RPG mode a target is let go first (MODES.md 5.2).
            Key::Escape if self.rpg_mode() && self.rpg.target.is_some() => {
                self.rpg.clear_target();
            }
            Key::Escape if self.opts.bench_frames.is_none() => {
                self.menu = Some(GameMenu::default());
                self.release_keys();
            }
            Key::Tab if self.rpg_mode() => self.rpg_cycle(),
            Key::Inventory => self.open_inventory(),
            Key::Equipment => self.open_equipment(),
            Key::People => self.open_people(),
            Key::Use => self.open_stall(),
            Key::Gm => self.open_gm(),
            Key::Character => self.open_character(),
            // A script's `F` (CLIENT.md 9): the first item cell, the kit's, as a person's
            // key is (MODES.md 11.3); a frame's press, let go the same frame.
            Key::Kit if self.online.is_some() => {
                self.input.just_pressed.insert(KeyCode::KeyF);
            }
            _ => {}
        }
    }

    /// Characters were typed.
    pub(super) fn ui_text(&mut self, text: &str) {
        if self.capturing() {
            self.ui_input
                .text
                .extend(text.chars().filter(|c| !c.is_control()));
        }
    }

    /// The left button went down at `at` while a screen is up.
    pub(super) fn ui_press(&mut self, at: (f32, f32), double: bool) {
        self.cursor = at;
        self.ui_input.pressed = true;
        self.ui_input.down = true;
        self.ui_input.double = double;
    }

    pub(super) fn ui_release(&mut self) {
        self.ui_input.released = true;
        self.ui_input.down = false;
    }

    /// What a person changed is written down; a run that changed nothing leaves the
    /// file alone (CLIENT.md 8).
    pub(super) fn keep_settings(&mut self) {
        if self.settings == self.settings_kept {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = &self.settings_path {
            self.settings.save(path);
        }
        #[cfg(target_arch = "wasm32")]
        self.settings.save();
        self.settings_kept = self.settings.clone();
    }

    /// What the settings say, done: called when one changes and when the window is ready.
    pub(super) fn apply_settings(&mut self) {
        self.sound
            .set_volume(self.settings.volume, self.settings.mute);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(a) = &self.active {
            let wanted = self
                .settings
                .fullscreen
                .then_some(winit::window::Fullscreen::Borderless(None));
            if a.window.fullscreen().is_some() != wanted.is_some() {
                a.window.set_fullscreen(wanted);
            }
        }
    }

    /// Ctrl+V: what the clipboard holds goes where typing would (CLIENT.md 3). In a browser
    /// the clipboard is the page's, and the page's own form is where a password goes.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn paste(&mut self) {
        /// More than any field takes.
        const MOST: usize = 1024;
        let (tx, rx) = std::sync::mpsc::channel();
        let read = std::thread::Builder::new()
            .name("gm-paste".into())
            .spawn(
                move || match arboard::Clipboard::new().and_then(|mut c| c.get_text()) {
                    Ok(text) => {
                        let _ = tx.send(text.chars().take(MOST).collect());
                    }
                    Err(e) => log::debug!("nothing to paste: {e}"),
                },
            );
        if read.is_ok() {
            self.pasting = Some(rx);
        }
    }

    /// What a paste brought, typed where the keyboard is now.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn pasted(&mut self) {
        let Some(rx) = &self.pasting else { return };
        match rx.try_recv() {
            Ok(text) => {
                self.pasting = None;
                self.ui_text(&text);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.pasting = None,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
    }

    /// What the screens, the menu and the chat line asked for in this frame.
    /// What the inventory or the stall asked for: buying and wearing are said to the
    /// zone (ITEMS.md 5).
    pub(super) fn bag_act(&mut self, action: BagAction) {
        let say = match action {
            BagAction::None => return,
            BagAction::Close => {
                self.bag = None;
                return;
            }
            BagAction::Buy {
                stall,
                listing,
                price,
            } => FromClient::StallBuy {
                stall,
                listing,
                price,
            },
            BagAction::Wear { item } => FromClient::Wear { item },
            BagAction::WearIn { item, place } => FromClient::WearIn { item, place },
            BagAction::TakeOff { item } => FromClient::TakeOff { item },
        };
        if let Some(o) = &mut self.online {
            o.net.send_control(say);
        }
    }

    pub(super) fn act(
        &mut self,
        front: Action,
        menu: MenuAction,
        said: Option<Said>,
        event_loop: &ActiveEventLoop,
    ) {
        if let (Some(said), Some(o)) = (said, &mut self.online) {
            // A line to the zone, to the party or to one character; and the two things
            // the chat line can ask of the party (PARTY.md 5).
            o.net.send_control(match said {
                Said::Say(text) => FromClient::Chat(text),
                Said::Party(text) => FromClient::PartySay(text),
                Said::Whisper { to, text } => FromClient::Whisper { to, text },
                Said::Invite(name) => {
                    o.social.asked = Some(Instant::now());
                    FromClient::PartyInvite { name }
                }
                Said::Leave => {
                    o.social.asked = Some(Instant::now());
                    FromClient::PartyLeave
                }
            });
        }
        match menu {
            MenuAction::None => {}
            MenuAction::Resume => self.menu = None,
            MenuAction::Inventory => self.open_inventory(),
            MenuAction::Equipment => self.open_equipment(),
            MenuAction::People => self.open_people(),
            MenuAction::Gm => self.open_gm(),
            MenuAction::Character => self.open_character(),
            MenuAction::Travel(zone) => {
                if let Some(o) = &mut self.online {
                    o.net.send_control(FromClient::Travel(zone.clone()));
                    o.say(format!("travel to {zone} requested"), hud::DIM);
                }
                self.menu = None;
            }
            MenuAction::Leave => self.leave_zone(""),
            MenuAction::Quit => event_loop.exit(),
            MenuAction::Changed => self.apply_settings(),
            // The tap that pressed the button is a gesture the browser still honours.
            MenuAction::PageFullscreen => {
                #[cfg(target_arch = "wasm32")]
                crate::web::tell_page("fullscreen", "toggle");
                self.menu = None;
            }
        }
        match front {
            Action::None => {}
            Action::Session(account) => {
                if let Some(hub) = &self.hub {
                    hub.set_session(account.as_ref().map(|a| a.session));
                }
                if let Some(a) = &account
                    && self.returns_to_screens()
                {
                    self.settings.email = a.email.clone();
                }
                self.account = account;
            }
            Action::Enter { ticket, name } => self.enter_zone(*ticket, name, event_loop),
            Action::CancelEntering => self.hang_up(),
            Action::Quit => event_loop.exit(),
            Action::Failed(why) => self.fail(event_loop, &why),
        }
    }

    /// What only a page has (CLIENT.md 4.1, WEB.md 3.4): its own form is the login screen,
    /// and the browser gives and takes the pointer as it sees fit.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn page_frame(&mut self) {
        use crate::front::Screen;
        if let Some(front) = &mut self.front {
            let at_login = self.front_up && front.screen == Screen::Login && !front.on_autopilot();
            // The form is shown while the login screen is up, with the last refusal.
            let tell = match (at_login, front.busy()) {
                (true, false) => format!("login {}", front.notice()),
                (true, true) => "login-wait ".to_string(),
                (false, _) => "screen ".to_string(),
            };
            if tell != self.told {
                if let Some((kind, text)) = tell.split_once(' ') {
                    crate::web::tell_page(kind, text);
                }
                self.told = tell;
            }
            if at_login
                && !front.busy()
                && let Some((email, password, register)) = crate::web::take_login()
            {
                front.page_login(email, password, register);
            }
        }
        self.page_keyboard();
        // The pointer: Escape gives it back to the browser and never reaches the page, so
        // losing it is what Escape is here: the menu opens, or the chat line is dropped.
        let locked = crate::web::pointer_locked();
        if self.grabbed && !locked {
            let waited = self
                .grab_asked
                .is_none_or(|at| at.elapsed().as_secs_f32() > 0.5);
            if self.was_locked {
                self.grabbed = false;
                if self.chat.open {
                    self.chat.drop_line();
                } else if !self.screen_up() {
                    self.ui_key(Key::Escape);
                }
            } else if waited {
                // It was asked for and never given: the next click asks again.
                self.grabbed = false;
            }
        }
        // Given later than it was waited for: it is the game's all the same.
        if locked && !self.grabbed && self.wants_pointer() {
            self.grabbed = true;
        }
        self.was_locked = locked;
    }

    /// The phone's keyboard is the browser's (WEB.md 3.6): the page is told which text
    /// field of the canvas has the keys and what it holds, and where the fields are, so
    /// that a finger on one brings the keyboard up; what the keyboard typed comes back
    /// through the page as the keys and text a keyboard of the window's would have sent.
    #[cfg(target_arch = "wasm32")]
    pub(super) fn page_keyboard(&mut self) {
        for typed in crate::web::take_typed() {
            match typed.strip_prefix('k') {
                Some(name) => {
                    if let Some(key) = Key::parse(name) {
                        self.ui_key(key);
                    }
                }
                None => {
                    if let Some(text) = typed.strip_prefix('t') {
                        self.ui_text(text);
                    }
                }
            }
        }
        let field = if self.capturing() {
            self.ui.typing_in().map(|label| {
                let head = format!("{label}: ");
                self.ui
                    .seen
                    .iter()
                    .filter(|s| s.kind == ui::SeenKind::Field)
                    .find_map(|s| s.text.strip_prefix(&head))
                    .unwrap_or("")
                    .to_string()
            })
        } else {
            None
        };
        if field != self.told_field {
            match &field {
                Some(value) => crate::web::tell_page("field", value),
                None => crate::web::tell_page("no-field", ""),
            }
            self.told_field = field;
        }
        // The fields' places, for a finger (a mouse needs no keyboard brought up).
        let mut rects = Vec::new();
        if self.fingers.seen && self.capturing() {
            let dpr = self
                .active
                .as_ref()
                .map_or(1.0, |a| a.window.scale_factor() as f32)
                .max(0.5);
            for s in self
                .ui
                .seen
                .iter()
                .filter(|s| s.kind == ui::SeenKind::Field)
            {
                rects.extend([
                    s.rect.x / dpr,
                    s.rect.y / dpr,
                    s.rect.w / dpr,
                    s.rect.h / dpr,
                ]);
            }
        }
        if rects != self.told_fields {
            crate::web::tell_fields(&rects);
            self.told_fields = rects;
        }
    }

    /// What the script does this frame goes in where a person's events would (CLIENT.md 9).
    /// `false`: the script failed and the program is ending.
    pub(super) fn run_script(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(script) = &mut self.ui_script else {
            return true;
        };
        if !std::mem::take(&mut self.ui_drawn) {
            return true;
        }
        let events = match script.step(&self.ui, Instant::now()) {
            Ok(events) => events,
            Err(e) => {
                self.fail(event_loop, &e);
                return false;
            }
        };
        for event in events {
            match event {
                crate::script::Event::Press { at, double } => self.ui_press(at, double),
                crate::script::Event::Release => self.ui_release(),
                crate::script::Event::Move(at) => self.cursor = at,
                crate::script::Event::Text(text) => self.ui_text(&text),
                crate::script::Event::Key(key) => self.ui_key(key),
                crate::script::Event::Say(text) => {
                    #[cfg(not(target_arch = "wasm32"))]
                    println!("ui-script: {text}");
                    #[cfg(target_arch = "wasm32")]
                    crate::web::tell_page("say", &text);
                }
                crate::script::Event::Quit => {
                    log::info!("ui-script: ok");
                    #[cfg(not(target_arch = "wasm32"))]
                    println!("ui-script: ok");
                    #[cfg(target_arch = "wasm32")]
                    {
                        if let Some(o) = &mut self.online {
                            o.net.close();
                        }
                        if let Some(hub) = &self.hub {
                            hub.logout();
                        }
                        crate::web::tell_page("done", "ui-script: ok");
                    }
                    event_loop.exit();
                }
            }
        }
        true
    }

    /// The scale the HUD and the screens are drawn at this frame (CLIENT.md 3).
    pub(super) fn ui_scale(&self) -> f32 {
        let size = self.active.as_ref().map_or((1280.0, 720.0), |a| {
            (a.config.width as f32, a.config.height as f32)
        });
        ui::scale_for(size, PANEL_UNITS, self.ui_scale_choice())
    }

    /// The scale asked for: the setting; by the window when it is 0, except on a touch
    /// screen, whose pixels are small under a finger: there by the device's pixel ratio
    /// (CLIENT.md 3, MODES.md 5.6), still never larger than the panels can fit.
    pub(super) fn ui_scale_choice(&self) -> u8 {
        if self.settings.ui_scale != 0 || !self.fingers.seen {
            return self.settings.ui_scale;
        }
        let dpr = self
            .active
            .as_ref()
            .map_or(1.0, |a| a.window.scale_factor() as f32);
        ui::touch_scale(dpr)
    }
}
