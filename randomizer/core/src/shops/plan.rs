use crate::settings::{Cond, Setting, Settings, Value};

pub const SETTING: &str = "shuffle_shops";
/// The special deal price range, named by the wallet that can pay the highest price, like the
/// shopsanity prices of the N64 randomizer. The remake's wallets hold 200, 500 and 1000 rupees.
pub const PRICES: &str = "shopsanity_prices";
/// The shop settings this one replaced (prices, price bounds and deal item classes). Their
/// numbers and ids are retired in crate::settings::list::RETIRED.
pub const OLD: [(u16, &str); 4] = [(59, "shop_prices"), (60, "shop_price_max"), (61, "shop_price_min"), (62, "shop_deal_items")];
pub const LOW: i32 = 0;
pub const HIGH: i32 = 995;
/// The lowest price of a weighted deal price above the Cheap range.
pub const FLOOR: i32 = 10;

pub const NOTE: &str = "Shop shelves are shuffled. The shop goods trade places between the eight shelf shops, each keeps its own price, and the logic needs a bigger wallet for anything above 200 rupees. Experimental, untested in the game.";
pub const DEALS: &str = "Special deals sell a random item once, through a new shelf actor per item and price. A deal can hold any item a shelf can sell, dungeon items included (the shop credits their dungeon), and songs only when songs can go anywhere. Deals take the right side of each shop first (Right Side Top Left, Top Right, Bottom Left, Bottom Right), then the left side; the shop goods of a deal shelf join the item pool (a Deku Shield or Hylian Shield as itself, ammo as its pool ammo, bottle contents as filler). The other shelves sell the goods of the same shelf position from any shop. In the Kokiri, Goron and Zora Shops, which have a keeper per age, a deal bought from one keeper is sold out at the other too. Experimental, only one Kokiri Shop deal is tested in the game.";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Off,
    Shuffle,
    Deals(u8),
    Random,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prices {
    Affordable,
    Weighted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Opts {
    pub level: Level,
    pub prices: Prices,
    pub min: i32,
    pub max: i32,
    pub kinds: Kinds,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Kinds {
    pub stacks: bool,
    pub upgrades: bool,
    pub hearts: bool,
    pub songs: bool,
}

impl Kinds {
    pub const ALL: Kinds = Kinds { stacks: true, upgrades: true, hearts: true, songs: true };
}

impl Default for Opts {
    fn default() -> Opts {
        Opts { level: Level::Off, prices: Prices::Weighted, min: 10, max: 200, kinds: Kinds::default() }
    }
}

impl Opts {
    pub fn on(&self) -> bool {
        self.level != Level::Off
    }

    pub fn deals(&self) -> bool {
        matches!(self.level, Level::Deals(_) | Level::Random)
    }

    pub fn notes(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if self.on() {
            out.push(NOTE.to_string());
            out.push(crate::shops::door::NOTE.to_string());
        }
        if self.deals() {
            out.push(DEALS.to_string());
        }
        out
    }
}

pub const DEAL_LEVELS: [&str; 5] = ["deals_1", "deals_2", "deals_3", "deals_4", "random"];

fn with_deals(s: Setting) -> Setting {
    s.dep(SETTING, Cond::In(DEAL_LEVELS.iter().map(|v| Value::s(v)).collect()))
}

pub fn settings() -> Vec<Setting> {
    vec![
        Setting::choice(58, SETTING, "shuffle", "Shopsanity", "off")
            .tip("Whether the shelves of the eight shelf shops sell shuffled goods or random items, like shopsanity in the N64 randomizer. A slot that sells a random item is a special deal. The Happy Mask Shop gets no special deals. Shop goods keep their own price wherever they go (a tunic costs 600), and a child can buy a tunic. Prices above 200 rupees need a bigger wallet in the logic. Experimental, none of these edits are tested in the game yet except one Kokiri Shop deal.")
            .opt("off", "Off", "Every shop sells its usual goods.")
            .opt("shuffle", "0 Items", "The usual shop goods move between the shops, each to the same shelf position in another shop, and keep their prices. Nothing new is sold.")
            .opt("deals_1", "1 Item", "One right side shelf in each shop is a special deal that sells a random item once, its goods join the item pool. The other shelves sell shop goods shuffled by shelf position.")
            .opt("deals_2", "2 Items", "Two right side shelves in each shop are special deals, their goods join the item pool. The other shelves sell shop goods shuffled by shelf position.")
            .opt("deals_3", "3 Items", "Three right side shelves in each shop are special deals, their goods join the item pool. The other shelves sell shop goods shuffled by shelf position.")
            .opt("deals_4", "4 Items", "The four right side shelves of each shop are special deals and their goods join the item pool. The left side sells the left side goods of every shop, each shuffled with the same shelf position elsewhere (Left Side Bottom Left with Left Side Bottom Left).")
            .opt("random", "Random", "Each shop gets 0 to 4 special deals, the Kokiri Shop at least one."),
        with_deals(
            Setting::choice(95, PRICES, "shuffle", "Shopsanity Prices", "cheap")
                .tip("The price range of special deals. The remake's wallets hold 200, 500 and 1000 rupees. Prices lean to the middle of the range, in steps of 5 rupees. Kokiri Shop deals never cost more than 200, so the Kokiri Sword and other early items stay in reach. Shop goods always keep their own price.")
                .opt("affordable", "Affordable", "Every special deal costs 10 rupees.")
                .opt("cheap", "Cheap", "0 to 95 rupees, no wallet upgrade needed.")
                .opt("adult", "Adult Wallet", "10 to 500 rupees, the logic asks for the Adult's Wallet above 200.")
                .opt("giant", "Giant's Wallet", "10 to 995 rupees, the logic asks for the Giant's Wallet above 500."),
        ),
    ]
}

fn level(v: &str) -> Option<Level> {
    match v {
        "off" => Some(Level::Off),
        "shuffle" => Some(Level::Shuffle),
        "deals_1" => Some(Level::Deals(1)),
        "deals_2" => Some(Level::Deals(2)),
        "deals_3" => Some(Level::Deals(3)),
        "deals_4" => Some(Level::Deals(4)),
        "random" => Some(Level::Random),
        _ => None,
    }
}

/// The price kind and range of a shopsanity price choice.
fn prices(v: &str) -> Option<(Prices, i32, i32)> {
    let a: i32 = crate::shops::price::AFFORDABLE;
    match v {
        "affordable" => Some((Prices::Affordable, a, a)),
        "cheap" => Some((Prices::Weighted, LOW, crate::shops::price::CHEAP)),
        "adult" => Some((Prices::Weighted, FLOOR, crate::shops::price::ADULT)),
        "giant" => Some((Prices::Weighted, FLOOR, HIGH)),
        _ => None,
    }
}

pub fn read(s: &Settings, bad: &mut Vec<String>) -> Opts {
    let mut o: Opts = Opts::default();
    match s.text(SETTING) {
        Ok(v) => match level(v) {
            Some(l) => o.level = l,
            None => bad.push(format!("{SETTING} is {v}, which is not a known choice")),
        },
        Err(e) => bad.push(e.to_string()),
    }
    if !o.deals() {
        return o;
    }
    match s.text(PRICES) {
        Ok(v) => match prices(v) {
            Some((p, min, max)) => {
                o.prices = p;
                o.min = min;
                o.max = max;
            }
            None => bad.push(format!("{PRICES} is {v}, which is not a known choice")),
        },
        Err(e) => bad.push(e.to_string()),
    }
    // Every class the shops can sell once is on, the way any pool item can stand in an N64
    // shop. Songs still need songs anywhere (shops::rules::Class::on).
    o.kinds = Kinds::ALL;
    o
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Value;

    #[test]
    fn off_by_default_and_every_level_reads() {
        let mut bad: Vec<String> = Vec::new();
        let o: Opts = read(&Settings::default(), &mut bad);
        assert!(bad.is_empty() && !o.on() && !o.deals() && o.notes().is_empty());
        let mut s: Settings = Settings::default();
        for (v, l) in [("shuffle", Level::Shuffle), ("deals_1", Level::Deals(1)), ("deals_4", Level::Deals(4)), ("random", Level::Random)] {
            s.set(SETTING, Value::s(v)).unwrap();
            let o: Opts = read(&s, &mut bad);
            assert_eq!(o.level, l);
            assert!(o.notes()[0].contains("Experimental"));
        }
        assert!(bad.is_empty(), "{bad:?}");
    }

    #[test]
    fn the_replaced_shop_settings_are_retired_and_old_strings_still_import() {
        let real: &crate::settings::Schema = crate::settings::schema();
        for (num, id) in OLD {
            assert!(real.by_num(num).is_none() && real.get(id).is_none(), "{id}");
            assert_eq!(crate::settings::list::retired_num(num), Some(id));
        }
        let j: crate::json::Json = crate::json::parse("{\"shuffle_shops\": \"deals_2\", \"shop_prices\": \"uniform\", \"shop_price_max\": 300, \"shop_deal_items\": [\"upgrades\"]}").unwrap();
        let l: crate::settings::Loaded = Settings::from_json(&j).unwrap();
        assert_eq!(l.settings.get(SETTING), Some(&Value::s("deals_2")));
        assert_eq!(l.warnings.len(), 3, "{:?}", l.warnings);
        assert!(l.warnings.iter().all(|w| w.ends_with("was removed, its value is ignored")), "{:?}", l.warnings);
    }

    #[test]
    fn prices_follow_the_wallets_and_every_class_is_on_for_deals() {
        let mut s: Settings = Settings::default();
        s.set(SETTING, Value::s("deals_2")).unwrap();
        let mut bad: Vec<String> = Vec::new();
        let o: Opts = read(&s, &mut bad);
        assert_eq!((o.prices, o.min, o.max, o.kinds), (Prices::Weighted, 0, 95, Kinds::ALL));
        for (v, p, min, max) in [("affordable", Prices::Affordable, 10, 10), ("cheap", Prices::Weighted, 0, 95), ("adult", Prices::Weighted, 10, 500), ("giant", Prices::Weighted, 10, 995)] {
            s.set(PRICES, Value::s(v)).unwrap();
            let o: Opts = read(&s, &mut bad);
            assert_eq!((o.prices, o.min, o.max), (p, min, max), "{v}");
        }
        s.set(SETTING, Value::s("shuffle")).unwrap();
        let o: Opts = read(&s, &mut bad);
        assert_eq!((o.prices, o.kinds), (Prices::Weighted, Kinds::default()));
        assert!(bad.is_empty(), "{bad:?}");
    }
}
