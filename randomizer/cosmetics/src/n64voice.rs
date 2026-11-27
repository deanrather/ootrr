use std::sync::Arc;

use crate::n64rom::{Envelope, Sample};

const FASTEST: f64 = 3.99996;

pub fn octave(i: u8) -> f32 {
    2f32.powf((f32::from(i) - 128.0) / 127.0)
}

pub fn release_rate(i: u8) -> f32 {
    let x: f32 = match i {
        0 => return 0.0,
        255 => 0.25,
        254 => 0.33,
        253 => 0.5,
        252 => 0.66,
        251 => 0.75,
        128..=250 => f32::from(251 - i),
        16..=127 => 4.0 * f32::from(143 - i),
        _ => 60.0 * f32::from(23 - i),
    };
    1.0 / (3.0 * x)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Start,
    Loop,
    Fade,
    Hang,
    Decay,
    Release,
    Sustain,
    Off,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Vibrato {
    pub rate_target: f32,
    pub rate_start: f32,
    pub rate_delay: u32,
    pub extent_target: f32,
    pub extent_start: f32,
    pub extent_delay: u32,
    pub delay: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct Slide {
    pub at: u32,
    pub speed: u32,
    pub extent: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Control {
    pub freq: f32,
    pub volume: f32,
    pub pan: u8,
    pub reverb: u8,
}

pub struct Voice {
    pub owner: u32,
    pub attached: bool,
    pub dead: bool,
    pub level: f32,
    pub control: Control,
    pub vibrato: Vibrato,
    pub slide: Option<Slide>,
    pub want: Option<State>,
    pub fade: f32,
    pub sustain: f32,
    sample: Option<Arc<Sample>>,
    position: f64,
    envelope: Envelope,
    index: usize,
    state: State,
    wait: i32,
    step: f32,
    gains: Option<(f32, f32)>,
    phase: u32,
    wobble_wait: u32,
    rate: f32,
    rate_left: u32,
    extent: f32,
    extent_left: u32,
}

fn sine(i: u32) -> f32 {
    (std::f32::consts::TAU * (i & 63) as f32 / 64.0).sin() * 32767.0
}

fn pan_gain(pan: u8) -> f32 {
    (f32::from(pan.min(127)) / 127.0 * std::f32::consts::FRAC_PI_2).cos()
}

impl Voice {
    pub fn new(owner: u32, sample: Option<Arc<Sample>>, envelope: Envelope, vibrato: Vibrato, control: Control) -> Voice {
        Voice {
            owner,
            attached: true,
            dead: false,
            level: 0.0,
            control,
            vibrato,
            slide: None,
            want: None,
            fade: 0.0,
            sustain: 0.0,
            sample,
            position: 0.0,
            envelope,
            index: 0,
            state: State::Start,
            wait: 0,
            step: 0.0,
            gains: None,
            phase: 0,
            wobble_wait: vibrato.delay,
            rate: if vibrato.rate_delay != 0 { vibrato.rate_start } else { vibrato.rate_target },
            rate_left: vibrato.rate_delay,
            extent: if vibrato.extent_delay != 0 { vibrato.extent_start } else { vibrato.extent_target },
            extent_left: vibrato.extent_delay,
        }
    }

    fn envelope_step(&mut self) -> f32 {
        for _ in 0..8 {
            if self.state == State::Start {
                self.index = 0;
                self.state = State::Loop;
            }
            if self.state != State::Loop {
                break;
            }
            let Some(&(delay, arg)) = self.envelope.get(self.index) else {
                self.state = State::Hang;
                break;
            };
            match delay {
                0 => self.state = State::Off,
                -1 => self.state = State::Hang,
                -2 => {
                    self.index = usize::try_from(arg).unwrap_or(0);
                    continue;
                }
                -3 => self.state = State::Start,
                _ => {
                    self.wait = ((f32::from(delay) * 0.75) as i32).max(1);
                    let target: f32 = (f32::from(arg) / 32767.0).powi(2);
                    self.step = (target - self.level) / self.wait as f32;
                    self.state = State::Fade;
                    self.index += 1;
                }
            }
            break;
        }
        match self.state {
            State::Fade => {
                self.level += self.step;
                self.wait -= 1;
                if self.wait <= 0 {
                    self.state = State::Loop;
                }
            }
            State::Decay | State::Release => {
                self.level -= self.fade;
                if self.sustain != 0.0 && self.state == State::Decay {
                    if self.level < self.sustain {
                        self.level = self.sustain;
                        self.wait = 128;
                        self.state = State::Sustain;
                    }
                } else if self.level < 0.00001 {
                    self.level = 0.0;
                    self.state = State::Off;
                }
            }
            State::Sustain => {
                self.wait -= 1;
                if self.wait <= 0 {
                    self.state = State::Release;
                }
            }
            _ => {}
        }
        if let Some(s) = self.want.take() {
            self.state = s;
        }
        if self.state == State::Off {
            self.dead = true;
        }
        self.level.clamp(0.0, 1.0)
    }

    fn bend(&mut self) -> f32 {
        let mut f: f32 = 1.0;
        if let Some(mut s) = self.slide {
            s.at += s.speed;
            let i: u32 = ((s.at >> 8) & 0xFF).min(127);
            f *= 1.0 + s.extent * (octave(i as u8 + 128) - 1.0);
            if i < 127 {
                self.slide = Some(s);
            }
        }
        if self.wobble_wait != 0 {
            self.wobble_wait -= 1;
            return f;
        }
        let v: Vibrato = self.vibrato;
        if self.extent_left != 0 {
            self.extent += (v.extent_target - self.extent) / self.extent_left as f32;
            self.extent_left -= 1;
        } else if v.extent_target != self.extent {
            self.extent_left = v.extent_delay;
            if self.extent_left == 0 {
                self.extent = v.extent_target;
            }
        }
        if self.rate_left != 0 {
            self.rate += (v.rate_target - self.rate) / self.rate_left as f32;
            self.rate_left -= 1;
        } else if v.rate_target != self.rate {
            self.rate_left = v.rate_delay;
            if self.rate_left == 0 {
                self.rate = v.rate_target;
            }
        }
        if self.extent != 0.0 {
            self.phase = self.phase.wrapping_add(self.rate as u32);
            let pitch: f32 = sine(self.phase >> 10) + 32768.0;
            let e: f32 = self.extent / 4096.0 + 1.0;
            f /= (e - 1.0 / e) * pitch / 65536.0 + 1.0 / e;
        }
        f
    }

    pub fn mix(&mut self, dry: &mut [[f32; 2]], wet: &mut [[f32; 2]]) {
        let level: f32 = self.envelope_step();
        let freq: f64 = f64::from(self.control.freq * self.bend()).min(FASTEST);
        let gain: f32 = (self.control.volume * level).clamp(0.0, 1.0);
        let pan: u8 = self.control.pan & 0x7F;
        let to: (f32, f32) = (gain * pan_gain(pan), gain * pan_gain(127 - pan));
        let from: (f32, f32) = self.gains.unwrap_or(to);
        self.gains = Some(to);
        let Some(s) = self.sample.clone().filter(|s| s.data.len() > 1) else {
            self.dead = true;
            return;
        };
        let loops: bool = s.loops && s.end > s.start;
        let send: f32 = f32::from(self.control.reverb) / 128.0;
        let n: usize = dry.len();
        for (k, (d, w)) in dry.iter_mut().zip(wet.iter_mut()).enumerate() {
            if loops {
                if self.position >= s.end as f64 {
                    self.position = s.start as f64 + (self.position - s.start as f64) % (s.end - s.start) as f64;
                }
            } else if self.position >= (s.data.len() - 1) as f64 {
                self.dead = true;
                break;
            }
            let i: usize = (self.position as usize).min(s.data.len() - 1);
            let j: usize = (i + 1).min(s.data.len() - 1);
            let x: f32 = s.data[i] + (s.data[j] - s.data[i]) * (self.position - i as f64) as f32;
            let t: f32 = k as f32 / n as f32;
            let left: f32 = x * (from.0 + (to.0 - from.0) * t);
            let right: f32 = x * (from.1 + (to.1 - from.1) * t);
            d[0] += left;
            d[1] += right;
            w[0] += left * send;
            w[1] += right * send;
            self.position += freq;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_looping_note_fades_out_after_release_and_dies() {
        let sample: Arc<Sample> = Arc::new(Sample { data: vec![1000.0; 64], start: 0, end: 64, loops: true });
        let envelope: Envelope = vec![(1, 32767), (-1, 0)].into();
        let control: Control = Control { freq: 1.0, volume: 1.0, pan: 64, reverb: 0 };
        let mut v: Voice = Voice::new(1, Some(sample), envelope, Vibrato::default(), control);
        let mut dry: Vec<[f32; 2]> = vec![[0.0; 2]; 100];
        let mut wet: Vec<[f32; 2]> = vec![[0.0; 2]; 100];
        for _ in 0..4 {
            v.mix(&mut dry, &mut wet);
        }
        assert!(!v.dead && dry[99][0] > 100.0);
        v.fade = 0.5;
        v.want = Some(State::Release);
        for _ in 0..4 {
            v.mix(&mut dry, &mut wet);
        }
        assert!(v.dead);
    }
}
