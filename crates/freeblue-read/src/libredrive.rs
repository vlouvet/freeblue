//! LibreDrive raw-read path (spec 11 §11.4.6–§11.4.7) — **no MakeMKV**.
//!
//! A LibreDrive-capable drive returns *bus-encrypted* content on a cold read, but
//! after a fixed, **read-only** sequence of `READ BUFFER` (`0x3C`) commands it
//! returns **raw** (content-encrypted, non-bus) sectors — exactly what the decrypt
//! core wants. We reverse-engineered that sequence by observing MakeMKV (SCSI CDBs
//! are protocol facts, not copyrightable expression — spec 11 §11.6) and replay it
//! ourselves. Proven on the LG WH16NS60: a cold disc that decrypts 2/32 jumps to
//! 32/32 after replaying [`LIBREDRIVE_UNLOCK_WH16NS60`].

#![cfg(target_os = "linux")]

use crate::libredrive_unlock::LIBREDRIVE_UNLOCK_WH16NS60;
use crate::scsi::ScsiDev;
use crate::ALIGNED_UNIT_LEN;
use std::io;

/// Optical logical-block size.
pub const SECTOR_LEN: usize = 2048;
/// An Aligned Unit is 3 sectors (spec 05 §5.1).
pub const SECTORS_PER_UNIT: u32 = (ALIGNED_UNIT_LEN / SECTOR_LEN) as u32;
/// READ(10) batch size, in units (kept a whole number of units so every batch
/// boundary is also an Aligned-Unit boundary — spec 12 §12.5).
const BATCH_UNITS: u32 = 16;

/// Put the drive into raw-read mode by replaying the LibreDrive unlock sequence.
///
/// Returns `Ok(true)` if the drive answered the handshake with the LibreDrive
/// signature (`MMkv`), `Ok(false)` if it's a normal (non-LibreDrive) drive — in
/// which case raw reads won't work and the caller should not trust the content.
pub fn unlock(dev: &ScsiDev) -> io::Result<bool> {
    let mut libredrive = false;
    for (i, &(offset, len)) in LIBREDRIVE_UNLOCK_WH16NS60.iter().enumerate() {
        let resp = dev.read_buffer(2, 0x77, offset, len)?;
        // The first command is the 64-byte handshake; it carries ASCII "MMkv"
        // on a LibreDrive-capable drive.
        if i == 0 {
            libredrive = resp.windows(4).any(|w| w == b"MMkv");
        }
    }
    Ok(libredrive)
}

/// Streams clip-LBA-aligned 6144-byte Aligned Units from a `disc_extent`.
///
/// Reads start at the clip's first LBA — which **is** an Aligned-Unit boundary —
/// and advance a whole number of units per READ(10), so units never straddle a
/// read boundary (spec 12 §12.5). Assumes [`unlock`] already ran on `dev`.
pub struct RawUnitIter {
    dev: ScsiDev,
    next_lba: u32,
    units_left: u64,
    batch: Vec<u8>,
    batch_pos: usize,
}

impl RawUnitIter {
    pub fn new(dev: ScsiDev, start_lba: u64, num_units: u64) -> Self {
        RawUnitIter {
            dev,
            next_lba: start_lba as u32,
            units_left: num_units,
            batch: Vec::new(),
            batch_pos: 0,
        }
    }

    fn refill(&mut self) -> io::Result<bool> {
        if self.units_left == 0 {
            return Ok(false);
        }
        let want = BATCH_UNITS.min(self.units_left as u32);
        let sectors = (want * SECTORS_PER_UNIT) as u16;
        self.batch = self.dev.read10(self.next_lba, sectors)?;
        self.batch_pos = 0;
        if self.batch.len() < ALIGNED_UNIT_LEN {
            return Ok(false); // short read at end of medium
        }
        self.next_lba += want * SECTORS_PER_UNIT;
        Ok(true)
    }
}

impl Iterator for RawUnitIter {
    type Item = io::Result<[u8; ALIGNED_UNIT_LEN]>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.units_left == 0 {
            return None;
        }
        if self.batch_pos + ALIGNED_UNIT_LEN > self.batch.len() {
            match self.refill() {
                Ok(false) => return None,
                Err(e) => return Some(Err(e)),
                Ok(true) => {}
            }
        }
        let mut u = [0u8; ALIGNED_UNIT_LEN];
        u.copy_from_slice(&self.batch[self.batch_pos..self.batch_pos + ALIGNED_UNIT_LEN]);
        self.batch_pos += ALIGNED_UNIT_LEN;
        self.units_left -= 1;
        Some(Ok(u))
    }
}
