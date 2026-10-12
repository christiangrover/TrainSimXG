//! Materials for every [`Mat`], including photo textures embedded in the
//! executable (CC0, from Poly Haven - see assets/textures/CREDITS.md).

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

macro_rules! tex {
    ($name:literal) => {
        (
            include_bytes!(concat!("../assets/textures/", $name, "_diff.jpg")).as_slice(),
            include_bytes!(concat!("../assets/textures/", $name, "_nor.jpg")).as_slice(),
        )
    };
}

fn texture_files(m: Mat) -> Option<(&'static [u8], &'static [u8])> {
    Some(match m {
        Mat::Grass | Mat::Foliage => tex!("leafy_grass"),
        Mat::Earth => tex!("brown_mud_rocks_01"),
        Mat::Rock => tex!("rocky_terrain_02"),
        Mat::Sand => tex!("coast_sand_01"),
        Mat::Ballast => tex!("gravel"),
        Mat::Asphalt => tex!("asphalt_02"),
        Mat::TableWood => tex!("fine_grained_wood"),
        Mat::Floor => tex!("old_wood_floor"),
        Mat::Brick => tex!("red_brick"),
        Mat::Plaster => tex!("painted_plaster_wall"),
        Mat::RoofClay => tex!("clay_roof_tiles"),
        Mat::RoofSlate => tex!("grey_roof_tiles"),
        Mat::Concrete => tex!("concrete"),
        Mat::Planks => tex!("brown_planks_03"),
        Mat::Stone => tex!("old_stone_wall"),
        Mat::Corrugated => tex!("corrugated_iron"),
        _ => return None,
    })
}

/// Decodes a JPEG into an RGBA texture with a full mip chain (Bevy doesn't
/// generate mipmaps itself, and without them distant tiling textures shimmer).
fn load_texture(bytes: &[u8], srgb: bool, green: bool) -> Image {
    let mut rgba = image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg).expect("embedded texture is a valid JPEG").to_rgba8();
    if green {
        // The photo is an autumn lawn full of brown leaves: keep its detail
        // (brightness) but map it onto fresh greens.
        for px in rgba.pixels_mut() {
            let [r, g, b, _] = px.0.map(|v| v as f32);
            let l = ((0.3 * r + 0.59 * g + 0.11 * b - 50.0) / 140.0).clamp(0.0, 1.0);
            let (dark, light) = ([38.0, 78.0, 28.0], [150.0, 196.0, 86.0]);
            for c in 0..3 {
                let mapped = dark[c] + (light[c] - dark[c]) * l;
                px.0[c] = (mapped * 0.88 + [r, g, b][c] * 0.12).min(255.0) as u8;
            }
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
        if let Some((diff, nor)) = texture_files(m) {
            let green = matches!(m, Mat::Grass | Mat::Foliage);
            sm.base_color_texture = Some(images.add(load_texture(diff, true, green)));
            sm.normal_map_texture = Some(images.add(load_texture(nor, false, false)));
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
