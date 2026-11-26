pub mod game;
pub mod gdm;
pub mod reader;
pub mod procmaps;
pub mod snap;
#[cfg(windows)]
pub mod win;
#[cfg(target_os = "linux")]
#[path = "linux.rs"]
pub mod win;
#[cfg(not(any(windows, target_os = "linux")))]
#[path = "nowin.rs"]
pub mod win;

#[cfg(test)]
mod tests {
    use crate::reader::Reader;
    use std::time::Instant;

    #[test]
    #[ignore]
    fn a_running_game_gives_a_snapshot() {
        let mut r: Reader = Reader::new();
        let s = r.poll(Instant::now()).unwrap();
        println!("pid {:?} rows {} keys {}", r.attached(), s.rows.len(), s.keys.len());
        assert!(s.rows.len() > 10_000);
    }
}
