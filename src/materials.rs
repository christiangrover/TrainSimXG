//! Materials for every [`Mat`], including photo textures embedded in the
//! executable (CC0, from ambientCG and Poly Haven - see assets/textures/CREDITS.md).

use crate::mesh::{Geo, Mat};
use bevy::asset::RenderAssetUsages;
use bevy::image::{ImageAddressMode, ImageFilterMode, ImageSampler, ImageSamplerDescriptor};
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;

/// Signal lamp colours (traffic lights and crossing lights).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lamp {
    Red,
    Yellow,
    Green,
}

#[derive(Resource)]
pub struct Materials {
    map: HashMap<Mat, Handle<StandardMaterial>>,
    lamps: HashMap<(Lamp, bool), Handle<StandardMaterial>>,
}

impl Materials {
    /// A signal lamp, lit or dark.
    pub fn lamp(&self, lamp: Lamp, on: bool) -> Handle<StandardMaterial> {
        self.lamps[&(lamp, on)].clone()
    }

    pub fn get(&self, m: Mat) -> Handle<StandardMaterial> {
        self.map[&m].clone()
    }
}

/// One material's texture maps (JPEG bytes embedded in the executable).
struct TexSet {
    color: &'static [u8],
    normal: &'static [u8],
    roughness: Option<&'static [u8]>,
    occlusion: Option<&'static [u8]>,
}

/// An ambientCG material with colour, normal, roughness and (optionally) occlusion maps.
macro_rules! acg {
    ($id:literal) => {
        TexSet {
            color: include_bytes!(concat!("../assets/textures/acg/", $id, "_Color.jpg")),
            normal: include_bytes!(concat!("../assets/textures/acg/", $id, "_NormalGL.jpg")),
            roughness: Some(include_bytes!(concat!("../assets/textures/acg/", $id, "_Roughness.jpg"))),
            occlusion: Some(include_bytes!(concat!("../assets/textures/acg/", $id, "_AmbientOcclusion.jpg"))),
        }
    };
    ($id:literal, no_ao) => {
        TexSet {
            color: include_bytes!(concat!("../assets/textures/acg/", $id, "_Color.jpg")),
            normal: include_bytes!(concat!("../assets/textures/acg/", $id, "_NormalGL.jpg")),
            roughness: Some(include_bytes!(concat!("../assets/textures/acg/", $id, "_Roughness.jpg"))),
            occlusion: None,
        }
    };
}

/// A Poly Haven material (colour and normal maps).
macro_rules! ph {
    ($name:literal) => {
        TexSet {
            color: include_bytes!(concat!("../assets/textures/", $name, "_diff.jpg")),
            normal: include_bytes!(concat!("../assets/textures/", $name, "_nor.jpg")),
            roughness: None,
            occlusion: None,
        }
    };
}

fn texture_files(m: Mat) -> Option<TexSet> {
    Some(match m {
        Mat::Grass => acg!("Grass001"),
        Mat::Foliage => acg!("Grass001"),
        Mat::Earth => ph!("brown_mud_rocks_01"),
        Mat::Rock => acg!("Rock064"),
        Mat::Sand => acg!("Ground054"),
        Mat::Ballast => acg!("Gravel023"),
        Mat::Asphalt => acg!("Asphalt033"),
        Mat::TableWood => ph!("fine_grained_wood"),
        Mat::Floor => acg!("WoodFloor051"),
        Mat::Brick => acg!("Bricks097"),
        Mat::Plaster => acg!("PaintedPlaster017", no_ao),
        Mat::RoofClay => acg!("RoofingTiles006"),
        Mat::RoofSlate => acg!("RoofingTiles013A"),
        Mat::Concrete => acg!("Concrete034", no_ao),
        Mat::Paving => acg!("PavingStones138"),
        Mat::Planks => acg!("Planks037A"),
        Mat::Stone => acg!("Bricks075A"),
        Mat::Corrugated => acg!("CorrugatedSteel005"),
        Mat::Paint => acg!("PaintedMetal004", no_ao),
        _ => return None,
    })
}

/// How a colour texture is adjusted on load.
#[derive(Clone, Copy, PartialEq)]
enum Tint {
    None,
    /// Grey it out and brighten it, so vertex colours (each train's paint
    /// colour) show through with the texture's wear and chips on top.
    Neutral,
}
/// Decodes a JPEG into an RGBA texture with a full mip chain (Bevy doesn't
/// generate mipmaps itself, and without them distant tiling textures shimmer).
fn load_texture(bytes: &[u8], srgb: bool, tint: Tint) -> Image {
    let mut rgba = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg).expect("embedded texture is a valid JPEG").to_rgba8();
    if tint == Tint::Neutral {
        let lum = |p: &image::Rgba<u8>| 0.3 * p.0[0] as f32 + 0.59 * p.0[1] as f32 + 0.11 * p.0[2] as f32;
        let mean = rgba.pixels().map(lum).sum::<f32>() / (rgba.width() * rgba.height()) as f32;
        for px in rgba.pixels_mut() {
            // Keep only a hint of the wear, so the train's own paint colour shows.
            let rel = (lum(px) / mean.max(1.0)).clamp(0.6, 1.4);
            let l = (235.0 * (1.0 + 0.18 * (rel - 1.0))).min(255.0) as u8;
            px.0[0] = l;
            px.0[1] = l;
            px.0[2] = l;
        }
    }
    let (w0, h0) = rgba.dimensions();
    let (mut w, mut h) = (w0, h0);
    let mut level = rgba.into_raw();
    let mut data = level.clone();
    let mut mips = 1;
    while w > 1 || h > 1 {
        let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
        let mut next = vec![0u8; (nw * nh * 4) as usize];
        for y in 0..nh {
            for x in 0..nw {
                for c in 0..4 {
                    let mut sum = 0u32;
                    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                        let sx = (x * 2 + dx).min(w - 1);
                        let sy = (y * 2 + dy).min(h - 1);
                        sum += level[((sy * w + sx) * 4 + c) as usize] as u32;
                    }
                    next[((y * nw + x) * 4 + c) as usize] = (sum / 4) as u8;
                }
            }
        }
        data.extend_from_slice(&next);
        level = next;
        w = nw;
        h = nh;
        mips += 1;
    }
    let format = if srgb { TextureFormat::Rgba8UnormSrgb } else { TextureFormat::Rgba8Unorm };
    let size = Extent3d { width: w0, height: h0, depth_or_array_layers: 1 };
    let first = data[..(w0 * h0 * 4) as usize].to_vec();
    let mut img = Image::new(size, TextureDimension::D2, first, format, RenderAssetUsages::RENDER_WORLD);
    img.data = Some(data);
    img.texture_descriptor.mip_level_count = mips;
    img.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        mag_filter: ImageFilterMode::Linear,
        min_filter: ImageFilterMode::Linear,
        mipmap_filter: ImageFilterMode::Linear,
        anisotropy_clamp: 16,
        ..default()
    });
    img
}

pub fn create(images: &mut Assets<Image>, mats: &mut Assets<StandardMaterial>) -> Materials {
    let mut map = HashMap::new();
    for m in Mat::ALL {
        let mut sm = StandardMaterial { base_color: Color::WHITE, perceptual_roughness: 0.85, ..default() };
        match m {
            Mat::Paint => sm.perceptual_roughness = 0.35,
            Mat::Steel => {
                sm.perceptual_roughness = 0.35;
                sm.metallic = 0.85;
            }
            Mat::Glass => {
                sm.perceptual_roughness = 0.08;
                sm.metallic = 0.3;
                sm.reflectance = 0.8;
            }
            Mat::Window => {
                sm.base_color = Color::srgba(0.55, 0.65, 0.7, 0.18);
                sm.alpha_mode = AlphaMode::Blend;
                sm.perceptual_roughness = 0.1;
                sm.reflectance = 0.25;
            }
            Mat::Light => sm.emissive = LinearRgba::rgb(6.0, 5.0, 3.0),
            Mat::FlashRed => sm.emissive = LinearRgba::rgb(10.0, 0.3, 0.2),
            Mat::FlashBlue => sm.emissive = LinearRgba::rgb(0.3, 0.8, 12.0),
            Mat::Fire => {
                sm.base_color = Color::srgb(0.3, 0.1, 0.0);
                sm.emissive = LinearRgba::rgb(8.0, 3.0, 0.5);
            }
            Mat::Water => {
                sm.perceptual_roughness = 0.06;
                sm.reflectance = 0.6;
            }
            Mat::Corrugated => sm.metallic = 0.5,
            Mat::TableWood | Mat::Floor => sm.perceptual_roughness = 0.55,
            Mat::RoofSlate => sm.perceptual_roughness = 0.6,
            _ => {}
        }
        if let Some(t) = texture_files(m) {
            let tint = if m == Mat::Paint { Tint::Neutral } else { Tint::None };
            sm.base_color_texture = Some(images.add(load_texture(t.color, true, tint)));
            sm.normal_map_texture = Some(images.add(load_texture(t.normal, false, Tint::None)));
            if let Some(r) = t.roughness {
                // Roughness is read from the green channel (and metalness from blue,
                // scaled by `metallic`); a grey map serves for both.
                sm.metallic_roughness_texture = Some(images.add(load_texture(r, false, Tint::None)));
                sm.perceptual_roughness = 1.0;
            }
            if let Some(o) = t.occlusion {
                sm.occlusion_texture = Some(images.add(load_texture(o, false, Tint::None)));
            }
        }
        if m == Mat::Earth || m == Mat::Rock || m == Mat::Grass {
            // Terrain is seen from inside tunnels too.
            sm.double_sided = true;
            sm.cull_mode = None;
        }
        map.insert(m, mats.add(sm));
    }
    let mut lamps = HashMap::new();
    for (lamp, (r, g, b)) in [(Lamp::Red, (1.0, 0.08, 0.05)), (Lamp::Yellow, (1.0, 0.65, 0.05)), (Lamp::Green, (0.1, 1.0, 0.3))] {
        let lit = StandardMaterial { base_color: Color::srgb(r, g, b), emissive: LinearRgba::rgb(r * 12.0, g * 12.0, b * 12.0), ..default() };
        let dark = StandardMaterial { base_color: Color::srgb(r * 0.18, g * 0.18, b * 0.18), perceptual_roughness: 0.3, ..default() };
        lamps.insert((lamp, true), mats.add(lit));
        lamps.insert((lamp, false), mats.add(dark));
    }
    Materials { map, lamps }
}

/// Spawns a multi-material model as a parent entity with one child per material.
pub fn spawn_geo(commands: &mut Commands, meshes: &mut Assets<Mesh>, mats: &Materials, geo: Geo, tf: Transform) -> Entity {
    let parts = meshes_of(meshes, geo);
    spawn_parts(commands, mats, &parts, tf)
}

/// Uploads a model's meshes so they can be spawned many times.
pub fn meshes_of(meshes: &mut Assets<Mesh>, geo: Geo) -> Vec<(Mat, Handle<Mesh>)> {
    geo.build().into_iter().map(|(m, mesh)| (m, meshes.add(mesh))).collect()
}

pub fn spawn_parts(commands: &mut Commands, mats: &Materials, parts: &[(Mat, Handle<Mesh>)], tf: Transform) -> Entity {
    commands
        .spawn((tf, Visibility::default()))
        .with_children(|c| {
            for (m, h) in parts {
                c.spawn((Mesh3d(h.clone()), MeshMaterial3d(mats.get(*m))));
            }
        })
        .id()
}

/// Emergency light bars flash red and blue alternately.
pub fn flash_lights(time: Res<Time>, mats: Option<Res<Materials>>, mut assets: ResMut<Assets<StandardMaterial>>) {
    let Some(mats) = mats else { return };
    let t = time.elapsed_secs() * 6.0;
    let on = t.sin() > 0.0;
    if let Some(m) = assets.get_mut(&mats.get(Mat::FlashRed)) {
        m.emissive = if on { LinearRgba::rgb(12.0, 0.3, 0.2) } else { LinearRgba::rgb(0.3, 0.0, 0.0) };
    }
    if let Some(m) = assets.get_mut(&mats.get(Mat::FlashBlue)) {
        m.emissive = if on { LinearRgba::rgb(0.0, 0.0, 0.3) } else { LinearRgba::rgb(0.3, 0.8, 14.0) };
    }
}
