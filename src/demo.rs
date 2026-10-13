//! A starter layout so there is something running the first time you open the game:
//! a main loop with a station, a passing siding with switches, a tunnel through a
//! hill, a raised loop bridging a river, and a town whose roads cross the line.

use crate::grid::*;
use crate::models_vehicle::CarKind;
use crate::trains::Trains;
use bevy::prelude::*;

fn set(tiles: &mut [Tile], x: i32, z: i32, f: impl FnOnce(&mut Tile)) {
    if in_bounds(IVec2::new(x, z)) {
        f(&mut tiles[(z * GRID + x) as usize]);
    }
}

fn track(tiles: &mut [Tile], x: i32, z: i32, kind: TrackKind, rot: u8, level: i32) {
    set(tiles, x, z, |t| t.content = Content::Track(Track::new(kind, rot, level)));
}

fn road(tiles: &mut [Tile], x: i32, z: i32) {
    set(tiles, x, z, |t| {
        if t.content == Content::Empty {
            t.content = Content::Road;
        }
    });
}

fn decor(tiles: &mut [Tile], x: i32, z: i32, kind: DecorKind, rot: u8) {
    set(tiles, x, z, |t| {
        if t.content == Content::Empty && t.ground != Ground::Water {
            t.content = Content::Decor { kind, rot, variant: ((x * 7 + z * 3) & 0xff) as u8 };
        }
    });
}

/// Rectangle loop of track from (x0, z0) to (x1, z1) inclusive.
fn track_loop(tiles: &mut [Tile], x0: i32, z0: i32, x1: i32, z1: i32, level: i32) {
    for x in x0 + 1..x1 {
        track(tiles, x, z0, TrackKind::Straight, 1, level);
        track(tiles, x, z1, TrackKind::Straight, 1, level);
    }
    for z in z0 + 1..z1 {
        track(tiles, x0, z, TrackKind::Straight, 0, level);
        track(tiles, x1, z, TrackKind::Straight, 0, level);
    }
    track(tiles, x0, z0, TrackKind::Curve, 0, level);
    track(tiles, x1, z0, TrackKind::Curve, 1, level);
    track(tiles, x1, z1, TrackKind::Curve, 2, level);
    track(tiles, x0, z1, TrackKind::Curve, 3, level);
}

pub fn build_demo(grid: &mut Grid, trains: &mut Trains) {
    let mut tiles = vec![Tile::default(); (GRID * GRID) as usize];

    // --- Landscape ---------------------------------------------------------
    // A hill in the north-east that the main line tunnels through.
    for z in 20..38 {
        for x in 54..76 {
            let dx = (x as f32 - 65.0) / 10.0;
            let dz = (z as f32 - 29.0) / 8.0;
            let f = 1.0 - (dx * dx + dz * dz);
            if f > 0.0 {
                let h = (3.0 + f * 8.0).round() as i32;
                set(&mut tiles, x, z, |t| {
                    t.height = h;
                    t.ground = if h > 9 { Ground::Rock } else { Ground::Grass };
                });
            }
        }
    }
    // A plateau in the west, cut by a river running north to a pond.
    for z in 44..82 {
        for x in 0..24 {
            set(&mut tiles, x, z, |t| t.height = 2);
        }
    }
    for z in 36..96 {
        for x in 12..15 {
            set(&mut tiles, x, z, |t| {
                t.height = 0;
                t.ground = Ground::Water;
            });
        }
    }
    for z in 84..98 {
        for x in 4..24 {
            let d = ((x as f32 - 13.5).powi(2) / 60.0 + (z as f32 - 91.0).powi(2) / 25.0).sqrt();
            if d < 1.0 {
                set(&mut tiles, x, z, |t| {
                    t.height = 0;
                    t.ground = Ground::Water;
                });
            } else if d < 1.3 {
                set(&mut tiles, x, z, |t| t.ground = Ground::Sand);
            }
        }
    }

    // A lake south of the main line, with a port on its shore.
    for z in 68..75 {
        for x in 62..74 {
            let d = ((x as f32 - 67.5) / 6.5).powi(2) + ((z as f32 - 71.5) / 4.0).powi(2);
            if d < 1.0 {
                set(&mut tiles, x, z, |t| t.ground = Ground::Water);
            } else if d < 1.3 {
                set(&mut tiles, x, z, |t| t.ground = Ground::Sand);
            }
        }
    }

    // --- Main line ---------------------------------------------------------
    let (x0, z0, x1, z1) = (25, 30, 75, 65);
    track_loop(&mut tiles, x0, z0, x1, z1, 0);
    // Station on the south side.
    for x in 46..=50 {
        track(&mut tiles, x, z1, TrackKind::Station, 1, 0);
    }
    // The port's goods station, crane, water tower and fuel depot.
    track(&mut tiles, 63, z1, TrackKind::Station, 1, 0);
    track(&mut tiles, 64, z1, TrackKind::Station, 1, 0);
    set(&mut tiles, 67, 67, |t| t.content = Content::Decor { kind: DecorKind::Port, rot: 0, variant: 0 });
    set(&mut tiles, 64, z1 - 1, |t| t.content = Content::Decor { kind: DecorKind::WaterTower, rot: 1, variant: 0 });
    set(&mut tiles, 66, z1 - 1, |t| t.content = Content::Decor { kind: DecorKind::FuelDepot, rot: 0, variant: 0 });
    // Passing siding just north of the main line, joined by two switches.
    track(&mut tiles, 38, z1, TrackKind::SwitchLeft, 1, 0); // stem W, straight E, branch N
    track(&mut tiles, 38, z1 - 1, TrackKind::Curve, 0, 0); // S <-> E
    for x in 39..60 {
        track(&mut tiles, x, z1 - 1, TrackKind::Straight, 1, 0);
    }
    track(&mut tiles, 60, z1 - 1, TrackKind::Curve, 1, 0); // W <-> S
    track(&mut tiles, 60, z1, TrackKind::SwitchRight, 3, 0); // stem E, straight W, branch N

    // --- Raised west loop bridging the river --------------------------------
    let lvl = 2;
    track_loop(&mut tiles, 4, 48, 21, 78, lvl);

    // --- Town ----------------------------------------------------------------
    let road_z = [40, 48, 56];
    let road_x = [35, 45, 55];
    // Level crossings where the north-south roads meet the railway.
    for &x in &road_x {
        track(&mut tiles, x, z1, TrackKind::LevelCrossing, 1, 0);
        if (39..60).contains(&x) {
            track(&mut tiles, x, z1 - 1, TrackKind::LevelCrossing, 1, 0);
        }
    }
    for &z in &road_z {
        for x in 28..73 {
            road(&mut tiles, x, z);
        }
    }
    for &x in &road_x {
        for z in 38..73 {
            road(&mut tiles, x, z);
        }
    }
    for x in 35..56 {
        road(&mut tiles, x, 72);
    }

    // Emergency services facing the streets.
    for (x, z, kind, rot) in [(37, 47, DecorKind::PoliceStation, 0), (53, 41, DecorKind::FireStation, 2), (47, 57, DecorKind::Hospital, 2)] {
        decor(&mut tiles, x, z, kind, rot);
    }

    // Houses along the roads, facing the street.
    let houses = [DecorKind::House, DecorKind::Cottage, DecorKind::Shop, DecorKind::House, DecorKind::Apartment, DecorKind::House];
    let mut i = 0;
    for &z in &road_z {
        for x in (29..72).step_by(2) {
            decor(&mut tiles, x, z - 1, houses[i % houses.len()], 0);
            decor(&mut tiles, x, z + 1, houses[(i + 3) % houses.len()], 2);
            i += 1;
        }
    }
    decor(&mut tiles, 50, 52, DecorKind::Church, 0);
    decor(&mut tiles, 40, 52, DecorKind::Fountain, 0);
    for x in [47, 50] {
        decor(&mut tiles, x, z1 + 1, DecorKind::Person, 0);
    }
    decor(&mut tiles, 48, z1 + 1, DecorKind::Bench, 2);
    decor(&mut tiles, 49, z1 + 1, DecorKind::Lamp, 2);
    for x in (40..72).step_by(4) {
        decor(&mut tiles, x, 70, DecorKind::Lamp, 0);
    }
    for x in 36..55 {
        decor(&mut tiles, x, 73, DecorKind::House, 2);
    }

    // Farm in the south-east.
    decor(&mut tiles, 82, 74, DecorKind::Barn, 0);
    decor(&mut tiles, 84, 74, DecorKind::Silo, 0);
    decor(&mut tiles, 88, 80, DecorKind::Windmill, 0);
    for x in 79..92 {
        decor(&mut tiles, x, 70, DecorKind::Fence, 2);
    }

    // Forests and scattered rocks (deterministic pseudo-random).
    let mut seed = 12345u32;
    let mut rnd = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for _ in 0..1100 {
        let x = (rnd() % GRID as u32) as i32;
        let z = (rnd() % GRID as u32) as i32;
        let in_town = (27..74).contains(&x) && (33..74).contains(&z);
        if in_town {
            continue;
        }
        let k = match rnd() % 10 {
            0..=4 => DecorKind::PineTree,
            5..=7 => DecorKind::OakTree,
            8 => DecorKind::Bush,
            _ => DecorKind::Flowers,
        };
        decor(&mut tiles, x, z, k, (rnd() % 4) as u8);
    }
    for _ in 0..30 {
        let x = (rnd() % GRID as u32) as i32;
        let z = (rnd() % GRID as u32) as i32;
        decor(&mut tiles, x, z, DecorKind::Rock, (rnd() % 4) as u8);
    }

    grid.replace_all(tiles);

    // A shuttle line that crosses over the main line on an overpass.
    for z in 20..38 {
        let _ = crate::build::lay_track(grid, IVec2::new(31, z), TrackKind::Straight, 0);
    }
    let over = IVec2::new(31, z0);
    if let Some(x) = grid.edit(over).and_then(|x| x.track_mut()) {
        crate::build::raise_piece(x, true);
        x.rot = 1; // the shuttle goes over the top
    }
    crate::track::smooth_levels(grid, over);

    trains.list.clear();
    trains.next_id = 0;
    // Both main-line trains run clockwise so they follow each other round.
    let _ = trains.spawn_heading(grid, IVec2::new(45, z0), &[CarKind::SteamEngine, CarKind::Tender, CarKind::Passenger, CarKind::Dining, CarKind::Sleeper, CarKind::Caboose], Some(E));
    let _ = trains.spawn_heading(grid, IVec2::new(x1, 52), &[CarKind::DieselRoad, CarKind::Boxcar, CarKind::Tanker, CarKind::Hopper, CarKind::LogCar, CarKind::Caboose], Some(S));
    let _ = trains.spawn(grid, IVec2::new(4, 62), &[CarKind::SteamEngine, CarKind::Tender, CarKind::Passenger, CarKind::Passenger]);
    let _ = trains.spawn_heading(grid, IVec2::new(31, 22), &[CarKind::DieselEngine, CarKind::Passenger, CarKind::Caboose], Some(S));
    trains.selected = trains.list.first().map(|t| t.id);
}
