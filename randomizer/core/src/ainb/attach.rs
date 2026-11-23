use super::rd::Rd;
use super::tab::PSZ;
use super::Ainb;
use crate::error::{Error, Res};
use crate::murmur::murmur3_32;

pub const ENTRY: usize = 0x10;
pub const BLOCK: usize = 0x64;
pub const COUNT: usize = 4;
pub const BASE: usize = 0x20;
const STR: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct Att {
    pub name: u32,
    pub hash: u32,
    pub exb: [u16; 2],
    pub flags: u32,
    pub pe: [[u32; 2]; 6],
    pub tail: [u32; 6],
}

impl Att {
    pub fn parse(r: &Rd, o: usize, at: usize) -> Res<Att> {
        if r.ptr(o + 4)? != at {
            return Err(Error::Format("ainb: attachment parameters out of order"));
        }
        let mut pe: [[u32; 2]; 6] = [[0; 2]; 6];
        let mut tail: [u32; 6] = [0; 6];
        for k in 0..6 {
            pe[k] = [r.u32(at + 4 + 8 * k)?, r.u32(at + 8 + 8 * k)?];
            tail[k] = r.u32(at + 0x34 + 8 * k)?;
            if r.ptr(at + 0x38 + 8 * k)? != at + BLOCK {
                return Err(Error::Format("ainb: attachment parameter data is not supported"));
            }
        }
        Ok(Att { name: r.u32(o)?, hash: r.u32(o + 0xc)?, exb: [r.u16(o + 8)?, r.u16(o + 10)?], flags: r.u32(at)?, pe, tail })
    }

    pub fn entry(&self, at: usize, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.name.to_le_bytes());
        out.extend_from_slice(&(at as u32).to_le_bytes());
        out.extend_from_slice(&self.exb[0].to_le_bytes());
        out.extend_from_slice(&self.exb[1].to_le_bytes());
        out.extend_from_slice(&self.hash.to_le_bytes());
    }

    pub fn block(&self, at: usize, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.flags.to_le_bytes());
        for k in 0..6 {
            out.extend_from_slice(&self.pe[k][0].to_le_bytes());
            out.extend_from_slice(&self.pe[k][1].to_le_bytes());
        }
        for k in 0..6 {
            out.extend_from_slice(&self.tail[k].to_le_bytes());
            out.extend_from_slice(&((at + BLOCK) as u32).to_le_bytes());
        }
    }
}

fn r16(e: &[u8], o: usize) -> usize {
    u16::from_le_bytes([e[o], e[o + 1]]) as usize
}

fn r32(e: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([e[o], e[o + 1], e[o + 2], e[o + 3]])
}

impl Ainb {
    pub fn att_range(&self, i: usize) -> (usize, usize) {
        let e: &[u8] = &self.els[i];
        (r32(e, BASE) as usize, r16(e, COUNT))
    }

    pub fn el_atts(&self, i: usize) -> Vec<usize> {
        let (b, n): (usize, usize) = self.att_range(i);
        if n == 0 {
            return Vec::new();
        }
        self.att_ids.get(b..b + n).map(|v| v.iter().map(|&x| x as usize).collect()).unwrap_or_default()
    }

    pub fn att_name(&self, j: usize) -> &str {
        self.atts.get(j).map_or("?", |t| self.s(t.name))
    }

    pub fn find_atts(&self, name: &str) -> Vec<usize> {
        (0..self.atts.len()).filter(|&j| self.att_name(j) == name).collect()
    }

    pub fn node_props_end(&self, k: usize) -> usize {
        let held: usize = self.atts.iter().map(|t| t.pe[k][1] as usize).sum();
        (self.props[k].len() / PSZ[k]).saturating_sub(held)
    }

    pub fn put_props(&mut self, k: usize, d: &[u8]) -> Res<u32> {
        if k >= 6 || d.len() % PSZ[k] != 0 {
            return Err(Error::Format("ainb: bad property records"));
        }
        let at: usize = self.node_props_end(k);
        let n: u32 = (d.len() / PSZ[k]) as u32;
        let o: usize = at * PSZ[k];
        self.props[k].splice(o..o, d.iter().copied());
        for t in self.atts.iter_mut() {
            if t.pe[k][0] as usize >= at {
                t.pe[k][0] += n;
            }
        }
        Ok(at as u32)
    }

    pub fn copy_att(&mut self, src: &Ainb, j: usize) -> Res<usize> {
        let t: &Att = src.atts.get(j).ok_or(Error::Format("ainb: no such attachment"))?;
        let nm: &str = src.pool.get(t.name).ok_or(Error::Format("ainb: attachment name outside the pool"))?;
        let mut c: Att = t.clone();
        c.name = self.pool.add(nm);
        for k in 0..6 {
            let (b, n): (usize, usize) = (t.pe[k][0] as usize, t.pe[k][1] as usize);
            let mut d: Vec<u8> = src.props[k].get(b * PSZ[k]..(b + n) * PSZ[k]).ok_or(Error::Format("ainb: attachment property range out of bounds"))?.to_vec();
            for r in d.chunks_mut(PSZ[k]) {
                let pn: &str = src.pool.get(r32(r, 0)).ok_or(Error::Format("ainb: property name outside the pool"))?;
                r[0..4].copy_from_slice(&self.pool.add(pn).to_le_bytes());
                if k == STR {
                    let pv: &str = src.pool.get(r32(r, 8)).ok_or(Error::Format("ainb: property value outside the pool"))?;
                    r[8..12].copy_from_slice(&self.pool.add(pv).to_le_bytes());
                }
            }
            c.pe[k][0] = (self.props[k].len() / PSZ[k]) as u32;
            self.props[k].extend(d);
        }
        self.atts.push(c);
        Ok(self.atts.len() - 1)
    }

    pub fn new_att(&mut self, name: &str, flags: u32) -> usize {
        let nm: u32 = self.pool.add(name);
        let mut pe: [[u32; 2]; 6] = [[0; 2]; 6];
        for k in 0..6 {
            pe[k][0] = (self.props[k].len() / PSZ[k]) as u32;
        }
        self.atts.push(Att { name: nm, hash: murmur3_32(name.as_bytes(), 0), exb: [0; 2], flags, pe, tail: [0; 6] });
        self.atts.len() - 1
    }

    pub fn set_el_atts(&mut self, i: usize, ids: &[usize]) -> Res<()> {
        if i >= self.els.len() || ids.len() > 0xffff || ids.iter().any(|&j| j >= self.atts.len()) {
            return Err(Error::Format("ainb: bad attachment list"));
        }
        let (_, old): (usize, usize) = self.att_range(i);
        if old != 0 {
            return Err(Error::Format("ainb: the element already has attachments"));
        }
        if ids.is_empty() {
            return Ok(());
        }
        let later: bool = (i + 1..self.els.len()).any(|x| self.att_range(x).1 > 0);
        if later {
            return Err(Error::Format("ainb: a later element already has attachments"));
        }
        let b: usize = self.att_ids.len();
        self.att_ids.extend(ids.iter().map(|&j| j as u32));
        let e: &mut [u8] = &mut self.els[i];
        e[COUNT..COUNT + 2].copy_from_slice(&(ids.len() as u16).to_le_bytes());
        e[BASE..BASE + 4].copy_from_slice(&(b as u32).to_le_bytes());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::synth;
    use super::super::audit;
    use super::*;

    #[test]
    fn an_attachment_round_trips_and_new_node_properties_go_before_it() {
        let mut a: Ainb = synth();
        let j: usize = a.new_att("LookIKControllerArbiterRequestPause", 0);
        let nm: u32 = a.pool.add("ControllerName");
        let v: u32 = a.pool.add("Head");
        let mut r: Vec<u8> = Vec::new();
        r.extend_from_slice(&nm.to_le_bytes());
        r.extend_from_slice(&0u32.to_le_bytes());
        r.extend_from_slice(&v.to_le_bytes());
        a.atts[j].pe[STR] = [(a.props[STR].len() / PSZ[STR]) as u32, 1];
        a.props[STR].extend(r);
        a.set_el_atts(1, &[j]).unwrap();
        assert!(audit(&a).is_empty(), "{:?}", audit(&a));
        let b: Vec<u8> = a.write().unwrap();
        let c: Ainb = Ainb::parse(&b).unwrap();
        assert_eq!((&c.atts, &c.att_ids, &c.params, &c.props), (&a.atts, &a.att_ids, &a.params, &a.props));
        assert_eq!(c.write().unwrap(), b);
        assert_eq!(c.el_atts(1), vec![j]);
        assert_eq!(c.att_name(j), "LookIKControllerArbiterRequestPause");
        let before: usize = c.atts[j].pe[STR][0] as usize;
        let mut d: Ainb = c.clone();
        let at: u32 = d.put_props(STR, &d.props[STR][..PSZ[STR]].to_vec()).unwrap();
        assert_eq!(at as usize, before);
        assert_eq!(d.atts[j].pe[STR][0] as usize, before + 1);
        assert!(d.set_el_atts(1, &[j]).is_err());
        assert!(d.set_el_atts(0, &[9]).is_err());
    }
}
