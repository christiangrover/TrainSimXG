//! Trains: consists of cars rolling along the track, automatic running with
//! station stops, manual (engineer) control, and steam smoke.

use crate::grid::*;
use crate::mesh::*;
use crate::models_vehicle::*;
use crate::materials::{meshes_of, spawn_parts, Materials};
use crate::track::*;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const MAX_SPEED: f32 = 3.0;
const ACCEL: f32 = 1.2;
const GAP: f32 = 0.08;

pub struct TrainsPlugin;

impl Plugin for TrainsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Trains>()
            .init_resource::<Sim>()
            .init_resource::<CarMeshes>()
            .init_resource::<Consist>()
            .add_systems(Update, (run_trains, sync_car_visuals, update_cargo_visuals, smoke).chain());
    }
}

#[derive(Resource)]
pub struct Sim {
    pub running: bool,
    pub time_scale: f32,
}

impl Default for Sim {
    fn default() -> Self {
        Sim { running: true, time_scale: 1.0 }
    }
}

/// The consist being assembled in the train library.
#[derive(Resource)]
pub struct Consist {
    pub cars: Vec<CarKind>,
}

impl Default for Consist {
    fn default() -> Self {
        Consist { cars: vec![CarKind::SteamEngine, CarKind::Tender, CarKind::Passenger, CarKind::Dining, CarKind::Sleeper, CarKind::Caboose] }
    }
}

/// What a freight car is carrying.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum CargoKind {
    #[default]
    Empty,
    Logs,
    Rocks,
    Water,
    Oil,
}

impl CargoKind {
    pub fn label(self) -> &'static str {
        match self {
            CargoKind::Empty => "empty",
            CargoKind::Logs => "logs",
            CargoKind::Rocks => "rocks",
            CargoKind::Water => "water",
            CargoKind::Oil => "oil",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Cargo {
    pub kind: CargoKind,
    /// 0 = empty, 1 = full.
    pub amount: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Car {
    pub kind: CarKind,
    pub pos: TrackPos,
    #[serde(default)]
    pub cargo: Cargo,
}

impl Car {
    /// Can this car take (more of) the given cargo?
    pub fn can_take(&self, kind: CargoKind) -> bool {
        let fits = match self.kind {
            CarKind::LogCar => kind == CargoKind::Logs,
            CarKind::Hopper => kind == CargoKind::Rocks,
            CarKind::Tanker => matches!(kind, CargoKind::Water | CargoKind::Oil),
            _ => false,
        };
        fits && (self.cargo.kind == kind || self.cargo.amount <= 0.001) && self.cargo.amount < 0.999
    }

    /// Adds cargo, returning true if any went in.
    pub fn load(&mut self, kind: CargoKind, amount: f32) -> bool {
        if !self.can_take(kind) {
            return false;
        }
        self.cargo.kind = kind;
        self.cargo.amount = (self.cargo.amount + amount).min(1.0);
        true
    }
}

/// How an engine is powered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    Steam,
    Diesel,
    /// No engine in the train (it rolls "by magic").
    None,
}

fn full() -> f32 {
    1.0
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Train {
    pub id: u32,
    pub name: String,
    pub cars: Vec<Car>,
    pub speed: f32,
    /// Manual control: the engineer drives with the throttle.
    pub manual: bool,
    /// Throttle notch, -1.0 (full reverse) .. 1.0 (full ahead).
    pub throttle: f32,
    pub brake: bool,
    /// Automatic running: cruise speed and current direction (+1 / -1).
    pub cruise: f32,
    pub auto_dir: f32,
    #[serde(default)]
    pub dwell: f32,
    #[serde(default)]
    pub last_station: Option<IVec2>,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub wait: f32,
    /// Brake lever position, 0 (off) .. 1 (full service brake).
    #[serde(default)]
    pub brake_lever: f32,
    /// Emergency brake: stops hard and stays on until released.
    #[serde(default)]
    pub ebrake: bool,
    /// Manual fuel: fuel, coal and water run down and must be topped up.
    #[serde(default)]
    pub fuel_manual: bool,
    /// Diesel fuel, or coal in a steam engine's tender (0..1).
    #[serde(default = "full")]
    pub fuel: f32,
    /// Water in a steam engine's boiler and tender (0..1).
    #[serde(default = "full")]
    pub water: f32,
    /// How hot the steam engine's fire is (0..1).
    #[serde(default = "full")]
    pub fire: f32,
    /// Boiler pressure (0..1); this is what moves a steam engine.
    #[serde(default = "full")]
    pub pressure: f32,
    /// Seconds until the automatic fireman next shovels coal.
    #[serde(skip)]
    pub stoke_timer: f32,
}

impl Train {
    pub fn power_kind(&self) -> Power {
        match self.cars.iter().find(|c| c.kind.is_engine()).map(|c| c.kind) {
            Some(CarKind::SteamEngine) => Power::Steam,
            Some(_) => Power::Diesel,
            None => Power::None,
        }
    }

    /// 0..1: how much pulling power the engine has right now.
    pub fn power(&self) -> f32 {
        if !self.fuel_manual {
            return 1.0;
        }
        match self.power_kind() {
            Power::Steam => {
                if self.water <= 0.0 {
                    0.0
                } else {
                    self.pressure.clamp(0.0, 1.0)
                }
            }
            Power::Diesel => {
                if self.fuel > 0.0 {
                    1.0
                } else {
                    0.0
                }
            }
            Power::None => 1.0,
        }
    }

    /// The fireman throws a shovel of coal on the fire. Returns false if the tender is empty.
    pub fn shovel_coal(&mut self) -> bool {
        if self.fuel <= 0.0 {
            return false;
        }
        self.fuel = (self.fuel - 0.015).max(0.0);
        self.fire = (self.fire + 0.1).min(1.15);
        true
    }

    /// Burns fuel for one time step. work is how hard the engine is pulling (0..1).
    fn burn(&mut self, dt: f32, work: f32) {
        if !self.fuel_manual {
            self.fuel = 1.0;
            self.water = 1.0;
            self.fire = 0.9;
            self.pressure = 1.0;
            return;
        }
        match self.power_kind() {
            Power::Steam => {
                self.fire = (self.fire - dt * (0.004 + 0.02 * work)).max(0.0);
                let target = if self.water > 0.0 { self.fire.min(1.0) } else { 0.0 };
                self.pressure += (target - self.pressure) * (0.5 * dt).min(1.0);
                self.water = (self.water - dt * (0.0005 + 0.003 * work)).max(0.0);
                // Self-driving trains have their own fireman.
                if !self.manual {
                    self.stoke_timer -= dt;
                    if self.stoke_timer <= 0.0 && self.fire < 0.75 {
                        self.stoke_timer = 1.2;
                        self.shovel_coal();
                    }
                }
            }
            Power::Diesel => {
                self.fuel = (self.fuel - dt * (0.001 + 0.005 * work)).max(0.0);
                self.pressure = 1.0;
            }
            Power::None => {}
        }
    }
}

/// Lets tests run the fuel model directly.
#[cfg(test)]
pub fn burn_for_test(t: &mut Train, dt: f32) {
    let work = t.throttle.abs();
    t.burn(dt, work);
}

#[derive(Resource, Default)]
pub struct Trains {
    pub list: Vec<Train>,
    pub next_id: u32,
    pub selected: Option<u32>,
    /// Set when the list changes structurally so visuals get rebuilt.
    pub dirty: bool,
}

impl Trains {
    pub fn get(&self, id: u32) -> Option<&Train> {
        self.list.iter().find(|t| t.id == id)
    }
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Train> {
        self.list.iter_mut().find(|t| t.id == id)
    }
    pub fn selected(&self) -> Option<&Train> {
        self.selected.and_then(|id| self.get(id))
    }

    /// Puts a new train on the track with its first car at `tile`.
    pub fn spawn(&mut self, grid: &Grid, tile: IVec2, kinds: &[CarKind]) -> Result<u32, String> {
        self.spawn_heading(grid, tile, kinds, None)
    }

    /// Like [`Self::spawn`], optionally heading out through a given tile edge.
    pub fn spawn_heading(&mut self, grid: &Grid, tile: IVec2, kinds: &[CarKind], heading: Option<u8>) -> Result<u32, String> {
        if kinds.is_empty() {
            return Err("The consist is empty - add some cars in the train library.".into());
        }
        let Some(mut pos) = start_pos(grid, tile) else {
            return Err("Click on a piece of track to place the train.".into());
        };
        // Face the direction with more track ahead of us, so the cars trail behind.
        let back = pos.reversed().ahead(grid, 3.0);
        let fwd = pos.ahead(grid, 3.0);
        if (pos.world(grid).0 - fwd.world(grid).0).length() < (pos.world(grid).0 - back.world(grid).0).length() {
            pos = pos.reversed();
        }
        if let Some(h) = heading {
            if pos.entry == h {
                pos = pos.reversed();
            }
        }
        let mut cars = Vec::new();
        let mut cur = pos;
        for (i, &k) in kinds.iter().enumerate() {
            if i > 0 {
                let d = (kinds[i - 1].length() + k.length()) * 0.5 + GAP;
                cur = cur.behind(grid, d);
            }
            cars.push(Car { kind: k, pos: cur, cargo: Cargo::default() });
        }
        let id = self.next_id;
        self.next_id += 1;
        let names = ["Blue Comet", "Dixie Flyer", "Iron Horse", "Silver Arrow", "Night Owl", "Sunrise Express", "Red Rocket", "Pine Valley"];
        self.list.push(Train {
            id,
            name: format!("{} #{}", names[id as usize % names.len()], id + 1),
            cars,
            speed: 0.0,
            manual: false,
            throttle: 0.0,
            brake: false,
            cruise: 1.6,
            auto_dir: 1.0,
            dwell: 0.0,
            last_station: None,
            status: String::new(),
            wait: 0.0,
            brake_lever: 0.0,
            ebrake: false,
            fuel_manual: false,
            fuel: 1.0,
            water: 1.0,
            fire: 0.9,
            pressure: 1.0,
            stoke_timer: 0.0,
        });
        self.selected = Some(id);
        self.dirty = true;
        Ok(id)
    }

    pub fn remove(&mut self, id: u32) {
        self.list.retain(|t| t.id != id);
        if self.selected == Some(id) {
            self.selected = self.list.first().map(|t| t.id);
        }
        self.dirty = true;
    }
}

/// World transform of a car: centred between two bogies so it follows curves.
pub fn car_transform(grid: &Grid, car: &Car) -> Transform {
    let f = car.pos.ahead(grid, 0.22).world(grid).0;
    let b = car.pos.behind(grid, 0.22).world(grid).0;
    let (p, tangent) = car.pos.world(grid);
    let d = f - b;
    let dir = if d.length() > 0.05 { d } else { tangent };
    Transform::from_translation((f + b) * 0.25 + p * 0.5).with_rotation(basis_from_dir(dir))
}

fn move_train(grid: &Grid, train: &mut Train, d: f32) -> f32 {
    if d == 0.0 || train.cars.is_empty() {
        return 0.0;
    }
    let n = train.cars.len();
    let forward = d > 0.0;
    let dist = d.abs();
    let step = |p: &mut TrackPos, dist: f32| -> f32 {
        if forward {
            p.advance(grid, dist)
        } else {
            let mut r = p.reversed();
            let m = r.advance(grid, dist);
            *p = r.reversed();
            m
        }
    };
    let lead = if forward { 0 } else { n - 1 };
    let moved = step(&mut train.cars[lead].pos, dist);
    for i in 0..n {
        if i != lead {
            step(&mut train.cars[i].pos, moved);
        }
    }
    moved
}

pub fn run_trains(time: Res<Time>, sim: Res<Sim>, grid: Res<Grid>, mut trains: ResMut<Trains>) {
    // Drop trains whose track was bulldozed away.
    let before = trains.list.len();
    trains.list.retain(|t| t.cars.iter().all(|c| c.pos.valid(&grid)));
    if trains.list.len() != before {
        trains.dirty = true;
        if let Some(sel) = trains.selected {
            if trains.get(sel).is_none() {
                trains.selected = trains.list.first().map(|t| t.id);
            }
        }
    }
    if !sim.running {
        return;
    }
    let dt = time.delta_secs().min(0.05) * sim.time_scale;

    // Snapshot of every car position for collision checks.
    let g: &Grid = &grid;
    let others: Vec<(u32, Vec3)> = trains
        .list
        .iter()
        .flat_map(|t| t.cars.iter().map(move |c| (t.id, c.pos.world(g).0)))
        .collect();

    for train in trains.list.iter_mut() {
        let n = train.cars.len();
        let power = train.power();
        // Weak steam or an empty tank means less (or no) pulling power.
        let pull = if power < 0.05 { 0.0 } else { 0.25 + 0.75 * power };
        if train.ebrake {
            train.throttle = 0.0;
        }
        let mut target = if train.manual { train.throttle * MAX_SPEED * pull } else { train.auto_dir * train.cruise * pull };
        train.status.clear();
        if train.fuel_manual {
            match train.power_kind() {
                Power::Diesel if train.fuel <= 0.0 => train.status = "Out of fuel! Stop at a station or fuel depot.".into(),
                Power::Steam if train.water <= 0.0 => train.status = "Out of water! Stop at a station or water tower.".into(),
                Power::Steam if train.fuel <= 0.0 && train.fire < 0.2 => train.status = "Out of coal! Stop at a station.".into(),
                Power::Steam if train.manual && train.pressure < 0.35 => train.status = "Low steam pressure - press 6 and shovel coal!".into(),
                _ => {}
            }
        }

        if !train.manual {
            let lead = if train.auto_dir > 0.0 { train.cars[0].pos } else { train.cars[n - 1].pos.reversed() };
            // Station stops.
            if train.dwell > 0.0 {
                train.dwell -= dt;
                target = 0.0;
                train.status = "Stopped at station".into();
            } else if let Some(tr) = grid.track(lead.tile) {
                if tr.kind == TrackKind::Station && train.last_station != Some(lead.tile) {
                    if lead.t >= 0.5 {
                        train.dwell = 4.0;
                        train.last_station = Some(lead.tile);
                        train.speed = 0.0;
                        target = 0.0;
                    } else {
                        target = target.signum() * target.abs().min(0.5);
                    }
                }
            }
            if train.last_station.is_some_and(|s| (s - lead.tile).abs().max_element() > 3) {
                train.last_station = None;
            }
            // Don't run into other trains.
            let (lp, ld) = lead.world(&grid);
            let probe = lead.ahead(&grid, 0.9).world(&grid).0;
            let blocked = others.iter().any(|(id, p)| {
                *id != train.id && ((*p - lp).dot(ld) > 0.0 && (*p - lp).length() < 1.3 || (*p - probe).length() < 0.5)
            });
            if blocked {
                target = 0.0;
                train.status = "Waiting for the line to clear".into();
                train.wait += dt;
                // Two trains nose to nose would wait forever: back away.
                if train.wait > 6.0 {
                    train.wait = 0.0;
                    train.auto_dir = -train.auto_dir;
                }
            } else {
                train.wait = 0.0;
            }
        }

        // With no power, the train just coasts (gently slowing).
        if pull == 0.0 && target == 0.0 && !train.brake && train.brake_lever < 0.01 && !train.ebrake && train.manual {
            target = train.speed * 0.98;
        }
        let braking = train.brake_lever.max(if train.brake { 1.0 } else { 0.0 });
        if braking > 0.01 && train.manual {
            target = 0.0;
        }
        if train.ebrake {
            target = 0.0;
            train.status = "EMERGENCY BRAKE".into();
        }
        let dv = target - train.speed;
        let slowing = dv.abs() > 0.0 && dv.signum() != train.speed.signum() && train.speed.abs() > 0.0;
        let a = if train.ebrake {
            ACCEL * 6.0
        } else if slowing {
            ACCEL * (1.0 + braking * 3.0)
        } else {
            ACCEL
        };
        train.speed += dv.clamp(-a * dt, a * dt);
        let work = if train.manual { train.throttle.abs() } else { (train.speed.abs() / MAX_SPEED).max(0.2) };
        train.burn(dt, if train.speed.abs() < 0.01 && train.throttle == 0.0 { 0.0 } else { work });
        let want = train.speed * dt;
        let moved = move_train(&grid, train, want);
        if want.abs() > 1e-5 && moved + 1e-5 < want.abs() {
            train.speed = 0.0;
            if train.manual {
                train.status = "End of the line!".into();
            } else {
                train.auto_dir = -train.auto_dir;
                train.last_station = None;
            }
        }
    }
}

#[derive(Component)]
pub struct CarVisual {
    pub train: u32,
    pub index: usize,
    pub kind: CarKind,
}

type Parts = Vec<(Mat, Handle<Mesh>)>;

#[derive(Resource, Default)]
pub struct CarMeshes {
    cars: HashMap<CarKind, Parts>,
    logs: Vec<Parts>,
    rocks: Parts,
    water: Parts,
    oil: Parts,
}

impl CarMeshes {
    fn load_cargo(&mut self, meshes: &mut Assets<Mesh>) {
        if self.logs.is_empty() {
            self.logs = (0..LOGS).map(|i| meshes_of(meshes, cargo_log_mesh(i))).collect();
            self.rocks = meshes_of(meshes, cargo_rock_mesh());
            self.water = meshes_of(meshes, cargo_band_mesh(rgb(0.2, 0.45, 0.85)));
            self.oil = meshes_of(meshes, cargo_band_mesh(rgb(0.05, 0.05, 0.05)));
        }
    }
}

/// A visible piece of a car's load.
#[derive(Component)]
struct CargoVisual {
    train: u32,
    car: usize,
    part: CargoPart,
}

#[derive(Clone, Copy, PartialEq)]
enum CargoPart {
    Log(usize),
    Rocks,
    Band(CargoKind),
}

fn sync_car_visuals(
    mut commands: Commands,
    grid: Res<Grid>,
    mut trains: ResMut<Trains>,
    mut cache: ResMut<CarMeshes>,
    mut meshes: ResMut<Assets<Mesh>>,
    mats: Option<Res<Materials>>,
    mut q: Query<(Entity, &CarVisual, &mut Transform)>,
) {
    let Some(mats) = mats else { return };
    if trains.dirty {
        trains.dirty = false;
        for (e, _, _) in &q {
            commands.entity(e).despawn();
        }
        cache.load_cargo(&mut meshes);
        for t in &trains.list {
            for (i, c) in t.cars.iter().enumerate() {
                let parts = cache.cars.entry(c.kind).or_insert_with(|| meshes_of(&mut meshes, car_mesh(c.kind, default_paint(c.kind)))).clone();
                let e = spawn_parts(&mut commands, &mats, &parts, car_transform(&grid, c));
                commands.entity(e).insert(CarVisual { train: t.id, index: i, kind: c.kind });
                // Load pieces, shown or hidden as cargo comes and goes.
                let pieces: Vec<(CargoPart, &Parts, Transform)> = match c.kind {
                    CarKind::LogCar => (0..LOGS).map(|k| (CargoPart::Log(k), &cache.logs[k], Transform::IDENTITY)).collect(),
                    CarKind::Hopper => vec![(CargoPart::Rocks, &cache.rocks, Transform::from_xyz(0.0, 0.19, 0.0))],
                    CarKind::Tanker => vec![
                        (CargoPart::Band(CargoKind::Water), &cache.water, Transform::IDENTITY),
                        (CargoPart::Band(CargoKind::Oil), &cache.oil, Transform::IDENTITY),
                    ],
                    _ => vec![],
                };
                for (part, p, tf) in pieces {
                    let child = spawn_parts(&mut commands, &mats, p, tf);
                    commands.entity(child).insert((CargoVisual { train: t.id, car: i, part }, Visibility::Hidden));
                    commands.entity(e).add_child(child);
                }
            }
        }
        return;
    }
    for (_, v, mut tf) in &mut q {
        if let Some(c) = trains.get(v.train).and_then(|t| t.cars.get(v.index)) {
            *tf = car_transform(&grid, c);
        }
    }
}

fn update_cargo_visuals(trains: Res<Trains>, mut q: Query<(&CargoVisual, &mut Visibility, &mut Transform)>) {
    for (cv, mut vis, mut tf) in &mut q {
        let cargo = trains.get(cv.train).and_then(|t| t.cars.get(cv.car)).map(|c| c.cargo).unwrap_or_default();
        let show = match cv.part {
            CargoPart::Log(k) => cargo.kind == CargoKind::Logs && (cargo.amount * LOGS as f32).round() as usize > k,
            CargoPart::Rocks => {
                tf.scale = Vec3::new(1.0, cargo.amount.clamp(0.05, 1.0), 1.0);
                cargo.kind == CargoKind::Rocks && cargo.amount > 0.02
            }
            CargoPart::Band(kind) => cargo.kind == kind && cargo.amount > 0.02,
        };
        let want = if show { Visibility::Inherited } else { Visibility::Hidden };
        if *vis != want {
            *vis = want;
        }
    }
}
#[derive(Component)]
struct Puff {
    age: f32,
    life: f32,
    grow: f32,
    vel: Vec3,
}

/// Steam engines puff white smoke; diesels give dark exhaust, heavier when working hard.
#[allow(clippy::too_many_arguments)]
fn smoke(
    mut commands: Commands,
    time: Res<Time>,
    sim: Res<Sim>,
    trains: Res<Trains>,
    cars: Query<(&CarVisual, &Transform), Without<Puff>>,
    mut puffs: Query<(Entity, &mut Puff, &mut Transform)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut mats: ResMut<Assets<StandardMaterial>>,
    mut res: Local<Option<(Handle<Mesh>, Handle<StandardMaterial>, Handle<StandardMaterial>)>>,
    mut timer: Local<f32>,
) {
    let dt = time.delta_secs();
    let (mesh, steam, exhaust) = res
        .get_or_insert_with(|| {
            let puff = |c: Color| StandardMaterial { base_color: c, alpha_mode: AlphaMode::Blend, perceptual_roughness: 1.0, ..default() };
            (
                meshes.add(Sphere::new(0.05).mesh().ico(1).unwrap()),
                mats.add(puff(Color::srgba(0.95, 0.95, 0.97, 0.35))),
                mats.add(puff(Color::srgba(0.2, 0.2, 0.22, 0.3))),
            )
        })
        .clone();
    for (e, mut p, mut tf) in &mut puffs {
        p.age += dt;
        tf.translation += p.vel * dt;
        let s = 0.6 + p.age * p.grow;
        let fade = (1.0 - p.age / p.life).max(0.0);
        tf.scale = Vec3::splat(s * fade.sqrt());
        if p.age > p.life {
            commands.entity(e).despawn();
        }
    }
    if !sim.running {
        return;
    }
    *timer += dt;
    if *timer < 0.12 {
        return;
    }
    *timer = 0.0;
    for (v, tf) in &cars {
        let Some(stack) = v.kind.exhaust() else { continue };
        let Some(t) = trains.get(v.train) else { continue };
        let speed = t.speed.abs();
        let target = if t.manual { t.throttle.abs() * MAX_SPEED } else { t.cruise };
        let working = speed < target - 0.05;
        let steam_loco = v.kind == CarKind::SteamEngine;
        let chance = if steam_loco {
            if speed < 0.05 { 0.3 } else { 1.0 }
        } else if working {
            0.9
        } else {
            0.25
        };
        if rand::random::<f32>() > chance {
            continue;
        }
        // An over-stoked fire makes thick black smoke.
        let black = steam_loco && t.fire > 1.0;
        let (mat, life, grow, scale) = match (steam_loco, black) {
            (true, false) => (steam.clone(), 1.8, 1.1, 1.0),
            (true, true) => (exhaust.clone(), 2.2, 1.3, 1.2),
            _ => (exhaust.clone(), 1.2, 0.9, 0.55),
        };
        let jitter = Vec3::new(rand::random::<f32>() * 0.1 - 0.05, 0.0, rand::random::<f32>() * 0.1 - 0.05);
        commands.spawn((
            Mesh3d(mesh.clone()),
            MeshMaterial3d(mat),
            Transform::from_translation(tf.transform_point(stack)).with_scale(Vec3::splat(scale)),
            Puff { age: 0.0, life, grow: grow * scale, vel: jitter + Vec3::Y * if steam_loco { 0.35 } else { 0.25 } },
            bevy::pbr::NotShadowCaster,
        ));
    }
}