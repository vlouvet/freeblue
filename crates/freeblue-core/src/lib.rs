//! `freeblue` orchestration — the spec 00 §0.4 contract:
//!
//! ```text
//! input:   encrypted UHD/BD volume  +  AACS key material (keydb or device keys)
//! output:  plaintext M2TS bytes  (== MakeMKV's output)
//! ```
//!
//! Pipeline (every step `[Disc]`-verified, spec 09 §9.10.1):
//! `MKB → media key → Kvu → unit key → AES-CBC content decrypt`.
//!
//! ```text
//!   keys ──► [freeblue-mkb]   Kpc → Km   (processing-key path)
//!   Km   ──► [freeblue-crypto] Kvu = AES-G(Km, IDv)
//!   Kvu  ──► [freeblue-mkb/keys] Kcu = AES-128D(Kvu, UnitKeyFile)
//!   Kcu  ──► [freeblue-content] per-Aligned-Unit decrypt → plaintext M2TS
//! ```
//!
//! For a **keydb-listed disc** the `Kvu`/`Kcu` are supplied directly (spec 06
//! §6.5.1), so the MKB step is skipped and decryption needs no device keys.

use freeblue_content::decrypt_aligned_unit;
use freeblue_crypto::{aes_128d, aes_g, Block};
use freeblue_disc::encrypted_unit_key;

/// `Kvu = AES-G(Km, IDv)` (spec 02 §2.4.3, `[Disc]`-verified).
pub fn volume_unique_key(media_key: &Block, volume_id: &Block) -> Block {
    aes_g(media_key, volume_id)
}

/// Unwrap a CPS Unit Key from the Unit Key File: `Kcu = AES-128D(Kvu, enc)`
/// (spec 04 §4.5, `[Disc]`-verified).
pub fn unwrap_unit_key(volume_unique_key: &Block, encrypted_unit_key: &Block) -> Block {
    aes_128d(volume_unique_key, encrypted_unit_key)
}

/// Decrypt one Aligned Unit with a CPS Unit Key (re-export convenience).
pub use freeblue_content::ContentError;
pub fn decrypt_unit(unit_key: &Block, unit: &[u8]) -> Result<Vec<u8>, ContentError> {
    decrypt_aligned_unit(unit_key, unit)
}

/// Orchestration errors (spec 08 §8.4): distinct from a decode bug so the caller
/// can act (e.g. fall back to a different key source).
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("no volume unique key to unwrap the unit key (need a keydb unit key, or media key + volume id → Kvu)")]
    NoVolumeUniqueKey,
    #[error(transparent)]
    Disc(#[from] freeblue_disc::DiscError),
}

/// Resolve the CPS Unit Key for `cps_index` to feed [`decrypt_units`]. Two
/// sources (spec 04 §4.5, spec 06 §6.5.1):
///
/// - **keydb path** — when the community key DB supplies the already-unwrapped
///   unit key (its `U` field, `keydb_unit_key`), use it directly: no MKB, no
///   Volume ID, no device keys needed (the 80% path).
/// - **unwrap path** — otherwise unwrap the Unit Key File's encrypted key with
///   the Volume Unique Key: `Kcu = AES-128D(Kvu, enc)` (`[BD §3.9]`, `[Disc]`).
///
/// `NoVolumeUniqueKey` when neither a keydb unit key nor a `Kvu` is available;
/// `Disc` when the Unit Key File is too short for `cps_index`.
pub fn resolve_unit_key(
    keydb_unit_key: Option<&Block>,
    volume_unique_key: Option<&Block>,
    unit_key_file: &[u8],
    cps_index: usize,
) -> Result<Block, CoreError> {
    if let Some(k) = keydb_unit_key {
        return Ok(*k);
    }
    let kvu = volume_unique_key.ok_or(CoreError::NoVolumeUniqueKey)?;
    let enc = encrypted_unit_key(unit_key_file, cps_index)?;
    Ok(unwrap_unit_key(kvu, &enc))
}

/// Decrypt a clip's raw (on-disc) 6144-byte Aligned Units with its CPS Unit Key
/// into plaintext M2TS units, **lazily** (spec 05 §5.4) — the right half of the
/// §0.4 contract. Read-source-agnostic: `raw_units` come from `freeblue-read`
/// (spec 11) or a folder/image dump, so the verified content cipher stays
/// decoupled from the (in-flux) read path. Each item is
/// `Err(ContentError::BadUnitLength)` if a unit isn't 6144 bytes.
///
/// A full `decrypt_clip(disc, keys, clip)` (spec 08 §8.4) is just
/// `decrypt_units(resolve_unit_key(…)?, reader.read_units(clip)?)` once a
/// `freeblue-read::UnitReader` is plumbed in.
pub fn decrypt_units<I>(
    unit_key: Block,
    raw_units: I,
) -> impl Iterator<Item = Result<Vec<u8>, ContentError>>
where
    I: IntoIterator<Item = Vec<u8>>,
{
    raw_units
        .into_iter()
        .map(move |u| decrypt_aligned_unit(&unit_key, &u))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(s: &str) -> Block {
        let v = hex::decode(s).unwrap();
        let mut b = [0u8; 16];
        b.copy_from_slice(&v);
        b
    }

    // Structural: Kvu derivation composes the verified primitive. (The real
    // M/I/V vector is secret — full [Disc] check runs from $FREEBLUE_FIXTURES.)
    #[test]
    fn vuk_is_aes_g() {
        let km = h("00112233445566778899AABBCCDDEEFF");
        let idv = h("0102030405060708090A0B0C0D0E0F10");
        assert_eq!(volume_unique_key(&km, &idv), aes_g(&km, &idv));
    }

    #[test]
    fn resolve_prefers_the_keydb_unit_key() {
        // keydb supplies the already-unwrapped key → returned verbatim, no file,
        // no Kvu touched.
        let u = h("AABBCCDDEEFF00112233445566778899");
        let got = resolve_unit_key(Some(&u), None, &[], 0).unwrap();
        assert_eq!(got, u);
    }

    #[test]
    fn resolve_unwraps_with_kvu_roundtrip() {
        // No keydb key → unwrap the Unit Key File's encrypted key with Kvu.
        // Build the fixture the disc would hold: enc = AES-128E(Kvu, Kcu) at the
        // [Disc] offset 112, so the unwrap must return Kcu (spec 04 §4.5).
        let kvu = h("000102030405060708090A0B0C0D0E0F");
        let kcu = h("0F0E0D0C0B0A09080706050403020100");
        let enc = freeblue_crypto::aes_128e(&kvu, &kcu);
        let mut file = vec![0u8; 112];
        file.extend_from_slice(&enc);

        let got = resolve_unit_key(None, Some(&kvu), &file, 0).unwrap();
        assert_eq!(got, kcu, "Kcu = AES-128D(Kvu, AES-128E(Kvu, Kcu))");
    }

    #[test]
    fn resolve_errors_without_a_keydb_key_or_kvu() {
        assert!(matches!(
            resolve_unit_key(None, None, &[], 0),
            Err(CoreError::NoVolumeUniqueKey)
        ));
    }

    #[test]
    fn resolve_propagates_a_short_unit_key_file() {
        let kvu = h("000102030405060708090A0B0C0D0E0F");
        // Header only, no key bytes → disc parser errors → CoreError::Disc.
        let short = vec![0u8; 112];
        assert!(matches!(
            resolve_unit_key(None, Some(&kvu), &short, 0),
            Err(CoreError::Disc(_))
        ));
    }

    #[test]
    fn decrypt_units_maps_each_unit_and_passes_the_seed_through() {
        use freeblue_content::ALIGNED_UNIT_LEN;
        let key = h("00112233445566778899AABBCCDDEEFF");
        // Two synthetic 6144-byte units (arbitrary bytes — exercises the stream
        // wrapper, not the cipher KAT, which is freeblue-content's job).
        let mut a = vec![0x11u8; ALIGNED_UNIT_LEN];
        a[..16].copy_from_slice(&h("0102030405060708090A0B0C0D0E0F10"));
        let mut b = vec![0x22u8; ALIGNED_UNIT_LEN];
        b[..16].copy_from_slice(&h("1112131415161718191A1B1C1D1E1F20"));

        let out: Vec<_> = decrypt_units(key, vec![a.clone(), b.clone()])
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(out.len(), 2);
        // Each result == the direct call, and the clear seed passes through.
        assert_eq!(out[0], decrypt_aligned_unit(&key, &a).unwrap());
        assert_eq!(out[0][..16], a[..16]);
        assert_eq!(out[1][..16], b[..16]);
    }

    #[test]
    fn decrypt_units_propagates_a_bad_length() {
        let key = h("00112233445566778899AABBCCDDEEFF");
        let mut results = decrypt_units(key, vec![vec![0u8; 100]]);
        assert!(matches!(
            results.next(),
            Some(Err(ContentError::BadUnitLength(100)))
        ));
    }
}
