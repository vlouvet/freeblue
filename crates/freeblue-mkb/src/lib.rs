//! AACS Media Key Block (MKB) parsing + media-key derivation (spec 03).
//!
//! The processing-key -> media-key path is `[Disc]`-verified: on GoT (BD
//! MKBv63), [`derive_media_key`] reproduced the disc's keydb `M` field exactly
//! and validated against the `0x81` Verify record (spec 03 §3.4.2). Algorithm
//! and record layout match `libaacs` `_validate_pk` / `_calc_mk_pks`.

use freeblue_crypto::{aes_128d, Block};

/// Record type IDs we care about (spec 03 §3.3, §3.3.1).
pub mod record {
    pub const TYPE_AND_VERSION: u8 = 0x10;
    pub const EXPLICIT_SUBSET_DIFFERENCE: u8 = 0x04;
    pub const MEDIA_KEY_DATA: u8 = 0x05;
    pub const VERIFY_MEDIA_KEY_V1: u8 = 0x81; // AACS v1
    pub const VERIFY_MEDIA_KEY_V2: u8 = 0x86; // AACS 2.0 (spec 03 §3.3.1)
    pub const END_OF_MKB: u8 = 0x00;
}

/// AACS media-key verification constant (spec 03 §3.4.2, [CCE §3.2.5.4]):
/// `[AES-128D(Km, Dv)]msb64 == 0x0123456789ABCDEF`.
pub const VERIFY_CONST: [u8; 8] = [0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF];

#[derive(Debug, PartialEq, Eq)]
pub enum MkbError {
    Truncated,
    MissingRecord(u8),
    /// No processing key produced a media key that validates (revoked, or wrong
    /// key set for this MKB — spec 03 §3.6).
    NoValidProcessingKey,
}

/// A parsed MKB: a list of `(type, body)` records. `body` excludes the 4-byte
/// `type(1) + length(3)` header (libaacs `_simple_record` semantics).
pub struct Mkb<'a> {
    records: Vec<(u8, &'a [u8])>,
}

impl<'a> Mkb<'a> {
    /// Parse the TLV record stream (spec 03 §3.3). Each record is
    /// `type:u8 | length:u24-be | data` where `length` covers the whole record.
    pub fn parse(data: &'a [u8]) -> Result<Self, MkbError> {
        let mut records = Vec::new();
        let mut off = 0usize;
        while off + 4 <= data.len() {
            let t = data[off];
            let len = u32::from_be_bytes([0, data[off + 1], data[off + 2], data[off + 3]]) as usize;
            if t == record::END_OF_MKB || len == 0 {
                records.push((t, &data[off..off]));
                break;
            }
            if off + len > data.len() || len < 4 {
                return Err(MkbError::Truncated);
            }
            records.push((t, &data[off + 4..off + len]));
            off += len;
        }
        Ok(Mkb { records })
    }

    fn body(&self, ty: u8) -> Option<&'a [u8]> {
        self.records.iter().find(|(t, _)| *t == ty).map(|(_, b)| *b)
    }

    /// MKB version (from the Type-and-Version record, last 4 bytes of its body).
    pub fn version(&self) -> Option<u32> {
        let b = self.body(record::TYPE_AND_VERSION)?;
        if b.len() < 8 {
            return None;
        }
        Some(u32::from_be_bytes([b[4], b[5], b[6], b[7]]))
    }

    /// The 16-byte media-key Verification Data `Dv` (v1 `0x81` or v2 `0x86`).
    pub fn verify_data(&self) -> Option<Block> {
        let b = self
            .body(record::VERIFY_MEDIA_KEY_V1)
            .or_else(|| self.body(record::VERIFY_MEDIA_KEY_V2))?;
        if b.len() < 16 {
            return None;
        }
        let mut dv = [0u8; 16];
        dv.copy_from_slice(&b[..16]);
        Some(dv)
    }

    /// True iff `mk` is the correct media key for this MKB (spec 03 §3.4.2).
    pub fn verify_media_key(&self, mk: &Block) -> bool {
        match self.verify_data() {
            Some(dv) => aes_128d(mk, &dv)[..8] == VERIFY_CONST,
            None => false,
        }
    }

    /// Derive the Media Key from one or more processing keys (spec 03 §3.4.2).
    ///
    /// For each subset `a`: `mk = AES-128D(pk, cvalue[a])`, then
    /// `mk[12..16] ^= uv[a]` (the 4 bytes at offset `1 + a*5` of the
    /// subset-difference record), then verify. Returns the first match.
    pub fn derive_media_key(&self, processing_keys: &[Block]) -> Result<Block, MkbError> {
        let subdiff = self
            .body(record::EXPLICIT_SUBSET_DIFFERENCE)
            .ok_or(MkbError::MissingRecord(record::EXPLICIT_SUBSET_DIFFERENCE))?;
        let cvalues = self
            .body(record::MEDIA_KEY_DATA)
            .ok_or(MkbError::MissingRecord(record::MEDIA_KEY_DATA))?;
        let dv = self
            .verify_data()
            .ok_or(MkbError::MissingRecord(record::VERIFY_MEDIA_KEY_V1))?;

        // Count UV entries: 5-byte records until the first byte has bits 0xC0 set.
        let mut num_uvs = 0usize;
        let mut r = 0usize;
        while r + 5 <= subdiff.len() {
            if subdiff[r] & 0xC0 != 0 {
                break;
            }
            num_uvs += 1;
            r += 5;
        }

        for pk in processing_keys {
            for a in 0..num_uvs {
                let cv_off = a * 16;
                let uv_off = 1 + a * 5;
                if cv_off + 16 > cvalues.len() || uv_off + 4 > subdiff.len() {
                    break;
                }
                let mut cv = [0u8; 16];
                cv.copy_from_slice(&cvalues[cv_off..cv_off + 16]);
                let mut mk = aes_128d(pk, &cv);
                for k in 0..4 {
                    mk[12 + k] ^= subdiff[uv_off + k];
                }
                if aes_128d(&mk, &dv)[..8] == VERIFY_CONST {
                    return Ok(mk);
                }
            }
        }
        Err(MkbError::NoValidProcessingKey)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_truncated_record() {
        // type 0x10, length 0xFFFFFF (way past end) -> Truncated.
        let data = [0x10, 0xFF, 0xFF, 0xFF, 0x00];
        assert_eq!(Mkb::parse(&data).err(), Some(MkbError::Truncated));
    }

    #[test]
    fn parse_walks_records_and_reads_version() {
        // Type+Version (len 12, version=0x52=82), then End-of-MKB.
        let mut data = vec![0x10, 0x00, 0x00, 0x0C];
        data.extend_from_slice(&[0x48, 0x14, 0x10, 0x03, 0x00, 0x00, 0x00, 0x52]);
        data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // End-of-MKB
        let mkb = Mkb::parse(&data).unwrap();
        assert_eq!(mkb.version(), Some(82));
    }

    // The full derive_media_key path is `[Disc]`-verified (spec 03 §3.4.2) but
    // needs a real MKB + processing key, which are not committed (spec 10 §10.4).
    // Loaded from $FREEBLUE_FIXTURES on the build host (spec 09 §9.6).
    #[test]
    #[ignore = "needs $FREEBLUE_FIXTURES: real MKB_RO.inf + processing key (spec 09 §9.6)"]
    fn derive_media_key_matches_keydb() {
        // TODO(build-host): parse fixtures/got_mkb.inf, derive with the MKBv63
        // processing key, assert == the disc's keydb `M` and verify_media_key().
    }
}
