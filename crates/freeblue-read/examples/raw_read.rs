//! Read raw (content-encrypted, non-bus) Aligned Units from a LibreDrive disc —
//! NO MakeMKV. Replays the LibreDrive unlock over SG_IO, then READ(10)s a clip
//! extent. Usage: raw_read <device> <start_lba> <num_units> <out_file>
//!   sudo cargo run -p freeblue-read --example raw_read -- /dev/sr0 7629550 8 /tmp/u.bin
use freeblue_read::{ClipId, LibreDriveReader, UnitReader};
use std::io::Write;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let a: Vec<String> = std::env::args().collect();
    if a.len() != 5 {
        eprintln!("usage: raw_read <device> <start_lba> <num_units> <out_file>");
        std::process::exit(2);
    }
    let (dev, start, n, out) = (&a[1], a[2].parse::<u64>()?, a[3].parse::<u64>()?, &a[4]);
    let mut r = LibreDriveReader::open(dev);
    let clip = ClipId { path: None, disc_extent: Some((start, n)) };
    let mut f = std::fs::File::create(out)?;
    let mut count = 0usize;
    for unit in r.read_units(&clip)? {
        f.write_all(&unit?)?;
        count += 1;
    }
    eprintln!("wrote {count} units ({} bytes) from lba {start} to {out}", count * 6144);
    Ok(())
}
