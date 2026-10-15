//! What goes out every tick: the snapshots over the pool and their datagrams.

use super::*;

impl Run {
    /// The snapshots (PROTOCOL.md 3): the shared table and the PVS rows once, each session's
    /// delta on the pool, the datagrams out.
    pub(super) fn snapshots(&mut self) {
        let tick = self.zone.tick;
        let t_snap = Instant::now();
        self.table.rebuild(&self.zone, &self.world, &|id| {
            wire_kind(self.director.kind_of(id))
        });
        for s in self.sessions.values() {
            if let Some(leaf) = s.eye_leaf(&self.zone, &self.world) {
                self.pvs.prepare(&self.world.bsp, leaf);
            }
            // Squad sight: the rows of a commanding client's companions (COMPANIONS.md 5.2).
            if self
                .zone
                .player(s.id)
                .is_some_and(crate::session::commanding)
            {
                for m in self.director.squad(s.id) {
                    if let Some(p) = self.zone.player(m.id).filter(|p| p.alive) {
                        self.pvs.prepare(
                            &self.world.bsp,
                            self.world.bsp.leaf_for_point(p.mover.eye()),
                        );
                    }
                }
            }
        }
        // Each session's snapshot only reads the zone, the table and the PVS rows; the
        // per-session history is its own. Spread them over the pool (rayon).
        let outgoing: Vec<(EntityId, Vec<u8>)> = {
            let zone = &self.zone;
            let world = &*self.world;
            let table = &self.table;
            let pvs = &self.pvs;
            let mut slots: Vec<&mut Session> = self.sessions.values_mut().collect();
            slots
                .par_iter_mut()
                .filter_map(|s| {
                    s.build_snapshot(zone, world, table, pvs, tick)
                        .map(|bytes| (s.id, bytes))
                })
                .collect()
        };
        self.phases.snapshot += t_snap.elapsed();
        let t_send = Instant::now();
        for (id, bytes) in outgoing {
            if let Some(s) = self.sessions.get_mut(&id)
                && s.conn.send_datagram(Bytes::from(bytes)).is_err()
            {
                s.send_failures += 1;
            }
        }
        self.phases.send += t_send.elapsed();
    }
}
