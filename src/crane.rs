//! Port cranes: swing the jib, run the trolley, lower the hook (logs) or the
//! grab bucket (rocks), and load the cars of a train standing alongside.
//! When nobody is at the controls, the crane loads stopped trains by itself.

use crate::camera::{CamMode, CamState};
use crate::grid::*;
use crate::materials::{meshes_of, spawn_parts, Materials};
use crate::mesh::Geo;
use crate::models_tile::*;
use crate::models_vehicle::CarKind;
use crate::services::{CraneBusy, Depots};
use crate::sounds::{SoundQueue, Sfx};
use crate::trains::*;
use crate::ui::UiCapture;
use bevy::prelude::*;
use std::collections::HashMap;
use std::f32::consts::{PI, TAU};

pub struct CranePlugin;

impl Plugin for CranePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Cranes>()
            .init_resource::<CraneInput>()
            .add_systems(Update, (sync_cranes, operate_cranes, place_crane_parts).chain().after(crate::services::find_depots));
    }
}

pub const MIN_REACH: f32 = 0.35;
pub const MAX_REACH: f32 = 2.6;
pub const MAX_HOOK: f32 = CRANE_TOP - 0.2;
const TRAVEL: f32 = 0.95;
const LOG_LOAD: f32 = 1.0 / crate::models_vehicle::LOGS as f32;
const ROCK_LOAD: f32 = 0.25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Hook,
    Bucket,
}

impl Tool {
    pub fn cargo(self) -> CargoKind {
        match self {
            Tool::Hook => CargoKind::Logs,
            Tool::Bucket => CargoKind::Rocks,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Step {
    Idle,
    ToPile,
    LowerPile,
    RaisePile,
    ToCar,
    LowerCar,
    RaiseCar,
    ReturnLoad,
}

pub struct CraneState {
    pub tile: IVec2,
    /// Direction the jib points (0 = +Z, increasing anticlockwise from above).
    pub yaw: f32,
    pub reach: f32,
    /// Hook height above the quay.
    pub hook: f32,
    pub tool: Tool,
    pub carrying: Option<CargoKind>,
    /// A short message for the crane driver.
    pub message: String,
    step: Step,
    target: Option<(u32, usize)>,
    think: f32,
}

#[derive(Resource, Default)]
pub struct Cranes {
    pub map: HashMap<IVec2, CraneState>,
    /// The crane shown in the crane view.
    pub active: Option<IVec2>,
}

/// Controls for the crane being driven, from the keyboard or the on-screen buttons.
#[derive(Resource, Default)]
pub struct CraneInput {
    pub turn: f32,
    pub reach: f32,
    pub hoist: f32,
    pub action: bool,
    pub switch_tool: bool,
}

/// Port tile -> (world origin at quay level, port rotation).
fn crane_frame(grid: &Grid, t: IVec2) -> (Vec3, u8) {
    let tile = grid.get(t).copied().unwrap_or_default();
    let rot = match tile.content {
        Content::Decor { rot, .. } => rot,
        _ => 0,
    };
    (tile_center(t) + Vec3::Y * (tile.surface_y() + 0.05), rot)
}

fn dir(yaw: f32) -> Vec3 {
    Vec3::new(yaw.sin(), 0.0, yaw.cos())
}

pub fn hook_world(grid: &Grid, c: &CraneState) -> Vec3 {
    let (o, _) = crane_frame(grid, c.tile);
    o + dir(c.yaw) * c.reach + Vec3::Y * c.hook
}

fn pile_world(grid: &Grid, tile: IVec2, kind: CargoKind) -> Vec3 {
    let (o, rot) = crane_frame(grid, tile);
    o + rot_quat(rot) * if kind == CargoKind::Logs { LOG_PILE } else { ROCK_PILE }
}

/// yaw and reach that put the hook over a point.
fn aim(origin: Vec3, p: Vec3) -> (f32, f32) {
    let d = p - origin;
    (d.x.atan2(d.z), Vec2::new(d.x, d.z).length().clamp(MIN_REACH, MAX_REACH))
}

fn wrap(a: f32) -> f32 {
    (a + PI).rem_euclid(TAU) - PI
}

fn approach(v: &mut f32, goal: f32, rate: f32) -> bool {
    *v += (goal - *v).clamp(-rate, rate);
    (*v - goal).abs() < 0.01
}

fn approach_angle(v: &mut f32, goal: f32, rate: f32) -> bool {
    let d = wrap(goal - *v);
    *v = wrap(*v + d.clamp(-rate, rate));
    d.abs() < 0.01
}

/// The car (train id, index) hanging under the hook, if any, and its height.
fn car_under(grid: &Grid, trains: &Trains, hook: Vec3) -> Option<(u32, usize, f32)> {
    for t in &trains.list {
        for (i, c) in t.cars.iter().enumerate() {
            let tf = car_transform(grid, c);
            let d = hook - tf.translation;
            let fwd = tf.rotation * Vec3::Z;
            let along = d.with_y(0.0).dot(fwd).abs();
            let side = (d.with_y(0.0) - fwd * d.with_y(0.0).dot(fwd)).length();
            if along < 0.36 && side < 0.17 {
                return Some((t.id, i, tf.translation.y));
            }
        }
    }
    None
}

/// Lets go of whatever the crane carries: onto a car, back on the pile, or (oops) on the ground.
fn release(grid: &Grid, trains: &mut Trains, c: &mut CraneState, sounds: &mut SoundQueue) {
    let Some(kind) = c.carrying else { return };
    let hook = hook_world(grid, c);
    let amount = if kind == CargoKind::Logs { LOG_LOAD } else { ROCK_LOAD };
    if let Some((id, i, y)) = car_under(grid, trains, hook) {
        if hook.y - y < 0.75 {
            let car = &mut trains.get_mut(id).unwrap().cars[i];
            if car.load(kind, amount) {
                c.message = format!("Loaded {}!", kind.label());
                c.carrying = None;
                sounds.play(Sfx::Thud, 0.8);
                return;
            }
            c.message = match car.kind {
                CarKind::LogCar if kind == CargoKind::Rocks => "Rocks go in the hopper, not the log car!".into(),
                CarKind::Hopper if kind == CargoKind::Logs => "Logs go on the log car, not the hopper!".into(),
                CarKind::LogCar | CarKind::Hopper => "That car is full!".into(),
                _ => format!("That car can't carry {}.", kind.label()),
            };
            return;
        }
        c.message = "Lower it down onto the car first.".into();
        return;
    }
    let pile = pile_world(grid, c.tile, kind);
    if Vec2::new(hook.x - pile.x, hook.z - pile.z).length() < 0.3 {
        c.message = format!("Put the {} back on the pile.", kind.label());
        sounds.play(Sfx::Clank, 0.5);
    } else {
        c.message = format!("Oops - dropped the {}!", kind.label());
        sounds.play(Sfx::Thud, 0.7);
    }
    c.carrying = None;
}

/// Picks up from the pile under the hook, if the right tool is fitted and it's low enough.
fn grab(grid: &Grid, c: &mut CraneState, sounds: &mut SoundQueue) -> bool {
    let kind = c.tool.cargo();
    let hook = hook_world(grid, c);
    let pile = pile_world(grid, c.tile, kind);
    let close = Vec2::new(hook.x - pile.x, hook.z - pile.z).length() < 0.25;
    if close && c.hook < 0.4 {
        c.carrying = Some(kind);
        c.message = format!("Got some {}! Now swing over a car.", kind.label());
        sounds.play(Sfx::Clank, 0.6);
        return true;
    }
    let other = pile_world(grid, c.tile, if kind == CargoKind::Logs { CargoKind::Rocks } else { CargoKind::Logs });
    c.message = if Vec2::new(hook.x - other.x, hook.z - other.z).length() < 0.25 {
        format!("Switch to the {} for that pile (Tab).", if kind == CargoKind::Logs { "bucket" } else { "hook" })
    } else if close {
        "Lower the hook further (F).".into()
    } else {
        format!("Swing the hook over the {} pile.", kind.label())
    };
    false
}

pub fn crane_eye(grid: &Grid, c: &CraneState) -> (Vec3, Vec3) {
    let (o, _) = crane_frame(grid, c.tile);
    let q = Quat::from_rotation_y(c.yaw);
    (o + Vec3::Y * CRANE_TOP + q * Vec3::new(0.11, 0.14, 0.1), hook_world(grid, c))
}

fn sync_cranes(depots: Res<Depots>, grid: Res<Grid>, mut cranes: ResMut<Cranes>) {
    if !depots.is_changed() {
        return;
    }
    cranes.map.retain(|t, _| depots.ports.contains(t));
    for &t in &depots.ports {
        cranes.map.entry(t).or_insert_with(|| {
            let (_, rot) = crane_frame(&grid, t);
            // Start pointing inland, over where a train would be.
            let inland = rot_quat(rot) * Vec3::NEG_Z;
            CraneState {
                tile: t,
                yaw: inland.x.atan2(inland.z),
                reach: 1.0,
                hook: 0.8,
                tool: Tool::Hook,
                carrying: None,
                message: String::new(),
                step: Step::Idle,
                target: None,
                think: 0.0,
            }
        });
    }
    if cranes.active.is_none_or(|a| !cranes.map.contains_key(&a)) {
        cranes.active = cranes.map.keys().min_by_key(|t| (t.y, t.x)).copied();
    }
}

#[allow(clippy::too_many_arguments)]
fn operate_cranes(
    time: Res<Time>,
    sim: Res<Sim>,
    keys: Res<ButtonInput<KeyCode>>,
    ui: Res<UiCapture>,
    cam: Res<CamState>,
    grid: Res<Grid>,
    mut input: ResMut<CraneInput>,
    mut cranes: ResMut<Cranes>,
    mut trains: ResMut<Trains>,
    mut busy: ResMut<CraneBusy>,
    mut sounds: ResMut<SoundQueue>,
) {
    let dt = time.delta_secs().min(0.05);
    if cam.mode == CamMode::Crane && !ui.keyboard && keys.just_pressed(KeyCode::KeyC) && cranes.map.len() > 1 {
        let mut all: Vec<IVec2> = cranes.map.keys().copied().collect();
        all.sort_by_key(|t| (t.y, t.x));
        let i = cranes.active.and_then(|a| all.iter().position(|t| *t == a)).map(|i| (i + 1) % all.len()).unwrap_or(0);
        cranes.active = Some(all[i]);
    }
    let driving = if cam.mode == CamMode::Crane { cranes.active } else { None };
    if driving.is_some() && !ui.keyboard {
        let k = |a: KeyCode, b: KeyCode| (keys.pressed(a) || keys.pressed(b)) as i32 as f32;
        input.turn += k(KeyCode::KeyA, KeyCode::ArrowLeft) - k(KeyCode::KeyD, KeyCode::ArrowRight);
        input.reach += k(KeyCode::KeyW, KeyCode::ArrowUp) - k(KeyCode::KeyS, KeyCode::ArrowDown);
        input.hoist += keys.pressed(KeyCode::KeyR) as i32 as f32 - keys.pressed(KeyCode::KeyF) as i32 as f32;
        input.action |= keys.just_pressed(KeyCode::Space);
        input.switch_tool |= keys.just_pressed(KeyCode::Tab);
    }
    busy.0.clear();
    let tiles: Vec<IVec2> = cranes.map.keys().copied().collect();
    for t in tiles {
        let c = cranes.map.get_mut(&t).unwrap();
        let (origin, _) = crane_frame(&grid, t);
        if driving == Some(t) {
            // The player is at the controls.
            c.step = Step::Idle;
            c.target = None;
            c.yaw = wrap(c.yaw + input.turn.clamp(-1.0, 1.0) * 1.0 * dt);
            c.reach = (c.reach + input.reach.clamp(-1.0, 1.0) * 0.7 * dt).clamp(MIN_REACH, MAX_REACH);
            c.hook = (c.hook + input.hoist.clamp(-1.0, 1.0) * 0.6 * dt).clamp(0.02, MAX_HOOK);
            if input.switch_tool {
                if c.carrying.is_some() {
                    c.message = "Put the load down before changing tools.".into();
                } else {
                    c.tool = if c.tool == Tool::Hook { Tool::Bucket } else { Tool::Hook };
                    c.message = format!("Fitted the {}.", if c.tool == Tool::Hook { "hook (for logs)" } else { "grab bucket (for rocks)" });
                    sounds.play(Sfx::Clank, 0.4);
                }
            }
            if input.action {
                if c.carrying.is_some() {
                    release(&grid, &mut trains, c, &mut sounds);
                } else {
                    grab(&grid, c, &mut sounds);
                }
            }
            continue;
        }
        if !sim.running {
            continue;
        }
        // Automatic operation: find a stopped car that needs logs or rocks.
        let rate = TRAVEL * dt;
        if let Some((id, _)) = c.target {
            busy.0.insert(id);
        }
        let target_pos = c.target.and_then(|(id, i)| {
            let tr = trains.get(id)?;
            let car = tr.cars.get(i)?;
            (tr.speed.abs() < 0.05 && car.can_take(c.tool.cargo())).then(|| car_transform(&grid, car).translation)
        });
        match c.step {
            Step::Idle => {
                c.think -= dt;
                approach(&mut c.hook, 0.9, rate);
                if c.think > 0.0 {
                    continue;
                }
                c.think = 0.5;
                c.target = None;
                'search: for tr in &trains.list {
                    if tr.speed.abs() > 0.05 {
                        continue;
                    }
                    for (i, car) in tr.cars.iter().enumerate() {
                        for tool in [Tool::Hook, Tool::Bucket] {
                            if !car.can_take(tool.cargo()) {
                                continue;
                            }
                            let p = car_transform(&grid, car).translation;
                            let d = Vec2::new(p.x - origin.x, p.z - origin.z).length();
                            if (MIN_REACH..MAX_REACH - 0.05).contains(&d) {
                                c.target = Some((tr.id, i));
                                c.tool = tool;
                                c.step = Step::ToPile;
                                break 'search;
                            }
                        }
                    }
                }
            }
            Step::ToPile | Step::ReturnLoad => {
                let kind = c.carrying.unwrap_or(c.tool.cargo());
                let (y, r) = aim(origin, pile_world(&grid, t, kind));
                let up = approach(&mut c.hook, 0.9, rate);
                if c.hook > 0.6 {
                    let a = approach_angle(&mut c.yaw, y, rate * 1.2);
                    let b = approach(&mut c.reach, r, rate);
                    if a && b && up {
                        c.step = Step::LowerPile;
                    }
                }
            }
            Step::LowerPile => {
                if approach(&mut c.hook, 0.15, rate) {
                    if c.carrying.is_some() {
                        // Taking an unwanted load back.
                        c.carrying = None;
                        sounds.play(Sfx::Clank, 0.3);
                        c.step = Step::Idle;
                    } else {
                        grab(&grid, c, &mut sounds);
                        c.step = Step::RaisePile;
                    }
                }
            }
            Step::RaisePile => {
                if approach(&mut c.hook, 0.9, rate) {
                    c.step = Step::ToCar;
                }
            }
            Step::ToCar => match target_pos {
                Some(p) => {
                    let (y, r) = aim(origin, p);
                    let a = approach_angle(&mut c.yaw, y, rate * 1.2);
                    let b = approach(&mut c.reach, r, rate);
                    if a && b {
                        c.step = Step::LowerCar;
                    }
                }
                None => {
                    c.target = None;
                    c.step = Step::ReturnLoad;
                }
            },
            Step::LowerCar => match target_pos {
                Some(p) => {
                    if approach(&mut c.hook, p.y - origin.y + 0.5, rate) {
                        release(&grid, &mut trains, c, &mut sounds);
                        c.step = Step::RaiseCar;
                    }
                }
                None => c.step = Step::ReturnLoad,
            },
            Step::RaiseCar => {
                if approach(&mut c.hook, 0.9, rate) {
                    c.step = if c.carrying.is_some() { Step::ReturnLoad } else { Step::ToPile };
                    if target_pos.is_none() {
                        c.target = None;
                        c.step = if c.carrying.is_some() { Step::ReturnLoad } else { Step::Idle };
                    }
                }
            }
        }
    }
    *input = CraneInput::default();
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Part {
    Top,
    Trolley,
    Rope,
    Hook,
    Bucket,
    Logs,
    Rocks,
}

#[derive(Component)]
struct CranePart {
    tile: IVec2,
    part: Part,
}

#[derive(Default)]
struct CraneMeshes(Vec<(Part, Vec<(crate::mesh::Mat, Handle<Mesh>)>)>);

fn rope_mesh() -> Geo {
    let mut m = Geo::new();
    m.mat(crate::mesh::Mat::Steel);
    m.cuboid(Vec3::new(0.0, -0.5, 0.0), Vec3::new(0.003, 0.5, 0.003), Quat::IDENTITY, Color::srgb(0.15, 0.15, 0.15));
    m
}

#[allow(clippy::too_many_arguments)]
fn place_crane_parts(
    mut commands: Commands,
    grid: Res<Grid>,
    cranes: Res<Cranes>,
    mats: Option<Res<Materials>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut cache: Local<CraneMeshes>,
    mut parts: Query<(Entity, &CranePart, &mut Transform, &mut Visibility)>,
) {
    let Some(mats) = mats else { return };
    if cache.0.is_empty() {
        for (p, g) in [
            (Part::Top, crane_top_mesh()),
            (Part::Trolley, crane_trolley_mesh()),
            (Part::Rope, rope_mesh()),
            (Part::Hook, crane_hook_mesh()),
            (Part::Bucket, crane_bucket_mesh()),
            (Part::Logs, crane_load_mesh(true)),
            (Part::Rocks, crane_load_mesh(false)),
        ] {
            cache.0.push((p, meshes_of(&mut meshes, g)));
        }
    }
    let mut seen: HashMap<IVec2, usize> = HashMap::new();
    for (e, cp, mut tf, mut vis) in &mut parts {
        let Some(c) = cranes.map.get(&cp.tile) else {
            commands.entity(e).despawn();
            continue;
        };
        *seen.entry(cp.tile).or_default() += 1;
        let (o, _) = crane_frame(&grid, cp.tile);
        let q = Quat::from_rotation_y(c.yaw);
        let jib_y = o.y + CRANE_TOP + 0.05;
        let trolley = o + dir(c.yaw) * c.reach + Vec3::Y * (CRANE_TOP + 0.05);
        let hook = hook_world(&grid, c);
        let show = |on: bool| if on { Visibility::Inherited } else { Visibility::Hidden };
        let (t, v) = match cp.part {
            Part::Top => (Transform::from_translation(o + Vec3::Y * CRANE_TOP).with_rotation(q), Visibility::Inherited),
            Part::Trolley => (Transform::from_translation(trolley).with_rotation(q), Visibility::Inherited),
            Part::Rope => {
                let len = (jib_y - 0.035 - hook.y).max(0.01);
                (Transform::from_translation(trolley - Vec3::Y * 0.035).with_scale(Vec3::new(1.0, len, 1.0)), Visibility::Inherited)
            }
            Part::Hook => (Transform::from_translation(hook).with_rotation(q), show(c.tool == Tool::Hook)),
            Part::Bucket => (Transform::from_translation(hook).with_rotation(q), show(c.tool == Tool::Bucket)),
            Part::Logs => (Transform::from_translation(hook).with_rotation(q), show(c.carrying == Some(CargoKind::Logs))),
            Part::Rocks => (Transform::from_translation(hook).with_rotation(q), show(c.carrying == Some(CargoKind::Rocks))),
        };
        *tf = t;
        if *vis != v {
            *vis = v;
        }
    }
    for t in cranes.map.keys() {
        if seen.get(t).copied().unwrap_or(0) == 0 {
            for (p, m) in &cache.0 {
                let e = spawn_parts(&mut commands, &mats, m, Transform::from_translation(tile_center(*t)));
                commands.entity(e).insert(CranePart { tile: *t, part: *p });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::demo::build_demo;
    use crate::sounds::SoundQueue;
    use bevy::time::TimeUpdateStrategy;
    use std::time::Duration;

    /// The demo's freight train should stop at the port, get logs and rocks
    /// from the crane, and fill its tank car with oil at the depot.
    #[test]
    fn port_loads_the_freight_train() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(crate::services::ServicesPlugin)
            .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(20)))
            .init_resource::<Sim>()
            .init_resource::<CamState>()
            .init_resource::<UiCapture>()
            .init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<SoundQueue>()
            .init_resource::<Cranes>()
            .init_resource::<CraneInput>()
            .insert_resource(Grid::default())
            .insert_resource(Trains::default())
            .add_systems(Update, (crate::trains::run_trains, sync_cranes, operate_cranes).chain());
        {
            let world = app.world_mut();
            let mut grid = world.remove_resource::<Grid>().unwrap();
            let mut trains = world.remove_resource::<Trains>().unwrap();
            build_demo(&mut grid, &mut trains);
            world.insert_resource(grid);
            world.insert_resource(trains);
        }
        let (mut logs, mut rocks, mut oil) = (0.0f32, 0.0f32, 0.0f32);
        let mut frames = 0;
        for _ in 0..(150 * 50) {
            frames += 1;
            app.update();
            let trains = app.world().resource::<Trains>();
            for t in &trains.list {
                for c in &t.cars {
                    match c.cargo.kind {
                        CargoKind::Logs => logs = logs.max(c.cargo.amount),
                        CargoKind::Rocks => rocks = rocks.max(c.cargo.amount),
                        CargoKind::Oil => oil = oil.max(c.cargo.amount),
                        _ => {}
                    }
                }
            }
            if logs > 0.99 && rocks > 0.99 && oil > 0.99 {
                break;
            }
        }
        eprintln!("after {:.1} simulated s: logs {logs:.2}, rocks {rocks:.2}, oil {oil:.2}", frames as f32 * 0.02);
        assert!(app.world().resource::<Cranes>().map.len() == 1, "the demo has one port crane");
        assert!(logs > 0.99, "log car only got {logs:.2} logs");
        assert!(rocks > 0.99, "hopper only got {rocks:.2} rocks");
        assert!(oil > 0.99, "tank car only got {oil:.2} oil");
    }
}