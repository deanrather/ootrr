use crate::n64rom::Tables;
use crate::n64seq::{Fonts, Player};

pub const RATE: usize = 32000;
const UPDATES: usize = 180;
const ECHO: usize = 3072;
const DECAY: f32 = 0.375;
const NO_LOOP_SECONDS: usize = 150;
const FANFARE_SECONDS: usize = 15;
const MOST_SECONDS: usize = 300;

pub struct Take {
    pub audio: Vec<[f32; 2]>,
    pub repeat: Option<(usize, usize)>,
}

fn sample_at(update: usize) -> usize {
    update * RATE / UPDATES
}

fn fade(audio: &mut [[f32; 2]]) {
    for (i, frame) in audio.iter_mut().rev().take(RATE).enumerate() {
        let gain: f32 = i as f32 / RATE as f32;
        frame[0] *= gain;
        frame[1] *= gain;
    }
}

pub fn render(data: &[u8], sets: &[u8], fonts: &Fonts, tables: &Tables, fanfare: bool) -> Take {
    let mut p: Player = Player::new(data, sets, fonts, tables);
    let mut audio: Vec<[f32; 2]> = Vec::new();
    let mut ring: Vec<[f32; 2]> = vec![[0.0; 2]; ECHO];
    let mut at: usize = 0;
    let mut quiet: usize = 0;
    let mut turn: Option<(usize, usize)> = None;
    for k in 0..MOST_SECONDS * UPDATES {
        p.now = k;
        let n: usize = sample_at(k + 1) - sample_at(k);
        p.update();
        let mut dry: Vec<[f32; 2]> = vec![[0.0; 2]; n];
        let mut wet: Vec<[f32; 2]> = vec![[0.0; 2]; n];
        p.mix(&mut dry, &mut wet);
        for (d, w) in dry.iter_mut().zip(wet) {
            let echo: [f32; 2] = ring[at];
            ring[at] = [echo[0] * DECAY + w[0], echo[1] * DECAY + w[1]];
            at = (at + 1) % ECHO;
            d[0] += echo[0];
            d[1] += echo[1];
        }
        audio.extend(dry);
        if p.done {
            quiet += 1;
            if quiet > UPDATES && !p.sounding() {
                break;
            }
        }
        if !fanfare && turn.is_none() {
            turn = p.repeat.map(|(first, now)| (now, now + (now - first))).filter(|t| t.1 <= MOST_SECONDS * UPDATES);
        }
        match turn {
            Some((_, end)) if k + 1 >= end => break,
            None if k + 1 >= NO_LOOP_SECONDS * UPDATES => break,
            _ => {}
        }
        if fanfare && k + 1 >= FANFARE_SECONDS * UPDATES {
            fade(&mut audio);
            break;
        }
    }
    let repeat: Option<(usize, usize)> = turn.map(|(a, b)| (sample_at(a), sample_at(b))).filter(|r| r.1 <= audio.len() && r.0 < r.1);
    if let Some((_, end)) = repeat {
        audio.truncate(end);
    }
    Take { audio, repeat }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fanfare_that_never_ends_is_cut_and_fades_out() {
        let endless: [u8; 4] = [0xFD, 0x7F, 0xFB, 0x00];
        let data: Vec<u8> = [&endless[..], &[0x00]].concat();
        let fonts: Fonts = Fonts::new();
        let mut args: [u8; 80] = [0; 80];
        args[usize::from(0xFB_u8 - 0xB0)] = 0x81;
        let tables: Tables = Tables { velocity: [0; 16], gate: [0; 16], args, envelope: vec![(-1, 0)].into() };
        let take: Take = render(&data, &[0], &fonts, &tables, true);
        assert!(take.audio.len() <= FANFARE_SECONDS * RATE);
        assert!(take.repeat.is_none());
        let mut full: Vec<[f32; 2]> = vec![[1.0, 1.0]; RATE * 2];
        fade(&mut full);
        assert_eq!(full[0], [1.0, 1.0]);
        assert_eq!(full[RATE * 2 - 1], [0.0, 0.0]);
    }
}
