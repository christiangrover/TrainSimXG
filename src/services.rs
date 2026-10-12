//! What happens when a train stops: refuelling and unloading at stations,
//! water towers, fuel depots, and waiting while a crane loads it.

use crate::grid::*;
use crate::models_vehicle::CarKind;
use crate::trains::*;
use bevy::prelude::*;
use std::collections::HashSet;

pub struct ServicesPlugin;

impl Plugin for ServicesPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CraneBusy>().init_resource::<Depots>().add_systems(Update, (find_depots, services.after(crate::trains::run_trains)).chain());
    }
}

/// Trains a crane is currently loading (they wait for it).
#[derive(Resource, Default)]
pub struct CraneBusy(pub HashSet<u32>);

/// Where the water towers, fuel depots and ports are.
#[derive(Resource, Default)]
pub struct Depots {
    pub water: Vec<IVec2>,
    pub fuel: Vec<IVec2>,
    pub ports: Vec<IVec2>,
    timer: f32,
    generation: u32,
}

pub fn find_depots(time: Res<Time>, grid: Res<Grid>, mut d: ResMut<Depots>) {
    d.timer -= time.delta_secs();
    if d.timer > 0.0 && d.generation == grid.generation && !grid.is_changed() {
        return;
    }
    d.timer = 1.0;
    d.generation = grid.generation;
    d.water = grid.find_decor(DecorKind::WaterTower);
    d.fuel = grid.find_decor(DecorKind::FuelDepot);
    d.ports = grid.find_decor(DecorKind::Port);
}

fn near(list: &[IVec2], p: Vec3, r: f32) -> bool {
    list.iter().any(|t| {
        let c = tile_center(*t);
        Vec2::new(p.x - c.x, p.z - c.z).length() < r
    })
}

/// How fast things fill, per second (1.0 = empty to full).
const REFUEL: f32 = 0.25;
const FILL: f32 = 0.2;
const UNLOAD: f32 = 0.2;

fn services(time: Res<Time>, sim: Res<Sim>, grid: Res<Grid>, depots: Res<Depots>, busy: Res<CraneBusy>, mut trains: ResMut<Trains>) {
    if !sim.running {
        return;
    }
    let dt = time.delta_secs().min(0.05) * sim.time_scale;
    for train in trains.list.iter_mut() {
        if train.speed.abs() > 0.05 {
            continue;
        }
        let power = train.power_kind();
        let mut work: Option<String> = None;
        let at_station = train.cars.iter().any(|c| grid.track(c.pos.tile).is_some_and(|t| t.kind == TrackKind::Station));
        if at_station {
            // Stations top up the engine...
            let mut topped = false;
            if train.fuel < 1.0 && power != Power::None {
                train.fuel = (train.fuel + REFUEL * dt).min(1.0);
                topped = true;
            }
            if train.water < 1.0 && power == Power::Steam {
                train.water = (train.water + REFUEL * dt).min(1.0);
                topped = true;
            }
            if topped {
                work = Some(if power == Power::Steam { "Taking on coal and water".into() } else { "Refuelling".into() });
            }
        }
        for i in 0..train.cars.len() {
            let p = train.cars[i].pos.world(&grid).0;
            let kind = train.cars[i].kind;
            // ...and unload freight, unless this is the port's own station.
            if at_station && train.cars[i].cargo.amount > 0.0 && !near(&depots.ports, p, 3.5) {
                let what = train.cars[i].cargo.kind;
                let c = &mut train.cars[i].cargo;
                c.amount = (c.amount - UNLOAD * dt).max(0.0);
                if c.amount <= 0.0 {
                    c.kind = CargoKind::Empty;
                }
                work = Some(format!("Unloading {}", what.label()));
            }
            if near(&depots.water, p, 1.4) {
                if matches!(kind, CarKind::SteamEngine | CarKind::Tender) && train.water < 1.0 && train.fuel_manual {
                    train.water = (train.water + 0.4 * dt).min(1.0);
                    work = Some("Filling up with water".into());
                }
                if train.cars[i].load(CargoKind::Water, FILL * dt) {
                    work = Some("Filling the tank car with water".into());
                }
            }
            if near(&depots.fuel, p, 1.4) {
                if matches!(kind, CarKind::DieselEngine | CarKind::DieselRoad) && train.fuel < 1.0 && train.fuel_manual {
                    train.fuel = (train.fuel + 0.4 * dt).min(1.0);
                    work = Some("Refuelling at the depot".into());
                }
                if train.cars[i].load(CargoKind::Oil, FILL * dt) {
                    work = Some("Filling the tank car with oil".into());
                }
            }
        }
        if busy.0.contains(&train.id) {
            work = Some("The crane is loading cargo".into());
        }
        if let Some(w) = work {
            // Self-driving trains wait at the station until the job is done.
            if !train.manual && train.dwell > 0.0 {
                train.dwell = train.dwell.max(0.3);
            }
            if train.status.is_empty() || train.status.starts_with("Stopped") {
                train.status = w;
            }
        }
    }
}
