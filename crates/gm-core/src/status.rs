//! Status effects on a mover (MATRIX.md 8): a fixed array of slots so the predicted mover
//! stays `Copy`, plus the multipliers the movement and damage code read.
//!
//! Times are in the mover's **frame ticks** (the same clock as cooldowns and scripts), so a
//! status expires identically on the client and the server.

use crate::matrix::CHILL_PER_STACK;
use crate::sim::tick_delta;
use crate::tick::{Tick, ms_ticks};
use crate::vocab::{ApplyStatus, StackRule, Status};

pub const MAX_STATUSES: usize = 8;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "bitcode", derive(bitcode::Encode, bitcode::Decode))]
pub struct StatusSlot {
    pub status: Option<Status>,
    /// Frame tick at which the slot expires.
    pub until: Tick,
    pub magnitude: f32,
    pub stacks: u8,
    /// Who applied it (0 = nobody / the world), for damage-over-time attribution.
    pub source: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Statuses {
    pub slots: [StatusSlot; MAX_STATUSES],
    /// Chill cannot be applied until this frame tick (after a Freeze).
    pub chill_immune_until: Tick,
    /// Stagger build-up is discarded until this frame tick (after a Stagger).
    pub stagger_immune_until: Tick,
}

/// Outcome of applying a status, for the caller's events.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
    Applied,
    Refreshed,
    /// Chill reached its stack limit: the target is now frozen (Root).
    Frozen,
    Immune,
    NoRoom,
}

impl Statuses {
    pub fn get(&self, status: Status) -> Option<&StatusSlot> {
        self.slots.iter().find(|s| s.status == Some(status))
    }

    pub fn has(&self, status: Status) -> bool {
        self.get(status).is_some()
    }

    /// Largest magnitude among slots of `status` (0 when absent).
    pub fn magnitude(&self, status: Status) -> f32 {
        self.slots
            .iter()
            .filter(|s| s.status == Some(status))
            .map(|s| s.magnitude)
            .fold(0.0, f32::max)
    }

    pub fn stacks(&self, status: Status) -> u32 {
        self.slots
            .iter()
            .filter(|s| s.status == Some(status))
            .map(|s| s.stacks as u32)
            .sum()
    }

    /// Bitmask over `Status::index` of the active statuses (the wire's cosmetic summary).
    pub fn mask(&self) -> u32 {
        self.slots
            .iter()
            .filter_map(|s| s.status)
            .fold(0, |m, s| m | (1u32 << s.index()))
    }

    /// Who taunted this body (MATRIX.md 8): the source of its Taunt, while it lasts.
    pub fn taunted_by(&self) -> Option<u32> {
        self.get(Status::Taunt)
            .map(|s| s.source)
            .filter(|&id| id != 0)
    }

    pub fn active(&self) -> impl Iterator<Item = &StatusSlot> {
        self.slots.iter().filter(|s| s.status.is_some())
    }

    pub fn clear(&mut self) {
        *self = Statuses::default();
    }

    fn drop_where(&mut self, drop: impl Fn(Status) -> bool) -> Applied {
        for slot in &mut self.slots {
            if slot.status.is_some_and(&drop) {
                *slot = StatusSlot::default();
            }
        }
        Applied::Applied
    }

    pub fn remove(&mut self, status: Status) {
        for s in &mut self.slots {
            if s.status == Some(status) {
                *s = StatusSlot::default();
            }
        }
    }

    /// Drop expired slots. `now` is the mover's frame tick.
    pub fn expire(&mut self, now: Tick) {
        for s in &mut self.slots {
            if s.status.is_some() && tick_delta(now, s.until) >= 0 {
                *s = StatusSlot::default();
            }
        }
    }

    /// Apply a verb at frame tick `now`; `duration` is already scaled by the defender's status
    /// duration factor (MATRIX.md 6) unless the status has a fixed duration.
    pub fn apply(
        &mut self,
        verb: &ApplyStatus,
        duration: Tick,
        now: Tick,
        source: u32,
        dt: f32,
    ) -> Applied {
        match verb.status {
            Status::Cleanse => return self.drop_where(|st| !st.is_boon()),
            Status::Dispel => return self.drop_where(Status::is_boon),
            _ => {}
        }
        let duration = duration.max(1);
        if verb.status == Status::Chill && tick_delta(now, self.chill_immune_until) < 0 {
            return Applied::Immune;
        }
        let existing = self
            .slots
            .iter()
            .position(|s| s.status == Some(verb.status));
        let mut outcome = Applied::Applied;
        match (verb.stacking, existing) {
            (StackRule::Independent, _) | (_, None) => {
                let Some(free) = self.slots.iter().position(|s| s.status.is_none()) else {
                    return Applied::NoRoom;
                };
                self.slots[free] = StatusSlot {
                    status: Some(verb.status),
                    until: now.wrapping_add(duration),
                    magnitude: verb.magnitude,
                    stacks: 1,
                    source,
                };
            }
            (StackRule::Refresh, Some(i)) => {
                let s = &mut self.slots[i];
                s.until = now.wrapping_add(duration);
                s.magnitude = s.magnitude.max(verb.magnitude);
                s.stacks = (s.stacks + 1).min(verb.max_stacks.max(1));
                s.source = source;
                outcome = Applied::Refreshed;
            }
            (StackRule::Extend, Some(i)) => {
                let s = &mut self.slots[i];
                let remaining = tick_delta(s.until, now).max(0) as Tick;
                let cap = duration.saturating_mul(3);
                s.until = now.wrapping_add((remaining + duration).min(cap).max(remaining));
                s.magnitude = s.magnitude.max(verb.magnitude);
                s.stacks = (s.stacks + 1).min(verb.max_stacks.max(1));
                s.source = source;
                outcome = Applied::Refreshed;
            }
        }
        if verb.status == Status::Chill && self.stacks(Status::Chill) >= verb.max_stacks as u32 {
            // Freeze (MATRIX.md 8): root, clear the chill, immunity.
            self.remove(Status::Chill);
            let root = ApplyStatus {
                status: Status::Root,
                duration: 1,
                magnitude: 1.0,
                max_stacks: 1,
                stacking: StackRule::Refresh,
                target: verb.target,
                dispellable: verb.dispellable,
            };
            let freeze_ticks = ms_ticks(crate::matrix::FREEZE_MS, dt);
            self.apply(&root, freeze_ticks, now, source, dt);
            self.chill_immune_until = now
                .wrapping_add(freeze_ticks)
                .wrapping_add(ms_ticks(crate::matrix::CHILL_IMMUNITY_MS, dt));
            return Applied::Frozen;
        }
        outcome
    }

    /// Movement speed multiplier from Slow, Haste, Chill and Stagger; 0 when rooted.
    pub fn speed_scale(&self) -> f32 {
        if self.has(Status::Root) || self.downed() {
            return 0.0;
        }
        let mut s = 1.0;
        s *= 1.0 - self.magnitude(Status::Slow).clamp(0.0, 1.0);
        s *= 1.0 + self.magnitude(Status::Haste).max(0.0);
        s *= (1.0 - CHILL_PER_STACK * self.stacks(Status::Chill) as f32).max(0.0);
        if self.has(Status::Stagger) {
            s *= 0.3;
        }
        s
    }

    /// Stagger: no activation, no guard.
    pub fn staggered(&self) -> bool {
        self.has(Status::Stagger)
    }

    /// On the ground (MODES.md 4.5): no activation, no guard, no movement of its own.
    pub fn downed(&self) -> bool {
        self.has(Status::Knockdown) || self.has(Status::Launched)
    }

    pub fn silenced(&self) -> bool {
        self.has(Status::Silence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vocab::StatusTarget;

    const DT: f32 = 1.0 / 64.0;

    fn verb(status: Status, stacking: StackRule, max_stacks: u8, magnitude: f32) -> ApplyStatus {
        ApplyStatus {
            status,
            duration: 64,
            magnitude,
            max_stacks,
            stacking,
            target: StatusTarget::Hit,
            dispellable: true,
        }
    }

    #[test]
    fn refresh_extend_independent_and_expiry() {
        let mut st = Statuses::default();
        let slow = verb(Status::Slow, StackRule::Refresh, 1, 0.3);
        assert_eq!(st.apply(&slow, 64, 100, 1, DT), Applied::Applied);
        assert_eq!(st.apply(&slow, 64, 120, 1, DT), Applied::Refreshed);
        assert_eq!(st.get(Status::Slow).unwrap().until, 184);
        assert!((st.speed_scale() - 0.7).abs() < 1e-6);
        let bleed = verb(Status::Bleed, StackRule::Independent, 5, 4.0);
        st.apply(&bleed, 64, 100, 2, DT);
        st.apply(&bleed, 64, 100, 3, DT);
        assert_eq!(st.active().count(), 3);
        let haste = verb(Status::Haste, StackRule::Extend, 1, 0.2);
        st.apply(&haste, 64, 100, 1, DT);
        st.apply(&haste, 64, 110, 1, DT);
        // 54 remaining + 64 = 118, under the 3x cap.
        assert_eq!(st.get(Status::Haste).unwrap().until, 228);
        assert_eq!(
            st.mask() & (1 << Status::Haste.index()),
            1 << Status::Haste.index()
        );
        st.expire(184);
        assert!(!st.has(Status::Slow));
        assert!(!st.has(Status::Bleed));
        assert!(st.has(Status::Haste));
    }

    #[test]
    fn cleanse_takes_the_banes_and_dispel_the_boons() {
        let mut st = Statuses::default();
        for s in [Status::Bleed, Status::Slow, Status::Haste, Status::Fury] {
            st.apply(&verb(s, StackRule::Refresh, 1, 0.5), 64, 1, 1, DT);
        }
        st.apply(
            &verb(Status::Cleanse, StackRule::Refresh, 1, 0.0),
            0,
            2,
            1,
            DT,
        );
        assert!(!st.has(Status::Bleed) && !st.has(Status::Slow));
        assert!(st.has(Status::Haste) && st.has(Status::Fury));
        st.apply(
            &verb(Status::Dispel, StackRule::Refresh, 1, 0.0),
            0,
            3,
            1,
            DT,
        );
        assert_eq!(st.active().count(), 0);
    }

    #[test]
    fn a_freeze_lasts_a_second_at_the_towns_rate() {
        let dt = 1.0 / 20.0;
        let mut st = Statuses::default();
        let chill = verb(Status::Chill, StackRule::Extend, 3, 0.0);
        for now in 10..13 {
            st.apply(&chill, 20, now, 1, dt);
        }
        assert_eq!(st.get(Status::Root).unwrap().until, 12 + 20);
        assert_eq!(st.chill_immune_until, 12 + 20 + 40);
    }

    #[test]
    fn chill_freezes_at_the_stack_limit_then_grants_immunity() {
        let mut st = Statuses::default();
        let chill = verb(Status::Chill, StackRule::Extend, 3, 0.0);
        assert_eq!(st.apply(&chill, 64, 10, 1, DT), Applied::Applied);
        assert!((st.speed_scale() - 0.85).abs() < 1e-6);
        assert_eq!(st.apply(&chill, 64, 11, 1, DT), Applied::Refreshed);
        assert_eq!(st.apply(&chill, 64, 12, 1, DT), Applied::Frozen);
        assert!(st.has(Status::Root) && !st.has(Status::Chill));
        assert_eq!(st.speed_scale(), 0.0);
        assert_eq!(st.apply(&chill, 64, 13, 1, DT), Applied::Immune);
        st.expire(12 + 64);
        assert!(!st.has(Status::Root));
        assert_eq!(st.apply(&chill, 64, 12 + 64 + 127, 1, DT), Applied::Immune);
        assert_eq!(st.apply(&chill, 64, 12 + 64 + 128, 1, DT), Applied::Applied);
    }

    #[test]
    fn slots_are_bounded() {
        let mut st = Statuses::default();
        let bleed = verb(Status::Bleed, StackRule::Independent, 9, 1.0);
        for _ in 0..MAX_STATUSES {
            assert_eq!(st.apply(&bleed, 64, 1, 1, DT), Applied::Applied);
        }
        assert_eq!(st.apply(&bleed, 64, 1, 1, DT), Applied::NoRoom);
    }
}
