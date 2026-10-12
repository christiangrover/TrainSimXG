//! Driving from the cab: throttle, brakes, emergency brake and horn for the
//! engineer; coal shovelling for the fireman; and the engine sounds.

use crate::camera::{CamMode, CamState};
use crate::materials::{meshes_of, spawn_parts, Materials};
use crate::mesh::{Geo, Mat};
use crate::models_vehicle::CarKind;
use crate::sounds::{set_engine_loop, EngineLoop, SoundQueue, Sfx, Sounds};
use crate::trains::*;
use crate::ui::UiCapture;
use bevy::audio::AudioSink;
use bevy::prelude::*;

pub struct CabPlugin;

impl Plugin for CabPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CabRequests>()
            .init_resource::<Fireman>()
            .add_systems(Update, (cab_controls, engine_sounds, spawn_shovel, animate_shovel, fire_glow).chain());
    }
}

/// Button presses from the on-screen cab controls.
#[derive(Resource, Default)]
pub struct CabRequests {
    pub horn: bool,
    pub ebrake: bool,
    pub shovel: bool,
}

#[derive(Resource)]
pub struct Fireman {
    /// Shovel animation progress 0..1, or None when resting.
    anim: Option<f32>,
    /// Brief brightening of the fire after each shovelful.
    flare: f32,
    horn_cooldown: f32,
    chuff_dist: f32,
    /// The last thing the fireman should know about.
    pub message: String,
}

impl Default for Fireman {
    fn default() -> Self {
        Fireman { anim: None, flare: 0.0, horn_cooldown: 0.0, chuff_dist: 0.0, message: String::new() }
    }
}

fn train_view(mode: CamMode) -> bool {
    matches!(mode, CamMode::Follow | CamMode::Passenger | CamMode::Engineer | CamMode::Fireman)
}

#[allow(clippy::too_many_arguments)]
fn cab_controls(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    ui: Res<UiCapture>,
    cam: Res<CamState>,
    mut req: ResMut<CabRequests>,
    mut trains: ResMut<Trains>,
    mut fireman: ResMut<Fireman>,
    mut sounds: ResMut<SoundQueue>,
) {
    fireman.horn_cooldown -= time.delta_secs();
    let keys_ok = !ui.keyboard;
    let Some(id) = trains.selected else {
        *req = CabRequests::default();
        return;
    };
    let Some(t) = trains.get_mut(id) else { return };
    let steam = t.power_kind() == Power::Steam;

    // Horn / whistle from any view that's with the train.
    let horn = req.horn || (keys_ok && train_view(cam.mode) && keys.just_pressed(KeyCode::KeyH));
    if horn && fireman.horn_cooldown <= 0.0 {
        sounds.play(if steam { Sfx::Whistle } else { Sfx::Horn }, 0.7);
        fireman.horn_cooldown = 1.0;
    }

    if cam.mode == CamMode::Engineer {
        t.manual = true;
        if keys_ok {
            if keys.just_pressed(KeyCode::KeyW) || keys.just_pressed(KeyCode::ArrowUp) {
                t.throttle = (t.throttle + 0.25).min(1.0);
            }
            if keys.just_pressed(KeyCode::KeyS) || keys.just_pressed(KeyCode::ArrowDown) {
                t.throttle = (t.throttle - 0.25).max(-1.0);
            }
            if keys.just_pressed(KeyCode::KeyX) {
                t.throttle = 0.0;
            }
            t.brake = keys.pressed(KeyCode::Space);
        }
        if req.ebrake || (keys_ok && keys.just_pressed(KeyCode::KeyE)) {
            t.ebrake = !t.ebrake;
            if t.ebrake {
                t.throttle = 0.0;
                sounds.play(Sfx::Hiss, 0.8);
            }
        }
    } else if req.ebrake {
        t.ebrake = !t.ebrake;
    }

    // The fireman shovels coal (Space, or the on-screen button).
    if cam.mode == CamMode::Fireman && (req.shovel || (keys_ok && keys.just_pressed(KeyCode::Space))) && fireman.anim.is_none() {
        if t.fuel <= 0.0 && t.fuel_manual {
            fireman.message = "The tender is out of coal! Stop at a station.".into();
        } else {
            fireman.anim = Some(0.0);
            sounds.play(Sfx::Shovel, 0.7);
        }
    }
    *req = CabRequests::default();
}

#[allow(clippy::too_many_arguments)]
fn engine_sounds(
    mut commands: Commands,
    time: Res<Time>,
    sim: Res<Sim>,
    cam: Res<CamState>,
    trains: Res<Trains>,
    sounds: Option<Res<Sounds>>,
    mut fireman: ResMut<Fireman>,
    mut queue: ResMut<SoundQueue>,
    mut rumble: Query<(Entity, Option<&mut AudioSink>), With<EngineLoop>>,
) {
    let Some(sounds) = sounds else { return };
    let t = trains.selected();
    let kind = t.and_then(|t| t.cars.first()).map(|c| c.kind);
    let listening = train_view(cam.mode) && sim.running;
    // Steam: a chuff every so often as the wheels turn.
    if let (Some(t), Some(CarKind::SteamEngine), true) = (t, kind, listening) {
        fireman.chuff_dist += t.speed.abs() * time.delta_secs() * sim.time_scale;
        if fireman.chuff_dist > 0.3 {
            fireman.chuff_dist = 0.0;
            let close = matches!(cam.mode, CamMode::Engineer | CamMode::Fireman);
            let effort = if t.manual { t.throttle.abs() } else { 0.6 };
            queue.play(Sfx::Chuff, (0.15 + 0.3 * effort) * if close { 1.0 } else { 0.6 });
        }
    }
    // Diesel: an engine rumble that rises with the throttle.
    let (level, pitch) = match (t, kind) {
        (Some(t), Some(CarKind::DieselEngine | CarKind::DieselRoad)) if listening && t.power() > 0.0 => {
            let notch = if t.manual { t.throttle.abs() } else { (t.speed.abs() / MAX_SPEED).min(1.0) };
            let near = if cam.mode == CamMode::Engineer { 1.0 } else { 0.4 };
            ((0.12 + 0.25 * notch) * near, 0.8 + 0.5 * notch)
        }
        _ => (0.0, 1.0),
    };
    set_engine_loop(&mut commands, &sounds, &mut rumble, level, pitch);
}

#[derive(Component)]
struct Shovel;

#[derive(Component)]
struct ShovelCoal;

fn shovel_geo() -> Geo {
    let mut m = Geo::new();
    m.mat(Mat::Steel);
    m.cuboid(Vec3::ZERO, Vec3::new(0.028, 0.002, 0.034), Quat::IDENTITY, Color::srgb(0.35, 0.35, 0.37));
    for x in [-0.028f32, 0.028] {
        m.cuboid(Vec3::new(x, 0.006, 0.0), Vec3::new(0.002, 0.006, 0.034), Quat::IDENTITY, Color::srgb(0.35, 0.35, 0.37));
    }
    m.mat(Mat::Planks);
    let (a, b) = (Vec3::new(0.0, 0.006, 0.03), Vec3::new(0.0, 0.06, 0.3));
    let d = b - a;
    m.cuboid((a + b) * 0.5, Vec3::new(0.006, 0.006, d.length() * 0.5), crate::mesh::basis_from_dir(d), Color::srgb(0.7, 0.55, 0.38));
    m.cuboid(b + Vec3::new(0.0, 0.0, 0.01), Vec3::new(0.022, 0.006, 0.006), Quat::IDENTITY, Color::srgb(0.7, 0.55, 0.38));
    m
}

fn coal_geo() -> Geo {
    let mut m = Geo::new();
    m.mat(Mat::Plain);
    for (x, z, r) in [(0.0f32, 0.0f32, 0.014f32), (0.012, 0.01, 0.01), (-0.01, -0.008, 0.011), (0.006, -0.014, 0.009)] {
        m.sphere(Vec3::new(x, 0.01, z), r, Color::srgb(0.05, 0.05, 0.05));
    }
    m
}

/// The shovel is held in front of the fireman's camera.
fn spawn_shovel(
    mut commands: Commands,
    mut done: Local<bool>,
    mats: Option<Res<Materials>>,
    mut meshes: ResMut<Assets<Mesh>>,
    cam: Query<Entity, With<Camera3d>>,
) {
    if *done {
        return;
    }
    let (Some(mats), Ok(cam)) = (mats, cam.single()) else { return };
    let shovel = spawn_parts(&mut commands, &mats, &meshes_of(&mut meshes, shovel_geo()), Transform::IDENTITY);
    let coal = spawn_parts(&mut commands, &mats, &meshes_of(&mut meshes, coal_geo()), Transform::IDENTITY);
    commands.entity(coal).insert((ShovelCoal, Visibility::Hidden));
    commands.entity(shovel).insert((Shovel, Visibility::Hidden)).add_child(coal);
    commands.entity(cam).add_child(shovel);
    *done = true;
}

fn pose(p: Vec3, yaw: f32, pitch: f32) -> Transform {
    Transform::from_translation(p).with_rotation(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch))
}

fn blend(a: Transform, b: Transform, t: f32) -> Transform {
    let s = t * t * (3.0 - 2.0 * t);
    Transform::from_translation(a.translation.lerp(b.translation, s)).with_rotation(a.rotation.slerp(b.rotation, s))
}

#[allow(clippy::too_many_arguments)]
fn animate_shovel(
    time: Res<Time>,
    cam: Res<CamState>,
    mut fireman: ResMut<Fireman>,
    mut trains: ResMut<Trains>,
    mut shovel: Query<(&mut Transform, &mut Visibility), (With<Shovel>, Without<ShovelCoal>)>,
    mut coal: Query<&mut Visibility, (With<ShovelCoal>, Without<Shovel>)>,
) {
    let dt = time.delta_secs();
    fireman.flare = (fireman.flare - dt * 1.5).max(0.0);
    let Ok((mut tf, mut vis)) = shovel.single_mut() else { return };
    let show = cam.mode == CamMode::Fireman;
    *vis = if show { Visibility::Inherited } else { Visibility::Hidden };
    let rest = pose(Vec3::new(0.07, -0.1, -0.2), 0.15, -0.15);
    let dig = pose(Vec3::new(0.2, -0.17, -0.1), -1.1, -0.35);
    let carry = pose(Vec3::new(0.05, -0.09, -0.2), 0.0, 0.1);
    let throw = pose(Vec3::new(0.0, -0.06, -0.33), 0.0, 0.5);
    let Some(u) = fireman.anim else {
        *tf = rest;
        if let Ok(mut c) = coal.single_mut() {
            *c = Visibility::Hidden;
        }
        return;
    };
    let u2 = u + dt / 1.1;
    *tf = match u2 {
        x if x < 0.25 => blend(rest, dig, x / 0.25),
        x if x < 0.4 => dig,
        x if x < 0.65 => blend(dig, carry, (x - 0.4) / 0.25),
        x if x < 0.78 => blend(carry, throw, (x - 0.65) / 0.13),
        x => blend(throw, rest, ((x - 0.78) / 0.22).min(1.0)),
    };
    let loaded = (0.32..0.75).contains(&u2);
    if let Ok(mut c) = coal.single_mut() {
        *c = if loaded { Visibility::Inherited } else { Visibility::Hidden };
    }
    // Into the firebox it goes.
    if u < 0.75 && u2 >= 0.75 {
        fireman.flare = 1.0;
        if let Some(t) = trains.selected.and_then(|id| trains.get_mut(id)) {
            if t.fuel_manual {
                t.shovel_coal();
                fireman.message = if t.fire > 1.0 { "Too much coal! Look at that black smoke!".into() } else { String::new() };
            }
        }
    }
    fireman.anim = if u2 >= 1.0 { None } else { Some(u2) };
}

/// The fire seen through the firebox door glows with how well it's burning.
fn fire_glow(time: Res<Time>, fireman: Res<Fireman>, trains: Res<Trains>, mats: Option<Res<Materials>>, mut assets: ResMut<Assets<StandardMaterial>>) {
    let Some(mats) = mats else { return };
    let fire = trains.selected().filter(|t| t.power_kind() == Power::Steam).map(|t| t.fire).unwrap_or(0.9);
    let t = time.elapsed_secs();
    let flicker = 0.85 + 0.15 * (t * 13.0).sin() * (t * 7.3).sin();
    let k = (0.15 + fire.min(1.1)) * flicker * (1.0 + fireman.flare * 1.5);
    if let Some(m) = assets.get_mut(&mats.get(Mat::Fire)) {
        m.emissive = LinearRgba::rgb(5.0 * k, 1.6 * k, 0.2 * k);
    }
}
