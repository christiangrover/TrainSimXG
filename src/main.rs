//! Table Top Trains: build a model railway on a 100 x 100 table and watch it run.

// No console window behind the game in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod build;
mod cab;
mod cab_ui;
mod camera;
mod crane;
mod demo;
mod grid;
mod materials;
mod mesh;
mod models_tile;
mod models_vehicle;
mod roads;
mod save;
mod scene;
mod services;
mod sounds;
mod track;
mod trains;
mod ui;

#[cfg(test)]
mod tests;

use bevy::prelude::*;
use bevy::window::PresentMode;
use bevy_egui::EguiPlugin;

fn main() {
    // Debug aid: TRAINSIM_TRAFFIC_CHECK=<runs> soak-tests road traffic headlessly and exits.
    if let Ok(runs) = std::env::var("TRAINSIM_TRAFFIC_CHECK") {
        let runs: u32 = runs.parse().unwrap_or(1);
        let mut failed = 0;
        for i in 1..=runs {
            match roads::soak::run() {
                Ok(d) => println!("run {i}: ok, {d:.0} tiles driven"),
                Err(e) => {
                    failed += 1;
                    println!("run {i}: FAILED {e}");
                }
            }
        }
        println!("{failed} of {runs} traffic runs failed");
        std::process::exit(if failed == 0 { 0 } else { 1 });
    }
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: "Table Top Trains".into(),
                resolution: (1600.0_f32, 900.0_f32).into(),
                present_mode: PresentMode::AutoVsync,
                ..default()
            }),
            ..default()
        }))
        .add_plugins(EguiPlugin::default())
        .insert_resource(grid::Grid::default())
        .add_plugins((
            scene::ScenePlugin,
            camera::CameraPlugin,
            trains::TrainsPlugin,
            roads::RoadsPlugin,
            build::BuildPlugin,
            ui::UiPlugin,
            sounds::SoundsPlugin,
            services::ServicesPlugin,
            crane::CranePlugin,
            cab::CabPlugin,
        ))
        .add_systems(Startup, load_demo)
        .add_systems(Update, materials::flash_lights)
        .add_systems(Update, auto_screenshots.run_if(|| std::env::var("TRAINSIM_SHOTS").is_ok()))
        .run();
}

/// Debug aid: with TRAINSIM_SHOTS=<dir>, visit each camera view, save a
/// screenshot of it, then quit.
fn auto_screenshots(
    mut commands: Commands,
    mut frame: Local<u32>,
    mut held: Local<u32>,
    mut closed_for: Local<u32>,
    mut cam: ResMut<camera::CamState>,
    mut trains: ResMut<trains::Trains>,
    vehicles: Query<(&roads::RoadCar, &Transform)>,
    mut cab_req: ResMut<cab::CabRequests>,
    mut cranes: ResMut<crane::Cranes>,
    closed: Res<roads::ClosedCrossings>,
    grid: Res<grid::Grid>,
    mut exit: EventWriter<AppExit>,
) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    use camera::CamMode;
    let dir = std::env::var("TRAINSIM_SHOTS").unwrap();
    *frame += 1;
    // (view, overview focus tile and zoom, file name, train to watch)
    let views = [
        (CamMode::Overview, None, "overview", 0),
        (CamMode::Overview, Some((IVec2::new(45, 63), 9.0)), "station", 0),
        (CamMode::Overview, Some((IVec2::new(45, 50), 7.0)), "town", 0),
        (CamMode::Overview, Some((IVec2::new(13, 76), 8.0)), "bridge", 0),
        (CamMode::Overview, Some((IVec2::new(45, 48), 1.6)), "signals", 0),
        (CamMode::Overview, Some((IVec2::new(45, 65), 2.2)), "crossing", 0),
        (CamMode::Overview, Some((IVec2::new(37, 48), 2.2)), "police", 0),
        (CamMode::Overview, Some((IVec2::new(47, 56), 2.5)), "hospital", 0),
        (CamMode::Overview, Some((IVec2::new(53, 40), 2.2)), "fire", 0),
        (CamMode::Overview, Some((IVec2::ZERO, 0.9)), "emergency_a", 0),
        (CamMode::Overview, Some((IVec2::ZERO, 0.9)), "emergency_b", 2),
        (CamMode::Follow, None, "follow_steam", 0),
        (CamMode::Follow, None, "follow_diesel", 1),
        (CamMode::Passenger, None, "passenger", 0),
        (CamMode::Engineer, None, "engineer", 1),
        (CamMode::Fireman, None, "fireman", 0),
        (CamMode::Walk, None, "walk", 0),
        (CamMode::Overview, Some((IVec2::new(66, 66), 3.5)), "port", 0),
        (CamMode::Crane, None, "crane", 0),
        (CamMode::Crane, None, "crane_grab", 0),
        (CamMode::Crane, None, "crane_drop", 0),
        (CamMode::Overview, Some((IVec2::new(66, 65), 2.0)), "loading", 0),
    ];
    let step = 150;
    let i = (*frame / step) as usize;
    // Hold the crossing shot until a train is actually coming (up to ~2 minutes).
    *closed_for = if closed.0.is_empty() { 0 } else { *closed_for + 1 };
    if i < views.len() && views[i].2 == "crossing" && *frame % step == step - 2 && *closed_for < 90 && *held < 7200 {
        *held += 1;
        *frame -= 1;
    }
    if i >= views.len() + 1 {
        exit.write(AppExit::Success);
        return;
    }
    if *frame % step == 1 && i < views.len() {
        trains.selected = trains.list.get(views[i].3).map(|t| t.id);
        cam.set_mode(views[i].0);
        match views[i].1 {
            Some((t, d)) => {
                cam.focus = grid::tile_center(t);
                cam.dist = d;
                cam.pitch = 0.6;
            }
            None => {
                *cam = camera::CamState { mode: views[i].0, follow_dist: 1.6, follow_pitch: 0.25, follow_yaw: 2.3, ..default() };
            }
        }
    }
    // The emergency_* views track the n-th emergency vehicle.
    if i < views.len() && views[i].2.starts_with("emergency") {
        if let Some((_, tf)) = vehicles.iter().filter(|(c, _)| c.is_emergency()).nth(views[i].3) {
            cam.focus = tf.translation;
            cam.yaw += 0.004;
        }
    }
    // The crossing shot looks at whichever crossing is closed for a train.
    if i < views.len() && views[i].2 == "crossing" {
        if let Some(t) = closed.0.iter().next() {
            cam.focus = grid::tile_center(*t);
        }
    }
    // Pose the crane: hook down on the log pile, or carrying logs down onto the log car.
    if i < views.len() && views[i].2.starts_with("crane_") {
        let log_car = trains.list.iter().flat_map(|t| t.cars.iter()).find(|c| c.kind == models_vehicle::CarKind::LogCar).map(|c| trains::car_transform(&grid, c).translation);
        if let Some(c) = cranes.active.and_then(|t| cranes.map.get_mut(&t)) {
            c.tool = crane::Tool::Hook;
            if views[i].2 == "crane_grab" {
                c.carrying = None;
                let p = crane::log_pile_pos(&grid, c);
                crane::point_at(&grid, c, p);
            } else if let Some(p) = log_car {
                c.carrying = Some(trains::CargoKind::Logs);
                crane::point_at(&grid, c, p);
            }
            c.hook = 0.0;
        }
    }
    // Mid-shovel for the fireman shot.
    if i < views.len() && views[i].0 == CamMode::Fireman && *frame % step == step - 45 {
        cab_req.shovel = true;
    }
    if *frame % step == step - 1 && i < views.len() {
        let path = format!("{dir}/{}.png", views[i].2);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
}

fn load_demo(mut grid: ResMut<grid::Grid>, mut trains: ResMut<trains::Trains>) {
    demo::build_demo(&mut grid, &mut trains);
}

