//! Meshes for things placed on tiles: track, roads, stations, bridges,
//! tunnel portals and decorations.

use crate::grid::*;
use crate::mesh::*;
use crate::track::*;
use bevy::prelude::*;

const BALLAST: Color = Color::srgb(0.55, 0.52, 0.47);
const TIE: Color = Color::srgb(0.36, 0.24, 0.15);
const RAIL: Color = Color::srgb(0.78, 0.78, 0.82);
const ASPHALT: Color = Color::srgb(0.25, 0.25, 0.27);
const STONE: Color = Color::srgb(0.6, 0.58, 0.55);
const WHITE: Color = Color::srgb(0.95, 0.95, 0.92);

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
        // On a switch, highlight the route that is currently set.
        let active = !tr.is_switch() || (pi == 1) == tr.diverge;
        let rail = if active { RAIL } else { Color::srgb(0.5, 0.48, 0.45) };
        for i in 0..n {
            let (p0, _) = pts[i];
            let (p1, _) = pts[i + 1];
            let mid = (p0 + p1) * 0.5;
            let dir = p1 - p0;
            let len = dir.length();
            let q = basis_from_dir(dir);
            if pi == 0 || tr.kind != TrackKind::Crossing {
                mb.cuboid(mid + Vec3::Y * 0.01, Vec3::new(0.19, 0.012, len * 0.5 + 0.01), q, BALLAST);
            }
            mb.cuboid(mid + Vec3::Y * 0.03, Vec3::new(0.15, 0.008, 0.025), q, TIE);
            for side in [-1.0f32, 1.0] {
                let off = q * Vec3::X * 0.09 * side;
                mb.cuboid(mid + off + Vec3::Y * 0.048, Vec3::new(0.01, 0.012, len * 0.5 + 0.004), q, rail);
            }
            if elevated {
                // Bridge deck and side girders.
                mb.cuboid(mid - Vec3::Y * 0.02, Vec3::new(0.24, 0.02, len * 0.5 + 0.01), q, Color::srgb(0.35, 0.3, 0.28));
                for side in [-1.0f32, 1.0] {
                    let off = q * Vec3::X * 0.23 * side;
                    mb.cuboid(mid + off + Vec3::Y * 0.06, Vec3::new(0.012, 0.05, len * 0.5 + 0.01), q, Color::srgb(0.55, 0.15, 0.12));
                }
            }
        }
        if elevated && pi == 0 {
            let (pm, _) = pts[n / 2];
            let top = pm.y - 0.04;
            if top > terrain_y {
                let h = (top - terrain_y) * 0.5;
                mb.cuboid(Vec3::new(pm.x, terrain_y + h, pm.z), Vec3::new(0.09, h, 0.09), Quat::IDENTITY, STONE);
                mb.cuboid(Vec3::new(pm.x, top - 0.02, pm.z), Vec3::new(0.2, 0.025, 0.1), basis_from_dir(pts[n / 2].1.with_y(0.0)), STONE);
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
            s.aabb(Vec3::new(0.3 * side - 0.03, 0.0, 0.3), Vec3::new(0.3 * side + 0.03, 0.18, 0.36), Color::srgb(0.2, 0.2, 0.2));
            let c = if tr.diverge { Color::srgb(1.0, 0.55, 0.0) } else { Color::srgb(0.1, 0.85, 0.2) };
            s.cuboid(Vec3::new(0.3 * side, 0.22, 0.33), Vec3::new(0.06, 0.05, 0.015), Quat::IDENTITY, c);
            mb.append(s, Transform::from_xyz(0.0, y0, 0.0).with_rotation(rq));
        }
        TrackKind::Station => {
            mb.append(station_mesh(), Transform::from_xyz(0.0, y0, 0.0).with_rotation(rq));
        }
        TrackKind::LevelCrossing => {
            let mut s = Geo::new();
            s.aabb(Vec3::new(-0.5, 0.0, -0.3), Vec3::new(-0.19, 0.03, 0.3), ASPHALT);
            s.aabb(Vec3::new(0.19, 0.0, -0.3), Vec3::new(0.5, 0.03, 0.3), ASPHALT);
            s.aabb(Vec3::new(-0.19, 0.0, -0.3), Vec3::new(0.19, 0.05, 0.3), Color::srgb(0.42, 0.3, 0.2));
            for (x, z) in [(0.32f32, 0.38f32), (-0.32, -0.38)] {
                s.aabb(Vec3::new(x - 0.02, 0.0, z - 0.02), Vec3::new(x + 0.02, 0.22, z + 0.02), WHITE);
                s.cuboid(Vec3::new(x, 0.25, z), Vec3::new(0.05, 0.03, 0.01), Quat::IDENTITY, Color::srgb(0.9, 0.1, 0.1));
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
            p.aabb(Vec3::new(-0.3, yl, fz - 0.01), Vec3::new(0.3, yl + 0.46, fz), Color::srgb(0.03, 0.03, 0.03));
            p.aabb(Vec3::new(-0.38, yl, fz), Vec3::new(-0.26, top, fz + 0.06), STONE);
            p.aabb(Vec3::new(0.26, yl, fz), Vec3::new(0.38, top, fz + 0.06), STONE);
            p.aabb(Vec3::new(-0.38, yl + 0.46, fz), Vec3::new(0.38, top.max(yl + 0.56), fz + 0.06), STONE);
            // Base orientation faces +Z (south); rotate to face edge e.
            let rot = Quat::from_rotation_y(-std::f32::consts::FRAC_PI_2 * ((e + 2) % 4) as f32);
            mb.append(p, Transform::from_rotation(rot));
        }
    }
}

fn station_mesh() -> Geo {
    let mut s = Geo::new();
    // Platform on the +X side.
    s.aabb(Vec3::new(0.2, 0.0, -0.5), Vec3::new(0.5, 0.12, 0.5), Color::srgb(0.75, 0.72, 0.66));
    s.aabb(Vec3::new(0.2, 0.12, -0.5), Vec3::new(0.23, 0.125, 0.5), Color::srgb(0.95, 0.85, 0.1));
    // Little ticket office.
    s.aabb(Vec3::new(0.36, 0.12, -0.22), Vec3::new(0.5, 0.36, 0.22), Color::srgb(0.85, 0.75, 0.55));
    s.roof(Vec3::new(0.43, 0.36, 0.0), 0.5, 0.2, 0.1, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), Color::srgb(0.55, 0.2, 0.15), Color::srgb(0.85, 0.75, 0.55));
    s.aabb(Vec3::new(0.355, 0.12, -0.05), Vec3::new(0.36, 0.28, 0.05), Color::srgb(0.35, 0.2, 0.1));
    s.aabb(Vec3::new(0.355, 0.2, 0.1), Vec3::new(0.36, 0.28, 0.18), Color::srgb(0.3, 0.5, 0.75));
    s.aabb(Vec3::new(0.355, 0.2, -0.18), Vec3::new(0.36, 0.28, -0.1), Color::srgb(0.3, 0.5, 0.75));
    // Canopy over the platform edge.
    for z in [-0.4f32, 0.4] {
        s.aabb(Vec3::new(0.28, 0.12, z - 0.015), Vec3::new(0.31, 0.42, z + 0.015), Color::srgb(0.2, 0.3, 0.2));
    }
    s.aabb(Vec3::new(0.22, 0.42, -0.48), Vec3::new(0.38, 0.44, 0.48), Color::srgb(0.25, 0.4, 0.28));
    // Sign and bench.
    s.aabb(Vec3::new(0.29, 0.34, 0.15), Vec3::new(0.3, 0.4, 0.32), Color::srgb(0.1, 0.2, 0.6));
    s.aabb(Vec3::new(0.27, 0.16, -0.32), Vec3::new(0.32, 0.18, -0.12), TIE);
    s
}

fn road_mesh(mb: &mut Geo, edges: &[u8], y: f32) {
    let h = 0.02;
    mb.aabb(Vec3::new(-0.3, y, -0.3), Vec3::new(0.3, y + h, 0.3), ASPHALT);
    for &e in edges {
        let d = edge_dir(e).as_vec2();
        let (min, max) = match e {
            N => (Vec2::new(-0.3, -0.5), Vec2::new(0.3, -0.3)),
            E => (Vec2::new(0.3, -0.3), Vec2::new(0.5, 0.3)),
            S => (Vec2::new(-0.3, 0.3), Vec2::new(0.3, 0.5)),
            _ => (Vec2::new(-0.5, -0.3), Vec2::new(-0.3, 0.3)),
        };
        mb.aabb(Vec3::new(min.x, y, min.y), Vec3::new(max.x, y + h, max.y), ASPHALT);
        // Centre-line dashes.
        for k in [0.1f32, 0.35] {
            let c = d * k;
            let half = if d.x != 0.0 { Vec3::new(0.06, 0.003, 0.012) } else { Vec3::new(0.012, 0.003, 0.06) };
            mb.cuboid(Vec3::new(c.x, y + h, c.y), half, Quat::IDENTITY, Color::srgb(0.95, 0.85, 0.2));
        }
    }
    if edges.is_empty() {
        mb.cuboid(Vec3::new(0.0, y + h, 0.0), Vec3::new(0.05, 0.003, 0.05), Quat::IDENTITY, WHITE);
    }
}

/// Gate arm for level crossings: a red/white bar along local +Z.
pub fn gate_arm_mesh() -> Geo {
    let mut m = Geo::new();
    for i in 0..7 {
        let c = if i % 2 == 0 { Color::srgb(0.9, 0.1, 0.1) } else { WHITE };
        let z0 = i as f32 * 0.1;
        m.aabb(Vec3::new(-0.012, -0.012, z0), Vec3::new(0.012, 0.012, z0 + 0.1), c);
    }
    m
}

fn hash(t: IVec2, k: u32) -> f32 {
    let mut h = (t.x as u32).wrapping_mul(73856093) ^ (t.y as u32).wrapping_mul(19349663) ^ k.wrapping_mul(83492791);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^= h >> 15;
    (h % 10000) as f32 / 10000.0
}

const WALLS: [Color; 6] = [
    Color::srgb(0.95, 0.9, 0.75),
    Color::srgb(0.85, 0.45, 0.35),
    Color::srgb(0.6, 0.75, 0.9),
    Color::srgb(0.95, 0.85, 0.4),
    Color::srgb(0.75, 0.85, 0.65),
    Color::srgb(0.9, 0.9, 0.9),
];
const ROOFS: [Color; 4] = [
    Color::srgb(0.65, 0.2, 0.15),
    Color::srgb(0.3, 0.3, 0.35),
    Color::srgb(0.45, 0.28, 0.18),
    Color::srgb(0.2, 0.35, 0.5),
];
const GLASS: Color = Color::srgb(0.25, 0.4, 0.6);

fn windows_on_box(m: &mut Geo, min: Vec3, max: Vec3, rows: u32, cols: u32) {
    let w = max.x - min.x;
    let h = max.y - min.y;
    for r in 0..rows {
        for c in 0..cols {
            let cx = min.x + w * (c as f32 + 0.5) / cols as f32;
            let cy = min.y + h * (r as f32 + 0.55) / rows as f32;
            let hw = w / cols as f32 * 0.22;
            let hh = h / rows as f32 * 0.22;
            for z in [min.z - 0.004, max.z + 0.004] {
                m.cuboid(Vec3::new(cx, cy, z), Vec3::new(hw, hh, 0.004), Quat::IDENTITY, GLASS);
            }
        }
    }
}

/// Decoration models, origin at ground level, base orientation "front faces +Z".
pub fn decor_mesh(kind: DecorKind, variant: u8, t: IVec2) -> Geo {
    let mut m = Geo::new();
    let v = variant as usize;
    let wall = WALLS[v % WALLS.len()];
    let roof = ROOFS[v % ROOFS.len()];
    let green = Color::srgb(0.2 + hash(t, 1) * 0.1, 0.45 + hash(t, 2) * 0.15, 0.18);
    match kind {
        DecorKind::House => {
            m.aabb(Vec3::new(-0.32, 0.0, -0.25), Vec3::new(0.32, 0.38, 0.25), wall);
            m.roof(Vec3::new(0.0, 0.38, 0.0), 0.72, 0.58, 0.25, Quat::IDENTITY, roof, wall);
            m.aabb(Vec3::new(0.15, 0.4, -0.1), Vec3::new(0.23, 0.68, -0.02), Color::srgb(0.5, 0.3, 0.25));
            m.aabb(Vec3::new(-0.06, 0.0, 0.25), Vec3::new(0.06, 0.2, 0.26), Color::srgb(0.4, 0.22, 0.12));
            windows_on_box(&mut m, Vec3::new(-0.32, 0.05, -0.25), Vec3::new(0.32, 0.38, 0.25), 2, 3);
        }
        DecorKind::Cottage => {
            m.aabb(Vec3::new(-0.25, 0.0, -0.2), Vec3::new(0.25, 0.26, 0.2), WHITE);
            m.roof(Vec3::new(0.0, 0.26, 0.0), 0.58, 0.5, 0.22, Quat::IDENTITY, Color::srgb(0.75, 0.6, 0.3), WHITE);
            m.aabb(Vec3::new(-0.05, 0.0, 0.2), Vec3::new(0.05, 0.16, 0.21), Color::srgb(0.2, 0.4, 0.2));
            windows_on_box(&mut m, Vec3::new(-0.25, 0.04, -0.2), Vec3::new(0.25, 0.26, 0.2), 1, 3);
        }
        DecorKind::Shop => {
            m.aabb(Vec3::new(-0.4, 0.0, -0.3), Vec3::new(0.4, 0.42, 0.3), wall);
            m.aabb(Vec3::new(-0.42, 0.42, -0.32), Vec3::new(0.42, 0.46, 0.32), Color::srgb(0.4, 0.4, 0.42));
            m.aabb(Vec3::new(-0.3, 0.04, 0.3), Vec3::new(0.3, 0.22, 0.305), GLASS);
            for i in 0..8 {
                let c = if i % 2 == 0 { Color::srgb(0.85, 0.15, 0.15) } else { WHITE };
                let x0 = -0.4 + i as f32 * 0.1;
                m.cuboid(Vec3::new(x0 + 0.05, 0.26, 0.36), Vec3::new(0.05, 0.008, 0.07), Quat::from_rotation_x(0.35), c);
            }
            m.aabb(Vec3::new(-0.25, 0.3, 0.3), Vec3::new(0.25, 0.38, 0.31), Color::srgb(0.1, 0.2, 0.5));
        }
        DecorKind::Apartment => {
            let h = 0.9 + (v % 3) as f32 * 0.3;
            m.aabb(Vec3::new(-0.38, 0.0, -0.38), Vec3::new(0.38, h, 0.38), wall);
            m.aabb(Vec3::new(-0.4, h, -0.4), Vec3::new(0.4, h + 0.04, 0.4), Color::srgb(0.35, 0.35, 0.38));
            windows_on_box(&mut m, Vec3::new(-0.38, 0.05, -0.38), Vec3::new(0.38, h, 0.38), (h / 0.22) as u32, 4);
            let rotq = Quat::from_rotation_y(std::f32::consts::FRAC_PI_2);
            let mut side = Geo::new();
            windows_on_box(&mut side, Vec3::new(-0.38, 0.05, -0.38), Vec3::new(0.38, h, 0.38), (h / 0.22) as u32, 4);
            m.append(side, Transform::from_rotation(rotq));
        }
        DecorKind::Church => {
            m.aabb(Vec3::new(-0.2, 0.0, -0.4), Vec3::new(0.2, 0.4, 0.25), Color::srgb(0.85, 0.83, 0.78));
            m.roof(Vec3::new(0.0, 0.4, -0.075), 0.65, 0.46, 0.22, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), Color::srgb(0.35, 0.35, 0.4), Color::srgb(0.85, 0.83, 0.78));
            m.aabb(Vec3::new(-0.12, 0.0, 0.2), Vec3::new(0.12, 0.75, 0.42), Color::srgb(0.85, 0.83, 0.78));
            m.frustum(Vec3::new(0.0, 0.95, 0.31), 0.15, 0.0, 0.2, 4, Quat::from_rotation_y(std::f32::consts::FRAC_PI_4), Color::srgb(0.35, 0.35, 0.4));
            m.aabb(Vec3::new(-0.05, 0.0, 0.42), Vec3::new(0.05, 0.18, 0.425), Color::srgb(0.4, 0.22, 0.12));
            m.aabb(Vec3::new(-0.01, 1.15, 0.3), Vec3::new(0.01, 1.28, 0.32), Color::srgb(0.9, 0.8, 0.2));
            m.aabb(Vec3::new(-0.04, 1.22, 0.3), Vec3::new(0.04, 1.24, 0.32), Color::srgb(0.9, 0.8, 0.2));
        }
        DecorKind::Barn => {
            let red = Color::srgb(0.7, 0.15, 0.12);
            m.aabb(Vec3::new(-0.3, 0.0, -0.4), Vec3::new(0.3, 0.35, 0.4), red);
            m.roof(Vec3::new(0.0, 0.35, 0.0), 0.86, 0.66, 0.25, Quat::from_rotation_y(std::f32::consts::FRAC_PI_2), Color::srgb(0.35, 0.35, 0.38), red);
            m.aabb(Vec3::new(-0.14, 0.0, 0.4), Vec3::new(0.14, 0.26, 0.405), WHITE);
            m.aabb(Vec3::new(-0.12, 0.02, 0.405), Vec3::new(0.12, 0.24, 0.41), red);
        }
        DecorKind::Silo => {
            let c = Color::srgb(0.75, 0.78, 0.8);
            m.cylinder(Vec3::new(0.0, 0.45, 0.0), 0.22, 0.45, 12, Quat::IDENTITY, c);
            m.sphere(Vec3::new(0.0, 0.9, 0.0), 0.22, Color::srgb(0.55, 0.6, 0.65));
        }
        DecorKind::Windmill => {
            m.frustum(Vec3::new(0.0, 0.4, 0.0), 0.25, 0.15, 0.4, 8, Quat::IDENTITY, Color::srgb(0.9, 0.88, 0.8));
            m.cone(Vec3::new(0.0, 0.8, 0.0), 0.2, 0.2, 8, Color::srgb(0.45, 0.25, 0.15));
            m.aabb(Vec3::new(-0.04, 0.0, 0.2), Vec3::new(0.04, 0.15, 0.24), Color::srgb(0.4, 0.22, 0.12));
            for k in 0..4 {
                let q = Quat::from_rotation_z(k as f32 * std::f32::consts::FRAC_PI_2 + 0.3);
                m.cuboid(Vec3::new(0.0, 0.75, 0.2) + q * Vec3::new(0.0, 0.3, 0.0), Vec3::new(0.06, 0.28, 0.01), q, Color::srgb(0.85, 0.8, 0.7));
            }
        }
        DecorKind::PineTree => {
            let s = 0.8 + hash(t, 3) * 0.5;
            m.cylinder(Vec3::new(0.0, 0.1 * s, 0.0), 0.04, 0.1 * s, 6, Quat::IDENTITY, Color::srgb(0.4, 0.25, 0.12));
            for (i, r) in [0.32f32, 0.25, 0.17].iter().enumerate() {
                m.cone(Vec3::new(0.0, (0.15 + i as f32 * 0.2) * s, 0.0), r * s, 0.35 * s, 8, Color::srgb(0.1, 0.35 + i as f32 * 0.04, 0.18));
            }
        }
        DecorKind::OakTree => {
            let s = 0.8 + hash(t, 4) * 0.5;
            m.cylinder(Vec3::new(0.0, 0.18 * s, 0.0), 0.05, 0.18 * s, 6, Quat::IDENTITY, Color::srgb(0.4, 0.25, 0.12));
            m.sphere(Vec3::new(0.0, 0.5 * s, 0.0), 0.28 * s, green);
            m.sphere(Vec3::new(0.15 * s, 0.42 * s, 0.08), 0.2 * s, green);
            m.sphere(Vec3::new(-0.12 * s, 0.45 * s, -0.1), 0.2 * s, green);
        }
        DecorKind::Bush => {
            m.sphere(Vec3::new(0.0, 0.1, 0.0), 0.16, green);
            m.sphere(Vec3::new(0.15, 0.08, 0.1), 0.12, green);
            m.sphere(Vec3::new(-0.12, 0.07, 0.12), 0.1, green);
        }
        DecorKind::Flowers => {
            let cols = [Color::srgb(0.95, 0.2, 0.3), Color::srgb(1.0, 0.85, 0.1), Color::srgb(0.7, 0.3, 0.9), WHITE];
            for i in 0..14 {
                let x = hash(t, 10 + i) - 0.5;
                let z = hash(t, 40 + i) - 0.5;
                let p = Vec3::new(x * 0.8, 0.0, z * 0.8);
                m.aabb(p + Vec3::new(-0.005, 0.0, -0.005), p + Vec3::new(0.005, 0.06, 0.005), Color::srgb(0.2, 0.5, 0.15));
                m.sphere(p + Vec3::Y * 0.07, 0.025, cols[i as usize % 4]);
            }
        }
        DecorKind::Rock => {
            let g = Color::srgb(0.5, 0.5, 0.52);
            m.cuboid(Vec3::new(0.0, 0.1, 0.0), Vec3::new(0.2, 0.12, 0.16), Quat::from_euler(EulerRot::XYZ, 0.3, 0.5, 0.2), g);
            m.cuboid(Vec3::new(0.18, 0.06, 0.12), Vec3::new(0.1, 0.07, 0.09), Quat::from_euler(EulerRot::XYZ, 0.6, 0.2, 0.4), Color::srgb(0.45, 0.45, 0.47));
        }
        DecorKind::Fence => {
            let wood = Color::srgb(0.6, 0.45, 0.3);
            for i in 0..5 {
                let x = -0.4 + i as f32 * 0.2;
                m.aabb(Vec3::new(x - 0.015, 0.0, 0.42), Vec3::new(x + 0.015, 0.16, 0.45), wood);
            }
            for y in [0.06f32, 0.12] {
                m.aabb(Vec3::new(-0.5, y, 0.425), Vec3::new(0.5, y + 0.02, 0.445), wood);
            }
        }
        DecorKind::Lamp => {
            m.cylinder(Vec3::new(0.0, 0.25, 0.0), 0.015, 0.25, 6, Quat::IDENTITY, Color::srgb(0.15, 0.15, 0.18));
            m.aabb(Vec3::new(-0.01, 0.48, 0.0), Vec3::new(0.01, 0.5, 0.12), Color::srgb(0.15, 0.15, 0.18));
            m.aabb(Vec3::new(-0.035, 0.44, 0.08), Vec3::new(0.035, 0.48, 0.15), Color::srgb(1.0, 0.95, 0.6));
        }
        DecorKind::Bench => {
            let wood = Color::srgb(0.55, 0.35, 0.2);
            m.aabb(Vec3::new(-0.2, 0.08, -0.05), Vec3::new(0.2, 0.1, 0.05), wood);
            m.aabb(Vec3::new(-0.2, 0.1, -0.06), Vec3::new(0.2, 0.18, -0.045), wood);
            for x in [-0.17f32, 0.17] {
                m.aabb(Vec3::new(x - 0.01, 0.0, -0.05), Vec3::new(x + 0.01, 0.08, 0.05), Color::srgb(0.2, 0.2, 0.2));
            }
        }
        DecorKind::Fountain => {
            m.cylinder(Vec3::new(0.0, 0.05, 0.0), 0.38, 0.05, 14, Quat::IDENTITY, STONE);
            m.cylinder(Vec3::new(0.0, 0.1, 0.0), 0.33, 0.005, 14, Quat::IDENTITY, Color::srgb(0.3, 0.55, 0.85));
            m.cylinder(Vec3::new(0.0, 0.2, 0.0), 0.04, 0.12, 8, Quat::IDENTITY, STONE);
            m.sphere(Vec3::new(0.0, 0.34, 0.0), 0.07, Color::srgb(0.6, 0.8, 1.0));
        }
        DecorKind::Person => {
            let shirt = [Color::srgb(0.9, 0.2, 0.2), Color::srgb(0.2, 0.4, 0.9), Color::srgb(0.95, 0.8, 0.2), Color::srgb(0.3, 0.7, 0.3)][(hash(t, 7) * 4.0) as usize % 4];
            m.aabb(Vec3::new(-0.03, 0.0, -0.02), Vec3::new(0.03, 0.08, 0.02), Color::srgb(0.2, 0.2, 0.35));
            m.aabb(Vec3::new(-0.04, 0.08, -0.025), Vec3::new(0.04, 0.16, 0.025), shirt);
            m.sphere(Vec3::new(0.0, 0.19, 0.0), 0.03, Color::srgb(0.95, 0.78, 0.62));
        }
    }
    m
}
