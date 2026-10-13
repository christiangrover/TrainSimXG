//! Tests for the simulation core: track following, switches, ramps, saving.

use crate::demo::build_demo;
use crate::grid::*;
use crate::track::smooth_levels;
use crate::models_vehicle::CarKind;
use crate::save;
use crate::track::*;
use crate::trains::{CargoKind, Trains};
use bevy::prelude::*;

fn put(grid: &mut Grid, x: i32, z: i32, kind: TrackKind, rot: u8, level: i32) {
    grid.edit(IVec2::new(x, z)).unwrap().content = Content::Track(Track::new(kind, rot, level));
}

/// A 4 x 3 loop with corners at (10,10) and (13,12).
fn small_loop() -> Grid {
    let mut g = Grid::default();
    for x in 11..13 {
        put(&mut g, x, 10, TrackKind::Straight, 1, 0);
        put(&mut g, x, 12, TrackKind::Straight, 1, 0);
    }
    put(&mut g, 10, 11, TrackKind::Straight, 0, 0);
    put(&mut g, 13, 11, TrackKind::Straight, 0, 0);
    put(&mut g, 10, 10, TrackKind::Curve, 0, 0);
    put(&mut g, 13, 10, TrackKind::Curve, 1, 0);
    put(&mut g, 13, 12, TrackKind::Curve, 2, 0);
    put(&mut g, 10, 12, TrackKind::Curve, 3, 0);
    g
}

#[test]
fn train_goes_round_a_loop_and_comes_back() {
    let g = small_loop();
    let start = start_pos(&g, IVec2::new(11, 10)).unwrap();
    let (p0, _) = start.world(&g);
    // Loop length: 6 straights + 4 quarter circles of radius 0.5.
    let len = 6.0 + 4.0 * std::f32::consts::FRAC_PI_4;
    let mut p = start;
    let moved = p.advance(&g, len);
    assert!((moved - len).abs() < 1e-3, "moved {moved}");
    assert!((p.world(&g).0 - p0).length() < 1e-3);
    // And in reverse.
    let mut r = start.reversed();
    r.advance(&g, len);
    assert!((r.world(&g).0 - p0).length() < 1e-3);
}

#[test]
fn track_stops_at_dead_end() {
    let mut g = Grid::default();
    put(&mut g, 5, 5, TrackKind::Straight, 1, 0);
    put(&mut g, 6, 5, TrackKind::Straight, 1, 0);
    let mut p = start_pos(&g, IVec2::new(5, 5)).unwrap();
    let moved = p.advance(&g, 10.0);
    assert!(moved < 2.0 && moved > 0.4, "moved {moved}");
}

#[test]
fn switch_routes_follow_its_setting() {
    let mut g = Grid::default();
    // Switch at (5,5): stem W, straight E, branch N (SwitchLeft rot 1).
    put(&mut g, 4, 5, TrackKind::Straight, 1, 0);
    put(&mut g, 5, 5, TrackKind::SwitchLeft, 1, 0);
    put(&mut g, 6, 5, TrackKind::Straight, 1, 0);
    put(&mut g, 5, 4, TrackKind::Straight, 0, 0);
    let from_w = TrackPos { tile: IVec2::new(4, 5), entry: W, exit: E, t: 0.5 };
    let a = from_w.ahead(&g, 1.7);
    assert_eq!(a.tile, IVec2::new(6, 5));
    g.edit(IVec2::new(5, 5)).unwrap().track_mut().unwrap().diverge = true;
    let b = from_w.ahead(&g, 1.7);
    assert_eq!(b.tile, IVec2::new(5, 4));
    // Coming back from the branch always merges onto the stem.
    let back = TrackPos { tile: IVec2::new(5, 4), entry: N, exit: S, t: 0.5 }.ahead(&g, 1.7);
    assert_eq!(back.tile, IVec2::new(4, 5));
}

#[test]
fn ramps_join_levels_and_mismatches_block() {
    let mut g = Grid::default();
    put(&mut g, 5, 5, TrackKind::Straight, 0, 0);
    put(&mut g, 5, 4, TrackKind::Ramp, 0, 0); // rises toward N
    put(&mut g, 5, 3, TrackKind::Straight, 0, 1);
    let p = TrackPos { tile: IVec2::new(5, 5), entry: S, exit: N, t: 0.5 };
    let up = p.ahead(&g, 2.0);
    assert_eq!(up.tile, IVec2::new(5, 3));
    assert!((up.world(&g).0.y - (LEVEL_H + RAIL_Y)).abs() < 1e-4);
    // Lower the top piece: the levels no longer match, so the train stops.
    g.edit(IVec2::new(5, 3)).unwrap().track_mut().unwrap().level = 0;
    assert_eq!(p.ahead(&g, 2.0).tile, IVec2::new(5, 4));
}

#[test]
fn trains_spawn_with_cars_trailing_behind() {
    let g = small_loop();
    let mut trains = Trains::default();
    let id = trains.spawn(&g, IVec2::new(12, 10), &[CarKind::DieselEngine, CarKind::Boxcar, CarKind::Caboose]).unwrap();
    let t = trains.get(id).unwrap();
    assert_eq!(t.cars.len(), 3);
    let a = t.cars[0].pos.world(&g).0;
    let b = t.cars[1].pos.world(&g).0;
    assert!((a - b).length() > 0.6);
    assert!(trains.spawn(&g, IVec2::new(50, 50), &[CarKind::Passenger]).is_err());
}

#[test]
fn save_round_trip_keeps_layout_and_trains() {
    let mut g = Grid::default();
    let mut trains = Trains::default();
    build_demo(&mut g, &mut trains);
    let json = save::to_json(&g, &trains).unwrap();
    let mut g2 = Grid::default();
    let mut t2 = Trains::default();
    save::from_json(&json, &mut g2, &mut t2).unwrap();
    assert!(g.tiles == g2.tiles);
    assert_eq!(trains.list.len(), t2.list.len());
    assert_eq!(t2.list[0].cars.len(), trains.list[0].cars.len());
    assert!(save::from_json("{not json", &mut g2, &mut t2).is_err());
}

#[test]
fn demo_trains_are_all_on_valid_track() {
    let mut g = Grid::default();
    let mut trains = Trains::default();
    build_demo(&mut g, &mut trains);
    assert_eq!(trains.list.len(), 4);
    for (i, t) in trains.list.iter().enumerate() {
        for c in &t.cars {
            assert!(c.pos.valid(&g), "{} has a car off the rails", t.name);
        }
        if i == 3 {
            // The shuttle runs out and back, over the main line on an overpass.
            let mut p = t.cars[0].pos;
            assert!(p.advance(&g, 30.0) > 10.0, "the shuttle line is too short or blocked");
            continue;
        }
        // The other demo lines are closed loops: a long run never hits a dead end.
        let mut p = t.cars[0].pos;
        let m = p.advance(&g, 500.0); assert!((m - 500.0).abs() < 1e-2, "{} hit the end of the line at {:?} (track {:?})", t.name, p, g.track(p.tile));
    }
}

fn test_train(kinds: &[CarKind]) -> (Grid, Trains, u32) {
    let g = small_loop();
    let mut trains = Trains::default();
    let id = trains.spawn(&g, IVec2::new(12, 10), kinds).unwrap();
    (g, trains, id)
}

#[test]
fn steam_needs_stoking_on_manual_fuel() {
    let (_, mut trains, id) = test_train(&[CarKind::SteamEngine, CarKind::Tender]);
    let t = trains.get_mut(id).unwrap();
    t.manual = true;
    t.fuel_manual = true;
    t.throttle = 1.0;
    assert!(t.power() > 0.9);
    // Run a few minutes without shovelling: the fire dies, the steam goes.
    for _ in 0..(180 * 20) {
        t.speed = 1.0;
        crate::trains::burn_for_test(t, 0.05);
    }
    assert!(t.power() < 0.3, "pressure should have dropped, power {}", t.power());
    let coal = t.fuel;
    for _ in 0..8 {
        assert!(t.shovel_coal());
    }
    assert!(t.fuel < coal, "shovelling uses coal from the tender");
    for _ in 0..(10 * 20) {
        crate::trains::burn_for_test(t, 0.05);
    }
    assert!(t.power() > 0.5, "stoking brings the steam back, power {}", t.power());
}

#[test]
fn diesel_runs_dry_and_auto_fuel_never_does() {
    let (_, mut trains, id) = test_train(&[CarKind::DieselRoad, CarKind::Boxcar]);
    let t = trains.get_mut(id).unwrap();
    t.manual = true;
    t.throttle = 1.0;
    for _ in 0..(200 * 20) {
        crate::trains::burn_for_test(t, 0.05);
    }
    assert_eq!(t.power(), 1.0, "auto fuel never runs out");
    t.fuel_manual = true;
    for _ in 0..(300 * 20) {
        crate::trains::burn_for_test(t, 0.05);
    }
    assert!(t.fuel > 0.0, "a full tank lasts more than 5 minutes flat out");
    for _ in 0..(300 * 20) {
        crate::trains::burn_for_test(t, 0.05);
    }
    assert_eq!(t.fuel, 0.0);
    assert_eq!(t.power(), 0.0, "an empty tank means no power");
}

#[test]
fn cars_only_take_the_right_cargo() {
    let (_, mut trains, id) = test_train(&[CarKind::LogCar, CarKind::Hopper, CarKind::Tanker]);
    let t = trains.get_mut(id).unwrap();
    assert!(!t.cars[0].load(CargoKind::Rocks, 0.5), "no rocks on the log car");
    assert!(t.cars[0].load(CargoKind::Logs, 0.5));
    assert!(t.cars[1].load(CargoKind::Rocks, 1.5));
    assert!(!t.cars[1].load(CargoKind::Rocks, 0.1), "a full hopper takes no more");
    assert!(t.cars[2].load(CargoKind::Oil, 0.3));
    assert!(!t.cars[2].load(CargoKind::Water, 0.3), "can't mix water into oil");
}
#[test]
fn raising_track_makes_a_smooth_slope() {
    let mut g = Grid::default();
    for x in 0..12 {
        put(&mut g, x, 5, TrackKind::Straight, 1, 0);
    }
    // Raise the middle piece three times, smoothing after each, like the tool does.
    for _ in 0..3 {
        g.edit(IVec2::new(6, 5)).unwrap().track_mut().unwrap().level += 1;
        assert!(smooth_levels(&mut g, IVec2::new(6, 5)).is_empty());
    }
    let levels: Vec<i32> = (0..12).map(|x| g.track(IVec2::new(x, 5)).unwrap().max_level()).collect();
    assert_eq!(levels, vec![0, 0, 0, 1, 2, 3, 3, 3, 2, 1, 0, 0]);
    // A train can run the whole way over the hump and back down.
    let mut p = TrackPos { tile: IVec2::new(0, 5), entry: W, exit: E, t: 0.0 };
    let moved = p.advance(&g, 11.5);
    assert!((moved - 11.5).abs() < 1e-3, "blocked after {moved}");
    // Lowering it again flattens the line.
    for _ in 0..3 {
        g.edit(IVec2::new(6, 5)).unwrap().track_mut().unwrap().level -= 1;
        smooth_levels(&mut g, IVec2::new(6, 5));
    }
    assert!((0..12).all(|x| g.track(IVec2::new(x, 5)).unwrap().kind == TrackKind::Straight && g.track(IVec2::new(x, 5)).unwrap().level == 0));
}
#[test]
fn raising_pieces_side_by_side_builds_a_flat_bridge() {
    let mut g = Grid::default();
    for x in 0..14 {
        put(&mut g, x, 5, TrackKind::Straight, 1, 0);
    }
    // Raise tiles 5..=8 twice each, one after another, like dragging the tool along.
    let raise = |g: &mut Grid, x: i32| {
        crate::build::raise_piece(g.edit(IVec2::new(x, 5)).unwrap().track_mut().unwrap(), true);
        smooth_levels(g, IVec2::new(x, 5));
    };
    for _ in 0..2 {
        for x in 5..=8 {
            raise(&mut g, x);
        }
    }
    let tops: Vec<i32> = (0..14).map(|x| g.track(IVec2::new(x, 5)).unwrap().max_level()).collect();
    assert_eq!(&tops[5..=8], &[2, 2, 2, 2], "a flat bridge, not pinnacles: {tops:?}");
    for x in 5..=8 {
        assert_eq!(g.track(IVec2::new(x, 5)).unwrap().kind, TrackKind::Straight);
    }
    let mut p = TrackPos { tile: IVec2::new(0, 5), entry: W, exit: E, t: 0.0 };
    assert!((p.advance(&g, 13.5) - 13.5).abs() < 1e-3, "a train gets over the bridge");
}
/// A plus-shaped layout: an east-west line along z = 5 and a north-south line
/// along x = 6, each 13 tiles long, laid with the track tool's own logic.
fn crossing_lines() -> Grid {
    let mut g = Grid::default();
    for x in 0..13 {
        crate::build::lay_track(&mut g, IVec2::new(x, 5), TrackKind::Straight, 1).unwrap();
    }
    for z in 0..13 {
        crate::build::lay_track(&mut g, IVec2::new(6, z), TrackKind::Straight, 0).unwrap();
    }
    g
}

#[test]
fn crossing_lines_make_a_diamond_then_raising_makes_an_overpass() {
    let mut g = crossing_lines();
    let x = IVec2::new(6, 5);
    assert_eq!(g.track(x).unwrap().kind, TrackKind::Crossing, "lines at the same height cross on the level");
    // Raise the crossing: one line goes up on a bridge, with ramps either side.
    crate::build::raise_piece(g.edit(x).unwrap().track_mut().unwrap(), true);
    assert!(smooth_levels(&mut g, x).is_empty());
    let o = *g.track(x).unwrap();
    assert_eq!(o.kind, TrackKind::Overpass);
    assert!(o.upper - o.level >= CLEARANCE);
    // Trains run the full length of both lines: one over the top, one underneath.
    let mut ew = TrackPos { tile: IVec2::new(0, 5), entry: W, exit: E, t: 0.0 };
    assert!((ew.advance(&g, 12.5) - 12.5).abs() < 1e-3, "the east-west line is blocked");
    let mut ns = TrackPos { tile: IVec2::new(6, 0), entry: N, exit: S, t: 0.0 };
    assert!((ns.advance(&g, 12.5) - 12.5).abs() < 1e-3, "the north-south line is blocked");
    // They pass the crossing point at different heights.
    let over = TrackPos { tile: x, entry: W, exit: E, t: 0.5 }.world(&g).0.y;
    let under = TrackPos { tile: x, entry: N, exit: S, t: 0.5 }.world(&g).0.y;
    assert!(over - under >= CLEARANCE as f32 * LEVEL_H - 1e-4, "over {over}, under {under}");
    // Lower it all the way and it's a level crossing again.
    for _ in 0..3 {
        crate::build::raise_piece(g.edit(x).unwrap().track_mut().unwrap(), false);
        smooth_levels(&mut g, x);
    }
    assert_eq!(g.track(x).unwrap().kind, TrackKind::Crossing);
    assert!((0..13).all(|z| g.track(IVec2::new(6, z)).unwrap().max_level() == 0));
}

#[test]
fn a_raised_line_laid_across_another_becomes_an_overpass() {
    let mut g = Grid::default();
    // A north-south line on the ground...
    for z in 0..13 {
        crate::build::lay_track(&mut g, IVec2::new(6, z), TrackKind::Straight, 0).unwrap();
    }
    // ...and an east-west bridge line two levels up, laid from both ends towards it.
    for x in (0..6).chain((7..13).rev()) {
        crate::build::lay_track(&mut g, IVec2::new(x, 5), TrackKind::Straight, 1).unwrap();
        if x == 2 || x == 10 {
            for _ in 0..2 {
                crate::build::raise_piece(g.edit(IVec2::new(x, 5)).unwrap().track_mut().unwrap(), true);
                smooth_levels(&mut g, IVec2::new(x, 5));
            }
        }
    }
    crate::build::lay_track(&mut g, IVec2::new(6, 5), TrackKind::Straight, 1).unwrap();
    let o = *g.track(IVec2::new(6, 5)).unwrap();
    assert_eq!(o.kind, TrackKind::Overpass, "got {o:?}");
    let mut ns = TrackPos { tile: IVec2::new(6, 0), entry: N, exit: S, t: 0.0 };
    assert!((ns.advance(&g, 12.5) - 12.5).abs() < 1e-3, "the line underneath still runs");
}