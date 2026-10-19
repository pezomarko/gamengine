use super::*;

impl App {
    /// A screen has the pointer and the keys: one before the game, or the game menu
    /// (CLIENT.md 6).
    pub(super) fn screen_up(&self) -> bool {
        self.screens.front_up
            || self.screens.menu.is_some()
            || self.screens.bag.is_some()
            || self.screens.smithy.is_some()
            || self.screens.forge.is_some()
            || self.screens.people.is_some()
            || self.screens.gm_page.is_some()
            || self.screens.character_page.is_some()
            || self.screens.title.is_some()
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
        match (
            &self.session.hub,
            &self.session.account,
            self.session.character,
        ) {
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

    /// Somebody of `role` nothing hurts (the smith, the trainer: ITEMS.md 6.6) stands within
    /// `reach` of the own body, alive: the rule the zone decides with.
    pub(super) fn npc_in_reach(online: Option<&Online>, role: &str, reach: f32) -> bool {
        let Some(o) = online else { return false };
        let (Some(c), Some(pack)) = (&o.client, &o.pack) else {
            return false;
        };
        let of_role = |id: &u32| {
            matches!(o.kinds.get(id), Some(BodyKind::Creature { def })
                if pack.creatures.get(*def as usize).is_some_and(|d| d.npc && d.role == role))
        };
        let me = c.mover.mv.origin;
        c.others_at(c.render_tick(0.0))
            .iter()
            .any(|e| e.alive() && of_role(&e.id) && (e.pos - me).length() <= reach)
    }

    /// A forge of the map (ITEMS.md 6.6) is in the own body's reach, by the stall's rule.
    pub(super) fn forge_in_reach(&self) -> bool {
        let Some(c) = self.online.as_ref().and_then(|o| o.client.as_ref()) else {
            return false;
        };
        let feet = c.mover.mv.origin + Vec3::Z * Hull::Player.mins().z;
        self.forges
            .iter()
            .any(|f| gm_net::control::stall_in_reach(f.origin.into(), feet.into()))
    }

    /// What `E` would open where the body stands, and the corner's words for it: the
    /// forge first, then the smith, then a stall (ITEMS.md 6.6).
    pub(super) fn use_hint(&self) -> Option<String> {
        self.session.hub.as_ref()?;
        if self.forge_in_reach() {
            return Some("Forge  E craft".to_string());
        }
        if Self::npc_in_reach(self.online.as_ref(), "smith", TRAINER_REACH) {
            return Some("Blacksmith  E talk".to_string());
        }
        Self::stall_in_reach(self.online.as_ref()).map(|s| format!("{}'s stall  E look", s.owner))
    }

    /// `E`: the forge, the smith or the stall the body stands at.
    pub(super) fn open_use(&mut self) {
        if self.forge_in_reach() {
            self.open_forge();
        } else if Self::npc_in_reach(self.online.as_ref(), "smith", TRAINER_REACH) {
            self.open_smithy();
        } else {
            self.open_stall();
        }
    }

    /// The smith's wares (ITEMS.md 6.6).
    pub(super) fn open_smithy(&mut self) {
        match self.owner() {
            Some((hub, session, character)) => {
                self.screens.smithy = Some(Smithy::open(hub, session, character, Instant::now()));
                self.screens.menu = None;
                self.release_keys();
            }
            None => self.note("there is nothing to buy without a hub"),
        }
    }

    /// The forge (ITEMS.md 6.6).
    pub(super) fn open_forge(&mut self) {
        match self.owner() {
            Some((hub, session, character)) => {
                self.screens.forge = Some(Forge::open(hub, session, character, Instant::now()));
                self.screens.menu = None;
                self.release_keys();
            }
            None => self.note("there is nothing to forge without a hub"),
        }
    }

    /// What the smith's shop asked for: a buy is said to the zone (ITEMS.md 6.6).
    pub(super) fn smithy_act(&mut self, action: SmithyAction) {
        match action {
            SmithyAction::None => {}
            SmithyAction::Close => self.screens.smithy = None,
            SmithyAction::Buy { material, price } => {
                if let Some(o) = &mut self.online {
                    o.net.send_control(FromClient::SmithBuy { material, price });
                }
            }
        }
    }

    /// What the forge asked for: a craft is said to the zone (ITEMS.md 6.6).
    pub(super) fn forge_act(&mut self, action: ForgeAction) {
        match action {
            ForgeAction::None => {}
            ForgeAction::Close => self.screens.forge = None,
            ForgeAction::Forge {
                template,
                components,
                fee,
            } => {
                if let Some(o) = &mut self.online {
                    o.net.send_control(FromClient::Forge {
                        template,
                        components,
                        fee,
                    });
                }
            }
        }
    }

    /// `I`: the inventory of the character being played; with it up already, the bag's
    /// grid shown or hidden beside the equipment (EQUIPMENT.md 4).
    pub(super) fn open_inventory(&mut self) {
        if let Some(bag) = &mut self.screens.bag
            && bag.page == crate::bag::Page::Inventory
        {
            if !bag.toggle_grid() {
                self.screens.bag = None;
            }
            return;
        }
        match self.owner() {
            Some((hub, session, character)) => {
                self.screens.bag = Some(Bag::inventory(hub, session, character, Instant::now()));
                self.screens.menu = None;
                self.release_keys();
            }
            None => self.note("there is no inventory without a hub"),
        }
    }

    /// `O`: the equipment of the character being played (EQUIPMENT.md 4): what the body
    /// wears, with the places at its sides; with the inventory up already, shown or
    /// hidden beside the bag.
    pub(super) fn open_equipment(&mut self) {
        if let Some(bag) = &mut self.screens.bag
            && bag.page == crate::bag::Page::Inventory
        {
            if !bag.toggle_equip() {
                self.screens.bag = None;
            }
            return;
        }
        match self.owner() {
            Some((hub, session, character)) => {
                self.screens.bag = Some(Bag::equipment(hub, session, character, Instant::now()));
                self.screens.menu = None;
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
        self.screens.people = Some(People::here(session, character, Instant::now()));
        self.screens.menu = None;
        self.screens.bag = None;
        self.release_keys();
    }

    /// `K`: the character's page (MATRIX.md 9.1): the thirty points and the kit, worn at
    /// the trainer.
    pub(super) fn open_character(&mut self) {
        let Some(o) = &self.online else { return };
        if o.client.is_none() {
            return;
        }
        self.screens.character_page = Some(CharacterPage::default());
        self.screens.menu = None;
        self.screens.bag = None;
        self.screens.people = None;
        self.screens.gm_page = None;
        self.release_keys();
    }

    /// What the character's page asked for: the zone is asked, and answers
    /// (`RespecResult`, shown as the page's note).
    pub(super) fn character_act(&mut self, action: CharacterAction) {
        match action {
            CharacterAction::None => {}
            CharacterAction::Close => self.screens.character_page = None,
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
        let _ = (c, pack);
        Self::npc_in_reach(self.online.as_ref(), "trainer", TRAINER_REACH)
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
        self.screens.gm_page = Some(GmPage::default());
        self.screens.menu = None;
        self.screens.bag = None;
        self.screens.people = None;
        self.screens.character_page = None;
        self.release_keys();
    }

    /// What the game master's page asked for: the zone is asked, and answers.
    pub(super) fn gm_act(&mut self, action: GmAction) {
        match action {
            GmAction::None => {}
            GmAction::Close => self.screens.gm_page = None,
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
            PeopleAction::Close => self.screens.people = None,
            PeopleAction::Zone(say) => {
                if let Some(o) = &mut self.online {
                    o.net.send_control(say);
                }
            }
            PeopleAction::Whisper(name) => {
                self.screens.people = None;
                self.screens.chat.open_with(format!("/w {name} "));
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
                self.screens.bag = Some(Bag::stall(hub, session, character, id, &owner, now));
                self.screens.menu = None;
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
        self.screen_up() || self.screens.chat.open
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
        self.screens
            .front
            .as_ref()
            .is_some_and(|f| !f.on_autopilot())
    }

    /// A key the screens act on was pressed, by a person or by a script (CLIENT.md 6).
    pub(super) fn ui_key(&mut self, key: Key) {
        if self.capturing() {
            self.screens.ui_input.keys.push(key);
            return;
        }
        // In the game: Enter opens the chat line, Escape the menu (a replay has the menu too:
        // it is where Quit is).
        match key {
            Key::Enter if self.online.as_ref().is_some_and(|o| o.client.is_some()) => {
                self.screens.chat.open = true;
                self.release_keys();
            }
            // In the RPG mode a target is let go first (MODES.md 5.2).
            Key::Escape if self.rpg_mode() && self.rpg.target.is_some() => {
                self.rpg.clear_target();
            }
            Key::Escape if self.opts.bench_frames.is_none() => {
                self.screens.menu = Some(GameMenu::default());
                self.release_keys();
            }
            Key::Tab if self.rpg_mode() => self.rpg_cycle(),
            Key::Inventory => self.open_inventory(),
            Key::Equipment => self.open_equipment(),
            Key::People => self.open_people(),
            Key::Use => self.open_use(),
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
            self.screens
                .ui_input
                .text
                .extend(text.chars().filter(|c| !c.is_control()));
        }
    }

    /// The left button went down at `at` while a screen is up.
    pub(super) fn ui_press(&mut self, at: (f32, f32), double: bool) {
        self.pointer.cursor = at;
        self.screens.ui_input.pressed = true;
        self.screens.ui_input.down = true;
        self.screens.ui_input.double = double;
    }

    pub(super) fn ui_release(&mut self) {
        self.screens.ui_input.released = true;
        self.screens.ui_input.down = false;
    }

    /// What a person changed is written down; a run that changed nothing leaves the
    /// file alone (CLIENT.md 8).
    pub(super) fn keep_settings(&mut self) {
        if self.settings == self.prefs.kept {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = &self.prefs.path {
            self.settings.save(path);
        }
        #[cfg(target_arch = "wasm32")]
        self.settings.save();
        self.prefs.kept = self.settings.clone();
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
            self.pointer.pasting = Some(rx);
        }
    }

    /// What a paste brought, typed where the keyboard is now.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn pasted(&mut self) {
        let Some(rx) = &self.pointer.pasting else {
            return;
        };
        match rx.try_recv() {
            Ok(text) => {
                self.pointer.pasting = None;
                self.ui_text(&text);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => self.pointer.pasting = None,
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
                self.screens.bag = None;
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
            MenuAction::Resume => self.screens.menu = None,
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
                self.screens.menu = None;
            }
            MenuAction::Leave => self.leave_zone(""),
            MenuAction::Quit => event_loop.exit(),
            MenuAction::Changed => self.apply_settings(),
            // The tap that pressed the button is a gesture the browser still honours.
            MenuAction::PageFullscreen => {
                #[cfg(target_arch = "wasm32")]
                crate::web::tell_page("fullscreen", "toggle");
                self.screens.menu = None;
            }
        }
        match front {
            Action::None => {}
            Action::Session(account) => {
                if let Some(hub) = &self.session.hub {
                    hub.set_session(account.as_ref().map(|a| a.session));
                }
                if let Some(a) = &account
                    && self.returns_to_screens()
                {
                    self.settings.email = a.email.clone();
                }
                self.session.account = account;
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
        if let Some(front) = &mut self.screens.front {
            let at_login =
                self.screens.front_up && front.screen == Screen::Login && !front.on_autopilot();
            // The form is shown while the login screen is up, with the last refusal.
            let tell = match (at_login, front.busy()) {
                (true, false) => format!("login {}", front.notice()),
                (true, true) => "login-wait ".to_string(),
                (false, _) => "screen ".to_string(),
            };
            if tell != self.web.told {
                if let Some((kind, text)) = tell.split_once(' ') {
                    crate::web::tell_page(kind, text);
                }
                self.web.told = tell;
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
        if self.pointer.grabbed && !locked {
            let waited = self
                .pointer
                .grab_asked
                .is_none_or(|at| at.elapsed().as_secs_f32() > 0.5);
            if self.web.was_locked {
                self.pointer.grabbed = false;
                if self.screens.chat.open {
                    self.screens.chat.drop_line();
                } else if !self.screen_up() {
                    self.ui_key(Key::Escape);
                }
            } else if waited {
                // It was asked for and never given: the next click asks again.
                self.pointer.grabbed = false;
            }
        }
        // Given later than it was waited for: it is the game's all the same.
        if locked && !self.pointer.grabbed && self.wants_pointer() {
            self.pointer.grabbed = true;
        }
        self.web.was_locked = locked;
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
            self.screens.ui.typing_in().map(|label| {
                let head = format!("{label}: ");
                self.screens
                    .ui
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
        if field != self.web.told_field {
            match &field {
                Some(value) => crate::web::tell_page("field", value),
                None => crate::web::tell_page("no-field", ""),
            }
            self.web.told_field = field;
        }
        // The fields' places, for a finger (a mouse needs no keyboard brought up).
        let mut rects = Vec::new();
        if self.pointer.fingers.seen && self.capturing() {
            let dpr = self
                .active
                .as_ref()
                .map_or(1.0, |a| a.window.scale_factor() as f32)
                .max(0.5);
            for s in self
                .screens
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
        if rects != self.web.told_fields {
            crate::web::tell_fields(&rects);
            self.web.told_fields = rects;
        }
    }

    /// What the script does this frame goes in where a person's events would (CLIENT.md 9).
    /// `false`: the script failed and the program is ending.
    pub(super) fn run_script(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(script) = &mut self.screens.ui_script else {
            return true;
        };
        if !std::mem::take(&mut self.screens.ui_drawn) {
            return true;
        }
        let events = match script.step(&self.screens.ui, Instant::now()) {
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
                crate::script::Event::Move(at) => self.pointer.cursor = at,
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
                        if let Some(hub) = &self.session.hub {
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
        if self.settings.ui_scale != 0 || !self.pointer.fingers.seen {
            return self.settings.ui_scale;
        }
        let dpr = self
            .active
            .as_ref()
            .map_or(1.0, |a| a.window.scale_factor() as f32);
        ui::touch_scale(dpr)
    }
}
