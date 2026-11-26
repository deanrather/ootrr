use crate::byml::Byml;
use crate::slice::data::Data;

pub fn name_for(fake: &str) -> String {
    format!("{fake}_Obj")
}

pub fn lift(d: &Data, info: &Byml, look: &str, actor: &str) -> f32 {
    let tier: Option<&str> = d.item(look).is_some_and(|i| !i.tiers.is_empty()).then_some(actor);
    crate::slice::key_spots::resolve(d, look, tier).ok().and_then(|p| crate::pickup_pack::lift::for_pack(info, &p.pack)).unwrap_or(0.0)
}
