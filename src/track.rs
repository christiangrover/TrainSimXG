//! Track geometry: the path a wheel follows through a tile, and moving a
//! position along connected track pieces (through switches, ramps, etc).

use crate::grid::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::f32::consts::FRAC_PI_2;

/// Height of the rail head above the track level.
pub const RAIL_Y: f32 = 0.06;

/// Point and tangent along the path from edge `a` to edge `b` in a tile, in
/// tile-local coordinates (origin at tile centre, y = 0 at level 0).
pub fn path_local(a: u8, b: u8, t: f32, ya: f32, yb: f32) -> (Vec3, Vec3) {
    let y = ya + (yb - ya) * t;
    let pa = edge_mid(a);
    let pb = edge_mid(b);
    if a == opposite(b) {
        let p = pa.lerp(pb, t);
        let d = (pb - pa).extend(yb - ya);
        (Vec3::new(p.x, y, p.y), Vec3::new(d.x, d.z, d.y).normalize())
    } else {
        // Quarter circle around the shared corner.
        let c = pa + pb;
        let va = pa - c;
        let vb = pb - c;
        let th = t * FRAC_PI_2;
        let p = c + va * th.cos() + vb * th.sin();
        let d = -va * th.sin() + vb * th.cos();
        let len = FRAC_PI_2 * 0.5;
        (Vec3::new(p.x, y, p.y), Vec3::new(d.x, (yb - ya) / len * 0.5, d.y).normalize())
    }
}

pub fn path_len(a: u8, b: u8) -> f32 {
    if a == opposite(b) {
        1.0
    } else {
        FRAC_PI_2 * 0.5
    }
}

/// A position on the track network, oriented in the direction of travel.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrackPos {
    pub tile: IVec2,
    pub entry: u8,
    pub exit: u8,
    pub t: f32,
}

impl TrackPos {
    pub fn reversed(self) -> Self {
        TrackPos { tile: self.tile, entry: self.exit, exit: self.entry, t: 1.0 - self.t }
    }

    /// Is this position still valid on the current grid?
    pub fn valid(&self, grid: &Grid) -> bool {
        match grid.track(self.tile) {
            Some(tr) => {
                let es = tr.edges();
                es.contains(&self.entry) && es.contains(&self.exit) && self.entry != self.exit
            }
            None => false,
        }
    }

    /// World-space position (on top of the rails) and travel direction.
    pub fn world(&self, grid: &Grid) -> (Vec3, Vec3) {
        let (ya, yb) = match grid.track(self.tile) {
            Some(tr) => (tr.edge_level(self.entry) as f32 * LEVEL_H, tr.edge_level(self.exit) as f32 * LEVEL_H),
            None => (0.0, 0.0),
        };
        let (p, d) = path_local(self.entry, self.exit, self.t, ya, yb);
        (tile_center(self.tile) + p + Vec3::Y * RAIL_Y, d)
    }

    /// Moves forward by `dist`. Returns the distance actually travelled
    /// (less than `dist` when the end of the line is reached).
    pub fn advance(&mut self, grid: &Grid, dist: f32) -> f32 {
        let mut left = dist;
        let mut moved = 0.0;
        // Each hop covers at least a quarter-circle, so this bound is never hit on real track.
        let max_hops = (dist / (FRAC_PI_2 * 0.5)) as usize + 2;
        for _ in 0..max_hops {
            let len = path_len(self.entry, self.exit);
            let remaining = (1.0 - self.t) * len;
            if left <= remaining {
                self.t += left / len;
                return moved + left;
            }
            // Try to roll onto the next tile.
            match self.next(grid) {
                Some(n) => {
                    moved += remaining;
                    left -= remaining;
                    *self = n;
                }
                None => {
                    moved += remaining;
                    self.t = 1.0;
                    return moved;
                }
            }
        }
        moved
    }

    /// The position at the start of the next tile, if the track continues.
    pub fn next(&self, grid: &Grid) -> Option<TrackPos> {
        let here = grid.track(self.tile)?;
        let level = here.edge_level(self.exit);
        let nt = self.tile + edge_dir(self.exit);
        let entry = opposite(self.exit);
        let there = grid.track(nt)?;
        if !there.edges().contains(&entry) || there.edge_level(entry) != level {
            return None;
        }
        let exit = there.exit_for(entry)?;
        Some(TrackPos { tile: nt, entry, exit, t: 0.0 })
    }

    /// The position `dist` ahead (or the furthest reachable).
    pub fn ahead(mut self, grid: &Grid, dist: f32) -> Self {
        self.advance(grid, dist);
        self
    }

    pub fn behind(self, grid: &Grid, dist: f32) -> Self {
        self.reversed().ahead(grid, dist).reversed()
    }
}

/// A starting position on a tile's track (first path, heading toward its second edge).
pub fn start_pos(grid: &Grid, tile: IVec2) -> Option<TrackPos> {
    let tr = grid.track(tile)?;
    let (a, b) = tr.paths()[0];
    Some(TrackPos { tile, entry: a, exit: b, t: 0.5 })
}

/// Straight track that can be turned into a ramp (and back).
fn reshapeable(t: &Track) -> bool {
    matches!(t.kind, TrackKind::Straight | TrackKind::Ramp)
}

/// A straight or ramp running from edge `near` (at level `a`) to edge `far` (at level `g`).
fn straight_between(near: u8, far: u8, a: i32, g: i32) -> Track {
    if a == g {
        Track::new(TrackKind::Straight, near % 2, a)
    } else {
        // A ramp's high end is its rotated North edge.
        let high = if a > g { near } else { far };
        Track::new(TrackKind::Ramp, high, a.min(g))
    }
}

/// Walks along straight track from `start` through edge `dir` and returns the
/// level of the first piece that can't be sloped (or the far end of the line).
fn anchor_level(grid: &Grid, start: IVec2, dir: u8, origin: IVec2) -> i32 {
    let mut cur = start;
    for _ in 0..64 {
        let next = cur + edge_dir(dir);
        match grid.track(next).filter(|t| t.edges().contains(&opposite(dir))) {
            Some(t) if reshapeable(t) && next != origin => cur = next,
            Some(t) => return t.edge_level(opposite(dir)),
            None => return grid.track(cur).map(|t| if reshapeable(t) { t.level } else { t.edge_level(dir) }).unwrap_or(0),
        }
    }
    grid.track(cur).map(|t| t.edge_level(dir)).unwrap_or(0)
}

/// After a piece's height changes, reshapes the straight track on either side
/// into ramps so the line climbs or falls one level per tile, as far out as
/// needed. Returns tiles where a step is left (curves, switches, stations...).
pub fn smooth_levels(grid: &mut Grid, origin: IVec2) -> Vec<IVec2> {
    let mut queue = vec![origin];
    let mut done = std::collections::HashSet::from([origin]);
    let mut stuck = vec![];
    while let Some(t) = queue.pop() {
        let Some(tr) = grid.track(t).copied() else { continue };
        for e in tr.edges() {
            let n = t + edge_dir(e);
            let back = opposite(e);
            let Some(nt) = grid.track(n).copied() else { continue };
            if !nt.edges().contains(&back) {
                continue;
            }
            let a = tr.edge_level(e);
            if nt.edge_level(back) == a {
                continue;
            }
            if done.contains(&n) || !reshapeable(&nt) {
                stuck.push(n);
                continue;
            }
            // Head for the height of the next fixed point along the line: a curve,
            // switch, station or the end of the track.
            let far = opposite(back);
            let f = anchor_level(grid, n, far, origin);
            // Step at most one level per tile toward it.
            let g = (a - (a - f).signum()).clamp(0, MAX_LEVEL - 1);
            let g = if f == a { a } else { g };
            if let Some(x) = grid.edit(n) {
                x.content = Content::Track(straight_between(back, far, a, g));
            }
            done.insert(n);
            queue.push(n);
        }
    }
    stuck
}