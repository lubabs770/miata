//! Procedural engine sound. The decoder runs on the audio thread and reads
//! rpm/load/running from atomics the game updates each frame.

use std::f32::consts::TAU;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::time::Duration;

use bevy::audio::{AddAudioSource, ChannelCount, SampleRate, Source};
use bevy::prelude::*;

use crate::AppState;
use drivetrain::Event;

use crate::driving::Drive;
use crate::input::Pedals;

const SAMPLE_RATE: u32 = 44_100;
/// 4-cylinder 4-stroke: two firing pulses per crank revolution.
const PULSES_PER_REV: f32 = 2.0;
/// Re-read the atomics every this many samples.
const CONTROL_EVERY: u32 = 64;

#[derive(Default)]
pub struct EngineParams {
    rpm: AtomicU32,
    load: AtomicU32,
    /// 1.0 running, 0.0 off (as f32 bits).
    on: AtomicU32,
}

impl EngineParams {
    fn set(&self, rpm: f32, load: f32, on: bool) {
        self.rpm.store(rpm.to_bits(), Relaxed);
        self.load.store(load.to_bits(), Relaxed);
        self.on
            .store(if on { 1.0f32 } else { 0.0 }.to_bits(), Relaxed);
    }
    fn get(&self) -> (f32, f32, f32) {
        let f = |a: &AtomicU32| f32::from_bits(a.load(Relaxed));
        (f(&self.rpm), f(&self.load), f(&self.on))
    }
}

#[derive(Asset, TypePath)]
pub struct EngineSound(Arc<EngineParams>);

#[derive(Resource)]
struct Engine(Arc<EngineParams>);

pub struct EngineDecoder {
    params: Arc<EngineParams>,
    phase: f32,
    rpm: f32,
    load: f32,
    amp: f32,
    target: (f32, f32, f32),
    n: u32,
    noise: u32,
}

impl EngineDecoder {
    fn new(params: Arc<EngineParams>) -> Self {
        Self {
            params,
            phase: 0.0,
            rpm: 0.0,
            load: 0.0,
            amp: 0.0,
            target: (0.0, 0.0, 0.0),
            n: 0,
            noise: 0x1234_5678,
        }
    }
}

impl Iterator for EngineDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        if self.n.is_multiple_of(CONTROL_EVERY) {
            self.target = self.params.get();
        }
        self.n = self.n.wrapping_add(1);
        let (rpm, load, on) = self.target;
        // One-pole smoothing so parameter steps don't click.
        self.rpm += (rpm - self.rpm) * 0.002;
        self.load += (load - self.load) * 0.002;
        self.amp += (on - self.amp) * 0.0005;

        let freq = self.rpm / 60.0 * PULSES_PER_REV;
        self.phase = (self.phase + freq / SAMPLE_RATE as f32).fract();
        let p = self.phase * TAU;
        let tone =
            p.sin() + 0.5 * (2.0 * p).sin() + 0.25 * (3.0 * p).sin() + 0.12 * (0.5 * p).sin();
        // xorshift noise, louder under load: the "working" rasp.
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        let noise = (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0;
        Some((tone * (0.6 + 0.4 * self.load) + noise * 0.15 * self.load) * self.amp * 0.15)
    }
}

impl Source for EngineDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(SAMPLE_RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

impl Decodable for EngineSound {
    type Decoder = EngineDecoder;
    fn decoder(&self) -> Self::Decoder {
        EngineDecoder::new(self.0.clone())
    }
}

pub fn plugin(app: &mut App) {
    app.add_audio_source::<EngineSound>()
        .add_audio_source::<Effect>()
        .insert_resource(Engine(Arc::default()))
        .init_resource::<LastHandbrake>()
        // Start only after the click-to-start gesture so browsers allow audio.
        .add_systems(OnEnter(AppState::Driving), start_engine_sound)
        .add_systems(
            Update,
            (feed_engine_sound, play_effects).run_if(in_state(AppState::Driving)),
        );
}

fn start_engine_sound(
    mut commands: Commands,
    mut sounds: ResMut<Assets<EngineSound>>,
    engine: Res<Engine>,
) {
    commands.spawn(AudioPlayer(sounds.add(EngineSound(engine.0.clone()))));
}

fn feed_engine_sound(engine: Res<Engine>, drive: Res<Drive>, pedals: Res<Pedals>) {
    let s = drive.sim.state();
    engine
        .0
        .set(s.engine_rpm, pedals.controls.throttle, s.engine_running);
}

/// One-shot sound effects, synthesised like the engine.
#[derive(Asset, TypePath, Clone, Copy, PartialEq, Debug)]
pub enum Effect {
    /// Gears clashing: harsh noise with a metallic ring.
    Grind,
    /// Engine dying: a low thump and shudder.
    Stall,
    /// Handbrake lever ratchet clicks.
    Ratchet,
    /// Paddle shift refused: a short double beep.
    Refused,
}

impl Effect {
    fn seconds(self) -> f32 {
        match self {
            Effect::Grind => 0.45,
            Effect::Stall => 0.6,
            Effect::Ratchet => 0.3,
            Effect::Refused => 0.25,
        }
    }

    /// Sample at time `t` seconds; `noise` is white noise in -1..1.
    fn sample(self, t: f32, noise: f32) -> f32 {
        let fade = (1.0 - t / self.seconds()).max(0.0);
        match self {
            Effect::Grind => {
                let ring = (TAU * 2300.0 * t).sin() * (TAU * 37.0 * t).sin();
                (noise * 0.7 + ring * 0.5) * fade * 0.5
            }
            Effect::Stall => {
                let thump = (TAU * 45.0 * t).sin() * (-t * 6.0).exp();
                let shudder = (TAU * 12.0 * t).sin().max(0.0) * noise * 0.3;
                (thump + shudder) * fade * 0.8
            }
            Effect::Ratchet => {
                // Six clicks: short noise bursts.
                let phase = (t * 20.0).fract();
                if phase < 0.15 {
                    noise * 0.6 * fade
                } else {
                    0.0
                }
            }
            Effect::Refused => {
                let on = !(0.1..0.15).contains(&t);
                if on {
                    (TAU * 880.0 * t).sin() * 0.2
                } else {
                    0.0
                }
            }
        }
    }
}

pub struct EffectDecoder {
    effect: Effect,
    n: u32,
    noise: u32,
}

impl Iterator for EffectDecoder {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        let t = self.n as f32 / SAMPLE_RATE as f32;
        if t >= self.effect.seconds() {
            return None;
        }
        self.n += 1;
        self.noise ^= self.noise << 13;
        self.noise ^= self.noise >> 17;
        self.noise ^= self.noise << 5;
        let noise = (self.noise as f32 / u32::MAX as f32) * 2.0 - 1.0;
        Some(self.effect.sample(t, noise))
    }
}

impl Source for EffectDecoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(1).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        SampleRate::new(SAMPLE_RATE).unwrap()
    }
    fn total_duration(&self) -> Option<Duration> {
        Some(Duration::from_secs_f32(self.effect.seconds()))
    }
}

impl Decodable for Effect {
    type Decoder = EffectDecoder;
    fn decoder(&self) -> Self::Decoder {
        EffectDecoder {
            effect: *self,
            n: 0,
            noise: 0x9E37_79B9,
        }
    }
}

#[derive(Resource, Default)]
struct LastHandbrake(bool);

fn play_effects(
    mut commands: Commands,
    mut effects: ResMut<Assets<Effect>>,
    drive: Res<Drive>,
    pedals: Res<Pedals>,
    mut last_handbrake: ResMut<LastHandbrake>,
) {
    let mut play = |e: Effect| {
        commands.spawn((AudioPlayer(effects.add(e)), PlaybackSettings::DESPAWN));
    };
    for ev in &drive.last_events {
        match ev {
            Event::Grind => play(Effect::Grind),
            Event::Stalled => play(Effect::Stall),
            Event::ShiftRefused => play(Effect::Refused),
            _ => {}
        }
    }
    if pedals.handbrake_on != last_handbrake.0 {
        last_handbrake.0 = pedals.handbrake_on;
        play(Effect::Ratchet);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rms(d: &mut EngineDecoder, n: usize) -> f32 {
        (d.take(n).map(|x| x * x).sum::<f32>() / n as f32).sqrt()
    }

    #[test]
    fn effects_end_and_make_sound() {
        for e in [
            Effect::Grind,
            Effect::Stall,
            Effect::Ratchet,
            Effect::Refused,
        ] {
            let samples: Vec<f32> = e.decoder().collect();
            let expected = (e.seconds() * SAMPLE_RATE as f32).ceil() as usize;
            assert!(samples.len().abs_diff(expected) <= 1, "{e:?} length");
            assert!(samples.iter().any(|x| x.abs() > 0.05), "{e:?} is silent");
            assert!(samples.iter().all(|x| x.abs() <= 1.0), "{e:?} clips");
        }
    }

    #[test]
    fn silent_when_off_audible_when_running() {
        let params = Arc::new(EngineParams::default());
        let mut d = EngineDecoder::new(params.clone());
        params.set(3000.0, 0.0, false);
        assert!(rms(&mut d, 44_100) < 1e-4);
        params.set(3000.0, 0.5, true);
        let _ = rms(&mut d, 44_100); // let the fade-in settle
        assert!(rms(&mut d, 44_100) > 0.02);
    }
}
