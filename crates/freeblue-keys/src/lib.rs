//! Key sources: `KEYDB.cfg` parsing and the device-key set (spec 06 §6.5).
//!
//! Format is **verified** against a real 182k-entry community keydb (spec 06
//! §6.5.1); the parser below is the first in-repo TDD task. **No keys are ever
//! committed** (spec 10 §10.4) — callers supply a path at runtime.
//!
//! Per-disc entry fields (verified semantics, spec 06 §6.5.1):
//! `<disc-id> = (title) | D <date> | M <media key> | I <volume id> |
//!  V <volume unique key> | U n-<unit key> ; MKBvNN …`
//! Global records: `| DK | DEVICE_KEY .. DEVICE_NODE .. KEY_UV .. KEY_U_MASK_SHIFT`,
//! `| PK | <processing key> ; MKBvNN`, `| HC | HOST_PRIV_KEY .. HOST_CERT ..`.

use freeblue_crypto::Block;
use zeroize::Zeroize;

/// Per-disc key material from a `KEYDB.cfg` entry. Zeroized on drop.
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
        if let Some(k) = self.media_key.as_mut() {
            k.zeroize();
        }
        if let Some(k) = self.volume_unique_key.as_mut() {
            k.zeroize();
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

/// A loaded key database (global keys + per-disc lookups).
#[derive(Default)]
pub struct KeyDb {
    pub processing_keys: Vec<ProcessingKey>,
    // device_keys: Vec<DeviceKey>,   // DK records (spec 06 §6.5) — TODO
    // per-disc map: disc-id -> DiscKeys                          — TODO
}

#[derive(Debug, thiserror::Error)]
pub enum KeyError {
    #[error("malformed KEYDB.cfg at line {0}")]
    Parse(usize),
    #[error("bad hex in key material")]
    Hex(#[from] hex::FromHexError),
}

impl KeyDb {
    /// Parse a `KEYDB.cfg` (spec 06 §6.5). First in-repo TDD task.
    pub fn parse(_text: &str) -> Result<Self, KeyError> {
        todo!("spec 06 §6.5: parse per-disc D/M/I/V/U entries + DK/PK/HC records")
    }

    /// Look up a disc by its 20-byte AACS disc-id.
    pub fn disc(&self, _disc_id: &[u8; 20]) -> Option<DiscKeys> {
        todo!("spec 06 §6.5: per-disc lookup")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    #[ignore = "TDD: implement KeyDb::parse against a synthetic KEYDB.cfg fixture (spec 06 §6.5)"]
    fn parses_disc_entry_fields() {}
}
