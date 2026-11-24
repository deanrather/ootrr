use crate::rng::Rng;
use crate::shops::plan::{Opts, Prices, HIGH, LOW};

pub const STEP: i32 = 5;
pub const CHILD: i32 = 200;
pub const ADULT: i32 = 500;
pub const GIANT: i32 = 1000;
pub const AFFORDABLE: i32 = 10;
/// The top of the Cheap price range, as in the N64 randomizers' cheap shop prices.
pub const CHEAP: i32 = 95;

pub fn wallets(price: i32) -> u8 {
    if price > ADULT {
        2
    } else if price > CHILD {
        1
    } else {
        0
    }
}

pub fn rule(price: i32) -> Option<&'static str> {
    match wallets(price) {
        0 => None,
        1 => Some("Progressive_Wallet"),
        _ => Some("(Progressive_Wallet, 2)"),
    }
}

fn bounds(o: &Opts) -> (i32, i32) {
    let lo: i32 = ((o.min.clamp(LOW, HIGH) + STEP - 1) / STEP) * STEP;
    let hi: i32 = (o.max.clamp(LOW, HIGH) / STEP) * STEP;
    (lo, hi.max(lo))
}

fn uniform(rng: &mut Rng, lo: i32, hi: i32) -> i32 {
    let n: u64 = ((hi - lo) / STEP + 1) as u64;
    lo + STEP * rng.below(n) as i32
}

pub fn draw(o: &Opts, rng: &mut Rng) -> i32 {
    let (lo, hi): (i32, i32) = bounds(o);
    match o.prices {
        Prices::Affordable => AFFORDABLE,
        Prices::Weighted => {
            let a: i32 = uniform(rng, lo, hi);
            let b: i32 = uniform(rng, lo, hi);
            ((a + b) / 2 / STEP) * STEP
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shops::plan::Level;

    fn o(p: Prices, min: i32, max: i32) -> Opts {
        Opts { level: Level::Deals(1), prices: p, min, max, kinds: crate::shops::plan::Kinds::default() }
    }

    #[test]
    fn the_remake_wallets_set_the_rule() {
        assert_eq!(rule(5), None);
        assert_eq!(rule(200), None);
        assert_eq!(rule(205), Some("Progressive_Wallet"));
        assert_eq!(rule(500), Some("Progressive_Wallet"));
        assert_eq!(rule(600), Some("(Progressive_Wallet, 2)"));
        assert_eq!(rule(995), Some("(Progressive_Wallet, 2)"));
        assert!(995 < GIANT);
    }

    #[test]
    fn draws_stay_in_range_in_steps_of_five_and_repeat_for_a_seed() {
        for p in [Prices::Weighted] {
            let mut a: Rng = Rng::from_text("x/shops");
            let mut b: Rng = Rng::from_text("x/shops");
            let mut seen: Vec<i32> = Vec::new();
            for _ in 0..400 {
                let v: i32 = draw(&o(p, 12, 203), &mut a);
                assert_eq!(v, draw(&o(p, 12, 203), &mut b));
                assert!((15..=200).contains(&v) && v % 5 == 0, "{v}");
                seen.push(v);
            }
            seen.sort_unstable();
            seen.dedup();
            assert!(seen.len() > 10);
        }
        let mut r: Rng = Rng::from_text("y");
        assert_eq!(draw(&o(Prices::Affordable, 50, 90), &mut r), 10);
        assert_eq!(draw(&o(Prices::Weighted, 50, 50), &mut r), 50);
    }

    #[test]
    fn weighted_leans_to_the_middle() {
        let mut r: Rng = Rng::from_text("lean");
        let mut mid: usize = 0;
        for _ in 0..1000 {
            let v: i32 = draw(&o(Prices::Weighted, 10, 400), &mut r);
            if (110..=300).contains(&v) {
                mid += 1;
            }
        }
        assert!(mid > 600, "{mid}");
    }
}
