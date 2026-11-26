use crate::msbt::Tok;
use crate::rng::Rng;

pub const OLD_NAMES: [(&str, &str); 1] = [("Item_GlassTruth", "Lens of Truth")];

pub fn old_name(fake: &str) -> Option<&'static str> {
    OLD_NAMES.iter().find(|(a, _)| super::fake_name(a) == fake).map(|(_, n)| *n)
}

fn inner(c: &[char], i: usize) -> bool {
    i > 0 && i + 1 < c.len() && c[i - 1].is_alphabetic() && c[i].is_alphabetic() && c[i + 1].is_alphabetic()
}

pub fn word(s: &str, r: &mut Rng) -> String {
    let c: Vec<char> = s.chars().collect();
    let spots: Vec<usize> = (0..c.len()).filter(|i| inner(&c, *i)).collect();
    if spots.is_empty() {
        return s.to_string();
    }
    let i: usize = spots[r.below(spots.len() as u64) as usize];
    let mut out: Vec<char> = c.clone();
    match r.below(3) {
        0 if c[i] != c[i + 1] => out.swap(i, i + 1),
        1 if c[i - 1] != c[i] && c[i] != c[i + 1] => {
            out.remove(i);
        }
        _ => out.insert(i, c[i]),
    }
    out.into_iter().collect()
}

pub fn name(t: &[Tok], r: &mut Rng) -> Vec<Tok> {
    let mut out: Vec<Tok> = t.to_vec();
    let at: Option<usize> = out
        .iter()
        .enumerate()
        .filter_map(|(i, x)| match x {
            Tok::Text(s) => Some((i, s.chars().filter(|c| c.is_alphabetic()).count())),
            _ => None,
        })
        .max_by_key(|x| x.1)
        .map(|x| x.0);
    if let Some(i) = at
        && let Tok::Text(s) = &out[i]
    {
        out[i] = Tok::Text(word(s, r));
    }
    out
}
