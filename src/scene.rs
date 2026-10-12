//! The room, the table, lighting, and keeping terrain / tile meshes in sync
//! with the grid.

use crate::grid::*;
use crate::mesh::*;
use crate::materials::{self, meshes_of, spawn_geo, spawn_parts, Lamp, Materials};
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

#[derive(Resource, Default)]
pub struct TileEntities {
    tiles: HashMap<IVec2, Entity>,
    chunks: HashMap<IVec2, Entity>,
    generation: u32,
    initialized: bool,
}

/// One lamp of a traffic signal at a road junction.
#[derive(Component)]
pub struct SignalLamp {
    pub tile: IVec2,
    /// The edge whose incoming traffic this signal controls.
    pub edge: u8,
    pub lamp: Lamp,
}

/// One of a level crossing's flashing red lamps (`pair` 0 / 1 flash alternately).
#[derive(Component)]
pub struct CrossingLamp {
    pub tile: IVec2,
    pub pair: usize,
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
    mut images: ResMut<Assets<Image>>,
    mut std_mats: ResMut<Assets<StandardMaterial>>,
) {
    let mats = materials::create(&mut images, &mut std_mats);

    // Table top, rim and legs.
    let half = GRID as f32 / 2.0;
    let mut t = Geo::new();
    t.mat(Mat::TableWood);
    let wood = rgb(0.85, 0.7, 0.55);
    t.aabb(Vec3::new(-half - 2.0, -1.2, -half - 2.0), Vec3::new(half + 2.0, -0.12, half + 2.0), wood);
    for (x, z) in [(-1.0f32, -1.0f32), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        let c = Vec3::new(x * (half - 1.0), 0.0, z * (half - 1.0));
        t.aabb(c + Vec3::new(-1.5, FLOOR_Y, -1.5), c + Vec3::new(1.5, -1.2, 1.5), rgb(0.7, 0.55, 0.42));
    }
    let rim = rgb(0.75, 0.58, 0.45);
    t.aabb(Vec3::new(-half - 2.0, -0.12, -half - 2.0), Vec3::new(-half, 0.15, half + 2.0), rim);
    t.aabb(Vec3::new(half, -0.12, -half - 2.0), Vec3::new(half + 2.0, 0.15, half + 2.0), rim);
    t.aabb(Vec3::new(-half, -0.12, -half - 2.0), Vec3::new(half, 0.15, -half), rim);
    t.aabb(Vec3::new(-half, -0.12, half), Vec3::new(half, 0.15, half + 2.0), rim);
    spawn_geo(&mut commands, &mut meshes, &mats, t, Transform::default());

    // The room: wooden floor, a rug, plastered walls and a sunny window.
    let room = 160.0;
    let mut r = Geo::new();
    r.mat(Mat::Floor);
    r.aabb(Vec3::new(-room, FLOOR_Y - 1.0, -room), Vec3::new(room, FLOOR_Y, room), rgb(0.9, 0.8, 0.7));
    r.mat(Mat::Plain);
    r.aabb(Vec3::new(-80.0, FLOOR_Y, -80.0), Vec3::new(80.0, FLOOR_Y + 0.05, 80.0), rgb(0.5, 0.17, 0.17));
    r.aabb(Vec3::new(-76.0, FLOOR_Y + 0.05, -76.0), Vec3::new(76.0, FLOOR_Y + 0.07, 76.0), rgb(0.62, 0.25, 0.2));
    r.mat(Mat::Plaster);
    let wall = rgb(0.95, 0.92, 0.82);
    let wh = 90.0;
    r.aabb(Vec3::new(-room, FLOOR_Y, -room - 1.0), Vec3::new(room, FLOOR_Y + wh, -room), wall);
    r.aabb(Vec3::new(-room, FLOOR_Y, room), Vec3::new(room, FLOOR_Y + wh, room + 1.0), wall);
    r.aabb(Vec3::new(-room - 1.0, FLOOR_Y, -room), Vec3::new(-room, FLOOR_Y + wh, room), rgb(0.88, 0.93, 0.85));
    r.aabb(Vec3::new(room, FLOOR_Y, -room), Vec3::new(room + 1.0, FLOOR_Y + wh, room), rgb(0.88, 0.93, 0.85));
    r.mat(Mat::Paint);
    r.aabb(Vec3::new(-room, FLOOR_Y, -room), Vec3::new(room, FLOOR_Y + 3.0, -room + 0.5), rgb(0.95, 0.95, 0.95));
    r.aabb(Vec3::new(-42.0, FLOOR_Y + 28.0, -room + 0.01), Vec3::new(42.0, FLOOR_Y + 77.0, -room + 0.25), rgb(0.95, 0.95, 0.95));
    r.mat(Mat::Plain);
    r.aabb(Vec3::new(-40.0, FLOOR_Y + 30.0, -room + 0.02), Vec3::new(40.0, FLOOR_Y + 75.0, -room + 0.3), rgb(0.75, 0.88, 1.0));
    spawn_geo(&mut commands, &mut meshes, &mats, r, Transform::default());
    commands.insert_resource(mats);

    commands.insert_resource(AmbientLight { color: rgb(0.85, 0.9, 1.0), brightness: 350.0, ..default() });
    commands.insert_resource(ClearColor(rgb(0.55, 0.7, 0.85)));
    // Afternoon sun slanting in through the window.
    commands.spawn((
        DirectionalLight { illuminance: 9000.0, color: rgb(1.0, 0.95, 0.85), shadows_enabled: true, ..default() },
        Transform::from_xyz(30.0, 70.0, 55.0).looking_at(Vec3::ZERO, Vec3::Y),
        CascadeShadowConfigBuilder { maximum_distance: 200.0, first_cascade_far_bound: 12.0, num_cascades: 4, ..default() }.build(),
    ));
    commands.spawn((
        PointLight { intensity: 1.0e8, range: 400.0, color: rgb(1.0, 0.9, 0.75), shadows_enabled: false, ..default() },
        Transform::from_xyz(0.0, FLOOR_Y + 85.0, 0.0),
    ));
}

/// Gentle per-tile tint so tiled textures don't look uniform.
fn ground_tint(tile: &Tile, t: IVec2) -> (Mat, Color) {
    let n = (((t.x * 7 + t.y * 13) ^ (t.x * t.y)) & 7) as f32 / 7.0;
    let lift = tile.height as f32 * 0.015;
    match tile.ground {
        Ground::Grass => (Mat::Grass, rgb(0.9 + n * 0.08 + lift, 0.95 + n * 0.05 + lift, 0.85 + lift)),
        Ground::Sand => (Mat::Sand, rgb(1.0, 0.97, 0.9)),
        Ground::Water => (Mat::Water, rgb(0.16, 0.36 + n * 0.03, 0.52)),
        Ground::Rock => (Mat::Rock, rgb(0.9 + lift, 0.9 + lift, 0.9 + lift)),
    }
}

fn chunk_geo(grid: &Grid, c: IVec2) -> Geo {
    let mut mb = Geo::new();
    for z in c.y * CHUNK..(c.y + 1) * CHUNK {
        for x in c.x * CHUNK..(c.x + 1) * CHUNK {
            let t = IVec2::new(x, z);
            let Some(tile) = grid.get(t) else { continue };
            let ctr = tile_center(t);
            let y = tile.surface_y();
            let p = |dx: f32, dz: f32, yy: f32| Vec3::new(ctr.x + dx, yy, ctr.z + dz);
            let (mat, col) = ground_tint(tile, t);
            mb.mat(mat);
            mb.quad(p(-0.5, 0.5, y), p(0.5, 0.5, y), p(0.5, -0.5, y), p(-0.5, -0.5, y), col);
            mb.mat(if tile.ground == Ground::Rock { Mat::Rock } else { Mat::Earth });
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
                let earth = rgb(0.95, 0.9, 0.85);
                let layer = |k: i32| if k % 2 == 0 { earth } else { rgb(0.82, 0.76, 0.7) };
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
    mb
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
        if let Some(e) = ents.chunks.remove(&c) {
            commands.entity(e).despawn();
        }
        let e = spawn_geo(&mut commands, &mut meshes, &mats, chunk_geo(&grid, c), Transform::default());
        ents.chunks.insert(c, e);
    }
}

fn sync_tiles(
    mut commands: Commands,
    mut grid: ResMut<Grid>,
    mut ents: ResMut<TileEntities>,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Res<Materials>,
    mut arm: Local<Option<Vec<(Mat, Handle<Mesh>)>>>,
    mut bulb: Local<Option<Handle<Mesh>>>,
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
    if todo.is_empty() {
        return;
    }
    let arm = arm.get_or_insert_with(|| meshes_of(&mut meshes, gate_arm_mesh())).clone();
    let bulb = bulb.get_or_insert_with(|| meshes.add(Sphere::new(0.017).mesh().ico(2).unwrap())).clone();
    for t in todo {
        if let Some(e) = ents.tiles.remove(&t) {
            commands.entity(e).despawn();
        }
        let mb = tile_mesh(&grid, t);
        if mb.is_empty() {
            continue;
        }
        let e = spawn_geo(&mut commands, &mut meshes, &mats, mb, Transform::from_translation(tile_center(t)));
        if let Some(tr) = grid.track(t) {
            if tr.kind == TrackKind::LevelCrossing {
                let rq = rot_quat(tr.rot);
                let y0 = tr.level as f32 * LEVEL_H;
                for (x, z, yaw) in [(0.32f32, 0.38f32, std::f32::consts::PI), (-0.32, -0.38, 0.0)] {
                    let base = rq * Quat::from_rotation_y(yaw);
                    let tf = Transform::from_translation(rq * Vec3::new(x, y0 + 0.24, z)).with_rotation(base);
                    let a = spawn_parts(&mut commands, &mats, &arm, tf);
                    commands.entity(a).insert(GateArm { tile: t, base, open: 1.0 });
                    commands.entity(e).add_child(a);
                }
                for (i, p) in CROSSING_LAMPS.iter().enumerate() {
                    let lamp = commands
                        .spawn((
                            Mesh3d(bulb.clone()),
                            MeshMaterial3d(mats.lamp(Lamp::Red, false)),
                            Transform::from_translation(rq * (*p + Vec3::Y * y0)).with_scale(Vec3::splat(1.3)),
                            CrossingLamp { tile: t, pair: i % 2 },
                        ))
                        .id();
                    commands.entity(e).add_child(lamp);
                }
            }
        }
        // Traffic signals at road junctions.
        if grid.get(t).is_some_and(|x| x.content == Content::Road) {
            let y = grid.get(t).unwrap().surface_y();
            for (edge, head, facing) in signal_heads(&grid.road_edges(t)) {
                for (p, lamp) in signal_lamps(head, facing).into_iter().zip([Lamp::Red, Lamp::Yellow, Lamp::Green]) {
                    let l = commands
                        .spawn((
                            Mesh3d(bulb.clone()),
                            MeshMaterial3d(mats.lamp(lamp, false)),
                            Transform::from_translation(p + Vec3::Y * y).with_scale(Vec3::splat(1.2)),
                            SignalLamp { tile: t, edge, lamp },
                        ))
                        .id();
                    commands.entity(e).add_child(l);
                }
            }
        }
        ents.tiles.insert(t, e);
    }
}