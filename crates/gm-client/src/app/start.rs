use super::*;

/// What the command line (or the page) already said of the way in (CLIENT.md 2): with a
/// user and a password the screens are taken by themselves as far as it reaches.
pub(super) fn auto_of(opts: &Options) -> Option<Auto> {
    (!opts.user.is_empty()).then(|| Auto {
        email: opts.user.clone(),
        password: opts.password.clone(),
        register: opts.register,
        character: opts.character.clone(),
        preset: opts.build.clone(),
        zone: opts.zone.clone(),
    })
}

/// What a client is started with besides its map (CLIENT.md 2).
pub(super) struct Start {
    pub(super) online: Option<Online>,
    pub(super) hub: Option<Hub>,
    pub(super) front: Option<Front>,
    pub(super) settings: Settings,
    pub(super) map_hash: u64,
    pub(super) ui_script: Option<UiScript>,
    /// The content bundle (CONTENT.md 6), or a client without one.
    pub(super) content: Content,
}

pub(super) fn app(opts: Options, bsp: Bsp, palette: world::Palette, sim: Sim, start: Start) -> App {
    let mesh = world::build(&bsp, &palette);
    let faces_total = mesh
        .face_ranges
        .iter()
        .filter(|r| r.index_count > 0)
        .count();
    let viewport = if opts.third_person {
        Viewport::Third
    } else {
        Viewport::First
    };
    let touch = opts.touch;
    let mut app = App {
        view_kick: 0.0,
        view_swing: 0.0,
        view_stride: 0.0,
        view_model: None,
        content: start.content,
        offline_prop: None,
        offline_worn: Vec::new(),
        offline_off: None,
        offline_sheathed: false,
        front_up: start.front.is_some() && start.online.is_none(),
        title: None,
        hub: start.hub,
        account: None,
        front: start.front,
        menu: None,
        chat: Chat::default(),
        bag: None,
        people: None,
        gm_page: None,
        character_page: None,
        party_view: Vec::new(),
        sound: crate::sound::Sound::new(
            start.settings.volume,
            start.settings.mute,
            opts.sound_dump.clone(),
        ),
        sound_from: None,
        character: None,
        ui: UiState::default(),
        ui_input: UiInput::default(),
        cursor: (0.0, 0.0),
        last_press: None,
        shift: false,
        command: false,
        #[cfg(not(target_arch = "wasm32"))]
        pasting: None,
        leaving: Vec::new(),
        hub_touched: Instant::now(),
        settings_kept: start.settings.clone(),
        #[cfg(not(target_arch = "wasm32"))]
        settings_path: None,
        settings: start.settings,
        ui_script: start.ui_script,
        map_hash: start.map_hash,
        was_up: false,
        ui_drawn: false,
        grab_asked: None,
        #[cfg(target_arch = "wasm32")]
        was_locked: false,
        #[cfg(target_arch = "wasm32")]
        told: String::new(),
        #[cfg(target_arch = "wasm32")]
        told_field: None,
        #[cfg(target_arch = "wasm32")]
        told_fields: Vec::new(),
        zone_ended: None,
        opts,
        window: None,
        bsp,
        mesh: Some(mesh),
        active: None,
        sim,
        online: start.online,
        input: Input::default(),
        viewport,
        stats: FrameStats::new(),
        last_frame: Instant::now(),
        last_title: Instant::now(),
        started: Instant::now(),
        title_frame: 0,
        grabbed: false,
        adapter_info: None,
        faces_total,
        exit_requested: false,
        acquire_timeouts: 0,
        entities: Vec::new(),
        bodies: Vec::new(),
        revoked: Vec::new(),
        bench_yaw0: 0.0,
        aim: (0.0, 0.0),
        pending_map: None,
        palette,
        script_target: None,
        last_report: Instant::now(),
        report_frame: 0,
        #[cfg(target_arch = "wasm32")]
        pending_gpu: Default::default(),
        #[cfg(target_arch = "wasm32")]
        pending_fetch: Default::default(),
        #[cfg(target_arch = "wasm32")]
        first_frame_ms: 0.0,
        #[cfg(not(target_arch = "wasm32"))]
        playback: None,
        effects: Default::default(),
        fx: Default::default(),
        tags: Vec::new(),
        pops: Vec::new(),
        combo: (0, None),
        view_punch: (0.0, 0.0),
        eye_drop: 0.0,
        zoom: 1.0,
        rpg: crate::rpg::Rpg::new(),
        last_vp: None,
        cursor_icon: CursorIcon::Default,
        rpg_right: None,
        fingers: Fingers::expecting(touch),
        touch_buttons: Vec::new(),
        tap_fire: None,
        facings: HashMap::new(),
        squad_view: Vec::new(),
        target_view: None,
    };
    app.bench_yaw0 = app.sim.yaw;
    app
}

/// Open the replay named on the command line and point the options at its map.
#[cfg(not(target_arch = "wasm32"))]
pub fn open_replay(opts: &mut Options) -> Result<Option<crate::playback::Playback>, Error> {
    let Some(path) = opts.replay.clone() else {
        return Ok(None);
    };
    let bytes = std::fs::read(&path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let playback = crate::playback::Playback::open(&bytes, opts.follow.as_deref(), opts.from)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    let header = &playback.replay.header;
    opts.map = opts.maps_dir.join(format!("{}.bsp", header.map));
    match std::fs::read(&opts.map) {
        Ok(map) if fnv1a64(&map) == header.map_hash => {}
        Ok(_) => log::warn!(
            "{} is not the build of the map the replay was recorded on: bodies may stand in walls",
            opts.map.display()
        ),
        Err(e) => return Err(format!("the replay's map {}: {e}", opts.map.display()).into()),
    }
    log::info!(
        "replay {}: {} on {}, {:.1} s, {} frames, following {}",
        path.display(),
        header.zone,
        header.map,
        playback.seconds(),
        playback.replay.frames.len(),
        playback.name(playback.follow)
    );
    Ok(Some(playback))
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run(mut opts: Options) -> Result<(), Error> {
    let playback = open_replay(&mut opts)?;
    let map_bytes =
        std::fs::read(&opts.map).map_err(|e| format!("loading {}: {e}", opts.map.display()))?;
    let bsp = Bsp::load(&opts.map).map_err(|e| format!("loading {}: {e}", opts.map.display()))?;
    log::info!(
        "loaded {} ({} faces, {} leaves)",
        opts.map.display(),
        bsp.faces.len(),
        bsp.leaves.len()
    );
    let palette = world::load_palette(&opts.palette);
    let sim = Sim::at(&bsp, opts.start);
    // A run that somebody scripted (a benchmark, a replay, a crowd) is not a person's: the
    // person's settings do not send it to a login screen, and do not change what it
    // measures either (a ticked "fullscreen"). It reads settings only when it names them.
    let personal = opts.bench_frames.is_none()
        && opts.seconds == 0.0
        && opts.script.is_none()
        && opts.replay.is_none()
        && opts.crowd == 0
        && opts.start.is_none()
        && !opts.offline
        && opts.connect.is_none();
    let settings_path = match &opts.settings {
        Some(path) => Some(path.clone()),
        None if personal => crate::settings::default_path(),
        None => None,
    };
    let settings = settings_path
        .as_deref()
        .map(Settings::load)
        .unwrap_or_default();
    // Where the hub is: the command line, else the person's settings, else what the build
    // ships beside the program (CLIENT.md 2). `--offline` and `--connect` ask for none.
    // A hub that is named and cannot be used is said on a screen, like one that is not
    // named at all: nobody started this from a terminal.
    let resolve = |hub: &str| {
        use std::net::ToSocketAddrs;
        hub.to_socket_addrs()
            .ok()
            .and_then(|mut found| found.next())
    };
    let read = |cert: &std::path::Path| {
        std::fs::read(cert).map_err(|e| {
            format!(
                "The hub's certificate ({}) cannot be read: {e}.",
                cert.display()
            )
        })
    };
    let named = |hub: &str, cert: std::path::PathBuf, by: &str| match resolve(hub) {
        Some(addr) => read(&cert).map(|der| Some((addr, der))),
        None => Err(format!("The hub {by} names ({hub}) cannot be found.")),
    };
    let hub_at: Result<Option<(std::net::SocketAddr, Vec<u8>)>, String> = match opts.hub {
        // A command line that is wrong ends the program, with the reason.
        Some(addr) => {
            let cert = std::fs::read(&opts.hub_cert)
                .map_err(|e| format!("reading hub certificate {}: {e}", opts.hub_cert.display()))?;
            Ok(Some((addr, cert)))
        }
        None if !personal => Ok(None),
        None if !settings.hub.is_empty() => named(
            &settings.hub,
            std::path::PathBuf::from(&settings.hub_cert),
            "the settings file",
        ),
        None => match crate::settings::site_hub(&crate::install_root()) {
            Some((hub, cert)) => named(&hub, cert, "this build"),
            None => Ok(None),
        },
    };
    // A person, and no hub to talk to: say so on a screen instead of walking off
    // (CLIENT.md 2).
    let title = match &hub_at {
        Ok(Some(_)) => None,
        _ if !personal => None,
        found => Some(crate::menu::Title {
            settings: settings_path
                .as_deref()
                .map_or_else(String::new, |p| p.display().to_string()),
            why: found.as_ref().err().cloned(),
        }),
    };
    let hub_at = hub_at.unwrap_or(None);
    // The command line's password has done its work once the screens have it.
    let auto = auto_of(&opts);
    opts.password.clear();
    let map_hash = fnv1a64(&map_bytes);
    let ui_script = opts.ui_script.as_deref().map(UiScript::parse).transpose()?;
    let content = Content::load(&crate::install_root().join(&opts.assets));
    let start = match (hub_at, opts.connect) {
        // Through the hub: the screens, or the command line in their place.
        (Some((addr, cert_der)), _) if playback.is_none() => {
            let hub = Hub::new(&ZoneAddr {
                addr: Some(addr),
                cert_der,
                web: None,
            })?;
            let front = Front::new(
                Box::new(hub.clone()),
                settings.email.clone(),
                settings.character.clone(),
                auto,
            );
            Start {
                online: None,
                hub: Some(hub),
                front: Some(front),
                settings,
                map_hash,
                ui_script,
                content,
            }
        }
        (_, Some(addr)) => {
            let cert = std::fs::read(&opts.cert)
                .map_err(|e| format!("reading zone certificate {}: {e}", opts.cert.display()))?;
            let entry = Entry {
                zone: ZoneAddr {
                    addr: Some(addr),
                    cert_der: cert,
                    web: None,
                },
                token: Vec::new(),
                zone_name: String::new(),
            };
            Start {
                online: Some(online(&opts, &sim, map_hash, entry)?),
                hub: None,
                front: None,
                settings,
                map_hash,
                ui_script,
                content,
            }
        }
        _ => Start {
            online: None,
            hub: None,
            front: None,
            settings,
            map_hash,
            ui_script,
            content,
        },
    };

    let event_loop = EventLoop::new()?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = app(opts, bsp, palette, sim, start);
    app.playback = playback;
    app.title = title;
    app.settings_path = settings_path;
    event_loop.run_app(&mut app)?;
    if let Some(o) = &mut app.online {
        o.net.close();
    }
    // Goodbyes still on their way are waited for: the zone frees the body now.
    for (mut net, _) in app.leaving.drain(..) {
        net.close();
    }
    if let Some(hub) = &app.hub {
        if app.account.is_some() {
            log::info!("logging out of the hub");
        }
        hub.logout();
    }
    app.keep_settings();

    let report = app.stats.report();
    let scripted = app.opts.bench_frames.is_some() || app.opts.seconds > 0.0;
    if let (true, Some(info)) = (scripted, &app.adapter_info) {
        let faces = app.active.as_ref().map_or(0, |a| a.renderer.faces_drawn);
        let draws = app.active.as_ref().map_or(1, |a| a.renderer.draw_calls);
        print_bench(&report, info, "windowed", faces, app.faces_total, draws);
    } else if report.frames > 0 {
        let (_, peak) = crate::stats::rss_bytes();
        log::info!(
            "{} frames, {:.1} fps average, peak RSS {} bytes ({:.1} MiB)",
            report.frames,
            report.fps_avg,
            peak,
            peak as f64 / 1048576.0
        );
    }
    if let Some(a) = &app.active {
        print_bench_avatars(&report, &a.avatars, &a.renderer);
    }
    log::info!("{}", app.sound.report());
    if scripted || app.opts.sound_dump.is_some() {
        println!("{}", app.sound.report());
    }
    if let Some(said) = app.sound.finish() {
        log::info!("{said}");
    }
    if let Some(o) = &app.online
        && let Some(c) = &o.client
    {
        let s = c.stats;
        log::info!(
            "net: {} snapshots, {} gaps (max {}), {} corrections ({} unexplained, max {:.1} u), {} inputs sent, delay {} ticks",
            s.snapshots,
            s.gaps,
            s.max_gap,
            s.corrections,
            s.corrections_unexplained,
            s.max_correction,
            s.inputs_sent,
            c.delay_ticks
        );
    }
    if app.exit_requested {
        return Err("exited on error".into());
    }
    Ok(())
}

/// A map and its coloured lightmaps from the page's assets, with the hash of the `.bsp`.
/// With `wanted` (the hash the zone told, WEB.md 5) the URLs carry it as a stamp: a browser
/// that kept last build's map in its cache has nothing under this name and asks the site
/// (found 2026-10-06: a map rebuilt on the site, the old one played from the LAN).
#[cfg(target_arch = "wasm32")]
pub(super) async fn fetch_map(
    assets: &str,
    name: &str,
    wanted: Option<u64>,
) -> Result<(Bsp, u64), String> {
    /// A map that has not arrived in this long is not coming (the zone waits 60 s for a
    /// client's first input).
    const FETCH_MS: u32 = 45_000;
    let stamp = wanted.map_or(String::new(), |h| format!("?v={h:016x}"));
    let url = format!("{assets}/maps/{name}.bsp{stamp}");
    let fetch = async {
        let bytes = crate::web::fetch_bytes(&url)
            .await?
            .ok_or_else(|| format!("{url}: no such map on this site"))?;
        let mut bsp = Bsp::parse(&bytes).map_err(|e| format!("{url}: {e}"))?;
        if let Some(lit) =
            crate::web::fetch_bytes(&format!("{assets}/maps/{name}.lit{stamp}")).await?
        {
            bsp.attach_lit(&lit).map_err(|e| format!("{url}: {e}"))?;
        }
        Ok((bsp, fnv1a64(&bytes)))
    };
    crate::web::timeout_ms(FETCH_MS, fetch)
        .await
        .unwrap_or_else(|| Err(format!("{url}: the download took too long")))
}

/// The browser client (WEB.md 3, 5): options from the page, assets by `fetch`, the login
/// awaited, then the event loop on the page's own.
#[cfg(target_arch = "wasm32")]
pub async fn run_web() -> Result<(), String> {
    use crate::web::{PageOptions, parse_hash, tell_page};
    use gm_net::control::WebAddr;
    use winit::platform::web::EventLoopExtWebSys;

    let page = PageOptions::read();
    let web_addr = |url: &str, hash: &str| -> Result<Option<WebAddr>, String> {
        let Some(url) = page.string(url) else {
            return Ok(None);
        };
        let cert_sha256 = match page.string(hash) {
            Some(h) => Some(parse_hash(&h).ok_or("a certificate hash must be 64 hex digits")?),
            None => None,
        };
        Ok(Some(WebAddr { url, cert_sha256 }))
    };
    let mut opts = Options {
        name: page.string("name").unwrap_or_else(|| "web".into()),
        build: page.string("build"),
        team: page.number("team").unwrap_or(0.0) as u8,
        third_person: page.flag("third-person"),
        user: page.string("user").unwrap_or_default(),
        password: page.take("password").unwrap_or_default(),
        register: page.flag("register"),
        character: page.string("character").unwrap_or_default(),
        seconds: page.number("seconds").unwrap_or(0.0) as f32,
        script: page.string("script"),
        report: page.flag("report"),
        travel_to: page.string("travel-to"),
        travel_after: page.number("travel-after").unwrap_or(0.0) as f32,
        cache_mb: page.number("cache-mb").unwrap_or(128.0) as u64,
        hub_web: web_addr("hub", "hub-cert")?,
        connect_web: web_addr("connect", "cert")?,
        assets: page.string("assets").unwrap_or_else(|| "assets".into()),
        ui_script: page.string("ui-script"),
        touch: page.flag("touch"),
        ..Options::default()
    };
    if let Some(zone) = page.string("zone") {
        opts.zone = zone;
    }
    if let Some(mb) = page.number("vram-mb") {
        opts.vram_mb = mb as u64;
    }
    let map = page.string("map").unwrap_or_else(|| "test_room".into());
    if !valid_map_name(&map) {
        return Err(format!("not a map name: {map:?}"));
    }

    tell_page("status", "loading the map");
    let (bsp, map_hash) = fetch_map(&opts.assets, &map, None).await?;
    let palette = world::palette_from_bytes(
        crate::web::fetch_bytes(&format!("{}/textures/palette.lmp", opts.assets))
            .await?
            .as_deref(),
    );
    let sim = Sim::at(&bsp, opts.start);
    let settings = Settings::load();
    let ui_script = opts.ui_script.as_deref().map(UiScript::parse).transpose()?;
    let content = Content::fetch(&opts.assets).await;
    // Through the hub: the screens, or the page's options in their place (the page's form
    // gave the email and the password, CLIENT.md 4.1).
    let through_hub = opts.hub_web.is_some() && opts.connect_web.is_none();
    let start = if through_hub {
        let hub = Hub::new(&ZoneAddr {
            addr: None,
            cert_der: Vec::new(),
            web: opts.hub_web.clone(),
        })
        .map_err(|e| e.to_string())?;
        let front = Front::new(
            Box::new(hub.clone()),
            settings.email.clone(),
            settings.character.clone(),
            auto_of(&opts),
        );
        // The password has done its work; nothing keeps it (WEB.md 5).
        opts.password.clear();
        Start {
            online: None,
            hub: Some(hub),
            front: Some(front),
            settings,
            map_hash,
            ui_script,
            content,
        }
    } else {
        let online = match opts.connect_web.clone() {
            Some(web) => {
                tell_page("status", "connecting to the zone");
                let entry = Entry {
                    zone: ZoneAddr {
                        addr: None,
                        cert_der: Vec::new(),
                        web: Some(web),
                    },
                    token: Vec::new(),
                    zone_name: String::new(),
                };
                Some(online(&opts, &sim, map_hash, entry).map_err(|e| e.to_string())?)
            }
            None => None,
        };
        Start {
            online,
            hub: None,
            front: None,
            settings,
            map_hash,
            ui_script,
            content,
        }
    };
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    // Frames come from `requestAnimationFrame`: each redraw asks for the next.
    event_loop.set_control_flow(ControlFlow::Wait);
    event_loop.spawn_app(app(opts, bsp, palette, sim, start));
    Ok(())
}
