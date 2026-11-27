use std::collections::BTreeMap as BM;
use std::sync::Arc;

use crate::n64rom::{Drum, Envelope, Font, Instrument, Sound, Tables};
use crate::n64voice::{octave, release_rate, Control, Slide, State, Vibrato, Voice};
use crate::rng::Rng;

pub type Fonts = BM<u8, Arc<Font>>;

const TEMPO_MAX: u32 = 10770;
const GATE_SCALE: f32 = 60.0 * 3.0 / 32006.0 / TEMPO_MAX as f32;
const STEPS: usize = 2000;
const SHORTEST_LOOP: usize = 180;
pub fn note_freq(n: u8) -> f32 {
    let mut n: i32 = i32::from(n & 0x7F);
    if n >= 0x75 {
        n -= 128;
    }
    2f32.powf((n - 39) as f32 / 12.0)
}

#[derive(Clone, Debug, Default)]
struct Script {
    pc: usize,
    stack: Vec<usize>,
    count: [u8; 8],
    value: i8,
}

impl Script {
    fn at(pc: usize) -> Script {
        Script { pc, ..Script::default() }
    }
}

enum Flow {
    On,
    End,
    Wait(u32),
}

struct Data(Vec<u8>);

impl Data {
    fn get(&self, at: usize) -> u8 {
        self.0.get(at).copied().unwrap_or(0xFF)
    }

    fn set(&mut self, at: usize, v: u8) {
        if let Some(x) = self.0.get_mut(at) {
            *x = v;
        }
    }

    fn be(&self, at: usize) -> u16 {
        u16::from_be_bytes([self.get(at), self.get(at + 1)])
    }

    fn u8(&self, s: &mut Script) -> u8 {
        let v: u8 = self.get(s.pc);
        s.pc = s.pc.wrapping_add(1) & 0xFFFF;
        v
    }

    fn u16(&self, s: &mut Script) -> u16 {
        let high: u8 = self.u8(s);
        u16::from_be_bytes([high, self.u8(s)])
    }

    fn var(&self, s: &mut Script) -> u32 {
        let v: u32 = u32::from(self.u8(s));
        if v & 0x80 == 0 {
            return v;
        }
        ((v << 8) & 0x7F00) | u32::from(self.u8(s))
    }

    fn envelope(&self, at: usize) -> Envelope {
        (0..32).map(|i| (self.be(at + 4 * i) as i16, self.be(at + 4 * i + 2) as i16)).collect::<Vec<(i16, i16)>>().into()
    }

    fn arg(&self, s: &mut Script, code: u8) -> u16 {
        match (code & 3, code & 0x80) {
            (1, 0) => u16::from(self.u8(s)),
            (1, _) => self.u16(s),
            _ => 0,
        }
    }

    fn flow(&self, s: &mut Script, cmd: u8, arg: u16) -> Flow {
        match cmd {
            0xFF => match s.stack.pop() {
                Some(pc) => s.pc = pc,
                None => return Flow::End,
            },
            0xFD => return Flow::Wait(self.var(s)),
            0xFE => return Flow::Wait(1),
            0xFC | 0xF8 => {
                if s.stack.len() >= 8 {
                    return Flow::End;
                }
                if cmd == 0xF8 {
                    s.count[s.stack.len()] = arg as u8;
                }
                s.stack.push(s.pc);
                if cmd == 0xFC {
                    s.pc = usize::from(arg);
                }
            }
            0xF7 => {
                let Some(&back) = s.stack.last() else {
                    return Flow::End;
                };
                let d: usize = s.stack.len() - 1;
                s.count[d] = s.count[d].wrapping_sub(1);
                if s.count[d] != 0 {
                    s.pc = back;
                } else {
                    s.stack.pop();
                }
            }
            0xF6 => {
                s.stack.pop();
            }
            0xF5 | 0xF9 | 0xFA | 0xFB => {
                let skip: bool = (cmd == 0xFA && s.value != 0) || (cmd == 0xF9 && s.value >= 0) || (cmd == 0xF5 && s.value < 0);
                if !skip {
                    s.pc = usize::from(arg);
                }
            }
            0xF2..=0xF4 => {
                let skip: bool = (cmd == 0xF3 && s.value != 0) || (cmd == 0xF2 && s.value >= 0);
                if !skip {
                    s.pc = s.pc.wrapping_add_signed(isize::from(arg as u8 as i8)) & 0xFFFF;
                }
            }
            _ => {}
        }
        Flow::On
    }
}

struct Layer {
    id: u32,
    script: Script,
    envelope: Envelope,
    release: u8,
    enabled: bool,
    stop: bool,
    continuous: bool,
    ignore_drum_pan: bool,
    slide_mode: u8,
    slide_note: u8,
    slide_time: u32,
    gate: u8,
    pan: u8,
    transpose: i32,
    delay: u32,
    gate_delay: u32,
    last_delay: u32,
    default_delay: u32,
    voice: Option<u32>,
    instrument: Option<Arc<Instrument>>,
    which: u16,
    sound: Option<Sound>,
    freq: f32,
    bend: f32,
    velocity: f32,
    control: Control,
}

struct Channel {
    enabled: bool,
    stopped: bool,
    script: Script,
    io: [i8; 8],
    layers: [Option<Layer>; 4],
    has_instrument: bool,
    transpose: i32,
    large: bool,
    new_pan: u8,
    pan_weight: u8,
    reverb: u8,
    delay: u32,
    envelope: Envelope,
    release: u8,
    sustain: u8,
    vibrato: Vibrato,
    volume: f32,
    scale: f32,
    freq: f32,
    font: u8,
    which: u16,
    instrument: Option<Arc<Instrument>>,
    table: usize,
    register: u16,
}

impl Channel {
    fn new(font: u8, envelope: Envelope) -> Channel {
        Channel {
            enabled: false,
            stopped: false,
            script: Script::default(),
            io: [-1; 8],
            layers: [None, None, None, None],
            has_instrument: false,
            transpose: 0,
            large: false,
            new_pan: 0x40,
            pan_weight: 0x80,
            reverb: 0,
            delay: 0,
            envelope,
            release: 0xF0,
            sustain: 0,
            vibrato: Vibrato { rate_target: 2048.0, rate_start: 2048.0, ..Vibrato::default() },
            volume: 1.0,
            scale: 1.0,
            freq: 1.0,
            font,
            which: 0,
            instrument: None,
            table: 0,
            register: 0,
        }
    }
}

struct Mix {
    voices: BM<u32, Voice>,
    next: u32,
}

impl Mix {
    fn alive(&self, layer: &Layer) -> Option<&Voice> {
        layer.voice.and_then(|v| self.voices.get(&v)).filter(|v| !v.dead && v.owner == layer.id)
    }

    fn decay(&mut self, layer: &Layer, release: u8, sustain: u8) {
        let Some(v) = layer.voice.and_then(|v| self.voices.get_mut(&v)).filter(|v| !v.dead && v.owner == layer.id && v.attached) else {
            return;
        };
        v.control = layer.control;
        v.attached = false;
        v.want = Some(State::Decay);
        v.fade = release_rate(if layer.release == 0 { release } else { layer.release });
        v.sustain = f32::from(sustain) * v.level / 256.0;
    }

    fn detach(&mut self, id: u32) {
        if let Some(v) = self.voices.get_mut(&id).filter(|v| v.attached) {
            v.attached = false;
            v.fade = 1.0 / 3.0;
            v.want = Some(State::Release);
        }
    }
}

pub struct Player<'a> {
    data: Data,
    sets: &'a [u8],
    fonts: &'a Fonts,
    tables: &'a Tables,
    rng: Rng,
    transpose: i32,
    tempo: u32,
    tempo_add: i32,
    clock: i32,
    delay: u32,
    fade: f32,
    fade_scale: f32,
    fade_left: u32,
    fade_next: u32,
    fade_step: f32,
    velocity_table: Option<usize>,
    gate_table: Option<usize>,
    script: Script,
    io: [i8; 8],
    channels: Vec<Channel>,
    mix: Mix,
    layers: u32,
    visited: BM<(usize, usize), usize>,
    pub now: usize,
    pub repeat: Option<(usize, usize)>,
    pub done: bool,
}

impl<'a> Player<'a> {
    pub fn new(data: &[u8], sets: &'a [u8], fonts: &'a Fonts, tables: &'a Tables) -> Player<'a> {
        let mut bytes: Vec<u8> = data.to_vec();
        bytes.resize(data.len() + 16, 0xFF);
        let font: u8 = sets.last().copied().unwrap_or(0);
        Player {
            data: Data(bytes),
            sets,
            fonts,
            tables,
            rng: Rng::from_seed(1),
            transpose: 0,
            tempo: 5760,
            tempo_add: 0,
            clock: 0,
            delay: 0,
            fade: 1.0,
            fade_scale: 1.0,
            fade_left: 0,
            fade_next: 0,
            fade_step: 0.0,
            velocity_table: None,
            gate_table: None,
            script: Script::default(),
            io: [-1; 8],
            channels: (0..16).map(|_| Channel::new(font, tables.envelope.clone())).collect(),
            mix: Mix { voices: BM::new(), next: 0 },
            layers: 0,
            visited: BM::new(),
            now: 0,
            repeat: None,
            done: false,
        }
    }

    pub fn sounding(&self) -> bool {
        !self.mix.voices.is_empty()
    }

    pub fn mix(&mut self, dry: &mut [[f32; 2]], wet: &mut [[f32; 2]]) {
        let mut gone: Vec<u32> = Vec::new();
        for c in &self.channels {
            for layer in c.layers.iter().flatten() {
                if let Some(v) = layer.voice.and_then(|v| self.mix.voices.get_mut(&v)).filter(|v| v.attached && v.owner == layer.id) {
                    v.control = layer.control;
                    v.vibrato = c.vibrato;
                }
            }
        }
        for (id, v) in self.mix.voices.iter_mut() {
            v.mix(dry, wet);
            if v.dead {
                gone.push(*id);
            }
        }
        for id in gone {
            self.mix.voices.remove(&id);
        }
    }

    pub fn update(&mut self) {
        self.clock += self.tempo as i32 + self.tempo_add;
        if self.clock >= TEMPO_MAX as i32 {
            self.clock -= TEMPO_MAX as i32;
            self.tick();
        }
        if self.fade_left != 0 {
            self.fade = (self.fade + self.fade_step).clamp(0.0, 1.0);
            self.fade_left -= 1;
        }
        let volume: f32 = self.fade * self.fade_scale;
        for c in self.channels.iter_mut().filter(|c| c.enabled) {
            let applied: f32 = (c.volume * c.scale * volume).powi(2);
            let pan: u32 = u32::from(c.new_pan) * u32::from(c.pan_weight);
            let weight: u32 = 0x80u32.saturating_sub(u32::from(c.pan_weight));
            for layer in c.layers.iter_mut().flatten().filter(|l| l.enabled) {
                layer.control = Control { freq: layer.freq * c.freq, volume: layer.velocity * applied, pan: ((pan + u32::from(layer.pan) * weight) >> 7).min(127) as u8, reverb: c.reverb };
            }
        }
    }

    fn tick(&mut self) {
        if self.done {
            return;
        }
        if self.delay > 1 {
            self.delay -= 1;
        } else {
            self.run();
        }
        if self.done {
            return;
        }
        for i in 0..self.channels.len() {
            if !self.channels[i].enabled {
                continue;
            }
            if !self.channels[i].stopped {
                if self.channels[i].delay >= 2 {
                    self.channels[i].delay -= 1;
                } else {
                    self.run_channel(i);
                }
            }
            if !self.channels[i].enabled {
                continue;
            }
            for l in 0..4 {
                if let Some(mut layer) = self.channels[i].layers[l].take() {
                    self.run_layer(i, &mut layer);
                    self.channels[i].layers[l] = Some(layer);
                }
            }
        }
    }

    fn finish(&mut self) {
        self.done = true;
        for i in 0..self.channels.len() {
            self.disable(i);
        }
    }

    fn free(&mut self, i: usize, l: usize) {
        let (release, sustain): (u8, u8) = (self.channels[i].release, self.channels[i].sustain);
        if let Some(layer) = self.channels[i].layers[l].take() {
            self.mix.decay(&layer, release, sustain);
        }
    }

    fn disable(&mut self, i: usize) {
        for l in 0..4 {
            self.free(i, l);
        }
        self.channels[i].enabled = false;
    }

    fn enable(&mut self, i: usize, pc: usize) {
        for l in 0..4 {
            self.free(i, l);
        }
        let c: &mut Channel = &mut self.channels[i];
        c.enabled = true;
        c.script = Script::at(pc);
        c.delay = 0;
    }

    fn set_layer(&mut self, i: usize, l: usize, pc: usize) {
        if l >= 4 {
            return;
        }
        self.free(i, l);
        self.layers += 1;
        let c: &mut Channel = &mut self.channels[i];
        c.layers[l] = Some(Layer {
            id: self.layers,
            script: Script::at(pc),
            envelope: c.envelope.clone(),
            release: 0,
            enabled: true,
            stop: false,
            continuous: false,
            ignore_drum_pan: false,
            slide_mode: 0,
            slide_note: 0,
            slide_time: 0,
            gate: 0x80,
            pan: 0x40,
            transpose: 0,
            delay: 0,
            gate_delay: 0,
            last_delay: 0,
            default_delay: 0,
            voice: None,
            instrument: None,
            which: 0xFF,
            sound: None,
            freq: 1.0,
            bend: 1.0,
            velocity: 1.0,
            control: Control { freq: 1.0, volume: 0.0, pan: 64, reverb: 0 },
        });
    }

    fn font(&self, i: usize) -> Option<&Arc<Font>> {
        self.fonts.get(&self.channels[i].font)
    }

    fn set_font(&mut self, i: usize, back: u8) {
        if let Some(at) = self.sets.len().checked_sub(usize::from(back) + 1) {
            self.channels[i].font = self.sets[at];
        }
    }

    fn instrument(&self, i: usize, n: u8) -> Option<Arc<Instrument>> {
        self.font(i)?.instruments.get(usize::from(n))?.clone().map(Arc::new)
    }

    fn set_instrument(&mut self, i: usize, n: u8) {
        if n >= 0x7E {
            let c: &mut Channel = &mut self.channels[i];
            c.which = match n {
                0x7E => 1,
                0x7F => 0,
                _ => u16::from(n),
            };
            c.instrument = None;
            c.has_instrument = true;
            return;
        }
        let found: Option<Arc<Instrument>> = self.instrument(i, n);
        let c: &mut Channel = &mut self.channels[i];
        match found {
            Some(inst) => {
                c.which = u16::from(n) + 2;
                match &inst.envelope {
                    Some(e) => {
                        c.envelope = e.clone();
                        c.release = inst.release;
                    }
                    None => c.envelope = self.tables.envelope.clone(),
                }
                c.instrument = Some(inst);
                c.has_instrument = true;
            }
            None => {
                c.which = 0;
                c.has_instrument = false;
            }
        }
    }

    fn run_layer(&mut self, i: usize, layer: &mut Layer) {
        if !layer.enabled {
            return;
        }
        let (release, sustain): (u8, u8) = (self.channels[i].release, self.channels[i].sustain);
        if layer.delay > 1 {
            layer.delay -= 1;
            if !layer.stop && layer.delay <= layer.gate_delay {
                self.mix.decay(layer, release, sustain);
                layer.stop = true;
            }
            return;
        }
        if !layer.continuous {
            self.mix.decay(layer, release, sustain);
            if let Some(v) = layer.voice.take() {
                if let Some(x) = self.mix.voices.get_mut(&v) {
                    x.attached = false;
                }
            }
        }
        if matches!(layer.slide_mode & 0x7F, 1 | 2) {
            layer.slide_mode = 0;
        }
        let Some(cmd) = self.layer_command(i, layer) else {
            return;
        };
        if let Some(note) = self.timing(i, layer, cmd) {
            if let Some((same, slide)) = self.pitch(i, layer, note) {
                self.start(i, layer, same, slide);
            }
        }
        if layer.stop {
            self.mix.decay(layer, release, sustain);
        }
    }

    fn end_layer(&mut self, i: usize, layer: &mut Layer) {
        self.mix.decay(layer, self.channels[i].release, self.channels[i].sustain);
        layer.enabled = false;
    }

    fn layer_command(&mut self, i: usize, layer: &mut Layer) -> Option<u8> {
        for _ in 0..STEPS {
            let cmd: u8 = self.data.u8(&mut layer.script);
            if cmd < 0xC1 {
                return Some(cmd);
            }
            if cmd >= 0xF2 {
                let arg: u16 = self.data.arg(&mut layer.script, self.tables.args[usize::from(cmd - 0xB0)]);
                if !matches!(self.data.flow(&mut layer.script, cmd, arg), Flow::On) {
                    break;
                }
                continue;
            }
            let s: &mut Script = &mut layer.script;
            match cmd {
                0xC1 => layer.velocity = f32::from(self.data.u8(s)).powi(2) / 16129.0,
                0xCA => layer.pan = self.data.u8(s),
                0xC9 => layer.gate = self.data.u8(s),
                0xC2 => layer.transpose = i32::from(self.data.u8(s)),
                0xC4 | 0xC5 => {
                    layer.continuous = cmd == 0xC4;
                    self.mix.decay(layer, self.channels[i].release, self.channels[i].sustain);
                }
                0xC3 => layer.default_delay = self.data.var(s),
                0xC6 => {
                    let n: u8 = self.data.u8(s);
                    if n >= 0x7E {
                        layer.which = match n {
                            0x7E => 1,
                            0x7F => 0,
                            _ => u16::from(n),
                        };
                        layer.instrument = None;
                        if n == 0xFF {
                            layer.release = 0;
                        }
                    } else if let Some(inst) = self.instrument(i, n) {
                        layer.which = u16::from(n) + 2;
                        match &inst.envelope {
                            Some(e) => {
                                layer.envelope = e.clone();
                                layer.release = inst.release;
                            }
                            None => layer.envelope = self.tables.envelope.clone(),
                        }
                        layer.instrument = Some(inst);
                    } else {
                        layer.which = 0xFF;
                    }
                }
                0xC7 => {
                    layer.slide_mode = self.data.u8(s);
                    let t: i32 = i32::from(self.data.u8(s)) + self.channels[i].transpose + layer.transpose + self.transpose;
                    layer.slide_note = if (0..0x80).contains(&(t & 0xFF)) { (t & 0xFF) as u8 } else { 0 };
                    layer.slide_time = if layer.slide_mode & 0x80 != 0 { u32::from(self.data.u8(s)) } else { self.data.var(s) };
                }
                0xC8 => layer.slide_mode = 0,
                0xCB => {
                    layer.envelope = self.data.envelope(usize::from(self.data.u16(s)));
                    layer.release = self.data.u8(s);
                }
                0xCF => layer.release = self.data.u8(s),
                0xCC => layer.ignore_drum_pan = true,
                0xCD => {
                    self.data.u8(s);
                }
                0xCE => {
                    let n: u8 = self.data.u8(s).wrapping_add(0x80);
                    layer.bend = octave(n).powf(1.0 / 6.0);
                }
                0xD0..=0xDF => {
                    let v: u8 = self.velocity_table.map_or(self.tables.velocity[usize::from(cmd & 15)], |t| self.data.get(t + usize::from(cmd & 15)));
                    layer.velocity = f32::from(v).powi(2) / 16129.0;
                }
                0xE0..=0xEF => layer.gate = self.gate_table.map_or(self.tables.gate[usize::from(cmd & 15)], |t| self.data.get(t + usize::from(cmd & 15))),
                _ => {}
            }
        }
        self.end_layer(i, layer);
        None
    }

    fn timing(&mut self, i: usize, layer: &mut Layer, cmd: u8) -> Option<u8> {
        let s: &mut Script = &mut layer.script;
        if cmd == 0xC0 {
            layer.delay = self.data.var(s);
            layer.stop = true;
            return None;
        }
        layer.stop = false;
        let kind: u8 = cmd & 0xC0;
        let delay: u32;
        if self.channels[i].large {
            if kind == 0x80 {
                delay = layer.last_delay;
            } else {
                delay = self.data.var(s);
                layer.last_delay = delay;
            }
            let velocity: u8 = self.data.u8(s).min(127);
            layer.gate = if kind == 0x40 { 0 } else { self.data.u8(s) };
            layer.velocity = f32::from(velocity).powi(2) / 16129.0;
        } else {
            delay = match kind {
                0x00 => {
                    layer.last_delay = self.data.var(s);
                    layer.last_delay
                }
                0x40 => layer.default_delay,
                _ => layer.last_delay,
            };
        }
        layer.delay = delay;
        layer.gate_delay = (u32::from(layer.gate) * delay) >> 8;
        Some(cmd - kind)
    }

    fn pitch(&mut self, i: usize, layer: &mut Layer, note: u8) -> Option<(bool, Option<Slide>)> {
        let c: &Channel = &self.channels[i];
        let mut which: u16 = layer.which;
        if which == 0xFF {
            if !c.has_instrument {
                return None;
            }
            which = c.which;
        }
        let mut same: bool = true;
        let mut slide: Option<Slide> = None;
        if which == 0 {
            let n: usize = ((i32::from(note) + c.transpose + layer.transpose) & 0xFF) as usize;
            let Some(drum) = self.font(i).and_then(|f| f.drums.get(n)).and_then(|d| d.clone()) else {
                layer.stop = true;
                return None;
            };
            let drum: Drum = drum;
            layer.envelope = drum.envelope;
            layer.release = drum.release;
            if !layer.ignore_drum_pan {
                layer.pan = drum.pan;
            }
            layer.freq = drum.sound.tuning;
            layer.sound = Some(drum.sound);
        } else if which == 1 || which >= 0x80 {
            layer.stop = true;
            return None;
        } else {
            let n: i32 = (i32::from(note) + self.transpose + c.transpose + layer.transpose) & 0xFF;
            let inst: Option<Arc<Instrument>> = if layer.which == 0xFF { c.instrument.clone() } else { layer.instrument.clone() };
            let (Ok(n), Some(inst)) = (u8::try_from(n), inst) else {
                layer.stop = true;
                return None;
            };
            let at: u8 = if layer.slide_mode != 0 { n.max(layer.slide_note) } else { n };
            let Some(sound) = inst.sound(at).filter(|_| n < 0x80).cloned() else {
                layer.stop = true;
                return None;
            };
            same = layer.sound.as_ref().is_some_and(|s| Arc::ptr_eq(&s.sample, &sound.sample));
            layer.freq = note_freq(n) * sound.tuning;
            if layer.slide_mode != 0 {
                let mode: u8 = layer.slide_mode & 0x7F;
                let a: f32 = note_freq(n) * sound.tuning;
                let b: f32 = note_freq(layer.slide_note) * sound.tuning;
                let (from, to): (f32, f32) = match mode {
                    1 | 3 | 5 => (b, a),
                    2 | 4 => (a, b),
                    _ => (a, a),
                };
                let speed: u32 = if layer.slide_mode & 0x80 != 0 {
                    let s: u32 = self.tempo * 0x8000 / TEMPO_MAX;
                    let over: u32 = layer.delay.saturating_mul(layer.slide_time);
                    if over != 0 { s * 0x100 / over } else { s }
                } else {
                    0x20000 / (layer.slide_time.saturating_mul(3)).max(1)
                };
                slide = Some(Slide { at: 0, speed: speed.clamp(1, 0x7FFF), extent: to / from - 1.0 });
                layer.freq = from;
                if mode == 5 {
                    layer.slide_note = n;
                }
            }
            layer.sound = Some(sound);
        }
        layer.freq *= layer.bend;
        if layer.delay == 0 {
            let length: usize = layer.sound.as_ref().map_or(0, |s| s.sample.end);
            let t: f32 = length as f32 * self.tempo as f32 * GATE_SCALE / layer.freq;
            layer.gate_delay = 0;
            layer.delay = t.min(32766.0) as u32 + 1;
        }
        Some((same, slide))
    }

    fn start(&mut self, i: usize, layer: &mut Layer, same: bool, slide: Option<Slide>) {
        let keep: bool = layer.continuous && same && self.mix.alive(layer).is_some_and(|v| v.attached);
        if !keep {
            if let Some(v) = layer.voice.take() {
                self.mix.detach(v);
            }
            let c: &Channel = &self.channels[i];
            let envelope: Envelope = if layer.release == 0 { c.envelope.clone() } else { layer.envelope.clone() };
            let control: Control = Control { freq: layer.freq * c.freq, volume: 0.0, pan: layer.pan, reverb: c.reverb };
            let sample = layer.sound.as_ref().map(|s| s.sample.clone());
            self.mix.next += 1;
            self.mix.voices.insert(self.mix.next, Voice::new(layer.id, sample, envelope, c.vibrato, control));
            layer.voice = Some(self.mix.next);
        }
        if let Some(v) = layer.voice.and_then(|v| self.mix.voices.get_mut(&v)) {
            v.slide = slide;
        }
    }

    fn random(&mut self, n: u32) -> u32 {
        self.rng.below(u64::from(n.max(1))) as u32
    }

    fn run_channel(&mut self, i: usize) {
        for _ in 0..STEPS {
            let mut s: Script = std::mem::take(&mut self.channels[i].script);
            let cmd: u8 = self.data.u8(&mut s);
            let mut out: bool = false;
            if cmd >= 0xF2 {
                let arg: u16 = self.data.arg(&mut s, self.tables.args[usize::from(cmd - 0xB0)]);
                match self.data.flow(&mut s, cmd, arg) {
                    Flow::On => {}
                    Flow::End => {
                        self.disable(i);
                        out = true;
                    }
                    Flow::Wait(n) => {
                        self.channels[i].delay = n;
                        out = true;
                    }
                }
            } else if cmd >= 0xB0 {
                out = self.setting(i, &mut s, cmd);
            } else if cmd >= 0x70 {
                self.layer_order(i, &mut s, cmd);
            } else {
                let low: usize = usize::from(cmd & 15);
                match cmd & 0xF0 {
                    0x00 => {
                        self.channels[i].delay = u32::from(cmd & 15);
                        out = true;
                    }
                    0x10 => self.channels[i].io[low & 7] = 1,
                    0x60 => {
                        s.value = self.channels[i].io[low & 7];
                        if low < 2 {
                            self.channels[i].io[low] = -1;
                        }
                    }
                    0x50 => s.value = s.value.wrapping_sub(self.channels[i].io[low & 7]),
                    0x20 => {
                        let pc: usize = usize::from(self.data.u16(&mut s));
                        self.enable(low, pc);
                    }
                    0x30 => {
                        let port: usize = usize::from(self.data.u8(&mut s) & 7);
                        self.channels[low].io[port] = s.value;
                    }
                    0x40 => {
                        let port: usize = usize::from(self.data.u8(&mut s) & 7);
                        s.value = self.channels[low].io[port];
                    }
                    _ => {}
                }
            }
            self.channels[i].script = s;
            if out {
                return;
            }
        }
        self.disable(i);
    }

    fn layer_order(&mut self, i: usize, s: &mut Script, cmd: u8) {
        let top: u8 = cmd & 0xF8;
        let mut l: usize = usize::from(cmd & 7);
        if top != 0x70 && l >= 4 {
            l = 0;
        }
        match top {
            0x80 => s.value = self.channels[i].layers[l & 3].as_ref().map_or(-1, |x| i8::from(!x.enabled)),
            0x88 => {
                let pc: usize = usize::from(self.data.u16(s));
                self.set_layer(i, l, pc);
            }
            0x90 => self.free(i, l & 3),
            0x98 => {
                if s.value != -1 {
                    let at: usize = self.channels[i].table.wrapping_add_signed(2 * isize::from(s.value));
                    self.set_layer(i, l, usize::from(self.data.be(at)));
                }
            }
            0x70 => self.channels[i].io[l] = s.value,
            0x78 => {
                let rel: i16 = self.data.u16(s) as i16;
                self.set_layer(i, l, s.pc.wrapping_add_signed(isize::from(rel)) & 0xFFFF);
            }
            _ => {}
        }
    }

    fn setting(&mut self, i: usize, s: &mut Script, cmd: u8) -> bool {
        let code: u8 = self.tables.args[usize::from(cmd - 0xB0)];
        let mut args: [u16; 3] = [0; 3];
        let mut wide: u8 = code;
        for a in args.iter_mut().take(usize::from(code & 3)) {
            *a = if wide & 0x80 != 0 { self.data.u16(s) } else { u16::from(self.data.u8(s)) };
            wide <<= 1;
        }
        let u: u16 = args[0];
        let at: usize = usize::from(u);
        let b: u8 = u as u8;
        let signed: i8 = b as i8;
        let offset = |base: usize, n: i8| -> usize { base.wrapping_add_signed(isize::from(n)) };
        let pairs = |base: usize, n: i8| -> usize { base.wrapping_add_signed(2 * isize::from(n)) };
        match cmd {
            0xEA => {
                self.channels[i].stopped = true;
                return true;
            }
            0xEB => {
                self.set_font(i, b);
                self.set_instrument(i, args[1] as u8);
            }
            0xC1 => self.set_instrument(i, b),
            0xC6 => self.set_font(i, b),
            0xC7 => self.data.set(usize::from(args[1]), (s.value as u8).wrapping_add(b)),
            0xC8 => s.value = s.value.wrapping_sub(signed),
            0xCC => s.value = signed,
            0xC9 => s.value &= signed,
            0xCD => self.disable(usize::from(b & 15)),
            0xCB => s.value = self.data.get(offset(at, s.value)) as i8,
            0xB7 => self.channels[i].register = self.random(if u == 0 { 0x10000 } else { u32::from(u) }) as u16,
            0xB8 => s.value = self.random(if b == 0 { 0x10000 } else { u32::from(b) }) as u8 as i8,
            0xBD => {
                let r: u32 = self.random(if u == 0 { 0x10000 } else { u32::from(u) }) + u32::from(args[1]);
                self.channels[i].register = (((r / 256 + 0x80) << 8) | (r % 256)) as u16;
            }
            _ => {
                let data: &mut Data = &mut self.data;
                let c: &mut Channel = &mut self.channels[i];
                let v: &mut Vibrato = &mut c.vibrato;
                match cmd {
                    0xC2 => c.table = at,
                    0xC5 => {
                        if s.value != -1 {
                            c.table = usize::from(data.be(pairs(c.table, s.value)));
                        }
                    }
                    0xC3 => c.large = false,
                    0xC4 => c.large = true,
                    0xDF => c.volume = f32::from(b) / 127.0,
                    0xE0 => c.scale = f32::from(b) / 128.0,
                    0xDE => c.freq = f32::from(u) / 32768.0,
                    0xD3 => c.freq = octave(b.wrapping_add(0x80)),
                    0xEE => c.freq = octave(b.wrapping_add(0x80)).powf(1.0 / 6.0),
                    0xDD => c.new_pan = b,
                    0xDC => c.pan_weight = b,
                    0xDB => c.transpose = i32::from(signed),
                    0xDA => c.envelope = data.envelope(at),
                    0xD9 => c.release = b,
                    0xD8 => {
                        v.extent_target = f32::from(b) * 8.0;
                        v.extent_start = 0.0;
                        v.extent_delay = 0;
                    }
                    0xD7 => {
                        v.rate_delay = 0;
                        v.rate_target = f32::from(b) * 32.0;
                        v.rate_start = v.rate_target;
                    }
                    0xE2 => {
                        v.extent_start = f32::from(b) * 8.0;
                        v.extent_target = f32::from(args[1] as u8) * 8.0;
                        v.extent_delay = u32::from(args[2] as u8) * 16;
                    }
                    0xE1 => {
                        v.rate_start = f32::from(b) * 32.0;
                        v.rate_target = f32::from(args[1] as u8) * 32.0;
                        v.rate_delay = u32::from(args[2] as u8) * 16;
                    }
                    0xE3 => v.delay = u32::from(b) * 16,
                    0xD4 => c.reverb = b,
                    0xCE => c.register = u,
                    0xCF => {
                        data.set(at, (c.register >> 8) as u8);
                        data.set(at + 1, c.register as u8);
                    }
                    0xD2 => c.sustain = b,
                    0xE4 => {
                        if s.value != -1 && s.stack.len() < 8 {
                            s.stack.push(s.pc);
                            s.pc = usize::from(data.be(pairs(c.table, s.value)));
                        }
                    }
                    0xE7 => {
                        c.transpose = i32::from(data.get(at + 3) as i8);
                        c.new_pan = data.get(at + 4);
                        c.pan_weight = data.get(at + 5);
                        c.reverb = data.get(at + 6);
                    }
                    0xE8 => {
                        c.transpose = i32::from(data.u8(s) as i8);
                        c.new_pan = data.u8(s);
                        c.pan_weight = data.u8(s);
                        c.reverb = data.u8(s);
                        data.u8(s);
                    }
                    0xEC => {
                        *v = Vibrato { delay: v.delay, ..Vibrato::default() };
                        c.sustain = 0;
                        c.freq = 1.0;
                    }
                    0xB2 => c.register = data.be(pairs(at, s.value)),
                    0xB4 => c.table = usize::from(c.register),
                    0xB5 => c.register = data.be(pairs(c.table, s.value)),
                    0xB6 => s.value = data.get(offset(c.table, s.value)) as i8,
                    0xBC => c.register = c.register.wrapping_add(u),
                    _ => {}
                }
            }
        }
        false
    }

    fn run(&mut self) {
        for _ in 0..STEPS {
            let at: (usize, usize) = (self.script.pc, self.script.stack.len());
            let mut s: Script = std::mem::take(&mut self.script);
            let cmd: u8 = self.data.u8(&mut s);
            if cmd >= 0xF2 {
                let before: usize = s.pc;
                let arg: u16 = self.data.arg(&mut s, self.tables.args[usize::from(cmd - 0xB0)]);
                let flow: Flow = self.data.flow(&mut s, cmd, arg);
                if matches!(flow, Flow::On) && matches!(cmd, 0xF2..=0xF5 | 0xF9..=0xFB) && s.pc < before && self.repeat.is_none() {
                    if let Some(&first) = self.visited.get(&(s.pc, s.stack.len())).filter(|f| **f + SHORTEST_LOOP <= self.now) {
                        self.repeat = Some((first, self.now));
                    }
                }
                self.script = s;
                match flow {
                    Flow::On => continue,
                    Flow::End => self.finish(),
                    Flow::Wait(n) => self.delay = n,
                }
                return;
            }
            self.visited.entry(at).or_insert(self.now);
            let stop: bool = if cmd >= 0xC0 { self.order(&mut s, cmd) } else { self.channel_order(&mut s, cmd) };
            self.script = s;
            if stop {
                self.finish();
                return;
            }
        }
        self.finish();
    }

    fn order(&mut self, s: &mut Script, cmd: u8) -> bool {
        match cmd {
            0xDF => self.transpose = i32::from(self.data.u8(s) as i8),
            0xDE => self.transpose += i32::from(self.data.u8(s) as i8),
            0xDD => self.tempo = (u32::from(self.data.u8(s)) * 48).clamp(1, TEMPO_MAX),
            0xDC => self.tempo_add = i32::from(self.data.u8(s) as i8) * 48,
            0xDA => {
                let kind: u8 = self.data.u8(s);
                let t: u32 = u32::from(self.data.u16(s));
                if kind <= 1 {
                    self.fade_next = t;
                    if kind == 1 {
                        self.fade = 0.0;
                    }
                } else if kind == 2 && t != 0 {
                    self.fade_left = t;
                    self.fade_step = -self.fade / t as f32;
                }
            }
            0xDB => {
                let v: f32 = f32::from(self.data.u8(s)) / 127.0;
                self.fade_left = self.fade_next;
                if self.fade_next != 0 {
                    self.fade_step = (v - self.fade) / self.fade_next as f32;
                } else {
                    self.fade = v;
                }
            }
            0xD9 => self.fade_scale = f32::from(self.data.u8(s) as i8) / 127.0,
            0xD7 => {
                let bits: u16 = self.data.u16(s);
                let font: u8 = self.sets.last().copied().unwrap_or(0);
                for (i, c) in self.channels.iter_mut().enumerate() {
                    if bits >> i & 1 == 1 {
                        c.font = font;
                    }
                }
            }
            0xD6 | 0xC5 => {
                self.data.u16(s);
            }
            0xD5 | 0xD3 | 0xD0 | 0xF1 => {
                self.data.u8(s);
            }
            0xD2 => self.velocity_table = Some(usize::from(self.data.u16(s))),
            0xD1 => self.gate_table = Some(usize::from(self.data.u16(s))),
            0xCE => {
                let n: u8 = self.data.u8(s);
                s.value = self.random(if n == 0 { 256 } else { u32::from(n) }) as u8 as i8;
            }
            0xCD => {
                let t: usize = usize::from(self.data.u16(s));
                if s.value != -1 && s.stack.len() < 8 {
                    s.stack.push(s.pc);
                    s.pc = usize::from(self.data.be(t.wrapping_add_signed(2 * isize::from(s.value))));
                }
            }
            0xCC => s.value = self.data.u8(s) as i8,
            0xC9 => s.value &= self.data.u8(s) as i8,
            0xC8 => s.value = s.value.wrapping_sub(self.data.u8(s) as i8),
            0xC7 => {
                let add: u8 = self.data.u8(s);
                let t: usize = usize::from(self.data.u16(s));
                self.data.set(t, (s.value as u8).wrapping_add(add));
            }
            0xC6 => return true,
            0xEF => {
                self.data.u16(s);
                self.data.u8(s);
            }
            0xC4 => {
                self.data.u16(s);
            }
            _ => {}
        }
        false
    }

    fn channel_order(&mut self, s: &mut Script, cmd: u8) -> bool {
        let low: usize = usize::from(cmd & 15);
        match cmd & 0xF0 {
            0x00 => s.value = i8::from(!self.channels[low].enabled),
            0x50 => s.value = s.value.wrapping_sub(self.io[low & 7]),
            0x70 => self.io[low & 7] = s.value,
            0x80 => {
                s.value = self.io[low & 7];
                if low < 2 {
                    self.io[low] = -1;
                }
            }
            0x40 => self.disable(low),
            0x90 => {
                let pc: usize = usize::from(self.data.u16(s));
                self.enable(low, pc);
            }
            0xA0 => {
                let rel: i16 = self.data.u16(s) as i16;
                self.enable(low, s.pc.wrapping_add_signed(isize::from(rel)) & 0xFFFF);
            }
            0xB0 => {
                self.data.u8(s);
                self.data.u16(s);
            }
            0x60 => {
                self.data.u16(s);
            }
            _ => {}
        }
        false
    }
}
