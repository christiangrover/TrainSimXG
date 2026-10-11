//! The room, the table, lighting, and keeping terrain / tile meshes in sync
//! with the grid.

use crate::grid::*;
use crate::mesh::*;
use crate::models_tile::*;
use bevy::pbr::CascadeShadowConfigBuilder;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

pub const CHUNK: i32 = 10;
/// Height of the floor below the table top.
pub const FLOOR_Y: f32 = -38.0;

pub struct ScenePlugin;

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TileEntities>()
            .add_systems(Startup, setup_scene)
            .add_systems(PostUpdate, (sync_terrain, sync_tiles).chain().before(TransformSystem::TransformPropagate));
    }
}

#[derive(Resource)]
pub struct Materials {
    pub vertex: Handle<StandardMaterial>,
    pub terrain: Handle<StandardMaterial>,
}

#[derive(Resource, Default)]
pub struct TileEntities {
    tiles: HashMap<IVec2, Entity>,
    chunks: HashMap<IVec2, Entity>,
    generation: u32,
    initialized: bool,
}

#[derive(Component)]
pub struct GateArm {
    pub tile: IVec2,
    pub base: Quat,
    pub open: f32,
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
) {
    let vertex = mats.add(StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.8, ..default() });
    let terrain = mats.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        double_sided: true,
        cull_mode: None,
        ..default()
    });
    commands.insert_resource(Materials { vertex: vertex.clone(), terrain });

    // Table top, rim and legs, floor and walls.
    let half = GRID as f32 / 2.0;
    let wood = rgb(0.55, 0.36, 0.2);
    let mut t = Geo::new();
    t.aabb(Vec3::new(-half - 2.0, -1.2, -half - 2.0), Vec3::new(half + 2.0, -0.12, half + 2.0), wood);
    for (x, z) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let c = Vec3::new(x * (half - 1.0), 0.0, z * (half - 1.0));
        t.aabb(c + Vec3::new(-1.5, FLOOR_Y, -1.5), c + Vec3::new(1.5, -1.2, 1.5), rgb(0.45, 0.28, 0.15));
    }
    t.aabb(Vec3::new(-half - 2.0, -0.12, -half - 2.0), Vec3::new(-half, 0.15, half + 2.0), rgb(0.5, 0.32, 0.18));
    t.aabb(Vec3::new(half, -0.12, -half - 2.0), Vec3::new(half + 2.0, 0.15, half + 2.0), rgb(0.5, 0.32, 0.18));
    t.aabb(Vec3::new(-half, -0.12, -half - 2.0), Vec3::new(half, 0.15, -half), rgb(0.5, 0.32, 0.18));
    t.aabb(Vec3::new(-half, -0.12, half), Vec3::new(half, 0.15, half + 2.0), rgb(0.5, 0.32, 0.18));
    commands.spawn((Mesh3d(meshes.add(t.build())), MeshMaterial3d(vertex.clone())));

    let room = 160.0;
    let mut r = Geo::new();
    r.aabb(Vec3::new(-room, FLOOR_Y - 1.0, -room), Vec3::new(room, FLOOR_Y, room), rgb(0.6, 0.5, 0.42));
    // A rug under the table.
    r.aabb(Vec3::new(-80.0, FLOOR_Y, -80.0), Vec3::new(80.0, FLOOR_Y + 0.05, 80.0), rgb(0.55, 0.2, 0.2));
    let wall = rgb(0.85, 0.82, 0.72);
    let wh = 90.0;
    r.aabb(Vec3::new(-room, FLOOR_Y, -room - 1.0), Vec3::new(room, FLOOR_Y + wh, -room), wall);
    r.aabb(Vec3::new(-room, FLOOR_Y, room), Vec3::new(room, FLOOR_Y + wh, room + 1.0), wall);
    r.aabb(Vec3::new(-room - 1.0, FLOOR_Y, -room), Vec3::new(-room, FLOOR_Y + wh, room), rgb(0.8, 0.85, 0.75));
    r.aabb(Vec3::new(room, FLOOR_Y, -room), Vec3::new(room + 1.0, FLOOR_Y + wh, room), rgb(0.8, 0.85, 0.75));
    // Skirting boards and a big window.
    r.aabb(Vec3::new(-room, FLOOR_Y, -room), Vec3::new(room, FLOOR_Y + 3.0, -room + 0.5), rgb(0.95, 0.95, 0.95));
    r.aabb(Vec3::new(-40.0, FLOOR_Y + 30.0, -room + 0.01), Vec3::new(40.0, FLOOR_Y + 75.0, -room + 0.3), rgb(0.6, 0.8, 1.0));
    commands.spawn((Mesh3d(meshes.add(r.build())), MeshMaterial3d(vertex)));

    commands.insert_resource(AmbientLight { color: Color::WHITE, brightness: 280.0, ..default() });
    commands.insert_resource(ClearColor(rgb(0.55, 0.7, 0.85)));
    commands.spawn((
        DirectionalLight { illuminance: 6500.0, shadows_enabled: true, ..default() },
        Transform::from_xyz(30.0, 80.0, 45.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder { maximum_distance: 220.0, first_cascade_far_bound: 15.0, ..default() }.build(),
    ));
    commands.spawn((
        PointLight { intensity: 1.5e8, range: 400.0, shadows_enabled: false, ..default() },
        Transform::from_xyz(0.0, FLOOR_Y + 85.0, 0.0),
    ));
}

fn ground_color(tile: &Tile, t: IVec2) -> Color {
    let n = (((t.x * 7 + t.y * 13) ^ (t.x * t.y)) & 7) as f32 / 7.0;
    let lift = tile.height as f32 * 0.025;
    match tile.ground {
        Ground::Grass => rgb(0.3 + n * 0.04 + lift, 0.58 + n * 0.05 + lift, 0.25 + lift * 0.5),
        Ground::Sand => rgb(0.86 + n * 0.03, 0.78 + n * 0.03, 0.55),
        Ground::Water => rgb(0.2, 0.45 + n * 0.03, 0.8),
        Ground::Rock => rgb(0.5 + n * 0.04 + lift, 0.48 + n * 0.04 + lift, 0.45 + lift),
    }
}

fn chunk_mesh(grid: &Grid, c: IVec2) -> Mesh {
    let mut mb = Geo::new();
    for z in c.y * CHUNK..(c.y + 1) * CHUNK {
        for x in c.x * CHUNK..(c.x + 1) * CHUNK {
            let t = IVec2::new(x, z);
            let Some(tile) = grid.get(t) else { continue };
            let ctr = tile_center(t);
            let y = tile.surface_y();
            let p = |dx: f32, dz: f32, yy: f32| Vec3::new(ctr.x + dx, yy, ctr.z + dz);
            mb.quad(p(-0.5, 0.5, y), p(0.5, 0.5, y), p(0.5, -0.5, y), p(-0.5, -0.5, y), ground_color(tile, t));
            for e in 0..4u8 {
                let nt = t + edge_dir(e);
                let ny = grid.get(nt).map(|n| n.surface_y()).unwrap_or(0.0);
                if ny >= y {
                    continue;
                }
                // Corners for the East face, rotated to edge e.
                let steps = (e + 3) % 4;
                let mut a = Vec2::new(0.5, 0.5);
                let mut b = Vec2::new(0.5, -0.5);
                for _ in 0..steps {
                    a = Vec2::new(-a.y, a.x);
                    b = Vec2::new(-b.y, b.x);
                }
                let earth = if tile.ground == Ground::Rock { rgb(0.42, 0.4, 0.38) } else { rgb(0.5, 0.36, 0.22) };
                let layer = |k: i32| if k % 2 == 0 { earth } else { earth.mix(&rgb(0.3, 0.22, 0.15), 0.25) };
                // One band per level so cliffs look like stacked layers.
                let mut lo = ny;
                let mut k = 0;
                while lo < y - 0.001 {
                    let hi = (lo + LEVEL_H).min(y);
                    mb.quad(p(a.x, a.y, lo), p(b.x, b.y, lo), p(b.x, b.y, hi), p(a.x, a.y, hi), layer(k));
                    lo = hi;
                    k += 1;
                }
            }
        }
    }
    mb.build()
}

fn sync_terrain(
    mut commands: Commands,
    mut grid: ResMut<Grid>,
    mut ents: ResMut<TileEntities>,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<Materials>,
) {
    let full = !ents.initialized || ents.generation != grid.generation;
    let mut chunks: HashSet<IVec2> = HashSet::new();
    if full {
        for z in 0..GRID / CHUNK {
            for x in 0..GRID / CHUNK {
                chunks.insert(IVec2::new(x, z));
            }
        }
    } else {
        for t in grid.terrain_dirty.drain(..) {
            chunks.insert(IVec2::new(t.x.div_euclid(CHUNK), t.y.div_euclid(CHUNK)));
        }
    }
    grid.terrain_dirty.clear();
    for c in chunks {
        let mesh = meshes.add(chunk_mesh(&grid, c));
        if let Some(&e) = ents.chunks.get(&c) {
            commands.entity(e).insert(Mesh3d(mesh));
        } else {
            let e = commands.spawn((Mesh3d(mesh), MeshMaterial3d(mats.terrain.clone()), Transform::default())).id();
            ents.chunks.insert(c, e);
        }
    }
}

fn sync_tiles(
    mut commands: Commands,
    mut grid: ResMut<Grid>,
    mut ents: ResMut<TileEntities>,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<Materials>,
) {
    let full = !ents.initialized || ents.generation != grid.generation;
    let mut todo: HashSet<IVec2> = HashSet::new();
    if full {
        for (_, e) in ents.tiles.drain() {
            commands.entity(e).despawn();
        }
        for z in 0..GRID {
            for x in 0..GRID {
                let t = IVec2::new(x, z);
                if grid.get(t).map(|t| t.content != Content::Empty).unwrap_or(false) {
                    todo.insert(t);
                }
            }
        }
        ents.generation = grid.generation;
        ents.initialized = true;
    }
    todo.extend(grid.dirty_tiles.drain(..));
    let arm = gate_arm_mesh().build();
    let arm = meshes.add(arm);
    for t in todo {
        if let Some(e) = ents.tiles.remove(&t) {
            commands.entity(e).despawn();
        }
        let mb = tile_mesh(&grid, t);
        if mb.is_empty() {
            continue;
        }
        let e = commands
            .spawn((Mesh3d(meshes.add(mb.build())), MeshMaterial3d(mats.vertex.clone()), Transform::from_translation(tile_center(t))))
            .id();
        if let Some(tr) = grid.track(t) {
            if tr.kind == TrackKind::LevelCrossing {
                let rq = rot_quat(tr.rot);
                let y0 = tr.level as f32 * LEVEL_H;
                for (x, z, yaw) in [(0.32f32, 0.38f32, std::f32::consts::PI), (-0.32, -0.38, 0.0)] {
                    let base = rq * Quat::from_rotation_y(yaw);
                    commands.entity(e).with_child((
                        Mesh3d(arm.clone()),
                        MeshMaterial3d(mats.vertex.clone()),
                        Transform::from_translation(rq * Vec3::new(x, y0 + 0.24, z)).with_rotation(base),
                        GateArm { tile: t, base, open: 1.0 },
                    ));
                }
            }
        }
        ents.tiles.insert(t, e);
    }
}
