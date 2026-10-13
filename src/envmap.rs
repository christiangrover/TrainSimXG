//! Image-based lighting from a real room photo (an HDRI from Poly Haven, CC0).
//! Bevy 0.16 needs pre-filtered cubemaps, so they're built here at startup:
//! a blurred "diffuse" cube for soft fill light, and a mipmapped "specular"
//! cube whose blurrier mips give rougher surfaces softer reflections.

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat, TextureViewDescriptor, TextureViewDimension};
use std::f32::consts::{PI, TAU};

const HDRI: &[u8] = include_bytes!("../assets/textures/lebombo_1k.hdr");
const SPECULAR_SIZE: u32 = 128;
const DIFFUSE_SIZE: u32 = 16;

struct Equirect {
    w: usize,
    h: usize,
    px: Vec<Vec3>,
}

impl Equirect {
    fn load() -> Self {
        let img = image::load_from_memory_with_format(HDRI, image::ImageFormat::Hdr).expect("embedded HDRI is valid").to_rgb32f();
        let (w, h) = (img.width() as usize, img.height() as usize);
        let px = img.pixels().map(|p| Vec3::new(p.0[0], p.0[1], p.0[2])).collect();
        Equirect { w, h, px }
    }

    fn at(&self, x: usize, y: usize) -> Vec3 {
        self.px[y.min(self.h - 1) * self.w + x % self.w]
    }

    /// Bilinear sample in a direction (+Y up).
    fn sample(&self, d: Vec3) -> Vec3 {
        let u = 0.5 + d.z.atan2(d.x) / TAU;
        let v = d.y.clamp(-1.0, 1.0).acos() / PI;
        let fx = u * self.w as f32 - 0.5;
        let fy = v * self.h as f32 - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor().max(0.0));
        let (tx, ty) = (fx - x0, (fy - y0).clamp(0.0, 1.0));
        let x0 = x0.rem_euclid(self.w as f32) as usize;
        let y0 = y0 as usize;
        let a = self.at(x0, y0).lerp(self.at(x0 + 1, y0), tx);
        let b = self.at(x0, y0 + 1).lerp(self.at(x0 + 1, y0 + 1), tx);
        a.lerp(b, ty)
    }

    /// A smaller copy, for the diffuse integral.
    fn shrink(&self, w: usize, h: usize) -> Equirect {
        let mut px = vec![Vec3::ZERO; w * h];
        let (sx, sy) = (self.w / w, self.h / h);
        for y in 0..h {
            for x in 0..w {
                let mut sum = Vec3::ZERO;
                for j in 0..sy {
                    for i in 0..sx {
                        sum += self.at(x * sx + i, y * sy + j);
                    }
                }
                px[y * w + x] = sum / (sx * sy) as f32;
            }
        }
        Equirect { w, h, px }
    }
}

/// Direction through texel (x, y) of cube face `f` (+X, -X, +Y, -Y, +Z, -Z).
fn face_dir(f: usize, x: u32, y: u32, size: u32) -> Vec3 {
    let u = 2.0 * (x as f32 + 0.5) / size as f32 - 1.0;
    let v = 2.0 * (y as f32 + 0.5) / size as f32 - 1.0;
    match f {
        0 => Vec3::new(1.0, -v, -u),
        1 => Vec3::new(-1.0, -v, u),
        2 => Vec3::new(u, 1.0, v),
        3 => Vec3::new(u, -1.0, -v),
        4 => Vec3::new(u, -v, 1.0),
        _ => Vec3::new(-u, -v, -1.0),
    }
    .normalize()
}

fn f16(v: f32) -> u16 {
    // Positive, finite values only (radiance).
    let v = v.clamp(0.0, 65000.0);
    if v < 6.1e-5 {
        return (v / 5.96e-8) as u16;
    }
    let bits = v.to_bits();
    let exp = ((bits >> 23) & 0xff) as i32 - 127 + 15;
    let mant = (bits >> 13) & 0x3ff;
    ((exp as u32) << 10 | mant) as u16
}

fn push(data: &mut Vec<u8>, c: Vec3) {
    for v in [c.x, c.y, c.z, 1.0] {
        data.extend_from_slice(&f16(v).to_le_bytes());
    }
}

fn cube_image(data: Vec<u8>, size: u32, mips: u32) -> Image {
    let ext = Extent3d { width: size, height: size, depth_or_array_layers: 6 };
    let first = data[..(size * size * 8 * 6) as usize].to_vec();
    let mut img = Image::new(ext, TextureDimension::D2, first, TextureFormat::Rgba16Float, RenderAssetUsages::RENDER_WORLD);
    img.data = Some(data);
    img.texture_descriptor.mip_level_count = mips;
    img.texture_view_descriptor = Some(TextureViewDescriptor { dimension: Some(TextureViewDimension::Cube), ..default() });
    img
}

/// Builds the (diffuse, specular) cubemaps for an [`EnvironmentMapLight`].
pub fn build(images: &mut Assets<Image>) -> (Handle<Image>, Handle<Image>) {
    let eq = Equirect::load();

    // Specular: each face sampled sharply, then box-filtered down a full mip chain.
    // Data order is layer-major: all of face 0's mips, then face 1's, and so on.
    let mips = SPECULAR_SIZE.ilog2() + 1;
    let mut spec = Vec::new();
    for f in 0..6 {
        let mut level: Vec<Vec3> = (0..SPECULAR_SIZE * SPECULAR_SIZE).map(|i| eq.sample(face_dir(f, i % SPECULAR_SIZE, i / SPECULAR_SIZE, SPECULAR_SIZE))).collect();
        let mut size = SPECULAR_SIZE;
        loop {
            level.iter().for_each(|c| push(&mut spec, *c));
            if size == 1 {
                break;
            }
            let n = size / 2;
            level = (0..n * n)
                .map(|i| {
                    let (x, y) = ((i % n) * 2, (i / n) * 2);
                    (level[(y * size + x) as usize] + level[(y * size + x + 1) as usize] + level[((y + 1) * size + x) as usize] + level[((y + 1) * size + x + 1) as usize]) * 0.25
                })
                .collect();
            size = n;
        }
    }

    // Diffuse: the cosine-weighted average of the whole room seen from each direction.
    let small = eq.shrink(64, 32);
    // (direction, radiance, solid-angle weight)
    let samples: Vec<(Vec3, Vec3, f32)> = (0..small.h)
        .flat_map(|y| (0..small.w).map(move |x| (x, y)))
        .map(|(x, y)| {
            let theta = (y as f32 + 0.5) / small.h as f32 * PI;
            let phi = ((x as f32 + 0.5) / small.w as f32 - 0.5) * TAU;
            let d = Vec3::new(theta.sin() * phi.cos(), theta.cos(), theta.sin() * phi.sin());
            (d, small.at(x, y), theta.sin())
        })
        .collect();
    let mut diff = Vec::new();
    for f in 0..6 {
        for i in 0..DIFFUSE_SIZE * DIFFUSE_SIZE {
            let n = face_dir(f, i % DIFFUSE_SIZE, i / DIFFUSE_SIZE, DIFFUSE_SIZE);
            let (mut sum, mut wsum) = (Vec3::ZERO, 0.0);
            for (d, c, s) in &samples {
                let w = n.dot(*d).max(0.0) * s;
                sum += *c * w;
                wsum += w;
            }
            push(&mut diff, sum / wsum.max(1e-6));
        }
    }
    (images.add(cube_image(diff, DIFFUSE_SIZE, 1)), images.add(cube_image(spec, SPECULAR_SIZE, mips)))
}
