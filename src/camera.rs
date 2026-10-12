//! Camera views: overview (orbit), walking around the table, following a
//! train, sitting in a passenger car, and driving from the engineer's cab.

use crate::grid::*;
use crate::scene::FLOOR_Y;
use crate::trains::*;
use crate::ui::UiCapture;
use bevy::core_pipeline::bloom::Bloom;
use bevy::core_pipeline::prepass::{DepthPrepass, NormalPrepass};
use bevy::core_pipeline::smaa::Smaa;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::pbr::ScreenSpaceAmbientOcclusion;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll};
use bevy::prelude::*;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CamState>()
            .add_systems(Startup, spawn_camera)
            .add_systems(Update, (camera_input, engineer_controls, apply_camera).chain());
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CamMode {
    Overview,
    Walk,
    Follow,
    Passenger,
    Engineer,
}

impl CamMode {
    pub const ALL: [CamMode; 5] = [CamMode::Overview, CamMode::Walk, CamMode::Follow, CamMode::Passenger, CamMode::Engineer];
    pub fn label(self) -> &'static str {
        match self {
            CamMode::Overview => "1 Overview",
            CamMode::Walk => "2 Walk around",
            CamMode::Follow => "3 Follow train",
            CamMode::Passenger => "4 Passenger",
            CamMode::Engineer => "5 Engineer",
        }
    }
    pub fn needs_train(self) -> bool {
        matches!(self, CamMode::Follow | CamMode::Passenger | CamMode::Engineer)
    }
}

#[derive(Resource)]
pub struct CamState {
    pub mode: CamMode,
    pub focus: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub dist: f32,
    pub walk_pos: Vec3,
    pub walk_yaw: f32,
    pub walk_pitch: f32,
    pub follow_yaw: f32,
    pub follow_pitch: f32,
    pub follow_dist: f32,
    /// Look-around offsets inside a train.
    pub look_yaw: f32,
    pub look_pitch: f32,
    pub passenger_car: usize,
    /// Passengers look out of the right (true) or left window.
    pub right_window: bool,
}

impl Default for CamState {
    fn default() -> Self {
        CamState {
            mode: CamMode::Overview,
            focus: Vec3::new(0.0, 0.0, 0.0),
            yaw: 0.6,
            pitch: 0.75,
            dist: 45.0,
            walk_pos: Vec3::new(0.0, 18.0, 80.0),
            walk_yaw: 0.0,
            walk_pitch: -0.3,
            follow_yaw: 0.6,
            follow_pitch: 0.35,
            follow_dist: 3.5,
            look_yaw: 0.0,
            look_pitch: 0.0,
            passenger_car: 0,
            right_window: true,
        }
    }
}

impl CamState {
    pub fn set_mode(&mut self, mode: CamMode) {
        self.mode = mode;
        self.look_yaw = 0.0;
        self.look_pitch = 0.0;
    }
}

fn spawn_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Camera { hdr: true, ..default() },
        Projection::Perspective(PerspectiveProjection { near: 0.01, far: 2000.0, fov: 60f32.to_radians(), ..default() }),
        Tonemapping::TonyMcMapface,
        // Soft contact shadows, glowing lights, and smooth edges.
        Msaa::Off,
        Smaa::default(),
        DepthPrepass,
        NormalPrepass,
        ScreenSpaceAmbientOcclusion::default(),
        Bloom::NATURAL,
        Transform::from_xyz(0.0, 30.0, 40.0).looking_at(Vec3::ZERO, Vec3::Y),
        DistanceFog { color: Color::srgb(0.55, 0.7, 0.85), falloff: FogFalloff::Linear { start: 250.0, end: 600.0 }, ..default() },
    ));
}

fn camera_input(
    keys: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    time: Res<Time>,
    ui: Res<UiCapture>,
    mut cam: ResMut<CamState>,
    mut trains: ResMut<Trains>,
) {
    let dt = time.delta_secs();
    if !ui.keyboard {
        for (k, m) in [
            (KeyCode::Digit1, CamMode::Overview),
            (KeyCode::Digit2, CamMode::Walk),
            (KeyCode::Digit3, CamMode::Follow),
            (KeyCode::Digit4, CamMode::Passenger),
            (KeyCode::Digit5, CamMode::Engineer),
        ] {
            if keys.just_pressed(k) {
                cam.set_mode(m);
            }
        }
        // Cycle through trains.
        if keys.just_pressed(KeyCode::KeyT) && !trains.list.is_empty() {
            let idx = trains.selected.and_then(|s| trains.list.iter().position(|t| t.id == s)).map(|i| (i + 1) % trains.list.len()).unwrap_or(0);
            trains.selected = Some(trains.list[idx].id);
        }
        if keys.just_pressed(KeyCode::Tab) {
            if let Some(t) = trains.selected() {
                let n = t.cars.len();
                let mut i = cam.passenger_car;
                for _ in 0..n {
                    i = (i + 1) % n;
                    if t.cars[i].kind.has_seats() {
                        break;
                    }
                }
                cam.passenger_car = i;
            }
        }
        if keys.just_pressed(KeyCode::KeyV) {
            cam.right_window = !cam.right_window;
        }
    }
    if cam.mode.needs_train() && trains.selected().is_none() {
        cam.mode = CamMode::Overview;
    }
    // Passengers sit in a car with seats, not in the engine.
    if cam.mode == CamMode::Passenger {
        if let Some(t) = trains.selected() {
            let ok = t.cars.get(cam.passenger_car).is_some_and(|c| c.kind.has_seats());
            if !ok {
                cam.passenger_car = t.cars.iter().position(|c| c.kind.has_seats()).unwrap_or(t.cars.len() / 2);
            }
        }
    }

    let drag = if mouse.pressed(MouseButton::Right) || mouse.pressed(MouseButton::Middle) { motion.delta * 0.005 } else { Vec2::ZERO };
    let wheel = if ui.pointer { 0.0 } else { scroll.delta.y };
    let shift = keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight);
    let key = |k: KeyCode| !ui.keyboard && keys.pressed(k);

    match cam.mode {
        CamMode::Overview => {
            cam.yaw -= drag.x;
            cam.pitch = (cam.pitch + drag.y).clamp(0.08, 1.52);
            if key(KeyCode::KeyQ) {
                cam.yaw += 1.5 * dt;
            }
            if key(KeyCode::KeyE) {
                cam.yaw -= 1.5 * dt;
            }
            cam.dist = (cam.dist * (1.0 - wheel * 0.1)).clamp(2.0, 160.0);
            let fwd = Vec3::new(-cam.yaw.sin(), 0.0, -cam.yaw.cos());
            let right = Vec3::new(-fwd.z, 0.0, fwd.x);
            let mut mv = Vec3::ZERO;
            if key(KeyCode::KeyW) || key(KeyCode::ArrowUp) {
                mv += fwd;
            }
            if key(KeyCode::KeyS) || key(KeyCode::ArrowDown) {
                mv -= fwd;
            }
            if key(KeyCode::KeyD) || key(KeyCode::ArrowRight) {
                mv += right;
            }
            if key(KeyCode::KeyA) || key(KeyCode::ArrowLeft) {
                mv -= right;
            }
            let speed = cam.dist * 0.8 * if shift { 2.5 } else { 1.0 };
            cam.focus += mv.normalize_or_zero() * speed * dt;
            let h = GRID as f32 / 2.0 + 5.0;
            cam.focus.x = cam.focus.x.clamp(-h, h);
            cam.focus.z = cam.focus.z.clamp(-h, h);
        }
        CamMode::Walk => {
            cam.walk_yaw -= drag.x;
            cam.walk_pitch = (cam.walk_pitch - drag.y).clamp(-1.4, 1.4);
            // Scroll to crouch down or stand on tiptoes.
            cam.walk_pos.y = (cam.walk_pos.y + wheel * 1.5).clamp(1.0, 45.0);
            let fwd = Vec3::new(-cam.walk_yaw.sin(), 0.0, -cam.walk_yaw.cos());
            let right = Vec3::new(-fwd.z, 0.0, fwd.x);
            let mut mv = Vec3::ZERO;
            if key(KeyCode::KeyW) || key(KeyCode::ArrowUp) {
                mv += fwd;
            }
            if key(KeyCode::KeyS) || key(KeyCode::ArrowDown) {
                mv -= fwd;
            }
            if key(KeyCode::KeyD) || key(KeyCode::ArrowRight) {
                mv += right;
            }
            if key(KeyCode::KeyA) || key(KeyCode::ArrowLeft) {
                mv -= right;
            }
            let speed = if shift { 60.0 } else { 25.0 };
            let mut p = cam.walk_pos + mv.normalize_or_zero() * speed * dt;
            // Can't walk through the table: stay outside its edge.
            let edge = GRID as f32 / 2.0 + 4.0;
            if p.x.abs() < edge && p.z.abs() < edge {
                if edge - p.x.abs() < edge - p.z.abs() {
                    p.x = edge * p.x.signum();
                } else {
                    p.z = edge * p.z.signum();
                }
            }
            p.x = p.x.clamp(-150.0, 150.0);
            p.z = p.z.clamp(-150.0, 150.0);
            cam.walk_pos = p;
        }
        CamMode::Follow => {
            cam.follow_yaw -= drag.x;
            cam.follow_pitch = (cam.follow_pitch + drag.y).clamp(0.02, 1.5);
            cam.follow_dist = (cam.follow_dist * (1.0 - wheel * 0.1)).clamp(0.8, 40.0);
        }
        CamMode::Passenger | CamMode::Engineer => {
            cam.look_yaw -= drag.x;
            cam.look_pitch = (cam.look_pitch - drag.y).clamp(-1.2, 1.2);
        }
    }
}

fn engineer_controls(keys: Res<ButtonInput<KeyCode>>, ui: Res<UiCapture>, cam: Res<CamState>, mut trains: ResMut<Trains>) {
    if cam.mode != CamMode::Engineer || ui.keyboard {
        return;
    }
    let Some(id) = trains.selected else { return };
    let Some(t) = trains.get_mut(id) else { return };
    t.manual = true;
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

fn apply_camera(
    time: Res<Time>,
    cam: Res<CamState>,
    grid: Res<Grid>,
    trains: Res<Trains>,
    mut q: Query<&mut Transform, With<Camera3d>>,
) {
    let Ok(mut tf) = q.single_mut() else { return };
    let dt = time.delta_secs();
    let car_tf = |idx: Option<usize>| -> Option<(Transform, crate::models_vehicle::CarKind)> {
        let t = trains.selected()?;
        let i = idx.unwrap_or(0).min(t.cars.len().saturating_sub(1));
        let c = t.cars.get(i)?;
        Some((car_transform(&grid, c), c.kind))
    };
    let target: Transform;
    let mut smooth = false;
    match cam.mode {
        CamMode::Overview => {
            let off = Vec3::new(cam.yaw.sin() * cam.pitch.cos(), cam.pitch.sin(), cam.yaw.cos() * cam.pitch.cos()) * cam.dist;
            target = Transform::from_translation(cam.focus + off).looking_at(cam.focus, Vec3::Y);
            smooth = true;
        }
        CamMode::Walk => {
            let p = cam.walk_pos;
            let dir = Vec3::new(-cam.walk_yaw.sin() * cam.walk_pitch.cos(), cam.walk_pitch.sin(), -cam.walk_yaw.cos() * cam.walk_pitch.cos());
            target = Transform::from_translation(p).looking_to(dir, Vec3::Y);
            let _ = FLOOR_Y;
        }
        CamMode::Follow => {
            let Some((ct, _)) = car_tf(None) else { return };
            let fwd = ct.rotation * Vec3::Z;
            let base_yaw = fwd.x.atan2(fwd.z) + std::f32::consts::PI;
            let yaw = base_yaw + cam.follow_yaw;
            let off = Vec3::new(yaw.sin() * cam.follow_pitch.cos(), cam.follow_pitch.sin(), yaw.cos() * cam.follow_pitch.cos()) * cam.follow_dist;
            let look = ct.translation + Vec3::Y * 0.25;
            target = Transform::from_translation(look + off).looking_at(look, Vec3::Y);
            smooth = true;
        }
        CamMode::Passenger => {
            let Some((ct, _)) = car_tf(Some(cam.passenger_car)) else { return };
            let side = if cam.right_window { -1.0 } else { 1.0 };
            let eye = ct.transform_point(Vec3::new(0.06 * side, 0.27, 0.0));
            let local = Quat::from_rotation_y(side * std::f32::consts::FRAC_PI_2 + cam.look_yaw) * Quat::from_rotation_x(-cam.look_pitch);
            target = Transform::from_translation(eye).with_rotation(ct.rotation * local * Quat::from_rotation_y(std::f32::consts::PI));
        }
        CamMode::Engineer => {
            let Some((ct, kind)) = car_tf(Some(0)) else { return };
            let eye = ct.transform_point(kind.cab_eye());
            let local = Quat::from_rotation_y(cam.look_yaw) * Quat::from_rotation_x(-cam.look_pitch);
            target = Transform::from_translation(eye).with_rotation(ct.rotation * local * Quat::from_rotation_y(std::f32::consts::PI));
        }
    }
    // Never let an outside camera dip into a hill or under a bridge deck.
    if smooth {
        let mut target = target;
        let t = world_to_tile(target.translation);
        if let Some(tile) = grid.get(t) {
            let mut ground = tile.surface_y();
            if let Some(tr) = tile.track() {
                ground = ground.max(tr.max_level() as f32 * LEVEL_H);
            }
            if target.translation.y < ground + 0.3 {
                let look = target.translation + target.forward() * 2.0;
                target.translation.y = ground + 0.3;
                target.look_at(look, Vec3::Y);
            }
        }
        let k = 1.0 - (-12.0 * dt).exp();
        tf.translation = tf.translation.lerp(target.translation, k);
        tf.rotation = tf.rotation.slerp(target.rotation, k);
    } else {
        *tf = target;
    }
}
