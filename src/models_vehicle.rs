//! Rolling stock and road vehicle models. Front faces +Z, origin at rail top.

use crate::mesh::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::f32::consts::FRAC_PI_2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CarKind {
    SteamEngine,
    DieselEngine,
    Tender,
    Passenger,
    Sleeper,
    Dining,
    Boxcar,
    Tanker,
    Hopper,
    LogCar,
    Caboose,
}

impl CarKind {
    pub const ALL: [CarKind; 11] = [
        CarKind::SteamEngine,
        CarKind::DieselEngine,
        CarKind::Tender,
        CarKind::Passenger,
        CarKind::Sleeper,
        CarKind::Dining,
        CarKind::Boxcar,
        CarKind::Tanker,
        CarKind::Hopper,
        CarKind::LogCar,
        CarKind::Caboose,
    ];

    pub fn label(self) -> &'static str {
        match self {
            CarKind::SteamEngine => "Steam engine",
            CarKind::DieselEngine => "Diesel engine",
            CarKind::Tender => "Coal tender",
            CarKind::Passenger => "Passenger car",
            CarKind::Sleeper => "Sleeper car",
            CarKind::Dining => "Dining (food) car",
            CarKind::Boxcar => "Boxcar (freight)",
            CarKind::Tanker => "Tanker",
            CarKind::Hopper => "Coal hopper",
            CarKind::LogCar => "Log car",
            CarKind::Caboose => "Caboose",
        }
    }

    pub fn is_engine(self) -> bool {
        matches!(self, CarKind::SteamEngine | CarKind::DieselEngine)
    }

    pub fn has_seats(self) -> bool {
        matches!(self, CarKind::Passenger | CarKind::Sleeper | CarKind::Dining | CarKind::Caboose)
    }

    /// Length used for spacing cars along the track.
    pub fn length(self) -> f32 {
        match self {
            CarKind::SteamEngine => 0.9,
            CarKind::DieselEngine => 0.9,
            CarKind::Tender => 0.55,
            CarKind::Caboose => 0.65,
            _ => 0.85,
        }
    }

    /// Where the engineer's eyes are (local space).
    pub fn cab_eye(self) -> Vec3 {
        match self {
            CarKind::SteamEngine => Vec3::new(0.1, 0.5, -0.2),
            _ => Vec3::new(0.0, 0.38, 0.3),
        }
    }
}

const DARK: Color = Color::srgb(0.12, 0.12, 0.13);
const WHEEL: Color = Color::srgb(0.2, 0.2, 0.22);
const GLASS: Color = Color::srgb(0.75, 0.88, 0.95);

fn chassis(m: &mut Geo, half_len: f32, wheel_col: Color) {
    m.aabb(Vec3::new(-0.13, 0.055, -half_len), Vec3::new(0.13, 0.09, half_len), DARK);
    let axle = Quat::from_rotation_z(FRAC_PI_2);
    let bz = half_len - 0.14;
    for z in [-bz - 0.06, -bz + 0.06, bz - 0.06, bz + 0.06] {
        for x in [-0.095f32, 0.095] {
            m.cylinder(Vec3::new(x, 0.04, z), 0.04, 0.012, 8, axle, wheel_col);
        }
    }
    // Couplers.
    m.aabb(Vec3::new(-0.02, 0.06, half_len), Vec3::new(0.02, 0.08, half_len + 0.06), DARK);
    m.aabb(Vec3::new(-0.02, 0.06, -half_len - 0.06), Vec3::new(0.02, 0.08, -half_len), DARK);
}

/// Coach body with a band of windows on both sides.
fn coach(m: &mut Geo, body: Color, band: Color, windows: u32, big: bool) {
    let hl = 0.39;
    chassis(m, hl, WHEEL);
    m.aabb(Vec3::new(-0.15, 0.09, -hl), Vec3::new(0.15, 0.34, hl), body);
    m.aabb(Vec3::new(-0.152, 0.2, -hl), Vec3::new(0.152, 0.3, hl), band);
    // Rounded-ish roof.
    m.aabb(Vec3::new(-0.14, 0.34, -hl), Vec3::new(0.14, 0.37, hl), Color::srgb(0.35, 0.35, 0.37));
    m.aabb(Vec3::new(-0.09, 0.37, -hl + 0.02), Vec3::new(0.09, 0.385, hl - 0.02), Color::srgb(0.35, 0.35, 0.37));
    let wh = if big { 0.045 } else { 0.03 };
    for i in 0..windows {
        let z = -hl + 0.08 + (2.0 * hl - 0.16) * (i as f32 + 0.5) / windows as f32;
        let ww = (2.0 * hl - 0.16) / windows as f32 * 0.36;
        for x in [-0.154f32, 0.154] {
            m.cuboid(Vec3::new(x, 0.255, z), Vec3::new(0.003, wh, ww), Quat::IDENTITY, GLASS);
        }
    }
    // End doors.
    for z in [-hl - 0.002, hl + 0.002] {
        m.cuboid(Vec3::new(0.0, 0.2, z), Vec3::new(0.04, 0.09, 0.003), Quat::IDENTITY, Color::srgb(0.3, 0.25, 0.2));
    }
}

pub fn car_mesh(kind: CarKind, paint: Color) -> Geo {
    let mut m = Geo::new();
    match kind {
        CarKind::SteamEngine => {
            let hl = 0.42;
            chassis(&mut m, hl, Color::srgb(0.7, 0.1, 0.1));
            let along = Quat::from_rotation_x(FRAC_PI_2);
            // Boiler.
            m.cylinder(Vec3::new(0.0, 0.21, 0.13), 0.11, 0.25, 12, along, DARK);
            m.cylinder(Vec3::new(0.0, 0.21, 0.385), 0.115, 0.02, 12, along, Color::srgb(0.25, 0.25, 0.27));
            for z in [0.0f32, 0.15, 0.3] {
                m.cylinder(Vec3::new(0.0, 0.21, z), 0.113, 0.006, 12, along, Color::srgb(0.8, 0.65, 0.2));
            }
            // Chimney, dome, headlamp.
            m.frustum(Vec3::new(0.0, 0.37, 0.31), 0.035, 0.05, 0.06, 8, Quat::IDENTITY, DARK);
            m.sphere(Vec3::new(0.0, 0.32, 0.12), 0.05, Color::srgb(0.8, 0.65, 0.2));
            m.cuboid(Vec3::new(0.0, 0.34, 0.4), Vec3::new(0.03, 0.025, 0.02), Quat::IDENTITY, Color::srgb(1.0, 0.95, 0.6));
            // Cab.
            m.aabb(Vec3::new(-0.15, 0.09, -0.42), Vec3::new(0.15, 0.4, -0.12), paint);
            m.aabb(Vec3::new(-0.17, 0.4, -0.45), Vec3::new(0.17, 0.43, -0.1), DARK);
            for x in [-0.152f32, 0.152] {
                m.cuboid(Vec3::new(x, 0.32, -0.27), Vec3::new(0.003, 0.05, 0.07), Quat::IDENTITY, GLASS);
            }
            m.cuboid(Vec3::new(0.0, 0.32, -0.119), Vec3::new(0.12, 0.04, 0.003), Quat::IDENTITY, GLASS);
            // Running boards and cowcatcher.
            m.aabb(Vec3::new(-0.15, 0.09, -0.12), Vec3::new(0.15, 0.1, 0.42), paint);
            m.cuboid(Vec3::new(0.0, 0.06, 0.45), Vec3::new(0.13, 0.03, 0.04), Quat::from_rotation_x(-0.5), Color::srgb(0.7, 0.1, 0.1));
        }
        CarKind::DieselEngine => {
            let hl = 0.43;
            chassis(&mut m, hl, WHEEL);
            m.aabb(Vec3::new(-0.11, 0.09, -hl + 0.04), Vec3::new(0.11, 0.3, 0.15), paint);
            m.aabb(Vec3::new(-0.15, 0.09, 0.15), Vec3::new(0.15, 0.4, hl), paint);
            m.aabb(Vec3::new(-0.155, 0.11, -hl), Vec3::new(0.155, 0.15, hl), Color::srgb(0.95, 0.95, 0.9));
            m.cuboid(Vec3::new(0.0, 0.34, hl + 0.002), Vec3::new(0.12, 0.04, 0.003), Quat::IDENTITY, GLASS);
            for x in [-0.152f32, 0.152] {
                m.cuboid(Vec3::new(x, 0.34, 0.3), Vec3::new(0.003, 0.04, 0.08), Quat::IDENTITY, GLASS);
            }
            m.aabb(Vec3::new(-0.16, 0.4, 0.13), Vec3::new(0.16, 0.42, hl + 0.01), DARK);
            m.cuboid(Vec3::new(0.0, 0.25, hl + 0.004), Vec3::new(0.025, 0.02, 0.004), Quat::IDENTITY, Color::srgb(1.0, 0.95, 0.6));
            for z in [-0.25f32, -0.05] {
                m.aabb(Vec3::new(-0.06, 0.3, z - 0.04), Vec3::new(0.06, 0.32, z + 0.04), DARK);
            }
        }
        CarKind::Tender => {
            let hl = 0.26;
            chassis(&mut m, hl, WHEEL);
            m.aabb(Vec3::new(-0.15, 0.09, -hl), Vec3::new(0.15, 0.3, hl), paint);
            m.aabb(Vec3::new(-0.13, 0.3, -hl + 0.02), Vec3::new(0.13, 0.34, hl - 0.02), Color::srgb(0.08, 0.08, 0.08));
        }
        CarKind::Passenger => coach(&mut m, paint, Color::srgb(0.92, 0.88, 0.72), 7, false),
        CarKind::Sleeper => coach(&mut m, paint, Color::srgb(0.8, 0.68, 0.3), 5, false),
        CarKind::Dining => {
            coach(&mut m, paint, Color::srgb(0.95, 0.93, 0.85), 4, true);
            m.aabb(Vec3::new(-0.04, 0.385, 0.2), Vec3::new(0.04, 0.43, 0.26), DARK);
        }
        CarKind::Boxcar => {
            let hl = 0.4;
            chassis(&mut m, hl, WHEEL);
            m.aabb(Vec3::new(-0.15, 0.09, -hl), Vec3::new(0.15, 0.37, hl), paint);
            m.aabb(Vec3::new(-0.14, 0.37, -hl), Vec3::new(0.14, 0.39, hl), Color::srgb(0.3, 0.3, 0.3));
            for x in [-0.153f32, 0.153] {
                m.aabb(Vec3::new(x - 0.003, 0.11, -0.1), Vec3::new(x + 0.003, 0.34, 0.1), Color::srgb(0.4, 0.2, 0.12));
            }
        }
        CarKind::Tanker => {
            let hl = 0.4;
            chassis(&mut m, hl, WHEEL);
            m.cylinder(Vec3::new(0.0, 0.21, 0.0), 0.12, 0.37, 12, Quat::from_rotation_x(FRAC_PI_2), paint);
            m.cylinder(Vec3::new(0.0, 0.34, 0.0), 0.04, 0.03, 8, Quat::IDENTITY, DARK);
            m.aabb(Vec3::new(-0.14, 0.09, -0.02), Vec3::new(0.14, 0.12, 0.02), DARK);
        }
        CarKind::Hopper => {
            let hl = 0.38;
            chassis(&mut m, hl, WHEEL);
            m.aabb(Vec3::new(-0.15, 0.13, -hl), Vec3::new(0.15, 0.32, hl), paint);
            m.cuboid(Vec3::new(0.0, 0.11, 0.0), Vec3::new(0.12, 0.03, hl - 0.1), Quat::IDENTITY, paint);
            m.aabb(Vec3::new(-0.13, 0.32, -hl + 0.02), Vec3::new(0.13, 0.345, hl - 0.02), Color::srgb(0.08, 0.08, 0.08));
        }
        CarKind::LogCar => {
            let hl = 0.4;
            chassis(&mut m, hl, WHEEL);
            m.aabb(Vec3::new(-0.15, 0.09, -hl), Vec3::new(0.15, 0.11, hl), Color::srgb(0.45, 0.3, 0.2));
            let along = Quat::from_rotation_x(FRAC_PI_2);
            for (x, y) in [(-0.08f32, 0.155f32), (0.0, 0.155), (0.08, 0.155), (-0.04, 0.235), (0.04, 0.235)] {
                m.cylinder(Vec3::new(x, y, 0.0), 0.042, 0.36, 7, along, Color::srgb(0.55, 0.38, 0.22));
            }
            for z in [-0.38f32, 0.38] {
                for x in [-0.14f32, 0.14] {
                    m.aabb(Vec3::new(x - 0.01, 0.11, z - 0.01), Vec3::new(x + 0.01, 0.3, z + 0.01), DARK);
                }
            }
        }
        CarKind::Caboose => {
            let hl = 0.3;
            chassis(&mut m, hl, WHEEL);
            m.aabb(Vec3::new(-0.15, 0.09, -0.24), Vec3::new(0.15, 0.34, 0.24), paint);
            m.aabb(Vec3::new(-0.16, 0.34, -0.27), Vec3::new(0.16, 0.36, 0.27), DARK);
            m.aabb(Vec3::new(-0.09, 0.36, -0.08), Vec3::new(0.09, 0.45, 0.08), paint);
            m.aabb(Vec3::new(-0.1, 0.45, -0.1), Vec3::new(0.1, 0.47, 0.1), DARK);
            for x in [-0.153f32, 0.153] {
                for z in [-0.12f32, 0.12] {
                    m.cuboid(Vec3::new(x, 0.25, z), Vec3::new(0.003, 0.035, 0.04), Quat::IDENTITY, GLASS);
                }
            }
            for x in [-0.092f32, 0.092] {
                m.cuboid(Vec3::new(x, 0.405, 0.0), Vec3::new(0.003, 0.025, 0.05), Quat::IDENTITY, GLASS);
            }
        }
    }
    m
}

pub fn default_paint(kind: CarKind) -> Color {
    match kind {
        CarKind::SteamEngine => rgb(0.1, 0.25, 0.15),
        CarKind::DieselEngine => rgb(0.95, 0.75, 0.1),
        CarKind::Tender => rgb(0.1, 0.25, 0.15),
        CarKind::Passenger => rgb(0.15, 0.45, 0.25),
        CarKind::Sleeper => rgb(0.12, 0.18, 0.4),
        CarKind::Dining => rgb(0.5, 0.12, 0.15),
        CarKind::Boxcar => rgb(0.55, 0.28, 0.16),
        CarKind::Tanker => rgb(0.2, 0.2, 0.22),
        CarKind::Hopper => rgb(0.35, 0.3, 0.28),
        CarKind::LogCar => rgb(0.4, 0.3, 0.2),
        CarKind::Caboose => rgb(0.75, 0.12, 0.1),
    }
}

/// Road vehicles: 0 = car, 1 = van, 2 = bus, 3 = truck. Front faces +Z, origin at road level.
pub fn road_vehicle_mesh(style: u8, paint: Color) -> Geo {
    let mut m = Geo::new();
    let tire = Quat::from_rotation_z(FRAC_PI_2);
    let (hl, hw, h) = match style {
        0 => (0.11, 0.055, 0.06),
        1 => (0.12, 0.06, 0.1),
        2 => (0.2, 0.065, 0.12),
        _ => (0.18, 0.065, 0.11),
    };
    for z in [-hl * 0.65, hl * 0.65] {
        for x in [-hw, hw] {
            m.cylinder(Vec3::new(x, 0.022, z), 0.022, 0.008, 8, tire, DARK);
        }
    }
    match style {
        0 => {
            m.aabb(Vec3::new(-hw, 0.02, -hl), Vec3::new(hw, 0.05, hl), paint);
            m.aabb(Vec3::new(-hw + 0.008, 0.05, -hl * 0.55), Vec3::new(hw - 0.008, 0.08, hl * 0.35), GLASS);
            m.aabb(Vec3::new(-hw + 0.006, 0.08, -hl * 0.5), Vec3::new(hw - 0.006, 0.088, hl * 0.3), paint);
        }
        1 => {
            m.aabb(Vec3::new(-hw, 0.02, -hl), Vec3::new(hw, 0.12, hl), paint);
            m.aabb(Vec3::new(-hw - 0.002, 0.07, hl * 0.5), Vec3::new(hw + 0.002, 0.11, hl + 0.002), GLASS);
        }
        2 => {
            m.aabb(Vec3::new(-hw, 0.02, -hl), Vec3::new(hw, h + 0.02, hl), paint);
            m.aabb(Vec3::new(-hw - 0.002, 0.08, -hl + 0.02), Vec3::new(hw + 0.002, 0.12, hl + 0.002), GLASS);
        }
        _ => {
            m.aabb(Vec3::new(-hw, 0.02, hl * 0.5), Vec3::new(hw, 0.11, hl), paint);
            m.aabb(Vec3::new(-hw - 0.002, 0.07, hl * 0.75), Vec3::new(hw + 0.002, 0.1, hl + 0.002), GLASS);
            m.aabb(Vec3::new(-hw, 0.03, -hl), Vec3::new(hw, 0.13, hl * 0.45), Color::srgb(0.85, 0.85, 0.85));
        }
    }
    m.aabb(Vec3::new(-hw * 0.7, 0.03, hl), Vec3::new(-hw * 0.4, 0.045, hl + 0.003), Color::srgb(1.0, 1.0, 0.8));
    m.aabb(Vec3::new(hw * 0.4, 0.03, hl), Vec3::new(hw * 0.7, 0.045, hl + 0.003), Color::srgb(1.0, 1.0, 0.8));
    m.aabb(Vec3::new(-hw * 0.7, 0.03, -hl - 0.003), Vec3::new(-hw * 0.4, 0.045, -hl), Color::srgb(0.9, 0.1, 0.1));
    m.aabb(Vec3::new(hw * 0.4, 0.03, -hl - 0.003), Vec3::new(hw * 0.7, 0.045, -hl), Color::srgb(0.9, 0.1, 0.1));
    m
}
