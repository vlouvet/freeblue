//! On-disc AACS structures and raw (encrypted) stream access (spec 04).
//!
//! Confirmed v2 `/AACS/` layout (spec 04 §4.1, `[Disc]`): `MKB_RO.inf`,
//! `Unit_Key_RO.inf`, `Content00x.cer`, `ContentRevocation.lst`,
//! `DH_Pairing_Server.cer`. `freeblue` reads the **raw encrypted** `m2ts` via a
//! plain UDF read (no AACS in the path) and decrypts itself (spec 05).
//!
//! This crate currently reads the AACS structures from an **extracted folder
//! dump** (the shape of the MKBv82 corpus fixture, spec 04 §4.6) and parses the
//! Unit Key File. The image/UDF read path and raw `m2ts` streaming live in
//! `freeblue-read` (spec 11).

use std::path::{Path, PathBuf};

use freeblue_crypto::Block;

/// Where the disc is read from (spec 04 §4.6).
pub enum DiscSource {
    /// A raw image or block device (e.g. `/dev/sr1`).
    Image(PathBuf),
    /// An extracted folder dump (`BDMV/`, `AACS/` present).
    Folder(PathBuf),
}

#[derive(Debug, thiserror::Error)]
pub enum DiscError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("AACS structure not found: {0}")]
    Missing(&'static str),
    #[error("malformed AACS structure: {0}")]
    Malformed(&'static str),
    #[error("reading from an image/device needs the UDF path (freeblue-read, spec 11); use an extracted folder dump for now (spec 04 §4.6)")]
    ImageReadUnsupported,
    #[error("volume id unavailable (needs drive auth / host cert; spec 04 §4.3)")]
    VolumeIdUnavailable,
}

pub struct Disc {
    source: DiscSource,
}

impl Disc {
    /// Open a disc source. For a [`DiscSource::Folder`] the `AACS/` directory
    /// must exist (the minimal "this is a usable dump" check); an image source
    /// opens lazily (its reads go through `freeblue-read`, spec 11).
    pub fn open(source: DiscSource) -> Result<Self, DiscError> {
        if let DiscSource::Folder(root) = &source {
            if !aacs_dir(root).is_dir() {
                return Err(DiscError::Missing("AACS directory"));
            }
        }
        Ok(Disc { source })
    }

    /// Raw bytes of `AACS/MKB_RO.inf` (spec 04 §4.4), handed to spec 03's MKB
    /// parser (`freeblue-mkb`).
    pub fn mkb(&self) -> Result<Vec<u8>, DiscError> {
        self.read_aacs("MKB_RO.inf")
    }

    /// Raw bytes of `AACS/Unit_Key_RO.inf` (spec 04 §4.5), parsed into the
    /// encrypted CPS unit keys by [`encrypted_unit_key`].
    pub fn unit_key_file(&self) -> Result<Vec<u8>, DiscError> {
        self.read_aacs("Unit_Key_RO.inf")
    }

    /// 16-byte Volume ID. For keydb-listed discs this comes from the key db
    /// (the `I` field) and the drive-auth path is skipped (spec 06 §6.5.1).
    pub fn volume_id(&self) -> Result<Block, DiscError> {
        // The Volume ID isn't an AACS *file*; it comes from the keydb `I` field
        // (keydb-listed discs, spec 06 §6.5.1) or from drive auth (spec 04 §4.3),
        // neither of which this crate owns yet.
        Err(DiscError::VolumeIdUnavailable)
    }

    /// Read one named file from the source's `AACS/` directory (folder source
    /// only for now). `Missing` if absent, [`DiscError::ImageReadUnsupported`]
    /// for an image source.
    fn read_aacs(&self, name: &'static str) -> Result<Vec<u8>, DiscError> {
        match &self.source {
            DiscSource::Folder(root) => {
                let path = aacs_dir(root).join(name);
                std::fs::read(path).map_err(|e| {
                    if e.kind() == std::io::ErrorKind::NotFound {
                        DiscError::Missing(name)
                    } else {
                        DiscError::Io(e)
                    }
                })
            }
            DiscSource::Image(_) => Err(DiscError::ImageReadUnsupported),
        }
    }

    /// Stream raw (encrypted) 6144-byte Aligned Units of a clip's `m2ts`.
    pub fn read_clip_units(&self, _clip: &str) -> Result<impl Iterator<Item = Vec<u8>>, DiscError> {
        // Placeholder so the crate compiles; the real raw-read path is
        // `freeblue-read` (spec 11), which streams 6144-byte units from the UDF
        // volume (non-bus for BEE discs).
        Ok(std::iter::empty())
    }
}

/// The `AACS/` directory under a folder-dump `root`.
fn aacs_dir(root: &Path) -> PathBuf {
    root.join("AACS")
}

/// Offset of the first encrypted CPS Unit Key in `Unit_Key_RO.inf`: a 64-byte
/// header record (type `0x00`, len `0x40`) followed by a 48-byte sub-header
/// (spec 04 §4.5). ✅ **[Disc]-verified** on the MKBv82 disc — the first
/// encrypted unit key sits at offset 112.
const FIRST_UNIT_KEY_OFFSET: usize = 112;

/// The **encrypted** CPS Unit Key for `cps_unit_index`, sliced from raw
/// `Unit_Key_RO.inf` bytes: 16 bytes at `112 + index*16` (spec 04 §4.5).
/// Unwrap it with `freeblue_core::unwrap_unit_key` (= `AES-128D(Kvu, ·)`,
/// `[BD §3.9]`, `[Disc]`-verified). `Malformed` if the file is too short to hold
/// that key.
///
/// The *number* of CPS units (how many keys follow) is still **[?]** (spec 04
/// §4.5) — a basic single-CPS-unit disc (the target, e.g. GoT) uses index 0.
pub fn encrypted_unit_key(unit_key_file: &[u8], cps_unit_index: usize) -> Result<Block, DiscError> {
    let start = FIRST_UNIT_KEY_OFFSET + cps_unit_index * 16;
    let slice = unit_key_file
        .get(start..start + 16)
        .ok_or(DiscError::Malformed(
            "Unit_Key_RO.inf too short for the requested CPS unit key",
        ))?;
    let mut key = [0u8; 16];
    key.copy_from_slice(slice);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unique temp dir for one test, cleaned before use (single-process tests).
    fn tmpdir(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!("{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn encrypted_unit_key_reads_16_bytes_at_offset_112() {
        // 112-byte header, then two 16-byte encrypted keys (synthetic — not real
        // key material; this tests the offset/length math from spec 04 §4.5).
        let mut file = vec![0u8; FIRST_UNIT_KEY_OFFSET];
        let key0 = [0xAA; 16];
        let key1 = [0xBB; 16];
        file.extend_from_slice(&key0);
        file.extend_from_slice(&key1);
        assert_eq!(encrypted_unit_key(&file, 0).unwrap(), key0);
        assert_eq!(encrypted_unit_key(&file, 1).unwrap(), key1);
    }

    #[test]
    fn encrypted_unit_key_errors_when_too_short() {
        // Header + only a partial first key.
        let short = vec![0u8; FIRST_UNIT_KEY_OFFSET + 8];
        assert!(matches!(
            encrypted_unit_key(&short, 0),
            Err(DiscError::Malformed(_))
        ));
        // Exactly one key present: index 0 ok, index 1 out of range.
        let mut one = vec![0u8; FIRST_UNIT_KEY_OFFSET];
        one.extend_from_slice(&[0u8; 16]);
        assert!(encrypted_unit_key(&one, 0).is_ok());
        assert!(matches!(
            encrypted_unit_key(&one, 1),
            Err(DiscError::Malformed(_))
        ));
    }

    #[test]
    fn folder_source_reads_aacs_structures() {
        let dir = tmpdir("freeblue_disc_read");
        let aacs = dir.join("AACS");
        std::fs::create_dir_all(&aacs).unwrap();
        std::fs::write(aacs.join("MKB_RO.inf"), b"mkb-bytes").unwrap();
        std::fs::write(aacs.join("Unit_Key_RO.inf"), b"ukf-bytes").unwrap();

        let disc = Disc::open(DiscSource::Folder(dir.clone())).unwrap();
        assert_eq!(disc.mkb().unwrap(), b"mkb-bytes");
        assert_eq!(disc.unit_key_file().unwrap(), b"ukf-bytes");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn missing_structure_is_reported() {
        let dir = tmpdir("freeblue_disc_missing");
        std::fs::create_dir_all(dir.join("AACS")).unwrap();
        let disc = Disc::open(DiscSource::Folder(dir.clone())).unwrap();
        assert!(matches!(disc.mkb(), Err(DiscError::Missing(_))));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_without_an_aacs_dir_errors() {
        let dir = tmpdir("freeblue_disc_noaacs");
        std::fs::create_dir_all(&dir).unwrap();
        assert!(matches!(
            Disc::open(DiscSource::Folder(dir.clone())),
            Err(DiscError::Missing(_))
        ));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn image_structure_reads_are_not_supported_yet() {
        let disc = Disc::open(DiscSource::Image("/dev/sr1".into())).unwrap();
        assert!(matches!(disc.mkb(), Err(DiscError::ImageReadUnsupported)));
    }
}
