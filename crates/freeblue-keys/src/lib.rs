//! Key sources: `KEYDB.cfg` parsing and the device-key set (spec 06 §6.5).
//!
//! Format is **verified** against a real 182k-entry community keydb (spec 06
//! §6.5.1). **No real keys are ever committed** (spec 10 §10.4) — callers supply
//! a path at runtime; tests use synthetic (fake) keys.
//!
//! Per-disc entry (verified semantics, spec 06 §6.5.1):
//! `<disc-id> = (title) | D <date> | M <media key> | I <volume id> |
//!  V <volume unique key> | U n-<unit key> ; MKBvNN …`
//! Global records:
//! `| DK | DEVICE_KEY .. | DEVICE_NODE .. | KEY_UV .. | KEY_U_MASK_SHIFT ..`,
//! `| PK | <processing key> ; MKBvNN`,
//! `| HC | HOST_PRIV_KEY .. | HOST_CERT ..`.

use freeblue_crypto::Block;
use std::collections::HashMap;
use zeroize::Zeroize;

/// External Volume-ID ingestion (`discatt.dat` / libaacs `vid` cache), spec 06 §6.8.
pub mod external_vid;

/// 20-byte AACS disc identifier (the key of a per-disc entry).
pub type DiscId = [u8; 20];

/// Per-disc key material from a `KEYDB.cfg` entry. Secrets zeroized on drop.
#[derive(Default)]
pub struct DiscKeys {
    /// `M` — Media Key (Km).
    pub media_key: Option<Block>,
    /// `I` — Volume ID (IDv).
    pub volume_id: Option<Block>,
    /// `V` — Volume Unique Key (Kvu).
    pub volume_unique_key: Option<Block>,
    /// `U` — per-CPS-unit keys (Kcu), in unit order.
    pub unit_keys: Vec<Block>,
}

impl Drop for DiscKeys {
    fn drop(&mut self) {
        for k in [
            &mut self.media_key,
            &mut self.volume_id,
            &mut self.volume_unique_key,
        ] {
            if let Some(b) = k.as_mut() {
                b.zeroize();
            }
        }
        for k in &mut self.unit_keys {
            k.zeroize();
        }
    }
}

/// A processing key scoped to an MKB-version range (`PK` record).
pub struct ProcessingKey {
    pub key: Block,
    /// e.g. "MKBv63" or "MKBv64-MKBv65".
    pub mkb_versions: String,
}

/// A device key with its subset-difference tree placement (`DK` record).
pub struct DeviceKey {
    pub device_key: Block,
    pub device_node: Vec<u8>,
    pub key_uv: Vec<u8>,
    pub key_u_mask_shift: Vec<u8>,
}

/// A loaded key database (global keys + per-disc lookups).
#[derive(Default)]
pub struct KeyDb {
    discs: HashMap<DiscId, DiscKeys>,
    pub processing_keys: Vec<ProcessingKey>,
    pub device_keys: Vec<DeviceKey>,
    pub host_priv_key: Option<Vec<u8>>,
    pub host_cert: Option<Vec<u8>>,
}

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("malformed KEYDB.cfg at line {0}: {1}")]
    Parse(usize, &'static str),
    #[error("bad hex in key material at line {0}")]
    Hex(usize),
}

/// Strip an optional `0x`/`0X` prefix.
fn strip0x(s: &str) -> &str {
    s.strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s)
}

/// Decode hex (with optional `0x`) to bytes.
fn hex_vec(s: &str) -> Option<Vec<u8>> {
    hex::decode(strip0x(s.trim())).ok()
}

/// Decode hex to exactly a 16-byte [`Block`].
fn hex_block(s: &str) -> Option<Block> {
    let v = hex_vec(s)?;
    if v.len() != 16 {
        return None;
    }
    let mut b = [0u8; 16];
    b.copy_from_slice(&v);
    Some(b)
}

impl KeyDb {
    /// Parse a `KEYDB.cfg` (spec 06 §6.5). Lenient: unknown record shapes are
    /// skipped (the real keydb is large and evolving); only malformed hex in a
    /// recognized field is an error.
    pub fn parse(text: &str) -> Result<Self, KeyError> {
        let mut db = KeyDb::default();
        for (n, raw) in text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if line.starts_with("0x") || line.starts_with("0X") {
                db.parse_disc_entry(line, n)?;
            } else if line.starts_with('|') {
                db.parse_global_record(line, n)?;
            }
            // else: unrecognized line shape — skip leniently.
        }
        Ok(db)
    }

    fn parse_disc_entry(&mut self, line: &str, n: usize) -> Result<(), KeyError> {
        let (idpart, rest) = line
            .split_once('=')
            .ok_or(KeyError::Parse(n, "disc entry missing '='"))?;
        let idv = hex_vec(idpart).ok_or(KeyError::Hex(n))?;
        if idv.len() != 20 {
            return Err(KeyError::Parse(n, "disc-id is not 20 bytes"));
        }
        let mut id = [0u8; 20];
        id.copy_from_slice(&idv);

        // segs[0] = title; then alternating (tag, value) from index 1.
        let segs: Vec<&str> = rest.split('|').map(str::trim).collect();
        let mut dk = DiscKeys::default();
        let mut i = 1;
        while i + 1 < segs.len() {
            let tag = segs[i];
            // strip a trailing "; note" from the value.
            let val = segs[i + 1].split(';').next().unwrap_or("").trim();
            match tag {
                "M" => dk.media_key = hex_block(val).or(dk.media_key),
                "I" => dk.volume_id = hex_block(val).or(dk.volume_id),
                "V" => dk.volume_unique_key = hex_block(val).or(dk.volume_unique_key),
                "U" => {
                    for uk in val.split_whitespace() {
                        // "n-0xHEX" -> take the part after the last '-'.
                        if let Some(h) = uk.rsplit('-').next() {
                            if let Some(b) = hex_block(h) {
                                dk.unit_keys.push(b);
                            }
                        }
                    }
                }
                _ => {} // D (date), etc.
            }
            i += 2;
        }
        self.discs.insert(id, dk);
        Ok(())
    }

    fn parse_global_record(&mut self, line: &str, _n: usize) -> Result<(), KeyError> {
        let segs: Vec<&str> = line.split('|').map(str::trim).collect();
        // segs[0] == "" (leading '|'), segs[1] == record tag.
        match segs.get(1).copied().unwrap_or("") {
            "PK" => {
                let seg = segs.get(2).copied().unwrap_or("");
                let (val, note) = match seg.split_once(';') {
                    Some((v, nt)) => (v.trim(), nt.trim()),
                    None => (seg, ""),
                };
                if let Some(key) = hex_block(val) {
                    self.processing_keys.push(ProcessingKey {
                        key,
                        mkb_versions: note.to_string(),
                    });
                }
            }
            "DK" => {
                let (mut k, mut node, mut uv, mut mask) = (None, None, None, None);
                for seg in &segs[2..] {
                    let mut it = seg.split_whitespace();
                    let name = it.next().unwrap_or("");
                    let v = it.next().unwrap_or("");
                    match name {
                        "DEVICE_KEY" => k = hex_block(v),
                        "DEVICE_NODE" => node = hex_vec(v),
                        "KEY_UV" => uv = hex_vec(v),
                        "KEY_U_MASK_SHIFT" => mask = hex_vec(v),
                        _ => {}
                    }
                }
                if let Some(device_key) = k {
                    self.device_keys.push(DeviceKey {
                        device_key,
                        device_node: node.unwrap_or_default(),
                        key_uv: uv.unwrap_or_default(),
                        key_u_mask_shift: mask.unwrap_or_default(),
                    });
                }
            }
            "HC" => {
                for seg in &segs[2..] {
                    let mut it = seg.split_whitespace();
                    let name = it.next().unwrap_or("");
                    let v = it.next().unwrap_or("");
                    match name {
                        "HOST_PRIV_KEY" => self.host_priv_key = hex_vec(v),
                        "HOST_CERT" => self.host_cert = hex_vec(v),
                        _ => {}
                    }
                }
            }
            _ => {} // unknown record — skip.
        }
        Ok(())
    }

    /// Look up a disc by its 20-byte AACS disc-id.
    pub fn disc(&self, id: &DiscId) -> Option<&DiscKeys> {
        self.discs.get(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic fixture — all keys are FAKE (sequential bytes). Safe to commit.
    const FIXTURE: &str = "\
; # AACS keydb.cfg synthetic test fixture
# a comment line

0x0102030405060708090A0B0C0D0E0F1011121314 = TEST DISC (A Test Disc) | D | 2024-01-01 | M | 0x000102030405060708090A0B0C0D0E0F | I | 0x101112131415161718191A1B1C1D1E1F | V | 0x202122232425262728292A2B2C2D2E2F | U | 1-0x303132333435363738393A3B3C3D3E3F ; MKBv63/FindVUK 1.0 (BD)
| PK | 0x404142434445464748494A4B4C4D4E4F ; MKBv63
| DK | DEVICE_KEY 0x505152535455565758595A5B5C5D5E5F | DEVICE_NODE 0x0001 | KEY_UV 0x00000002 | KEY_U_MASK_SHIFT 0x03
| HC | HOST_PRIV_KEY 0x606162636465666768696A6B6C6D6E6F70717273 | HOST_CERT 0x8081
";

    fn b(s: &str) -> Block {
        let v = hex::decode(s).unwrap();
        let mut a = [0u8; 16];
        a.copy_from_slice(&v);
        a
    }
    fn id(s: &str) -> DiscId {
        let v = hex::decode(s).unwrap();
        let mut a = [0u8; 20];
        a.copy_from_slice(&v);
        a
    }

    #[test]
    fn parses_global_records() {
        let db = KeyDb::parse(FIXTURE).unwrap();
        assert_eq!(db.processing_keys.len(), 1);
        assert_eq!(
            db.processing_keys[0].key,
            b("404142434445464748494A4B4C4D4E4F")
        );
        assert_eq!(db.processing_keys[0].mkb_versions, "MKBv63");

        assert_eq!(db.device_keys.len(), 1);
        let dk = &db.device_keys[0];
        assert_eq!(dk.device_key, b("505152535455565758595A5B5C5D5E5F"));
        assert_eq!(dk.device_node, vec![0x00, 0x01]);
        assert_eq!(dk.key_uv, vec![0, 0, 0, 2]);
        assert_eq!(dk.key_u_mask_shift, vec![3]);

        assert_eq!(db.host_priv_key.as_ref().unwrap().len(), 20);
        assert_eq!(db.host_cert.as_deref(), Some(&[0x80u8, 0x81][..]));
    }

    #[test]
    fn parses_per_disc_entry() {
        let db = KeyDb::parse(FIXTURE).unwrap();
        let d = db
            .disc(&id("0102030405060708090A0B0C0D0E0F1011121314"))
            .expect("disc present");
        assert_eq!(d.media_key.unwrap(), b("000102030405060708090A0B0C0D0E0F"));
        assert_eq!(d.volume_id.unwrap(), b("101112131415161718191A1B1C1D1E1F"));
        assert_eq!(
            d.volume_unique_key.unwrap(),
            b("202122232425262728292A2B2C2D2E2F")
        );
        assert_eq!(d.unit_keys, vec![b("303132333435363738393A3B3C3D3E3F")]);
    }

    #[test]
    fn skips_comments_and_blanks() {
        let db = KeyDb::parse("; c\n# c\n\n   \n").unwrap();
        assert!(db.processing_keys.is_empty() && db.device_keys.is_empty());
    }

    #[test]
    fn unknown_disc_is_none() {
        let db = KeyDb::parse(FIXTURE).unwrap();
        assert!(db.disc(&[0xFF; 20]).is_none());
    }

    /// Structural check against the REAL community keydb (no key values asserted,
    /// per spec 10 §10.4). Loads `$FREEBLUE_FIXTURES/keydb.cfg`; skips if absent.
    #[test]
    #[ignore = "needs $FREEBLUE_FIXTURES/keydb.cfg (the real community keydb)"]
    fn parses_real_keydb_at_scale() {
        let dir = std::env::var("FREEBLUE_FIXTURES").expect("FREEBLUE_FIXTURES unset");
        let text = std::fs::read_to_string(format!("{dir}/keydb.cfg")).expect("read keydb.cfg");
        let db = KeyDb::parse(&text).expect("parse real keydb");
        // The real DB has 180k+ disc entries and a handful of global records.
        assert!(db.discs.len() > 100_000, "discs = {}", db.discs.len());
        assert!(!db.processing_keys.is_empty(), "no PK records");
        assert!(!db.device_keys.is_empty(), "no DK records");
        assert!(db.host_priv_key.is_some(), "no HC record");
        // Every entry should at least carry a VUK or a unit key (decryptable).
        let usable = db
            .discs
            .values()
            .filter(|d| d.volume_unique_key.is_some() || !d.unit_keys.is_empty())
            .count();
        assert!(
            usable as f64 > db.discs.len() as f64 * 0.9,
            "usable = {usable}"
        );
    }
}
