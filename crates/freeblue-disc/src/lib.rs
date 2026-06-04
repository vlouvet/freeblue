//! On-disc AACS structures and raw (encrypted) stream access (spec 04).
//!
//! Confirmed v2 `/AACS/` layout (spec 04 §4.1, `[Disc]`): `MKB_RO.inf`,
//! `Unit_Key_RO.inf`, `Content00x.cer`, `ContentRevocation.lst`,
//! `DH_Pairing_Server.cer`. `freeblue` reads the **raw encrypted** `m2ts` via a
//! plain UDF read (no AACS in the path) and decrypts itself (spec 05).

use freeblue_crypto::Block;

/// Where the disc is read from (spec 04 §4.6).
pub enum DiscSource {
    /// A raw image or block device (e.g. `/dev/sr1`).
    Image(std::path::PathBuf),
    /// An extracted folder dump (`BDMV/`, `AACS/` present).
    Folder(std::path::PathBuf),
}

#[derive(Debug, thiserror::Error)]
pub enum DiscError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("AACS structure not found: {0}")]
    Missing(&'static str),
    #[error("volume id unavailable (needs drive auth / host cert; spec 04 §4.3)")]
    VolumeIdUnavailable,
}

pub struct Disc {
    _source: DiscSource,
}

impl Disc {
    pub fn open(source: DiscSource) -> Result<Self, DiscError> {
        Ok(Disc { _source: source })
    }

    /// Raw bytes of `AACS/MKB_RO.inf` (spec 04 §4.4).
    pub fn mkb(&self) -> Result<Vec<u8>, DiscError> {
        todo!("spec 04 §4.4: read AACS/MKB_RO.inf")
    }

    /// Raw bytes of `AACS/Unit_Key_RO.inf` (spec 04 §4.5).
    pub fn unit_key_file(&self) -> Result<Vec<u8>, DiscError> {
        todo!("spec 04 §4.5: read AACS/Unit_Key_RO.inf")
    }

    /// 16-byte Volume ID. For keydb-listed discs this comes from the key db
    /// (the `I` field) and the drive-auth path is skipped (spec 06 §6.5.1).
    pub fn volume_id(&self) -> Result<Block, DiscError> {
        todo!("spec 04 §4.3 / 06 §6.5.1: volume id (drive auth or keydb)")
    }

    /// Stream raw (encrypted) 6144-byte Aligned Units of a clip's `m2ts`.
    pub fn read_clip_units(&self, _clip: &str) -> Result<impl Iterator<Item = Vec<u8>>, DiscError> {
        // Placeholder so the crate compiles; real impl streams from the UDF
        // volume in 6144-byte units (spec 04 §4.2, spec 05 §5.1).
        Ok(std::iter::empty())
    }
}
