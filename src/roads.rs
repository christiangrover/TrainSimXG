//! Road traffic (cars, vans, buses, trucks and emergency vehicles) and
//! level-crossing gates.

use crate::grid::*;
use crate::materials::{meshes_of, spawn_parts, Materials};
use crate::mesh::*;
use crate::models_vehicle::{road_vehicle_mesh, RoadStyle};
use crate::scene::GateArm;
use crate::trains::{Sim, Trains};
use bevy::prelude::*;
use rand::seq::SliceRandom;
use rand::Rng;
use std::collections::{HashMap, HashSet};

pub struct RoadsPlugin;

impl Plugin for RoadsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Traffic>()
            .init_resource::<ClosedCrossings>()
            .add_systems(Update, (update_crossings, animate_gates, spawn_traffic, spawn_emergency, emergency_calls, drive_cars).chain());
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
    /// Half the vehicle's length, for bumper-to-bumper spacing.
    half: f32,
    /// Junction tile reserved ahead of us.
    hold: Option<IVec2>,
    /// Junction we just left, kept until our tail is out of it.
    tail: Option<IVec2>,
    /// How long we've been waiting for a junction to clear.
    waited: f32,
    /// The junction we're stopped in front of, waiting for it to clear.
    wants: Option<IVec2>,
    /// Emergency vehicles: the station they belong to, and whether they're on a call.
    home: Option<IVec2>,
    on_call: bool,
    call_timer: f32,
}

impl RoadCar {
    pub fn is_emergency(&self) -> bool {
        self.home.is_some()
    }

    fn new(tile: IVec2, entry: u8, exit: u8, speed: f32, half: f32) -> Self {
        RoadCar { tile, entry, exit, t: 0.0, speed, cur: 0.0, half, hold: None, tail: None, waited: 0.0, wants: None, home: None, on_call: false, call_timer: 0.0 }
    }
}

/// The two light-bar versions of an emergency vehicle (lights off / flashing).
#[derive(Component)]
struct Beacons {
    off: Entity,
    on: Entity,
}

/// Tiles where only one car may be at a time: junctions and dead ends.
fn is_junction(grid: &Grid, t: IVec2) -> bool {
    grid.road_edges(t).len() != 2
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

type Parts = Vec<(Mat, Handle<Mesh>)>;

/// Uploaded road vehicle models, built on first use.
#[derive(Default)]
struct VehicleModels {
    civilian: Vec<(RoadStyle, Parts)>,
    emergency: HashMap<RoadStyle, (Parts, Parts)>,
}

impl VehicleModels {
    fn load(&mut self, meshes: &mut Assets<Mesh>) {
        if !self.civilian.is_empty() {
            return;
        }
        let civ = [
            (RoadStyle::Car, rgb(0.8, 0.1, 0.1)),
            (RoadStyle::Car, rgb(0.1, 0.3, 0.8)),
            (RoadStyle::Car, rgb(0.95, 0.95, 0.95)),
            (RoadStyle::Car, rgb(0.15, 0.15, 0.15)),
            (RoadStyle::Car, rgb(0.2, 0.6, 0.3)),
            (RoadStyle::Car, rgb(0.6, 0.6, 0.62)),
            (RoadStyle::Van, rgb(0.95, 0.8, 0.1)),
            (RoadStyle::Van, rgb(0.9, 0.9, 0.9)),
            (RoadStyle::Truck, rgb(0.2, 0.35, 0.7)),
            (RoadStyle::Bus, rgb(0.9, 0.45, 0.1)),
        ];
        for (style, paint) in civ {
            self.civilian.push((style, meshes_of(meshes, road_vehicle_mesh(style, paint, false))));
        }
        for style in [RoadStyle::Police, RoadStyle::Ambulance, RoadStyle::FireEngine] {
            let off = meshes_of(meshes, road_vehicle_mesh(style, Color::WHITE, false));
            let on = meshes_of(meshes, road_vehicle_mesh(style, Color::WHITE, true));
            self.emergency.insert(style, (off, on));
        }
    }
}

/// A clear, non-junction road tile to put a new vehicle on.
fn spawn_spot(grid: &Grid, t: IVec2, cars: &Query<(Entity, &RoadCar)>) -> Option<Vec<u8>> {
    if grid.get(t).map(|x| x.content) != Some(Content::Road) || is_junction(grid, t) {
        return None;
    }
    let edges = grid.road_edges(t);
    let near = cars.iter().any(|(_, c)| (c.tile - t).abs().max_element() <= 1);
    (!edges.is_empty() && !near).then_some(edges)
}

#[allow(clippy::too_many_arguments)]
fn spawn_traffic(
    mut commands: Commands,
    grid: Res<Grid>,
    traffic: Res<Traffic>,
    mats: Option<Res<Materials>>,
    mut meshes: ResMut<Assets<Mesh>>,
    q: Query<(Entity, &RoadCar)>,
    mut models: Local<VehicleModels>,
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
    let civilians: Vec<Entity> = q.iter().filter(|(_, c)| c.home.is_none()).map(|(e, _)| e).collect();
    if civilians.len() > traffic.target {
        for e in civilians.iter().take(civilians.len() - traffic.target) {
            commands.entity(*e).despawn();
        }
        return;
    }
    *cooldown -= time.delta_secs();
    if civilians.len() >= traffic.target || *cooldown > 0.0 {
        return;
    }
    *cooldown = 0.15;
    models.load(&mut meshes);
    let mut rng = rand::thread_rng();
    // Pick a random road tile to drop a car on.
    for _ in 0..40 {
        let t = IVec2::new(rng.gen_range(0..GRID), rng.gen_range(0..GRID));
        let Some(edges) = spawn_spot(&grid, t, &q) else { continue };
        let entry = *edges.choose(&mut rng).unwrap();
        let exit = choose_exit(&edges, entry, &mut rng);
        let (style, parts) = models.civilian.choose(&mut rng).unwrap();
        let speed = rng.gen_range(0.7..1.2);
        let e = spawn_parts(&mut commands, &mats, parts, Transform::from_translation(tile_center(t)));
        commands.entity(e).insert(RoadCar::new(t, entry, exit, speed, style.half_len()));
        break;
    }
}

fn station_vehicles(kind: DecorKind) -> Option<(RoadStyle, usize)> {
    match kind {
        DecorKind::PoliceStation => Some((RoadStyle::Police, 2)),
        DecorKind::FireStation => Some((RoadStyle::FireEngine, 1)),
        DecorKind::Hospital => Some((RoadStyle::Ambulance, 2)),
        _ => None,
    }
}

/// Each police station, fire station and hospital keeps its own vehicles on
/// the nearby roads; they go when the building does.
#[allow(clippy::too_many_arguments)]
fn spawn_emergency(
    mut commands: Commands,
    grid: Res<Grid>,
    mats: Option<Res<Materials>>,
    mut meshes: ResMut<Assets<Mesh>>,
    q: Query<(Entity, &RoadCar)>,
    mut models: Local<VehicleModels>,
    mut timer: Local<f32>,
    time: Res<Time>,
) {
    let Some(mats) = mats else { return };
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.5;
    models.load(&mut meshes);
    let home_kind = |t: IVec2| match grid.get(t).map(|x| x.content) {
        Some(Content::Decor { kind, .. }) => station_vehicles(kind),
        _ => None,
    };
    let mut have: HashMap<IVec2, usize> = HashMap::new();
    for (e, c) in &q {
        if let Some(h) = c.home {
            if home_kind(h).is_none() {
                commands.entity(e).despawn();
            } else {
                *have.entry(h).or_default() += 1;
            }
        }
    }
    let mut rng = rand::thread_rng();
    for z in 0..GRID {
        for x in 0..GRID {
            let home = IVec2::new(x, z);
            let Some((style, want)) = home_kind(home) else { continue };
            if have.get(&home).copied().unwrap_or(0) >= want {
                continue;
            }
            // Leave from a road near the station (one vehicle per check).
            let mut spots = vec![];
            for dz in -3..=3 {
                for dx in -3..=3 {
                    let t = home + IVec2::new(dx, dz);
                    if let Some(edges) = spawn_spot(&grid, t, &q) {
                        spots.push((t, edges));
                    }
                }
            }
            let Some((t, edges)) = spots.choose(&mut rng).cloned() else { continue };
            let entry = *edges.choose(&mut rng).unwrap();
            let exit = choose_exit(&edges, entry, &mut rng);
            let (off_parts, on_parts) = &models.emergency[&style];
            let off = spawn_parts(&mut commands, &mats, off_parts, Transform::IDENTITY);
            let on = spawn_parts(&mut commands, &mats, on_parts, Transform::IDENTITY);
            commands.entity(on).insert(Visibility::Hidden);
            debug!("{style:?} leaves the station at {home} from road tile {t}");
            let mut car = RoadCar::new(t, entry, exit, rng.gen_range(0.8..1.0), style.half_len());
            car.home = Some(home);
            car.call_timer = rng.gen_range(3.0..15.0);
            commands
                .spawn((Transform::from_translation(tile_center(t)), Visibility::default(), car, Beacons { off, on }))
                .add_children(&[off, on]);
            return;
        }
    }
}

/// Emergency vehicles go out on calls now and then: lights flashing, driving faster.
fn emergency_calls(time: Res<Time>, sim: Res<Sim>, mut q: Query<(&mut RoadCar, &Beacons)>, mut vis: Query<&mut Visibility>) {
    if !sim.running {
        return;
    }
    let mut rng = rand::thread_rng();
    for (mut c, b) in &mut q {
        c.call_timer -= time.delta_secs() * sim.time_scale;
        if c.call_timer > 0.0 {
            continue;
        }
        c.on_call = !c.on_call;
        c.call_timer = if c.on_call { rng.gen_range(15.0..30.0) } else { rng.gen_range(20.0..45.0) };
        if let Ok(mut v) = vis.get_mut(b.on) {
            *v = if c.on_call { Visibility::Inherited } else { Visibility::Hidden };
        }
        if let Ok(mut v) = vis.get_mut(b.off) {
            *v = if c.on_call { Visibility::Hidden } else { Visibility::Inherited };
        }
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
    let snapshot: Vec<(Entity, Vec3, Vec3, f32)> = q
        .iter()
        .map(|(e, c, _)| {
            let (p, f) = car_pose(&c, &grid);
            (e, p, f, c.half)
        })
        .collect();
    // Who is in, or has claimed, each junction.
    let mut owner: HashMap<IVec2, Entity> = HashMap::new();
    for (e, c, _) in &q {
        if is_junction(&grid, c.tile) {
            owner.entry(c.tile).or_insert(e);
        }
        for h in [c.hold, c.tail].into_iter().flatten() {
            owner.entry(h).or_insert(e);
        }
    }
    let wants: HashMap<Entity, IVec2> = q.iter().filter_map(|(e, c, _)| c.wants.map(|w| (e, w))).collect();
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
        let len = path_len(&c);
        let mut target = if c.on_call { c.speed * 1.5 } else { c.speed };
        c.wants = None;

        // Keep a safe gap to whatever is in our lane ahead (bumper to bumper),
        // slowing in proportion to the gap so we can never close it.
        for (oe, op, of, ohalf) in &snapshot {
            if *oe == e {
                continue;
            }
            let d = *op - p;
            let ahead = d.dot(f);
            let side = (d - f * ahead).length();
            let dist = d.length();
            // In our lane, or close in front and heading our way (on a curve the
            // car ahead sits off to the side of our straight-ahead line).
            let in_lane = side < 0.15 && of.dot(f) > -0.5;
            let close_on_curve = dist < c.half + ohalf + 0.3 && of.dot(f) > 0.0;
            if ahead > 0.0 && (in_lane || close_on_curve) {
                let gap = ahead.min(dist) - c.half - ohalf;
                if gap < 0.8 {
                    target = target.min(((gap - 0.06) * 3.0).max(0.0));
                }
            }
        }

        // Stuck behind another car: that's a queue, not waiting for the junction.
        let queued = target < c.speed - 1e-3;

        // Things that make us stop at the end of this tile: closed gates,
        // or a junction ahead that someone else is using.
        let next = c.tile + edge_dir(c.exit);
        let mut stop_at_edge = closed.0.contains(&next) && !closed.0.contains(&c.tile);
        if c.hold != Some(next) && is_junction(&grid, next) && grid.road_edges(next).contains(&opposite(c.exit)) && c.t > 0.3 {
            // Only push in when the owner is itself waiting for a junction that
            // we hold: two cars blocking each other, which would never clear.
            let deadlock = |o: &Entity| c.waited > 2.0 && wants.get(o).is_some_and(|w| owner.get(w) == Some(&e));
            match owner.get(&next) {
                Some(o) if *o != e && !deadlock(o) => {
                    stop_at_edge = true;
                    c.wants = Some(next);
                    if !queued {
                        c.waited += dt;
                    }
                }
                _ => {
                    // Claim it.
                    owner.insert(next, e);
                    c.hold = Some(next);
                    c.waited = 0.0;
                }
            }
        }
        if stop_at_edge {
            let room = (1.0 - c.t) * len - c.half - 0.03;
            target = target.min((room * 4.0).max(0.0));
        }

        let accel = if target > c.cur { 1.5 } else { 8.0 };
        c.cur += (target - c.cur).clamp(-accel * dt, accel * dt);
        c.t += c.cur * dt / len;
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
            // The tile we came from is across our new entry edge.
            let left = c.tile + edge_dir(c.entry);
            c.t = 0.0;
            if c.hold == Some(c.tile) {
                c.hold = None;
            }
            c.tail = is_junction(&grid, left).then_some(left);
        }
        // Release the junction behind us once our tail is clear of it.
        if c.tail.is_some() && c.t * len > c.half + 0.05 {
            c.tail = None;
        }
        let (p, f) = car_pose(&c, &grid);
        tf.translation = p;
        tf.rotation = basis_from_dir(f);
    }
}

/// Headless traffic soak test (also used by `cargo test`): drives 60 cars round the
/// demo town for ~64 s and fails on any overlap or gridlock.
pub mod soak {
    use super::*;
    use crate::demo::build_demo;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// Do two car footprints (oriented rectangles in the XZ plane) overlap?
    fn overlap(a: (Vec3, Vec3, f32), b: (Vec3, Vec3, f32)) -> bool {
        let w = 0.055;
        let rect = |(p, f, h): (Vec3, Vec3, f32)| {
            let f = Vec2::new(f.x, f.z).normalize();
            let r = Vec2::new(-f.y, f.x);
            let c = Vec2::new(p.x, p.z);
            [c + f * h + r * w, c + f * h - r * w, c - f * h - r * w, c - f * h + r * w]
        };
        let (ra, rb) = (rect(a), rect(b));
        // Separating axis test on both rectangles' edge normals.
        for poly in [&ra, &rb] {
            for i in 0..4 {
                let e = poly[(i + 1) % 4] - poly[i];
                let axis = Vec2::new(-e.y, e.x);
                let pa = ra.iter().map(|v| v.dot(axis));
                let pb = rb.iter().map(|v| v.dot(axis));
                let (amin, amax) = pa.fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
                let (bmin, bmax) = pb.fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)));
                if amax < bmin || bmax < amin {
                    return false;
                }
            }
        }
        true
    }

    pub fn run() -> Result<f32, String> {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(16)))
            .init_resource::<Sim>()
            .init_resource::<ClosedCrossings>()
            .insert_resource(Grid::default())
            .insert_resource(Trains::default())
            .add_systems(Update, (update_crossings, drive_cars).chain());
        {
            let world = app.world_mut();
            let mut grid = world.remove_resource::<Grid>().unwrap();
            let mut trains = world.remove_resource::<Trains>().unwrap();
            build_demo(&mut grid, &mut trains);
            // Spread 60 cars of mixed sizes over non-junction road tiles.
            let mut placed: Vec<IVec2> = vec![];
            let mut i = 0;
            'outer: for z in 0..GRID {
                for x in 0..GRID {
                    let t = IVec2::new(x, z);
                    let edges = grid.road_edges(t);
                    if grid.get(t).unwrap().content != Content::Road || edges.len() != 2 {
                        continue;
                    }
                    if placed.iter().any(|p| (*p - t).abs().max_element() <= 1) || (x + z) % 3 != 0 {
                        continue;
                    }
                    placed.push(t);
                    let half = [0.11, 0.12, 0.2, 0.18][i % 4];
                    let speed = 0.7 + (i % 5) as f32 * 0.1;
                    world.spawn((Transform::default(), RoadCar::new(t, edges[0], edges[1], speed, half)));
                    i += 1;
                    if i == 60 {
                        break 'outer;
                    }
                }
            }
            world.insert_resource(grid);
            world.insert_resource(trains);
        }
        app.update();
        let mut travelled = 0.0;
        let mut last: HashMap<Entity, Vec3> = HashMap::new();
        for frame in 0..4000 {
            app.update();
            let world = app.world_mut();
            let mut q = world.query::<(Entity, &RoadCar, &Transform)>();
            let cars: Vec<(Entity, (Vec3, Vec3, f32))> = q.iter(world).map(|(e, c, tf)| (e, (tf.translation, tf.rotation * Vec3::Z, c.half))).collect();
            let info: HashMap<Entity, String> = q
                .iter(world)
                .map(|(e, c, _)| (e, format!("tile {} entry {} exit {} t {:.2} v {:.2} hold {:?} tail {:?} wait {:.1}", c.tile, c.entry, c.exit, c.t, c.cur, c.hold, c.tail, c.waited)))
                .collect();
            if cars.len() != 60 {
                return Err(format!("{} cars left, expected 60", cars.len()));
            }
            for i in 0..cars.len() {
                for j in i + 1..cars.len() {
                    if overlap(cars[i].1, cars[j].1) {
                        return Err(format!("cars collided at frame {frame}:\n  {}\n  {}", info[&cars[i].0], info[&cars[j].0]));
                    }
                }
                if let Some(prev) = last.insert(cars[i].0, cars[i].1 .0) {
                    travelled += (cars[i].1 .0 - prev).length();
                }
            }
        }
        // ~64 s of driving at ~1 tile/s for 60 cars; demand a good share of that, so no gridlock.
        if travelled < 60.0 * 64.0 * 0.4 {
            return Err(format!("traffic stalled: only {travelled:.0} tiles driven in total"));
        }
        Ok(travelled)
    }
}

#[test]
fn road_cars_never_overlap_and_keep_moving() {
    soak::run().unwrap();
}