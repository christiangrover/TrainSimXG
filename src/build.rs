//! The toolbox: picking a tile under the mouse and placing / editing pieces.

use crate::camera::{CamMode, CamState};
use crate::grid::*;
use crate::track::*;
use crate::trains::*;
use crate::ui::{Toast, UiCapture};
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

pub struct BuildPlugin;

impl Plugin for BuildPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BuildState>().add_systems(Update, (pick_tile, use_tool, draw_preview).chain());
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Tool {
    Select,
    Track(TrackKind),
    Road,
    Decor(DecorKind),
    Raise,
    Lower,
    Flatten,
    Paint(Ground),
    TrackUp,
    TrackDown,
    Bulldoze,
    PlaceTrain,
}

impl Tool {
    pub fn label(self) -> String {
        match self {
            Tool::Select => "Select / flip switches".into(),
            Tool::Track(k) => k.label().into(),
            Tool::Road => "Road".into(),
            Tool::Decor(d) => d.label().into(),
            Tool::Raise => "Raise land".into(),
            Tool::Lower => "Lower land".into(),
            Tool::Flatten => "Flatten land".into(),
            Tool::Paint(g) => format!("Paint {:?}", g),
            Tool::TrackUp => "Raise track".into(),
            Tool::TrackDown => "Lower track".into(),
            Tool::Bulldoze => "Bulldozer".into(),
            Tool::PlaceTrain => "Place train".into(),
        }
    }
    fn drags(self) -> bool {
        matches!(self, Tool::Road | Tool::Decor(_) | Tool::Raise | Tool::Lower | Tool::Flatten | Tool::Paint(_) | Tool::Bulldoze | Tool::Track(TrackKind::Straight))
    }
    fn uses_brush(self) -> bool {
        matches!(self, Tool::Raise | Tool::Lower | Tool::Flatten | Tool::Paint(_))
    }
}

#[derive(Resource)]
pub struct BuildState {
    pub tool: Tool,
    pub rot: u8,
    pub brush: i32,
    pub hovered: Option<IVec2>,
    last: Option<IVec2>,
    flatten_to: i32,
    raise_timer: f32,
}

impl Default for BuildState {
    fn default() -> Self {
        BuildState { tool: Tool::Select, rot: 0, brush: 1, hovered: None, last: None, flatten_to: 0, raise_timer: 0.0 }
    }
}

fn pick_tile(
    windows: Query<&Window, With<PrimaryWindow>>,
    cams: Query<(&Camera, &GlobalTransform)>,
    grid: Res<Grid>,
    ui: Res<UiCapture>,
    cam_state: Res<CamState>,
    mut bs: ResMut<BuildState>,
) {
    bs.hovered = None;
    if ui.pointer || matches!(cam_state.mode, CamMode::Passenger | CamMode::Engineer) {
        return;
    }
    let Ok(win) = windows.single() else { return };
    let Some(cursor) = win.cursor_position() else { return };
    let Ok((cam, gt)) = cams.single() else { return };
    let Ok(ray) = cam.viewport_to_world(gt, cursor) else { return };
    // March along the ray until it dips below the ground (or a raised track).
    let top = MAX_LEVEL as f32 * LEVEL_H + 0.5;
    let dir = *ray.direction;
    let mut s = 0.0;
    if ray.origin.y > top {
        if dir.y >= 0.0 {
            return;
        }
        s = (ray.origin.y - top) / -dir.y;
    }
    let step = 0.04;
    for _ in 0..12000 {
        let p = ray.origin + dir * s;
        if p.y < -0.5 {
            break;
        }
        let t = world_to_tile(p);
        if let Some(tile) = grid.get(t) {
            let mut h = tile.surface_y();
            if let Some(tr) = tile.track() {
                h = h.max(tr.max_level() as f32 * LEVEL_H + 0.05);
            }
            if p.y <= h {
                bs.hovered = Some(t);
                return;
            }
        } else if dir.y >= 0.0 && p.y > top {
            break;
        }
        s += step;
    }
}

/// Track level that lines up with neighbouring track, else the ground.
fn auto_level(grid: &Grid, t: IVec2, piece: &Track) -> i32 {
    for e in piece.edges() {
        let n = t + edge_dir(e);
        if let Some(nt) = grid.track(n) {
            if nt.edges().contains(&opposite(e)) {
                let lvl = nt.edge_level(opposite(e));
                return lvl - (piece.edge_level(e) - piece.level);
            }
        }
    }
    grid.height(t)
}

fn use_tool(
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    ui: Res<UiCapture>,
    mut bs: ResMut<BuildState>,
    mut grid: ResMut<Grid>,
    mut trains: ResMut<Trains>,
    consist: Res<Consist>,
    mut toast: ResMut<Toast>,
) {
    if !ui.keyboard {
        if keys.just_pressed(KeyCode::KeyR) {
            bs.rot = (bs.rot + 1) % 4;
        }
        if keys.just_pressed(KeyCode::Escape) {
            bs.tool = Tool::Select;
        }
        if keys.just_pressed(KeyCode::BracketRight) {
            bs.brush = (bs.brush + 1).min(6);
        }
        if keys.just_pressed(KeyCode::BracketLeft) {
            bs.brush = (bs.brush - 1).max(1);
        }
    }
    let Some(t) = bs.hovered else {
        bs.last = None;
        return;
    };
    let pressed = mouse.just_pressed(MouseButton::Left);
    let held = mouse.pressed(MouseButton::Left);
    if !held {
        bs.last = None;
        return;
    }
    let tool = bs.tool;
    // Terrain tools repeat while held; others apply once per tile while dragging.
    let mut apply = pressed;
    if !pressed && tool.drags() && bs.last != Some(t) {
        apply = true;
    }
    if matches!(tool, Tool::Raise | Tool::Lower) {
        bs.raise_timer -= time.delta_secs();
        if pressed || bs.raise_timer <= 0.0 {
            apply = true;
        }
    }
    if !apply {
        return;
    }
    if matches!(tool, Tool::Raise | Tool::Lower) {
        bs.raise_timer = 0.18;
    }
    if pressed && tool == Tool::Flatten {
        bs.flatten_to = grid.height(t);
    }
    bs.last = Some(t);

    let brush_tiles = || {
        let r = bs.brush - 1;
        let mut v = vec![];
        for dz in -r..=r {
            for dx in -r..=r {
                if dx * dx + dz * dz <= r * r + r {
                    v.push(t + IVec2::new(dx, dz));
                }
            }
        }
        v
    };

    match tool {
        Tool::Select => {
            if let Some(tr) = grid.track(t).copied() {
                if tr.is_switch() {
                    if let Some(x) = grid.edit(t).and_then(|x| x.track_mut()) {
                        x.diverge = !x.diverge;
                    }
                    return;
                }
            }
            // Select the nearest train.
            let c = tile_center(t);
            let mut best = None;
            for tr in &trains.list {
                for car in &tr.cars {
                    let p = car.pos.world(&grid).0;
                    let d = Vec2::new(p.x - c.x, p.z - c.z).length();
                    if d < 1.5 && best.map(|(bd, _)| d < bd).unwrap_or(true) {
                        best = Some((d, tr.id));
                    }
                }
            }
            if let Some((_, id)) = best {
                trains.selected = Some(id);
            }
        }
        Tool::Track(kind) => {
            let Some(tile) = grid.get(t) else { return };
            if tile.ground == Ground::Water && tile.track().is_none() {
                // Building over water is fine: it becomes a bridge.
            }
            let mut piece = Track::new(kind, bs.rot, 0);
            piece.level = auto_level(&grid, t, &piece).clamp(0, MAX_LEVEL - 1);
            if let Some(x) = grid.edit(t) {
                x.content = Content::Track(piece);
            }
        }
        Tool::Road => {
            if grid.get(t).map(|x| x.ground == Ground::Water).unwrap_or(true) {
                toast.show("Roads can't go on water - use a road crossing on a bridge instead.");
                return;
            }
            if let Some(x) = grid.edit(t) {
                if x.track().is_some() {
                    toast.show("Use the 'Road crossing' track piece to cross a railway.");
                    return;
                }
                x.content = Content::Road;
            }
        }
        Tool::Decor(kind) => {
            let mut rot = bs.rot;
            if kind == DecorKind::Port {
                // Ports sit on the shore, crane and quay facing the water.
                match grid.water_edge(t) {
                    Some(e) => rot = (e + 2) % 4,
                    None => {
                        if pressed {
                            toast.show("A port has to go right beside water.");
                        }
                        return;
                    }
                }
            }
            let variant = (t.x * 31 + t.y * 17).unsigned_abs() as u8;
            if let Some(x) = grid.edit(t) {
                if x.ground == Ground::Water || x.track().is_some() {
                    return;
                }
                x.content = Content::Decor { kind, rot, variant };
            }
        }
        Tool::Raise | Tool::Lower | Tool::Flatten => {
            for b in brush_tiles() {
                let target = bs.flatten_to;
                if let Some(x) = grid.edit(b) {
                    x.height = match tool {
                        Tool::Raise => (x.height + 1).min(MAX_LEVEL),
                        Tool::Lower => (x.height - 1).max(0),
                        _ => target,
                    };
                    if x.ground == Ground::Water && tool == Tool::Raise {
                        x.ground = Ground::Grass;
                    }
                }
            }
        }
        Tool::Paint(g) => {
            for b in brush_tiles() {
                if let Some(x) = grid.edit(b) {
                    x.ground = g;
                    if g == Ground::Water && !matches!(x.content, Content::Track(_)) {
                        x.content = Content::Empty;
                    }
                }
            }
        }
        Tool::TrackUp | Tool::TrackDown => {
            if let Some(x) = grid.edit(t).and_then(|x| x.track_mut()) {
                x.level = if tool == Tool::TrackUp { (x.level + 1).min(MAX_LEVEL - 1) } else { (x.level - 1).max(0) };
            } else {
                toast.show("Click on a track piece to raise or lower it.");
            }
        }
        Tool::Bulldoze => {
            if let Some(x) = grid.edit(t) {
                x.content = Content::Empty;
            }
        }
        Tool::PlaceTrain => match trains.spawn(&grid, t, &consist.cars) {
            Ok(_) => {
                toast.show("Train placed! Press 3, 4 or 5 to ride along.");
                bs.tool = Tool::Select;
            }
            Err(e) => toast.show(&e),
        },
    }
}

fn draw_preview(mut gizmos: Gizmos, bs: Res<BuildState>, grid: Res<Grid>) {
    let Some(t) = bs.hovered else { return };
    let tile = grid.get(t).copied().unwrap_or_default();
    let c = tile_center(t);
    let y = tile.surface_y().max(tile.track().map(|tr| tr.max_level() as f32 * LEVEL_H).unwrap_or(0.0)) + 0.03;
    let color = match bs.tool {
        Tool::Bulldoze => Color::srgb(1.0, 0.2, 0.2),
        Tool::Select => Color::srgb(1.0, 1.0, 1.0),
        _ => Color::srgb(1.0, 0.9, 0.2),
    };
    let r = if bs.tool.uses_brush() { bs.brush - 1 } else { 0 };
    let h = 0.5 + r as f32;
    let corners = [Vec3::new(-h, 0.0, -h), Vec3::new(h, 0.0, -h), Vec3::new(h, 0.0, h), Vec3::new(-h, 0.0, h)];
    for i in 0..4 {
        gizmos.line(c + corners[i] + Vec3::Y * y, c + corners[(i + 1) % 4] + Vec3::Y * y, color);
    }
    // Show the rails of the piece about to be placed.
    if let Tool::Track(kind) = bs.tool {
        let mut piece = Track::new(kind, bs.rot, 0);
        piece.level = auto_level(&grid, t, &piece);
        for (a, b) in piece.paths() {
            let ya = piece.edge_level(a) as f32 * LEVEL_H + 0.1;
            let yb = piece.edge_level(b) as f32 * LEVEL_H + 0.1;
            let pts: Vec<Vec3> = (0..=10).map(|i| c + path_local(a, b, i as f32 / 10.0, ya, yb).0).collect();
            gizmos.linestrip(pts, Color::srgb(0.2, 1.0, 1.0));
        }
        if kind == TrackKind::Ramp {
            let hi = edge_mid(piece.edges()[0]);
            gizmos.arrow(c + Vec3::Y * (y + 0.1), c + Vec3::new(hi.x, y + 0.35, hi.y), Color::srgb(1.0, 0.4, 0.1));
        }
    }
    if let Tool::Decor(_) = bs.tool {
        let f = rot_quat(bs.rot) * Vec3::Z * 0.45;
        gizmos.arrow(c + Vec3::Y * y, c + f + Vec3::Y * y, color);
    }
}
