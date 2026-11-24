use crate::byml::{Byml, Node};
use crate::error::{Error as E, Res};

pub const LOC: &str = "LH Sun";
pub const FLOW: &str = "SubEvent_GetFireArrow";
pub const MAP: &str = "Banc/HyruleField/HyruleField_Adult_Static.bcett.byml.zs";
pub const ARROW: u64 = 4598657944866228526;
pub const ARROW_ACTOR: &str = "Obj_Arrow_Fire";

pub const NOTE: &str = "LH Sun is shuffled: shooting the sun after the Water Temple gives the placed item right after its scene, and the Fire Arrows pickup it used to reveal never appears.";

pub fn is(name: &str) -> bool {
    name == LOC
}

pub fn hide(raw: &[u8]) -> Res<Vec<u8>> {
    let mut y: Byml = Byml::parse(raw)?;
    let list: &mut Vec<Node> = y.root.get_mut("Actors").and_then(|a| a.as_arr_mut()).ok_or(E::Format("lh sun: the Hyrule Field map has no actors"))?;
    let arrow: &mut Node = list
        .iter_mut()
        .find(|a| matches!(a.get("Hash"), Some(Node::U64(h)) if *h == ARROW))
        .ok_or(E::Format("lh sun: the Fire Arrows pickup is gone from the Hyrule Field map, the game files differ"))?;
    if arrow.get("Gyaml").and_then(|g| g.as_str()) != Some(ARROW_ACTOR) {
        return Err(E::Format("lh sun: the Fire Arrows pickup is another actor now, the game files differ"));
    }
    arrow.set(
        "Presence",
        Node::Map(vec![
            ("FlagName".to_string(), Node::Str(crate::always::NAME.to_string())),
            ("IsCheckLazy".to_string(), Node::Bool(false)),
            ("IsNegation".to_string(), Node::Bool(true)),
        ]),
    );
    y.write()
}
