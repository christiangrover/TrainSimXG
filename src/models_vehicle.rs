//! Rolling stock and road vehicle models. Front faces +Z, origin at rail top
//! (or road surface).

use crate::mesh::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::f32::consts::FRAC_PI_2;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CarKind {
    SteamEngine,
    DieselEngine,
    DieselRoad,
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
    pub const ALL: [CarKind; 12] = [
        CarKind::SteamEngine,
        CarKind::DieselRoad,
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
            CarKind::DieselEngine => "Diesel streamliner",
            CarKind::DieselRoad => "Diesel road engine",
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
        matches!(self, CarKind::SteamEngine | CarKind::DieselEngine | CarKind::DieselRoad)
    }

    pub fn has_seats(self) -> bool {
        matches!(self, CarKind::Passenger | CarKind::Sleeper | CarKind::Dining | CarKind::Caboose)
    }

    /// Length used for spacing cars along the track.
    pub fn length(self) -> f32 {
        match self {
            CarKind::SteamEngine => 0.92,
            CarKind::DieselEngine => 0.92,
            CarKind::DieselRoad => 0.95,
            CarKind::Tender => 0.55,
            CarKind::Caboose => 0.65,
            _ => 0.85,
        }
    }

    /// Where the engineer's eyes are (local space).
    pub fn cab_eye(self) -> Vec3 {
        match self {
            CarKind::SteamEngine => Vec3::new(0.1, 0.37, -0.22),
            CarKind::DieselRoad => Vec3::new(0.07, 0.36, 0.24),
            _ => Vec3::new(0.0, 0.38, 0.3),
        }
    }

    /// Where smoke or exhaust comes out, if anywhere.
    pub fn exhaust(self) -> Option<Vec3> {
        match self {
            CarKind::SteamEngine => Some(Vec3::new(0.0, 0.47, 0.31)),
            CarKind::DieselEngine => Some(Vec3::new(0.0, 0.43, -0.05)),
            CarKind::DieselRoad => Some(Vec3::new(0.0, 0.38, -0.02)),
            _ => None,
        }
    }
}

const DARK: Color = Color::srgb(0.1, 0.1, 0.11);
const IRON: Color = Color::srgb(0.25, 0.24, 0.24);
const WHEEL: Color = Color::srgb(0.32, 0.3, 0.3);
const GLASS: Color = Color::srgb(0.45, 0.6, 0.7);
const WHITE: Color = Color::srgb(0.95, 0.95, 0.93);
const BRASS: Color = Color::srgb(0.85, 0.68, 0.3);
const YELLOW: Color = Color::srgb(0.95, 0.75, 0.1);
const ROOF: Color = Color::srgb(0.4, 0.4, 0.42);

fn along() -> Quat {
    Quat::from_rotation_x(FRAC_PI_2)
}
fn axle() -> Quat {
    Quat::from_rotation_z(FRAC_PI_2)
}

/// A bogie (truck): side frames, springs and wheelsets.
fn truck(m: &mut Geo, z: f32, axles: u32, r: f32) {
    let spacing = r * 2.3;
    let span = spacing * (axles - 1) as f32;
    m.mat(Mat::Steel);
    for i in 0..axles {
        let az = z - span * 0.5 + i as f32 * spacing;
        for x in [-0.09f32, 0.09] {
            m.cylinder(Vec3::new(x, r, az), r, 0.01, 12, axle(), WHEEL);
        }
        m.cylinder(Vec3::new(0.0, r, az), 0.008, 0.1, 6, axle(), DARK);
    }
    for x in [-0.115f32, 0.115] {
        m.aabb(Vec3::new(x - 0.01, r * 0.5, z - span * 0.5 - r * 0.9), Vec3::new(x + 0.01, r * 1.5, z + span * 0.5 + r * 0.9), IRON);
        m.cylinder(Vec3::new(x, r * 1.5 + 0.008, z), 0.012, 0.01, 6, Quat::IDENTITY, DARK);
    }
    m.aabb(Vec3::new(-0.11, r * 1.3, z - 0.02), Vec3::new(0.11, r * 1.6, z + 0.02), IRON);
}

/// Underframe, couplers and end beams.
fn underframe(m: &mut Geo, hl: f32, y: f32) {
    m.mat(Mat::Steel);
    m.aabb(Vec3::new(-0.14, y, -hl), Vec3::new(0.14, y + 0.03, hl), DARK);
    for s in [-1.0f32, 1.0] {
        let z = s * hl;
        m.aabb(Vec3::new(-0.025, y - 0.005, z.min(z + s * 0.06)), Vec3::new(0.025, y + 0.02, z.max(z + s * 0.06)), DARK);
        m.aabb(Vec3::new(-0.04, y - 0.012, z + s * 0.055 - 0.008), Vec3::new(0.04, y + 0.025, z + s * 0.055 + 0.008), DARK);
    }
    // Brake gear and air tanks under the floor.
    m.cylinder(Vec3::new(0.05, y - 0.02, 0.0), 0.022, 0.12, 8, along(), IRON);
    m.aabb(Vec3::new(-0.09, y - 0.04, -0.08), Vec3::new(-0.02, y, 0.08), IRON);
}

/// Framed window on a car side facing ±X. One-sided, so passengers can see out.
fn side_window(m: &mut Geo, x: f32, y: f32, z: f32, hw: f32, hh: f32, frame: Color) {
    let n = Vec3::X * x.signum();
    m.mat(Mat::Paint);
    m.panel(Vec3::new(x, y, z) + n * 0.002, n, Vec3::Z, hw + 0.006, hh + 0.006, frame);
    m.mat(Mat::Glass);
    m.panel(Vec3::new(x, y, z) + n * 0.003, n, Vec3::Z, hw, hh, GLASS);
}

/// A cab window facing `n` (one-sided: clear when you look out through it).
fn cab_window(m: &mut Geo, c: Vec3, n: Vec3, hw: f32, hh: f32) {
    m.mat(Mat::Glass);
    m.panel(c, n, Vec3::X, hw, hh, GLASS);
}

/// Handrail between two points (thin steel bar).
fn rail(m: &mut Geo, a: Vec3, b: Vec3, color: Color) {
    let d = b - a;
    m.mat(Mat::Steel);
    m.cuboid((a + b) * 0.5, Vec3::new(0.003, 0.003, d.length() * 0.5), basis_from_dir(d), color);
}

/// Seats seen from the passenger view.
fn seats(m: &mut Geo, hl: f32, color: Color) {
    m.mat(Mat::Plain);
    m.aabb(Vec3::new(-0.14, 0.11, -hl + 0.02), Vec3::new(0.14, 0.115, hl - 0.02), Color::srgb(0.35, 0.28, 0.22));
    let mut z = -hl + 0.08;
    while z < hl - 0.06 {
        for x in [-0.09f32, 0.09] {
            m.aabb(Vec3::new(x - 0.045, 0.115, z - 0.025), Vec3::new(x + 0.045, 0.16, z + 0.025), color);
            m.aabb(Vec3::new(x - 0.045, 0.16, z - 0.025), Vec3::new(x + 0.045, 0.22, z - 0.012), color);
        }
        z += 0.09;
    }
}

/// Coach body with a band of framed windows, a clerestory roof and end doors.
fn coach(m: &mut Geo, body: Color, band: Color, windows: u32, wh: f32, extra: impl Fn(&mut Geo)) {
    let hl = 0.4;
    underframe(m, hl, 0.09);
    truck(m, -0.28, 2, 0.04);
    truck(m, 0.28, 2, 0.04);
    m.mat(Mat::Paint);
    m.aabb(Vec3::new(-0.15, 0.115, -hl), Vec3::new(0.15, 0.34, hl), body);
    m.aabb(Vec3::new(-0.152, 0.2, -hl + 0.06), Vec3::new(0.152, 0.3, hl - 0.06), band);
    m.aabb(Vec3::new(-0.153, 0.115, -hl), Vec3::new(0.153, 0.125, hl), DARK);
    // Roof: curved-ish with a clerestory and vents.
    m.mat(Mat::Steel);
    m.aabb(Vec3::new(-0.155, 0.34, -hl), Vec3::new(0.155, 0.355, hl), ROOF);
    m.aabb(Vec3::new(-0.12, 0.355, -hl + 0.01), Vec3::new(0.12, 0.37, hl - 0.01), ROOF);
    m.aabb(Vec3::new(-0.07, 0.37, -hl + 0.04), Vec3::new(0.07, 0.39, hl - 0.04), ROOF);
    for i in 0..4 {
        let z = -0.27 + i as f32 * 0.18;
        m.cylinder(Vec3::new(0.0, 0.395, z), 0.015, 0.008, 8, Quat::IDENTITY, DARK);
    }
    // Windows.
    let span = 2.0 * hl - 0.2;
    for i in 0..windows {
        let z = -hl + 0.1 + span * (i as f32 + 0.5) / windows as f32;
        let hw = span / windows as f32 * 0.36;
        for x in [-0.153f32, 0.153] {
            side_window(m, x, 0.25, z, hw, wh, WHITE);
        }
    }
    // End doors with windows, diaphragms, steps and lamps.
    for s in [-1.0f32, 1.0] {
        let z = s * hl;
        for x in [-0.153f32, 0.153] {
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(x - 0.002, 0.13, z - s * 0.06 - 0.025), Vec3::new(x + 0.002, 0.32, z - s * 0.06 + 0.025), body.mix(&DARK, 0.25));
            side_window(m, x, 0.27, z - s * 0.06, 0.015, 0.025, WHITE);
            m.mat(Mat::Steel);
            m.aabb(Vec3::new(x - 0.01, 0.08, z - s * 0.06 - 0.025), Vec3::new(x + 0.01, 0.09, z - s * 0.06 + 0.025), DARK);
        }
        m.mat(Mat::Plain);
        m.aabb(Vec3::new(-0.09, 0.13, z.min(z + s * 0.035)), Vec3::new(0.09, 0.33, z.max(z + s * 0.035)), DARK);
    }
    seats(m, hl, Color::srgb(0.55, 0.15, 0.15));
    extra(m);
}

fn steam_engine(m: &mut Geo, paint: Color) {
    let hl = 0.42;
    underframe(m, hl, 0.085);
    // Pilot truck, three big drivers, trailing wheel.
    m.mat(Mat::Steel);
    for x in [-0.09f32, 0.09] {
        m.cylinder(Vec3::new(x, 0.035, 0.33), 0.035, 0.01, 10, axle(), WHEEL);
        m.cylinder(Vec3::new(x, 0.04, -0.35), 0.04, 0.01, 10, axle(), WHEEL);
    }
    let drivers = [-0.17f32, -0.02, 0.13];
    for z in drivers {
        for x in [-0.1f32, 0.1] {
            m.mat(Mat::Paint);
            m.cylinder(Vec3::new(x, 0.068, z), 0.068, 0.012, 16, axle(), Color::srgb(0.65, 0.08, 0.06));
            m.mat(Mat::Steel);
            m.cylinder(Vec3::new(x * 1.13, 0.068, z), 0.068, 0.003, 16, axle(), WHEEL);
            m.cylinder(Vec3::new(x * 1.2, 0.068, z), 0.014, 0.006, 8, axle(), Color::srgb(0.7, 0.7, 0.7));
            // Spokes.
            for k in 0..4 {
                let q = Quat::from_rotation_x(k as f32 * std::f32::consts::FRAC_PI_4);
                m.cuboid(Vec3::new(x * 1.12, 0.068, z), Vec3::new(0.004, 0.06, 0.006), q, DARK);
            }
        }
    }
    // Cylinders, side rods and main rods.
    for s in [-1.0f32, 1.0] {
        m.mat(Mat::Steel);
        m.cylinder(Vec3::new(s * 0.13, 0.1, 0.3), 0.04, 0.06, 12, along(), IRON);
        m.aabb(Vec3::new(s * 0.142 - 0.004, 0.055, -0.17), Vec3::new(s * 0.142 + 0.004, 0.068, 0.13), Color::srgb(0.75, 0.75, 0.78));
        rail(m, Vec3::new(s * 0.148, 0.1, 0.24), Vec3::new(s * 0.148, 0.06, -0.02), Color::srgb(0.75, 0.75, 0.78));
    }
    // Running boards, boiler, smokebox.
    m.mat(Mat::Paint);
    m.aabb(Vec3::new(-0.155, 0.135, -0.12), Vec3::new(0.155, 0.145, 0.38), DARK);
    m.cylinder(Vec3::new(0.0, 0.23, 0.12), 0.105, 0.24, 20, along(), paint.mix(&DARK, 0.5));
    for z in [-0.06f32, 0.08, 0.22] {
        m.cylinder(Vec3::new(0.0, 0.23, z), 0.108, 0.005, 20, along(), BRASS);
    }
    m.mat(Mat::Steel);
    m.cylinder(Vec3::new(0.0, 0.23, 0.39), 0.11, 0.04, 20, along(), Color::srgb(0.2, 0.2, 0.22));
    m.cylinder(Vec3::new(0.0, 0.23, 0.432), 0.085, 0.004, 20, along(), Color::srgb(0.15, 0.15, 0.16));
    m.cuboid(Vec3::new(0.06, 0.23, 0.437), Vec3::new(0.01, 0.01, 0.003), Quat::IDENTITY, Color::srgb(0.7, 0.7, 0.7));
    // Chimney, domes, bell, whistle, headlamp, handrails.
    m.frustum(Vec3::new(0.0, 0.37, 0.33), 0.035, 0.045, 0.05, 12, Quat::IDENTITY, DARK);
    m.cylinder(Vec3::new(0.0, 0.425, 0.33), 0.05, 0.008, 12, Quat::IDENTITY, DARK);
    m.mat(Mat::Paint);
    m.frustum(Vec3::new(0.0, 0.345, 0.17), 0.05, 0.04, 0.03, 12, Quat::IDENTITY, paint.mix(&DARK, 0.5));
    m.sphere(Vec3::new(0.0, 0.375, 0.17), 0.04, BRASS);
    m.frustum(Vec3::new(0.0, 0.345, 0.02), 0.045, 0.035, 0.025, 12, Quat::IDENTITY, paint.mix(&DARK, 0.5));
    m.sphere(Vec3::new(0.0, 0.37, 0.02), 0.035, BRASS);
    m.frustum(Vec3::new(0.0, 0.35, 0.25), 0.02, 0.012, 0.015, 8, Quat::IDENTITY, BRASS);
    m.mat(Mat::Steel);
    m.cylinder(Vec3::new(0.04, 0.37, -0.06), 0.008, 0.03, 6, Quat::IDENTITY, BRASS);
    m.aabb(Vec3::new(-0.035, 0.34, 0.39), Vec3::new(0.035, 0.39, 0.43), DARK);
    m.mat(Mat::Light);
    m.cylinder(Vec3::new(0.0, 0.365, 0.432), 0.022, 0.003, 12, along(), Color::WHITE);
    for s in [-1.0f32, 1.0] {
        rail(m, Vec3::new(s * 0.1, 0.3, -0.1), Vec3::new(s * 0.1, 0.3, 0.37), Color::srgb(0.8, 0.8, 0.8));
    }
    // Cab with windows, roof and backhead.
    m.mat(Mat::Paint);
    m.aabb(Vec3::new(-0.155, 0.115, -0.42), Vec3::new(0.155, 0.42, -0.12), paint);
    m.aabb(Vec3::new(-0.157, 0.15, -0.42), Vec3::new(0.157, 0.16, -0.12), BRASS);
    m.mat(Mat::Steel);
    m.aabb(Vec3::new(-0.175, 0.42, -0.46), Vec3::new(0.175, 0.44, -0.09), DARK);
    for x in [-0.155f32, 0.155] {
        side_window(m, x, 0.33, -0.25, 0.07, 0.055, BRASS);
    }
    for x in [-0.08f32, 0.08] {
        cab_window(m, Vec3::new(x, 0.35, -0.118), Vec3::Z, 0.05, 0.04);
    }
    m.mat(Mat::Steel);
    m.aabb(Vec3::new(-0.1, 0.15, -0.13), Vec3::new(0.1, 0.32, -0.12), Color::srgb(0.2, 0.2, 0.2));
    for x in [-0.04f32, 0.04] {
        m.cylinder(Vec3::new(x, 0.28, -0.135), 0.015, 0.004, 10, along(), BRASS);
    }
    // Firebox door on the backhead (glows with the fire) and a pressure gauge.
    m.mat(Mat::Steel);
    m.aabb(Vec3::new(-0.05, 0.17, -0.136), Vec3::new(0.05, 0.25, -0.13), Color::srgb(0.12, 0.12, 0.12));
    m.mat(Mat::Fire);
    m.aabb(Vec3::new(-0.035, 0.18, -0.138), Vec3::new(0.035, 0.24, -0.136), Color::WHITE);
    m.mat(Mat::Paint);
    m.cylinder(Vec3::new(0.0, 0.29, -0.137), 0.022, 0.002, 12, along(), WHITE);
    // Slatted cowcatcher and buffer beam.
    m.mat(Mat::Paint);
    m.aabb(Vec3::new(-0.15, 0.08, 0.42), Vec3::new(0.15, 0.13, 0.44), Color::srgb(0.65, 0.08, 0.06));
    for i in 0..7 {
        let x = -0.12 + i as f32 * 0.04;
        m.cuboid(Vec3::new(x, 0.055, 0.465), Vec3::new(0.008, 0.005, 0.05), Quat::from_rotation_x(-0.6), Color::srgb(0.65, 0.08, 0.06));
    }
}

fn diesel_streamliner(m: &mut Geo, paint: Color) {
    let hl = 0.44;
    underframe(m, hl, 0.09);
    truck(m, -0.28, 2, 0.045);
    truck(m, 0.28, 2, 0.045);
    // Fuel tank.
    m.mat(Mat::Steel);
    m.aabb(Vec3::new(-0.12, 0.06, -0.12), Vec3::new(0.12, 0.11, 0.12), IRON);
    // Silver body with a painted, sloped nose.
    m.mat(Mat::Paint);
    let silver = Color::srgb(0.78, 0.8, 0.82);
    m.aabb(Vec3::new(-0.15, 0.12, -hl), Vec3::new(0.15, 0.4, 0.22), silver);
    let (x0, x1, y0, y1, y2) = (-0.15f32, 0.15f32, 0.12f32, 0.27f32, 0.4f32);
    m.aabb(Vec3::new(x0, y0, 0.22), Vec3::new(x1, y1, 0.42), paint);
    m.quad(Vec3::new(x0, y1, 0.42), Vec3::new(x1, y1, 0.42), Vec3::new(x1, 0.36, 0.32), Vec3::new(x0, 0.36, 0.32), paint);
    m.quad(Vec3::new(x0, 0.36, 0.32), Vec3::new(x1, 0.36, 0.32), Vec3::new(x1, y2, 0.22), Vec3::new(x0, y2, 0.22), paint);
    for x in [x0, x1] {
        let pts = [Vec3::new(x, y1, 0.22), Vec3::new(x, y1, 0.42), Vec3::new(x, 0.36, 0.32), Vec3::new(x, y2, 0.22)];
        let p: Vec<Vec3> = if x > 0.0 { pts.to_vec() } else { pts.iter().rev().copied().collect() };
        m.poly(&p, paint);
    }
    // Warbonnet stripes and a yellow nose band.
    m.aabb(Vec3::new(-0.152, 0.2, -hl), Vec3::new(0.152, 0.23, 0.22), paint);
    m.aabb(Vec3::new(-0.152, 0.235, -hl), Vec3::new(0.152, 0.245, 0.22), YELLOW);
    m.aabb(Vec3::new(-0.152, 0.18, 0.22), Vec3::new(0.152, 0.19, 0.422), YELLOW);
    // Windscreen, headlight, number board, horn.
    for x in [-0.07f32, 0.07] {
        cab_window(m, Vec3::new(x, 0.34, 0.344), Quat::from_rotation_x(-0.785) * Vec3::Z, 0.055, 0.025);
    }
    m.mat(Mat::Light);
    m.cylinder(Vec3::new(0.0, 0.25, 0.423), 0.025, 0.003, 12, along(), Color::WHITE);
    m.mat(Mat::Paint);
    m.aabb(Vec3::new(-0.04, 0.29, 0.405), Vec3::new(0.04, 0.31, 0.415), WHITE);
    // Portholes and roof fans.
    for z in [-0.3f32, -0.12, 0.05] {
        for x in [-0.152f32, 0.152] {
            side_window(m, x, 0.32, z, 0.025, 0.025, silver);
        }
    }
    m.mat(Mat::Steel);
    for z in [-0.35f32, -0.2, -0.05] {
        m.cylinder(Vec3::new(0.0, 0.405, z), 0.045, 0.006, 12, Quat::IDENTITY, DARK);
    }
    m.cylinder(Vec3::new(0.05, 0.42, 0.2), 0.008, 0.02, 6, along(), Color::srgb(0.7, 0.7, 0.7));
    for x in [-0.153f32, 0.153] {
        side_window(m, x, 0.33, 0.27, 0.035, 0.03, silver);
    }
}

fn diesel_road(m: &mut Geo, paint: Color) {
    let hl = 0.45;
    // Frame and walkway.
    m.mat(Mat::Steel);
    m.aabb(Vec3::new(-0.155, 0.1, -hl), Vec3::new(0.155, 0.125, hl), DARK);
    underframe(m, hl, 0.08);
    truck(m, -0.29, 3, 0.04);
    truck(m, 0.29, 3, 0.04);
    m.mat(Mat::Steel);
    m.cylinder(Vec3::new(0.0, 0.075, 0.0), 0.05, 0.12, 12, along(), IRON);
    // Long hood with louvres and radiator fans.
    m.mat(Mat::Paint);
    let stripe = Color::srgb(0.95, 0.95, 0.9);
    m.aabb(Vec3::new(-0.1, 0.125, -0.43), Vec3::new(0.1, 0.34, 0.12), paint);
    m.aabb(Vec3::new(-0.102, 0.2, -0.43), Vec3::new(0.102, 0.215, 0.12), stripe);
    for i in 0..9 {
        let z = -0.38 + i as f32 * 0.055;
        for x in [-0.101f32, 0.101] {
            m.aabb(Vec3::new(x - 0.002, 0.24, z - 0.018), Vec3::new(x + 0.002, 0.32, z + 0.018), paint.mix(&DARK, 0.35));
        }
    }
    m.mat(Mat::Steel);
    for z in [-0.35f32, -0.25] {
        m.cylinder(Vec3::new(0.0, 0.345, z), 0.04, 0.006, 12, Quat::IDENTITY, DARK);
    }
    m.mat(Mat::Paint);
    m.aabb(Vec3::new(-0.06, 0.34, -0.12), Vec3::new(0.06, 0.37, 0.02), paint.mix(&DARK, 0.2));
    m.mat(Mat::Steel);
    m.cylinder(Vec3::new(0.0, 0.36, -0.02), 0.018, 0.02, 8, Quat::IDENTITY, DARK);
    // Cab.
    m.mat(Mat::Paint);
    m.aabb(Vec3::new(-0.14, 0.125, 0.12), Vec3::new(0.14, 0.42, 0.29), paint);
    m.aabb(Vec3::new(-0.142, 0.2, 0.12), Vec3::new(0.142, 0.215, 0.29), stripe);
    m.mat(Mat::Steel);
    m.aabb(Vec3::new(-0.15, 0.42, 0.11), Vec3::new(0.15, 0.435, 0.3), ROOF);
    m.cylinder(Vec3::new(0.06, 0.45, 0.2), 0.007, 0.03, 6, along(), Color::srgb(0.7, 0.7, 0.7));
    for x in [-0.142f32, 0.142] {
        side_window(m, x, 0.35, 0.2, 0.06, 0.045, WHITE);
    }
    for x in [-0.07f32, 0.07] {
        cab_window(m, Vec3::new(x, 0.36, 0.292), Vec3::Z, 0.055, 0.04);
        cab_window(m, Vec3::new(x, 0.36, 0.118), Vec3::NEG_Z, 0.05, 0.035);
    }
    // Short hood with headlight and number board.
    m.mat(Mat::Paint);
    m.aabb(Vec3::new(-0.1, 0.125, 0.29), Vec3::new(0.1, 0.3, 0.43), paint);
    m.aabb(Vec3::new(-0.102, 0.2, 0.29), Vec3::new(0.102, 0.215, 0.43), stripe);
    m.aabb(Vec3::new(-0.05, 0.27, 0.43), Vec3::new(0.05, 0.29, 0.435), WHITE);
    m.mat(Mat::Light);
    for z in [0.432f32, -0.432] {
        let s = z.signum();
        m.cylinder(Vec3::new(0.0, 0.25, z), 0.018, 0.004, 10, along(), Color::WHITE);
        // Ditch lights on the pilot.
        for x in [-0.11f32, 0.11] {
            m.cylinder(Vec3::new(x, 0.14, s * 0.462), 0.012, 0.004, 8, along(), Color::WHITE);
        }
    }
    // Pilots with safety stripes, snowplough, steps and handrails.
    for s in [-1.0f32, 1.0] {
        m.mat(Mat::Paint);
        let (za, zb) = (s * hl, s * (hl + 0.012));
        m.aabb(Vec3::new(-0.155, 0.05, za.min(zb)), Vec3::new(0.155, 0.125, za.max(zb)), YELLOW);
        for i in 0..5 {
            let x = -0.12 + i as f32 * 0.06;
            m.cuboid(Vec3::new(x, 0.087, s * (hl + 0.014)), Vec3::new(0.012, 0.035, 0.002), Quat::from_rotation_z(0.6), DARK);
        }
        m.cuboid(Vec3::new(0.0, 0.035, s * (hl + 0.02)), Vec3::new(0.14, 0.02, 0.012), Quat::from_rotation_x(s * 0.4), DARK);
        for x in [-0.14f32, 0.14] {
            let (a, b) = (s * (hl - 0.05), s * (hl - 0.01));
            m.aabb(Vec3::new(x - 0.015, 0.04, a.min(b)), Vec3::new(x + 0.015, 0.1, a.max(b)), YELLOW);
        }
    }
    for x in [-0.15f32, 0.15] {
        rail(m, Vec3::new(x, 0.21, -0.44), Vec3::new(x, 0.21, 0.12), YELLOW);
        rail(m, Vec3::new(x, 0.21, 0.29), Vec3::new(x, 0.21, 0.44), YELLOW);
        for z in [-0.42f32, -0.2, 0.0, 0.3, 0.42] {
            rail(m, Vec3::new(x, 0.125, z), Vec3::new(x, 0.21, z), YELLOW);
        }
    }
}

pub fn car_mesh(kind: CarKind, paint: Color) -> Geo {
    let mut m = Geo::new();
    match kind {
        CarKind::SteamEngine => steam_engine(&mut m, paint),
        CarKind::DieselEngine => diesel_streamliner(&mut m, paint),
        CarKind::DieselRoad => diesel_road(&mut m, paint),
        CarKind::Tender => {
            let hl = 0.26;
            underframe(&mut m, hl, 0.085);
            truck(&mut m, -0.15, 2, 0.035);
            truck(&mut m, 0.15, 2, 0.035);
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-0.15, 0.115, -hl), Vec3::new(0.15, 0.32, hl), paint);
            m.aabb(Vec3::new(-0.152, 0.15, -hl), Vec3::new(0.152, 0.16, hl), BRASS);
            m.aabb(Vec3::new(-0.155, 0.31, -hl), Vec3::new(0.155, 0.33, hl), paint.mix(&DARK, 0.3));
            // Heaped coal and the water hatch.
            m.mat(Mat::Plain);
            for (i, z) in [-0.05f32, 0.05, 0.15].iter().enumerate() {
                m.sphere(Vec3::new(0.0, 0.31, *z), 0.1 + i as f32 * 0.01, Color::srgb(0.06, 0.06, 0.06));
            }
            m.mat(Mat::Steel);
            m.cylinder(Vec3::new(0.0, 0.335, -0.18), 0.04, 0.008, 10, Quat::IDENTITY, DARK);
        }
        CarKind::Passenger => coach(&mut m, paint, Color::srgb(0.92, 0.88, 0.72), 7, 0.035, |_| {}),
        CarKind::Sleeper => coach(&mut m, paint, Color::srgb(0.8, 0.68, 0.3), 9, 0.022, |_| {}),
        CarKind::Dining => coach(&mut m, paint, Color::srgb(0.95, 0.93, 0.85), 4, 0.045, |m| {
            // Kitchen chimney and table lamps.
            m.mat(Mat::Steel);
            m.cylinder(Vec3::new(0.0, 0.41, 0.3), 0.015, 0.03, 8, Quat::IDENTITY, DARK);
            m.mat(Mat::Light);
            for z in [-0.2f32, -0.05, 0.1] {
                m.sphere(Vec3::new(0.12, 0.23, z), 0.01, Color::WHITE);
                m.sphere(Vec3::new(-0.12, 0.23, z), 0.01, Color::WHITE);
            }
        }),
        CarKind::Boxcar => {
            let hl = 0.41;
            underframe(&mut m, hl, 0.09);
            truck(&mut m, -0.28, 2, 0.04);
            truck(&mut m, 0.28, 2, 0.04);
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-0.15, 0.12, -hl), Vec3::new(0.15, 0.38, hl), paint);
            // Ribs, sliding door with rail, roof walk.
            for i in 0..9 {
                let z = -0.37 + i as f32 * 0.0925;
                for x in [-0.152f32, 0.152] {
                    m.aabb(Vec3::new(x - 0.003, 0.12, z - 0.006), Vec3::new(x + 0.003, 0.38, z + 0.006), paint.mix(&DARK, 0.2));
                }
            }
            for x in [-0.156f32, 0.156] {
                m.mat(Mat::Paint);
                m.aabb(Vec3::new(x - 0.004, 0.13, -0.1), Vec3::new(x + 0.004, 0.36, 0.1), paint.mix(&DARK, 0.35));
                m.mat(Mat::Steel);
                m.aabb(Vec3::new(x - 0.005, 0.36, -0.2), Vec3::new(x + 0.005, 0.37, 0.2), DARK);
            }
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-0.154, 0.38, -hl), Vec3::new(0.154, 0.395, hl), paint.mix(&DARK, 0.3));
            m.aabb(Vec3::new(-0.12, 0.26, 0.412), Vec3::new(0.12, 0.3, 0.414), WHITE);
            m.mat(Mat::Planks);
            m.aabb(Vec3::new(-0.025, 0.395, -hl), Vec3::new(0.025, 0.402, hl), Color::srgb(0.5, 0.4, 0.3));
        }
        CarKind::Tanker => {
            let hl = 0.4;
            underframe(&mut m, hl, 0.09);
            truck(&mut m, -0.28, 2, 0.04);
            truck(&mut m, 0.28, 2, 0.04);
            m.mat(Mat::Paint);
            m.cylinder(Vec3::new(0.0, 0.24, 0.0), 0.125, 0.37, 20, along(), paint);
            for z in [-0.37f32, 0.37] {
                m.sphere(Vec3::new(0.0, 0.24, z), 0.125, paint);
            }
            m.cylinder(Vec3::new(0.0, 0.38, 0.0), 0.05, 0.025, 12, Quat::IDENTITY, paint);
            m.aabb(Vec3::new(-0.11, 0.115, -0.3), Vec3::new(0.11, 0.13, 0.3), DARK);
            m.mat(Mat::Steel);
            m.aabb(Vec3::new(-0.14, 0.33, -0.08), Vec3::new(0.14, 0.34, 0.08), DARK);
            for x in [-0.14f32, 0.14] {
                rail(&mut m, Vec3::new(x, 0.34, -0.08), Vec3::new(x, 0.34, 0.08), DARK);
                rail(&mut m, Vec3::new(x, 0.12, 0.0), Vec3::new(x, 0.34, 0.0), DARK);
            }
        }
        CarKind::Hopper => {
            let hl = 0.39;
            underframe(&mut m, hl, 0.09);
            truck(&mut m, -0.28, 2, 0.04);
            truck(&mut m, 0.28, 2, 0.04);
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-0.15, 0.17, -hl), Vec3::new(0.15, 0.35, hl), paint);
            for z in [-0.2f32, 0.0, 0.2] {
                m.cuboid(Vec3::new(0.0, 0.13, z), Vec3::new(0.13, 0.04, 0.07), Quat::IDENTITY, paint.mix(&DARK, 0.2));
            }
            for i in 0..7 {
                let z = -0.33 + i as f32 * 0.11;
                for x in [-0.152f32, 0.152] {
                    m.aabb(Vec3::new(x - 0.003, 0.17, z - 0.006), Vec3::new(x + 0.003, 0.35, z + 0.006), paint.mix(&DARK, 0.25));
                }
            }
            m.mat(Mat::Plain);
            m.aabb(Vec3::new(-0.14, 0.18, -hl + 0.01), Vec3::new(0.14, 0.19, hl - 0.01), Color::srgb(0.12, 0.12, 0.12));
        }
        CarKind::LogCar => {
            let hl = 0.41;
            underframe(&mut m, hl, 0.09);
            truck(&mut m, -0.28, 2, 0.04);
            truck(&mut m, 0.28, 2, 0.04);
            m.mat(Mat::Planks);
            m.aabb(Vec3::new(-0.15, 0.12, -hl), Vec3::new(0.15, 0.14, hl), Color::srgb(0.6, 0.5, 0.4));
            m.mat(Mat::Steel);
            for z in [-0.38f32, -0.1, 0.1, 0.38] {
                for x in [-0.14f32, 0.14] {
                    m.aabb(Vec3::new(x - 0.008, 0.14, z - 0.008), Vec3::new(x + 0.008, 0.33, z + 0.008), DARK);
                }
            }
        }
        CarKind::Caboose => {
            let hl = 0.3;
            underframe(&mut m, hl, 0.09);
            truck(&mut m, -0.18, 2, 0.04);
            truck(&mut m, 0.18, 2, 0.04);
            m.mat(Mat::Planks);
            m.aabb(Vec3::new(-0.15, 0.12, -0.24), Vec3::new(0.15, 0.34, 0.24), paint);
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-0.16, 0.34, -0.29), Vec3::new(0.16, 0.355, 0.29), DARK);
            m.aabb(Vec3::new(-0.09, 0.355, -0.08), Vec3::new(0.09, 0.45, 0.08), paint);
            m.aabb(Vec3::new(-0.1, 0.45, -0.1), Vec3::new(0.1, 0.465, 0.1), DARK);
            m.mat(Mat::Steel);
            m.cylinder(Vec3::new(0.06, 0.4, -0.18), 0.01, 0.05, 6, Quat::IDENTITY, DARK);
            for x in [-0.152f32, 0.152] {
                for z in [-0.14f32, 0.14] {
                    side_window(&mut m, x, 0.26, z, 0.035, 0.035, WHITE);
                }
                side_window(&mut m, x * 0.6, 0.405, 0.0, 0.045, 0.025, WHITE);
            }
            // End platforms with railings and red marker lights.
            for s in [-1.0f32, 1.0] {
                m.mat(Mat::Planks);
                let (a, b) = (s * 0.24, s * 0.3);
                m.aabb(Vec3::new(-0.15, 0.11, a.min(b)), Vec3::new(0.15, 0.12, a.max(b)), Color::srgb(0.5, 0.4, 0.3));
                for x in [-0.14f32, 0.14] {
                    rail(&mut m, Vec3::new(x, 0.12, s * 0.29), Vec3::new(x, 0.25, s * 0.29), YELLOW);
                    rail(&mut m, Vec3::new(x, 0.25, s * 0.24), Vec3::new(x, 0.25, s * 0.29), YELLOW);
                }
                rail(&mut m, Vec3::new(-0.14, 0.25, s * 0.29), Vec3::new(0.14, 0.25, s * 0.29), YELLOW);
                m.mat(Mat::Paint);
                for x in [-0.13f32, 0.13] {
                    m.cuboid(Vec3::new(x, 0.32, s * 0.245), Vec3::splat(0.012), Quat::IDENTITY, Color::srgb(0.9, 0.05, 0.05));
                }
            }
            seats(&mut m, 0.22, Color::srgb(0.4, 0.3, 0.2));
        }
    }
    m
}

pub fn default_paint(kind: CarKind) -> Color {
    match kind {
        CarKind::SteamEngine => rgb(0.1, 0.25, 0.15),
        CarKind::DieselEngine => rgb(0.75, 0.12, 0.1),
        CarKind::DieselRoad => rgb(0.1, 0.25, 0.55),
        CarKind::Tender => rgb(0.1, 0.25, 0.15),
        CarKind::Passenger => rgb(0.15, 0.45, 0.25),
        CarKind::Sleeper => rgb(0.12, 0.18, 0.4),
        CarKind::Dining => rgb(0.5, 0.12, 0.15),
        CarKind::Boxcar => rgb(0.55, 0.28, 0.16),
        CarKind::Tanker => rgb(0.15, 0.15, 0.17),
        CarKind::Hopper => rgb(0.35, 0.3, 0.28),
        CarKind::LogCar => rgb(0.4, 0.3, 0.2),
        CarKind::Caboose => rgb(0.75, 0.12, 0.1),
    }
}

/// Road vehicle styles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoadStyle {
    Car,
    Van,
    Bus,
    Truck,
    Police,
    Ambulance,
    FireEngine,
}

impl RoadStyle {
    pub fn half_len(self) -> f32 {
        match self {
            RoadStyle::Car | RoadStyle::Police => 0.11,
            RoadStyle::Van => 0.12,
            RoadStyle::Ambulance => 0.13,
            RoadStyle::Bus => 0.2,
            RoadStyle::Truck => 0.18,
            RoadStyle::FireEngine => 0.21,
        }
    }
}

fn wheels(m: &mut Geo, hw: f32, zs: &[f32], r: f32) {
    for &z in zs {
        for x in [-hw + 0.005, hw - 0.005] {
            m.mat(Mat::Plain);
            m.cylinder(Vec3::new(x, r, z), r, 0.009, 10, axle(), DARK);
            m.mat(Mat::Steel);
            m.cylinder(Vec3::new(x * 1.08, r, z), r * 0.55, 0.002, 8, axle(), Color::srgb(0.75, 0.75, 0.78));
        }
    }
}

fn lights(m: &mut Geo, hw: f32, hl: f32, y: f32) {
    m.mat(Mat::Light);
    for x in [-hw * 0.65, hw * 0.65] {
        m.cuboid(Vec3::new(x, y, hl + 0.001), Vec3::new(0.012, 0.007, 0.002), Quat::IDENTITY, Color::WHITE);
    }
    m.mat(Mat::Paint);
    for x in [-hw * 0.7, hw * 0.7] {
        m.cuboid(Vec3::new(x, y, -hl - 0.001), Vec3::new(0.012, 0.007, 0.002), Quat::IDENTITY, Color::srgb(0.85, 0.05, 0.05));
    }
}

/// Saloon car body: bumpers, sloped screens, cabin with windows.
fn saloon(m: &mut Geo, paint: Color, roof: Color) {
    let (hl, hw) = (0.11f32, 0.052f32);
    wheels(m, hw, &[-0.07, 0.07], 0.02);
    m.mat(Mat::Paint);
    m.aabb(Vec3::new(-hw, 0.018, -hl), Vec3::new(hw, 0.05, hl), paint);
    let (zf, zr, yb, yt) = (0.04f32, -0.06f32, 0.05f32, 0.085f32);
    let w = hw - 0.004;
    m.mat(Mat::Glass);
    m.quad(Vec3::new(-w, yb, zf + 0.025), Vec3::new(w, yb, zf + 0.025), Vec3::new(w, yt, zf - 0.01), Vec3::new(-w, yt, zf - 0.01), GLASS);
    m.quad(Vec3::new(w, yb, zr - 0.02), Vec3::new(-w, yb, zr - 0.02), Vec3::new(-w, yt, zr + 0.01), Vec3::new(w, yt, zr + 0.01), GLASS);
    for x in [-w, w] {
        let pts = [Vec3::new(x, yb, zr - 0.02), Vec3::new(x, yb, zf + 0.025), Vec3::new(x, yt, zf - 0.01), Vec3::new(x, yt, zr + 0.01)];
        let p: Vec<Vec3> = if x > 0.0 { pts.to_vec() } else { pts.iter().rev().copied().collect() };
        m.poly(&p, GLASS);
    }
    m.mat(Mat::Paint);
    m.aabb(Vec3::new(-w, yt, zr + 0.01), Vec3::new(w, yt + 0.006, zf - 0.01), roof);
    for x in [-w - 0.001, w + 0.001] {
        m.aabb(Vec3::new(x - 0.001, yb, -0.012), Vec3::new(x + 0.001, yt, -0.006), paint);
        m.cuboid(Vec3::new(x * 1.15, 0.058, zf + 0.012), Vec3::new(0.006, 0.004, 0.003), Quat::IDENTITY, paint);
    }
    m.mat(Mat::Steel);
    for z in [hl, -hl] {
        m.aabb(Vec3::new(-hw - 0.002, 0.018, z - 0.006), Vec3::new(hw + 0.002, 0.03, z + 0.006), Color::srgb(0.6, 0.6, 0.62));
    }
    lights(m, hw, hl + 0.006, 0.04);
}

/// Roof light bar for emergency vehicles (flashing when on a call).
fn light_bar(m: &mut Geo, y: f32, z: f32, w: f32, on_call: bool) {
    m.mat(Mat::Steel);
    m.aabb(Vec3::new(-w, y, z - 0.012), Vec3::new(w, y + 0.006, z + 0.012), DARK);
    if on_call {
        m.mat(Mat::FlashRed);
        m.aabb(Vec3::new(-w, y + 0.006, z - 0.01), Vec3::new(-0.003, y + 0.018, z + 0.01), Color::WHITE);
        m.mat(Mat::FlashBlue);
        m.aabb(Vec3::new(0.003, y + 0.006, z - 0.01), Vec3::new(w, y + 0.018, z + 0.01), Color::WHITE);
    } else {
        m.mat(Mat::Paint);
        m.aabb(Vec3::new(-w, y + 0.006, z - 0.01), Vec3::new(-0.003, y + 0.018, z + 0.01), Color::srgb(0.6, 0.1, 0.1));
        m.aabb(Vec3::new(0.003, y + 0.006, z - 0.01), Vec3::new(w, y + 0.018, z + 0.01), Color::srgb(0.1, 0.15, 0.6));
    }
}

/// Road vehicles. Front faces +Z, origin at road level.
pub fn road_vehicle_mesh(style: RoadStyle, paint: Color, on_call: bool) -> Geo {
    let mut m = Geo::new();
    match style {
        RoadStyle::Car => saloon(&mut m, paint, paint),
        RoadStyle::Police => {
            saloon(&mut m, WHITE, WHITE);
            m.mat(Mat::Paint);
            let navy = Color::srgb(0.08, 0.12, 0.35);
            for x in [-0.0535f32, 0.0535] {
                m.aabb(Vec3::new(x - 0.001, 0.025, -0.05), Vec3::new(x + 0.001, 0.045, 0.05), navy);
            }
            m.aabb(Vec3::new(-0.052, 0.018, 0.07), Vec3::new(0.052, 0.051, 0.11), navy);
            m.aabb(Vec3::new(-0.052, 0.018, -0.11), Vec3::new(0.052, 0.051, -0.08), navy);
            light_bar(&mut m, 0.091, -0.01, 0.035, on_call);
        }
        RoadStyle::Van | RoadStyle::Ambulance => {
            let (hl, hw) = (style.half_len(), 0.058f32);
            let body = if style == RoadStyle::Ambulance { WHITE } else { paint };
            wheels(&mut m, hw, &[-hl * 0.6, hl * 0.62], 0.022);
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-hw, 0.02, -hl), Vec3::new(hw, 0.125, hl - 0.04), body);
            m.aabb(Vec3::new(-hw, 0.02, hl - 0.04), Vec3::new(hw, 0.07, hl), body);
            m.quad(Vec3::new(-hw, 0.07, hl), Vec3::new(hw, 0.07, hl), Vec3::new(hw, 0.115, hl - 0.04), Vec3::new(-hw, 0.115, hl - 0.04), body);
            m.mat(Mat::Glass);
            m.cuboid(Vec3::new(0.0, 0.095, hl - 0.018), Vec3::new(hw - 0.006, 0.018, 0.002), Quat::from_rotation_x(-0.8), GLASS);
            for x in [-hw - 0.001, hw + 0.001] {
                m.aabb(Vec3::new(x - 0.001, 0.07, hl - 0.075), Vec3::new(x + 0.001, 0.11, hl - 0.04), GLASS);
            }
            if style == RoadStyle::Ambulance {
                let red = Color::srgb(0.85, 0.1, 0.1);
                m.mat(Mat::Paint);
                for x in [-hw - 0.001, hw + 0.001] {
                    m.aabb(Vec3::new(x - 0.001, 0.045, -hl), Vec3::new(x + 0.001, 0.058, hl), red);
                    let c = Vec3::new(x + x.signum() * 0.001, 0.09, -0.03);
                    m.cuboid(c, Vec3::new(0.001, 0.008, 0.024), Quat::IDENTITY, red);
                    m.cuboid(c, Vec3::new(0.001, 0.024, 0.008), Quat::IDENTITY, red);
                }
                m.aabb(Vec3::new(-0.04, 0.06, -hl - 0.002), Vec3::new(0.04, 0.07, -hl), red);
                light_bar(&mut m, 0.125, hl - 0.06, 0.04, on_call);
            }
            m.mat(Mat::Steel);
            for z in [hl, -hl] {
                m.aabb(Vec3::new(-hw - 0.002, 0.018, z - 0.006), Vec3::new(hw + 0.002, 0.03, z + 0.006), DARK);
            }
            lights(&mut m, hw, hl + 0.006, 0.05);
        }
        RoadStyle::Bus => {
            let (hl, hw) = (0.2f32, 0.065f32);
            wheels(&mut m, hw, &[-0.13, 0.12], 0.024);
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-hw, 0.02, -hl), Vec3::new(hw, 0.14, hl), paint);
            m.aabb(Vec3::new(-hw - 0.001, 0.02, -hl), Vec3::new(hw + 0.001, 0.045, hl), WHITE);
            m.mat(Mat::Glass);
            m.aabb(Vec3::new(-hw - 0.002, 0.08, -hl + 0.02), Vec3::new(hw + 0.002, 0.125, hl - 0.005), GLASS);
            m.aabb(Vec3::new(-hw + 0.005, 0.06, hl), Vec3::new(hw - 0.005, 0.13, hl + 0.002), GLASS);
            m.mat(Mat::Light);
            m.aabb(Vec3::new(-0.04, 0.128, hl), Vec3::new(0.04, 0.138, hl + 0.003), Color::srgb(1.0, 0.7, 0.1));
            lights(&mut m, hw, hl + 0.004, 0.04);
        }
        RoadStyle::Truck => {
            let (hl, hw) = (0.18f32, 0.06f32);
            wheels(&mut m, hw, &[-0.13, -0.08, 0.12], 0.022);
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-hw, 0.025, 0.08), Vec3::new(hw, 0.12, hl), paint);
            m.mat(Mat::Glass);
            m.aabb(Vec3::new(-hw + 0.005, 0.075, hl), Vec3::new(hw - 0.005, 0.11, hl + 0.002), GLASS);
            for x in [-hw - 0.001, hw + 0.001] {
                m.aabb(Vec3::new(x - 0.001, 0.075, 0.12), Vec3::new(x + 0.001, 0.11, 0.16), GLASS);
            }
            m.mat(Mat::Corrugated);
            m.aabb(Vec3::new(-hw, 0.035, -hl), Vec3::new(hw, 0.14, 0.07), Color::srgb(0.92, 0.92, 0.92));
            m.mat(Mat::Steel);
            m.aabb(Vec3::new(-hw, 0.02, -hl), Vec3::new(hw, 0.035, hl), DARK);
            lights(&mut m, hw, hl + 0.004, 0.045);
        }
        RoadStyle::FireEngine => {
            let (hl, hw) = (0.21f32, 0.065f32);
            let red = Color::srgb(0.8, 0.06, 0.05);
            wheels(&mut m, hw, &[-0.14, -0.09, 0.13], 0.024);
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-hw, 0.025, 0.08), Vec3::new(hw, 0.13, hl), red);
            m.aabb(Vec3::new(-hw, 0.025, -hl), Vec3::new(hw, 0.11, 0.08), red);
            m.mat(Mat::Glass);
            m.aabb(Vec3::new(-hw + 0.005, 0.08, hl), Vec3::new(hw - 0.005, 0.12, hl + 0.002), GLASS);
            for x in [-hw - 0.001, hw + 0.001] {
                m.aabb(Vec3::new(x - 0.001, 0.08, 0.1), Vec3::new(x + 0.001, 0.12, 0.19), GLASS);
            }
            // Equipment lockers, white stripe, ladder on top.
            m.mat(Mat::Steel);
            for i in 0..4 {
                let z = -0.18 + i as f32 * 0.065;
                for x in [-hw - 0.001, hw + 0.001] {
                    m.aabb(Vec3::new(x - 0.001, 0.04, z), Vec3::new(x + 0.001, 0.1, z + 0.055), Color::srgb(0.75, 0.75, 0.78));
                }
            }
            m.mat(Mat::Paint);
            for x in [-hw - 0.002, hw + 0.002] {
                m.aabb(Vec3::new(x - 0.001, 0.03, -hl), Vec3::new(x + 0.001, 0.038, hl), WHITE);
            }
            m.mat(Mat::Steel);
            for x in [-0.03f32, 0.03] {
                m.aabb(Vec3::new(x - 0.004, 0.12, -hl - 0.03), Vec3::new(x + 0.004, 0.128, 0.06), Color::srgb(0.8, 0.8, 0.82));
            }
            for i in 0..12 {
                let z = -hl - 0.02 + i as f32 * 0.022;
                m.aabb(Vec3::new(-0.03, 0.121, z - 0.002), Vec3::new(0.03, 0.127, z + 0.002), Color::srgb(0.8, 0.8, 0.82));
            }
            m.cylinder(Vec3::new(0.0, 0.118, -0.05), 0.02, 0.008, 8, Quat::IDENTITY, DARK);
            m.aabb(Vec3::new(-hw - 0.003, 0.02, hl - 0.005), Vec3::new(hw + 0.003, 0.04, hl + 0.01), Color::srgb(0.8, 0.8, 0.82));
            light_bar(&mut m, 0.13, 0.15, 0.045, on_call);
            lights(&mut m, hw, hl + 0.01, 0.055);
        }
    }
    m
}

/// One log of a log car's load (index 0..LOGS).
pub const LOGS: usize = 6;
pub fn cargo_log_mesh(i: usize) -> Geo {
    let mut m = Geo::new();
    let (x, y) = [(-0.08f32, 0.185f32), (0.0, 0.185), (0.08, 0.185), (-0.04, 0.265), (0.04, 0.265), (0.0, 0.34)][i % LOGS];
    m.mat(Mat::Plain);
    m.cylinder(Vec3::new(x, y, 0.0), 0.042, 0.37, 9, along(), Color::srgb(0.45, 0.32, 0.2));
    m.cylinder(Vec3::new(x, y, 0.371), 0.036, 0.002, 9, along(), Color::srgb(0.8, 0.65, 0.45));
    m.cylinder(Vec3::new(x, y, -0.371), 0.036, 0.002, 9, along(), Color::srgb(0.8, 0.65, 0.45));
    m
}

/// A hopper's heap of rocks; origin at the bottom so it can be scaled by fill.
pub fn cargo_rock_mesh() -> Geo {
    let mut m = Geo::new();
    m.mat(Mat::Rock);
    for (i, z) in [-0.24f32, -0.12, 0.0, 0.12, 0.24].iter().enumerate() {
        let r = 0.11 + (i % 2) as f32 * 0.02;
        m.sphere(Vec3::new(0.0, r * 0.4, *z), r, Color::srgb(0.85, 0.84, 0.8));
    }
    m
}

/// The coloured band and dome cap showing what a tank car holds.
pub fn cargo_band_mesh(color: Color) -> Geo {
    let mut m = Geo::new();
    m.mat(Mat::Paint);
    m.cylinder(Vec3::new(0.0, 0.24, 0.0), 0.128, 0.12, 20, along(), color);
    m.cylinder(Vec3::new(0.0, 0.41, 0.0), 0.052, 0.006, 12, Quat::IDENTITY, color);
    m
}