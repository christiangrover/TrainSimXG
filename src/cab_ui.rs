//! On-screen controls for the engineer's cab, the fireman, and the crane
//! driver: big, clearly labelled levers, buttons and gauges.

use crate::cab::{CabRequests, Fireman};
use crate::crane::{CraneInput, Cranes, Hint, Tool};
use crate::trains::*;
use bevy_egui::egui;
use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use std::f32::consts::PI;

const PANEL: Color32 = Color32::from_rgb(38, 34, 30);
const BRASS: Color32 = Color32::from_rgb(200, 160, 70);
const CREAM: Color32 = Color32::from_rgb(245, 235, 210);
const GREEN: Color32 = Color32::from_rgb(70, 190, 90);
const RED: Color32 = Color32::from_rgb(220, 50, 40);
const AMBER: Color32 = Color32::from_rgb(240, 170, 40);

/// A round dial gauge. `zone` is the green "good" range; `max_label` shows the top value.
pub fn dial(ui: &mut egui::Ui, value: f32, max: f32, label: &str, unit: &str, zone: Option<(f32, f32)>, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
    let p = ui.painter_at(rect.expand(2.0));
    let c = rect.center();
    let r = size * 0.46;
    p.circle_filled(c, r + 3.0, BRASS);
    p.circle_filled(c, r, CREAM);
    // Sweep from 225 degrees (min) to -45 degrees (max), clockwise.
    let ang = |f: f32| PI * 1.25 - f.clamp(0.0, 1.0) * PI * 1.5;
    let at = |f: f32, rr: f32| c + Vec2::new(ang(f).cos(), -ang(f).sin()) * rr;
    if let Some((a, b)) = zone {
        let steps = 20;
        let pts: Vec<Pos2> = (0..=steps).map(|i| at(a + (b - a) * i as f32 / steps as f32, r * 0.82)).collect();
        p.add(egui::Shape::line(pts, Stroke::new(r * 0.12, GREEN)));
        let red: Vec<Pos2> = (0..=6).map(|i| at(b + (1.0 - b) * i as f32 / 6.0, r * 0.82)).collect();
        if b < 0.99 {
            p.add(egui::Shape::line(red, Stroke::new(r * 0.12, RED)));
        }
    }
    for i in 0..=10 {
        let f = i as f32 / 10.0;
        let len = if i % 5 == 0 { 0.22 } else { 0.12 };
        p.line_segment([at(f, r * 0.95), at(f, r * (0.95 - len))], Stroke::new(1.5f32, Color32::from_gray(40)));
    }
    p.text(c + Vec2::new(0.0, r * 0.38), Align2::CENTER_CENTER, label, FontId::proportional(size * 0.11), Color32::from_gray(40));
    let f = value / max;
    p.text(c + Vec2::new(0.0, r * 0.62), Align2::CENTER_CENTER, format!("{:.0} {unit}", value), FontId::monospace(size * 0.1), Color32::from_gray(30));
    p.line_segment([c, at(f, r * 0.8)], Stroke::new(3.0f32, RED));
    p.circle_filled(c, r * 0.08, Color32::from_gray(30));
}

/// A labelled level bar (0..1).
pub fn level_bar(ui: &mut egui::Ui, label: &str, v: f32, color: Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{label:>8}")).color(CREAM).monospace());
        let (rect, _) = ui.allocate_exact_size(Vec2::new(110.0, 16.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, 3.0, Color32::from_gray(25));
        let mut fill = rect;
        fill.set_width(rect.width() * v.clamp(0.0, 1.0));
        let c = if v < 0.15 { RED } else { color };
        p.rect_filled(fill, 3.0, c);
        p.text(rect.center(), Align2::CENTER_CENTER, format!("{:.0}%", v.clamp(0.0, 1.2) * 100.0), FontId::monospace(11.0), Color32::WHITE);
    });
}

fn big_button(ui: &mut egui::Ui, text: &str, fill: Color32, size: Vec2) -> egui::Response {
    ui.add(egui::Button::new(egui::RichText::new(text).size(16.0).strong().color(Color32::WHITE)).fill(fill).min_size(size).corner_radius(8.0))
}

/// Auto/Manual fuel switch.
fn fuel_switch(ui: &mut egui::Ui, t: &mut Train) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Fuel:").color(CREAM));
        ui.selectable_value(&mut t.fuel_manual, false, "Auto");
        ui.selectable_value(&mut t.fuel_manual, true, "Manual");
    })
    .response
    .on_hover_text("Manual: fuel, coal and water get used up. Stop at a station to fill up again.");
}

fn fuel_gauges(ui: &mut egui::Ui, t: &Train) {
    match t.power_kind() {
        Power::Steam => {
            level_bar(ui, "Coal", t.fuel, Color32::from_gray(150));
            level_bar(ui, "Water", t.water, Color32::from_rgb(70, 140, 230));
        }
        Power::Diesel => level_bar(ui, "Fuel", t.fuel, AMBER),
        Power::None => {}
    }
}

/// The cab's window frame around the view.
pub fn cab_frame(painter: &egui::Painter, area: Rect, diesel: bool) {
    let metal = if diesel { Color32::from_rgb(45, 50, 60) } else { Color32::from_rgb(30, 45, 35) };
    let w = area.width() * 0.07;
    painter.rect_filled(Rect::from_min_max(area.min, Pos2::new(area.min.x + w, area.max.y)), 0.0, metal);
    painter.rect_filled(Rect::from_min_max(Pos2::new(area.max.x - w, area.min.y), area.max), 0.0, metal);
    painter.rect_filled(Rect::from_min_max(area.min, Pos2::new(area.max.x, area.min.y + w * 0.8)), 0.0, metal);
    if diesel {
        let c = area.center().x;
        painter.rect_filled(Rect::from_min_max(Pos2::new(c - w * 0.18, area.min.y), Pos2::new(c + w * 0.18, area.max.y)), 0.0, metal);
    }
    // Rivets along the frame.
    for i in 0..12 {
        let y = area.min.y + w + i as f32 * (area.height() - w) / 12.0;
        for x in [area.min.x + w * 0.5, area.max.x - w * 0.5] {
            painter.circle_filled(Pos2::new(x, y), 3.0, Color32::from_gray(90));
        }
    }
}

/// The engineer's control desk.
pub fn engineer_panel(ctx: &egui::Context, area: Rect, t: &mut Train, req: &mut CabRequests) {
    let steam = t.power_kind() == Power::Steam;
    egui::Area::new(egui::Id::new("cab_desk")).order(egui::Order::Foreground).pivot(Align2::CENTER_BOTTOM).fixed_pos(Pos2::new(area.center().x, area.max.y - 6.0)).show(ctx, |ui| {
        egui::Frame::new().fill(PANEL).stroke(Stroke::new(3.0f32, BRASS)).corner_radius(12.0).inner_margin(10.0).show(ui, |ui| {
            if !t.status.is_empty() {
                ui.vertical_centered(|ui| ui.label(egui::RichText::new(&t.status).size(17.0).color(AMBER).strong()));
            }
            ui.horizontal(|ui| {
                // Speedometer (miles per hour at model scale, for fun).
                dial(ui, t.speed.abs() * 20.0, MAX_SPEED * 20.0, "SPEED", "mph", None, 130.0);
                ui.separator();
                // Throttle / reverser lever.
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("THROTTLE").color(CREAM).strong());
                    ui.horizontal(|ui| {
                        ui.spacing_mut().slider_width = 110.0;
                        ui.add_enabled(!t.ebrake, egui::Slider::new(&mut t.throttle, -1.0..=1.0).vertical().step_by(0.25).show_value(false));
                        ui.vertical(|ui| {
                            ui.label(egui::RichText::new("FORWARD").color(GREEN).small());
                            ui.add_space(30.0);
                            ui.label(egui::RichText::new("STOP").color(CREAM).small());
                            ui.add_space(30.0);
                            ui.label(egui::RichText::new("REVERSE").color(AMBER).small());
                        });
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("BRAKE").color(CREAM).strong());
                    ui.spacing_mut().slider_width = 110.0;
                    ui.add(egui::Slider::new(&mut t.brake_lever, 0.0..=1.0).vertical().show_value(false));
                });
                ui.separator();
                ui.vertical(|ui| {
                    let (text, fill) = if t.ebrake { ("RELEASE\nBRAKE", AMBER) } else { ("EMERGENCY\nSTOP", RED) };
                    if big_button(ui, text, fill, Vec2::new(120.0, 58.0)).clicked() {
                        req.ebrake = true;
                    }
                    ui.add_space(4.0);
                    if big_button(ui, if steam { "WHISTLE" } else { "HORN" }, Color32::from_rgb(60, 90, 160), Vec2::new(120.0, 48.0)).clicked() {
                        req.horn = true;
                    }
                });
                ui.separator();
                ui.vertical(|ui| {
                    if steam {
                        dial(ui, t.pressure.min(1.2) * 200.0, 240.0, "STEAM", "psi", Some((0.5, 0.85)), 90.0);
                    } else {
                        dial(ui, t.fuel * 100.0, 100.0, "FUEL", "%", Some((0.25, 1.0)), 90.0);
                    }
                    fuel_switch(ui, t);
                    fuel_gauges(ui, t);
                });
            });
        });
    });
}

/// The fireman's gauges and the big shovel button.
pub fn fireman_panel(ctx: &egui::Context, area: Rect, t: &mut Train, req: &mut CabRequests, fireman: &Fireman) {
    egui::Area::new(egui::Id::new("fireman")).order(egui::Order::Foreground).pivot(Align2::RIGHT_BOTTOM).fixed_pos(Pos2::new(area.max.x - 8.0, area.max.y - 8.0)).show(ctx, |ui| {
        egui::Frame::new().fill(PANEL).stroke(Stroke::new(3.0f32, BRASS)).corner_radius(12.0).inner_margin(10.0).show(ui, |ui| {
            ui.set_width(260.0);
            ui.label(egui::RichText::new("You're the fireman!").size(18.0).color(AMBER).strong());
            ui.horizontal(|ui| {
                dial(ui, t.pressure.min(1.2) * 200.0, 240.0, "STEAM", "psi", Some((0.5, 0.85)), 110.0);
                ui.vertical(|ui| {
                    level_bar(ui, "Fire", t.fire.min(1.0), Color32::from_rgb(240, 110, 30));
                    fuel_gauges(ui, t);
                });
            });
            if big_button(ui, "SHOVEL COAL  (Space)", Color32::from_rgb(150, 70, 20), Vec2::new(250.0, 54.0)).clicked() {
                req.shovel = true;
            }
            fuel_switch(ui, t);
            let tip = if !t.fuel_manual {
                "Fuel is on Auto, so the fire never goes out. Switch to Manual and the engine needs you!".to_string()
            } else if !fireman.message.is_empty() {
                fireman.message.clone()
            } else if t.pressure < 0.5 {
                "Pressure is low - shovel more coal to keep the needle in the green!".into()
            } else if t.fire > 1.0 {
                "Whoa, that's plenty! Wait for the fire to burn down.".into()
            } else {
                "Keep the steam needle in the green. More coal = hotter fire = more steam.".into()
            };
            ui.label(egui::RichText::new(tip).color(CREAM));
        });
    });
}

fn hold(ui: &mut egui::Ui, text: &str) -> bool {
    let r = ui.add(egui::Button::new(egui::RichText::new(text).size(16.0).strong()).min_size(Vec2::new(78.0, 40.0)).corner_radius(6.0));
    r.is_pointer_button_down_on()
}

/// The crane driver's controls.
pub fn crane_panel(ctx: &egui::Context, area: Rect, cranes: &mut Cranes, input: &mut CraneInput) {
    let Some(c) = cranes.active.and_then(|t| cranes.map.get(&t)) else { return };
    let (tool, carrying, message, hint) = (c.tool, c.carrying, c.message.clone(), c.hint);
    egui::Area::new(egui::Id::new("crane")).order(egui::Order::Foreground).pivot(Align2::CENTER_BOTTOM).fixed_pos(Pos2::new(area.center().x, area.max.y - 6.0)).show(ctx, |ui| {
        egui::Frame::new().fill(PANEL).stroke(Stroke::new(3.0f32, Color32::from_rgb(240, 180, 20))).corner_radius(12.0).inner_margin(10.0).show(ui, |ui| {
            let what = match carrying {
                Some(k) => format!("Carrying {}", k.label()),
                None => format!("{} - empty", if tool == Tool::Hook { "Hook (for logs)" } else { "Grab bucket (for rocks)" }),
            };
            ui.vertical_centered(|ui| {
                ui.label(egui::RichText::new(what).size(17.0).color(AMBER).strong());
                // What the crane can do right now, in big friendly words.
                let (say, col) = match hint {
                    Hint::CanGrab => ("Ready! Press GRAB", GREEN),
                    Hint::LowerToGrab => ("Lower the hook onto the pile (F)", AMBER),
                    Hint::WrongTool => ("Wrong tool for this pile - press Tab", RED),
                    Hint::CanDrop(..) => ("Ready! Press DROP to load the car", GREEN),
                    Hint::LowerToDrop(..) => ("Lower it onto the car (F)", AMBER),
                    Hint::WrongCar(..) => ("That car can't carry this", RED),
                    Hint::CarFull(..) => ("That car is full", RED),
                    Hint::OverPile => ("Over the pile - DROP puts it back", AMBER),
                    Hint::Nothing if carrying.is_some() => ("Swing over a car with a white outline", CREAM),
                    Hint::Nothing => ("Swing over the pile with the white ring", CREAM),
                };
                ui.label(egui::RichText::new(say).size(16.0).color(col).strong());
                if !message.is_empty() {
                    ui.label(egui::RichText::new(message).color(CREAM));
                }
            });
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("SWING").color(CREAM).strong());
                    ui.horizontal(|ui| {
                        if hold(ui, "◀ A") {
                            input.turn += 1.0;
                        }
                        if hold(ui, "D ▶") {
                            input.turn -= 1.0;
                        }
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("TROLLEY").color(CREAM).strong());
                    ui.horizontal(|ui| {
                        if hold(ui, "In S") {
                            input.reach -= 1.0;
                        }
                        if hold(ui, "Out W") {
                            input.reach += 1.0;
                        }
                    });
                });
                ui.separator();
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("HOOK").color(CREAM).strong());
                    ui.horizontal(|ui| {
                        if hold(ui, "▲ Up R") {
                            input.hoist += 1.0;
                        }
                        if hold(ui, "▼ Down F") {
                            input.hoist -= 1.0;
                        }
                    });
                });
                ui.separator();
                let ready = matches!(hint, Hint::CanGrab | Hint::CanDrop(..));
                let txt = if carrying.is_some() { "DROP  (Space)" } else { "GRAB  (Space)" };
                // The button lights up when it will work.
                let fill = if ready { Color32::from_rgb(30, 170, 60) } else { Color32::from_rgb(80, 80, 85) };
                if big_button(ui, txt, fill, Vec2::new(140.0, 62.0)).clicked() {
                    input.action = true;
                }
                if big_button(ui, if tool == Tool::Hook { "Use bucket\n(Tab)" } else { "Use hook\n(Tab)" }, Color32::from_rgb(70, 80, 100), Vec2::new(100.0, 62.0)).clicked() {
                    input.switch_tool = true;
                }
            });
            ui.label(egui::RichText::new("Logs go on log cars, rocks go in hoppers. Trains stopped next to the port get loaded automatically when you're not driving. C = next crane").small().color(CREAM));
        });
    });
}
