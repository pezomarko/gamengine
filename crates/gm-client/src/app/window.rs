use super::*;

pub(super) struct Active {
    pub(super) window: Arc<Window>,
    pub(super) surface: wgpu::Surface<'static>,
    pub(super) config: wgpu::SurfaceConfiguration,
    /// The format frames are drawn in: the surface's, or its sRGB twin as a view format
    /// where the surface itself is not sRGB (a WebGPU canvas).
    pub(super) view_format: wgpu::TextureFormat,
    pub(super) gpu: Gpu,
    pub(super) renderer: Renderer,
    pub(super) avatars: Avatars,
    /// The leaves the visible faces were last gathered from.
    pub(super) drawn_from: Vec<usize>,
}

/// The frame's size in device pixels: the window's natively. In a browser the page owns
/// the canvas's CSS size and winit never sets its backing size (WEB.md 3.5), so a canvas
/// on a phone of 2.6 device pixels a CSS pixel was drawn at a third of its pixels, and
/// fingers (reported in device pixels) landed outside the frame: the backing is the CSS
/// size times the device pixel ratio, which configuring the surface sets on the canvas.
pub(super) fn frame_size(window: &Window) -> winit::dpi::PhysicalSize<u32> {
    #[cfg(target_arch = "wasm32")]
    if let Some(size) = crate::web::canvas_device_size() {
        return size;
    }
    window.inner_size()
}

impl App {
    /// Give up: say why (on the page too, in a browser) and leave the loop.
    pub(super) fn fail(&mut self, event_loop: &ActiveEventLoop, why: &str) {
        log::error!("{why}");
        #[cfg(target_arch = "wasm32")]
        crate::web::tell_page("error", why);
        self.clock.exit_requested = true;
        event_loop.exit();
    }

    /// The window has its device: configure the surface and build the renderer on it.
    pub(super) fn activate(
        &mut self,
        window: Arc<Window>,
        surface: wgpu::Surface<'static>,
        gpu: Gpu,
    ) -> Result<(), Error> {
        let (config, view_format) = self.configure(&window, &surface, &gpu)?;
        let mesh = self
            .world
            .mesh
            .take()
            .ok_or("world mesh already consumed")?;
        let mut renderer = Renderer::new(&gpu, view_format, &mesh, (config.width, config.height));
        // The bundle's atlas is the HUD's texture from here on (LOOK.md 2.2).
        if let Some(atlas) = self.content.take_atlas() {
            renderer.hud.set_atlas(&gpu, atlas);
        }
        // Models come from the hub, on whatever session is logged in when one is wanted.
        let source = self.session.hub.as_ref().map(|h| h.model_source());
        let avatars = Avatars::new(
            &gpu,
            &mut renderer.characters,
            &self.opts,
            &self.bsp,
            source,
        )?;
        self.clock.adapter_info = Some(gpu.info.clone());
        self.active = Some(Active {
            window: window.clone(),
            surface,
            config,
            view_format,
            gpu,
            renderer,
            avatars,
            drawn_from: vec![usize::MAX],
        });
        self.fitting_room();
        // A browser gives the pointer only to a click (WEB.md 3.4): there the first click grabs.
        if cfg!(not(target_arch = "wasm32")) && self.wants_pointer() && self.focused() {
            self.set_grab(true);
        }
        self.screens.was_up = self.screen_up();
        self.apply_settings();
        self.clock.last_frame = Instant::now();
        self.clock.started = Instant::now();
        // Offline there is nothing left to wait for; online the status clears at `Welcome`.
        #[cfg(target_arch = "wasm32")]
        if self.online.is_none() {
            crate::web::tell_page("status", "");
        }
        window.request_redraw();
        Ok(())
    }

    /// The surface configured for the window: frames are drawn in sRGB (where the
    /// surface has no sRGB format of its own, a WebGPU canvas being `bgra8unorm`, its
    /// sRGB twin is asked for as a view format), and presented as asked, or by Mailbox:
    /// no tearing, no blocking, and it works where Fifo stalls (X11 with a compositing
    /// window manager on RADV presented one frame per second); a benchmark measures
    /// throughput, so it runs without vsync unless told otherwise.
    fn configure(
        &self,
        window: &Window,
        surface: &wgpu::Surface<'static>,
        gpu: &Gpu,
    ) -> Result<(wgpu::SurfaceConfiguration, wgpu::TextureFormat), Error> {
        let size = frame_size(window);
        let caps = surface.get_capabilities(&gpu.adapter);
        let mut config = surface
            .get_default_config(&gpu.adapter, size.width.max(1), size.height.max(1))
            .ok_or("surface not supported by the adapter")?;
        let view_format = match caps.formats.iter().copied().find(|f| f.is_srgb()) {
            Some(format) => {
                config.format = format;
                format
            }
            None => {
                config.format = caps.formats[0];
                let srgb = config.format.add_srgb_suffix();
                if srgb != config.format {
                    config.view_formats.push(srgb);
                }
                srgb
            }
        };
        let bench = self.opts.bench_frames.is_some();
        config.present_mode = match self.opts.present {
            Some(mode) if caps.present_modes.contains(&mode) => mode,
            Some(mode) => {
                log::warn!(
                    "present mode {mode:?} unsupported here (available: {:?}); using the default",
                    caps.present_modes
                );
                wgpu::PresentMode::AutoVsync
            }
            // A browser presents with the display, whatever is asked.
            None if cfg!(target_arch = "wasm32") => wgpu::PresentMode::AutoVsync,
            None if bench || !self.opts.vsync => wgpu::PresentMode::AutoNoVsync,
            None if caps.present_modes.contains(&wgpu::PresentMode::Mailbox) => {
                wgpu::PresentMode::Mailbox
            }
            None => wgpu::PresentMode::AutoVsync,
        };
        log::info!("present modes available: {:?}", caps.present_modes);
        config.desired_maximum_frame_latency = 2;
        surface.configure(&gpu.device, &config);
        log::info!(
            "surface {}x{} {:?} (drawn as {:?}) {:?}",
            config.width,
            config.height,
            config.format,
            view_format,
            config.present_mode
        );
        Ok((config, view_format))
    }

    /// The fitting rooms of the command line (CONTENT.md 9): `--prop KEY` in the own
    /// hand and the crowd's, from the bundle (natively on the GPU at once; in a browser
    /// it arrives later and nobody holds it); `--off KEY` in the left hand; `--worn
    /// KEY[,KEY...]` the armours' overlays on the striker's body (CONTENT.md 3.3,
    /// EQUIPMENT.md 5.2); `--sheathed` the prop on the back (LOOK.md 6.7).
    fn fitting_room(&mut self) {
        let Some(a) = self.active.as_mut() else {
            return;
        };
        if let Some(key) = self.opts.prop.clone() {
            let (gpu, characters) = (&a.gpu, &mut a.renderer.characters);
            let slot = self
                .content
                .prop(&key, |model| Some(characters.add_model(gpu, model)));
            if slot.is_none() {
                log::warn!("--prop {key}: the bundle has no such prop on the desktop");
            }
            self.offline.prop = slot;
            a.avatars.crowd_prop = slot;
        }
        if let Some(key) = self.opts.off.clone() {
            let (gpu, characters) = (&a.gpu, &mut a.renderer.characters);
            let slot = self
                .content
                .prop(&key, |model| Some(characters.add_model(gpu, model)));
            if slot.is_none() {
                log::warn!("--off {key}: the bundle has no such prop on the desktop");
            }
            if let Some(slot) = slot
                && self.content.is_shield(&key)
            {
                a.avatars.flat.insert(slot);
            }
            self.offline.off = slot;
            a.avatars.crowd_off = slot;
        }
        if let Some(keys) = self.opts.worn.clone() {
            let (gpu, characters, avatars) = (&a.gpu, &mut a.renderer.characters, &mut a.avatars);
            let mut worn = Vec::new();
            let mut patches = Vec::new();
            for key in keys.split(',').map(str::trim).filter(|k| !k.is_empty()) {
                if self.content.is_patch(key) {
                    // The patch for whichever official striker the bundle has (one class).
                    let id = (0..4u8).find_map(|armour| {
                        self.content
                            .patch(key, 1, armour, |p| Some(avatars.add_patch(p.clone())))
                    });
                    match id {
                        Some(id) => patches.push(id),
                        None => {
                            log::warn!("--worn {key}: the bundle has no such patch for the striker")
                        }
                    }
                    continue;
                }
                let slot = self
                    .content
                    .overlay(key, 1, |model| Some(characters.add_model(gpu, model)));
                match slot {
                    Some(s) => worn.push((s, self.content.covers(key))),
                    None => {
                        log::warn!("--worn {key}: the bundle has no such overlay for the striker")
                    }
                }
            }
            self.offline.worn = worn.clone();
            self.offline.patches = patches.clone();
            avatars.crowd_worn = worn;
            avatars.crowd_patches = patches;
        }
        if self.opts.sheathed {
            self.offline.sheathed = true;
            a.avatars.crowd_sheathed = true;
        }
    }

    /// The window has the keyboard. The pointer is taken by itself only then: a zone that
    /// lets the character in while the person looks at another window does not confine
    /// their pointer to this one. (A click on the window takes it whatever the focus.)
    pub(super) fn focused(&self) -> bool {
        self.active.as_ref().is_some_and(|a| a.window.has_focus())
    }

    pub(super) fn set_grab(&mut self, grab: bool) {
        let Some(a) = &self.active else { return };
        // In a browser the page asks for the pointer itself: a refusal there is an answer
        // the page takes, not an error left in the console.
        #[cfg(target_arch = "wasm32")]
        if grab {
            self.pointer.grab_asked = Some(Instant::now());
            crate::web::ask_for_pointer();
        } else {
            crate::web::give_pointer_back();
        }
        #[cfg(not(target_arch = "wasm32"))]
        if grab {
            self.pointer.grab_asked = Some(Instant::now());
            let ok = a
                .window
                .set_cursor_grab(CursorGrabMode::Confined)
                .or_else(|_| a.window.set_cursor_grab(CursorGrabMode::Locked))
                .is_ok();
            if !ok {
                log::warn!("cursor grab unsupported here; mouse look still works while focused");
            }
        } else {
            let _ = a.window.set_cursor_grab(CursorGrabMode::None);
        }
        a.window.set_cursor_visible(!grab);
        self.pointer.grabbed = grab;
    }

    pub(super) fn configure_surface(&mut self) {
        let Some(a) = &mut self.active else { return };
        let size = frame_size(&a.window);
        if size.width == 0 || size.height == 0 {
            return;
        }
        a.config.width = size.width;
        a.config.height = size.height;
        a.surface.configure(&a.gpu.device, &a.config);
        a.renderer.resize(&a.gpu, (size.width, size.height));
    }

    /// Replace the world (BSP, mesh, renderer) with another map.
    pub(super) fn switch_map(&mut self, bsp: Bsp, hash: u64) {
        self.rpg.forget_map();
        self.world.map_hash = hash;
        // A map that says what its air is (SOUND.md 3) is believed over its name.
        if let Some(air) = ambience_of(&bsp) {
            self.sound.air_named(air);
        }
        let mesh = world::build(&bsp, &self.world.palette);
        self.world.faces_total = mesh
            .face_ranges
            .iter()
            .filter(|r| r.index_count > 0)
            .count();
        if let Some(a) = &mut self.active {
            a.renderer.set_world(&a.gpu, &mesh);
            a.drawn_from = vec![usize::MAX];
        } else {
            self.world.mesh = Some(mesh);
        }
        self.sim = Sim::new(&bsp);
        self.bsp = bsp;
        log::info!(
            "switched map ({} faces, {} leaves)",
            self.bsp.faces.len(),
            self.bsp.leaves.len()
        );
    }
}
