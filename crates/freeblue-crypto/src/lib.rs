//! AACS cryptographic primitives for `freeblue`.
//!
//! All-original, pure-Rust (RustCrypto `aes`); nothing is copied from
//! `libaacs` — see [`specs/08-reference-implementation.md`] §8.6. Each function
//! is cited to the spec section that defines it, and the `[Disc]`-verified
//! formulas (proven byte-for-byte against real discs, spec 09 §9.10.1) are
//! marked as such.
//!
//! Primitives:
//! - [`aes_128e`] / [`aes_128d`] — AES-128 single-block (FIPS-197).
//! - [`aes_g`]  — AACS one-way function `AES-128D(k,d) ⊕ d`  (spec 02 §2.3.2).
//! - [`aes_g3`] — Triple AES Generator, SD-tree node derivation (spec 02 §2.3.3).

use aes::cipher::generic_array::GenericArray;
use aes::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes128;

/// A 128-bit AACS key / block.
pub type Block = [u8; 16];

/// AES-128 ECB single-block **encrypt** (FIPS-197).
pub fn aes_128e(key: &Block, data: &Block) -> Block {
    let cipher = Aes128::new(GenericArray::from_slice(key));
    let mut b = GenericArray::clone_from_slice(data);
    cipher.encrypt_block(&mut b);
    let mut out = [0u8; 16];
    out.copy_from_slice(&b);
    out
}

/// AES-128 ECB single-block **decrypt** (FIPS-197).
pub fn aes_128d(key: &Block, data: &Block) -> Block {
    let cipher = Aes128::new(GenericArray::from_slice(key));
    let mut b = GenericArray::clone_from_slice(data);
    cipher.decrypt_block(&mut b);
    let mut out = [0u8; 16];
    out.copy_from_slice(&b);
    out
}

#[inline]
fn xor16(a: Block, b: &Block) -> Block {
    let mut o = [0u8; 16];
    for i in 0..16 {
        o[i] = a[i] ^ b[i];
    }
    o
}

/// AES-based one-way function **AES-G** (spec 02 §2.3.2, [CCE §2.1.3]):
///
/// ```text
/// AES-G(k, d) = AES-128D(k, d) XOR d
/// ```
///
/// `[Disc]`-verified: `Kvu = AES-G(Km, IDv)` reproduced the real VUK on both a
/// v1 (GoT, MKBv63) and a v2 (MKBv82) disc (spec 06 §6.5.2).
pub fn aes_g(key: &Block, data: &Block) -> Block {
    xor16(aes_128d(key, data), data)
}

/// The three 128-bit outputs of [`aes_g3`] for one node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ag3 {
    /// Subsidiary Device Key for the **left** child (ignored at a leaf).
    pub left: Block,
    /// The **Processing Key** (the middle output).
    pub processing_key: Block,
    /// Subsidiary Device Key for the **right** child (ignored at a leaf).
    pub right: Block,
}

/// The AES-G3 seed constant `s0` (spec 02 §2.3.3, [CCE §3.2.2]); byte-identical
/// to `libaacs`'s `_aesg3` seed.
pub const AES_G3_SEED: Block = [
    0x7B, 0x10, 0x3C, 0x5D, 0xCB, 0x08, 0xC4, 0xE5, 0x1A, 0x27, 0xB0, 0x17, 0x99, 0x05, 0x3B, 0xD9,
];

/// **AES-G3** — Triple AES Generator (spec 02 §2.3.3, [CCE §3.2.2]).
///
/// Runs three rounds over `s0, s0+1, s0+2` (incrementing the seed's last byte),
/// each round `AES-128D(k, sN) ⊕ sN`, producing (left child, **processing
/// key**, right child). Used to descend the subset-difference tree (spec 03).
pub fn aes_g3(device_key: &Block) -> Ag3 {
    let round = |inc: u8| -> Block {
        let mut s = AES_G3_SEED;
        s[15] = s[15].wrapping_add(inc);
        xor16(aes_128d(device_key, &s), &s)
    };
    Ag3 {
        left: round(0),
        processing_key: round(1),
        right: round(2),
    }
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
    fn aes128_fips197_vector() {
        // FIPS-197 Appendix B / C.1 known answer.
        let k = h("000102030405060708090A0B0C0D0E0F");
        let p = h("00112233445566778899AABBCCDDEEFF");
        let c = h("69C4E0D86A7B0430D8CDB78070B4C55A");
        assert_eq!(aes_128e(&k, &p), c);
        assert_eq!(aes_128d(&k, &c), p);
    }

    #[test]
    fn aes_g_kat() {
        // Regression vector (deterministic from the AES-G definition).
        let k = h("000102030405060708090A0B0C0D0E0F");
        let d = h("00112233445566778899AABBCCDDEEFF");
        assert_eq!(aes_g(&k, &d), h("763B78864D7C7EEB674233F88B4D4427"));
    }

    #[test]
    fn aes_g3_kat() {
        // Regression vector for dk = 0102..0f10.
        let dk = h("0102030405060708090A0B0C0D0E0F10");
        let out = aes_g3(&dk);
        assert_eq!(out.left, h("7163BB2F4DD7583ABE819981B92D5A8E"));
        assert_eq!(out.processing_key, h("84F2F664DDB67A5E29F9F14F9CCD0295"));
        assert_eq!(out.right, h("CCF519A4BF33183813D2F5B376F48170"));
    }

    #[test]
    fn aes_g3_seed_is_canonical() {
        assert_eq!(
            hex::encode_upper(AES_G3_SEED),
            "7B103C5DCB08C4E51A27B01799053BD9"
        );
    }
}
