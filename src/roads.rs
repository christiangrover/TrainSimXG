//! Road traffic and level-crossing gates.

use crate::grid::*;
use crate::mesh::*;
use crate::models_vehicle::road_vehicle_mesh;
use crate::scene::{GateArm, Materials};
use crate::trains::{Sim, Trains};
use bevy::prelude::*;
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::HashSet;

pub struct RoadsPlugin;

impl Plugin for RoadsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Traffic>()
            .init_resource::<ClosedCrossings>()
            .add_systems(Update, (update_crossings, animate_gates, spawn_traffic, drive_cars).chain());
    }
}

#[derive(Resource)]
pub struct Traffic {
    pub target: usize,
}

impl Default for Traffic {
    fn default() -> Self {
        Traffic { target: 20 }
    }
}

#[derive(Resource, Default)]
pub struct ClosedCrossings(pub HashSet<IVec2>);

#[derive(Component)]
pub struct RoadCar {
    tile: IVec2,
    entry: u8,
    exit: u8,
    t: f32,
    speed: f32,
    cur: f32,
}

fn update_crossings(grid: Res<Grid>, trains: Res<Trains>, mut closed: ResMut<ClosedCrossings>) {
    closed.0.clear();
    for t in &trains.list {
        for c in &t.cars {
            let p = c.pos.world(&grid).0;
            let ct = world_to_tile(p);
            for dz in -3..=3 {
                for dx in -3..=3 {
                    let n = ct + IVec2::new(dx, dz);
                    if let Some(tr) = grid.track(n) {
                        if tr.kind == TrackKind::LevelCrossing {
                            let c = tile_center(n);
                            if Vec2::new(p.x - c.x, p.z - c.z).length() < 2.6 {
                                closed.0.insert(n);
                            }
                        }
                    }
                }
            }
        }
    }
}

fn animate_gates(time: Res<Time>, closed: Res<ClosedCrossings>, mut q: Query<(&mut GateArm, &mut Transform)>) {
    for (mut g, mut tf) in &mut q {
        let target = if closed.0.contains(&g.tile) { 0.0 } else { 1.0 };
        let step = time.delta_secs() * 0.8;
        g.open += (target - g.open).clamp(-step, step);
        tf.rotation = g.base * Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2 * g.open);
    }
}

fn spawn_traffic(
    mut commands: Commands,
    grid: Res<Grid>,
    traffic: Res<Traffic>,
    mats: Option<Res<Materials>>,
    mut meshes: ResMut<Assets<Mesh>>,
    q: Query<(Entity, &RoadCar)>,
    mut cache: Local<Vec<Handle<Mesh>>>,
    mut gen: Local<u32>,
    mut cooldown: Local<f32>,
    time: Res<Time>,
) {
    let Some(mats) = mats else { return };
    // Clear all traffic when a new layout is loaded.
    if *gen != grid.generation {
        *gen = grid.generation;
        for (e, _) in &q {
            commands.entity(e).despawn();
        }
        return;
    }
    let count = q.iter().count();
    if count > traffic.target {
        for (e, _) in q.iter().take(count - traffic.target) {
            commands.entity(e).despawn();
        }
        return;
    }
    *cooldown -= time.delta_secs();
    if count >= traffic.target || *cooldown > 0.0 {
        return;
    }
    *cooldown = 0.15;
    if cache.is_empty() {
        let paints = [rgb(0.85, 0.1, 0.1), rgb(0.1, 0.3, 0.85), rgb(0.95, 0.8, 0.1), rgb(0.95, 0.95, 0.95), rgb(0.2, 0.65, 0.3), rgb(0.1, 0.1, 0.1), rgb(0.95, 0.5, 0.1)];
        for (i, p) in paints.iter().enumerate() {
            let style = [0u8, 0, 1, 0, 3, 0, 2][i];
            cache.push(meshes.add(road_vehicle_mesh(style, *p).build()));
        }
    }
    let mut rng = rand::thread_rng();
    // Pick a random road tile to drop a car on.
    for _ in 0..40 {
        let t = IVec2::new(rng.gen_range(0..GRID), rng.gen_range(0..GRID));
        if grid.get(t).map(|x| x.content) != Some(Content::Road) {
            continue;
        }
        let edges = grid.road_edges(t);
        if edges.is_empty() {
            continue;
        }
        let entry = *edges.choose(&mut rng).unwrap();
        let exit = choose_exit(&edges, entry, &mut rng);
        let occupied = q.iter().any(|(_, c)| c.tile == t);
        if occupied {
            continue;
        }
        let speed = rng.gen_range(0.7..1.2);
        commands.spawn((
            Mesh3d(cache.choose(&mut rng).unwrap().clone()),
            MeshMaterial3d(mats.vertex.clone()),
            Transform::from_translation(tile_center(t)),
            RoadCar { tile: t, entry, exit, t: 0.0, speed, cur: 0.0 },
        ));
        break;
    }
}

fn choose_exit(edges: &[u8], entry: u8, rng: &mut impl Rng) -> u8 {
    let options: Vec<u8> = edges.iter().copied().filter(|&e| e != entry).collect();
    options.choose(rng).copied().unwrap_or(entry)
}

fn road_y(grid: &Grid, t: IVec2) -> f32 {
    match grid.get(t) {
        Some(tile) => match tile.content {
            Content::Track(tr) => tr.level as f32 * LEVEL_H + 0.05,
            _ => tile.surface_y() + 0.02,
        },
        None => 0.0,
    }
}

fn car_pose(c: &RoadCar, grid: &Grid) -> (Vec3, Vec3) {
    let p0 = edge_mid(c.entry);
    let p2 = edge_mid(c.exit);
    let t = c.t;
    let (pos, tan) = if c.entry == c.exit {
        // Dead end: drive to the middle and turn around.
        if t < 0.5 {
            (p0 * (1.0 - t * 2.0), -p0)
        } else {
            (p0 * (t * 2.0 - 1.0), p0)
        }
    } else {
        let pos = p0 * (1.0 - t) * (1.0 - t) + p2 * t * t;
        let tan = -p0 * 2.0 * (1.0 - t) + p2 * 2.0 * t;
        (pos, tan)
    };
    let fwd = Vec3::new(tan.x, 0.0, tan.y).normalize_or(Vec3::Z);
    let right = Vec3::new(-fwd.z, 0.0, fwd.x);
    let c3 = tile_center(c.tile);
    (Vec3::new(c3.x + pos.x, road_y(grid, c.tile), c3.z + pos.y) + right * 0.12, fwd)
}

fn path_len(c: &RoadCar) -> f32 {
    if c.entry == opposite(c.exit) {
        1.0
    } else if c.entry == c.exit {
        1.0
    } else {
        0.8
    }
}

fn drive_cars(
    mut commands: Commands,
    time: Res<Time>,
    sim: Res<Sim>,
    grid: Res<Grid>,
    closed: Res<ClosedCrossings>,
    mut q: Query<(Entity, &mut RoadCar, &mut Transform)>,
) {
    let dt = time.delta_secs().min(0.05) * sim.time_scale;
    let snapshot: Vec<(Entity, Vec3, Vec3)> = q
        .iter()
        .map(|(e, c, _)| {
            let (p, f) = car_pose(&c, &grid);
            (e, p, f)
        })
        .collect();
    let mut rng = rand::thread_rng();
    for (e, mut c, mut tf) in &mut q {
        if grid.road_edges(c.tile).is_empty() {
            commands.entity(e).despawn();
            continue;
        }
        if !sim.running {
            continue;
        }
        let (p, f) = car_pose(&c, &grid);
        // Brake for cars ahead in our lane.
        let mut target = c.speed;
        for (oe, op, of) in &snapshot {
            if *oe == e {
                continue;
            }
            let d = *op - p;
            let ahead = d.dot(f);
            let side = (d - f * ahead).length();
            if ahead > 0.0 && ahead < 0.32 && side < 0.12 && of.dot(f) > -0.3 {
                target = 0.0;
            }
        }
        // Stop at closed crossing gates.
        let next = c.tile + edge_dir(c.exit);
        let on_crossing = closed.0.contains(&c.tile);
        if closed.0.contains(&next) && !on_crossing && c.t > 0.55 {
            target = 0.0;
        }
        let a = 2.0 * dt;
        c.cur += (target - c.cur).clamp(-a * 2.0, a);
        c.t += c.cur * dt / path_len(&c);
        if c.t >= 1.0 {
            let entry = opposite(c.exit);
            let edges = grid.road_edges(next);
            if edges.contains(&entry) {
                c.tile = next;
                c.entry = entry;
                c.exit = choose_exit(&edges, entry, &mut rng);
            } else {
                // Road ended or was removed: turn around on this tile.
                let ex = c.exit;
                c.entry = ex;
                c.exit = choose_exit(&grid.road_edges(c.tile), ex, &mut rng);
            }
            c.t = 0.0;
        }
        let (p, f) = car_pose(&c, &grid);
        tf.translation = p;
        tf.rotation = basis_from_dir(f);
    }
}
