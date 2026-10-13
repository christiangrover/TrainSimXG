//! People at stations: they wait on the platforms, and when a train stops
//! some passengers get off and walk into the station while the people
//! waiting walk to the carriage doors and get on.

use crate::grid::*;
use crate::materials::{meshes_of, spawn_parts, Materials};
use crate::mesh::{rgb, Geo, Mat};
use crate::models_vehicle::seat_positions;
use crate::services::PeopleBusy;
use crate::trains::*;
use bevy::prelude::*;
use rand::Rng;
use std::collections::HashMap;

pub struct PeoplePlugin;

impl Plugin for PeoplePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Stops>().add_systems(Update, (populate_stations, train_stops, walk_people).chain());
    }
}

/// Walking pace (tiles per second; about a brisk walk at model scale).
const WALK: f32 = 0.2;
/// Platform surface height above the track level.
const PLATFORM: f32 = 0.122;
const MAX_PEOPLE: usize = 260;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WalkState {
    /// Walking out of the station building to a spot on the platform.
    Arriving,
    Waiting,
    /// Walking to a carriage door to get on.
    Boarding,
    /// Got off a train; walking into the station building.
    Leaving,
}

#[derive(Component)]
pub struct Walker {
    pub state: WalkState,
    target: Vec3,
    /// The station tile this person belongs to.
    home: IVec2,
    /// The car they're getting on.
    board: Option<(u32, usize)>,
    /// Seconds to wait before appearing / setting off.
    delay: f32,
    idle: f32,
    phase: f32,
    from: Option<u32>,
}

#[derive(Component)]
struct Leg {
    side: f32,
}

/// Trains currently stopped at a station (so each stop is handled once).
#[derive(Resource, Default)]
struct Stops {
    /// Train -> when it pulled in (seconds).
    current: HashMap<u32, f32>,
    timer: f32,
}

/// A station tile's platform: (centre at platform height, rotation; platform on local +X).
fn platform(grid: &Grid, t: IVec2) -> Option<(Vec3, Quat)> {
    let tr = grid.track(t).filter(|tr| tr.kind == TrackKind::Station)?;
    Some((tile_center(t) + Vec3::Y * (tr.level as f32 * LEVEL_H + PLATFORM), rot_quat(tr.rot)))
}

fn on_platform(frame: (Vec3, Quat), x: f32, z: f32) -> Vec3 {
    frame.0 + frame.1 * Vec3::new(x, 0.0, z)
}

/// Where people come out of / go into the station building (the platform's ticket office).
fn office_door(grid: &Grid, t: IVec2) -> Vec3 {
    let mid = grid.platform_middle(t);
    platform(grid, mid).map(|f| on_platform(f, 0.33, 0.0)).unwrap_or(tile_center(t))
}

fn waiting_spot(frame: (Vec3, Quat), rng: &mut impl Rng) -> Vec3 {
    on_platform(frame, rng.gen_range(0.245..0.32), rng.gen_range(-0.44..0.44))
}

/// Body (no legs) for a walker; legs are separate so they can swing.
fn body_mesh(v: usize) -> Geo {
    let mut m = Geo::new();
    let shirt = [rgb(0.85, 0.2, 0.2), rgb(0.2, 0.4, 0.85), rgb(0.95, 0.8, 0.25), rgb(0.3, 0.65, 0.35), rgb(0.6, 0.35, 0.7), rgb(0.95, 0.95, 0.95)][v % 6];
    let skin = [rgb(0.95, 0.78, 0.62), rgb(0.75, 0.55, 0.4), rgb(0.5, 0.35, 0.25)][v % 3];
    let hair = [rgb(0.25, 0.15, 0.08), rgb(0.85, 0.7, 0.35), rgb(0.1, 0.1, 0.1), rgb(0.55, 0.3, 0.15)][v % 4];
    m.mat(Mat::Plain);
    m.aabb(Vec3::new(-0.033, 0.08, -0.02), Vec3::new(0.033, 0.155, 0.02), shirt);
    for x in [-0.043f32, 0.043] {
        m.aabb(Vec3::new(x - 0.009, 0.09, -0.01), Vec3::new(x + 0.009, 0.15, 0.01), shirt);
    }
    m.sphere(Vec3::new(0.0, 0.183, 0.0), 0.027, skin);
    m.sphere(Vec3::new(0.0, 0.195, -0.004), 0.025, hair);
    // Some carry a suitcase.
    if v % 3 == 0 {
        m.aabb(Vec3::new(0.05, 0.03, -0.025), Vec3::new(0.065, 0.075, 0.025), rgb(0.45, 0.25, 0.15));
    }
    m
}

fn leg_mesh(v: usize) -> Geo {
    let mut m = Geo::new();
    let trousers = [rgb(0.2, 0.22, 0.35), rgb(0.3, 0.25, 0.2), rgb(0.15, 0.15, 0.15)][v % 3];
    m.mat(Mat::Plain);
    m.aabb(Vec3::new(-0.011, -0.08, -0.011), Vec3::new(0.011, 0.0, 0.011), trousers);
    m.aabb(Vec3::new(-0.012, -0.08, -0.012), Vec3::new(0.012, -0.07, 0.018), rgb(0.1, 0.08, 0.06));
    m
}

type Parts = Vec<(Mat, Handle<Mesh>)>;

#[derive(Default)]
struct PeopleMeshes(Vec<(Parts, Parts)>);

fn spawn_walker(commands: &mut Commands, mats: &Materials, meshes: &PeopleMeshes, at: Vec3, walker: Walker, rng: &mut impl Rng) {
    let (body, leg) = &meshes.0[rng.gen_range(0..meshes.0.len())];
    let hidden = walker.delay > 0.0;
    let e = spawn_parts(commands, mats, body, Transform::from_translation(at));
    for side in [-1.0f32, 1.0] {
        let l = spawn_parts(commands, mats, leg, Transform::from_xyz(side * 0.016, 0.08, 0.0));
        commands.entity(l).insert(Leg { side });
        commands.entity(e).add_child(l);
    }
    commands.entity(e).insert((walker, if hidden { Visibility::Hidden } else { Visibility::Inherited }));
}

/// Keeps a few people waiting on every platform.
#[allow(clippy::too_many_arguments)]
fn populate_stations(
    mut commands: Commands,
    time: Res<Time>,
    grid: Res<Grid>,
    mats: Option<Res<Materials>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut people: Local<PeopleMeshes>,
    mut stations: Local<(u32, bool, Vec<IVec2>)>,
    mut timer: Local<f32>,
    walkers: Query<(Entity, &Walker)>,
) {
    let Some(mats) = mats else { return };
    if people.0.is_empty() {
        people.0 = (0..12).map(|v| (meshes_of(&mut meshes, body_mesh(v)), meshes_of(&mut meshes, leg_mesh(v)))).collect();
    }
    // Rescan for stations when the layout changes.
    if !stations.1 || stations.0 != grid.generation || grid.is_changed() {
        stations.0 = grid.generation;
        stations.1 = true;
        stations.2 = (0..GRID * GRID).map(|i| IVec2::new(i % GRID, i / GRID)).filter(|t| platform(&grid, *t).is_some()).collect();
        // People on a platform that's gone, go too.
        for (e, w) in &walkers {
            if platform(&grid, w.home).is_none() {
                commands.entity(e).despawn();
            }
        }
    }
    *timer -= time.delta_secs();
    if *timer > 0.0 {
        return;
    }
    *timer = 0.7;
    let total = walkers.iter().count();
    debug!("{} people about, {} stations", total, stations.2.len());
    let mut rng = rand::thread_rng();
    let mut count: HashMap<IVec2, usize> = HashMap::new();
    for (_, w) in &walkers {
        if matches!(w.state, WalkState::Arriving | WalkState::Waiting) {
            *count.entry(w.home).or_default() += 1;
        }
    }
    // One new arrival per check, at the station with the fewest people waiting.
    let want = |t: &IVec2| 2 + ((t.x * 7 + t.y * 3).rem_euclid(3)) as usize;
    if let Some(&t) = stations.2.iter().filter(|t| count.get(t).copied().unwrap_or(0) < want(t)).min_by_key(|t| count.get(t).copied().unwrap_or(0)) {
        if total < MAX_PEOPLE {
            let frame = platform(&grid, t).unwrap();
            let w = Walker { state: WalkState::Arriving, target: waiting_spot(frame, &mut rng), home: t, board: None, delay: 0.0, idle: 0.0, phase: 0.0, from: None };
            spawn_walker(&mut commands, &mats, &people, office_door(&grid, t), w, &mut rng);
        }
    }
}

/// The platform-side door positions of a car stopped at a station, with the
/// station tile each one opens onto.
fn doors(grid: &Grid, car: &Car) -> Vec<(Vec3, IVec2)> {
    let tf = car_transform(grid, car);
    let hl = if car.kind == crate::models_vehicle::CarKind::Caboose { 0.27 } else { 0.34 };
    let mut out = vec![];
    for z in [-hl, hl] {
        for side in [-1.0f32, 1.0] {
            let door = tf.transform_point(Vec3::new(side * 0.16, 0.0, z));
            let t = world_to_tile(door);
            if let Some(frame) = platform(grid, t) {
                // Only the side the platform is on, standing just on the platform edge.
                let out_dir = frame.1 * Vec3::X;
                if (tf.rotation * Vec3::X * side).dot(out_dir) > 0.5 {
                    let p = door + out_dir * 0.08;
                    out.push((Vec3::new(p.x, frame.0.y, p.z), t));
                }
            }
        }
    }
    out
}

/// When a train stops at a station: people get off, then people get on.
#[allow(clippy::too_many_arguments)]
fn train_stops(
    mut commands: Commands,
    grid: Res<Grid>,
    mats: Option<Res<Materials>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut people: Local<PeopleMeshes>,
    mut stops: ResMut<Stops>,
    mut trains: ResMut<Trains>,
    mut busy: ResMut<PeopleBusy>,
    mut walkers: Query<(Entity, &mut Walker)>,
    time: Res<Time>,
) {
    let Some(mats) = mats else { return };
    if people.0.is_empty() {
        people.0 = (0..12).map(|v| (meshes_of(&mut meshes, body_mesh(v)), meshes_of(&mut meshes, leg_mesh(v)))).collect();
    }
    let mut rng = rand::thread_rng();
    stops.timer -= time.delta_secs();
    let check_boarding = stops.timer <= 0.0;
    if check_boarding {
        stops.timer = 0.4;
    }
    // Trains that have left: anyone still heading for them goes back to waiting.
    let moving: Vec<u32> = trains.list.iter().filter(|t| t.speed.abs() > 0.05).map(|t| t.id).collect();
    for id in &moving {
        stops.current.remove(id);
    }
    for (_, mut w) in &mut walkers {
        if let Some((id, _)) = w.board {
            if moving.contains(&id) || trains.get(id).is_none() {
                w.board = None;
                w.state = WalkState::Waiting;
                w.idle = 0.0;
            }
        }
    }
    for train in trains.list.iter_mut() {
        if train.speed.abs() > 0.05 {
            continue;
        }
        let seated: Vec<usize> = (0..train.cars.len()).filter(|&i| train.cars[i].kind.carries_passengers()).collect();
        let at: Vec<(usize, Vec<(Vec3, IVec2)>)> = seated.iter().map(|&i| (i, doors(&grid, &train.cars[i]))).filter(|(_, d)| !d.is_empty()).collect();
        if at.is_empty() {
            continue;
        }
        // Just arrived: some passengers get off.
        if !stops.current.contains_key(&train.id) {
            debug!("{} stopped with {} doors at platform {}", train.name, at.iter().map(|a| a.1.len()).sum::<usize>(), at[0].1[0].1);
            stops.current.insert(train.id, time.elapsed_secs());
            let mut n = 0;
            for (i, ds) in &at {
                let car = &mut train.cars[*i];
                let off = ((car.passengers as f32) * rng.gen_range(0.3..0.7)).round() as u8;
                car.passengers -= off;
                debug!("{} people get off {} car {i}", off, train.name);
                for _ in 0..off {
                    let (door, t) = ds[rng.gen_range(0..ds.len())];
                    let w = Walker { state: WalkState::Leaving, target: office_door(&grid, t), home: t, board: None, delay: 0.3 + n as f32 * 0.45, idle: 0.0, phase: 0.0, from: Some(train.id) };
                    spawn_walker(&mut commands, &mats, &people, door, w, &mut rng);
                    n += 1;
                }
            }
        }
        // People waiting nearby head for the doors (while there are seats), for
        // a little while after the train pulls in - then it's time to go.
        let boarding_open = stops.current.get(&train.id).is_some_and(|t0| time.elapsed_secs() - t0 < 7.0);
        if check_boarding && boarding_open {
            for (i, ds) in &at {
                let cap = seat_positions(train.cars[*i].kind).len() as i32;
                let heading = walkers.iter().filter(|(_, w)| w.board == Some((train.id, *i))).count() as i32;
                let mut room = cap - train.cars[*i].passengers as i32 - heading;
                for (_, mut w) in &mut walkers {
                    if room <= 0 {
                        break;
                    }
                    let near = ds.iter().any(|(_, t)| (*t - w.home).abs().max_element() <= 1);
                    if w.state == WalkState::Waiting && near && w.delay <= 0.0 {
                        let (door, _) = ds.iter().min_by(|a, b| a.0.distance(w.target).total_cmp(&b.0.distance(w.target))).unwrap();
                        w.state = WalkState::Boarding;
                        w.board = Some((train.id, *i));
                        w.target = *door;
                        room -= 1;
                    }
                }
            }
        }
    }
    // Trains wait while people are getting on or off.
    busy.0.clear();
    for (_, w) in &walkers {
        if let Some((id, _)) = w.board {
            busy.0.insert(id);
        }
        if let (Some(id), true) = (w.from, w.delay > 0.0) {
            busy.0.insert(id);
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn walk_people(
    mut commands: Commands,
    time: Res<Time>,
    sim: Res<Sim>,
    grid: Res<Grid>,
    mut trains: ResMut<Trains>,
    mut walkers: Query<(Entity, &mut Walker, &mut Transform, &mut Visibility, &Children)>,
    mut legs: Query<(&Leg, &mut Transform), Without<Walker>>,
) {
    if !sim.running {
        return;
    }
    let dt = time.delta_secs().min(0.05) * sim.time_scale;
    let mut rng = rand::thread_rng();
    for (e, mut w, mut tf, mut vis, children) in &mut walkers {
        if w.delay > 0.0 {
            w.delay -= dt;
            if w.delay <= 0.0 {
                *vis = Visibility::Inherited;
            }
            continue;
        }
        let to = (w.target - tf.translation).with_y(0.0);
        let dist = to.length();
        let walking = dist > 0.01;
        if walking {
            let step = (WALK * dt).min(dist);
            let dir = to / dist;
            tf.translation += dir * step;
            tf.rotation = Quat::from_rotation_y(dir.x.atan2(dir.z));
            w.phase += step * 55.0;
        } else {
            w.phase = 0.0;
        }
        tf.translation.y = w.target.y + if walking { (w.phase * 2.0).sin().abs() * 0.006 } else { 0.0 };
        // Swing the legs.
        for c in children.iter() {
            if let Ok((leg, mut ltf)) = legs.get_mut(c) {
                ltf.rotation = Quat::from_rotation_x(w.phase.sin() * 0.55 * leg.side);
            }
        }
        if walking {
            continue;
        }
        match w.state {
            WalkState::Arriving => {
                w.state = WalkState::Waiting;
                w.idle = rng.gen_range(5.0..15.0);
                // Face the track.
                if let Some(frame) = platform(&grid, w.home) {
                    let d = frame.1 * Vec3::NEG_X;
                    tf.rotation = Quat::from_rotation_y(d.x.atan2(d.z));
                }
            }
            WalkState::Waiting => {
                w.idle -= dt;
                if w.idle <= 0.0 {
                    if let Some(frame) = platform(&grid, w.home) {
                        w.target = waiting_spot(frame, &mut rng);
                        w.state = WalkState::Arriving;
                    }
                }
            }
            WalkState::Boarding => {
                // Step aboard (if the train's still here and there's a seat).
                let boarded = w.board.and_then(|(id, i)| trains.get_mut(id).and_then(|t| t.cars.get_mut(i))).is_some_and(|car| {
                    let cap = seat_positions(car.kind).len() as u8;
                    if car.passengers < cap {
                        car.passengers += 1;
                        true
                    } else {
                        false
                    }
                });
                if boarded {
                    debug!("someone boards train {:?}", w.board);
                    commands.entity(e).despawn();
                } else {
                    w.board = None;
                    w.state = WalkState::Waiting;
                    w.idle = 1.0;
                }
            }
            WalkState::Leaving => {
                commands.entity(e).despawn();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::build_demo;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// The demo's passenger train should stop with its coach doors at a platform.
    #[test]
    fn passenger_train_stops_at_a_platform() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(crate::services::ServicesPlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(20)))
            .init_resource::<Sim>()
            .insert_resource(Grid::default())
            .insert_resource(Trains::default())
            .add_systems(Update, crate::trains::run_trains);
        {
            let world = app.world_mut();
            let mut grid = world.remove_resource::<Grid>().unwrap();
            let mut trains = world.remove_resource::<Trains>().unwrap();
            build_demo(&mut grid, &mut trains);
            world.insert_resource(grid);
            world.insert_resource(trains);
        }
        let mut stops = vec![];
        let mut was_moving = true;
        for frame in 0..(240 * 50) {
            app.update();
            let world = app.world();
            let grid = world.resource::<Grid>();
            let t = &world.resource::<Trains>().list[0];
            let stopped = t.speed.abs() < 0.01;
            if stopped && was_moving {
                let d: usize = t.cars.iter().filter(|c| c.kind.carries_passengers()).map(|c| doors(grid, c).len()).sum();
                let tiles: Vec<IVec2> = t.cars.iter().map(|c| c.pos.tile).collect();
                stops.push((frame as f32 * 0.02, d, t.status.clone(), tiles));
            }
            was_moving = !stopped;
        }
        for s in &stops {
            eprintln!("stop at {:.0}s: {} doors at a platform, status '{}', cars on {:?}", s.0, s.1, s.2, s.3);
        }
        assert!(stops.iter().any(|s| s.1 > 0), "never stopped with coach doors at a platform");
    }
}