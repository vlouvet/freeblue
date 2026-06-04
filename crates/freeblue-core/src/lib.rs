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

// TODO(TDD): `decrypt_clip(disc, keys, clip) -> impl Iterator<plaintext units>`
// tying freeblue-disc + freeblue-keys + freeblue-mkb + freeblue-content together
// (spec 08 §8.4). Lands once the disc/keys crates are implemented.

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
}
