use crate::rsizetable::crc32;

pub const RATE: u32 = 48000;
pub const PREFETCH: usize = 14336;
const FRAME: usize = 14;
const CHANNEL_HEAD: usize = 0x4C;
const HEAD: usize = 0x10;
const ALIGN: usize = 64;
const PAIRS: usize = 8;
const ROUNDS: usize = 5;

type Book = [(i32, i32); PAIRS];

pub fn resample(input: &[[f32; 2]], from: usize, side: usize, gain: f32) -> Vec<i16> {
    let n: usize = input.len() * RATE as usize / from;
    let get = |i: isize| -> f32 { input[i.clamp(0, input.len() as isize - 1) as usize][side] };
    (0..n)
        .map(|k| {
            let at: u64 = k as u64 * from as u64;
            let i: isize = (at / u64::from(RATE)) as isize;
            let t: f32 = (at % u64::from(RATE)) as f32 / RATE as f32;
            let (a, b, c, d): (f32, f32, f32, f32) = (get(i - 1), get(i), get(i + 1), get(i + 2));
            let v: f32 = b + 0.5 * t * (c - a + t * (2.0 * a - 5.0 * b + 4.0 * c - d + t * (3.0 * (b - c) + d - a)));
            (v * gain).round().clamp(-32768.0, 32767.0) as i16
        })
        .collect()
}

fn predictors(pcm: &[i16]) -> Book {
    let mut found: Vec<(f64, f64, f64)> = Vec::new();
    for f in pcm.chunks_exact(FRAME) {
        let mut r: [f64; 3] = [0.0; 3];
        for (lag, out) in r.iter_mut().enumerate() {
            *out = (lag..FRAME).map(|i| f64::from(f[i]) * f64::from(f[i - lag])).sum();
        }
        let det: f64 = r[0] * r[0] - r[1] * r[1];
        if det > 1.0 {
            let a: f64 = (r[1] * (r[0] - r[2])) / det;
            let b: f64 = (r[0] * r[2] - r[1] * r[1]) / det;
            found.push((a.clamp(-1.99, 1.99), b.clamp(-0.99, 0.99), r[0].sqrt()));
        }
    }
    let mut centres: [(f64, f64); PAIRS] = [(0.0, 0.0); PAIRS];
    if !found.is_empty() {
        let mut sorted: Vec<(f64, f64, f64)> = found.clone();
        sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (i, c) in centres.iter_mut().enumerate() {
            let at: (f64, f64, f64) = sorted[(2 * i + 1) * sorted.len() / (2 * PAIRS)];
            *c = (at.0, at.1);
        }
    }
    for _ in 0..ROUNDS {
        let mut sums: [(f64, f64, f64); PAIRS] = [(0.0, 0.0, 0.0); PAIRS];
        for (a, b, w) in &found {
            let far = |c: &(f64, f64)| (c.0 - a).powi(2) + (c.1 - b).powi(2);
            let best: usize = (0..PAIRS).min_by(|x, y| far(&centres[*x]).total_cmp(&far(&centres[*y]))).unwrap_or(0);
            sums[best] = (sums[best].0 + a * w, sums[best].1 + b * w, sums[best].2 + w);
        }
        for (c, s) in centres.iter_mut().zip(sums) {
            if s.2 > 0.0 {
                *c = (s.0 / s.2, s.1 / s.2);
            }
        }
    }
    centres.map(|c| ((c.0 * 2048.0).round() as i32, (c.1 * 2048.0).round() as i32))
}

fn trial(frame: &[i16], pair: (i32, i32), scale: u32, history: (i32, i32), out: &mut [u8; 8], decoded: &mut [i16; FRAME]) -> (u64, (i32, i32)) {
    let (mut h1, mut h2): (i32, i32) = history;
    let mut error: u64 = 0;
    out[1..].fill(0);
    for i in 0..FRAME {
        let want: i32 = frame.get(i).copied().map_or(0, i32::from);
        let guess: i64 = i64::from(pair.0) * i64::from(h1) + i64::from(pair.1) * i64::from(h2);
        let rest: i64 = (i64::from(want) << 11) - guess;
        let step: i64 = 2048 << scale;
        let v: i64 = ((rest + step / 2).div_euclid(step)).clamp(-8, 7);
        let got: i32 = (((v << scale) * 2048 + 1024 + guess) >> 11).clamp(-32768, 32767) as i32;
        error = error.saturating_add(u64::from(want.abs_diff(got)).pow(2));
        out[1 + i / 2] |= ((v & 15) as u8) << if i % 2 == 0 { 4 } else { 0 };
        decoded[i] = got as i16;
        h2 = h1;
        h1 = got;
    }
    (error, (h1, h2))
}

pub struct Coded {
    pub book: [i16; 16],
    pub data: Vec<u8>,
    pub decoded: Vec<i16>,
}

pub fn encode(pcm: &[i16]) -> Coded {
    let pairs: Book = predictors(pcm);
    let mut data: Vec<u8> = Vec::with_capacity(pcm.len().div_ceil(FRAME) * 8);
    let mut decoded: Vec<i16> = Vec::with_capacity(pcm.len() + FRAME);
    let mut history: (i32, i32) = (0, 0);
    for frame in pcm.chunks(FRAME) {
        let mut best: (u64, [u8; 8], [i16; FRAME], (i32, i32)) = (u64::MAX, [0; 8], [0; FRAME], history);
        for (p, pair) in pairs.iter().enumerate() {
            let mut worst: i64 = 0;
            for (i, v) in frame.iter().enumerate() {
                let before = |back: usize| -> i64 {
                    if i >= back {
                        i64::from(frame[i - back])
                    } else {
                        i64::from(if back - i == 1 { history.0 } else { history.1 })
                    }
                };
                let rest: i64 = i64::from(*v) - ((i64::from(pair.0) * before(1) + i64::from(pair.1) * before(2)) >> 11);
                worst = worst.max(rest.abs());
            }
            let mut scale: u32 = 0;
            while scale < 12 && (7i64 << scale) < worst {
                scale += 1;
            }
            for s in scale.saturating_sub(1)..=(scale + 1).min(12) {
                let mut out: [u8; 8] = [0; 8];
                let mut got: [i16; FRAME] = [0; FRAME];
                let (error, next): (u64, (i32, i32)) = trial(frame, *pair, s, history, &mut out, &mut got);
                if error < best.0 {
                    out[0] = ((p as u8) << 4) | s as u8;
                    best = (error, out, got, next);
                }
            }
        }
        data.extend(best.1);
        decoded.extend(best.2);
        history = best.3;
    }
    let mut book: [i16; 16] = [0; 16];
    for (i, p) in pairs.iter().enumerate() {
        book[2 * i] = p.0 as i16;
        book[2 * i + 1] = p.1 as i16;
    }
    decoded.truncate(pcm.len());
    Coded { book, data, decoded }
}

fn data_at(channels: usize) -> usize {
    (HEAD + CHANNEL_HEAD * channels).next_multiple_of(ALIGN)
}

pub fn write(channels: &[Coded], repeat_from: Option<usize>) -> Vec<u8> {
    let n: usize = channels.first().map_or(0, |c| c.decoded.len());
    let mut offsets: Vec<usize> = Vec::new();
    let mut at: usize = data_at(channels.len());
    for c in channels {
        offsets.push(at);
        at = (at + c.data.len()).next_multiple_of(ALIGN);
    }
    let all: Vec<u8> = channels.iter().flat_map(|c| c.data.iter().copied()).collect();
    let mut out: Vec<u8> = b"BWAV\xFF\xFE\x01\x00".to_vec();
    out.extend(crc32(&all).to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out.extend((channels.len() as u16).to_le_bytes());
    for (i, c) in channels.iter().enumerate() {
        out.extend(1u16.to_le_bytes());
        out.extend((i as u16).to_le_bytes());
        for v in [RATE, n as u32, n as u32] {
            out.extend(v.to_le_bytes());
        }
        for v in c.book {
            out.extend(v.to_le_bytes());
        }
        let end: u32 = repeat_from.map_or(u32::MAX, |_| n as u32);
        for v in [offsets[i] as u32, offsets[i] as u32, 1, end, repeat_from.unwrap_or(0) as u32] {
            out.extend(v.to_le_bytes());
        }
        let (ps, h1, h2): (u16, i16, i16) = repeat_from.map_or((0, 0, 0), |from| {
            let before = |back: usize| from.checked_sub(back).and_then(|x| c.decoded.get(x)).copied().unwrap_or(0);
            (u16::from(c.data.get(from / FRAME * 8).copied().unwrap_or(0)), before(1), before(2))
        });
        out.extend(ps.to_le_bytes());
        out.extend(h1.to_le_bytes());
        out.extend(h2.to_le_bytes());
        out.extend(0u16.to_le_bytes());
    }
    for (c, off) in channels.iter().zip(offsets) {
        out.resize(off, 0);
        out.extend_from_slice(&c.data);
    }
    out
}

fn le32(b: &[u8], at: usize) -> usize {
    b.get(at..at + 4).map_or(0, |w| u32::from_le_bytes([w[0], w[1], w[2], w[3]]) as usize)
}

pub fn bank_copy(stream: &[u8], prefetch: bool) -> Option<Vec<u8>> {
    let channels: usize = usize::from(*stream.get(0xE)?);
    if !prefetch {
        return stream.get(..HEAD + CHANNEL_HEAD * channels).map(|h| h.to_vec());
    }
    let size: usize = PREFETCH / FRAME * 8;
    let mut out: Vec<u8> = stream.get(..HEAD + CHANNEL_HEAD * channels)?.to_vec();
    out[0xC] = 1;
    let mut at: usize = data_at(channels);
    for c in 0..channels {
        let head: usize = HEAD + CHANNEL_HEAD * c;
        let from: usize = le32(stream, head + 0x30);
        out[head + 12..head + 16].copy_from_slice(&(PREFETCH as u32).to_le_bytes());
        out[head + 0x34..head + 0x38].copy_from_slice(&(at as u32).to_le_bytes());
        out.resize(at, 0);
        out.extend_from_slice(stream.get(from..from + size)?);
        at = (at + size).next_multiple_of(ALIGN);
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::n64rom::decode;

    fn wave(n: usize) -> Vec<i16> {
        (0..n).map(|i| ((i as f32 * 0.07).sin() * 9000.0 + (i as f32 * 0.31).sin() * 2000.0) as i16).collect()
    }

    #[test]
    fn coded_sound_stays_close_to_the_original() {
        let pcm: Vec<i16> = wave(4800);
        let c: Coded = encode(&pcm);
        assert_eq!(c.data.len(), pcm.len().div_ceil(FRAME) * 8);
        let noise: f64 = pcm.iter().zip(&c.decoded).map(|(a, b)| f64::from(a.abs_diff(*b)).powi(2)).sum();
        let signal: f64 = pcm.iter().map(|a| f64::from(*a).powi(2)).sum();
        assert!(10.0 * (signal / noise).log10() > 35.0, "{}", 10.0 * (signal / noise).log10());
        let silent: Vec<f32> = decode(&[0u8; 9], false, &[0; 16]);
        assert_eq!(silent.len(), 16);
    }

    #[test]
    fn the_header_carries_the_loop_and_the_bank_copy_its_opening_samples() {
        let sides: [Coded; 2] = [encode(&wave(20000)), encode(&wave(20000))];
        let b: Vec<u8> = write(&sides, Some(1000));
        assert_eq!((&b[..4], b[0xE], le32(&b, 0x10 + 8), le32(&b, 0x4C), le32(&b, 0x50)), (&b"BWAV"[..], 2, 20000, 20000, 1000));
        assert_eq!(le32(&b, 0x40) % ALIGN, 0);
        assert_eq!(bank_copy(&b, false).unwrap(), b[..0xA8].to_vec());
        let p: Vec<u8> = bank_copy(&b, true).unwrap();
        assert_eq!((p.len(), p[0xC], le32(&p, 0x10 + 12), le32(&p, 0x44), le32(&p, 0x5C + 0x34)), (16576, 1, PREFETCH, 192, 8384));
        assert_eq!(write(&sides, None)[0x4C..0x50], [0xFF; 4]);
    }
}
