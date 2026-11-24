use crate::error::Res;
use crate::shops::data::{is_loc, table, Table};
use crate::shops::layout::Room;
use crate::shops::plan::Opts;
use crate::slice::data::Data;
use crate::slice::support::Plan;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Class {
    Unique,
    Stack,
    Upgrade,
    Heart,
    Song,
}

impl Class {
    pub fn once_only(self) -> bool {
        matches!(self, Class::Stack | Class::Heart)
    }

    pub fn on(self, o: &Opts, songs: bool) -> bool {
        match self {
            Class::Unique => true,
            Class::Stack => o.kinds.stacks,
            Class::Upgrade => o.kinds.upgrades,
            Class::Heart => o.kinds.hearts,
            Class::Song => o.kinds.songs && songs,
        }
    }
}

/// Item kinds a shelf deal does not sell: shop goods (they move between shelves as goods), event
/// items and the Treasure Chest Game keys. A dungeon item sells as its own item and the shop
/// credits its dungeon after the sale (crate::shelf::hook::Credit).
pub const NOT_SOLD: [&str; 3] = ["TCGSmallKey", "Shop", "Event"];

/// How a shelf deal sells `item`, or None when no shelf can sell it.
pub fn class(d: &Data, item: &str) -> Option<Class> {
    let t: &Table = table().ok()?;
    if t.stack.iter().any(|(i, _)| i == item) {
        return Some(Class::Stack);
    }
    if t.hearts.iter().any(|i| i == item) {
        return Some(Class::Heart);
    }
    if d.family(item).is_some_and(|f| t.upgrades.iter().any(|u| u == f)) {
        return Some(Class::Upgrade);
    }
    let i = d.item(item)?;
    if i.kind == "Song" {
        return i.actor.as_deref().is_some_and(crate::slice::generate::is_song).then_some(Class::Song);
    }
    if NOT_SOLD.contains(&i.kind.as_str()) || item.contains("(Treasure Chest Game)") || !i.tiers.is_empty() {
        return None;
    }
    if crate::slice::pickups::WORLD_ONLY.contains(&item) || d.pots.only.contains(item) {
        return None;
    }
    i.actor.is_some().then_some(Class::Unique)
}

pub fn shelf_ok(d: &Data, item: &str) -> bool {
    class(d, item).is_some()
}

/// Whether the shop slot `name` may hold `item`. Every deal slot sells once, in every shop: the
/// one-keeper shops close a slot after a sale, and in the shops with a keeper per age
/// shelf::hook's sold sync closes it for every keeper.
pub fn at(d: &Data, o: &Opts, songs: bool, name: &str, item: &str) -> Option<bool> {
    if !o.on() || !is_loc(name) {
        return None;
    }
    if !o.deals() {
        return Some(false);
    }
    Some(class(d, item).is_some_and(|c| c.on(o, songs)))
}

/// Deals no longer compete for a short list of shelf items, so every deal slot of the level stays.
pub fn room(_d: &Data, p: &Plan) -> Res<Room> {
    if !p.shops.deals() {
        return Ok(Room::default());
    }
    Ok(Room { any: usize::MAX, once: usize::MAX })
}

pub fn cut_note(cut: usize, room: Room) -> Option<String> {
    (cut > 0).then(|| format!("{cut} special deals stay shop goods. Only {} deals fit this seed, so that a few of the items that can stand on a shelf stay free for other spots.", room.any.saturating_add(room.once)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shops::plan::{Kinds, Level, Prices};

    #[test]
    fn deal_slots_take_any_item_a_shelf_can_sell() {
        let d: Data = Data::load().unwrap();
        let off: Opts = Opts::default();
        let on: Opts = Opts { level: Level::Deals(2), prices: Prices::Weighted, min: 0, max: 95, kinds: Kinds::ALL };
        let plain: Opts = Opts { level: Level::Shuffle, ..on };
        assert_eq!(at(&d, &off, true, "KF Shop Right Side Bottom Left", "Boomerang"), None);
        assert_eq!(at(&d, &on, true, "KF Midos Top Left Chest", "Boomerang"), None);
        assert_eq!(at(&d, &plain, true, "KF Shop Right Side Bottom Left", "Boomerang"), Some(false));
        for shop in ["GC Shop Left Side Bottom Right", "KF Shop Right Side Top Right", "Market Bazaar Right Side Bottom Left"] {
            for good in ["Boomerang", "Progressive Hookshot", "Bow", "Slingshot", "Bomb Bag", "Magic Meter", "Piece of Heart", "Bombchus (10)", "Rupees (20)", "Bottle", "Bombs (10)", "Kokiri Emerald", "Recovery Heart"] {
                assert_eq!(at(&d, &on, true, shop, good), Some(true), "{shop} {good}");
            }
            for good in ["Small Key (Forest Temple)", "Boss Key (Forest Temple)", "Map (Forest Temple)", "Compass (Forest Temple)"] {
                assert_eq!(at(&d, &on, true, shop, good), Some(true), "{shop} {good}");
            }
            // Shop goods, event items, the Treasure Chest Game's own items and items without a
            // remake actor (NOT_SOLD) never stand on a deal.
            for bad in ["Buy Heart", "Scarecrow Song", "Small Key (Treasure Chest Game)", "Piece of Heart (Treasure Chest Game)", "Ice Trap"] {
                assert_eq!(at(&d, &on, true, shop, bad), Some(false), "{shop} {bad}");
            }
            assert_eq!(at(&d, &on, true, shop, "Minuet of Forest"), Some(true));
            assert_eq!(at(&d, &on, false, shop, "Minuet of Forest"), Some(false));
        }
        assert_eq!(class(&d, "Progressive Wallet"), Some(Class::Upgrade));
        assert_eq!(class(&d, "Bow"), Some(Class::Upgrade));
        assert_eq!(class(&d, "Recovery Heart"), Some(Class::Stack));
        assert_eq!(class(&d, "Rupees (20)"), Some(Class::Unique));
    }
}