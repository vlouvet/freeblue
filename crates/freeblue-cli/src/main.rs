//! `freeblue` CLI — thin front-end over `freeblue-core`/`-read` (spec 08 §8.4).
//!
//! Subcommands:
//!   freeblue unlock <device>
//!       Put a LibreDrive-capable drive into raw-read mode (spec 11 §11.4.7) so
//!       a normal OS mount / read returns content-encrypted (non-bus) sectors.
//!       Read-only — cannot harm the drive.
//!
//!   freeblue decrypt <m2ts> --unit-key <hex32> -o <out.m2ts>
//!       Decrypt a content-encrypted m2ts (e.g. a file on a mounted, unlocked
//!       disc, or a folder dump) to plaintext M2TS. The m2ts file starts on an
//!       Aligned-Unit boundary, so units are read aligned (spec 11 §11.4.6).
//!
//!   freeblue decrypt-disc <device> --start-lba N --num-units M --unit-key <hex32> -o <out>
//!       Unlock the drive, then read the extent directly over SG_IO and decrypt.
//!
//! Unit keys come from the community keydb (the `U` field) or `freeblue-disc`
//! unwrap; keydb auto-resolution by Volume ID is spec 12 §12.13 (TODO). For now
//! pass `--unit-key`.

use anyhow::{bail, Context, Result};
use freeblue_read::{ClipId, PlainUdfReader, UnitReader};
use std::io::Write;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("unlock") => cmd_unlock(&args[2..]),
        Some("decrypt") => cmd_decrypt(&args[2..]),
        Some("decrypt-disc") => cmd_decrypt_disc(&args[2..]),
        _ => {
            eprintln!("freeblue {}", env!("CARGO_PKG_VERSION"));
            eprintln!("usage:");
            eprintln!("  freeblue unlock <device>");
            eprintln!("  freeblue decrypt <m2ts> --unit-key <hex32> -o <out.m2ts>");
            eprintln!("  freeblue decrypt-disc <device> --start-lba N --num-units M --unit-key <hex32> -o <out>");
            std::process::exit(2);
        }
    }
}

/// Minimal `--flag value` option scanner.
fn opt<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .map(String::as_str)
}

fn parse_unit_key(args: &[String]) -> Result<[u8; 16]> {
    let hexk = opt(args, "--unit-key").context("--unit-key <hex32> is required")?;
    let v = hex::decode(hexk.trim_start_matches("0x")).context("--unit-key is not valid hex")?;
    if v.len() != 16 {
        bail!(
            "--unit-key must be 16 bytes (32 hex chars), got {}",
            v.len()
        );
    }
    let mut k = [0u8; 16];
    k.copy_from_slice(&v);
    Ok(k)
}

fn cmd_unlock(args: &[String]) -> Result<()> {
    let device = args.first().context("usage: freeblue unlock <device>")?;
    #[cfg(target_os = "linux")]
    {
        let ok = freeblue_read::libredrive_unlock(device)
            .with_context(|| format!("unlocking {device}"))?;
        if ok {
            println!("{device}: LibreDrive unlock OK — drive now returns raw sectors.");
            println!("Mount it read-only and `freeblue decrypt` the m2ts, or use `decrypt-disc`.");
            Ok(())
        } else {
            bail!("{device}: not a LibreDrive-capable drive (handshake mismatch); raw reads unavailable");
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = device;
        bail!("unlock needs SG_IO (Linux only)");
    }
}

/// Decrypt a content-encrypted m2ts file (PlainUdfReader) to plaintext.
fn cmd_decrypt(args: &[String]) -> Result<()> {
    let input = args
        .first()
        .context("usage: freeblue decrypt <m2ts> --unit-key <hex> -o <out>")?;
    let out = opt(args, "-o").context("-o <out.m2ts> is required")?;
    let unit_key = parse_unit_key(args)?;

    let mut reader = PlainUdfReader::new();
    let clip = ClipId::from_path(input);
    write_clip(&mut reader, &clip, unit_key, out)
}

/// Unlock a drive, then read+decrypt a disc extent directly over SG_IO.
fn cmd_decrypt_disc(args: &[String]) -> Result<()> {
    let device = args.first().context("usage: freeblue decrypt-disc <device> --start-lba N --num-units M --unit-key <hex> -o <out>")?;
    let start_lba: u64 = opt(args, "--start-lba")
        .context("--start-lba N required")?
        .parse()?;
    let num_units: u64 = opt(args, "--num-units")
        .context("--num-units M required")?
        .parse()?;
    let out = opt(args, "-o").context("-o <out> required")?;
    let unit_key = parse_unit_key(args)?;

    let mut reader = freeblue_read::LibreDriveReader::open(device);
    let clip = ClipId {
        path: None,
        disc_extent: Some((start_lba, num_units)),
    };
    write_clip(&mut reader, &clip, unit_key, out)
}

/// Drive the verified pipeline: read units via `reader`, decrypt with `unit_key`,
/// write plaintext to `out`. Reports a TS-sync sanity score on the first unit.
fn write_clip(
    reader: &mut dyn UnitReader,
    clip: &ClipId,
    unit_key: [u8; 16],
    out: &str,
) -> Result<()> {
    let mut f = std::fs::File::create(out).with_context(|| format!("creating {out}"))?;
    let units = freeblue_core::decrypt_clip(reader, clip, unit_key)
        .map_err(|e| anyhow::anyhow!("read failed: {e}"))?;
    let mut n = 0usize;
    let mut first_sync = None;
    for unit in units {
        let pt = unit.map_err(|e| anyhow::anyhow!("decrypt failed at unit {n}: {e}"))?;
        if first_sync.is_none() {
            first_sync = Some(ts_sync(&pt));
        }
        f.write_all(&pt)?;
        n += 1;
    }
    eprintln!(
        "wrote {n} Aligned Units ({} bytes) to {out}; first-unit TS-sync {}/32",
        n * 6144,
        first_sync.unwrap_or(0)
    );
    Ok(())
}

/// TS-sync sanity (a `0x47` every 192 bytes), mirroring
/// `freeblue-content::ts_sync_score` — a quick "did it decrypt?" signal.
fn ts_sync(unit: &[u8]) -> usize {
    (4..unit.len())
        .step_by(192)
        .filter(|&o| unit[o] == 0x47)
        .count()
}
