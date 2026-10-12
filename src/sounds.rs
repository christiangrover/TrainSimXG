//! Sound effects, synthesised at startup (no audio files needed): horns,
//! whistles, chuffing, brakes, shovelling and crane noises.

use bevy::audio::{AudioSink, AudioSinkPlayback, Volume};
use bevy::prelude::*;
use std::f32::consts::TAU;
use std::sync::Arc;

const RATE: u32 = 22050;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sfx {
    Horn,
    Whistle,
    Chuff,
    Hiss,
    Shovel,
    Clank,
    Thud,
    Bell,
}

#[derive(Resource)]
pub struct Sounds {
    clips: Vec<(Sfx, Handle<AudioSource>)>,
    pub rumble: Handle<AudioSource>,
}

/// Sounds requested this frame by any system.
#[derive(Resource, Default)]
pub struct SoundQueue(Vec<(Sfx, f32, f32)>);

impl SoundQueue {
    /// Queue a sound at a volume (0..1).
    pub fn play(&mut self, sfx: Sfx, volume: f32) {
        self.0.push((sfx, volume, 1.0));
    }
}

pub struct SoundsPlugin;

impl Plugin for SoundsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SoundQueue>().add_systems(Startup, make_sounds).add_systems(PostUpdate, play_queued);
    }
}

/// Tiny deterministic noise source.
struct Noise(u32);
impl Noise {
    fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

fn envelope(t: f32, len: f32, attack: f32, release: f32) -> f32 {
    (t / attack).min(1.0) * ((len - t) / release).clamp(0.0, 1.0)
}

fn render(len: f32, mut f: impl FnMut(f32) -> f32) -> Vec<f32> {
    let n = (len * RATE as f32) as usize;
    let mut out: Vec<f32> = (0..n).map(|i| f(i as f32 / RATE as f32)).collect();
    let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs())).max(1e-6);
    for s in &mut out {
        *s = *s / peak * 0.9;
    }
    out
}

/// 16-bit mono PCM WAV.
fn wav(samples: &[f32]) -> AudioSource {
    let data_len = (samples.len() * 2) as u32;
    let mut b = Vec::with_capacity(44 + data_len as usize);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data_len).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        b.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    AudioSource { bytes: Arc::from(b) }
}

/// A buzzy reed tone (like a horn) from a few harmonics.
fn reed(f: f32, t: f32) -> f32 {
    (1..7).map(|h| (TAU * f * h as f32 * t).sin() / h as f32).sum()
}

fn make_sounds(mut commands: Commands, mut audio: ResMut<Assets<AudioSource>>) {
    let mut noise = Noise(0x1234_5678);
    let horn = render(1.4, |t| {
        let vib = 1.0 + 0.004 * (TAU * 5.0 * t).sin();
        let s: f32 = [311.1f32, 370.0, 466.2].iter().map(|f| reed(f * vib, t)).sum();
        s * envelope(t, 1.4, 0.06, 0.2)
    });
    let whistle = render(1.3, |t| {
        let s: f32 = [587.3f32, 698.5, 880.0].iter().map(|f| (TAU * f * t).sin() + 0.2 * (TAU * 2.0 * f * t).sin()).sum();
        (s + noise.next() * 0.6) * envelope(t, 1.3, 0.1, 0.25)
    });
    let mut lp = 0.0;
    let chuff = render(0.22, |t| {
        lp += (noise.next() - lp) * 0.25;
        lp * (-t * 18.0).exp()
    });
    let mut prev = 0.0;
    let hiss = render(1.3, |t| {
        let n = noise.next();
        let hp = n - prev;
        prev = n;
        hp * (-t * 2.2).exp() * (t / 0.02).min(1.0)
    });
    let mut lp2 = 0.0;
    let shovel = render(0.45, |t| {
        lp2 += (noise.next() - lp2) * 0.4;
        let scrape = if t < 0.25 { lp2 * (1.0 + (TAU * 30.0 * t).sin()) * 0.5 } else { 0.0 };
        let thump = if t > 0.28 { (TAU * 90.0 * (t - 0.28)).sin() * (-(t - 0.28) * 25.0).exp() * 1.5 } else { 0.0 };
        scrape + thump
    });
    let clank = render(0.5, |t| {
        let s: f32 = [523.0f32, 1187.0, 2011.0, 2900.0].iter().enumerate().map(|(i, f)| (TAU * f * t).sin() / (i + 1) as f32).sum();
        s * (-t * 9.0).exp()
    });
    let thud = render(0.4, |t| ((TAU * 70.0 * t).sin() + noise.next() * 0.3) * (-t * 12.0).exp());
    let bell = render(0.5, |t| {
        let s: f32 = [1250.0f32, 1890.0, 3150.0].iter().enumerate().map(|(i, f)| (TAU * f * t).sin() / (i + 1) as f32).sum();
        s * (-t * 7.0).exp() * (t / 0.003).min(1.0)
    });
    // Exactly 55 cycles of 55 Hz so it loops seamlessly.
    let mut lp3 = 0.0;
    let rumble = render(1.0, |t| {
        lp3 += (noise.next() - lp3) * 0.05;
        reed(55.0, t) * 0.6 + reed(110.0, t) * 0.25 + lp3 * 2.0
    });
    let mut clips = vec![];
    for (sfx, s) in [
        (Sfx::Horn, horn),
        (Sfx::Whistle, whistle),
        (Sfx::Chuff, chuff),
        (Sfx::Hiss, hiss),
        (Sfx::Shovel, shovel),
        (Sfx::Clank, clank),
        (Sfx::Thud, thud),
        (Sfx::Bell, bell),
    ] {
        clips.push((sfx, audio.add(wav(&s))));
    }
    let rumble = audio.add(wav(&rumble));
    commands.insert_resource(Sounds { clips, rumble });
}

fn play_queued(mut commands: Commands, sounds: Option<Res<Sounds>>, mut queue: ResMut<SoundQueue>) {
    let Some(sounds) = sounds else {
        queue.0.clear();
        return;
    };
    for (sfx, vol, pitch) in queue.0.drain(..) {
        if let Some((_, h)) = sounds.clips.iter().find(|(s, _)| *s == sfx) {
            commands.spawn((
                AudioPlayer::new(h.clone()),
                PlaybackSettings { volume: Volume::Linear(vol), speed: pitch, ..PlaybackSettings::DESPAWN },
            ));
        }
    }
}

/// A looping engine sound whose volume and pitch can be changed each frame.
#[derive(Component)]
pub struct EngineLoop;

/// Keeps one looping diesel rumble alive; `level` 0 silences it.
pub fn set_engine_loop(
    commands: &mut Commands,
    sounds: &Sounds,
    q: &mut Query<(Entity, Option<&mut AudioSink>), With<EngineLoop>>,
    level: f32,
    pitch: f32,
) {
    match q.iter_mut().next() {
        Some((_, Some(mut sink))) => {
            sink.set_volume(Volume::Linear(level));
            sink.set_speed(pitch);
        }
        Some(_) => {}
        None if level > 0.0 => {
            commands.spawn((
                AudioPlayer::new(sounds.rumble.clone()),
                PlaybackSettings { volume: Volume::Linear(level), speed: pitch, ..PlaybackSettings::LOOP },
                EngineLoop,
            ));
        }
        None => {}
    }
}
