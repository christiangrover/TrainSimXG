//! Table Top Trains: build a model railway on a 100 x 100 table and watch it run.

mod build;
mod camera;
mod demo;
mod grid;
mod mesh;
mod models_tile;
mod models_vehicle;
mod roads;
mod save;
mod scene;
mod track;
mod trains;
mod ui;

#[cfg(test)]
mod tests;

use bevy::prelude::*;
use bevy::window::PresentMode;
use bevy_egui::EguiPlugin;

fn main() {
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
        ))
        .add_systems(Startup, load_demo)
        .add_systems(Update, auto_screenshots.run_if(|| std::env::var("TRAINSIM_SHOTS").is_ok()))
        .run();
}

/// Debug aid: with TRAINSIM_SHOTS=<dir>, visit each camera view, save a
/// screenshot of it, then quit.
fn auto_screenshots(
    mut commands: Commands,
    mut frame: Local<u32>,
    mut cam: ResMut<camera::CamState>,
    mut exit: EventWriter<AppExit>,
) {
    use bevy::render::view::screenshot::{save_to_disk, Screenshot};
    use camera::CamMode;
    let dir = std::env::var("TRAINSIM_SHOTS").unwrap();
    *frame += 1;
    // (view, overview focus tile and zoom, file name)
    let views = [
        (CamMode::Overview, None, "overview"),
        (CamMode::Overview, Some((IVec2::new(45, 63), 9.0)), "station"),
        (CamMode::Overview, Some((IVec2::new(13, 76), 8.0)), "bridge"),
        (CamMode::Overview, Some((IVec2::new(60, 31), 10.0)), "tunnel"),
        (CamMode::Follow, None, "follow"),
        (CamMode::Passenger, None, "passenger"),
        (CamMode::Engineer, None, "engineer"),
        (CamMode::Walk, None, "walk"),
    ];
    let step = 150;
    let i = (*frame / step) as usize;
    if i >= views.len() + 1 {
        exit.write(AppExit::Success);
        return;
    }
    if *frame % step == 1 && i < views.len() {
        cam.set_mode(views[i].0);
        match views[i].1 {
            Some((t, d)) => {
                cam.focus = grid::tile_center(t);
                cam.dist = d;
                cam.pitch = 0.6;
            }
            None => {
                *cam = camera::CamState { mode: views[i].0, ..default() };
            }
        }
    }
    if *frame % step == step - 1 && i < views.len() {
        let path = format!("{dir}/{}.png", views[i].2);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
}

fn load_demo(mut grid: ResMut<grid::Grid>, mut trains: ResMut<trains::Trains>) {
    demo::build_demo(&mut grid, &mut trains);
}
