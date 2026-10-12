//! Meshes for things placed on tiles: track, roads, stations, bridges,
//! tunnel portals and buildings / scenery.

use crate::grid::*;
use crate::mesh::*;
use crate::track::*;
use bevy::prelude::*;
use std::f32::consts::{FRAC_PI_2, FRAC_PI_4};

const WHITE: Color = Color::srgb(0.95, 0.95, 0.92);
const TRIM: Color = Color::srgb(0.93, 0.92, 0.88);
const GLASS: Color = Color::srgb(0.32, 0.42, 0.52);
const DARK: Color = Color::srgb(0.12, 0.12, 0.13);
const TEX: Color = Color::srgb(1.0, 1.0, 1.0);

/// Builds the mesh for everything sitting on a tile (excluding terrain),
/// in tile-local coordinates (origin = tile centre at table height 0).
pub fn tile_mesh(grid: &Grid, t: IVec2) -> Geo {
    let mut mb = Geo::new();
    let Some(tile) = grid.get(t) else { return mb };
    let ground = tile.surface_y();
    match tile.content {
        Content::Empty => {}
        Content::Track(tr) => track_mesh(&mut mb, grid, t, &tr, tile.height),
        Content::Road => road_mesh(&mut mb, &grid.road_edges(t), ground),
        Content::Decor { kind, rot, variant } => {
            let d = decor_mesh(kind, variant, t);
            mb.append(d, Transform::from_xyz(0.0, ground, 0.0).with_rotation(rot_quat(rot)));
        }
    }
    mb
}

fn path_points(tr: &Track, a: u8, b: u8, n: usize) -> Vec<(Vec3, Vec3)> {
    let ya = tr.edge_level(a) as f32 * LEVEL_H;
    let yb = tr.edge_level(b) as f32 * LEVEL_H;
    (0..=n).map(|i| path_local(a, b, i as f32 / n as f32, ya, yb)).collect()
}

fn track_mesh(mb: &mut Geo, grid: &Grid, t: IVec2, tr: &Track, height: i32) {
    let terrain_y = height as f32 * LEVEL_H;
    let elevated = tr.max_level() > height;
    let n = 8;
    for (pi, &(a, b)) in tr.paths().iter().enumerate() {
        let pts = path_points(tr, a, b, n);
        // On a switch, the route that is set has polished rails.
        let active = !tr.is_switch() || (pi == 1) == tr.diverge;
        let rail = if active { Color::srgb(0.85, 0.82, 0.8) } else { Color::srgb(0.5, 0.4, 0.33) };
        for i in 0..n {
            let (p0, _) = pts[i];
            let (p1, _) = pts[i + 1];
            let mid = (p0 + p1) * 0.5;
            let dir = p1 - p0;
            let len = dir.length();
            let q = basis_from_dir(dir);
            if pi == 0 || tr.kind != TrackKind::Crossing {
                mb.mat(Mat::Ballast);
                mb.cuboid(mid + Vec3::Y * 0.008, Vec3::new(0.2, 0.012, len * 0.5 + 0.01), q, Color::srgb(0.85, 0.82, 0.78));
                mb.cuboid(mid + Vec3::Y * 0.018, Vec3::new(0.16, 0.006, len * 0.5 + 0.01), q, Color::srgb(0.75, 0.72, 0.68));
            }
            mb.mat(Mat::Planks);
            for k in [0.25f32, 0.75] {
                let tp = p0.lerp(p1, k);
                mb.cuboid(tp + Vec3::Y * 0.03, Vec3::new(0.14, 0.007, 0.018), q, Color::srgb(0.5, 0.4, 0.33));
            }
            mb.mat(Mat::Steel);
            for side in [-1.0f32, 1.0] {
                let off = q * Vec3::X * 0.09 * side;
                // Rail foot, web and head.
                mb.cuboid(mid + off + Vec3::Y * 0.039, Vec3::new(0.012, 0.003, len * 0.5 + 0.004), q, Color::srgb(0.45, 0.35, 0.28));
                mb.cuboid(mid + off + Vec3::Y * 0.048, Vec3::new(0.004, 0.008, len * 0.5 + 0.004), q, Color::srgb(0.45, 0.35, 0.28));
                mb.cuboid(mid + off + Vec3::Y * 0.057, Vec3::new(0.008, 0.004, len * 0.5 + 0.004), q, rail);
            }
            if elevated {
                // Bridge deck, steel girders and railings.
                mb.mat(Mat::Planks);
                mb.cuboid(mid - Vec3::Y * 0.015, Vec3::new(0.25, 0.015, len * 0.5 + 0.01), q, Color::srgb(0.6, 0.5, 0.42));
                mb.mat(Mat::Paint);
                for side in [-1.0f32, 1.0] {
                    let off = q * Vec3::X * 0.24 * side;
                    mb.cuboid(mid + off - Vec3::Y * 0.02, Vec3::new(0.01, 0.06, len * 0.5 + 0.01), q, Color::srgb(0.5, 0.16, 0.12));
                    mb.cuboid(mid + off + Vec3::Y * 0.11, Vec3::new(0.006, 0.006, len * 0.5 + 0.01), q, Color::srgb(0.5, 0.16, 0.12));
                    mb.cuboid(mid + off + Vec3::Y * 0.065, Vec3::new(0.005, 0.045, 0.005), q, Color::srgb(0.5, 0.16, 0.12));
                }
            }
        }
        if elevated && pi == 0 {
            let (pm, _) = pts[n / 2];
            let top = pm.y - 0.08;
            if top > terrain_y {
                let h = (top - terrain_y) * 0.5;
                mb.mat(Mat::Stone);
                mb.cuboid(Vec3::new(pm.x, terrain_y + h, pm.z), Vec3::new(0.1, h, 0.1), Quat::IDENTITY, TEX);
                mb.cuboid(Vec3::new(pm.x, top - 0.02, pm.z), Vec3::new(0.24, 0.03, 0.12), basis_from_dir(pts[n / 2].1.with_y(0.0)), TEX);
            }
        }
    }

    let y0 = tr.level as f32 * LEVEL_H;
    let rq = rot_quat(tr.rot);
    match tr.kind {
        TrackKind::SwitchLeft | TrackKind::SwitchRight => {
            // Switch stand beside the stem with a coloured target.
            let side = if tr.kind == TrackKind::SwitchLeft { 1.0 } else { -1.0 };
            let mut s = Geo::new();
            s.mat(Mat::Paint);
            s.aabb(Vec3::new(0.3 * side - 0.03, 0.0, 0.3), Vec3::new(0.3 * side + 0.03, 0.03, 0.36), DARK);
            s.aabb(Vec3::new(0.3 * side - 0.008, 0.03, 0.322), Vec3::new(0.3 * side + 0.008, 0.18, 0.338), DARK);
            let c = if tr.diverge { Color::srgb(1.0, 0.55, 0.0) } else { Color::srgb(0.1, 0.85, 0.2) };
            s.cuboid(Vec3::new(0.3 * side, 0.21, 0.33), Vec3::new(0.05, 0.035, 0.008), Quat::IDENTITY, c);
            s.mat(Mat::Steel);
            s.cuboid(Vec3::new(0.18 * side, 0.03, 0.33), Vec3::new(0.12, 0.006, 0.006), Quat::IDENTITY, DARK);
            mb.append(s, Transform::from_xyz(0.0, y0, 0.0).with_rotation(rq));
        }
        TrackKind::Station => {
            mb.append(station_mesh(), Transform::from_xyz(0.0, y0, 0.0).with_rotation(rq));
        }
        TrackKind::LevelCrossing => {
            let mut s = Geo::new();
            s.mat(Mat::Asphalt);
            s.aabb(Vec3::new(-0.5, 0.0, -0.3), Vec3::new(-0.2, 0.03, 0.3), TEX);
            s.aabb(Vec3::new(0.2, 0.0, -0.3), Vec3::new(0.5, 0.03, 0.3), TEX);
            s.mat(Mat::Planks);
            s.aabb(Vec3::new(-0.2, 0.0, -0.3), Vec3::new(-0.105, 0.05, 0.3), Color::srgb(0.6, 0.5, 0.4));
            s.aabb(Vec3::new(-0.075, 0.0, -0.3), Vec3::new(0.075, 0.05, 0.3), Color::srgb(0.6, 0.5, 0.4));
            s.aabb(Vec3::new(0.105, 0.0, -0.3), Vec3::new(0.2, 0.05, 0.3), Color::srgb(0.6, 0.5, 0.4));
            for (x, z) in [(0.32f32, 0.38f32), (-0.32, -0.38)] {
                s.mat(Mat::Paint);
                s.aabb(Vec3::new(x - 0.012, 0.0, z - 0.012), Vec3::new(x + 0.012, 0.3, z + 0.012), WHITE);
                // Crossbuck sign.
                for r in [FRAC_PI_4, -FRAC_PI_4] {
                    s.cuboid(Vec3::new(x, 0.33, z), Vec3::new(0.06, 0.01, 0.003), Quat::from_rotation_z(r) * Quat::from_rotation_y(FRAC_PI_2), WHITE);
                }
                s.aabb(Vec3::new(x - 0.04, 0.2, z - 0.01), Vec3::new(x + 0.04, 0.215, z + 0.01), DARK);
                for d in [-0.03f32, 0.03] {
                    s.cuboid(Vec3::new(x + d, 0.225, z), Vec3::new(0.012, 0.012, 0.012), Quat::IDENTITY, Color::srgb(0.85, 0.1, 0.08));
                }
            }
            mb.append(s, Transform::from_xyz(0.0, y0, 0.0).with_rotation(rq));
        }
        _ => {}
    }

    // Tunnel portals where the track leaves the hillside.
    if grid.is_tunnel(t) {
        for e in tr.edges() {
            let nt = t + edge_dir(e);
            if grid.is_tunnel(nt) {
                continue;
            }
            let yl = tr.edge_level(e) as f32 * LEVEL_H;
            let top = (height as f32 * LEVEL_H).min(yl + 0.75);
            let mut p = Geo::new();
            let fz = 0.505;
            p.mat(Mat::Plain);
            p.aabb(Vec3::new(-0.3, yl, fz - 0.01), Vec3::new(0.3, yl + 0.46, fz), Color::srgb(0.02, 0.02, 0.02));
            p.mat(Mat::Stone);
            p.aabb(Vec3::new(-0.4, yl, fz), Vec3::new(-0.26, top, fz + 0.06), TEX);
            p.aabb(Vec3::new(0.26, yl, fz), Vec3::new(0.4, top, fz + 0.06), TEX);
            p.aabb(Vec3::new(-0.4, yl + 0.46, fz), Vec3::new(0.4, top.max(yl + 0.58), fz + 0.06), TEX);
            p.aabb(Vec3::new(-0.44, top.max(yl + 0.58) - 0.04, fz), Vec3::new(0.44, top.max(yl + 0.58), fz + 0.09), Color::srgb(0.8, 0.8, 0.8));
            // Base orientation faces +Z (south); rotate to face edge e.
            let rot = Quat::from_rotation_y(-FRAC_PI_2 * ((e + 2) % 4) as f32);
            mb.append(p, Transform::from_rotation(rot));
        }
    }
}

fn station_mesh() -> Geo {
    let mut s = Geo::new();
    // Platform on the +X side with a yellow safety line.
    s.mat(Mat::Concrete);
    s.aabb(Vec3::new(0.2, 0.0, -0.5), Vec3::new(0.5, 0.12, 0.5), Color::srgb(0.85, 0.83, 0.8));
    s.mat(Mat::Paint);
    s.aabb(Vec3::new(0.2, 0.12, -0.5), Vec3::new(0.225, 0.123, 0.5), Color::srgb(0.95, 0.8, 0.1));
    // Brick ticket office with a slate roof.
    s.mat(Mat::Brick);
    s.aabb(Vec3::new(0.36, 0.12, -0.22), Vec3::new(0.5, 0.38, 0.22), TEX);
    s.mat(Mat::RoofSlate);
    s.roof(Vec3::new(0.43, 0.38, 0.0), 0.5, 0.2, 0.1, Quat::from_rotation_y(FRAC_PI_2), TEX, TEX, Mat::Brick);
    s.mat(Mat::Planks);
    s.aabb(Vec3::new(0.355, 0.12, -0.04), Vec3::new(0.36, 0.28, 0.04), Color::srgb(0.45, 0.3, 0.2));
    window(&mut s, Vec3::new(0.36, 0.25, 0.12), Vec3::NEG_X, 0.07, 0.07, TRIM);
    window(&mut s, Vec3::new(0.36, 0.25, -0.12), Vec3::NEG_X, 0.07, 0.07, TRIM);
    // Canopy on green iron posts.
    s.mat(Mat::Paint);
    for z in [-0.4f32, 0.0, 0.4] {
        s.aabb(Vec3::new(0.285, 0.12, z - 0.01), Vec3::new(0.305, 0.43, z + 0.01), Color::srgb(0.15, 0.3, 0.2));
    }
    s.mat(Mat::Corrugated);
    s.cuboid(Vec3::new(0.31, 0.45, 0.0), Vec3::new(0.1, 0.008, 0.49), Quat::from_rotation_z(-0.12), Color::srgb(0.55, 0.7, 0.6));
    // Station sign, clock, bench and lamps.
    s.mat(Mat::Paint);
    s.aabb(Vec3::new(0.29, 0.33, 0.15), Vec3::new(0.3, 0.38, 0.33), Color::srgb(0.1, 0.2, 0.6));
    s.aabb(Vec3::new(0.288, 0.345, 0.17), Vec3::new(0.29, 0.365, 0.31), WHITE);
    s.cylinder(Vec3::new(0.295, 0.39, -0.2), 0.025, 0.004, 12, Quat::from_rotation_z(FRAC_PI_2), WHITE);
    s.mat(Mat::Planks);
    s.aabb(Vec3::new(0.25, 0.16, -0.32), Vec3::new(0.3, 0.17, -0.12), Color::srgb(0.6, 0.45, 0.3));
    s.aabb(Vec3::new(0.295, 0.17, -0.32), Vec3::new(0.3, 0.21, -0.12), Color::srgb(0.6, 0.45, 0.3));
    s.mat(Mat::Light);
    s.sphere(Vec3::new(0.25, 0.41, 0.2), 0.012, Color::WHITE);
    s.sphere(Vec3::new(0.25, 0.41, -0.2), 0.012, Color::WHITE);
    s
}

fn road_mesh(mb: &mut Geo, edges: &[u8], y: f32) {
    // Concrete pavement over the whole tile, asphalt road on top.
    mb.mat(Mat::Concrete);
    mb.aabb(Vec3::new(-0.5, y, -0.5), Vec3::new(0.5, y + 0.014, 0.5), Color::srgb(0.85, 0.84, 0.82));
    mb.mat(Mat::Asphalt);
    let h = 0.02;
    mb.aabb(Vec3::new(-0.3, y, -0.3), Vec3::new(0.3, y + h, 0.3), TEX);
    for &e in edges {
        let (min, max) = match e {
            N => (Vec2::new(-0.3, -0.5), Vec2::new(0.3, -0.3)),
            E => (Vec2::new(0.3, -0.3), Vec2::new(0.5, 0.3)),
            S => (Vec2::new(-0.3, 0.3), Vec2::new(0.3, 0.5)),
            _ => (Vec2::new(-0.5, -0.3), Vec2::new(-0.3, 0.3)),
        };
        mb.aabb(Vec3::new(min.x, y, min.y), Vec3::new(max.x, y + h, max.y), TEX);
    }
    mb.mat(Mat::Paint);
    for &e in edges {
        let d = edge_dir(e).as_vec2();
        // Centre-line dashes.
        for k in [0.12f32, 0.38] {
            let c = d * k;
            let half = if d.x != 0.0 { Vec3::new(0.06, 0.002, 0.008) } else { Vec3::new(0.008, 0.002, 0.06) };
            mb.cuboid(Vec3::new(c.x, y + h, c.y), half, Quat::IDENTITY, Color::srgb(0.95, 0.85, 0.3));
        }
    }
    // Stop lines and zebra crossing at junctions.
    if edges.len() >= 3 {
        for &e in edges {
            let d = edge_dir(e).as_vec2();
            let side = Vec2::new(-d.y, d.x);
            for i in 0..5 {
                let c = d * 0.4 + side * (-0.24 + i as f32 * 0.12);
                let half = if d.x != 0.0 { Vec3::new(0.05, 0.002, 0.03) } else { Vec3::new(0.03, 0.002, 0.05) };
                mb.cuboid(Vec3::new(c.x, y + h, c.y), half, Quat::IDENTITY, WHITE);
            }
        }
    }
}

/// Gate arm for level crossings: a red/white bar along local +Z.
pub fn gate_arm_mesh() -> Geo {
    let mut m = Geo::new();
    m.mat(Mat::Paint);
    for i in 0..7 {
        let c = if i % 2 == 0 { Color::srgb(0.85, 0.08, 0.08) } else { WHITE };
        let z0 = i as f32 * 0.1;
        m.aabb(Vec3::new(-0.01, -0.012, z0), Vec3::new(0.01, 0.012, z0 + 0.1), c);
    }
    m.mat(Mat::Steel);
    m.aabb(Vec3::new(-0.02, -0.02, -0.06), Vec3::new(0.02, 0.02, 0.0), DARK);
    m
}

fn hash(t: IVec2, k: u32) -> f32 {
    let mut h = (t.x as u32).wrapping_mul(73856093) ^ (t.y as u32).wrapping_mul(19349663) ^ k.wrapping_mul(83492791);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^= h >> 15;
    (h % 10000) as f32 / 10000.0
}

const PLASTER: [Color; 6] = [
    Color::srgb(1.0, 0.97, 0.88),
    Color::srgb(0.98, 0.78, 0.65),
    Color::srgb(0.75, 0.85, 0.95),
    Color::srgb(1.0, 0.9, 0.6),
    Color::srgb(0.82, 0.92, 0.75),
    Color::srgb(0.95, 0.95, 0.95),
];
const DOORS: [Color; 4] = [
    Color::srgb(0.45, 0.12, 0.1),
    Color::srgb(0.1, 0.25, 0.4),
    Color::srgb(0.15, 0.35, 0.2),
    Color::srgb(0.4, 0.28, 0.18),
];

/// A framed window on a wall facing `dir` (an axis direction), centred at `c`.
fn window(m: &mut Geo, c: Vec3, dir: Vec3, w: f32, h: f32, frame: Color) {
    let side = Vec3::Y.cross(dir).abs();
    let ext = |a: f32, b: f32, t: f32| side * a + Vec3::Y * b + dir.abs() * t;
    let old = m.mat(Mat::Paint);
    m.cuboid(c + dir * 0.004, ext(w * 0.5 + 0.01, h * 0.5 + 0.01, 0.004), Quat::IDENTITY, frame);
    m.mat(Mat::Glass);
    m.cuboid(c + dir * 0.009, ext(w * 0.5, h * 0.5, 0.002), Quat::IDENTITY, GLASS);
    m.mat(Mat::Paint);
    m.cuboid(c + dir * 0.012, ext(0.003, h * 0.5, 0.002), Quat::IDENTITY, frame);
    m.cuboid(c + dir * 0.012, ext(w * 0.5, 0.003, 0.002), Quat::IDENTITY, frame);
    m.cuboid(c + dir * 0.012 - Vec3::Y * (h * 0.5 + 0.01), ext(w * 0.5 + 0.018, 0.005, 0.01), Quat::IDENTITY, frame);
    m.mat(old);
}

/// Rows of windows on the front and back (±Z) of a box, and optionally the sides.
fn window_grid(m: &mut Geo, min: Vec3, max: Vec3, rows: u32, cols: u32, sides: bool, frame: Color) {
    let size = max - min;
    for r in 0..rows {
        let y = min.y + size.y * (r as f32 + 0.55) / rows as f32;
        let wh = (size.y / rows as f32 * 0.45).min(0.12);
        for c in 0..cols {
            let x = min.x + size.x * (c as f32 + 0.5) / cols as f32;
            let ww = (size.x / cols as f32 * 0.45).min(0.12);
            window(m, Vec3::new(x, y, max.z), Vec3::Z, ww, wh, frame);
            window(m, Vec3::new(x, y, min.z), Vec3::NEG_Z, ww, wh, frame);
        }
        if sides {
            let scols = ((size.z / size.x) * cols as f32).round().max(1.0) as u32;
            for c in 0..scols {
                let z = min.z + size.z * (c as f32 + 0.5) / scols as f32;
                let ww = (size.z / scols as f32 * 0.45).min(0.12);
                window(m, Vec3::new(max.x, y, z), Vec3::X, ww, wh, frame);
                window(m, Vec3::new(min.x, y, z), Vec3::NEG_X, ww, wh, frame);
            }
        }
    }
}

/// A door on the +Z face at x, with a frame and a step.
fn door(m: &mut Geo, x: f32, z: f32, w: f32, h: f32, color: Color) {
    let old = m.mat(Mat::Paint);
    m.aabb(Vec3::new(x - w * 0.5 - 0.01, 0.0, z), Vec3::new(x + w * 0.5 + 0.01, h + 0.01, z + 0.004), TRIM);
    m.aabb(Vec3::new(x - w * 0.5, 0.0, z + 0.004), Vec3::new(x + w * 0.5, h, z + 0.007), color);
    m.mat(Mat::Steel);
    m.cuboid(Vec3::new(x + w * 0.3, h * 0.5, z + 0.01), Vec3::splat(0.004), Quat::IDENTITY, Color::srgb(0.8, 0.7, 0.3));
    m.mat(Mat::Concrete);
    m.aabb(Vec3::new(x - w * 0.6, 0.0, z), Vec3::new(x + w * 0.6, 0.012, z + 0.04), TEX);
    m.mat(old);
}

/// A flat sign panel on the +Z face with simple white bars standing in for lettering.
fn sign(m: &mut Geo, c: Vec3, w: f32, h: f32, bg: Color, letters: u32) {
    let old = m.mat(Mat::Paint);
    m.aabb(c - Vec3::new(w * 0.5, h * 0.5, 0.0), c + Vec3::new(w * 0.5, h * 0.5, 0.008), bg);
    for i in 0..letters {
        let x = c.x - w * 0.4 + w * 0.8 * (i as f32 + 0.5) / letters as f32;
        let lw = w * 0.8 / letters as f32 * 0.3;
        m.aabb(Vec3::new(x - lw, c.y - h * 0.28, c.z + 0.008), Vec3::new(x + lw, c.y + h * 0.28, c.z + 0.011), WHITE);
    }
    m.mat(old);
}

/// A red cross (hospitals, ambulances) on the +Z face, or facing up when `up`.
fn red_cross(m: &mut Geo, c: Vec3, s: f32, up: bool) {
    let red = Color::srgb(0.85, 0.08, 0.08);
    let old = m.mat(Mat::Paint);
    if up {
        m.aabb(c - Vec3::new(s * 0.5, 0.0, s * 0.17), c + Vec3::new(s * 0.5, 0.004, s * 0.17), red);
        m.aabb(c - Vec3::new(s * 0.17, 0.0, s * 0.5), c + Vec3::new(s * 0.17, 0.004, s * 0.5), red);
    } else {
        m.aabb(c - Vec3::new(s * 0.5, s * 0.17, 0.0), c + Vec3::new(s * 0.5, s * 0.17, 0.004), red);
        m.aabb(c - Vec3::new(s * 0.17, s * 0.5, 0.0), c + Vec3::new(s * 0.17, s * 0.5, 0.004), red);
    }
    m.mat(old);
}

fn tree_trunk(m: &mut Geo, h: f32, r: f32) {
    let old = m.mat(Mat::Plain);
    m.frustum(Vec3::new(0.0, h * 0.5, 0.0), r, r * 0.6, h * 0.5, 7, Quat::IDENTITY, Color::srgb(0.33, 0.24, 0.16));
    m.mat(old);
}

/// Decoration models, origin at ground level, base orientation "front faces +Z".
pub fn decor_mesh(kind: DecorKind, variant: u8, t: IVec2) -> Geo {
    let mut m = Geo::new();
    let v = variant as usize;
    let plaster = PLASTER[v % PLASTER.len()];
    let door_col = DOORS[v % DOORS.len()];
    let leaf = Color::srgb(0.55 + hash(t, 1) * 0.15, 0.7 + hash(t, 2) * 0.2, 0.45);
    match kind {
        DecorKind::House => {
            let brick = v % 3 == 0;
            m.mat(if brick { Mat::Brick } else { Mat::Plaster });
            let wall = if brick { TEX } else { plaster };
            m.aabb(Vec3::new(-0.32, 0.0, -0.25), Vec3::new(0.32, 0.38, 0.25), wall);
            m.mat(Mat::Concrete);
            m.aabb(Vec3::new(-0.33, 0.0, -0.26), Vec3::new(0.33, 0.035, 0.26), Color::srgb(0.7, 0.7, 0.7));
            m.mat(if v % 2 == 0 { Mat::RoofClay } else { Mat::RoofSlate });
            m.roof(Vec3::new(0.0, 0.38, 0.0), 0.74, 0.6, 0.25, Quat::IDENTITY, TEX, wall, if brick { Mat::Brick } else { Mat::Plaster });
            m.mat(Mat::Brick);
            m.aabb(Vec3::new(0.15, 0.42, -0.12), Vec3::new(0.23, 0.72, -0.04), TEX);
            m.aabb(Vec3::new(0.14, 0.72, -0.13), Vec3::new(0.24, 0.74, -0.03), Color::srgb(0.7, 0.7, 0.7));
            door(&mut m, -0.12, 0.25, 0.08, 0.17, door_col);
            for (x, y) in [(0.1f32, 0.12f32), (0.22, 0.12), (-0.18, 0.29), (0.0, 0.29), (0.18, 0.29)] {
                window(&mut m, Vec3::new(x, y, 0.25), Vec3::Z, 0.07, 0.07, TRIM);
                window(&mut m, Vec3::new(x, y, -0.25), Vec3::NEG_Z, 0.07, 0.07, TRIM);
            }
            window(&mut m, Vec3::new(0.32, 0.2, 0.0), Vec3::X, 0.08, 0.08, TRIM);
            window(&mut m, Vec3::new(-0.32, 0.2, 0.0), Vec3::NEG_X, 0.08, 0.08, TRIM);
            // Front garden hedge.
            m.mat(Mat::Foliage);
            m.aabb(Vec3::new(0.02, 0.0, 0.33), Vec3::new(0.32, 0.05, 0.38), Color::srgb(0.5, 0.7, 0.45));
        }
        DecorKind::Cottage => {
            m.mat(Mat::Plaster);
            m.aabb(Vec3::new(-0.25, 0.0, -0.2), Vec3::new(0.25, 0.26, 0.2), Color::srgb(1.0, 0.98, 0.92));
            m.mat(Mat::RoofClay);
            m.roof(Vec3::new(0.0, 0.26, 0.0), 0.6, 0.52, 0.24, Quat::IDENTITY, Color::srgb(1.0, 0.85, 0.55), Color::srgb(1.0, 0.98, 0.92), Mat::Plaster);
            door(&mut m, 0.0, 0.2, 0.07, 0.15, Color::srgb(0.15, 0.35, 0.2));
            for x in [-0.15f32, 0.15] {
                window(&mut m, Vec3::new(x, 0.15, 0.2), Vec3::Z, 0.07, 0.07, TRIM);
                window(&mut m, Vec3::new(x, 0.15, -0.2), Vec3::NEG_Z, 0.07, 0.07, TRIM);
                m.mat(Mat::Paint);
                for d in [-0.055f32, 0.055] {
                    m.aabb(Vec3::new(x + d - 0.012, 0.11, 0.2), Vec3::new(x + d + 0.012, 0.19, 0.208), Color::srgb(0.15, 0.4, 0.25));
                }
            }
            m.mat(Mat::Stone);
            m.aabb(Vec3::new(-0.27, 0.0, -0.08), Vec3::new(-0.2, 0.55, 0.02), TEX);
            m.mat(Mat::Plain);
            for (i, x) in [-0.2f32, -0.12, 0.12, 0.2].iter().enumerate() {
                let c = [Color::srgb(0.9, 0.2, 0.3), Color::srgb(1.0, 0.8, 0.1), Color::srgb(0.7, 0.3, 0.9), WHITE][i];
                m.sphere(Vec3::new(*x, 0.03, 0.26), 0.03, c);
            }
        }
        DecorKind::Shop => {
            m.mat(Mat::Brick);
            m.aabb(Vec3::new(-0.4, 0.0, -0.3), Vec3::new(0.4, 0.46, 0.3), TEX);
            m.mat(Mat::Concrete);
            m.aabb(Vec3::new(-0.42, 0.46, -0.32), Vec3::new(0.42, 0.49, 0.32), Color::srgb(0.8, 0.8, 0.8));
            m.aabb(Vec3::new(-0.42, 0.0, 0.3), Vec3::new(0.42, 0.015, 0.4), Color::srgb(0.8, 0.8, 0.8));
            // Shop front: big windows, door, awning, sign.
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-0.38, 0.0, 0.3), Vec3::new(0.38, 0.26, 0.305), plaster.mix(&DARK, 0.6));
            m.mat(Mat::Glass);
            m.aabb(Vec3::new(-0.35, 0.05, 0.305), Vec3::new(-0.08, 0.23, 0.308), GLASS);
            m.aabb(Vec3::new(0.08, 0.05, 0.305), Vec3::new(0.35, 0.23, 0.308), GLASS);
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-0.05, 0.0, 0.305), Vec3::new(0.05, 0.2, 0.308), door_col);
            let stripe = [Color::srgb(0.85, 0.15, 0.15), Color::srgb(0.15, 0.45, 0.25), Color::srgb(0.15, 0.3, 0.7)][v % 3];
            for i in 0..8 {
                let c = if i % 2 == 0 { stripe } else { WHITE };
                let x0 = -0.4 + i as f32 * 0.1;
                m.cuboid(Vec3::new(x0 + 0.05, 0.29, 0.37), Vec3::new(0.05, 0.005, 0.075), Quat::from_rotation_x(0.35), c);
            }
            sign(&mut m, Vec3::new(0.0, 0.38, 0.3), 0.5, 0.08, stripe, 5);
            window_grid(&mut m, Vec3::new(-0.4, 0.3, -0.3), Vec3::new(0.4, 0.46, -0.3), 1, 3, false, TRIM);
        }
        DecorKind::Apartment => {
            let h = 0.9 + (v % 3) as f32 * 0.3;
            let floors = (h / 0.22) as u32;
            m.mat(if v % 2 == 0 { Mat::Concrete } else { Mat::Brick });
            m.aabb(Vec3::new(-0.38, 0.0, -0.38), Vec3::new(0.38, h, 0.38), if v % 2 == 0 { plaster } else { TEX });
            m.mat(Mat::Concrete);
            m.aabb(Vec3::new(-0.4, h, -0.4), Vec3::new(0.4, h + 0.04, 0.4), Color::srgb(0.6, 0.6, 0.6));
            m.aabb(Vec3::new(-0.15, h + 0.04, -0.15), Vec3::new(0.1, h + 0.14, 0.1), Color::srgb(0.7, 0.7, 0.7));
            window_grid(&mut m, Vec3::new(-0.38, 0.22, -0.38), Vec3::new(0.38, h, 0.38), floors - 1, 4, true, TRIM);
            // Balconies on the front.
            m.mat(Mat::Concrete);
            for f in 1..floors {
                let y = f as f32 * h / floors as f32 + 0.02;
                m.aabb(Vec3::new(-0.3, y - 0.01, 0.38), Vec3::new(-0.08, y, 0.45), Color::srgb(0.8, 0.8, 0.8));
                m.mat(Mat::Steel);
                m.aabb(Vec3::new(-0.3, y, 0.445), Vec3::new(-0.08, y + 0.05, 0.45), DARK);
                m.mat(Mat::Concrete);
            }
            m.mat(Mat::Glass);
            m.aabb(Vec3::new(0.05, 0.0, 0.38), Vec3::new(0.2, 0.17, 0.385), GLASS);
            m.mat(Mat::Concrete);
            m.aabb(Vec3::new(0.0, 0.17, 0.38), Vec3::new(0.25, 0.19, 0.46), Color::srgb(0.75, 0.75, 0.75));
        }
        DecorKind::Church => {
            m.mat(Mat::Stone);
            m.aabb(Vec3::new(-0.2, 0.0, -0.42), Vec3::new(0.2, 0.42, 0.22), TEX);
            m.mat(Mat::RoofSlate);
            m.roof(Vec3::new(0.0, 0.42, -0.1), 0.68, 0.46, 0.24, Quat::from_rotation_y(FRAC_PI_2), TEX, TEX, Mat::Stone);
            m.mat(Mat::Stone);
            m.aabb(Vec3::new(-0.12, 0.0, 0.18), Vec3::new(0.12, 0.8, 0.42), TEX);
            m.mat(Mat::RoofSlate);
            m.frustum(Vec3::new(0.0, 1.0, 0.3), 0.17, 0.0, 0.2, 4, Quat::from_rotation_y(FRAC_PI_4), TEX);
            m.mat(Mat::Paint);
            let gold = Color::srgb(0.9, 0.75, 0.25);
            m.aabb(Vec3::new(-0.008, 1.18, 0.29), Vec3::new(0.008, 1.32, 0.31), gold);
            m.aabb(Vec3::new(-0.04, 1.26, 0.29), Vec3::new(0.04, 1.275, 0.31), gold);
            m.cylinder(Vec3::new(0.0, 0.62, 0.42), 0.05, 0.004, 12, Quat::from_rotation_x(FRAC_PI_2), WHITE);
            door(&mut m, 0.0, 0.42, 0.09, 0.2, Color::srgb(0.4, 0.22, 0.12));
            m.mat(Mat::Glass);
            for z in [-0.3f32, -0.15, 0.0, 0.12] {
                for x in [-0.202f32, 0.202] {
                    m.aabb(Vec3::new(x - 0.003, 0.12, z - 0.03), Vec3::new(x + 0.003, 0.32, z + 0.03), Color::srgb(0.5, 0.4, 0.7));
                }
            }
            m.cylinder(Vec3::new(0.0, 0.48, 0.425), 0.04, 0.004, 12, Quat::from_rotation_x(FRAC_PI_2), Color::srgb(0.6, 0.35, 0.6));
        }
        DecorKind::Barn => {
            let red = Color::srgb(0.85, 0.35, 0.3);
            m.mat(Mat::Planks);
            m.aabb(Vec3::new(-0.3, 0.0, -0.4), Vec3::new(0.3, 0.35, 0.4), red);
            m.mat(Mat::Corrugated);
            m.roof(Vec3::new(0.0, 0.35, 0.0), 0.88, 0.68, 0.25, Quat::from_rotation_y(FRAC_PI_2), Color::srgb(0.6, 0.6, 0.62), red, Mat::Planks);
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-0.15, 0.0, 0.4), Vec3::new(0.15, 0.27, 0.405), WHITE);
            m.aabb(Vec3::new(-0.13, 0.02, 0.405), Vec3::new(0.13, 0.25, 0.41), Color::srgb(0.6, 0.15, 0.12));
            for r in [0.72f32, -0.72] {
                m.cuboid(Vec3::new(0.0, 0.135, 0.412), Vec3::new(0.16, 0.008, 0.003), Quat::from_rotation_z(r), WHITE);
            }
            // Hay bales.
            m.mat(Mat::Plain);
            for x in [-0.36f32, -0.42] {
                m.cylinder(Vec3::new(x, 0.05, 0.3), 0.05, 0.05, 10, Quat::from_rotation_z(FRAC_PI_2), Color::srgb(0.85, 0.75, 0.4));
            }
        }
        DecorKind::Silo => {
            m.mat(Mat::Corrugated);
            m.cylinder(Vec3::new(0.0, 0.48, 0.0), 0.22, 0.48, 16, Quat::IDENTITY, Color::srgb(0.9, 0.9, 0.92));
            m.mat(Mat::Steel);
            m.sphere(Vec3::new(0.0, 0.96, 0.0), 0.22, Color::srgb(0.7, 0.72, 0.75));
            m.aabb(Vec3::new(-0.03, 0.0, 0.215), Vec3::new(0.03, 0.95, 0.235), DARK);
            m.mat(Mat::Concrete);
            m.cylinder(Vec3::new(0.0, 0.02, 0.0), 0.26, 0.02, 16, Quat::IDENTITY, TEX);
        }
        DecorKind::Windmill => {
            m.mat(Mat::Plaster);
            m.frustum(Vec3::new(0.0, 0.4, 0.0), 0.25, 0.15, 0.4, 10, Quat::IDENTITY, Color::srgb(1.0, 0.98, 0.92));
            m.mat(Mat::Planks);
            m.cone(Vec3::new(0.0, 0.8, 0.0), 0.2, 0.22, 10, Color::srgb(0.6, 0.45, 0.35));
            m.aabb(Vec3::new(-0.04, 0.0, 0.2), Vec3::new(0.04, 0.15, 0.24), Color::srgb(0.5, 0.35, 0.25));
            window(&mut m, Vec3::new(0.0, 0.45, 0.19), Vec3::Z, 0.05, 0.06, TRIM);
            m.mat(Mat::Planks);
            m.cylinder(Vec3::new(0.0, 0.75, 0.2), 0.02, 0.03, 8, Quat::from_rotation_x(FRAC_PI_2), DARK);
            for k in 0..4 {
                let q = Quat::from_rotation_z(k as f32 * FRAC_PI_2 + 0.3);
                // Lattice sails: a spar and a frame of slats.
                m.cuboid(Vec3::new(0.0, 0.75, 0.23) + q * Vec3::new(0.0, 0.2, 0.0), Vec3::new(0.008, 0.2, 0.006), q, Color::srgb(0.6, 0.45, 0.35));
                for s in 0..5 {
                    let y = 0.12 + s as f32 * 0.06;
                    m.cuboid(Vec3::new(0.0, 0.75, 0.235) + q * Vec3::new(0.04, y, 0.0), Vec3::new(0.04, 0.004, 0.003), q, Color::srgb(0.85, 0.82, 0.75));
                }
            }
        }
        DecorKind::PineTree => {
            let s = 0.8 + hash(t, 3) * 0.6;
            tree_trunk(&mut m, 0.2 * s, 0.035);
            m.mat(Mat::Foliage);
            for (i, r) in [0.3f32, 0.24, 0.17, 0.1].iter().enumerate() {
                let tint = Color::srgb(0.35, 0.5 + i as f32 * 0.04, 0.38);
                m.frustum(Vec3::new(0.0, (0.25 + i as f32 * 0.17) * s, 0.0), r * s, r * s * 0.25, 0.14 * s, 9, Quat::from_rotation_y(i as f32), tint);
            }
            m.cone(Vec3::new(0.0, 0.85 * s, 0.0), 0.05 * s, 0.15 * s, 7, Color::srgb(0.35, 0.55, 0.38));
        }
        DecorKind::OakTree => {
            let s = 0.8 + hash(t, 4) * 0.5;
            tree_trunk(&mut m, 0.38 * s, 0.05);
            m.mat(Mat::Plain);
            for r in [0.6f32, -0.7, 2.2] {
                let q = Quat::from_rotation_y(r) * Quat::from_rotation_z(0.6);
                m.cuboid(Vec3::new(0.0, 0.36 * s, 0.0) + q * Vec3::Y * 0.07, Vec3::new(0.015, 0.08, 0.015), q, Color::srgb(0.33, 0.24, 0.16));
            }
            m.mat(Mat::Foliage);
            for i in 0..7 {
                let a = i as f32 * 2.4 + hash(t, 20 + i) * 0.5;
                let r = if i == 0 { 0.0 } else { 0.16 };
                let p = Vec3::new(a.cos() * r, (0.48 + hash(t, 30 + i) * 0.14) * s, a.sin() * r) * Vec3::new(s, 1.0, s);
                m.sphere(p, (0.16 + hash(t, 40 + i) * 0.06) * s, leaf);
            }
        }
        DecorKind::Bush => {
            m.mat(Mat::Foliage);
            m.sphere(Vec3::new(0.0, 0.1, 0.0), 0.16, leaf);
            m.sphere(Vec3::new(0.15, 0.08, 0.1), 0.12, leaf);
            m.sphere(Vec3::new(-0.12, 0.07, 0.12), 0.1, leaf);
        }
        DecorKind::Flowers => {
            let cols = [Color::srgb(0.95, 0.2, 0.3), Color::srgb(1.0, 0.85, 0.1), Color::srgb(0.7, 0.3, 0.9), WHITE];
            for i in 0..18 {
                let x = hash(t, 10 + i) - 0.5;
                let z = hash(t, 40 + i) - 0.5;
                let p = Vec3::new(x * 0.8, 0.0, z * 0.8);
                m.mat(Mat::Plain);
                m.aabb(p + Vec3::new(-0.004, 0.0, -0.004), p + Vec3::new(0.004, 0.06, 0.004), Color::srgb(0.2, 0.5, 0.15));
                m.sphere(p + Vec3::Y * 0.07, 0.022, cols[i as usize % 4]);
            }
        }
        DecorKind::Rock => {
            m.mat(Mat::Rock);
            m.cuboid(Vec3::new(0.0, 0.1, 0.0), Vec3::new(0.2, 0.12, 0.16), Quat::from_euler(EulerRot::XYZ, 0.3, 0.5, 0.2), TEX);
            m.cuboid(Vec3::new(0.18, 0.06, 0.12), Vec3::new(0.1, 0.07, 0.09), Quat::from_euler(EulerRot::XYZ, 0.6, 0.2, 0.4), Color::srgb(0.9, 0.9, 0.9));
        }
        DecorKind::Fence => {
            m.mat(Mat::Planks);
            let wood = Color::srgb(0.75, 0.6, 0.45);
            for i in 0..5 {
                let x = -0.4 + i as f32 * 0.2;
                m.aabb(Vec3::new(x - 0.015, 0.0, 0.42), Vec3::new(x + 0.015, 0.16, 0.45), wood);
            }
            for y in [0.06f32, 0.12] {
                m.aabb(Vec3::new(-0.5, y, 0.425), Vec3::new(0.5, y + 0.02, 0.445), wood);
            }
        }
        DecorKind::Lamp => {
            m.mat(Mat::Steel);
            m.cylinder(Vec3::new(0.0, 0.25, 0.0), 0.012, 0.25, 8, Quat::IDENTITY, DARK);
            m.aabb(Vec3::new(-0.008, 0.48, 0.0), Vec3::new(0.008, 0.495, 0.12), DARK);
            m.aabb(Vec3::new(-0.03, 0.47, 0.09), Vec3::new(0.03, 0.49, 0.15), DARK);
            m.mat(Mat::Light);
            m.aabb(Vec3::new(-0.022, 0.462, 0.1), Vec3::new(0.022, 0.47, 0.14), Color::WHITE);
        }
        DecorKind::Bench => {
            m.mat(Mat::Planks);
            let wood = Color::srgb(0.7, 0.5, 0.35);
            m.aabb(Vec3::new(-0.2, 0.08, -0.05), Vec3::new(0.2, 0.095, 0.05), wood);
            m.aabb(Vec3::new(-0.2, 0.1, -0.06), Vec3::new(0.2, 0.18, -0.048), wood);
            m.mat(Mat::Steel);
            for x in [-0.17f32, 0.17] {
                m.aabb(Vec3::new(x - 0.01, 0.0, -0.05), Vec3::new(x + 0.01, 0.08, 0.05), DARK);
            }
        }
        DecorKind::Fountain => {
            m.mat(Mat::Stone);
            m.cylinder(Vec3::new(0.0, 0.05, 0.0), 0.38, 0.05, 16, Quat::IDENTITY, TEX);
            m.mat(Mat::Water);
            m.cylinder(Vec3::new(0.0, 0.1, 0.0), 0.33, 0.005, 16, Quat::IDENTITY, Color::srgb(0.3, 0.5, 0.7));
            m.mat(Mat::Stone);
            m.cylinder(Vec3::new(0.0, 0.2, 0.0), 0.04, 0.12, 10, Quat::IDENTITY, TEX);
            m.cylinder(Vec3::new(0.0, 0.3, 0.0), 0.12, 0.015, 12, Quat::IDENTITY, TEX);
            m.mat(Mat::Water);
            m.sphere(Vec3::new(0.0, 0.37, 0.0), 0.05, Color::srgb(0.7, 0.85, 1.0));
        }
        DecorKind::Person => person(&mut m, t, Vec3::ZERO),
        DecorKind::PoliceStation => {
            m.mat(Mat::Brick);
            m.aabb(Vec3::new(-0.42, 0.0, -0.32), Vec3::new(0.42, 0.52, 0.28), Color::srgb(0.85, 0.85, 0.9));
            m.mat(Mat::Concrete);
            m.aabb(Vec3::new(-0.44, 0.52, -0.34), Vec3::new(0.44, 0.56, 0.3), Color::srgb(0.7, 0.7, 0.7));
            m.aabb(Vec3::new(-0.44, 0.0, 0.28), Vec3::new(0.44, 0.012, 0.5), Color::srgb(0.75, 0.75, 0.75));
            // Blue band with POLICE lettering, glass entrance, blue lamp.
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-0.42, 0.24, 0.28), Vec3::new(0.42, 0.27, 0.285), Color::srgb(0.1, 0.2, 0.55));
            sign(&mut m, Vec3::new(0.0, 0.44, 0.28), 0.42, 0.08, Color::srgb(0.1, 0.2, 0.55), 6);
            m.mat(Mat::Glass);
            m.aabb(Vec3::new(-0.08, 0.0, 0.28), Vec3::new(0.08, 0.2, 0.285), GLASS);
            m.mat(Mat::Concrete);
            m.aabb(Vec3::new(-0.12, 0.2, 0.28), Vec3::new(0.12, 0.22, 0.38), Color::srgb(0.8, 0.8, 0.8));
            m.mat(Mat::FlashBlue);
            m.cuboid(Vec3::new(0.0, 0.25, 0.37), Vec3::new(0.025, 0.025, 0.025), Quat::IDENTITY, Color::WHITE);
            window_grid(&mut m, Vec3::new(-0.42, 0.28, -0.32), Vec3::new(0.42, 0.52, 0.28), 1, 5, true, TRIM);
            for x in [-0.32f32, -0.2, 0.2, 0.32] {
                window(&mut m, Vec3::new(x, 0.13, 0.28), Vec3::Z, 0.07, 0.08, TRIM);
            }
            // Flag pole.
            m.mat(Mat::Steel);
            m.cylinder(Vec3::new(-0.38, 0.4, 0.44), 0.006, 0.4, 6, Quat::IDENTITY, Color::srgb(0.8, 0.8, 0.8));
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-0.374, 0.68, 0.44), Vec3::new(-0.26, 0.76, 0.443), Color::srgb(0.15, 0.3, 0.75));
        }
        DecorKind::FireStation => {
            m.mat(Mat::Brick);
            m.aabb(Vec3::new(-0.42, 0.0, -0.3), Vec3::new(0.3, 0.42, 0.3), TEX);
            m.aabb(Vec3::new(0.3, 0.0, -0.3), Vec3::new(0.46, 0.85, -0.1), TEX);
            m.mat(Mat::RoofSlate);
            m.frustum(Vec3::new(0.38, 0.9, -0.2), 0.12, 0.02, 0.05, 4, Quat::from_rotation_y(FRAC_PI_4), TEX);
            m.mat(Mat::Concrete);
            m.aabb(Vec3::new(-0.44, 0.42, -0.32), Vec3::new(0.32, 0.46, 0.32), Color::srgb(0.7, 0.7, 0.7));
            m.aabb(Vec3::new(-0.42, 0.0, 0.3), Vec3::new(0.3, 0.012, 0.5), Color::srgb(0.75, 0.75, 0.75));
            // Two red engine-bay doors with window strips.
            for x in [-0.22f32, 0.1] {
                m.mat(Mat::Paint);
                m.aabb(Vec3::new(x - 0.15, 0.0, 0.3), Vec3::new(x + 0.15, 0.3, 0.303), WHITE);
                m.aabb(Vec3::new(x - 0.13, 0.0, 0.303), Vec3::new(x + 0.13, 0.28, 0.307), Color::srgb(0.8, 0.08, 0.06));
                for r in 0..5 {
                    let y = 0.04 + r as f32 * 0.05;
                    m.aabb(Vec3::new(x - 0.13, y, 0.307), Vec3::new(x + 0.13, y + 0.004, 0.309), Color::srgb(0.6, 0.05, 0.04));
                }
                m.mat(Mat::Glass);
                m.aabb(Vec3::new(x - 0.11, 0.2, 0.307), Vec3::new(x + 0.11, 0.24, 0.31), GLASS);
            }
            sign(&mut m, Vec3::new(-0.06, 0.36, 0.3), 0.5, 0.07, Color::srgb(0.8, 0.08, 0.06), 4);
            window_grid(&mut m, Vec3::new(0.3, 0.45, -0.3), Vec3::new(0.46, 0.85, -0.1), 2, 1, false, TRIM);
            m.mat(Mat::Paint);
            m.cylinder(Vec3::new(0.36, 0.06, 0.38), 0.025, 0.06, 8, Quat::IDENTITY, Color::srgb(0.85, 0.1, 0.08));
        }
        DecorKind::Hospital => {
            m.mat(Mat::Plaster);
            m.aabb(Vec3::new(-0.45, 0.0, -0.42), Vec3::new(0.45, 1.05, 0.3), Color::srgb(0.97, 0.97, 0.97));
            m.mat(Mat::Concrete);
            m.aabb(Vec3::new(-0.47, 1.05, -0.44), Vec3::new(0.47, 1.09, 0.32), Color::srgb(0.75, 0.75, 0.75));
            m.aabb(Vec3::new(-0.45, 0.0, 0.3), Vec3::new(0.45, 0.012, 0.5), Color::srgb(0.75, 0.75, 0.75));
            // Rows of windows with a blue band between floors.
            window_grid(&mut m, Vec3::new(-0.45, 0.25, -0.42), Vec3::new(0.45, 1.05, 0.3), 4, 6, true, Color::srgb(0.6, 0.75, 0.9));
            m.mat(Mat::Paint);
            for f in 0..4 {
                let y = 0.25 + f as f32 * 0.2;
                m.aabb(Vec3::new(-0.452, y - 0.005, -0.422), Vec3::new(0.452, y + 0.005, 0.302), Color::srgb(0.3, 0.55, 0.8));
            }
            // Entrance with canopy, sign with red cross, rooftop helipad.
            m.mat(Mat::Glass);
            m.aabb(Vec3::new(-0.12, 0.0, 0.3), Vec3::new(0.12, 0.18, 0.305), GLASS);
            m.mat(Mat::Concrete);
            m.aabb(Vec3::new(-0.2, 0.18, 0.3), Vec3::new(0.2, 0.2, 0.45), Color::srgb(0.9, 0.9, 0.9));
            m.mat(Mat::Steel);
            for x in [-0.18f32, 0.18] {
                m.aabb(Vec3::new(x - 0.008, 0.0, 0.43), Vec3::new(x + 0.008, 0.18, 0.446), Color::srgb(0.8, 0.8, 0.8));
            }
            m.mat(Mat::Paint);
            m.aabb(Vec3::new(-0.36, 0.13, 0.3), Vec3::new(-0.2, 0.22, 0.306), WHITE);
            red_cross(&mut m, Vec3::new(-0.28, 0.175, 0.306), 0.07, false);
            m.aabb(Vec3::new(-0.08, 0.96, 0.3), Vec3::new(0.08, 1.04, 0.306), WHITE);
            red_cross(&mut m, Vec3::new(0.0, 1.0, 0.306), 0.06, false);
            m.mat(Mat::Concrete);
            m.cylinder(Vec3::new(0.0, 1.1, -0.06), 0.3, 0.01, 20, Quat::IDENTITY, Color::srgb(0.5, 0.5, 0.52));
            m.mat(Mat::Paint);
            let y = 1.112;
            m.aabb(Vec3::new(-0.1, y, -0.16), Vec3::new(-0.06, y + 0.002, 0.04), WHITE);
            m.aabb(Vec3::new(0.06, y, -0.16), Vec3::new(0.1, y + 0.002, 0.04), WHITE);
            m.aabb(Vec3::new(-0.06, y, -0.08), Vec3::new(0.06, y + 0.002, -0.04), WHITE);
        }
    }
    m
}

/// A little person (used by the Person decoration).
fn person(m: &mut Geo, t: IVec2, at: Vec3) {
    let shirt = [Color::srgb(0.9, 0.2, 0.2), Color::srgb(0.2, 0.4, 0.9), Color::srgb(0.95, 0.8, 0.2), Color::srgb(0.3, 0.7, 0.3)][(hash(t, 7) * 4.0) as usize % 4];
    m.mat(Mat::Plain);
    for x in [-0.015f32, 0.015] {
        m.aabb(at + Vec3::new(x - 0.011, 0.0, -0.012), at + Vec3::new(x + 0.011, 0.08, 0.012), Color::srgb(0.2, 0.2, 0.35));
    }
    m.aabb(at + Vec3::new(-0.035, 0.08, -0.02), at + Vec3::new(0.035, 0.155, 0.02), shirt);
    for x in [-0.045f32, 0.045] {
        m.aabb(at + Vec3::new(x - 0.01, 0.09, -0.01), at + Vec3::new(x + 0.01, 0.15, 0.01), shirt);
    }
    m.sphere(at + Vec3::new(0.0, 0.185, 0.0), 0.028, Color::srgb(0.95, 0.78, 0.62));
    m.sphere(at + Vec3::new(0.0, 0.198, -0.004), 0.026, [Color::srgb(0.3, 0.2, 0.1), Color::srgb(0.9, 0.8, 0.4), DARK][(hash(t, 8) * 3.0) as usize % 3]);
}
