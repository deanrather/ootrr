use crate::error::{Error as E, Res};

const LEN_BASE: [usize; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u32; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [usize; 30] = [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DIST_EXTRA: [u32; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];
const BAD: E = E::Format("custom music: a compressed block in the pack is damaged");

struct Bits<'a> {
    b: &'a [u8],
    at: usize,
}

impl Bits<'_> {
    fn bit(&mut self) -> Res<u32> {
        let byte: u8 = *self.b.get(self.at / 8).ok_or(BAD)?;
        let out: u32 = u32::from(byte >> (self.at % 8)) & 1;
        self.at += 1;
        Ok(out)
    }

    fn take(&mut self, n: u32) -> Res<usize> {
        let mut out: usize = 0;
        for i in 0..n {
            out |= (self.bit()? as usize) << i;
        }
        Ok(out)
    }
}

struct Huff {
    count: [u16; 16],
    symbol: Vec<u16>,
}

impl Huff {
    fn new(lengths: &[u8]) -> Huff {
        let mut count: [u16; 16] = [0; 16];
        for l in lengths {
            count[usize::from(*l & 15)] += 1;
        }
        count[0] = 0;
        let mut symbol: Vec<u16> = Vec::with_capacity(lengths.len());
        for bits in 1..16u8 {
            symbol.extend(lengths.iter().enumerate().filter(|(_, l)| **l == bits).map(|(i, _)| i as u16));
        }
        Huff { count, symbol }
    }

    fn read(&self, bits: &mut Bits) -> Res<u16> {
        let mut code: usize = 0;
        let mut first: usize = 0;
        let mut index: usize = 0;
        for len in 1..16 {
            code |= bits.bit()? as usize;
            let n: usize = usize::from(self.count[len]);
            if code < first + n {
                return self.symbol.get(index + code - first).copied().ok_or(BAD);
            }
            index += n;
            first = (first + n) << 1;
            code <<= 1;
        }
        Err(BAD)
    }
}

fn tables(bits: &mut Bits, kind: usize) -> Res<(Huff, Huff)> {
    if kind == 1 {
        let mut lit: [u8; 288] = [8; 288];
        lit[144..256].fill(9);
        lit[256..280].fill(7);
        return Ok((Huff::new(&lit), Huff::new(&[5; 30])));
    }
    let nlit: usize = bits.take(5)? + 257;
    let ndist: usize = bits.take(5)? + 1;
    let ncode: usize = bits.take(4)? + 4;
    let mut code: [u8; 19] = [0; 19];
    for &i in ORDER.iter().take(ncode) {
        code[i] = bits.take(3)? as u8;
    }
    let meta: Huff = Huff::new(&code);
    let mut all: Vec<u8> = Vec::new();
    while all.len() < nlit + ndist {
        let s: u16 = meta.read(bits)?;
        let (value, times): (u8, usize) = match s {
            0..=15 => (s as u8, 1),
            16 => (*all.last().ok_or(BAD)?, bits.take(2)? + 3),
            17 => (0, bits.take(3)? + 3),
            _ => (0, bits.take(7)? + 11),
        };
        all.extend(std::iter::repeat_n(value, times));
    }
    if all.len() != nlit + ndist {
        return Err(BAD);
    }
    Ok((Huff::new(&all[..nlit]), Huff::new(&all[nlit..])))
}

pub fn zlib(b: &[u8], limit: usize) -> Res<Vec<u8>> {
    let mut bits: Bits = Bits { b: b.get(2..).ok_or(BAD)?, at: 0 };
    let mut out: Vec<u8> = Vec::new();
    loop {
        let last: u32 = bits.bit()?;
        let kind: usize = bits.take(2)?;
        match kind {
            0 => {
                bits.at = bits.at.div_ceil(8) * 8;
                let n: usize = bits.take(16)?;
                bits.take(16)?;
                let from: usize = bits.at / 8;
                out.extend_from_slice(bits.b.get(from..from + n).ok_or(BAD)?);
                bits.at += n * 8;
            }
            1 | 2 => {
                let (lit, dist): (Huff, Huff) = tables(&mut bits, kind)?;
                loop {
                    let s: usize = usize::from(lit.read(&mut bits)?);
                    if s < 256 {
                        out.push(s as u8);
                    } else if s == 256 {
                        break;
                    } else {
                        let i: usize = s - 257;
                        let len: usize = *LEN_BASE.get(i).ok_or(BAD)? + bits.take(LEN_EXTRA[i])?;
                        let d: usize = usize::from(dist.read(&mut bits)?);
                        let back: usize = *DIST_BASE.get(d).ok_or(BAD)? + bits.take(DIST_EXTRA[d])?;
                        let start: usize = out.len().checked_sub(back).ok_or(BAD)?;
                        for k in 0..len {
                            let v: u8 = out[start + k];
                            out.push(v);
                        }
                    }
                    if out.len() > limit {
                        return Err(BAD);
                    }
                }
            }
            _ => return Err(BAD),
        }
        if out.len() > limit {
            return Err(BAD);
        }
        if last == 1 {
            return Ok(out);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fixed_code_block_and_a_stored_block_unpack_and_a_cut_one_is_refused() {
        let fixed: [u8; 16] = [0x78, 0xda, 0xcb, 0x48, 0xcd, 0xc9, 0xc9, 0x57, 0xc8, 0x00, 0x91, 0x00, 0x19, 0x91, 0x04, 0x49];
        assert_eq!(zlib(&fixed, 64).unwrap(), b"hello hello".to_vec());
        let stored: [u8; 10] = [0x78, 0x01, 0x01, 0x03, 0x00, 0xfc, 0xff, b'a', b'b', b'c'];
        assert_eq!(zlib(&stored, 64).unwrap(), b"abc".to_vec());
        assert!(zlib(&fixed[..5], 64).is_err());
        assert!(zlib(&stored, 2).is_err());
    }
}
