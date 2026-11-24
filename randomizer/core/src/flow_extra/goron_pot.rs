pub const LOC: &str = "GC Pot Freestanding PoH";
pub const FLOW: &str = "GoronCity_LargePot";
pub const ENTRY: &str = "GoronCity_LargePotRoulette_HeartPieceDrop";
pub const SCENE: &str = "Dm_Goron_0045";
/// The pot's shared roulette scene; the Heart Piece scene calls its RoulettePot and AfterDrop.
pub const ROULETTE_FLOW: &str = "Dm_Goron_0040";
pub const DROP_PRIZE: &str = "Obj_GoronCity_LargePot_A_01.EventGoronLargePotDropPrize";
pub const DROPPED: &str = "GoronCity_LargePot_HeartPieceDrop";
pub const TAKEN: &str = "GoronLargePot_IsGetHeartPiece";

pub fn is(name: &str) -> bool {
    name == LOC
}
