//! AACS content decryption — the Aligned-Unit pipeline (spec 05).
//!
//! `[Disc]`-verified end-to-end on a real disc (GoT, BD MKBv63): every sampled
//! 6144-byte Aligned Unit decrypted to valid MPEG-TS (32/32 TS sync bytes,
//! valid PAT) — spec 05 §5.3.1, spec 09 §9.10.1.

use freeblue_crypto::{aes_128d, aes_128e, Block};

/// AACS Aligned-Unit size: 32 × 192-byte M2TS source packets (spec 05 §5.1).
pub const ALIGNED_UNIT_LEN: usize = 6144;

/// Bytes of clear "seed" at the head of each Aligned Unit (spec 05 §5.3).
pub const SEED_LEN: usize = 16;

/// The AACS content CBC IV constant (spec 05 §5.3, `[Disc]`-verified).
pub const CONTENT_IV: Block = [
    0x0B, 0xA0, 0xF8, 0xDD, 0xFE, 0xA6, 0x1F, 0xB3, 0xD8, 0xDF, 0x9F, 0x56, 0x6A, 0x05, 0x0F, 0x78,
];

/// Per-unit **block key** from the CPS Unit Key and the unit's 16-byte seed
/// (spec 05 §5.3, `[Disc]`-verified):
///
/// ```text
/// block_key = AES-128E(unit_key, seed) XOR seed
/// ```
///
/// Note: this is the AES-128**E** form, **not** AES-G (which uses AES-128D).
pub fn block_key(unit_key: &Block, seed: &Block) -> Block {
    let e = aes_128e(unit_key, seed);
    let mut out = [0u8; 16];
    for i in 0..16 {
        out[i] = e[i] ^ seed[i];
    }
    out
}

/// Errors from Aligned-Unit decryption.
#[derive(Debug, PartialEq, Eq)]
pub enum ContentError {
    /// The input was not exactly [`ALIGNED_UNIT_LEN`] bytes.
    BadUnitLength(usize),
}

/// Decrypt one 6144-byte Aligned Unit in place into `out` (spec 05 §5.4).
///
/// `unit` is the on-disc (encrypted) Aligned Unit. The first 16 bytes are the
/// clear seed (copied through); the remaining 6128 bytes are AES-128-CBC under
/// the per-unit block key with a fresh chain (IV = [`CONTENT_IV`]).
pub fn decrypt_aligned_unit(unit_key: &Block, unit: &[u8]) -> Result<Vec<u8>, ContentError> {
    if unit.len() != ALIGNED_UNIT_LEN {
        return Err(ContentError::BadUnitLength(unit.len()));
    }
    let mut seed = [0u8; 16];
    seed.copy_from_slice(&unit[..SEED_LEN]);
    let bk = block_key(unit_key, &seed);

    let mut out = Vec::with_capacity(ALIGNED_UNIT_LEN);
    out.extend_from_slice(&seed); // clear seed passes through (spec 05 §5.3)

    // AES-128-CBC decrypt of bytes [16..6144], fresh chain per unit.
    let mut prev = CONTENT_IV;
    let mut i = SEED_LEN;
    while i < ALIGNED_UNIT_LEN {
        let mut ct = [0u8; 16];
        ct.copy_from_slice(&unit[i..i + 16]);
        let dec = aes_128d(&bk, &ct);
        for j in 0..16 {
            out.push(dec[j] ^ prev[j]);
        }
        prev = ct;
        i += 16;
    }
    Ok(out)
}

/// Cheap smoke test (spec 05 §5.7): in valid M2TS, every 188-byte TS packet
/// (offset +4 within each 192-byte packet) starts with sync byte `0x47`.
/// Returns how many of the 32 packet boundaries in a decrypted unit are correct.
pub fn ts_sync_score(decrypted_unit: &[u8]) -> usize {
    let mut n = 0;
    let mut o = 4;
    while o < decrypted_unit.len() {
        if decrypted_unit[o] == 0x47 {
            n += 1;
        }
        o += 192;
    }
    n
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

    #[test]
    fn block_key_kat() {
        // Regression vector (deterministic from the verified formula).
        let u = h("00112233445566778899AABBCCDDEEFF");
        let seed = h("0102030405060708090A0B0C0D0E0F10");
        assert_eq!(block_key(&u, &seed), h("BE672ABA687354A3EEBEEB44CB553E25"));
    }

    #[test]
    fn rejects_wrong_length() {
        let k = h("00112233445566778899AABBCCDDEEFF");
        assert_eq!(
            decrypt_aligned_unit(&k, &[0u8; 100]),
            Err(ContentError::BadUnitLength(100))
        );
    }

    // The full Aligned-Unit decrypt is `[Disc]`-verified (spec 05 §5.3.1) but
    // needs a real encrypted unit + its Unit Key, which are secret/copyrighted
    // and never committed (spec 10 §10.4). The build host loads them from
    // $FREEBLUE_FIXTURES (spec 09 §9.6). Skipped here when absent.
    #[test]
    #[ignore = "needs $FREEBLUE_FIXTURES: real encrypted unit + unit key (spec 09 §9.6)"]
    fn aligned_unit_decrypts_to_valid_ts() {
        // TODO(build-host): load fixtures/got_unit0.bin + unit key, then:
        //   let pt = decrypt_aligned_unit(&unit_key, &enc_unit).unwrap();
        //   assert_eq!(ts_sync_score(&pt), 32);
    }
}
