//! Read path (spec 11): obtain **AACS-content-encrypted (non-bus)** Aligned
//! Units from a disc/drive, to feed the verified decrypt core (freeblue-content).
//!
//! A `UnitReader` returns content-encrypted, *not* bus-encrypted, units. For
//! **non-BEE** discs a plain read suffices ([`PlainUdfReader`]); **BEE** discs
//! (incl. UHD, spec 04 §4.3.2) need a backend that strips/avoids bus encryption
//! ([`LibreDriveReader`] / [`AacsAuthReader`] — not yet implemented).

use std::io::Read;
use std::path::PathBuf;

// LibreDrive raw-read path (spec 11 §11.4.6–7) — Linux/SG_IO only.
#[cfg(target_os = "linux")]
mod libredrive;
#[cfg(target_os = "linux")]
mod libredrive_unlock;
#[cfg(target_os = "linux")]
mod scsi;

/// AACS Aligned-Unit size (spec 05 §5.1).
pub const ALIGNED_UNIT_LEN: usize = 6144;
/// One on-disc Aligned Unit.
pub type Unit = [u8; ALIGNED_UNIT_LEN];
/// Iterator of units (errors surface per-unit; the call itself fails fast).
pub type UnitIter<'a> = Box<dyn Iterator<Item = std::io::Result<Unit>> + 'a>;

/// Identifies a clip to read, with whatever a backend needs.
#[derive(Clone, Debug, Default)]
pub struct ClipId {
    /// File path to the clip's `m2ts` (file / UDF backends).
    pub path: Option<PathBuf>,
    /// `(start_lba, num_units)` on the disc (drive / LibreDrive backends).
    pub disc_extent: Option<(u64, u64)>,
}

impl ClipId {
    pub fn from_path(p: impl Into<PathBuf>) -> Self {
        ClipId { path: Some(p.into()), disc_extent: None }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    /// The disc is bus-encrypted (BEE) and this backend cannot strip it
    /// (spec 11 §11.2). Use a LibreDrive / AACS-auth backend.
    #[error("disc is bus-encrypted (BEE); this reader cannot strip it")]
    BusEncrypted,
    #[error("backend not implemented: {0}")]
    Unimplemented(&'static str),
    #[error("clip has no file path for a file-based reader")]
    NoPath,
    /// A drive backend needs `ClipId::disc_extent` (start_lba, num_units).
    #[error("clip has no disc extent for a drive-based reader")]
    NoExtent,
    /// The drive did not answer the LibreDrive handshake — raw reads would
    /// return bus-encrypted garbage, so we refuse (spec 11 §11.4.6).
    #[error("drive is not LibreDrive-capable; cannot get raw (non-bus) reads")]
    NotLibreDrive,
}

/// Yields AACS-content-encrypted (non-bus) Aligned Units for a clip.
pub trait UnitReader {
    fn read_units(&mut self, clip: &ClipId) -> Result<UnitIter<'_>, ReadError>;
}

// ─────────────────────────────────────────────────────────────────────────────
// PlainUdfReader — non-BEE / folder-dump path (spec 11 §11.3.1)
// ─────────────────────────────────────────────────────────────────────────────

/// Reads 6144-B units straight from a file (a mounted UDF `m2ts` or a dump).
///
/// Correct **only for non-BEE discs**. It cannot self-detect BEE from raw
/// ciphertext (encrypted content is high-entropy either way); BEE detection is
/// upstream (`freeblue-core` via the keydb / on-disc flag, spec 11 §11.5), and
/// core must not route a BEE disc here. The post-decrypt TS-sync check
/// (`freeblue-content::ts_sync_score`) is the backstop that catches a misroute.
#[derive(Default)]
pub struct PlainUdfReader;

impl PlainUdfReader {
    pub fn new() -> Self {
        PlainUdfReader
    }
}

impl UnitReader for PlainUdfReader {
    fn read_units(&mut self, clip: &ClipId) -> Result<UnitIter<'_>, ReadError> {
        let path = clip.path.as_ref().ok_or(ReadError::NoPath)?;
        let file = std::fs::File::open(path)?;
        Ok(Box::new(FileUnitIter { file }))
    }
}

struct FileUnitIter {
    file: std::fs::File,
}

impl Iterator for FileUnitIter {
    type Item = std::io::Result<Unit>;
    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = [0u8; ALIGNED_UNIT_LEN];
        let mut filled = 0;
        while filled < ALIGNED_UNIT_LEN {
            match self.file.read(&mut buf[filled..]) {
                Ok(0) => break, // EOF
                Ok(n) => filled += n,
                Err(ref e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(e) => return Some(Err(e)),
            }
        }
        match filled {
            0 => None,                       // clean EOF
            ALIGNED_UNIT_LEN => Some(Ok(buf)),
            _ => None,                       // trailing partial unit — stop
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// LibreDriveReader / AacsAuthReader — BEE/UHD backends (spec 11 §11.3.2-3)
// ─────────────────────────────────────────────────────────────────────────────

/// Reads **raw** (content-encrypted, non-bus) content from a LibreDrive-capable
/// drive **without MakeMKV** (spec 11 §11.4.6–7): replay the read-only LibreDrive
/// unlock sequence over `SG_IO`, then `READ(10)` the clip's extent. Needs
/// `ClipId::disc_extent`. Linux-only; on other platforms `read_units` errors.
pub struct LibreDriveReader {
    pub device: PathBuf,
}

impl LibreDriveReader {
    pub fn open(device: impl Into<PathBuf>) -> Self {
        LibreDriveReader { device: device.into() }
    }
}

impl UnitReader for LibreDriveReader {
    #[cfg(target_os = "linux")]
    fn read_units(&mut self, clip: &ClipId) -> Result<UnitIter<'_>, ReadError> {
        let (start_lba, num_units) = clip.disc_extent.ok_or(ReadError::NoExtent)?;
        let dev = scsi::ScsiDev::open(&self.device.to_string_lossy())?;
        if !libredrive::unlock(&dev)? {
            return Err(ReadError::NotLibreDrive);
        }
        Ok(Box::new(libredrive::RawUnitIter::new(dev, start_lba, num_units)))
    }

    #[cfg(not(target_os = "linux"))]
    fn read_units(&mut self, _clip: &ClipId) -> Result<UnitIter<'_>, ReadError> {
        Err(ReadError::Unimplemented("LibreDriveReader: SG_IO is Linux-only"))
    }
}

/// Reads non-bus content via AACS drive↔host auth + bus key (spec 11 §11.3.3).
/// Needs an unrevoked host certificate (spec 06 §6.6) — currently impractical.
pub struct AacsAuthReader {
    pub device: PathBuf,
}

impl UnitReader for AacsAuthReader {
    fn read_units(&mut self, _clip: &ClipId) -> Result<UnitIter<'_>, ReadError> {
        Err(ReadError::Unimplemented("AacsAuthReader: needs an unrevoked host cert (spec 06 §6.6)"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn plain_reader_yields_aligned_units() {
        // Two synthetic units (distinguishable: 0x11.. then 0x22..).
        let dir = std::env::temp_dir();
        let p = dir.join("freeblue_read_test.bin");
        {
            let mut f = std::fs::File::create(&p).unwrap();
            f.write_all(&[0x11u8; ALIGNED_UNIT_LEN]).unwrap();
            f.write_all(&[0x22u8; ALIGNED_UNIT_LEN]).unwrap();
        }
        let mut r = PlainUdfReader::new();
        let units: Vec<_> = r
            .read_units(&ClipId::from_path(&p))
            .unwrap()
            .map(|u| u.unwrap())
            .collect();
        std::fs::remove_file(&p).ok();
        assert_eq!(units.len(), 2);
        assert_eq!(units[0], [0x11u8; ALIGNED_UNIT_LEN]);
        assert_eq!(units[1], [0x22u8; ALIGNED_UNIT_LEN]);
    }

    #[test]
    fn plain_reader_drops_trailing_partial_unit() {
        let dir = std::env::temp_dir();
        let p = dir.join("freeblue_read_partial.bin");
        {
            let mut f = std::fs::File::create(&p).unwrap();
            f.write_all(&[0x33u8; ALIGNED_UNIT_LEN]).unwrap();
            f.write_all(&[0x44u8; 100]).unwrap(); // partial
        }
        let mut r = PlainUdfReader::new();
        let n = r.read_units(&ClipId::from_path(&p)).unwrap().count();
        std::fs::remove_file(&p).ok();
        assert_eq!(n, 1);
    }

    #[test]
    fn plain_reader_without_path_errors() {
        let mut r = PlainUdfReader::new();
        assert!(matches!(
            r.read_units(&ClipId::default()),
            Err(ReadError::NoPath)
        ));
    }

    // A LibreDrive read needs a disc extent; with none given it must error before
    // ever touching the drive (no hardware needed for this check).
    #[cfg(target_os = "linux")]
    #[test]
    fn libredrive_without_extent_errors_before_touching_drive() {
        let mut r = LibreDriveReader::open("/dev/sr0");
        assert!(matches!(
            r.read_units(&ClipId::default()),
            Err(ReadError::NoExtent)
        ));
    }

    // The embedded LibreDrive unlock sequence must be the observed handshake-first,
    // all-READ-BUFFER(0x3C mode 2 / buffer 0x77) table (spec 11 §11.4.7).
    #[cfg(target_os = "linux")]
    #[test]
    fn unlock_table_is_well_formed() {
        use crate::libredrive_unlock::LIBREDRIVE_UNLOCK_WH16NS60 as T;
        assert!(T.len() > 100, "unlock table suspiciously short");
        assert_eq!(T[0], (0x000000, 64), "must start with the 64-byte handshake read");
        // every read length is the 24-bit allocation length the CDB can carry
        assert!(T.iter().all(|&(_, len)| len == 64 || len == 4));
    }
}
