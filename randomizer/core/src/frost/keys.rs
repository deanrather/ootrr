use crate::dungeon_packs::{self as dp, Kind, Pack};
use crate::error::Res;
use crate::msbt::{self, Msbt, Tok};
use crate::rng::Rng;
use crate::slice::data::Data;

pub const SMALL: &str = "SmallKey";
pub const OLD_NAME: (&str, &str) = ("GerudoTrainingGround", "Gerudo Training Grounds");
pub const ENGLISH_ITEM: &str = "Small Koy";

pub fn is_key(d: &Data, item: &str) -> bool {
    d.item(item).is_some_and(|i| i.kind == SMALL)
}

pub fn looks(d: &Data) -> Vec<String> {
    d.items.iter().filter(|i| i.kind == SMALL && dp::of_item(i).is_some()).map(|i| i.name.clone()).collect()
}

pub fn actor(d: &Data, item: &str) -> Option<String> {
    if !is_key(d, item) {
        return None;
    }
    dp::of(d, item).map(|p| p.name())
}

pub fn pack_of(look: &str) -> Option<Pack> {
    dp::by_name(look).filter(|p| p.kind == Kind::Small)
}

pub fn name(m: &Msbt, places: &Msbt, p: Pack, english: bool, r: &mut Rng) -> Res<Vec<Tok>> {
    if english && p.dungeon == OLD_NAME.0 {
        let item: Vec<Tok> = crate::dungeon_text::name(m, &format!("{}_Name", Kind::Small.vanilla()))?;
        return dp::title(&item, &msbt::from_markup(OLD_NAME.1)?);
    }
    let item: Vec<Tok> = if english {
        msbt::from_markup(ENGLISH_ITEM)?
    } else {
        super::typo::name(&crate::dungeon_text::name(m, &format!("{}_Name", Kind::Small.vanilla()))?, r)
    };
    let place: Vec<Tok> = crate::dungeon_text::name(places, crate::dungeon_text::place_label(p.dungeon))?;
    dp::title(&item, &place)
}
