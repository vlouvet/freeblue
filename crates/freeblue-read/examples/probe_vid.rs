//! A1 experiment (spec 12 §12.15): read the AACS Volume ID off a drive, before
//! and after a LibreDrive unlock, to characterize whether an unlocked drive
//! answers READ DISC STRUCTURE `0x80` without the AKE handshake. Needs SG_IO
//! (root). Read-only; cannot harm the drive.
//!
//!   sudo target/debug/examples/probe_vid /dev/sr0

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02X}")).collect()
}

fn main() {
    let dev = std::env::args().nth(1).unwrap_or_else(|| "/dev/sr0".into());
    println!("device: {dev}");

    print!("read Volume ID, NO unlock    ... ");
    match freeblue_read::read_volume_id(&dev, false) {
        Ok(v) => println!("OK   vid={}", hex(&v)),
        Err(e) => println!("err: {e}"),
    }

    print!("read Volume ID, AFTER unlock ... ");
    match freeblue_read::read_volume_id(&dev, true) {
        Ok(v) => println!("OK   vid={}", hex(&v)),
        Err(e) => println!("err: {e}"),
    }
}
