//! A tiny procedural mesh builder. Every model in the game (track, houses,
//! trains, terrain) is assembled from flat-shaded, vertex-colored primitives.
//! Each primitive is drawn in the builder's current material ([`Mat`]); texture
//! coordinates are box-projected so tiling textures line up across faces.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use std::f32::consts::TAU;

/// Surface materials. Textured ones tile every `uv_scale` world units and are
/// tinted by the vertex color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Mat {
    /// Untextured matte paint.
    Plain,
    /// Untextured glossy paint (vehicles).
    Paint,
    Steel,
    Glass,
    /// Warm glowing light (lamps, headlights).
    Light,
    /// Emergency light bars; these flash.
    FlashRed,
    FlashBlue,
    /// A steam engine's fire, seen through the firebox door.
    Fire,
    Water,
    Grass,
    Foliage,
    Earth,
    Rock,
    Sand,
    Ballast,
    Asphalt,
    TableWood,
    Floor,
    Brick,
    Plaster,
    RoofClay,
    RoofSlate,
    Concrete,
    Planks,
    Stone,
    Corrugated,
}

impl Mat {
    pub const ALL: [Mat; 26] = [
        Mat::Fire,
        Mat::Plain,
        Mat::Paint,
        Mat::Steel,
        Mat::Glass,
        Mat::Light,
        Mat::FlashRed,
        Mat::FlashBlue,
        Mat::Water,
        Mat::Grass,
        Mat::Foliage,
        Mat::Earth,
        Mat::Rock,
        Mat::Sand,
        Mat::Ballast,
        Mat::Asphalt,
        Mat::TableWood,
        Mat::Floor,
        Mat::Brick,
        Mat::Plaster,
        Mat::RoofClay,
        Mat::RoofSlate,
        Mat::Concrete,
        Mat::Planks,
        Mat::Stone,
        Mat::Corrugated,
    ];

    /// World units covered by one repeat of the texture.
    pub fn uv_scale(self) -> f32 {
        match self {
            Mat::Grass => 2.5,
            Mat::Foliage => 0.6,
            Mat::Earth => 1.5,
            Mat::Rock => 3.0,
            Mat::Sand => 3.0,
            Mat::Ballast => 0.6,
            Mat::Asphalt => 1.2,
            Mat::TableWood => 12.0,
            Mat::Floor => 8.0,
            Mat::Brick => 0.35,
            Mat::Plaster => 0.6,
            Mat::RoofClay => 0.35,
            Mat::RoofSlate => 0.4,
            Mat::Concrete => 1.0,
            Mat::Planks => 0.5,
            Mat::Stone => 0.5,
            Mat::Corrugated => 0.35,
            _ => 1.0,
        }
    }
}

#[derive(Default)]
struct Part {
    pos: Vec<[f32; 3]>,
    nrm: Vec<[f32; 3]>,
    uv: Vec<[f32; 2]>,
    col: Vec<[f32; 4]>,
    idx: Vec<u32>,
}

pub struct Geo {
    parts: Vec<(Mat, Part)>,
    mat: Mat,
}

impl Default for Geo {
    fn default() -> Self {
        Geo { parts: vec![], mat: Mat::Plain }
    }
}

/// sRGB hex-ish helper: `rgb(0.8, 0.2, 0.1)`.
pub fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color::srgb(r, g, b)
}

fn lin(c: Color) -> [f32; 4] {
    c.to_linear().to_f32_array()
}

/// Rotation whose local +Z points along `dir` (yaw then pitch, never roll).
pub fn basis_from_dir(dir: Vec3) -> Quat {
    let d = dir.normalize_or(Vec3::Z);
    let yaw = d.x.atan2(d.z);
    let pitch = d.y.clamp(-1.0, 1.0).asin();
    Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-pitch)
}

/// Box-projected texture coordinate for a point on a face with normal `n`.
fn box_uv(p: Vec3, n: Vec3, scale: f32) -> [f32; 2] {
    let a = n.abs();
    let uv = if a.y >= a.x && a.y >= a.z {
        Vec2::new(p.x, p.z)
    } else if a.x >= a.z {
        Vec2::new(p.z * n.x.signum(), -p.y)
    } else {
        Vec2::new(-p.x * n.z.signum(), -p.y)
    };
    (uv / scale).to_array()
}

impl Geo {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the material used by the following primitives; returns the old one.
    pub fn mat(&mut self, m: Mat) -> Mat {
        std::mem::replace(&mut self.mat, m)
    }

    pub fn is_empty(&self) -> bool {
        self.parts.iter().all(|(_, p)| p.idx.is_empty())
    }

    fn part(&mut self, m: Mat) -> &mut Part {
        let i = match self.parts.iter().position(|(pm, _)| *pm == m) {
            Some(i) => i,
            None => {
                self.parts.push((m, Part::default()));
                self.parts.len() - 1
            }
        };
        &mut self.parts[i].1
    }

    /// Adds a flat polygon (convex, counter-clockwise when seen from the front).
    pub fn poly(&mut self, pts: &[Vec3], color: Color) {
        if pts.len() < 3 {
            return;
        }
        let n = (pts[1] - pts[0]).cross(pts[2] - pts[0]).normalize_or_zero();
        let scale = self.mat.uv_scale();
        let part = self.part(self.mat);
        let base = part.pos.len() as u32;
        let c = lin(color);
        for p in pts {
            part.pos.push(p.to_array());
            part.nrm.push(n.to_array());
            part.uv.push(box_uv(*p, n, scale));
            part.col.push(c);
        }
        for i in 1..(pts.len() as u32 - 1) {
            part.idx.extend_from_slice(&[base, base + i, base + i + 1]);
        }
    }
    pub fn quad(&mut self, a: Vec3, b: Vec3, c: Vec3, d: Vec3, color: Color) {
        self.poly(&[a, b, c, d], color);
    }

    /// A one-sided rectangle facing `normal`: visible from outside, invisible
    /// from behind (used for windows you look out of from inside a cab).
    /// `u` is the width direction; the height direction is `normal x u`.
    pub fn panel(&mut self, center: Vec3, normal: Vec3, u: Vec3, hu: f32, hv: f32, color: Color) {
        let n = normal.normalize();
        let u = (u - n * u.dot(n)).normalize() * hu;
        let v = n.cross(u.normalize()) * hv;
        self.quad(center - u - v, center + u - v, center + u + v, center - u + v, color);
    }

    /// Oriented box centered at `center` with half extents `h`, rotated by `rot`.
    pub fn cuboid(&mut self, center: Vec3, h: Vec3, rot: Quat, color: Color) {
        let p = |x: f32, y: f32, z: f32| center + rot * Vec3::new(x * h.x, y * h.y, z * h.z);
        let (a, b, c, d) = (p(-1., -1., 1.), p(1., -1., 1.), p(1., 1., 1.), p(-1., 1., 1.));
        let (e, f, g, hh) = (p(-1., -1., -1.), p(1., -1., -1.), p(1., 1., -1.), p(-1., 1., -1.));
        self.quad(a, b, c, d, color); // +z
        self.quad(f, e, hh, g, color); // -z
        self.quad(b, f, g, c, color); // +x
        self.quad(e, a, d, hh, color); // -x
        self.quad(d, c, g, hh, color); // +y
        self.quad(e, f, b, a, color); // -y
    }

    /// Axis-aligned box from min/max corners.
    pub fn aabb(&mut self, min: Vec3, max: Vec3, color: Color) {
        self.cuboid((min + max) * 0.5, (max - min) * 0.5, Quat::IDENTITY, color);
    }

    /// Cylinder whose axis is local +Y, centered at `center`, then rotated.
    pub fn cylinder(&mut self, center: Vec3, radius: f32, half_h: f32, seg: u32, rot: Quat, color: Color) {
        self.frustum(center, radius, radius, half_h, seg, rot, color);
    }

    /// Cone/truncated cone along local +Y (bottom radius r0, top radius r1).
    pub fn frustum(&mut self, center: Vec3, r0: f32, r1: f32, half_h: f32, seg: u32, rot: Quat, color: Color) {
        let ring = |r: f32, y: f32| -> Vec<Vec3> {
            (0..seg)
                .map(|i| {
                    let a = i as f32 / seg as f32 * TAU;
                    center + rot * Vec3::new(a.cos() * r, y, -a.sin() * r)
                })
                .collect()
        };
        let bot = ring(r0, -half_h);
        let top = ring(r1, half_h);
        for i in 0..seg as usize {
            let j = (i + 1) % seg as usize;
            if r1 > 0.0001 {
                self.quad(bot[i], bot[j], top[j], top[i], color);
            } else {
                self.poly(&[bot[i], bot[j], top[i]], color);
            }
        }
        if r1 > 0.0001 {
            self.poly(&top, color);
        }
        let mut b = bot.clone();
        b.reverse();
        self.poly(&b, color);
    }

    pub fn cone(&mut self, base_center: Vec3, radius: f32, height: f32, seg: u32, color: Color) {
        self.frustum(base_center + Vec3::Y * height * 0.5, radius, 0.0, height * 0.5, seg, Quat::IDENTITY, color);
    }

    /// Low-poly UV sphere.
    pub fn sphere(&mut self, center: Vec3, r: f32, color: Color) {
        let (lat, lon) = (5u32, 8u32);
        let pt = |i: u32, j: u32| {
            let th = std::f32::consts::PI * i as f32 / lat as f32;
            let ph = TAU * j as f32 / lon as f32;
            center + Vec3::new(th.sin() * ph.cos(), th.cos(), -th.sin() * ph.sin()) * r
        };
        for i in 0..lat {
            for j in 0..lon {
                let (a, b, c, d) = (pt(i, j), pt(i + 1, j), pt(i + 1, j + 1), pt(i, j + 1));
                if i == 0 {
                    self.poly(&[a, b, c], color);
                } else if i == lat - 1 {
                    self.poly(&[a, b, d], color);
                } else {
                    self.quad(a, b, c, d, color);
                }
            }
        }
    }

    /// Triangular prism roof: ridge along local X, spanning `w` (x) by `d` (z),
    /// base at `base_y`, peak height `h`, rotated about Y by `rot`.
    /// The slopes use the current material; the gable ends use gable_mat.
    #[allow(clippy::too_many_arguments)]
    pub fn roof(&mut self, center: Vec3, w: f32, d: f32, h: f32, rot: Quat, color: Color, gable: Color, gable_mat: Mat) {
        let p = |x: f32, y: f32, z: f32| center + rot * Vec3::new(x, y, z);
        let (hw, hd) = (w * 0.5, d * 0.5);
        let a = p(-hw, 0.0, hd);
        let b = p(hw, 0.0, hd);
        let c = p(hw, h, 0.0);
        let dd = p(-hw, h, 0.0);
        let e = p(-hw, 0.0, -hd);
        let f = p(hw, 0.0, -hd);
        self.quad(a, b, c, dd, color);
        self.quad(f, e, dd, c, color);
        let old = self.mat(gable_mat);
        self.poly(&[b, f, c], gable);
        self.poly(&[e, a, dd], gable);
        self.quad(e, f, b, a, gable);
        self.mat(old);
    }

    /// Appends another builder's geometry, transformed (texture coordinates are kept).
    pub fn append(&mut self, other: Geo, tf: Transform) {
        for (m, op) in other.parts {
            let part = self.part(m);
            let base = part.pos.len() as u32;
            part.pos.extend(op.pos.into_iter().map(|p| tf.transform_point(Vec3::from(p)).to_array()));
            part.nrm.extend(op.nrm.into_iter().map(|n| (tf.rotation * Vec3::from(n)).to_array()));
            part.uv.extend(op.uv);
            part.col.extend(op.col);
            part.idx.extend(op.idx.into_iter().map(|i| i + base));
        }
    }

    /// One mesh per material.
    pub fn build(self) -> Vec<(Mat, Mesh)> {
        self.parts
            .into_iter()
            .filter(|(_, p)| !p.idx.is_empty())
            .map(|(m, p)| {
                let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default())
                    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, p.pos)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, p.nrm)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, p.uv)
                    .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, p.col)
                    .with_inserted_indices(Indices::U32(p.idx));
                // Normal-mapped materials need tangents.
                let _ = mesh.generate_tangents();
                (m, mesh)
            })
            .collect()
    }
}