use ootrr_game::{bfres, byml, error, json, murmur, rng, romfs, rsizetable, tex, zs};

#[cfg(test)]
macro_rules! parts {
    ($run:ident, $($name:ident = $k:expr),+ $(,)?) => {
        $(
            #[test]
            fn $name() {
                $run($k)
            }
        )+
    };
}

pub mod bwav;
pub mod collapse;
pub mod colors;
pub mod cosmetic;
pub mod custom;
pub mod fanfare;
pub mod inflate;
pub mod n64render;
pub mod n64rom;
pub mod n64seq;
pub mod n64voice;
pub mod pack;
pub mod sfx;
pub mod silence;
pub mod slink;
pub mod title_logo;

#[cfg(test)]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;
