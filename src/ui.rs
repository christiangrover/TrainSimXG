//! egui panels: toolbox, train library, train controls, save / load, and the
//! passenger-window and engineer-cab overlays.

use crate::build::{BuildState, Tool};
use crate::camera::{CamMode, CamState};
use crate::demo::build_demo;
use crate::grid::*;
use crate::models_vehicle::CarKind;
use crate::roads::Traffic;
use crate::save;
use crate::trains::*;
use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts, EguiPrimaryContextPass};

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<UiCapture>()
            .init_resource::<Toast>()
            .init_resource::<SaveUi>()
            .add_systems(EguiPrimaryContextPass, ui_system);
    }
}

/// Whether egui is using the mouse / keyboard this frame (so the world ignores it).
#[derive(Resource, Default)]
pub struct UiCapture {
    pub pointer: bool,
    pub keyboard: bool,
}

#[derive(Resource, Default)]
pub struct Toast {
    msg: String,
    left: f32,
}

impl Toast {
    pub fn show(&mut self, msg: &str) {
        self.msg = msg.to_string();
        self.left = 3.5;
    }
}

#[derive(Resource)]
struct SaveUi {
    name: String,
    saves: Vec<String>,
    confirm_new: bool,
}

impl Default for SaveUi {
    fn default() -> Self {
        SaveUi { name: "my layout".into(), saves: save::list_saves(), confirm_new: false }
    }
}

fn tool_button(ui: &mut egui::Ui, bs: &mut BuildState, tool: Tool) {
    if ui.selectable_label(bs.tool == tool, tool.label()).clicked() {
        bs.tool = tool;
    }
}

#[allow(clippy::too_many_arguments)]
fn ui_system(
    mut contexts: EguiContexts,
    time: Res<Time>,
    mut cam: ResMut<CamState>,
    mut bs: ResMut<BuildState>,
    mut trains: ResMut<Trains>,
    mut sim: ResMut<Sim>,
    mut consist: ResMut<Consist>,
    mut grid: ResMut<Grid>,
    mut traffic: ResMut<Traffic>,
    mut toast: ResMut<Toast>,
    mut sui: ResMut<SaveUi>,
    mut capture: ResMut<UiCapture>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    // ---------------------------------------------------------------- top bar
    egui::TopBottomPanel::top("top").show(ctx, |ui| {
        ui.horizontal_wrapped(|ui| {
            ui.heading("Table Top Trains");
            ui.separator();
            let label = if sim.running { "⏸ Pause" } else { "▶ Run" };
            if ui.button(label).clicked() {
                sim.running = !sim.running;
            }
            ui.add(egui::Slider::new(&mut sim.time_scale, 0.25..=4.0).text("sim speed").logarithmic(true));
            ui.separator();
            ui.label("View:");
            for m in CamMode::ALL {
                let enabled = !m.needs_train() || trains.selected().is_some();
                if ui.add_enabled(enabled, egui::SelectableLabel::new(cam.mode == m, m.label())).clicked() {
                    cam.set_mode(m);
                }
            }
            ui.separator();
            ui.add(egui::TextEdit::singleline(&mut sui.name).desired_width(110.0));
            if ui.button("💾 Save").clicked() {
                match save::save(&sui.name, &grid, &trains) {
                    Ok(p) => toast.show(&format!("Saved to {}", p.display())),
                    Err(e) => toast.show(&e),
                }
                sui.saves = save::list_saves();
            }
            let mut to_load = None;
            egui::ComboBox::from_id_salt("load").selected_text("📂 Load…").show_ui(ui, |ui| {
                if sui.saves.is_empty() {
                    ui.label("No saved layouts yet");
                }
                for s in &sui.saves {
                    if ui.selectable_label(false, s).clicked() {
                        to_load = Some(s.clone());
                    }
                }
            });
            if let Some(name) = to_load {
                match save::load(&name, &mut grid, &mut trains) {
                    Ok(()) => {
                        toast.show(&format!("Loaded '{name}'"));
                        sui.name = name;
                    }
                    Err(e) => toast.show(&e),
                }
            }
            if ui.button("🔄").on_hover_text("Refresh the list of saves").clicked() {
                sui.saves = save::list_saves();
            }
            ui.separator();
            if sui.confirm_new {
                ui.label("Clear the whole table?");
                if ui.button("Yes, clear").clicked() {
                    grid.replace_all(vec![Tile::default(); (GRID * GRID) as usize]);
                    trains.list.clear();
                    trains.selected = None;
                    trains.dirty = true;
                    sui.confirm_new = false;
                }
                if ui.button("Demo layout").clicked() {
                    build_demo(&mut grid, &mut trains);
                    sui.confirm_new = false;
                }
                if ui.button("Cancel").clicked() {
                    sui.confirm_new = false;
                }
            } else if ui.button("🆕 New…").clicked() {
                sui.confirm_new = true;
            }
        });
    });

    // ------------------------------------------------------------- bottom help
    egui::TopBottomPanel::bottom("help").show(ctx, |ui| {
        let txt = match cam.mode {
            CamMode::Overview => "WASD/arrows pan · Q/E or right-drag rotate · wheel zoom · left-click use tool · R rotate piece · [ ] brush size · Esc select tool · T next train",
            CamMode::Walk => "WASD walk around the table · right-drag look · wheel crouch / stand tall · Shift run · left-click use tool",
            CamMode::Follow => "Right-drag orbit the train · wheel zoom · T next train",
            CamMode::Passenger => "Right-drag look around · Tab change car · V other window · T next train",
            CamMode::Engineer => "W / Up throttle up · S / Down throttle down (below zero = reverse) · X shut off · hold Space brake · right-drag look",
        };
        ui.label(txt);
    });

    // ----------------------------------------------------------------- toolbox
    egui::SidePanel::left("toolbox").default_width(190.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Toolbox");
            ui.label(format!("Rotation: {}° (R)", bs.rot as u32 * 90));
            tool_button(ui, &mut bs, Tool::Select);
            tool_button(ui, &mut bs, Tool::Bulldoze);
            egui::CollapsingHeader::new("Track").default_open(true).show(ui, |ui| {
                for k in TrackKind::ALL {
                    tool_button(ui, &mut bs, Tool::Track(k));
                }
                ui.small("Track joins up at the height of the track next to it. Ramps climb one level toward the arrow.");
                tool_button(ui, &mut bs, Tool::TrackUp);
                tool_button(ui, &mut bs, Tool::TrackDown);
                ui.small("Track above the ground becomes a bridge; track below the ground becomes a tunnel.");
            });
            egui::CollapsingHeader::new("Roads").default_open(true).show(ui, |ui| {
                tool_button(ui, &mut bs, Tool::Road);
                tool_button(ui, &mut bs, Tool::Track(TrackKind::LevelCrossing));
            });
            egui::CollapsingHeader::new("Terrain").default_open(true).show(ui, |ui| {
                tool_button(ui, &mut bs, Tool::Raise);
                tool_button(ui, &mut bs, Tool::Lower);
                tool_button(ui, &mut bs, Tool::Flatten);
                for g in [Ground::Grass, Ground::Sand, Ground::Rock, Ground::Water] {
                    tool_button(ui, &mut bs, Tool::Paint(g));
                }
                ui.add(egui::Slider::new(&mut bs.brush, 1..=6).text("brush"));
            });
            egui::CollapsingHeader::new("Buildings").default_open(true).show(ui, |ui| {
                for d in DecorKind::BUILDINGS {
                    tool_button(ui, &mut bs, Tool::Decor(d));
                }
            });
            egui::CollapsingHeader::new("Emergency services").default_open(true).show(ui, |ui| {
                for d in DecorKind::EMERGENCY {
                    tool_button(ui, &mut bs, Tool::Decor(d));
                }
                ui.small("Each one sends its own vehicles out onto nearby roads.");
            });
            egui::CollapsingHeader::new("Nature & details").default_open(false).show(ui, |ui| {
                for d in DecorKind::NATURE {
                    tool_button(ui, &mut bs, Tool::Decor(d));
                }
            });
        });
    });

    // --------------------------------------------------- train library & yard
    egui::SidePanel::right("trains").default_width(250.0).show(ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Train library");
            ui.label("Click to add cars to the train:");
            egui::Grid::new("lib").num_columns(2).show(ui, |ui| {
                for (i, k) in CarKind::ALL.iter().enumerate() {
                    if ui.button(format!("+ {}", k.label())).clicked() {
                        consist.cars.push(*k);
                    }
                    if i % 2 == 1 {
                        ui.end_row();
                    }
                }
            });
            ui.add_space(4.0);
            ui.label("Your train (front first):");
            let mut remove = None;
            let mut swap = None;
            for (i, k) in consist.cars.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui.small_button("✖").clicked() {
                        remove = Some(i);
                    }
                    if ui.small_button("⬆").clicked() && i > 0 {
                        swap = Some(i);
                    }
                    ui.label(format!("{}. {}", i + 1, k.label()));
                });
            }
            if let Some(i) = remove {
                consist.cars.remove(i);
            }
            if let Some(i) = swap {
                consist.cars.swap(i, i - 1);
            }
            ui.horizontal(|ui| {
                if ui.button("Clear").clicked() {
                    consist.cars.clear();
                }
                let placing = bs.tool == Tool::PlaceTrain;
                if ui.selectable_label(placing, "Place on track…").clicked() {
                    bs.tool = Tool::PlaceTrain;
                    toast.show("Click a piece of track to put the train there.");
                }
            });
            if !consist.cars.iter().any(|c| c.is_engine()) && !consist.cars.is_empty() {
                ui.colored_label(egui::Color32::YELLOW, "No engine - it will still roll, like magic!");
            }

            ui.separator();
            ui.heading("Trains on the table");
            if trains.list.is_empty() {
                ui.label("No trains yet.");
            }
            let selected = trains.selected;
            let mut select = None;
            let mut delete = None;
            for t in trains.list.iter_mut() {
                let is_sel = selected == Some(t.id);
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if ui.selectable_label(is_sel, &t.name).clicked() {
                            select = Some(t.id);
                        }
                        if ui.small_button("🗑").on_hover_text("Remove train").clicked() {
                            delete = Some(t.id);
                        }
                    });
                    ui.label(format!("{} cars · {:.1} tiles/s", t.cars.len(), t.speed.abs()));
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut t.manual, false, "Auto");
                        ui.selectable_value(&mut t.manual, true, "Manual");
                    });
                    if t.manual {
                        ui.add(egui::Slider::new(&mut t.throttle, -1.0..=1.0).text("throttle"));
                        ui.horizontal(|ui| {
                            if ui.button("◀ Back").clicked() {
                                t.throttle = -0.5;
                            }
                            if ui.button("■ Stop").clicked() {
                                t.throttle = 0.0;
                            }
                            if ui.button("Fwd ▶").clicked() {
                                t.throttle = 0.5;
                            }
                        });
                    } else {
                        ui.add(egui::Slider::new(&mut t.cruise, 0.0..=MAX_SPEED).text("speed"));
                        if ui.button("⇄ Reverse").clicked() {
                            t.auto_dir = -t.auto_dir;
                        }
                    }
                    if !t.status.is_empty() {
                        ui.small(&t.status);
                    }
                });
            }
            if let Some(id) = select {
                trains.selected = Some(id);
            }
            if let Some(id) = delete {
                trains.remove(id);
            }

            ui.separator();
            ui.heading("Road traffic");
            ui.add(egui::Slider::new(&mut traffic.target, 0..=80).text("cars"));
        });
    });

    // ------------------------------------------------------------- overlays
    let area = ctx.available_rect();
    let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Background, egui::Id::new("overlay")));
    // Riding through a tunnel: it's dark in here!
    if matches!(cam.mode, CamMode::Passenger | CamMode::Engineer) {
        let idx = if cam.mode == CamMode::Engineer { 0 } else { cam.passenger_car };
        if let Some(car) = trains.selected().and_then(|t| t.cars.get(idx)) {
            if grid.is_tunnel(car.pos.tile) {
                painter.rect_filled(area, 0.0, egui::Color32::from_rgba_unmultiplied(8, 6, 4, 248));
                painter.text(area.center(), egui::Align2::CENTER_CENTER, "~ tunnel ~", egui::FontId::proportional(22.0), egui::Color32::from_gray(90));
            }
        }
    }
    match cam.mode {
        CamMode::Passenger => {
            // Window frame around the view.
            let wood = egui::Color32::from_rgb(70, 45, 28);
            let t = 34.0;
            let r = area;
            painter.rect_filled(egui::Rect::from_min_max(r.min, egui::pos2(r.max.x, r.min.y + t * 1.6)), 0.0, wood);
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(r.min.x, r.max.y - t * 2.2), r.max), 0.0, wood);
            painter.rect_filled(egui::Rect::from_min_max(r.min, egui::pos2(r.min.x + t, r.max.y)), 0.0, wood);
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(r.max.x - t, r.min.y), r.max), 0.0, wood);
            let mid = r.center().x;
            painter.rect_filled(egui::Rect::from_min_max(egui::pos2(mid - t * 0.3, r.min.y), egui::pos2(mid + t * 0.3, r.max.y)), 0.0, wood);
            let sill = egui::Rect::from_min_max(egui::pos2(r.min.x, r.max.y - t * 2.2), egui::pos2(r.max.x, r.max.y - t * 1.8));
            painter.rect_filled(sill, 0.0, egui::Color32::from_rgb(110, 75, 45));
            if let Some(tr) = trains.selected() {
                let car = tr.cars.get(cam.passenger_car).map(|c| c.kind.label()).unwrap_or("");
                painter.text(egui::pos2(mid, r.max.y - t), egui::Align2::CENTER_CENTER, format!("{} - {}", tr.name, car), egui::FontId::proportional(18.0), egui::Color32::from_rgb(240, 220, 170));
            }
        }
        CamMode::Engineer => {
            if let Some(tr) = trains.selected() {
                let panel = egui::Rect::from_min_size(egui::pos2(area.center().x - 170.0, area.max.y - 100.0), egui::vec2(340.0, 90.0));
                painter.rect_filled(panel, 8.0, egui::Color32::from_rgba_unmultiplied(20, 20, 20, 200));
                let dir = if tr.throttle > 0.0 {
                    "AHEAD"
                } else if tr.throttle < 0.0 {
                    "REVERSE"
                } else {
                    "NEUTRAL"
                };
                let brake = if tr.brake { "  BRAKE" } else { "" };
                painter.text(panel.min + egui::vec2(14.0, 12.0), egui::Align2::LEFT_TOP, format!("{}  ·  {:.1} tiles/s", tr.name, tr.speed.abs()), egui::FontId::proportional(17.0), egui::Color32::WHITE);
                painter.text(panel.min + egui::vec2(14.0, 36.0), egui::Align2::LEFT_TOP, format!("Throttle {:+.0}%  {dir}{brake}", tr.throttle * 100.0), egui::FontId::monospace(16.0), egui::Color32::from_rgb(255, 210, 90));
                // Throttle bar.
                let bar = egui::Rect::from_min_size(panel.min + egui::vec2(14.0, 62.0), egui::vec2(312.0, 14.0));
                painter.rect_filled(bar, 3.0, egui::Color32::from_gray(60));
                let c = bar.center().x;
                let x = c + tr.throttle * bar.width() * 0.5;
                let fill = egui::Rect::from_x_y_ranges(c.min(x)..=c.max(x), bar.y_range());
                painter.rect_filled(fill, 3.0, if tr.throttle >= 0.0 { egui::Color32::from_rgb(80, 200, 90) } else { egui::Color32::from_rgb(230, 120, 60) });
                if !tr.status.is_empty() {
                    painter.text(egui::pos2(area.center().x, area.max.y - 120.0), egui::Align2::CENTER_CENTER, &tr.status, egui::FontId::proportional(20.0), egui::Color32::from_rgb(255, 120, 100));
                }
            }
        }
        _ => {}
    }

    if toast.left > 0.0 {
        toast.left -= time.delta_secs();
        let pos = egui::pos2(area.center().x, area.min.y + 24.0);
        let galley = painter.layout_no_wrap(toast.msg.clone(), egui::FontId::proportional(18.0), egui::Color32::WHITE);
        let rect = egui::Rect::from_center_size(pos, galley.size() + egui::vec2(24.0, 12.0));
        painter.rect_filled(rect, 6.0, egui::Color32::from_rgba_unmultiplied(0, 0, 0, 190));
        painter.galley(rect.min + egui::vec2(12.0, 6.0), galley, egui::Color32::WHITE);
    }

    capture.pointer = ctx.is_pointer_over_area() || ctx.wants_pointer_input();
    capture.keyboard = ctx.wants_keyboard_input();
    Ok(())
}
