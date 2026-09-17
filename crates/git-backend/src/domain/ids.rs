use std::fmt;

use serde::{Deserialize, Serialize};

macro_rules! id_newtype {
    ($(#[$meta:meta])* $name:ident, $inner:ty) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub $inner);

        impl From<$inner> for $name {
            fn from(value: $inner) -> Self {
                Self(value)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

id_newtype!(
    /// Stable handle for an opened repository within this process.
    RepoId,
    u64
);
id_newtype!(
    /// Stable handle for a long-running operation (diff stream, fetch, ...).
    OperationId,
    u64
);
id_newtype!(
    /// Monotonic counter bumped by every mutation or external change.
    Generation,
    u64
);
id_newtype!(
    /// Point-in-time token captured when reading repository state.
    SnapshotId,
    u64
);
id_newtype!(
    /// Absolute diff row position inside one operation.
    RowIndex,
    u32
);

impl Generation {
    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    pub fn matches_expected(
        &self,
        expected: Option<Generation>,
    ) -> Result<(), crate::error::GitError> {
        match expected {
            Some(expected) if expected != *self => Err(crate::error::GitError::StaleSnapshot {
                expected: expected.0,
                current: self.0,
            }),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShaKind {
    Sha1,
    Sha256,
}

/// A git object id. Stored as raw bytes so SHA-1 and SHA-256 repositories both work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectId {
    bytes: [u8; 32],
    len: u8,
}

impl ObjectId {
    pub const MAX_LEN: usize = 32;

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, crate::error::GitError> {
        if bytes.len() != 20 && bytes.len() != 32 {
            return Err(crate::error::GitError::invalid_input(format!(
                "object id must be 20 or 32 bytes, got {}",
                bytes.len()
            )));
        }
        let mut buf = [0u8; 32];
        buf[..bytes.len()].copy_from_slice(bytes);
        Ok(Self {
            bytes: buf,
            len: bytes.len() as u8,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    pub fn kind(&self) -> ShaKind {
        if self.len == 32 {
            ShaKind::Sha256
        } else {
            ShaKind::Sha1
        }
    }

    pub fn hex(&self) -> String {
        let mut out = String::with_capacity(self.len as usize * 2);
        for byte in self.as_bytes() {
            use std::fmt::Write;
            let _ = write!(out, "{byte:02x}");
        }
        out
    }

    pub fn short_hex(&self) -> String {
        let full = self.hex();
        full.chars().take(7).collect()
    }

    pub fn is_zero(&self) -> bool {
        self.as_bytes().iter().all(|b| *b == 0)
    }
}

impl From<gix::ObjectId> for ObjectId {
    fn from(value: gix::ObjectId) -> Self {
        Self::from_bytes(value.as_bytes()).expect("gix object ids are 20 or 32 bytes")
    }
}

impl TryFrom<ObjectId> for gix::ObjectId {
    type Error = crate::error::GitError;

    fn try_from(value: ObjectId) -> std::result::Result<Self, Self::Error> {
        gix::ObjectId::try_from(value.as_bytes())
            .map_err(|e| crate::error::GitError::internal(e.to_string()))
    }
}

impl fmt::Display for ObjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.hex())
    }
}

impl Serialize for ObjectId {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.hex())
    }
}

impl<'de> Deserialize<'de> for ObjectId {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let bytes = hex_decode(&s).map_err(serde::de::Error::custom)?;
        Self::from_bytes(&bytes).map_err(serde::de::Error::custom)
    }
}

pub fn hex_decode(input: &str) -> std::result::Result<Vec<u8>, String> {
    if input.len() % 2 != 0 {
        return Err("hex string has odd length".into());
    }
    let mut out = Vec::with_capacity(input.len() / 2);
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_val(bytes[i]).ok_or("invalid hex digit")?;
        let lo = hex_val(bytes[i + 1]).ok_or("invalid hex digit")?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hex_val(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrips_hex() {
        let raw = [7u8; 20];
        let oid = ObjectId::from_bytes(&raw).unwrap();
        assert_eq!(oid.hex(), "0707070707070707070707070707070707070707");
        assert_eq!(oid.kind(), ShaKind::Sha1);
        let decoded: ObjectId = serde_json::from_value(serde_json::json!(oid.hex())).unwrap();
        assert_eq!(decoded, oid);
    }

    #[test]
    fn rejects_bad_lengths_and_digits() {
        assert!(ObjectId::from_bytes(&[1u8; 3]).is_err());
        assert!(hex_decode("zz").is_err());
        assert!(hex_decode("abc").is_err());
    }

    #[test]
    fn sha256_ids_supported() {
        let raw = [9u8; 32];
        let oid = ObjectId::from_bytes(&raw).unwrap();
        assert_eq!(oid.kind(), ShaKind::Sha256);
        assert_eq!(oid.as_bytes().len(), 32);
    }

    #[test]
    fn generation_staleness_check() {
        let generation = Generation(5);
        assert!(generation.matches_expected(None).is_ok());
        assert!(generation.matches_expected(Some(Generation(5))).is_ok());
        let err = generation
            .matches_expected(Some(Generation(4)))
            .unwrap_err();
        assert_eq!(err.code(), "staleSnapshot");
        assert_eq!(generation.next(), Generation(6));
    }

    #[test]
    fn converts_from_gix() {
        let gix_id = gix::ObjectId::from_hex(b"0123456789012345678901234567890123456789").unwrap();
        let oid = ObjectId::from(gix_id);
        assert_eq!(oid.hex(), "0123456789012345678901234567890123456789");
    }
}
