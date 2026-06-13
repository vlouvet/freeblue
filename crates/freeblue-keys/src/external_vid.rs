//! External Volume-ID ingestion (spec 06 §6.8).
//!
//! When freeblue's own drive↔host AKE is blocked by the **MKBv82 host-cert
//! revocation** (spec 06 §6.6.1) and the disc is **not** in the keydb, the
//! practical path is to consume a Volume ID captured by a tool that holds an
//! *unrevoked* host cert (e.g. MakeMKV). Two sources:
//!
//!   - a MakeMKV `discatt.dat` blob, and
//!   - the libaacs `<cache>/aacs/vid/<discid>` cache,
//!
//! both keyed by the AACS **disc-id = SHA-1(`Unit_Key_RO.inf`)** — the same
//! 20-byte id that keys a keydb entry (spec 06 §6.5).
//!
//! freeblue only *reads* a VID another tool produced — it circumvents nothing.
//! Rule 4: a Volume ID is gated disc material; never committed, supplied only at
//! runtime, zeroized after use. Tests use synthetic bytes / git-ignored fixtures.

use crate::DiscId;
use freeblue_crypto::Block;
use sha1::{Digest, Sha1};
use std::path::Path;
use zeroize::Zeroize;

/// AACS disc-id = **SHA-1 of the whole `Unit_Key_RO.inf`** — the libaacs/keydb
/// convention, oracle-confirmed: `crypto_aacs_title_hash()` is
/// `gcry_md_hash_buffer(GCRY_MD_SHA1, ukf, len)` over the file libaacs reads as
/// `AACS/Unit_Key_RO.inf` `[libaacs aacs.c / crypto.c]`. This id keys both the
/// keydb per-disc entry (spec 06 §6.5) and the libaacs `vid` cache, so freeblue
/// can look a disc up from the disc itself (`freeblue-disc::Disc::unit_key_file`).
///
/// `[Disc]`: `disc_id(real Unit_Key_RO.inf) == that disc's keydb key` (the
/// `disc_id_matches_keydb` fixture test).
pub fn disc_id(unit_key_file: &[u8]) -> DiscId {
    let mut h = Sha1::new();
    h.update(unit_key_file);
    h.finalize().into()
}

/// A Volume ID obtained from an external oracle. Zeroized on drop (Rule 4).
pub struct ExternalVid {
    /// The 16-byte Volume ID (`IDv`), ready to feed `Kvu = AES-G(Km, IDv)`
    /// (spec 02 §2.4.3 / `freeblue-core::volume_unique_key`).
    pub volume_id: Block,
}

impl Drop for ExternalVid {
    fn drop(&mut self) {
        self.volume_id.zeroize();
    }
}

#[derive(Debug, thiserror::Error)]
pub enum VidError {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0} is too short to hold a Volume ID")]
    TooShort(&'static str),
    #[error("{0} is malformed (not the expected encoding)")]
    Malformed(&'static str),
}

/// `discatt.dat` fixed trailer: `VID (16 B)` then the drive certificate
/// (`92 B`) — `[Disc]`-pinned by byte-matching the keydb Volume ID inside a real
/// MakeMKV `discatt.dat` (spec 06 §6.8): the VID sits at `len − 108`.
const DISCATT_DRIVE_CERT_LEN: usize = 92;
const DISCATT_TRAILER_LEN: usize = 16 + DISCATT_DRIVE_CERT_LEN; // VID + DC

/// Extract the Volume ID from a MakeMKV `discatt.dat` blob.
///
/// The VID is the 16 bytes at `len − 108` (a `VID(16) | DriveCert(92)` trailer).
/// Pinned `[Disc]` against one real `discatt.dat`; if a future MakeMKV layout
/// differs, the fixture test catches it (residual cross-version `[?]`).
pub fn parse_discatt(bytes: &[u8]) -> Result<ExternalVid, VidError> {
    if bytes.len() < DISCATT_TRAILER_LEN {
        return Err(VidError::TooShort("discatt.dat"));
    }
    let start = bytes.len() - DISCATT_TRAILER_LEN;
    let mut volume_id = [0u8; 16];
    volume_id.copy_from_slice(&bytes[start..start + 16]);
    Ok(ExternalVid { volume_id })
}

/// Read the Volume ID from a libaacs `vid` cache.
///
/// `cache_dir` is libaacs's per-config dir (e.g. `~/.cache/aacs` /
/// `$XDG_CACHE_HOME/aacs`); the file is `cache_dir/vid/<disc_id-40hex>` and its
/// content is the 16-byte VID as **32 lowercase hex chars** (no prefix/newline)
/// `[libaacs keydbcfg.c keycache_save]`. `Ok(None)` if the disc has no entry.
pub fn read_vid_cache(cache_dir: &Path, disc: &DiscId) -> Result<Option<ExternalVid>, VidError> {
    let path = cache_dir.join("vid").join(hex::encode(disc));
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let raw = hex::decode(text.trim()).map_err(|_| VidError::Malformed("vid cache (hex)"))?;
    if raw.len() != 16 {
        return Err(VidError::Malformed("vid cache (length)"));
    }
    let mut volume_id = [0u8; 16];
    volume_id.copy_from_slice(&raw);
    Ok(Some(ExternalVid { volume_id }))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// SHA-1 wiring KAT — FIPS-180 published vector `SHA1("abc")`. No keys.
    #[test]
    fn disc_id_is_sha1_of_input() {
        assert_eq!(
            hex::encode(disc_id(b"abc")),
            "a9993e364706816aba3e25717850c26c9cd0d89d"
        );
    }

    /// Parser KAT — synthetic blob `[filler][fake VID 16][filler 92]`; the VID
    /// must come out of the `len − 108` slot. Fake bytes, no real keys.
    #[test]
    fn parse_discatt_extracts_trailer_vid() {
        let fake = [0xABu8; 16];
        let mut blob = vec![0x11u8; 50];
        blob.extend_from_slice(&fake);
        blob.extend(std::iter::repeat(0x22u8).take(DISCATT_DRIVE_CERT_LEN));
        // len = 50 + 16 + 92 = 158; VID slot = 158 − 108 = 50.
        assert_eq!(parse_discatt(&blob).unwrap().volume_id, fake);
    }

    #[test]
    fn parse_discatt_rejects_short() {
        assert!(matches!(
            parse_discatt(&[0u8; 50]),
            Err(VidError::TooShort(_))
        ));
    }

    /// vid-cache KAT — write a libaacs-format hex file and read it back.
    #[test]
    fn read_vid_cache_round_trip() {
        let base = std::env::temp_dir().join("fb_vidcache_rt");
        let _ = std::fs::remove_dir_all(&base);
        let disc: DiscId = [0u8; 20];
        let vdir = base.join("vid");
        std::fs::create_dir_all(&vdir).unwrap();
        std::fs::write(
            vdir.join(hex::encode(disc)),
            "00112233445566778899aabbccddeeff",
        )
        .unwrap();
        let vid = read_vid_cache(&base, &disc).unwrap().unwrap();
        assert_eq!(
            hex::encode(vid.volume_id),
            "00112233445566778899aabbccddeeff"
        );
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn read_vid_cache_absent_is_none() {
        let base = std::env::temp_dir().join("fb_vidcache_absent");
        let _ = std::fs::remove_dir_all(&base);
        assert!(read_vid_cache(&base, &[7u8; 20]).unwrap().is_none());
    }

    /// `[Disc]` gate: disc-id computed from a real `Unit_Key_RO.inf` equals that
    /// disc's keydb key. Fixtures only (Rule 4 / spec 09 §9.6).
    #[test]
    #[ignore = "needs $FREEBLUE_FIXTURES/{Unit_Key_RO.inf,disc_id.hex}"]
    fn disc_id_matches_keydb() {
        let dir = std::env::var("FREEBLUE_FIXTURES").expect("set $FREEBLUE_FIXTURES");
        let ukf = std::fs::read(format!("{dir}/Unit_Key_RO.inf")).unwrap();
        let want = std::fs::read_to_string(format!("{dir}/disc_id.hex")).unwrap();
        assert_eq!(hex::encode(disc_id(&ukf)), want.trim().to_lowercase());
    }

    /// `[Disc]` gate: a real MakeMKV `discatt.dat` yields a non-zero VID at the
    /// pinned trailer offset (spec 06 §6.8). Fixtures only.
    #[test]
    #[ignore = "needs $FREEBLUE_FIXTURES/discatt.dat"]
    fn parse_discatt_real_fixture() {
        let dir = std::env::var("FREEBLUE_FIXTURES").expect("set $FREEBLUE_FIXTURES");
        let blob = std::fs::read(format!("{dir}/discatt.dat")).unwrap();
        let vid = parse_discatt(&blob).expect("parse discatt.dat VID");
        assert_ne!(vid.volume_id, [0u8; 16]);
    }
}
