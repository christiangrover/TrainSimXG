//! The 100 x 100 table grid and everything that can be placed on a tile.

use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const GRID: i32 = 100;
/// World height of one terrain / track level.
pub const LEVEL_H: f32 = 0.25;
pub const MAX_LEVEL: i32 = 16;

/// Edges of a tile: 0 = North (-Z), 1 = East (+X), 2 = South (+Z), 3 = West (-X).
pub const N: u8 = 0;
pub const E: u8 = 1;
pub const S: u8 = 2;
pub const W: u8 = 3;

pub fn edge_dir(e: u8) -> IVec2 {
    match e % 4 {
        0 => IVec2::new(0, -1),
        1 => IVec2::new(1, 0),
        2 => IVec2::new(0, 1),
        _ => IVec2::new(-1, 0),
    }
}

pub fn opposite(e: u8) -> u8 {
    (e + 2) % 4
}

/// Tile-local offset (x, z) of an edge's midpoint.
pub fn edge_mid(e: u8) -> Vec2 {
    edge_dir(e).as_vec2() * 0.5
}

/// World position of the centre of a tile at table height 0.
pub fn tile_center(t: IVec2) -> Vec3 {
    Vec3::new(t.x as f32 - GRID as f32 / 2.0 + 0.5, 0.0, t.y as f32 - GRID as f32 / 2.0 + 0.5)
}

pub fn world_to_tile(p: Vec3) -> IVec2 {
    IVec2::new((p.x + GRID as f32 / 2.0).floor() as i32, (p.z + GRID as f32 / 2.0).floor() as i32)
}

pub fn in_bounds(t: IVec2) -> bool {
    t.x >= 0 && t.y >= 0 && t.x < GRID && t.y < GRID
}

/// Rotation quaternion for `rot` quarter turns (matches edge `e -> e + rot`).
pub fn rot_quat(rot: u8) -> Quat {
    Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2 * (rot % 4) as f32)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum Ground {
    #[default]
    Grass,
    Sand,
    Water,
    Rock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TrackKind {
    Straight,
    Curve,
    SwitchLeft,
    SwitchRight,
    Crossing,
    Ramp,
    Station,
    LevelCrossing,
}

impl TrackKind {
    pub const ALL: [TrackKind; 8] = [
        TrackKind::Straight,
        TrackKind::Curve,
        TrackKind::SwitchLeft,
        TrackKind::SwitchRight,
        TrackKind::Crossing,
        TrackKind::Ramp,
        TrackKind::Station,
        TrackKind::LevelCrossing,
    ];

    pub fn label(self) -> &'static str {
        match self {
            TrackKind::Straight => "Straight",
            TrackKind::Curve => "Curve",
            TrackKind::SwitchLeft => "Switch (left)",
            TrackKind::SwitchRight => "Switch (right)",
            TrackKind::Crossing => "Diamond crossing",
            TrackKind::Ramp => "Ramp (up/down)",
            TrackKind::Station => "Station",
            TrackKind::LevelCrossing => "Road crossing",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub kind: TrackKind,
    pub rot: u8,
    /// Height level of the track at its low end.
    pub level: i32,
    /// For switches: true = diverging route is set.
    #[serde(default)]
    pub diverge: bool,
}

impl Track {
    pub fn new(kind: TrackKind, rot: u8, level: i32) -> Self {
        Track { kind, rot: rot % 4, level, diverge: false }
    }

    fn r(&self, e: u8) -> u8 {
        (e + self.rot) % 4
    }

    /// Edges that track connects to (already rotated).
    pub fn edges(&self) -> Vec<u8> {
        use TrackKind::*;
        match self.kind {
            Straight | Ramp | Station | LevelCrossing => vec![self.r(N), self.r(S)],
            Curve => vec![self.r(S), self.r(E)],
            SwitchLeft => vec![self.r(S), self.r(N), self.r(W)],
            SwitchRight => vec![self.r(S), self.r(N), self.r(E)],
            Crossing => vec![N, E, S, W],
        }
    }

    /// Pairs of edges joined by a rail path (for drawing).
    pub fn paths(&self) -> Vec<(u8, u8)> {
        use TrackKind::*;
        match self.kind {
            Straight | Ramp | Station | LevelCrossing => vec![(self.r(S), self.r(N))],
            Curve => vec![(self.r(S), self.r(E))],
            SwitchLeft => vec![(self.r(S), self.r(N)), (self.r(S), self.r(W))],
            SwitchRight => vec![(self.r(S), self.r(N)), (self.r(S), self.r(E))],
            Crossing => vec![(S, N), (W, E)],
        }
    }

    pub fn is_switch(&self) -> bool {
        matches!(self.kind, TrackKind::SwitchLeft | TrackKind::SwitchRight)
    }

    /// Height level of the track where it meets edge `e`.
    pub fn edge_level(&self, e: u8) -> i32 {
        if self.kind == TrackKind::Ramp && e == self.r(N) {
            self.level + 1
        } else {
            self.level
        }
    }

    pub fn max_level(&self) -> i32 {
        if self.kind == TrackKind::Ramp {
            self.level + 1
        } else {
            self.level
        }
    }

    /// Where a train entering through `entry` leaves, given the switch setting.
    pub fn exit_for(&self, entry: u8) -> Option<u8> {
        use TrackKind::*;
        match self.kind {
            Crossing => Some(opposite(entry)),
            SwitchLeft | SwitchRight => {
                let stem = self.r(S);
                let straight = self.r(N);
                let branch = if self.kind == SwitchLeft { self.r(W) } else { self.r(E) };
                if entry == stem {
                    Some(if self.diverge { branch } else { straight })
                } else if entry == straight || entry == branch {
                    Some(stem)
                } else {
                    None
                }
            }
            _ => {
                let es = self.edges();
                if entry == es[0] {
                    Some(es[1])
                } else if entry == es[1] {
                    Some(es[0])
                } else {
                    None
                }
            }
        }
    }

    /// Edges along which a road passes through this tile (level crossings only).
    pub fn road_edges(&self) -> Vec<u8> {
        if self.kind == TrackKind::LevelCrossing {
            vec![self.r(E), self.r(W)]
        } else {
            vec![]
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DecorKind {
    House,
    Cottage,
    Shop,
    Apartment,
    Church,
    Barn,
    Silo,
    Windmill,
    PineTree,
    OakTree,
    Bush,
    Flowers,
    Rock,
    Fence,
    Lamp,
    Bench,
    Fountain,
    Person,
}

impl DecorKind {
    pub const ALL: [DecorKind; 18] = [
        DecorKind::House,
        DecorKind::Cottage,
        DecorKind::Shop,
        DecorKind::Apartment,
        DecorKind::Church,
        DecorKind::Barn,
        DecorKind::Silo,
        DecorKind::Windmill,
        DecorKind::PineTree,
        DecorKind::OakTree,
        DecorKind::Bush,
        DecorKind::Flowers,
        DecorKind::Rock,
        DecorKind::Fence,
        DecorKind::Lamp,
        DecorKind::Bench,
        DecorKind::Fountain,
        DecorKind::Person,
    ];

    pub fn label(self) -> &'static str {
        match self {
            DecorKind::House => "House",
            DecorKind::Cottage => "Cottage",
            DecorKind::Shop => "Shop",
            DecorKind::Apartment => "Apartments",
            DecorKind::Church => "Church",
            DecorKind::Barn => "Barn",
            DecorKind::Silo => "Silo",
            DecorKind::Windmill => "Windmill",
            DecorKind::PineTree => "Pine tree",
            DecorKind::OakTree => "Oak tree",
            DecorKind::Bush => "Bush",
            DecorKind::Flowers => "Flowers",
            DecorKind::Rock => "Boulder",
            DecorKind::Fence => "Fence",
            DecorKind::Lamp => "Street lamp",
            DecorKind::Bench => "Bench",
            DecorKind::Fountain => "Fountain",
            DecorKind::Person => "Person",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum Content {
    #[default]
    Empty,
    Track(Track),
    Road,
    Decor { kind: DecorKind, rot: u8, variant: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Tile {
    pub height: i32,
    pub ground: Ground,
    pub content: Content,
}

impl Tile {
    pub fn track(&self) -> Option<&Track> {
        match &self.content {
            Content::Track(t) => Some(t),
            _ => None,
        }
    }
    pub fn track_mut(&mut self) -> Option<&mut Track> {
        match &mut self.content {
            Content::Track(t) => Some(t),
            _ => None,
        }
    }
    pub fn surface_y(&self) -> f32 {
        if self.ground == Ground::Water {
            self.height as f32 * LEVEL_H - 0.06
        } else {
            self.height as f32 * LEVEL_H
        }
    }
}

/// The table: all tiles plus dirty flags so rendering only rebuilds what changed.
#[derive(Resource)]
pub struct Grid {
    pub tiles: Vec<Tile>,
    pub dirty_tiles: Vec<IVec2>,
    pub terrain_dirty: Vec<IVec2>,
    /// Bumped when the whole grid is replaced (load / new).
    pub generation: u32,
}

impl Default for Grid {
    fn default() -> Self {
        Grid {
            tiles: vec![Tile::default(); (GRID * GRID) as usize],
            dirty_tiles: vec![],
            terrain_dirty: vec![],
            generation: 0,
        }
    }
}

impl Grid {
    fn idx(t: IVec2) -> usize {
        (t.y * GRID + t.x) as usize
    }

    pub fn get(&self, t: IVec2) -> Option<&Tile> {
        in_bounds(t).then(|| &self.tiles[Self::idx(t)])
    }

    pub fn track(&self, t: IVec2) -> Option<&Track> {
        self.get(t).and_then(|x| x.track())
    }

    pub fn height(&self, t: IVec2) -> i32 {
        self.get(t).map(|x| x.height).unwrap_or(0)
    }

    /// Mutable access that marks the tile and its neighbours for redraw
    /// (roads, portals and bridges depend on neighbours).
    pub fn edit(&mut self, t: IVec2) -> Option<&mut Tile> {
        if !in_bounds(t) {
            return None;
        }
        self.mark(t);
        Some(&mut self.tiles[Self::idx(t)])
    }

    pub fn mark(&mut self, t: IVec2) {
        for d in [IVec2::ZERO, IVec2::X, IVec2::NEG_X, IVec2::Y, IVec2::NEG_Y] {
            let n = t + d;
            if in_bounds(n) {
                self.dirty_tiles.push(n);
                self.terrain_dirty.push(n);
            }
        }
    }

    pub fn replace_all(&mut self, tiles: Vec<Tile>) {
        self.tiles = tiles;
        self.generation += 1;
        self.dirty_tiles.clear();
        self.terrain_dirty.clear();
    }

    /// Does the tile carry a road a car can drive on, and along which edges?
    pub fn road_edges(&self, t: IVec2) -> Vec<u8> {
        match self.get(t).map(|x| x.content) {
            Some(Content::Road) => (0..4u8)
                .filter(|&e| {
                    let n = t + edge_dir(e);
                    self.road_edges_raw(n).contains(&opposite(e)) && self.height(n) == self.height(t)
                })
                .collect(),
            Some(Content::Track(tr)) => tr
                .road_edges()
                .into_iter()
                .filter(|&e| {
                    let n = t + edge_dir(e);
                    self.road_edges_raw(n).contains(&opposite(e))
                })
                .collect(),
            _ => vec![],
        }
    }

    /// Edges a road on this tile *could* connect along (roads connect anywhere).
    fn road_edges_raw(&self, t: IVec2) -> Vec<u8> {
        match self.get(t).map(|x| x.content) {
            Some(Content::Road) => vec![N, E, S, W],
            Some(Content::Track(tr)) => tr.road_edges(),
            _ => vec![],
        }
    }

    pub fn is_tunnel(&self, t: IVec2) -> bool {
        match (self.get(t), self.track(t)) {
            (Some(tile), Some(tr)) => tile.height > tr.max_level(),
            _ => false,
        }
    }
}
