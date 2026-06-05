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

/// Length of a bus-encryption block: the optical **sector** (spec 11 §11.4.4).
pub const SECTOR_LEN: usize = 2048;

/// Strip the **bus-encryption** layer from one 6144-byte Aligned Unit
/// (spec 11 §11.4.4, `[Disc]`-verified on Turbo UHD). BEE discs (spec 04 §4.3.2)
/// have the drive double-encrypt content over the bus with a `read_data_key`
/// derived in AACS drive↔host auth; a LibreDrive reader scrapes that key and
/// calls this **before** [`decrypt_aligned_unit`].
///
/// The bus layer is AES-128-CBC, applied **per 2048-byte sector** (3 per unit):
/// bytes `[0..16)` of each sector pass through; bytes `[16..2048)` are decrypted
/// with `read_data_key` and a fresh chain (IV = [`CONTENT_IV`], the same constant
/// content uses). Mirrors libaacs `aacs.c:_decrypt_unit_bus` (read as a reference
/// oracle only — no code copied, spec 08 §8.6).
pub fn bus_decrypt_unit(read_data_key: &Block, unit: &[u8]) -> Result<Vec<u8>, ContentError> {
    if unit.len() != ALIGNED_UNIT_LEN {
        return Err(ContentError::BadUnitLength(unit.len()));
    }
    let mut out = unit.to_vec();
    let mut s = 0;
    while s < ALIGNED_UNIT_LEN {
        // Sector head [0..16) is not bus-encrypted; CBC-decrypt [16..2048).
        let mut prev = CONTENT_IV;
        let mut i = s + SEED_LEN;
        while i < s + SECTOR_LEN {
            let mut ct = [0u8; 16];
            ct.copy_from_slice(&out[i..i + 16]);
            let dec = aes_128d(read_data_key, &ct);
            for j in 0..16 {
                out[i + j] = dec[j] ^ prev[j];
            }
            prev = ct;
            i += 16;
        }
        s += SECTOR_LEN;
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

    // ── bus decryption (spec 11 §11.4.4, BEE discs) ──────────────────────────
    // Synthetic, key/content-free per Rule 4. The bus layer is its own AES-128-CBC
    // over [16..2048) of each 2048-B sector (read_data_key, IV = CONTENT_IV); the
    // first 16 B of each sector pass through. The real-disc proof (Turbo UHD, the
    // unique scraped read_data_key taking units 0/32→31/32) lives outside the repo.

    /// Test-only bus *encrypt* (inverse of `bus_decrypt_unit`) to build vectors.
    fn bus_encrypt_unit(read_data_key: &Block, unit: &[u8]) -> Vec<u8> {
        let mut out = unit.to_vec();
        let mut s = 0;
        while s < ALIGNED_UNIT_LEN {
            let mut prev = CONTENT_IV;
            let mut i = s + 16;
            while i < s + 2048 {
                let mut x = [0u8; 16];
                for j in 0..16 {
                    x[j] = out[i + j] ^ prev[j];
                }
                let ct = aes_128e(read_data_key, &x);
                out[i..i + 16].copy_from_slice(&ct);
                prev = ct;
                i += 16;
            }
            s += 2048;
        }
        out
    }

    #[test]
    fn bus_decrypt_round_trips_and_preserves_sector_heads() {
        let rdk = h("0F0E0D0C0B0A09080706050403020100");
        // Distinctive plaintext: sector-head markers + a ramp body.
        let mut pt = vec![0u8; ALIGNED_UNIT_LEN];
        for (i, b) in pt.iter_mut().enumerate() {
            *b = (i % 251) as u8;
        }
        let ct = bus_encrypt_unit(&rdk, &pt);
        // Heads of each 2048-B sector are NOT bus-encrypted.
        for s in (0..ALIGNED_UNIT_LEN).step_by(2048) {
            assert_eq!(
                ct[s..s + 16],
                pt[s..s + 16],
                "sector {s} head must pass through"
            );
        }
        // Bodies ARE changed.
        assert_ne!(ct[16..2048], pt[16..2048], "sector body must be encrypted");
        // And bus_decrypt_unit inverts it exactly.
        let back = bus_decrypt_unit(&rdk, &ct).unwrap();
        assert_eq!(back, pt);
    }

    #[test]
    fn bus_decrypt_is_key_sensitive() {
        let pt = vec![0x5Au8; ALIGNED_UNIT_LEN];
        let ct = bus_encrypt_unit(&h("000102030405060708090A0B0C0D0E0F"), &pt);
        // Wrong key must not recover the plaintext body.
        let wrong = bus_decrypt_unit(&h("0102030405060708090A0B0C0D0E0F10"), &ct).unwrap();
        assert_ne!(wrong[16..2048], pt[16..2048]);
    }

    #[test]
    fn bus_decrypt_rejects_wrong_length() {
        let k = h("00112233445566778899AABBCCDDEEFF");
        assert_eq!(
            bus_decrypt_unit(&k, &[0u8; 100]),
            Err(ContentError::BadUnitLength(100))
        );
    }

    // Full thick-reader pipeline on a real BEE disc (Turbo UHD), spec 11 §11.4.4.
    // Proven at RE time: a captured bus-encrypted READ(10) unit, the scraped
    // read_data_key, and the keydb unit key together yield valid MPEG-TS (31/32 —
    // packet 0's sync sits under the clear seed). The inputs are secret/copyrighted
    // (Rule 4) so they load from $FREEBLUE_FIXTURES and this is skipped when absent.
    #[test]
    #[ignore = "needs $FREEBLUE_FIXTURES: bus-encrypted unit + read_data_key + unit key (spec 11 §11.4.4)"]
    fn bus_then_content_decrypts_bee_unit_to_valid_ts() {
        // TODO(build-host): load fixtures/turbo_bus_unit.bin, read_data_key, unit_key:
        //   let stripped = bus_decrypt_unit(&read_data_key, &bus_unit).unwrap();
        //   let pt = decrypt_aligned_unit(&unit_key, &stripped).unwrap();
        //   assert!(ts_sync_score(&pt) >= 31);
    }
}
