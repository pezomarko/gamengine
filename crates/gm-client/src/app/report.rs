use super::*;

impl App {
    /// One line of what the client measured since the last one (`--report`, WEB.md 8):
    /// the frames, the models, the zone and the hotbar, the UI's scale, where the body
    /// is, and what was heard.
    pub(super) fn report_line(&mut self) -> String {
        let r = self.stats.report_since(self.report_frame);
        self.report_frame = self.stats.frames();
        let mut line = format!(
            "fps={:.1} frame_ms_avg={:.2} frame_ms_p99={:.2} frame_ms_max={:.2}",
            r.fps_avg, r.ms_avg, r.ms_p99, r.ms_max
        );
        self.report_models(&mut line);
        self.report_zone(&mut line);
        self.report_ui(&mut line);
        // What was heard, in the same line (the gates read it as `GM-DONE ... sound: ...`).
        line.push(' ');
        line.push_str(&self.sound.report());
        line
    }

    /// The renderer and the model cache, when there is a renderer.
    fn report_models(&self, line: &mut String) {
        let Some(a) = &self.active else { return };
        let m = a.avatars.stats();
        line.push_str(&format!(
            " backend={:?} draw_calls={} characters={} with_model={} models={} fetched={} fetched_bytes={} cache_hits={} failed={} refused={}",
            a.gpu.info.backend,
            a.renderer.draw_calls,
            a.renderer.characters.drawn,
            a.avatars.with_model,
            m.ready,
            m.fetched,
            m.fetched_bytes,
            m.disk_hits,
            m.failed,
            m.refused
        ));
        #[cfg(target_arch = "wasm32")]
        if let Some(cache) = &a.avatars.cache {
            line.push_str(&format!(
                " cache_bytes={} cache_cap_bytes={}",
                cache.loader.store.total(),
                cache.loader.store.cap()
            ));
        }
    }

    /// The zone played: the prediction's statistics, the score, the hotbar as drawn
    /// (LOOK.md 3.2: key, ability, state, and how ready), the item cells (key, what is
    /// set, how many carried) with the last refusal's words while they show, what is
    /// held, and the bytes over the wire in a browser.
    fn report_zone(&self, line: &mut String) {
        let Some(o) = &self.online else { return };
        if let Some(c) = &o.client {
            let s = c.stats;
            line.push_str(&format!(
                " entity={} snapshots={} gaps={} max_gap={} corrections={} unexplained={} inputs={} health={}",
                c.my_id,
                s.snapshots,
                s.gaps,
                s.max_gap,
                s.corrections,
                s.corrections_unexplained,
                s.inputs_sent,
                c.own_health
            ));
        }
        line.push_str(&format!(
            " kills={} deaths={} players={} zone={}",
            o.kills,
            o.deaths,
            o.names.len(),
            if o.zone_name.is_empty() {
                "-"
            } else {
                &o.zone_name
            }
        ));
        let cells: Vec<String> = hotbar(o)
            .iter()
            .map(|c| format!("{}:{}:{}:{:.2}", c.key, c.ability, c.state, c.ready))
            .collect();
        if !cells.is_empty() {
            line.push_str(&format!(" hotbar={}", cells.join(",")));
        }
        if let Some(c) = &o.client {
            let items: Vec<String> = item_cells(o, c)
                .iter()
                .map(|i| format!("{}:{}:{}", i.key, i.ability, i.count.unwrap_or(0)))
                .collect();
            line.push_str(&format!(" items={}", items.join(",")));
            if let Some(why) = kit_note_of(o.kit_note) {
                line.push_str(&format!(" item_refused={}", why.replace(' ', "-")));
            }
        }
        let held: Vec<String> = o
            .looks
            .iter()
            .filter_map(|(id, l)| o.props.get(l.held as usize).map(|k| format!("{id}:{k}")))
            .collect();
        line.push_str(&format!(
            " held={} props_loaded={}",
            held.len(),
            self.content
                .props
                .values()
                .filter(|p| matches!(p, crate::content::PropState::Loaded(_)))
                .count()
        ));
        #[cfg(target_arch = "wasm32")]
        {
            let (rx, tx) = o.net.bytes();
            line.push_str(&format!(" rx_bytes={rx} tx_bytes={tx}"));
        }
    }

    /// The UI's scale and the density of the atlas it is drawn with (LOOK.md 2.2): the
    /// same number when the bundle has that atlas, offline too (the phone gate); where
    /// the body is and looks (offline, the local walk's), and whether a finger has
    /// touched the screen (WEB.md 3.5); a browser's memory and its first frame's time.
    fn report_ui(&self, line: &mut String) {
        if let Some(a) = &self.active {
            line.push_str(&format!(
                " ui_scale={} atlas_density={} size={}x{} dpr={:.3}",
                ui::scale_for(a.renderer.hud.size, PANEL_UNITS, self.ui_scale_choice()),
                a.renderer.hud.density(),
                a.config.width,
                a.config.height,
                a.window.scale_factor()
            ));
        }
        let pos = self
            .online
            .as_ref()
            .and_then(|o| o.client.as_ref())
            .map_or(self.sim.curr.origin, |c| c.mover.mv.origin);
        let axes = self.input.axes();
        line.push_str(&format!(
            " yaw={:.0} pos={:.0},{:.0},{:.0} touch={} axes={:.2},{:.2}",
            self.sim.yaw, pos.x, pos.y, pos.z, self.fingers.seen as u8, axes.0, axes.1
        ));
        #[cfg(target_arch = "wasm32")]
        line.push_str(&format!(
            " wasm_memory_bytes={} first_frame_ms={:.0}",
            crate::web::wasm_memory_bytes(),
            self.first_frame_ms
        ));
    }
}
